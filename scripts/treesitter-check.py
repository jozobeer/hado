#!/usr/bin/env python3
"""highlight-treesitter の門番本体（scripts/verify-treesitter.sh から呼ぶ）。

引数は setup script が返した tree-sitter CLI と Neovim の絶対パス。PATH 上の
同名ツールは見ない。

検査:
  1. 受理コーパス全件と stdlib/*.hd が ERROR も MISSING も出さずに parse できる
  2. 文字列と文字のエスケープ付きリテラルが parse できる
  3. 宣言形と第7節の優先順位の golden が node 列と一致する
  4. Neovim が hado の filetype と parser と highlights query を読み、約束した
     字句のトークン範囲に capture の highlight group の extmark を付ける
"""
import fcntl
import os
import pty
import select
import shutil
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
PKG = REPO / 'editors' / 'tree-sitter-hado'
GOLDEN = PKG / 'test' / 'golden'
DUMP = REPO / 'scripts' / 'treesitter_dump.py'
HL_LUA = REPO / 'scripts' / 'treesitter-hl.lua'

# 宣言形（付録C の形ごとに 1 本）と第7節の優先順位の境界。golden の名前は
# 門番が期待する探針の一覧で、ディスク上の *.hd / *.tree と一致していなければ
# ならない。件数はこの一覧とディスクの両方から導く。
GOLDEN_NAMES = (
    # 宣言形
    'def-value',
    'def-function',
    'def-typed-params',
    'def-condition-arms',
    'def-pattern-arms',
    'if-boolean',
    'if-condition',
    'if-pattern',
    'if-paren',
    'if-close',
    'as-continuation',
    'rail-binding',
    'discard-continuation',
    'fn-close',
    'with-continuation',
    'pipe-precedence',
    'library',
    'library-late',
    'library-middle',
    'module',
    'implement-body',
    'functor-def',
    'functor-apply',
    'shared-type',
    'type-realization',
    'type-adt',
    'type-named-record',
    'effect',
    'handler',
    # 字句の閉じ忘れは ERROR になる
    'malformed-string',
    'malformed-char',
    # 第7節の優先順位
    'prec-add-mul',
    'prec-mul-add',
    'prec-pipe-left',
    'prec-unary-apply',
    'prec-postfix-apply',
    'prec-not-and',
    'prec-and-or',
    'prec-add-eq',
    'prec-unary-mul',
    'prec-cmp-and',
    'prec-or-pipe',
    'prec-sub-left',
    'prec-div-left',
    'prec-and-left',
    'prec-or-left',
    'prec-cmp-nonassoc',
)

# ハイライトの探針。1 行 1 字句クラスにして、行内の一意な綴りから桁を計算する。
PROBE_LINES = (
    '// comment 12345',
    'type Probe = | Ctor(Int)',
    'def build = "text 42"',
    'def main = Ctor(1)',
    "def ch = 'a'",
    'def fl = 3.5',
    'def bo = true',
    'def du = __array_length',
    'def key = not false',
    r'def esc = "a\nb\tc\\d\"e"',
    r"def esc2 = '\n'",
    'type Named = Payload { value: Int }',
)
# (行, 行内の綴り, 期待する capture)
PROBES = (
    (0, '// comment 12345', '@comment'),
    (1, 'type', '@keyword'),
    (1, 'Probe', '@type'),
    (1, 'Ctor', '@constructor'),
    (1, 'Int', '@type'),
    (2, 'def', '@keyword'),
    (2, 'build', '@variable'),
    (2, '"text 42"', '@string'),
    (3, 'main', '@variable'),
    (3, 'Ctor', '@constructor'),
    (3, '1', '@number'),
    (4, "'a'", '@character'),
    (5, '3.5', '@number'),
    (6, 'true', '@boolean'),
    (7, '__array_length', '@function.builtin'),
    (8, 'not', '@keyword'),
    (8, 'false', '@boolean'),
    (9, r'"a\nb\tc\\d\"e"', '@string'),
    (10, r"'\n'", '@character'),
    # 名前付き field の constructor では、宣言した型名と payload の名前を分ける。
    (11, 'Named', '@type'),
    (11, 'Payload', '@constructor'),
)
# (行, 開始の綴り, 長さの基準になる綴り, 範囲内に現れてはいけない capture)
ABSENT = (
    (0, '//', '// comment 12345', '@number'),
    (2, '"text 42"', '"text 42"', '@number'),
    (9, r'"a\nb\tc\\d\"e"', r'"a\nb\tc\\d\"e"', '@number'),
    # `Payload` に @type が重ならない（`type_definition` の直接の子ではない）。
    (11, 'Payload', 'Payload', '@type'),
)
# 契約の `.hdi` も同じ filetype で開けること。位置ではなく filetype と
# parser・query・highlighter と extmark の有無だけを測る。
HDI_LINES = (
    'module Store',
    'type T',
    'def create(name: String): T',
)
# 門番が約束する字句クラス。query から読めた capture にこれが揃っていること。
PROMISED = (
    '@comment',
    '@number',
    '@string',
    '@character',
    '@boolean',
    '@keyword',
    '@variable',
    '@type',
    '@constructor',
    '@function.builtin',
)


def corpus_files():
    """ディスク上の受理コーパスと stdlib の .hd / .hdi を数える。"""
    files = [p for p in (REPO / 'tests' / 'accept').rglob('*')
             if p.is_file() and p.suffix in ('.hd', '.hdi')]
    files += [p for p in (REPO / 'stdlib').glob('*')
              if p.is_file() and p.suffix in ('.hd', '.hdi')]
    return sorted(files)


def build_parser(ts, work):
    pkg = work / 'pkg'
    pkg.mkdir()
    shutil.copy(PKG / 'grammar.js', pkg / 'grammar.js')
    shutil.copy(PKG / 'tree-sitter.json', pkg / 'tree-sitter.json')
    cache = work / 'cache'
    cache.mkdir()
    env = dict(os.environ, XDG_CACHE_HOME=str(cache))
    r = subprocess.run([ts, 'generate', '--js-runtime', 'native'], cwd=pkg,
                       env=env, capture_output=True, text=True, timeout=300)
    if r.returncode != 0:
        raise RuntimeError('tree-sitter generate failed: ' + (r.stderr or r.stdout)[-500:])
    so = work / 'hado.so'
    r = subprocess.run([ts, 'build', '-o', str(so), str(pkg)], env=env,
                       capture_output=True, text=True, timeout=600)
    if r.returncode != 0 or not so.is_file():
        raise RuntimeError('tree-sitter build failed: ' + (r.stderr or r.stdout)[-500:])
    return so


def parse_run(ts, so, cache, path, quiet=False):
    argv = [ts, 'parse', '-l', str(so), '--lang-name', 'hado', str(path)]
    if quiet:
        argv.insert(2, '-q')
    return subprocess.run(argv, capture_output=True, text=True,
                          env=dict(os.environ, XDG_CACHE_HOME=str(cache)), timeout=120)


def parse_clean(ts, so, cache, path):
    r = parse_run(ts, so, cache, path, quiet=True)
    return r.returncode == 0 and 'ERROR' not in r.stdout and 'MISSING' not in r.stdout, r


def dump_tree(ts, so, cache, path):
    r = parse_run(ts, so, cache, path)
    d = subprocess.run([sys.executable, str(DUMP)], input=r.stdout,
                       capture_output=True, text=True, timeout=120)
    return d.stdout


def run_in_pty(argv, env, cwd, timeout=120):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
    proc = subprocess.Popen(argv, stdin=slave, stdout=slave, stderr=slave,
                            env=env, cwd=cwd, close_fds=True)
    os.close(slave)
    out = b''
    deadline = time.time() + timeout
    while proc.poll() is None and time.time() < deadline:
        ready, _, _ = select.select([master], [], [], 0.2)
        if not ready:
            continue
        try:
            chunk = os.read(master, 65536)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
        # 端末に問い合わせる背景色の DSR に応答する。pty の相手は門番なので、
        # 応答しないと Neovim が待つ。
        if b'\x1b[6n' in chunk:
            os.write(master, b'\x1b[1;1R')
        # 想定外のエラー表示で Neovim が入力待ちになったら閉じる。ここで止まると
        # 門番が赤ではなくハングし、赤の理由が読めない。
        if b'Press ENTER' in chunk or b'-- More --' in chunk:
            os.write(master, b'\r')
    # Neovim は端末を閉じてから終了するので、pty の read が EIO で抜けた直後は
    # poll() がまだ None のことがある（CPU を取り合う 2 コアの runner で頻発する）。
    # poll() を終了の判定に使わず、締め切りまで終了を待つ。
    try:
        proc.wait(timeout=max(deadline - time.time(), 0))
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()
        os.close(master)
        raise RuntimeError('Neovim did not exit within %ds' % timeout)
    os.close(master)
    return proc.returncode, out


def nvim_probe(nvim, so, work, name, source):
    rtp = work / 'rtp'
    (rtp / 'queries' / 'hado').mkdir(parents=True, exist_ok=True)
    shutil.copy(PKG / 'queries' / 'hado' / 'highlights.scm',
                rtp / 'queries' / 'hado' / 'highlights.scm')
    probe = work / name
    probe.write_text('\n'.join(source) + '\n')
    out_file = work / (name + '.marks')
    if out_file.exists():
        out_file.unlink()
    env = dict(os.environ)
    env.update({
        'HADO_SO': str(so),
        'HADO_RTP': str(rtp),
        'HADO_FILE': str(probe),
        'HADO_OUT': str(out_file),
        'TERM': 'xterm-256color',
        'XDG_DATA_HOME': str(work / 'xdg' / 'data'),
        'XDG_CONFIG_HOME': str(work / 'xdg' / 'config'),
        'XDG_STATE_HOME': str(work / 'xdg' / 'state'),
        'XDG_CACHE_HOME': str(work / 'xdg' / 'cache'),
        'NVIM_LOG_FILE': str(work / 'nvim.log'),
    })
    for key in ('XDG_DATA_HOME', 'XDG_CONFIG_HOME', 'XDG_STATE_HOME', 'XDG_CACHE_HOME'):
        Path(env[key]).mkdir(parents=True, exist_ok=True)
    code, pty_out = run_in_pty(
        [nvim, '-u', 'NONE', '-i', 'NONE', '-c', 'luafile ' + str(HL_LUA), str(probe)],
        env, str(work))
    if code != 0:
        raise AssertionError('Neovim exited %d: %s' % (code, pty_out[-300:]))
    if not out_file.is_file():
        raise AssertionError('Neovim probe wrote no output: %s' % pty_out[-300:])
    return out_file.read_text()


def parse_probe_output(text):
    status = {}
    captures = {}
    marks = []
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == 'capture' and len(parts) == 3:
            captures[int(parts[1])] = parts[2]
        elif parts[0] == 'mark' and len(parts) == 6:
            marks.append(tuple(int(x) for x in parts[1:]))
        elif parts[0] in ('filetype', 'parser', 'query', 'active'):
            status[parts[0]] = ' '.join(parts[1:])
    return status, captures, marks


def main():
    if len(sys.argv) not in (3, 4):
        print('usage: treesitter-check.py <tree-sitter> <nvim> [work-dir]',
              file=sys.stderr)
        return 2
    ts, nvim = sys.argv[1], sys.argv[2]
    if len(sys.argv) == 4:
        # 呼んだ側が作業場所を所有し、後始末もする（門番の wrapper が消す）。
        work = Path(sys.argv[3])
        work.mkdir(parents=True, exist_ok=True)
    else:
        work = Path(tempfile.mkdtemp(prefix='hado-treesitter-'))
    passed = []
    total = []

    def check(name, fn):
        total.append(name)
        try:
            fn()
        except Exception as exc:  # noqa: BLE001 - 門番は失敗を集めて最後に報告する
            print('NG: %s (%s)' % (name, str(exc)[:300]))
        else:
            print('OK: %s' % name)
            passed.append(name)

    state = {}

    def build():
        state['so'] = build_parser(ts, work)
        state['cache'] = work / 'cache'

    check('parser builds', build)

    def corpus():
        files = corpus_files()
        assert files, 'no corpus files found'
        bad = []
        for path in files:
            ok, r = parse_clean(ts, state['so'], state['cache'], path)
            if not ok:
                bad.append('%s: %s' % (path.relative_to(REPO),
                                       (r.stdout or r.stderr).strip().splitlines()[:1]))
        assert not bad, '%d/%d files have ERROR or MISSING: %s' % (
            len(bad), len(files), bad[:5])
        state['corpus_count'] = len(files)

    check('accept corpus and stdlib parse without ERROR or MISSING', corpus)

    def escapes():
        path = work / 'escapes.hd'
        path.write_text(
            'def s = "a\\nb\\tc\\\\d\\"e"\n'
            "def c1 = '\\n'\n"
            "def c2 = '\\t'\n"
            "def c3 = '\\\\'\n"
            "def c4 = '\\''\n"
        )
        ok, r = parse_clean(ts, state['so'], state['cache'], path)
        assert ok, 'escape literals do not parse: %s' % (r.stdout or r.stderr)[-300:]

    check('escape literals parse without ERROR or MISSING', escapes)

    def goldens():
        on_disk = {p.stem for p in GOLDEN.glob('*.hd')}
        trees = {p.stem for p in GOLDEN.glob('*.tree')}
        expected = set(GOLDEN_NAMES)
        assert on_disk == expected, 'golden .hd set differs: %s' % sorted(
            on_disk.symmetric_difference(expected))
        assert trees == expected, 'golden .tree set differs: %s' % sorted(
            trees.symmetric_difference(expected))
        for name in GOLDEN_NAMES:
            got = dump_tree(ts, state['so'], state['cache'], GOLDEN / (name + '.hd'))
            want = (GOLDEN / (name + '.tree')).read_text()
            assert got == want, 'golden %s differs:\n--- got ---\n%s' % (name, got)
        nonassoc = dump_tree(ts, state['so'], state['cache'],
                             GOLDEN / 'prec-cmp-nonassoc.hd')
        assert 'ERROR' in nonassoc, 'a < b < c must keep an ERROR node'
        # 本文付き implement / functor 定義 / 本文の無い適用は別の node 列。
        impl = dump_tree(ts, state['so'], state['cache'], GOLDEN / 'implement-body.hd')
        fundef = dump_tree(ts, state['so'], state['cache'], GOLDEN / 'functor-def.hd')
        apply = dump_tree(ts, state['so'], state['cache'], GOLDEN / 'functor-apply.hd')
        assert len({impl, fundef, apply}) == 3, 'implement/functor forms collapsed'
        shared = dump_tree(ts, state['so'], state['cache'], GOLDEN / 'shared-type.hd')
        plain = dump_tree(ts, state['so'], state['cache'], GOLDEN / 'type-realization.hd')
        assert 'shared_type_realization' in shared and 'shared_type_realization' not in plain, \
            'shared type and plain type realization are not distinguished'
        assert shared != plain, 'shared type golden equals the plain one'
        state['golden_count'] = len(GOLDEN_NAMES)

    check('declaration forms and section 7 precedence goldens match', goldens)

    def highlights():
        text = nvim_probe(nvim, state['so'], work, 'probe.hd', PROBE_LINES)
        status, captures, marks = parse_probe_output(text)
        assert status.get('filetype') == 'hado', 'filetype: %r' % status.get('filetype')
        assert status.get('parser') == 'true', 'parser not available: %r' % status
        assert status.get('active') == 'true', 'highlighter is not active'
        q = status.get('query', '').split()
        assert q and q[0] == 'true' and int(q[1]) > 0, 'highlights query not loaded: %r' % q
        names = set(captures.values())
        missing = [c for c in PROMISED if c not in names]
        assert not missing, 'query lacks captures: %s' % missing
        type_ids = {i for i, n in captures.items() if n == '@type'}
        ctor_ids = {i for i, n in captures.items() if n == '@constructor'}
        assert not (type_ids & ctor_ids), 'type and constructor share a highlight group'
        # 約束した字句ごとに、その字句ちょうどの範囲の mark があること。
        index = {}
        for row, line in enumerate(PROBE_LINES):
            index[row] = line
        for row, needle, capture in PROBES:
            line = index[row]
            col = line.index(needle)
            end_col = col + len(needle)
            hit = [m for m in marks
                   if m[0] == row and m[1] == col and m[2] == row and m[3] == end_col
                   and captures.get(m[4]) == capture]
            assert hit, 'no %s mark at %d:%d-%d (%r); marks=%s' % (
                capture, row, col, end_col, needle, marks)
        # コメントと文字列の中の数字には @number が付かない。
        for row, start_needle, span_needle, capture in ABSENT:
            line = index[row]
            start = line.index(start_needle)
            end = line.index(span_needle) + len(span_needle)
            hit = [m for m in marks if m[0] == row and captures.get(m[4]) == capture
                   and m[1] < end and m[3] > start]
            assert not hit, '%s leaked into %r at row %d' % (capture, span_needle, row)
        # ファイル全体や行全体を覆う 1 つの mark ではないこと。
        last_row = len(PROBE_LINES) - 1
        whole = [m for m in marks if m[0] == 0 and m[1] == 0
                 and m[2] >= last_row]
        assert not whole, 'an extmark covers the whole file'
        assert len(marks) >= len(PROBES), 'too few extmarks: %d' % len(marks)
        # 契約の `.hdi` も同じ filetype で、parser と query と highlighter が働き、
        # extmark が付くこと。
        hdi_text = nvim_probe(nvim, state['so'], work, 'probe.hdi', HDI_LINES)
        hdi_status, _, hdi_marks = parse_probe_output(hdi_text)
        assert hdi_status.get('filetype') == 'hado', \
            '.hdi filetype: %r' % hdi_status.get('filetype')
        assert hdi_status.get('parser') == 'true', '.hdi parser not available'
        assert hdi_status.get('active') == 'true', '.hdi highlighter is not active'
        hq = hdi_status.get('query', '').split()
        assert hq and hq[0] == 'true' and int(hq[1]) > 0, '.hdi query not loaded'
        assert hdi_marks, '.hdi has no extmarks'
        state['mark_count'] = len(marks)

    check('Neovim marks each promised token with its capture group', highlights)

    print('PASSED: %d/%d' % (len(passed), len(total)))
    if state.get('corpus_count'):
        print('corpus files: %d' % state['corpus_count'])
    if state.get('golden_count'):
        print('goldens: %d' % state['golden_count'])
    if state.get('mark_count'):
        print('extmarks: %d' % state['mark_count'])
    return 0 if len(passed) == len(total) else 1


if __name__ == '__main__':
    sys.exit(main())
