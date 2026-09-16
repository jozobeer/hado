#!/usr/bin/env bash
# R63 Boolean の pattern 分岐を Boolean 分岐の正準形にする（docs/plan.md 59 節）。
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
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado="$target_dir/debug/hado"

cat >"$workdir/harness.py" <<'PYEOF'
import re
import subprocess
from pathlib import Path

HADO = None


def run(root, name, source, *args):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    return subprocess.run([HADO, *args, str(path)], capture_output=True)


def source_of(root, name):
    return (Path(root) / name / 'main.hd').read_text()


def accepted(root, name, source):
    result = run(root, name, source, 'check')
    assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr.decode())


def check_code(root, name, source, code, hint='absent'):
    """`check` reports `code`. `hint` is the expected canonical form, or None for none at all."""
    result = run(root, name, source, 'check')
    assert result.returncode == 1, (name, result.returncode, result.stderr.decode())
    text = result.stderr.decode()
    match = re.match(r'[^:]+:(\d+):(\d+): error\[(E\d+)\]', text)
    assert match and match.group(3) == code, (name, code, text)
    if hint is None:
        assert 'canonical form:' not in text, (name, text)
    elif hint != 'absent':
        assert 'canonical form: ' + hint in text, (name, hint, text)
    return result


def check_fix(root, name, source, expected):
    """`check --fix` applies the rewrite, passes, and leaves exactly `expected`."""
    result = run(root, name, source, 'check', '--fix')
    assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr.decode())
    text = source_of(root, name)
    assert text == expected, (name, repr(text), repr(expected))
    return text


def fmt(root, name, source, expected):
    result = run(root, name, source, 'fmt')
    assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr.decode())
    text = source_of(root, name)
    assert text == expected, (name, repr(text), repr(expected))
    second = run(root, name, text, 'fmt')
    assert second.returncode == 0 and source_of(root, name) == text, (name, 'fmt must be idempotent')
    return text


def fmt_check(root, name, source, want):
    result = run(root, name, source, 'fmt', '--check')
    assert result.returncode == want, (name, want, result.returncode, result.stderr.decode())
    assert source_of(root, name) == source, (name, 'fmt --check must not write')
PYEOF

# --- 63-a: Boolean の腕 2 本は Boolean 分岐の正準形になる ---
r63_two_arms() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, check_code, check_fix, fmt, fmt_check

SOURCE = 'def f (b: Bool): Int => if b | true => 1 | false => 2\ndef main = f true\n'
EXPECTED = 'def f (b: Bool): Int => if b => 1 | 2\ndef main = f true\n'
check_code(sys.argv[1], 'a/hint', SOURCE, 'E520', 'if b => 1 | 2')
fixed = check_fix(sys.argv[1], 'a/fix', SOURCE, EXPECTED)
accepted(sys.argv[1], 'a/fixed', fixed)
assert fmt(sys.argv[1], 'a/fmt', SOURCE, EXPECTED) == EXPECTED
fmt_check(sys.argv[1], 'a/fixed2', EXPECTED, 0)
print('two arms: the Boolean pattern branch becomes the condition form')
PYEOF3
}

# --- 63-b: 腕 1 本 + fallback ---
r63_arm_fallback() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, check_code, check_fix, fmt

SOURCE = 'def f (b: Bool): Int => if b | true => 1 | 2\ndef main = f true\n'
EXPECTED = 'def f (b: Bool): Int => if b => 1 | 2\ndef main = f true\n'
check_code(sys.argv[1], 'b/hint', SOURCE, 'E520', 'if b => 1 | 2')
fixed = check_fix(sys.argv[1], 'b/fix', SOURCE, EXPECTED)
accepted(sys.argv[1], 'b/fixed', fixed)
assert fmt(sys.argv[1], 'b/fmt', SOURCE, EXPECTED) == EXPECTED
print('arm and fallback: the same canonical form')
PYEOF3
}

# --- 63-c: E503 と同じファイルでも一括で settle する ---
r63_mixed() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import check_code, fmt

MIXED = 'type T = | A | B\ndef g (t: T, b: Bool): Int => if t | A => 1 | _ => (if b | true => 2 | false => 3)\ndef main = g A true\n'
FIXED = 'type T = | A | B\ndef g (t: T, b: Bool): Int => if t | A => 1 | (if b => 2 | 3)\ndef main = g A true\n'
check_code(sys.argv[1], 'c/mixed', MIXED, 'E503')
assert fmt(sys.argv[1], 'c/mixed-fmt', MIXED, FIXED) == FIXED
print('mixed: both rewrites settle in one fmt')
PYEOF3
}

# --- 63-d: 除外と、枝の位置でない E520 ---
r63_controls() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, check_code, run, source_of

NESTED = 'def f (o: Bool Option): Int =>\n  if o | Some(true) => 1 | Some(false) => 2 | None => 3\ndef main = f (Some(true))\n'
TWO_COLUMNS = 'def f (b: Bool, c: Bool): Int =>\n  if b, c | true, true => 1 | true, false => 2 | false, true => 3 | false, false => 4\ndef main = f true true\n'
accepted(sys.argv[1], 'd/nested', NESTED)
accepted(sys.argv[1], 'd/columns', TWO_COLUMNS)
# 規則そのものは残る。
check_code(sys.argv[1], 'd/rule', 'def f (b: Bool): Int => if b | true => 1 | false => 2\ndef main = f true\n', 'E520')
# 引数の位置の E520 は診断だけで、書き換えを出さない。
ARGUMENT = 'def id (n: Int): Int => n\ndef f (b: Bool): Int => id (if b | true => 1 | false => 2)\ndef main = f true\n'
check_code(sys.argv[1], 'd/argument', ARGUMENT, 'E520', None)
result = run(sys.argv[1], 'd/argument', ARGUMENT, 'check', '--fix')
assert result.returncode == 1, (result.returncode, result.stderr.decode())
assert source_of(sys.argv[1], 'd/argument') == ARGUMENT, 'check --fix must not write outside a branch'
print('controls: the exclusions are accepted and an argument stays unwritten')
PYEOF3
}

# --- 63-e: 仕様と層 ---
r63_docs() {
    python3 - <<'PYEOF3'
from pathlib import Path

tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r63' in full.split(), 'r63 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert 'verify-r63' in trace, 'spec-trace does not name verify-r63'
plan = Path('docs/plan.md').read_text()
assert '## 59. ' in plan, 'plan has no section 59'
spec = Path('docs/spec.md').read_text()
section = spec.split('## 24. ', 1)[1].split('\n## ', 1)[0]
assert '書き換え' in section, '§24 does not record the rewrite'
assert 'canonical form' in section, '§24 does not name the canonical form'
print('docs: §24, the trace row, the tier and the plan')
PYEOF3
}

# --- 63-f: コーパスロック ---
r63_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '63-a two Boolean arms become a Boolean branch' r63_two_arms
check '63-b an arm and a fallback become a Boolean branch' r63_arm_fallback
check '63-c E503 and E520 settle together' r63_mixed
check '63-d the exclusions and the argument position' r63_controls
check '63-e spec, trace, tier' r63_docs
check '63-f corpus lock' r63_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
