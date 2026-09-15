#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."

# 全走 1 回のあいだ、同じ内容の実走を 1 度に畳む（docs/plan.md 32 節）。
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh
passed=0
total=0
check() {
    local label="$1"
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
cargo build --offline -q --bin hado || exit 1
if ! R37_BAD=$'\xff' SEED=${SEED:-$(( (RANDOM << 15) | RANDOM ))} python3 - "$workdir" <<'PY'
from pathlib import Path
import json, os, random, shutil, sys
base = Path(sys.argv[1])
seed = int(os.environ['SEED'])
rng = random.Random(seed)
manifest = []
def add(label, source, expected, mode='type'):
    directory = base / str(len(manifest))
    directory.mkdir()
    (directory / 'main.hd').write_text(source)
    (directory / 'expect').write_text(str(expected) + '\n')
    manifest.append((directory.name, label, mode))
    return directory
def suffix(labels):
    return ' {' + ', '.join(sorted(set(labels))) + '}' if labels else ''
for arity in (0, 1):
    call = 'pick' if arity == 0 else 'pick 3'
    param = '' if arity == 0 else '(n: Int)'
    arm = '' if arity == 0 else ' _'
    for labels in ([], ['Error'], ['Error', 'Bar']):
        declarations = ('effect Bar\nonce other: Int\n' if 'Bar' in labels else '')
        declarations += f'effect Foo\nonce pick{param}: Int{suffix(labels)}\n'
        handled = declarations + f'handler Foo as H\nonce pick{arm} => 7\ndef main = Int.to_string (with H -> {call})\n'
        group = '37-a(2)' if not labels else '37-a(1)' if len(labels) == 1 else '37-a(4)'
        add(f'{group} arity={arity}', handled, 'String' + suffix(labels))
        add(f'37-a(3) arity={arity} labels={len(labels)}', declarations + f'def use (n: Int): Int{suffix(labels + ["Foo"])} => {call} + n\ndef main = Int.to_string (use 3)\n', 'String' + suffix(labels + ['Foo']))
        add(f'37-a(3) inferred arity={arity} labels={len(labels)}', declarations + f'def use (n: Int) => {call} + n\ndef main = Int.to_string (use 3)\n', 'String' + suffix(labels + ['Foo']))
add('37-a(5) abort', 'effect Stop\nabort stop\nhandler Stop as H\nabort stop => 9\ndef main = Int.to_string (with H -> stop)\n', 9, 'print')
add('37-a(5) again', 'effect Ask\nagain ask(value: Int): Unit\nhandler Ask as H\nagain ask _ => resume ()\ndef main = Int.to_string (with H -> do ask 7 -> 8)\n', 8, 'print')
values = [-1, 0, 3, 5, 2147483647, rng.randrange(10, 10000)]
for value in values:
    add('37-b2(1) parameter', f'def f (raise: Int): Int => raise\ndef main = Int.to_string (f ({value}))\n', value, 'print')
    add('37-b2(2) function', f'def inc (n: Int): Int => n + 1\ndef f (raise: Fn(Int => Int)): Int => raise ({value})\ndef main = Int.to_string (f inc)\n', value + 1, 'print')
    add('37-b2(3) let', f'def main = Int.to_string (let raise = {value} -> raise + 1)\n', value + 1, 'print')
    add('37-b2(5) user operation', f'effect Foo\nonce tick: Int\ndef f (tick: Int): Int => tick\ndef main = Int.to_string (f ({value}))\n', value, 'print')
add('37-b2(5) again let', 'effect Foo\nagain tick(value: Int): Unit\ndef f (tick: Int): Int => let x = tick -> x + 1\ndef main = Int.to_string (f 8)\n', 9, 'print')
add('37-b2(4) unshadowed', 'def main = Int.to_string (raise { message = "x" })\n', 'x', 'error')

def quote(value):
    return json.dumps(value, ensure_ascii=False)
def runtime(label, source, expected, arguments=(), environment=None, error=False, oracle=None):
    directory = add(label, source, expected, 'error' if error else 'print')
    (directory / 'run.json').write_text(json.dumps([arguments, environment or {}, oracle]))
    return directory
bad = os.environ['R37_BAD']
for shape, arguments in [('empty', []), ('one', ['one']), ('many', ['a', '日本語', str(rng.randrange(10000))]), ('empty-string', ['', 'tail']), ('invalid', [bad])]:
    runtime('37-c(1) '+shape, 'def main = args\n',
            'InvalidUtf8' if shape == 'invalid' else '[' + ', '.join(map(quote, arguments)) + ']',
            arguments, error=shape == 'invalid', oracle=['args'])
key = 'HADO_R37_' + str(rng.randrange(1000000))
for shape, value in [('set', 'value日本語'), ('unset', None), ('empty', ''), ('invalid', bad)]:
    runtime('37-c(2) '+shape, f'def main = env {quote(key)}\n',
            'InvalidUtf8' if shape == 'invalid' else 'None' if value is None else 'Some(' + quote(value) + ')',
            environment={key: value}, error=shape == 'invalid', oracle=['env', key])
runtime('37-c(2) invalid-name', 'def first | [head, ..._] => head | [] => ""\ndef main = env (first args)\n', 'InvalidUtf8', [bad], {bad: 'exists'}, True, ['env', bad])
directory = add('37-c(3) cwd', 'def main = cwd\n', '', 'print')
(directory / 'run.json').write_text(json.dumps([[], {}, ['cwd']]))
add('37-b prelude', 'def main = args\n', 'String List {Env, Error}', 'entry-type')
handler = 'handler Env as MyEnv\nonce args => ["override"]\nonce env _ => Some("overridden")\nonce cwd => "virtual"\n'
for operation, expected in [('args', '["override"]'), ('env "unused"', 'Some("overridden")'), ('cwd', 'virtual')]:
    runtime('37-d(2) override', handler + f'def main = with MyEnv -> {operation}\n', expected, ['native'])
runtime('37-d(3) error-handler', 'handler Error as H\nabort raise e => [e.message]\ndef main = with H -> args\n', '["InvalidUtf8"]', [bad])
for name in ('args', 'env', 'cwd'):
    for value in values:
        add('37-b2(6) parameter', f'def f ({name}: Int): Int => {name}\ndef main = f ({value})\n', value, 'print')
        add('37-b2(6) function', f'def inc (n: Int): Int => n + 1\ndef f ({name}: Fn(Int => Int)): Int => {name} ({value})\ndef main = f inc\n', value+1, 'print')
        add('37-b2(6) let', f'def main = let {name} = {value} -> {name} + 1\n', value+1, 'print')
def reject(label, source, code, anchor, sub=False, multiple=False):
    blank, indent = rng.randrange(1, 8), rng.randrange(1, 8)
    if sub and source.startswith('def '): source = 'library\n' + source.replace('def main = 0\n', '')
    source = '\n' * blank + '\n'.join(' ' * indent + line for line in source.splitlines()) + '\n'
    line = next(i for i, text in enumerate(source.splitlines(), 1) if anchor in text)
    col = source.splitlines()[line-1].index(anchor) + 1
    directory = add(label, source, json.dumps([code, line, col]), 'reject')
    if sub:
        (directory / 'sub').mkdir()
        (directory / 'main.hd').rename(directory / 'sub' / 'probe.hd')
        (directory / 'main.hd').write_text('def main = 0\n')
    elif multiple:
        (directory / 'peer.hd').write_text('')
    (directory / 'diagnostic-path').write_text('sub/probe.hd' if sub else 'main.hd')
reject('37-b user', 'effect Foo\nonce pick: Int\ndef main = pick\n', 'E706', 'def main')
reject('37-d(4) missing-arm', 'handler Env as H\nonce args => ["x"]\nonce env _ => None\ndef main = 0\n', 'E605', 'handler Env')
reject('37-e(1) effect', 'effect Env\nonce custom: Int\ndef main = 0\n', 'E205', 'Env')
reject('37-e module', 'module Env\ndef value: Int Pure\n', 'E205', 'Env')
for name in ('EnvDefault', 'IoDefault'):
    reject('37-e module '+name, f'module {name}\ndef value: Int Pure\n', 'E205', name)
for sub in (False, True):
    for multiple in (False, True):
        for shape, source, anchor in [
            ('def', 'def args = 1\n', 'args'),
            ('implementation', 'module M\ndef args: Int Pure\nimplement M as M\ndef args = 1\n', 'args ='),
            ('operation', 'effect Foo\nonce args: Int\n', 'args'),
        ]:
            reject('37-e(2) '+shape, source + 'def main = 0\n', 'E205', anchor, sub, multiple)
            if sub and multiple:
                (base / str(len(manifest)-1) / 'sub' / 'peer.hd').write_text('')
for effect, arms, body, expected in [
    ('Env', 'once args => ["sub"]\nonce env _ => None\nonce cwd => "subcwd"\n', 'args', '["sub"]'),
    ('Error', 'abort raise e => e.message\n', 'raise { message = "caught" }', 'caught'),
]:
    directory = add('37-e(3) handler', f'def main = sub::M.value\n', expected, 'print')
    (directory / 'sub').mkdir()
    (directory / 'sub' / 'sub.hd').write_text(f'handler {effect} as H\n{arms}module M\ndef value: '+('String List {Error}' if effect == 'Env' else 'String Pure')+f'\nimplement M as M\ndef value = with H -> {body}\n')
directory = add('37-e(4) visible', 'def main = sub::M.value\n', '[]', 'print')
(directory / 'sub').mkdir()
(directory / 'sub' / 'sub.hd').write_text('module M\ndef value: String List {Env, Error}\nimplement M as M\ndef value = args\n')
for contract in ('List', 'Option', 'String'):
    reject('37-e(5) contract', f'handler {contract} as H\nonce invented => 0\n', 'E605', 'handler', True)


# Io fixtures stay beside their own entry and are read independently by Python.
def summary(body, data):
    indices = sorted(set([0, len(data)//2, len(data)-1])) if data else []
    if not indices: return 'def main = [Bytes.length (' + body + ')]\n'
    return ('def first | [] => -1 | [head, ..._] => head\ndef rest | [] => [] | [_, ...tail] => tail\ndef rec at values index => if index == 0 => first values | at (rest values) (index - 1)\n'
            + 'def main = let bytes = ' + body + ' -> let values = Bytes.to_list bytes -> [Bytes.length bytes'
            + ''.join(f', at values {i}' for i in indices) + ']\n')
def file_case(shape, data, name='input'):
    directory = runtime('37-f(1) '+shape, summary('read_file '+quote(name), data), '', oracle=['file', name])
    (directory / name).write_bytes(data)
    if len(data) >= 1024: (directory / 'large-list').touch()
    if len(data) < 1024:
        directory = runtime('37-f(1) bytes-'+shape, f'def main = Bytes.to_list (read_file {quote(name)})\n', '', oracle=['file-list', name])
        (directory / name).write_bytes(data)
    try: data.decode('utf-8')
    except UnicodeError: return
    if len(data) < 1024:
        directory = runtime('37-f(1) text-'+shape, f'def main = Bytes.to_string (read_file {quote(name)})\n', '', oracle=['file-text', name])
        (directory / name).write_bytes(data)
for shape, data, name in [
    ('existing', f'hello {rng.randrange(100000)}\n'.encode(), 'input'), ('empty', b'', 'input'),
    ('binary', bytes([0,255,128,10])+rng.randbytes(37), 'input'),
    ('large', rng.randbytes(1024*1024+73), 'input'), ('unicode', '日本語\n'.encode(), '資料'),
]: file_case(shape, data, name)
for shape, data in [('empty', b''), ('ascii', b'write '+str(rng.randrange(10000)).encode()), ('binary', bytes([0,255,128,10])), ('overwrite', b'short')]:
    source = f'def main = do write_file "output" (Bytes.of_list {list(data)}) -> Bytes.to_list (read_file "output")\n'
    directory = runtime('37-f(2) '+shape, source, str(list(data)))
    (directory / 'written.json').write_text(json.dumps(list(data)))
    if shape == 'overwrite': (directory / 'output').write_bytes(b'old contents that must be truncated')
for shape, names in [('empty', []), ('one', ['single']), ('many', ['z','b','a']), ('unicode', ['語','é','a']), ('invalid', [bad]), ('subdir', ['z','folder','a']), ('hidden', ['z','.hidden','a'])]:
    directory = runtime('37-f(3) '+shape, 'def main = read_dir "entries"\n', '', error=shape=='invalid', oracle=['dir', 'entries'])
    (directory / 'entries').mkdir()
    for name in names:
        target = directory / 'entries' / name
        if name == 'folder': target.mkdir()
        else: target.touch()
for shape, data in [('closed', b''), ('empty', b''), ('content', b'stdin '+rng.randbytes(41)), ('large', rng.randbytes(1024*1024+97))]:
    directory = runtime('37-f(4) '+shape, summary('read_stdin', data), '', oracle=['file', 'stdin'])
    (directory / 'stdin').write_bytes(data)
    if len(data) >= 1024: (directory / 'large-list').touch()
    if shape == 'closed': (directory / 'closed-stdin').touch()
data = b'twice '+rng.randbytes(39)
directory = runtime('37-f(5) second', 'def main = let first = read_stdin -> let second = read_stdin -> [Bytes.length first, Bytes.length second]\n', str([len(data), 0]))
(directory / 'stdin').write_bytes(data)
failures = [
    ('read_file', 'missing', 'NotFound'), ('read_file', 'file/child', 'NotADirectory'),
    ('read_file', 'directory', 'IoFailure'), ('read_file', 'denied-file', 'PermissionDenied'),
    ('read_dir', 'missing', 'NotFound'), ('read_dir', 'file', 'NotADirectory'),
    ('read_dir', 'denied-dir', 'PermissionDenied'), ('write_file', 'missing/child', 'NotFound'),
    ('write_file', 'readonly/child', 'PermissionDenied'), ('write_file', 'directory', 'IoFailure'),
]
for operation, path, message in failures:
    call = operation + ' ' + quote(path) + (' (Bytes.of_string "new")' if operation=='write_file' else '')
    ok_path = 'directory' if operation=='read_dir' else 'file'
    ok_call = operation + ' ' + quote(ok_path) + (' (Bytes.of_string "new")' if operation=='write_file' else '')
    for kind, source, expected, error in [
        ('failure', 'def main = '+call+'\n', message, True),
        ('success', 'def main = '+ok_call+'\n', '[]' if operation=='read_dir' else '' if operation=='write_file' else 'Bytes.of_list [111, 107]', False),
        ('caught', 'handler Error as H\nabort raise e => e.message\nreturn _ => "success"\ndef main = with H -> '+call+'\n', message, False),
    ]:
        group = '37-g(5)' if kind=='caught' else '37-g(4)' if kind=='success' else '37-g('+str({'read_file':1,'read_dir':2,'write_file':3}[operation])+')'
        directory = runtime(group+' '+operation+' '+path+' '+kind, source, expected, error=error)
        if kind=='success' and operation=='write_file':
            (directory/'expect').write_bytes(b'')
            (directory/'unit-output').touch()  # Unit prints nothing at the top level (§54)
        (directory/'failure-fixture').touch()
        if message=='PermissionDenied': (directory/'requires-user').touch()
add('37-b Io', 'def main = [List.length args, Bytes.length read_stdin]\n', 'Int List {Env, Error, Io}', 'entry-type')
io_arms = ['once read_file _ => Bytes.of_string "virtual"', 'once write_file _ _ => ()', 'once log _ => ()', 'once read_dir _ => ["virtual"]', 'once read_stdin => Bytes.of_string "virtual"']
for operation, expected in [('Bytes.to_string (read_file "absent")', 'virtual'), ('write_file "absent/child" (Bytes.of_string "x")', ''), ('log "virtual"', ''), ('read_dir "absent"', '["virtual"]'), ('Bytes.to_string read_stdin', 'virtual')]:
    directory = runtime('37-d Io override', 'handler Io as MyIo\n'+'\n'.join(io_arms)+'\ndef main = with MyIo -> '+operation+'\n', expected)
    if expected=='':
        (directory/'expect').write_bytes(b'')
        (directory/'unit-output').touch()  # Unit prints nothing at the top level (§54)
for missing in range(len(io_arms)):
    reject('37-d Io missing-arm', 'handler Io as H\n'+'\n'.join(arm for i,arm in enumerate(io_arms) if i!=missing)+'\ndef main = 0\n', 'E605', 'handler Io')
for sub in (False, True):
    reject('37-e Io effect', 'effect Io\nonce custom: Int\ndef main = 0\n', 'E205', 'Io', sub)
    for name in ('read_file','write_file','log','read_dir','read_stdin'):
        for shape, source, anchor in [
            ('def', f'def {name} = 1\n', name),
            ('implementation', f'module M\ndef {name}: Int Pure\nimplement M as M\ndef {name} = 1\n', name+' ='),
            ('operation', f'effect Foo\nonce {name}: Int\n', name),
        ]:
            reject('37-e Io '+shape, source+'def main = 0\n', 'E205', anchor, sub, True)

(base / 'oracle.py').write_text('''import json, os, sys
try:
    kind = sys.argv[1]
    if kind == 'args':
        values = sys.argv[2:]
        for value in values: value.encode('utf-8')
        print(json.dumps(values, ensure_ascii=False))
    elif kind == 'env':
        name = sys.argv[2]
        name.encode('utf-8')
        value = os.environ.get(name)
        if value is not None: value.encode('utf-8')
        print('None' if value is None else 'Some(' + json.dumps(value, ensure_ascii=False) + ')')
    elif kind.startswith('file'):
        data = open(sys.argv[2], 'rb').read()
        if kind == 'file-text': print(data.decode('utf-8'))
        elif kind == 'file-list': print(list(data))
        else:
            indices = sorted(set([0, len(data)//2, len(data)-1])) if data else []
            print([len(data)] + [data[i] for i in indices])
    elif kind == 'dir':
        names = sorted(os.listdir(sys.argv[2]))
        assert '.' not in names and '..' not in names
        for name in names: name.encode('utf-8')
        print(json.dumps(names, ensure_ascii=False))
    elif kind == 'cwd':
        value = os.getcwd()
        value.encode('utf-8')
        print(value)
    else:
        raise AssertionError(kind)
except UnicodeError:
    print('InvalidUtf8')
''')
# Exercise the actual E2 entry against independently generated log files.
e2_shapes = ('empty', 'one-green', 'one-red', 'many-green', 'many-red', 'last-line', 'zero')
def e2_case(shape, expected='', arguments=('logs',), error=False):
    directory = runtime('37-h '+shape, Path('tools/gates/main.hd').read_text(), expected, arguments, error=error)
    shutil.copytree('tools/gates/report', directory / 'report')
    return directory
for shape in e2_shapes:
    directory = e2_case(shape)
    logs = directory / 'logs'
    logs.mkdir()
    count = 0 if shape == 'empty' else rng.randrange(3, 8) if shape.startswith('many') else 1
    for index in reversed(range(count)):
        name = f'{index:02d}-門番 {rng.randrange(100000)}.log'
        total = 0 if shape == 'zero' else rng.randrange(1, 100)
        passed = rng.randrange(total) if 'red' in shape and index % 2 == 0 else total
        earlier = f'PASSED: {total + 1}/{total + 3}\n' if shape == 'last-line' else ''
        (logs / name).write_text(f'noise\n PASSED: 99/99\n{earlier}PASSED: {passed}/{total}\nfinished\n')
    # Match the shell oracle's last anchored PASSED line, then sum in Python.
    counts = []
    import subprocess
    for log in sorted(logs.iterdir(), key=lambda path: os.fsencode(path.name)):
        lines = subprocess.run(['grep', '-E', '^PASSED:', str(log)], capture_output=True, check=True).stdout.splitlines()
        passed, total = map(int, lines[-1].removeprefix(b'PASSED: ').split(b'/'))
        counts.append((log.name, passed, total))
    red = ' '.join(name for name, passed, total in counts if passed < total) or 'none'
    expected = f'{sum(p for _, p, _ in counts)}/{sum(t for _, _, t in counts)} passed; red: {red}\n'
    (directory / 'expect').write_text(expected)
e2_case('missing-argv', 'MissingDirectory', (), True)
e2_case('not-found', 'NotFound', ('missing',), True)
directory = e2_case('not-directory', 'NotADirectory', ('file',), True)
(directory / 'file').touch()
for shape, data, expected in [('no-passed', b'noise\n PASSED: 9/9\n', 'InvalidLine'),
                              ('empty-log', b'', 'InvalidLine'),
                              ('invalid-utf8', b'PASSED: 1/1\n\xff', 'InvalidUtf8')]:
    directory = e2_case(shape, expected, error=True)
    (directory / 'logs').mkdir()
    (directory / 'logs' / 'entry.log').write_bytes(data)
assert manifest
(base / 'manifest.tsv').write_text(''.join('\t'.join(row) + '\n' for row in manifest))
print(f'seed={seed} probes={len(manifest)} values={values}')
PY
then
    echo 'NG: generator failed'
    exit 1
fi
validate_manifest() {
    python3 - "$workdir" <<'PY'
from pathlib import Path
import json, sys
base = Path(sys.argv[1])
def validate(rows):
    assert rows, 'empty manifest'
    seen, coverage, units = set(), set(), set()
    for row in rows:
        ident, label, mode = row.split('\t')
        assert ident.isdecimal() and ident not in seen
        seen.add(ident)
        coverage.add((' '.join(token.split('=')[0] for token in label.split()), mode))
        assert (base / ident / 'main.hd').stat().st_size > 0
        run = base / ident / 'run.json'
        oracle = json.loads(run.read_text())[2] if run.exists() else None
        unit = (base / ident / 'unit-output').exists()
        # An empty expect is allowed only when the oracle fills it at run time
        # (the runner asserts the oracle produced non-empty stdout), or when the
        # probe deliberately returns Unit, which prints nothing at the top level.
        assert (base / ident / 'expect').stat().st_size > 0 or oracle or unit, ident
        if unit: units.add(label)
    assert seen == {p.name for p in base.iterdir() if p.is_dir() and p.name.isdecimal()}
    assert units == {'37-g(4) write_file missing/child success', '37-g(4) write_file readonly/child success',
                     '37-g(4) write_file directory success', '37-d Io override'}, sorted(units)
    assert {item for item in coverage if not (item[0].startswith(('37-f', '37-g', '37-b Io', '37-d Io', '37-e Io', '37-h')))} == {
        ('37-a(1) arity', 'type'), ('37-a(2) arity', 'type'), ('37-a(4) arity', 'type'),
        ('37-a(3) arity labels', 'type'), ('37-a(3) inferred arity labels', 'type'),
        ('37-a(5) abort', 'print'), ('37-a(5) again', 'print'),
        ('37-b2(1) parameter', 'print'), ('37-b2(2) function', 'print'),
        ('37-b2(3) let', 'print'), ('37-b2(5) user operation', 'print'),
        ('37-b2(5) again let', 'print'), ('37-b2(4) unshadowed', 'error'),
    } | {
        ('37-c(1) ' + shape, 'error' if shape == 'invalid' else 'print')
        for shape in ('empty', 'one', 'many', 'empty-string', 'invalid')
    } | {
        ('37-c(2) ' + shape, 'error' if shape.startswith('invalid') else 'print')
        for shape in ('set', 'unset', 'empty', 'invalid', 'invalid-name')
    } | {
        ('37-c(3) cwd', 'print'), ('37-b prelude', 'entry-type'), ('37-b user', 'reject'),
        ('37-d(2) override', 'print'), ('37-d(3) error-handler', 'print'), ('37-d(4) missing-arm', 'reject'),
        ('37-e(1) effect', 'reject'), ('37-e module', 'reject'),
        *[('37-e module '+name, 'reject') for name in ('EnvDefault', 'IoDefault')],
        *[('37-e(2) ' + shape, 'reject') for shape in ('def', 'implementation', 'operation')],
        ('37-e(3) handler', 'print'), ('37-e(4) visible', 'print'), ('37-e(5) contract', 'reject'),
        *[('37-b2(6) ' + shape, 'print') for shape in ('parameter', 'function', 'let')],
    }, sorted(coverage)
    io_coverage = {item for item in coverage if item[0].startswith(('37-f', '37-g', '37-b Io', '37-d Io', '37-e Io'))}
    expected_io = {
        *[('37-f(1) '+shape, 'print') for shape in ('existing','empty','binary','large','unicode','bytes-existing','bytes-empty','bytes-binary','bytes-unicode','text-existing','text-empty','text-unicode')],
        *[('37-f(2) '+shape, 'print') for shape in ('empty','ascii','binary','overwrite')],
        *[('37-f(3) '+shape, 'error' if shape=='invalid' else 'print') for shape in ('empty','one','many','unicode','invalid','subdir','hidden')],
        *[('37-f(4) '+shape, 'print') for shape in ('closed','empty','content','large')],
        ('37-f(5) second', 'print'), ('37-b Io','entry-type'),
        ('37-d Io override','print'), ('37-d Io missing-arm','reject'),
        *[('37-e Io '+shape,'reject') for shape in ('effect','def','implementation','operation')],
    }
    for operation, paths in [('read_file', ('missing','file/child','directory','denied-file')), ('read_dir', ('missing','file','denied-dir')), ('write_file', ('missing/child','readonly/child','directory'))]:
        for path in paths:
            for kind, group, mode in [('failure','37-g('+str({'read_file':1,'read_dir':2,'write_file':3}[operation])+')','error'), ('success','37-g(4)','print'), ('caught','37-g(5)','print')]:
                expected_io.add((group+' '+operation+' '+path+' '+kind,mode))
    assert io_coverage == expected_io, (io_coverage ^ expected_io)
    e2_coverage = {item for item in coverage if item[0].startswith('37-h')}
    assert e2_coverage == {
        *[('37-h '+shape, 'print') for shape in ('empty','one-green','one-red','many-green','many-red','last-line','zero')],
        *[('37-h '+shape, 'error') for shape in ('missing-argv','not-found','not-directory','no-passed','empty-log','invalid-utf8')],
    }, e2_coverage
rows = (base / 'manifest.tsv').read_text().splitlines()
validate(rows)
def without(shape):
    return [row for row in rows
            if ' '.join(t.split('=')[0] for t in row.split('\t')[1].split()) != shape]
missing = [without(shape) for shape in
           ('37-a(5) abort', '37-a(5) again', '37-b2(5) again let', '37-b2(2) function',
            '37-a(3) inferred arity labels', '37-h last-line', '37-h empty', '37-h missing-argv')]
for bad in ([], rows[1:], [row for row in rows if not row.endswith('\terror')], rows + rows[:1], *missing):
    try:
        validate(bad)
    except AssertionError:
        pass
    else:
        raise AssertionError('incomplete or duplicate manifest accepted')
print('manifest controls: complete accepted; empty, all-success, duplicate, missing-sibling rejected')
PY
}
validate_manifest || exit 1
output_matches() {
    local mode="$1" status="$2" dir="$3"
    if [ "$mode" = error ]; then
        [ "$status" -eq 1 ] && [ "$(wc -c <"$dir/out")" -eq 0 ] && cmp -s "$dir/err" "$dir/expect"
    else
        [ "$status" -eq 0 ] && [ "$(wc -c <"$dir/err")" -eq 0 ] && cmp -s "$dir/out" "$dir/expect"
    fi
}
output_controls() {
    local dir="$workdir/control" mode
    mkdir -p "$dir"
    printf 'value\n' >"$dir/expect"
    for mode in print error; do
        local good=out empty=err status=0
        if [ "$mode" = error ]; then good=err; empty=out; status=1; fi
        cp "$dir/expect" "$dir/$good"
        : >"$dir/$empty"
        output_matches "$mode" "$status" "$dir" || return 1
        if output_matches "$mode" "$((1-status))" "$dir"; then return 1; fi
        printf '\n' >"$dir/$empty"
        if output_matches "$mode" "$status" "$dir"; then return 1; fi
        : >"$dir/$empty"
        printf 'different\n' >"$dir/$good"
        if output_matches "$mode" "$status" "$dir"; then return 1; fi
        : >"$dir/$good"
        if output_matches "$mode" "$status" "$dir"; then return 1; fi
    done
}
check 'output controls: equal, empty, different, newline, status' output_controls
probe() {
    local mode="$1" dir="$2" status=0
    if [ -e "$dir/requires-user" ] && [ "$(id -u)" -eq 0 ]; then echo 'NG: skipped-as-root'; return 1; fi
    if [ "$mode" = reject ]; then
        cargo run --offline -q -- check "$dir/main.hd" >"$dir/out" 2>"$dir/err" || status=$?
        cat "$dir/err"
        [ "$status" -eq 1 ] && [ "$(wc -c <"$dir/out")" -eq 0 ] || return 1
        python3 - "$dir" <<'PY'
from pathlib import Path
import json, re, sys
base = Path(sys.argv[1])
code, line, col = json.loads((base / 'expect').read_text())
path = (base / 'diagnostic-path').read_text()
actual = (base / 'err').read_text()
assert re.match(re.escape(f'{path}:{line}:{col}: error[{code}]') + r': [^\n]+\n\Z', actual), actual
PY
        return $?
    elif [ "$mode" = type ] || [ "$mode" = entry-type ]; then
        if [ "$mode" = entry-type ]; then
            cargo run --offline -q -- check "$dir/main.hd" >"$dir/check.out" 2>"$dir/check.err" || { cat "$dir/check.err"; return 1; }
            [ "$(wc -c <"$dir/check.out")" -eq 0 ] && [ "$(wc -c <"$dir/check.err")" -eq 0 ] || return 1
        fi
        cargo run --offline -q -- check --print-type "$dir/main.hd" >"$dir/out" 2>"$dir/err" || status=$?
    else
        cargo run --offline -q -- build "$dir/main.hd" >"$dir/build.out" 2>"$dir/build.err" || { cat "$dir/build.err"; return 1; }
        [ "$(wc -c <"$dir/build.err")" -eq 0 ] || return 1
        python3 - "$dir" <<'PY'
from pathlib import Path
import json, os, subprocess, sys
base = Path(sys.argv[1])
if (base / 'failure-fixture').exists():
    (base/'file').write_bytes(b'ok')
    (base/'directory').mkdir()
    (base/'denied-file').write_bytes(b'denied')
    (base/'denied-file').chmod(0)
    (base/'denied-dir').mkdir(mode=0)
    (base/'readonly').mkdir(mode=0o500)
arguments, overrides, oracle = json.loads((base / 'run.json').read_text()) if (base / 'run.json').exists() else ([], {}, None)
environment = dict(os.environ)
for key, value in overrides.items():
    if value is None: environment.pop(key, None)
    else: environment[key] = value
if oracle:
    oracle_args = oracle + (arguments if oracle[0] == 'args' else [])
    expected = subprocess.run([sys.executable, str(base.parent / 'oracle.py'), *oracle_args], cwd=base, env=environment, capture_output=True, timeout=30)
    assert expected.returncode == 0 and not expected.stderr and expected.stdout, expected
    (base / 'expect').write_bytes(expected.stdout)
with (base / 'out').open('wb') as out, (base / 'err').open('wb') as err:
    def prepare_process():
        if (base / 'closed-stdin').exists(): os.close(0)
        if (base / 'large-list').exists():
            # Bytes.to_list recursively builds a million cells for the independent list sample.
            import resource
            soft, hard = resource.getrlimit(resource.RLIMIT_STACK)
            resource.setrlimit(resource.RLIMIT_STACK, (max(soft, 128 * 1024 * 1024), hard))
    data = (base / 'stdin').read_bytes() if (base / 'stdin').exists() else b''
    if (base / 'closed-stdin').exists():
        result = subprocess.run([str(base / 'main'), *arguments], cwd=base, env=environment, preexec_fn=prepare_process, stdout=out, stderr=err, timeout=30)
    else:
        result = subprocess.run([str(base / 'main'), *arguments], cwd=base, env=environment, input=data, preexec_fn=prepare_process, stdout=out, stderr=err, timeout=30)
if (base / 'written.json').exists():
    assert (base / 'output').read_bytes() == bytes(json.loads((base / 'written.json').read_text()))
(base / 'status').write_text(str(result.returncode))
PY
        [ "$?" -eq 0 ] || return 1
        status=$(cat "$dir/status")
    fi
    cat "$dir/out" "$dir/err"
    output_matches "$mode" "$status" "$dir"
}
while IFS=$'\t' read -r ident label mode; do
    check "$label" probe "$mode" "$workdir/$ident"
done <"$workdir/manifest.tsv"
check '37-i Env and Io corpus headers and lock' python3 - <<'PY'
from pathlib import Path
import hashlib, re

def validate(sources, lock):
    tagged = {path: source for path, source in sources.items() if '// item: Env と Io' in source.splitlines()}
    assert tagged, 'missing Env and Io corpus'
    assert any(re.search(r'^// expect:', source, re.M) for source in tagged.values()), 'missing success'
    assert any(re.search(r'^// expect-error:', source, re.M) for source in tagged.values()), 'missing failure'
    entries = {}
    for line in lock.splitlines():
        digest, path = line.split('  ', 1)
        assert path not in entries, path
        entries[path] = digest
    for path, source in tagged.items():
        assert '// spec: §55' in source.splitlines(), path
        assert '// needs: prelude' in source.splitlines(), path
        assert len(re.findall(r'^// expect(?:-error)?:', source, re.M)) == 1, path
        assert entries.get(str(path)) == hashlib.sha256(source.encode()).hexdigest(), f'lock mismatch: {path}'
    return tagged

sources = {path: path.read_text() for path in Path('tests/accept').glob('*/main.hd')}
lock = Path('tests/corpus.sha256').read_text()
tagged = validate(sources, lock)
print(f'Env and Io corpus: {len(tagged)} files; success, failure, prelude and lock verified')
controls = [({}, lock)]
for path in tagged:
    changed = dict(sources)
    changed[path] = changed[path].replace('// needs: prelude\n', '', 1)
    controls.append((changed, lock))
    pattern = r'^([0-9a-f])([0-9a-f]{63}  ' + re.escape(str(path)) + r')$'
    altered, count = re.subn(pattern, lambda m: ('0' if m[1] != '0' else '1') + m[2], lock, flags=re.M)
    assert count == 1
    controls.append((sources, altered))
# The item tag belongs to the item, not to one file: when more than one file
# carries it, dropping it from a single file is invisible to validate. Drop it
# from every tagged file and require the empty set to be rejected.
controls.append(({path: source.replace('// item: Env と Io\n', '', 1) for path, source in sources.items()}, lock))
for changed, altered in controls:
    try:
        validate(changed, altered)
    except AssertionError:
        pass
    else:
        raise AssertionError('corpus mutation escaped')
print(f'corpus controls: {len(controls)} empty/tag/prelude/lock mutations rejected')
PY
check '37-i full corpus lock' sha256sum --strict --status -c tests/corpus.sha256

m6_launch_conditions() {
    python3 - "$workdir" <<'PY'
from pathlib import Path
import ast, os, subprocess, sys
base = Path(sys.argv[1]) / 'm6-launch'
base.mkdir()
case = base / 'case'
case.mkdir()
source = 'def main = [Bytes.length (read_file "main.hd"), Bytes.length read_stdin, List.length args]\n'
(case / 'main.hd').write_text(source)
compiler = base / 'compiler'
compiler.write_text('#!/usr/bin/env bash\nexec cargo run --offline -q -- "$@"\n')
compiler.chmod(0o755)
script = Path('scripts/verify-m6.sh').read_text().split("<<'PY'\n", 1)[1].rsplit('\nPY', 1)[0]
tree = ast.parse(script)
function = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'verify')
runner = base / 'runner.py'
runner.write_text('from pathlib import Path\nimport subprocess, sys\n'
                  + f'compiler = {str(compiler)!r}\ncase = Path({str(case)!r})\n'
                  + ast.unparse(function) + '\n'
                  + f'sys.exit(0 if verify("launch fixture", case / "main.hd", {str([len(source.encode()), 0, 0]).encode() + bytes([10])!r}) else 1)\n')
result = subprocess.run([sys.executable, str(runner)], input=b'stdin must not reach the case\n', capture_output=True, timeout=90)
sys.stdout.buffer.write(result.stdout)
sys.stderr.buffer.write(result.stderr)
assert result.returncode == 0 and not result.stderr, 'm6 must fix both cwd and stdin'
PY
}
check '37-j actual m6 launch function: relative file, EOF and empty argv' m6_launch_conditions

full_gate() {
    local gate="$1" status=0
    gate_cached "verify-$gate" bash "scripts/verify-$gate.sh" >"$workdir/$gate.log" 2>&1 || status=$?
    tail -n 4 "$workdir/$gate.log"
    [ "$status" -eq 0 ] || { cat "$workdir/$gate.log"; return 1; }
    python3 - "$workdir/$gate.log" <<'PY'
from pathlib import Path
import re, sys

def complete(source):
    lines = source.splitlines()
    match = re.fullmatch(r'PASSED: (\d+)/(\d+)', lines[-1]) if lines else None
    return bool(match and int(match[1]) > 0 and match[1] == match[2]
                and not any(line.startswith('NG:') for line in lines))
assert complete('PASSED: 1/1\n')
assert not any(map(complete, ('', 'PASSED: 0/0\n', 'PASSED: 1/2\n', 'NG: failed\nPASSED: 1/1\n')))
assert complete(Path(sys.argv[1]).read_text()), 'missing or non-full gate result'
PY
}
check '37-i corpus pairs and regressions' full_gate m1-corpus
check '37-i no missing pairs' grep -qxF 'OK: 3 pairs' "$workdir/m1-corpus.log"
check '37-k prelude corpus and cargo tests' full_gate m7
check '37-k cargo tests executed without failures' python3 - "$workdir/m7.log" <<'PY'
from pathlib import Path
import re, sys

def passed_tests(source):
    results = re.findall(r'^test result: (\w+)\. (\d+) passed; (\d+) failed;', source, re.M)
    assert results and sum(int(passed) for _, passed, _ in results) > 0
    assert all(state == 'ok' and int(failed) == 0 for state, _, failed in results)
    return sum(int(passed) for _, passed, _ in results)
assert passed_tests('test result: ok. 1 passed; 0 failed;') == 1
for source in ('', 'test result: ok. 0 passed; 0 failed;',
               'test result: FAILED. 1 passed; 1 failed;',
               'test result: ok. 1 passed; 0 failed;\ntest result: FAILED. 0 passed; 1 failed;'):
    try:
        passed_tests(source)
    except AssertionError:
        pass
    else:
        raise AssertionError('cargo summary mutation escaped')
print(f'cargo test: {passed_tests(Path(sys.argv[1]).read_text())} passed; 0 failed')
PY

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
