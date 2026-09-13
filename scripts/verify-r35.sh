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

if ! SEED=${SEED:-$(( (RANDOM << 15) | RANDOM ))} PAD=$RANDOM python3 - "$workdir" <<'PY'
from pathlib import Path
import json, os, random, sys
base = Path(sys.argv[1])
seed = int(os.environ['SEED'])
rng = random.Random(seed)
pad = int(os.environ['PAD'])
manifest = []
def add(label, source, expected, mode='print'):
    directory = base / str(len(manifest))
    directory.mkdir()
    (directory / 'main.hd').write_text(source + '\n')
    (directory / 'expect').write_text(expected + '\n')
    manifest.append((directory.name, label, mode))
def literal(ty, value):
    if ty == 'Bool': return str(value).lower()
    if ty == 'String': return json.dumps(value, ensure_ascii=False)
    if ty == 'Char': return "'" + value + "'"
    if ty == 'Cell': return '{ value = ' + str(value) + ' }'
    if ty == 'Int Array': return '(Array.of_list ' + show('Int', value) + ')'
    return str(value)
def show(ty, values):
    return '[' + ', '.join(literal(ty, v) for v in values) + ']'
def item(ty, i):
    return {'Int': lambda: i * 65539 - 500000,
            'Float': lambda: i + 0.25,
            'Bool': lambda: i % 2 == 0,
            'String': lambda: 'あ😀' + str(i),
            'Char': lambda: chr(0x40000 + i),
            'Cell': lambda: i * 65539,
            'Int Array': lambda: [i * 65539, i + 1]}[ty]()
def generator(ty):
    value = {'Int': 'n * 65539 - 500000', 'Float': '0.25', 'Bool': 'n % 2 == 0',
             'String': 'Int.to_string n', 'Char': "'𐀀'", 'Cell': '{ value = n * 65539 }',
             'Int Array': 'Array.of_list [n * 65539, n + 1]'}[ty]
    return f'def rec generate n => if n <= 0 => [] | [{value}, ...generate (n - 1)]\n'
lengths = [0, 1, rng.randint(5, 17), 3, 30000]
for ty in ['Int', 'Float', 'Bool', 'String', 'Char', 'Cell', 'Int Array']:
    for length in lengths:
        prefix = 'type Cell = { value: Int }\n' if ty == 'Cell' else ''
        prefix += f'def make (xs: {ty} List) => Array.of_list xs\n'
        values = [item(ty, i) for i in range(length)]
        if length == 30000:
            prefix += generator(ty)
            xs = f'(generate {length})'
            values = list(reversed(values))
            if ty == 'Int': values = [i * 65539 - 500000 for i in range(length, 0, -1)]
            if ty == 'Float': values = [0.25] * length
            if ty == 'Bool': values = [i % 2 == 0 for i in range(length, 0, -1)]
            if ty == 'String': values = [str(i) for i in range(length, 0, -1)]
            if ty == 'Char': values = ['𐀀'] * length
            if ty == 'Cell': values = [i * 65539 for i in range(length, 0, -1)]
            if ty == 'Int Array': values = [[i * 65539, i + 1] for i in range(length, 0, -1)]
        else:
            xs = show(ty, values)
        start = prefix + f'def main = let a = make {xs} -> '
        label = f'35-a {ty} length={length}'
        add(label + ' length', start + 'Array.length a', str(len(values)))
        if ty != 'Int Array' and length != 30000:
            add(label + ' roundtrip', start + 'Array.to_list a', show(ty, values))
        indices = sorted({0, length - 1, rng.randrange(length)}) if length else []
        for index in indices:
            if ty == 'Int Array':
                observed = f'[Array.length (Array.get a {index}), Array.get (Array.get a {index}) 0]'
                expected = show('Int', [len(values[index]), values[index][0]])
            else:
                observed = f'[Array.get a {index}]'
                expected = show(ty, [values[index]])
            add(label + f' get={index}', start + observed, expected)
            replacement = item(ty, length + 91)
            new_values = values.copy()
            new_values[index] = replacement
            changed = f'Array.set a {index} {literal(ty, replacement)}'
            if ty != 'Int Array' and length != 30000:
                add(label + f' set={index} immutable', start + f'let b = {changed} -> [Array.to_list b, Array.to_list a]',
                    '[' + show(ty, new_values) + ', ' + show(ty, values) + ']')
            else:
                observe = (lambda a: f'Array.get (Array.get {a} {index}) 0') if ty == 'Int Array' else (lambda a: f'Array.get {a} {index}')
                result_ty = 'Int' if ty == 'Int Array' else ty
                after = replacement[0] if ty == 'Int Array' else replacement
                before = values[index][0] if ty == 'Int Array' else values[index]
                add(label + f' set={index} immutable', start + f'let b = {changed} -> [{observe("b")}, {observe("a")}]', show(result_ty, [after, before]))
        for index in sorted({-1, length, length + 1}):
            for member in ['get', 'set']:
                call = f'Array.{member} a ({index})'
                if member == 'set': call += ' ' + literal(ty, item(ty, 7))
                if member == 'set' or ty == 'Int Array': call = f'Array.length ({call})'
                add(f'35-b {ty} length={length} {member}={index}', start + call, 'IndexOutOfRange', 'error')
for member in ['get', 'set']:
    expr = 'Array.get a 1' if member == 'get' else 'Array.length (Array.set a 1 9)'
    add(f'35-b handler {member}', 'def main = let a = Array.of_list [5] -> with H -> ' + expr + '\nhandler Error as H\nabort raise e => String.length e.message', str(len('IndexOutOfRange')))
large = 'def rec generate n => if n <= 0 => [] | [n, ...generate (n - 1)]\ndef main = let a = Array.of_list (generate 30000) -> '
values = list(range(30000, 0, -1))
add('35-c of_list 30000', large + '[Array.length a, Array.get a 29999, Array.get a 0]', show('Int', [len(values), values[-1], values[0]]), 'timeout')
changed = values.copy()
changed[-1] = -73
add('35-c set 30000', large + 'let b = Array.set a 29999 (-73) -> [Array.length b, Array.get b 29999, Array.get b 0, Array.get a 29999]', show('Int', [len(changed), changed[-1], changed[0], values[-1]]), 'timeout')
blank, indent = pad % 9 + 1, (pad // 9) % 13 + 1
source = '\n' * blank + ' ' * indent + 'type A Array = { value: A }\ndef main = 0'
add('35-f reserved Array anchor', source, f'{blank + 1}:{indent + len("type A ") + 1}: error[E205]:', 'code:E205')
add('35-f Int Array annotation', 'def f (xs: Int Array) => xs\ndef main = Array.length (f (Array.of_list [3, 5]))', '2')
shared = ('def main = read ({ value = Store.make (Array.of_list [3, 5, 7]) })\n'
          'def read box => Array.length box.value\ntype Box = { value: Store.T }\n'
          'module Store\ntype T\ndef make(value: Int Array): T Pure\n'
          'implement Store as Store\nshared type T = Int Array\ndef make value => value')
add('35-f2 shared Array before record registration', shared, '3')
add('35-f2 opaque negative control', shared.replace('shared type', 'type'), 'error[E305]:', 'code:E305')
assert manifest
(base / 'manifest.tsv').write_text(''.join('\t'.join(row) + '\n' for row in manifest))
print(f'seed={seed} types=7 lengths={lengths} probes={len(manifest)}')
PY
then
    echo 'NG: probe generator failed' >&2
    exit 1
fi

validate_manifest() {
    python3 - "$workdir" <<'PY'
from pathlib import Path
import sys
base = Path(sys.argv[1])
rows = (base / 'manifest.tsv').read_text().splitlines()
assert rows, 'empty manifest'
seen = set()
for row in rows:
    kind, label, mode = row.split('\t')
    assert kind.isdecimal() and kind not in seen
    seen.add(kind)
    assert mode in ('print', 'error', 'timeout', 'code:E205', 'code:E305')
    assert (base / kind / 'main.hd').is_file()
    assert (base / kind / 'expect').is_file()
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
check '35-a/35-b output controls: equal, empty, different, newline, status' output_controls
build_case() {
    cargo run --offline -q -- build "$1/main.hd" >"$1/build.out" 2>"$1/build.err" || { cat "$1/build.err"; return 1; }
    [ "$(wc -c <"$1/build.err")" -eq 0 ]
}
check_prints() {
    local dir="$1" status=0
    build_case "$dir" || return 1
    "$dir/main" >"$dir/out" 2>"$dir/err" || status=$?
    output_matches print "$status" "$dir"
}
expects_error() {
    local dir="$1" status=0
    build_case "$dir" || return 1
    "$dir/main" >"$dir/out" 2>"$dir/err" || status=$?
    output_matches error "$status" "$dir"
}
within_deadline() {
    local dir="$1" status=0
    /usr/bin/time -f 'build+run=%es' -o "$dir/time" timeout 2 bash -c '
        cargo run --offline -q -- build "$1/main.hd" >"$1/build.out" 2>"$1/build.err" &&
        "$1/main" >"$1/out" 2>"$1/err"
    ' _ "$dir" || status=$?
    cat "$dir/time"
    [ "$status" -eq 0 ] || return 1
    cat "$dir/out"
    [ "$(wc -c <"$dir/build.err")" -eq 0 ] && output_matches print "$status" "$dir"
}
expects_code() {
    local dir="$1" status=0
    cargo run --offline -q -- check "$dir/main.hd" >"$dir/out" 2>"$dir/err" || status=$?
    [ "$status" -eq 1 ] && [ "$(wc -c <"$dir/out")" -eq 0 ] || return 1
    python3 - "$dir" <<'PY'
from pathlib import Path
import sys
base = Path(sys.argv[1])
expected = (base / 'expect').read_text().rstrip('\n')
actual = (base / 'err').read_text()
assert len(actual.splitlines()) == 1, actual
assert expected in actual, actual
PY
}
while IFS=$'\t' read -r kind label mode; do
    dir="$workdir/$kind"
    case "$mode" in
        print) check "$label" check_prints "$dir" ;;
        error) check "$label" expects_error "$dir" ;;
        timeout) check "$label" within_deadline "$dir" ;;
        code:*) check "$label" expects_code "$dir" ;;
        *) exit 1 ;;
    esac
done <"$workdir/manifest.tsv"
echo "PASSED: $passed/$total"
[ "$total" -gt 0 ] && [ "$passed" -eq "$total" ]
