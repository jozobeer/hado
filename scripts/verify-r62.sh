#!/usr/bin/env bash
# R62 `_` の fallback と Boolean 分岐の正準形（docs/plan.md 58 節）。
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

cat >"$workdir/harness.py" <<'PY'
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


def no_rewrite(root, name, source, code):
    """The diagnostic fires, no rewrite is offered, and neither tool writes."""
    check_code(root, name, source, code, hint=None)
    result = run(root, name, source, 'check', '--fix')
    assert result.returncode == 1, (name, result.returncode)
    assert source_of(root, name) == source, (name, 'check --fix must not write')
    fmt_check(root, name, source, 0)
PY

# --- 62-a: `_` の catch-all は fallback の正準形になる ---
r62_fallback() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, check_code, check_fix, fmt, fmt_check

SOURCE = 'type T = | A | B\ndef f (t: T): Int => if t | A => 1 | _ => 0\ndef main = f A\n'
EXPECTED = 'type T = | A | B\ndef f (t: T): Int => if t | A => 1 | 0\ndef main = f A\n'
check_code(sys.argv[1], 'a/hint', SOURCE, 'E503', '| 0')
fixed = check_fix(sys.argv[1], 'a/fix', SOURCE, EXPECTED)
accepted(sys.argv[1], 'a/fixed', fixed)
assert fmt(sys.argv[1], 'a/fmt', SOURCE, EXPECTED) == EXPECTED
fmt_check(sys.argv[1], 'a/fixed2', EXPECTED, 0)
print('fallback: the catch-all becomes the fallback in both fixers')
PY
}

# --- 62-b: Boolean の pattern 分岐は Boolean 分岐の正準形になる ---
r62_boolean_branch() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, check_code, check_fix, fmt

TWO_ARMS = 'def f (b: Bool): Int => if b | true => 1 | false => 2\ndef main = f true\n'
TWO_ARMS_EXPECTED = 'def f (b: Bool): Int => if b => 1 | 2\ndef main = f true\n'
ARM_AND_FALLBACK = 'def f (b: Bool): Int => if b | true => 1 | 2\ndef main = f true\n'
ARM_AND_FALLBACK_EXPECTED = 'def f (b: Bool): Int => if b => 1 | 2\ndef main = f true\n'
check_code(sys.argv[1], 'b/arms-hint', TWO_ARMS, 'E520', 'if b => 1 | 2')
fixed = check_fix(sys.argv[1], 'b/arms', TWO_ARMS, TWO_ARMS_EXPECTED)
accepted(sys.argv[1], 'b/arms-ok', fixed)
fixed = check_fix(sys.argv[1], 'b/fallback', ARM_AND_FALLBACK, ARM_AND_FALLBACK_EXPECTED)
accepted(sys.argv[1], 'b/fallback-ok', fixed)
assert fmt(sys.argv[1], 'b/fmt', TWO_ARMS, TWO_ARMS_EXPECTED) == TWO_ARMS_EXPECTED
print('boolean branch: both out-of-canonical forms become the condition form')
PY
}

# --- 62-c: §24 の除外はそのまま受理される ---
r62_exclusions() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted

NESTED = '''def f (o: Bool Option): Int =>
  if o | Some(true) => 1 | Some(false) => 2 | None => 3
def main = f (Some(true))
'''
TWO_COLUMNS = '''def f (b: Bool, c: Bool): Int =>
  if b, c | true, true => 1 | true, false => 2 | false, true => 3 | false, false => 4
def main = f true true
'''
accepted(sys.argv[1], 'c/nested', NESTED)
accepted(sys.argv[1], 'c/columns', TWO_COLUMNS)
print('exclusions: a nested Bool literal and two condition columns are not E520')
PY
}

# --- 62-d: 書き換える範囲にコメントがあれば出さない ---
r62_comment() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import no_rewrite, source_of

SOURCE = 'type T = | A | B\ndef f (t: T): Int => if t | A => 1 | _ => // why\n  0\ndef main = f A\n'
no_rewrite(sys.argv[1], 'd/comment', SOURCE, 'E503')
assert '// why' in source_of(sys.argv[1], 'd/comment'), 'the comment must survive'
print('comment: a comment inside the arm keeps the rewrite away')
PY
}

# --- 62-e: 既存の書き換えはそのまま ---
r62_existing() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import check_code, check_fix

# E501: そのまま返す束縛。R57 の門番と同じ形。
SOURCE = 'def main =\n  let v = 1 ->\n  v\n'
EXPECTED = 'def main =\n  1\n'
check_fix(sys.argv[1], 'e/e501', SOURCE, EXPECTED)
# E503 と E520 を同じファイルに置くと、一括の fmt が両方を直す。
MIXED = 'type T = | A | B\ndef f (t: T, b: Bool): Int => if t | A => 1 | _ => (if b | true => 2 | false => 3)\ndef main = f A true\n'
FIXED = 'type T = | A | B\ndef f (t: T, b: Bool): Int => if t | A => 1 | (if b => 2 | 3)\ndef main = f A true\n'
check_code(sys.argv[1], 'e/mixed', MIXED, 'E503')
from harness import fmt
assert fmt(sys.argv[1], 'e/mixed-fmt', MIXED, FIXED) == FIXED
print('existing: an earlier rewrite still applies, and both new ones settle together')
PY
}

# --- 62-f: 文書と層 ---
r62_docs() {
    python3 - <<'PY'
from pathlib import Path

tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r62' in full.split(), 'r62 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert 'verify-r62' in trace, 'spec-trace does not name verify-r62'
plan = Path('docs/plan.md').read_text()
assert '## 58. ' in plan, 'plan has no section 58'
spec = Path('docs/spec.md').read_text()
section = spec.split('## 24. ', 1)[1].split('\n## ', 1)[0]
assert 'canonical form' in section, '§24 does not name the canonical form of the rewrite'
section = spec.split('## 29. ', 1)[1].split('\n## ', 1)[0]
assert 'canonical form' in section, '§29 does not name the canonical form of the rewrite'
print('docs: §24, §29, the trace row, the tier and the plan')
PY
}

# --- 62-g: コーパスロック ---
r62_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '62-a the catch-all underscore becomes the fallback' r62_fallback
check '62-b Boolean pattern arms become a Boolean branch' r62_boolean_branch
check '62-c the exclusions of §24 are accepted' r62_exclusions
check '62-d a comment inside the arm keeps the rewrite away' r62_comment
check '62-e the earlier rewrites still apply' r62_existing
check '62-f spec, trace, tier' r62_docs
check '62-g corpus lock' r62_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
