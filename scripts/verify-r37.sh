#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
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
import json, os, random, sys
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
        declarations = ('effect Bar\nreply other: Int\n' if 'Bar' in labels else '')
        declarations += f'effect Foo\nreply pick{param}: Int{suffix(labels)}\n'
        handled = declarations + f'handler Foo as H\nreply pick{arm} => 7\ndef main = Int.to_string (with H -> {call})\n'
        group = '37-a(2)' if not labels else '37-a(1)' if len(labels) == 1 else '37-a(4)'
        add(f'{group} arity={arity}', handled, 'String' + suffix(labels))
        add(f'37-a(3) arity={arity} labels={len(labels)}', declarations + f'def use (n: Int): Int{suffix(labels + ["Foo"])} => {call} + n\ndef main = Int.to_string (use 3)\n', 'String' + suffix(labels + ['Foo']))
        add(f'37-a(3) inferred arity={arity} labels={len(labels)}', declarations + f'def use (n: Int) => {call} + n\ndef main = Int.to_string (use 3)\n', 'String' + suffix(labels + ['Foo']))
add('37-a(5) abort', 'effect Stop\nabort stop\nhandler Stop as H\nabort stop => 9\ndef main = Int.to_string (with H -> stop)\n', 9, 'print')
add('37-a(5) capture', 'effect Ask\ncapture ask(value: Int): Unit\nhandler Ask as H\ncapture ask _ => resume ()\ndef main = Int.to_string (with H -> do ask 7 -> 8)\n', 8, 'print')
values = [-1, 0, 3, 5, 2147483647, rng.randrange(10, 10000)]
for value in values:
    add('37-b2(1) parameter', f'def f (raise: Int): Int => raise\ndef main = Int.to_string (f ({value}))\n', value, 'print')
    add('37-b2(2) function', f'def inc (n: Int): Int => n + 1\ndef f (raise: Fn(Int => Int)): Int => raise ({value})\ndef main = Int.to_string (f inc)\n', value + 1, 'print')
    add('37-b2(3) let', f'def main = Int.to_string (let raise = {value} -> raise + 1)\n', value + 1, 'print')
    add('37-b2(5) user operation', f'effect Foo\nreply tick: Int\ndef f (tick: Int): Int => tick\ndef main = Int.to_string (f ({value}))\n', value, 'print')
add('37-b2(5) capture let', 'effect Foo\ncapture tick(value: Int): Unit\ndef f (tick: Int): Int => let x = tick -> x + 1\ndef main = Int.to_string (f 8)\n', 9, 'print')
add('37-b2(4) unshadowed', 'def main = Int.to_string (raise { message = "x" })\n', 'x', 'error')

def quote(value):
    return json.dumps(value, ensure_ascii=False)
def runtime(label, source, expected, arguments=(), environment=None, error=False):
    directory = add(label, source, expected, 'error' if error else 'print')
    (directory / 'run.json').write_text(json.dumps([arguments, environment or {}]))
    return directory
bad = os.environ['R37_BAD']
for shape, arguments in [('empty', []), ('one', ['one']), ('many', ['a', '日本語', str(rng.randrange(10000))]), ('empty-string', ['', 'tail']), ('invalid', [bad])]:
    runtime('37-c(1) '+shape, 'def main = args\n',
            'InvalidUtf8' if shape == 'invalid' else '[' + ', '.join(map(quote, arguments)) + ']',
            arguments, error=shape == 'invalid')
key = 'HADO_R37_' + str(rng.randrange(1000000))
for shape, value in [('set', 'value日本語'), ('unset', None), ('empty', ''), ('invalid', bad)]:
    runtime('37-c(2) '+shape, f'def main = env {quote(key)}\n',
            'InvalidUtf8' if shape == 'invalid' else 'None' if value is None else 'Some(' + quote(value) + ')',
            environment={key: value}, error=shape == 'invalid')
runtime('37-c(2) invalid-name', 'def first | [head, ..._] => head | [] => ""\ndef main = env (first args)\n', 'InvalidUtf8', [bad], {bad: 'exists'}, True)
directory = add('37-c(3) cwd', 'def main = cwd\n', '', 'print')
(directory / 'expect').write_text(str(directory.resolve()) + '\n')
add('37-b prelude', 'def main = args\n', 'String List {Env, Error}')
handler = 'handler Env as MyEnv\nreply args => ["override"]\nreply env _ => Some("overridden")\nreply cwd => "virtual"\n'
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
reject('37-b user', 'effect Foo\nreply pick: Int\ndef main = pick\n', 'E706', 'def main')
reject('37-d(4) missing-arm', 'handler Env as H\nreply args => ["x"]\nreply env _ => None\ndef main = 0\n', 'E605', 'handler Env')
reject('37-e(1) effect', 'effect Env\nreply custom: Int\ndef main = 0\n', 'E205', 'Env')
reject('37-e module', 'module Env\ndef value: Int Pure\n', 'E205', 'Env')
for sub in (False, True):
    for multiple in (False, True):
        for shape, source, anchor in [
            ('def', 'def args = 1\n', 'args'),
            ('implementation', 'module M\ndef args: Int Pure\nimplement M as M\ndef args = 1\n', 'args ='),
            ('operation', 'effect Foo\nreply args: Int\n', 'args'),
        ]:
            reject('37-e(2) '+shape, source + 'def main = 0\n', 'E205', anchor, sub, multiple)
            if sub and multiple:
                (base / str(len(manifest)-1) / 'sub' / 'peer.hd').write_text('')
for effect, arms, body, expected in [
    ('Env', 'reply args => ["sub"]\nreply env _ => None\nreply cwd => "subcwd"\n', 'args', '["sub"]'),
    ('Error', 'abort raise e => e.message\n', 'raise { message = "caught" }', 'caught'),
]:
    directory = add('37-e(3) handler', f'def main = sub::M.value\n', expected, 'print')
    (directory / 'sub').mkdir()
    (directory / 'sub' / 'sub.hd').write_text(f'handler {effect} as H\n{arms}module M\ndef value: '+('String List {Error}' if effect == 'Env' else 'String Pure')+f'\nimplement M as M\ndef value = with H -> {body}\n')
directory = add('37-e(4) visible', 'def main = sub::M.value\n', '[]', 'print')
(directory / 'sub').mkdir()
(directory / 'sub' / 'sub.hd').write_text('module M\ndef value: String List {Env, Error}\nimplement M as M\ndef value = args\n')
for contract in ('List', 'Option', 'String'):
    reject('37-e(5) contract', f'handler {contract} as H\nreply invented => 0\n', 'E605', 'handler', True)

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
import sys
base = Path(sys.argv[1])
def validate(rows):
    assert rows, 'empty manifest'
    seen, coverage = set(), set()
    for row in rows:
        ident, label, mode = row.split('\t')
        assert ident.isdecimal() and ident not in seen
        seen.add(ident)
        coverage.add((' '.join(token.split('=')[0] for token in label.split()), mode))
        assert (base / ident / 'main.hd').stat().st_size > 0
        assert (base / ident / 'expect').stat().st_size > 0
    assert seen == {p.name for p in base.iterdir() if p.is_dir() and p.name.isdecimal()}
    assert coverage == {
        ('37-a(1) arity', 'type'), ('37-a(2) arity', 'type'), ('37-a(4) arity', 'type'),
        ('37-a(3) arity labels', 'type'), ('37-a(3) inferred arity labels', 'type'),
        ('37-a(5) abort', 'print'), ('37-a(5) capture', 'print'),
        ('37-b2(1) parameter', 'print'), ('37-b2(2) function', 'print'),
        ('37-b2(3) let', 'print'), ('37-b2(5) user operation', 'print'),
        ('37-b2(5) capture let', 'print'), ('37-b2(4) unshadowed', 'error'),
    } | {
        ('37-c(1) ' + shape, 'error' if shape == 'invalid' else 'print')
        for shape in ('empty', 'one', 'many', 'empty-string', 'invalid')
    } | {
        ('37-c(2) ' + shape, 'error' if shape.startswith('invalid') else 'print')
        for shape in ('set', 'unset', 'empty', 'invalid', 'invalid-name')
    } | {
        ('37-c(3) cwd', 'print'), ('37-b prelude', 'type'), ('37-b user', 'reject'),
        ('37-d(2) override', 'print'), ('37-d(3) error-handler', 'print'), ('37-d(4) missing-arm', 'reject'),
        ('37-e(1) effect', 'reject'), ('37-e module', 'reject'),
        *[('37-e(2) ' + shape, 'reject') for shape in ('def', 'implementation', 'operation')],
        ('37-e(3) handler', 'print'), ('37-e(4) visible', 'print'), ('37-e(5) contract', 'reject'),
        *[('37-b2(6) ' + shape, 'print') for shape in ('parameter', 'function', 'let')],
    }, sorted(coverage)
rows = (base / 'manifest.tsv').read_text().splitlines()
validate(rows)
def without(shape):
    return [row for row in rows
            if ' '.join(t.split('=')[0] for t in row.split('\t')[1].split()) != shape]
missing = [without(shape) for shape in
           ('37-a(5) abort', '37-a(5) capture', '37-b2(5) capture let', '37-b2(2) function',
            '37-a(3) inferred arity labels')]
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
    elif [ "$mode" = type ]; then
        cargo run --offline -q -- check --print-type "$dir/main.hd" >"$dir/out" 2>"$dir/err" || status=$?
    else
        cargo run --offline -q -- build "$dir/main.hd" >"$dir/build.out" 2>"$dir/build.err" || { cat "$dir/build.err"; return 1; }
        [ "$(wc -c <"$dir/build.err")" -eq 0 ] || return 1
        python3 - "$dir" <<'PY'
from pathlib import Path
import json, os, subprocess, sys
base = Path(sys.argv[1])
arguments, overrides = json.loads((base / 'run.json').read_text()) if (base / 'run.json').exists() else ([], {})
environment = dict(os.environ)
for key, value in overrides.items():
    if value is None: environment.pop(key, None)
    else: environment[key] = value
with (base / 'out').open('wb') as out, (base / 'err').open('wb') as err:
    result = subprocess.run([str(base / 'main'), *arguments], cwd=base, env=environment, stdin=subprocess.DEVNULL, stdout=out, stderr=err, timeout=30)
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
echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
