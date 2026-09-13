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
import json, os, random, re, sys
base = Path(sys.argv[1])
seed = int(os.environ['SEED'])
rng = random.Random(seed)
manifest = []

def add(group, label, expression, expected, mode='print', handler=''):
    directory = base / str(len(manifest))
    directory.mkdir()
    (directory / 'main.hd').write_text('def main = ' + expression + '\n' + handler)
    (directory / 'expect').write_text(str(expected) + '\n')
    manifest.append((directory.name, group + ' ' + label, mode))

def literal(s):
    if '\0' in s or '\r' in s:
        return '(Bytes.to_string (Bytes.of_list [' + ', '.join(map(str, s.encode())) + ']))'
    return json.dumps(s, ensure_ascii=False)

def oracle(s):
    valid = re.fullmatch(r'-?[0-9]+', s) is not None and -2**63 <= int(s) < 2**63
    return int(s) if valid else 'InvalidInt'

bodies = ['', '0', str(rng.randint(1, 9)), str(rng.randint(10, 999999)),
          '007', '00', 'a42', '4a2', '42a', ' 42', '4 2', '42 ',
          '1_000', '4.2', '４２', '4あ2', '4\0' + '2', '\t42', '42\n', '42\r']
inputs = {sign + body for sign in ['', '-', '+', '--'] for body in bodies}
inputs.update(['-', '+', '--'])
boundaries = [str(2**63 - 1), str(2**63), str(-2**63), str(-2**63 - 1),
              str(2**64), str(10**19), str(10**20), '9' * 37]
inputs.update(boundaries)
for value in ['5', str(2**63 - 1), str(2**63)]:
    inputs.update(['0' * 25 + value, '-' + '0' * 25 + value])
inputs.update(['0' * 1000, '-' + '0' * 1000])
handler = 'handler Error as H\nabort raise e => e.message\n'
for s in sorted(inputs):
    expected = oracle(s)
    label = json.dumps(s, ensure_ascii=False)
    call = 'Int.parse ' + literal(s)
    if expected != 'InvalidInt':
        add('36-a', label, call, expected)
    else:
        add('36-b', label, call, expected, 'error')
        # 各拒否形に、文法を修復し範囲内へ戻した対照を伴わせる。
        digits = ''.join(c for c in s if c in '0123456789') or '0'
        neighbor = (-1 if s.startswith('-') else 1) * int(digits)
        neighbor = max(-2**63, min(2**63 - 1, neighbor))
        add('36-b', 'valid control for ' + label,
            'Int.parse ' + literal(str(neighbor)), neighbor)
    add('36-c', label, 'with H -> Int.to_string (' + call + ')', expected, handler=handler)
values = [-2**63, 2**63 - 1, 0] + [rng.randrange(-2**63, 2**63) for _ in range(24)]
for n in values:
    value = '(-9223372036854775807 - 1)' if n == -2**63 else '(' + str(n) + ')'
    add('36-a', 'roundtrip ' + str(n), 'Int.parse (Int.to_string ' + value + ')', n)
assert manifest
(base / 'manifest.tsv').write_text(''.join('\t'.join(row) + '\n' for row in manifest))
print(f'seed={seed} classified_inputs={len(inputs)} roundtrips={len(values)} probes={len(manifest)}')
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

def validate(rows):
    assert rows, 'empty manifest'
    seen, coverage = set(), set()
    for row in rows:
        kind, label, mode = row.split('\t')
        assert kind.isdecimal() and kind not in seen
        seen.add(kind)
        group = label.split()[0]
        assert group in ('36-a', '36-b', '36-c')
        assert mode in ('print', 'error')
        coverage.add((group, mode))
        assert (base / kind / 'main.hd').is_file()
        assert (base / kind / 'expect').stat().st_size > 0
    assert coverage == {('36-a', 'print'), ('36-b', 'print'), ('36-b', 'error'), ('36-c', 'print')}

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
check '36-a/36-b/36-c output controls: equal, empty, different, newline, status' output_controls
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
check '36-d Int member types match spec section 49' \
    python3 - docs/spec.md stdlib/int.hd "$workdir" <<'PY'
from pathlib import Path
import re
import sys

module = 'Int'


def only_block(pattern, text):
    blocks = re.findall(pattern, text, flags=re.M | re.S)
    if len(blocks) != 1:
        raise ValueError(f"expected one block, found {len(blocks)}: {pattern}")
    return blocks[0]


def member_block(spec):
    section = only_block(r"^# 49\.[^\n]*\n(.*?)(?=^# |\Z)", spec)
    return only_block(r"member の型は次のとおり[^\n]*\n\s*```ocaml\n(.*?)```", section)


def argument_type(argument):
    match = re.fullmatch(r"\s*\w+\s*:\s*(.+)", argument)
    if not match:
        raise ValueError(f"invalid argument: {argument}")
    return " ".join(match[1].split())


def members_of(source):
    body = only_block(rf"^module {module}[ \t]*\n(.*?)(?=^(?:module|implement)\b|\Z)", source)
    members = {}
    for line in body.splitlines():
        line = line.split("//", 1)[0].strip()
        if not line:
            continue
        match = re.fullmatch(r"def\s+(\w+)\s*\(([^()]*)\)\s*:\s*(.+?)\s+(Pure|\{[^{}]+\})", line)
        if not match or match[1] in members:
            raise ValueError(f"invalid or duplicate member: {line}")
        members[match[1]] = (
            tuple(argument_type(arg) for arg in match[2].split(",")) if match[2].strip() else (),
            " ".join(match[3].split()),
            re.sub(r"\s+", "", match[4]),
        )
    if not members:
        raise ValueError(f"empty {module} contract")
    return members


def differences(expected, actual):
    return {name: (expected.get(name), actual.get(name))
            for name in expected.keys() | actual.keys()
            if expected.get(name) != actual.get(name)}


spec = Path(sys.argv[1]).read_text()
block = member_block(spec)
expected = members_of(block)
assert not differences(expected, members_of(block)), "equal contracts must match"
contract_block = only_block(rf"^(module {module}[ \t]*\n.*?)(?=^module|\Z)", block)

# 仕様の写しを再読して照合する。期待値を被験側から取る判定では、この対照が落ちる。
changed, count = re.subn(r"^(def to_string\([^\n]*\):[^\n]*) Pure$", r"\1 {Error}", contract_block, flags=re.M)
assert count == 1, "spec mutation must change member to_string exactly once"
copy = Path(sys.argv[3]) / "spec-effect.md"
copy.write_text(spec.replace(contract_block, changed, 1))
changed_members = members_of(member_block(copy.read_text()))
assert set(differences(changed_members, expected)) == {"to_string"}, "spec effect mutation escaped"
print("   (対照: §49 の写しの member to_string Pure → {Error} を検出)")

for pattern, replacement in (
    (r"^(def to_string\([^:]+: [^)]*)(\))", r"\1 List\2"),
    (r"^(def to_string\([^)]*\): \w+)", r"\1 List"),
):
    changed, count = re.subn(pattern, replacement, contract_block, flags=re.M)
    assert count == 1, "type mutation must change member to_string exactly once"
    assert set(differences(expected, members_of(changed))) == {"to_string"}, "type mutation escaped"

for empty in ("", f"module {module}\n"):
    try:
        members_of(empty)
    except ValueError:
        pass
    else:
        raise AssertionError("empty contract accepted")
try:
    member_block("")
except ValueError:
    pass
else:
    raise AssertionError("empty spec accepted")
print("   (対照: 全件一致は受理、空の仕様・契約と引数型・戻り型の不一致は拒否)")

actual = members_of(Path(sys.argv[2]).read_text())
mismatches = differences(expected, actual)
for name in sorted(expected.keys() | actual.keys()):
    if name in mismatches:
        print(f"   {module}.{name}: expected={expected.get(name)}, actual={actual.get(name)}")
    else:
        print(f"   {module}.{name}: args={actual[name][0]}, result={actual[name][1]}, effect={actual[name][2]}")
sys.exit(1 if mismatches else 0)
PY
while IFS=$'\t' read -r kind label mode; do
    dir="$workdir/$kind"
    case "$mode" in
        print) check "$label" check_prints "$dir" ;;
        error) check "$label" expects_error "$dir" ;;
        *) exit 1 ;;
    esac
done <"$workdir/manifest.tsv"
echo "PASSED: $passed/$total"
[ "$total" -gt 0 ] && [ "$passed" -eq "$total" ]
