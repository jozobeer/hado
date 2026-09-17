#!/usr/bin/env bash
# tier: full
# R77: 「一巡」——隔離した試し書きから、赤・修正・整形・実行・テスト・型表示までを 1 本で測る。
# 検体は 1 つずつ別ディレクトリに置く（check は入力根配下の全 .hd を読むため。§50）。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF77'
import json, os, subprocess, sys, tempfile
from pathlib import Path

hado = sys.argv[1]
tmp = tempfile.mkdtemp()
passed, total = [], []

CLI = ('def usage = "usage: cli <name>"\n'
       'def message\n'
       '  | [] => raise { message = usage }\n'
       '  | [name, ..._] => name\n'
       'def main =\n'
       '  let args = EnvDefault.args ->\n'
       '  IoDefault.log (message args)\n')
TEST = 'def main =\n  if 1 == 1 => ()\n  | raise { message = "should not happen" }\n'
# 正準でない腕（E505）。check --fix が有限回で受理に到達することを測る。
FIXABLE = 'def message\n  | [] => "usage"\n  | [name, ...rest] => name\ndef main = IoDefault.log (message [])\n'
# 試し書きの誤り（隔離されていれば根を壊さない）。
BROKEN = 'def scratch = missing_name\n'


def place(root, name, source):
    path = os.path.join(tmp, root, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    Path(path).write_text(source)
    return path


def run(args):
    return subprocess.run([hado] + args, capture_output=True, text=True, timeout=180)


def entry(root):
    return os.path.join(tmp, root, 'main.hd')


def diagnostics(path):
    out = run(['check', '--format', 'json', path])
    return [json.loads(l) for l in out.stderr.splitlines() if l.strip().startswith('{')]


def check(name, fn):
    total.append(name)
    try:
        fn()
        passed.append(name)
        print('OK: %s' % name)
    except Exception as exc:
        print('NG: %s (%s)' % (name, str(exc)[:240]))


def r77_isolation():
    # ドット始まりは読まれない（試し書きの隔離先。§50）。ドット無しなら根を壊す。
    place('iso-dotted', 'main.hd', CLI)
    place('iso-dotted', '.scratch/exp.hd', BROKEN)
    place('iso-dotted', '.hidden.hd', BROKEN)
    r = run(['check', entry('iso-dotted')])
    assert r.returncode == 0, ('dotted', r.returncode, r.stderr[:160])
    place('iso-plain', 'main.hd', CLI)
    place('iso-plain', 'scratch.hd', BROKEN)
    r = run(['check', entry('iso-plain')])
    assert r.returncode == 1, ('plain', r.returncode)
    assert 'scratch.hd' in r.stderr, ('plain diagnostic', r.stderr[:160])


def r77_fix_loop():
    path = place('fix', 'main.hd', FIXABLE)
    first = diagnostics(path)
    assert [d['code'] for d in first] == ['E505'], first
    assert first[0]['canonical'] == '_', first[0]
    for _ in range(4):
        run(['check', '--fix', path])
        if run(['check', path]).returncode == 0:
            break
    assert run(['check', path]).returncode == 0, 'the fix loop did not reach acceptance'
    assert run(['fmt', '--check', path]).returncode == 0, 'the fixed file is not canonical'


def r77_run():
    path = place('run', 'main.hd', CLI)
    r = subprocess.run([hado, 'run', path, '--', 'alice'], capture_output=True, text=True, timeout=180)
    assert (r.returncode, r.stdout.strip(), r.stderr) == (0, 'alice', ''), (r.returncode, r.stdout, r.stderr[:120])
    r = subprocess.run([hado, 'run', path], capture_output=True, text=True, timeout=180)
    assert r.returncode == 1 and r.stdout == '', ('usage case', r.returncode, r.stdout)
    assert 'usage: cli <name>' in r.stderr, ('usage stderr', r.stderr[:120])


def r77_test():
    place('test', 'main.hd', CLI)
    place('test', 'a_test.hd', TEST)
    r = run(['test', entry('test')])
    assert r.returncode == 0, (r.returncode, r.stderr[:160])
    assert 'test: a_test.hd ok' in r.stdout, r.stdout[:160]
    assert '1/1 passed' in r.stdout, r.stdout[:160]


def r77_print_type_and_docs():
    path = place('type', 'main.hd', CLI)
    r = run(['check', '--print-type=message', path])
    assert r.returncode == 0, (r.returncode, r.stderr[:160])
    assert r.stdout.strip(), 'no type was printed'
    full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], capture_output=True, text=True, check=True).stdout.split()
    assert 'r77' in full, 'r77 is not in the full tier'
    assert 'verify-r77' in Path('docs/spec-trace.md').read_text(), 'the trace has no r77 row'
    assert Path('docs/rounds/r77.md').is_file(), 'docs/rounds/r77.md is missing'
    assert subprocess.run(['sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256']).returncode == 0


for name, fn in (
    ('77-a scratch files are isolated by a leading dot', r77_isolation),
    ('77-b a diagnostic carries its canonical form and the loop reaches acceptance', r77_fix_loop),
    ('77-c run: stdout + 0 on success, stderr + 1 on a usage failure', r77_run),
    ('77-d test reports ok and passes', r77_test),
    ('77-e print-type and the docs/lock', r77_print_type_and_docs),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF77
