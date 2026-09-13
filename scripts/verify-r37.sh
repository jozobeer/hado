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
if ! SEED=${SEED:-$(( (RANDOM << 15) | RANDOM ))} python3 - "$workdir" <<'PY'
from pathlib import Path
import os, random, sys
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
        add(f'37-a(3) arity={arity} labels={len(labels)}', declarations + f'def use (_: Int): Int{suffix(labels + ["Foo"])} => {call}\ndef main = Int.to_string (use 3)\n', 'String' + suffix(labels + ['Foo']))
add('37-a(5) abort', 'effect Stop\nabort stop\nhandler Stop as H\nabort stop => 9\ndef main = Int.to_string (with H -> stop)\n', 9, 'print')
add('37-a(5) capture', 'effect Ask\ncapture ask(value: Int): Unit\nhandler Ask as H\ncapture ask _ => resume ()\ndef main = Int.to_string (with H -> do ask 7 -> 8)\n', 8, 'print')
values = [-1, 0, 3, 5, 2147483647, rng.randrange(10, 10000)]
for value in values:
    add('37-b2(1) parameter', f'def f (raise: Int): Int => raise\ndef main = Int.to_string (f ({value}))\n', value, 'print')
    add('37-b2(2) function', f'def inc (n: Int): Int => n + 1\ndef f (raise: Fn(Int => Int)): Int => raise ({value})\ndef main = Int.to_string (f inc)\n', value + 1, 'print')
    add('37-b2(3) let', f'def main = Int.to_string (let raise = {value} -> raise + 1)\n', value + 1, 'print')
    add('37-b2(5) user operation', f'effect Foo\nreply tick: Int\ndef f (tick: Int): Int => tick\ndef main = Int.to_string (f ({value}))\n', value, 'print')
add('37-b2(4) unshadowed', 'def main = Int.to_string (raise { message = "x" })\n', 'x', 'error')
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
        coverage.add((label.split()[0], mode))
        assert (base / ident / 'main.hd').stat().st_size > 0
        assert (base / ident / 'expect').stat().st_size > 0
    assert coverage == {(f'37-a({i})', 'type') for i in range(1, 5)} | {('37-a(5)', 'print')} | {(f'37-b2({i})', 'error' if i == 4 else 'print') for i in range(1, 6)}
rows = (base / 'manifest.tsv').read_text().splitlines()
validate(rows)
for bad in ([], [row for row in rows if not row.endswith('\terror')], rows + rows[:1]):
    try:
        validate(bad)
    except AssertionError:
        pass
    else:
        raise AssertionError('incomplete or duplicate manifest accepted')
print('manifest controls: complete accepted; empty, all-success, duplicate rejected')
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
    if [ "$mode" = type ]; then
        cargo run --offline -q -- check --print-type "$dir/main.hd" >"$dir/out" 2>"$dir/err" || status=$?
    else
        cargo run --offline -q -- build "$dir/main.hd" >"$dir/build.out" 2>"$dir/build.err" || { cat "$dir/build.err"; return 1; }
        [ "$(wc -c <"$dir/build.err")" -eq 0 ] || return 1
        (cd "$dir" && ./main </dev/null) >"$dir/out" 2>"$dir/err" || status=$?
    fi
    cat "$dir/out" "$dir/err"
    output_matches "$mode" "$status" "$dir"
}
while IFS=$'\t' read -r ident label mode; do
    check "$label" probe "$mode" "$workdir/$ident"
done <"$workdir/manifest.tsv"
echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
