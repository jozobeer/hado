#!/usr/bin/env bash
# tier: full
# R62 `_` の catch-all を fallback の正準形にする（`docs/rounds/r62.md`）。
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

# --- 62-a: `_` の catch-all は fallback の正準形になる ---
r62_fallback() {
    python3 - "$workdir" "$hado" <<'PYEOF'
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
PYEOF
}

# --- 62-b: `_ =>` の後ろのコメントは残る ---
r62_comment() {
    python3 - "$workdir" "$hado" <<'PYEOF'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, check_code, check_fix, source_of

SOURCE = 'type T = | A | B\ndef f (t: T): Int => if t | A => 1 | _ => // why\n  0\ndef main = f A\n'
EXPECTED = 'type T = | A | B\ndef f (t: T): Int => if t | A => 1 | // why\n  0\ndef main = f A\n'
check_code(sys.argv[1], 'b/hint', SOURCE, 'E503', '| 0')
fixed = check_fix(sys.argv[1], 'b/comment', SOURCE, EXPECTED)
assert '// why' in source_of(sys.argv[1], 'b/comment'), 'the comment must survive'
accepted(sys.argv[1], 'b/comment-ok', fixed)
print('comment: the rewrite keeps a comment that follows the arrow')
PYEOF
}

# --- 62-c: §24 の除外は受理、規則そのものは残る ---
r62_exclusions() {
    python3 - "$workdir" "$hado" <<'PYEOF'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, check_code

NESTED = '''def f (o: Bool Option): Int =>
  if o | Some(true) => 1 | Some(false) => 2 | None => 3
def main = f (Some(true))
'''
TWO_COLUMNS = '''def f (b: Bool, c: Bool): Int =>
  if b, c | true, true => 1 | true, false => 2 | false, true => 3 | false, false => 4
def main = f true true
'''
TWO_ARMS = 'def f (b: Bool): Int => if b | true => 1 | false => 2\ndef main = f true\n'
ARM_AND_FALLBACK = 'def f (b: Bool): Int => if b | true => 1 | 2\ndef main = f true\n'
accepted(sys.argv[1], 'c/nested', NESTED)
accepted(sys.argv[1], 'c/columns', TWO_COLUMNS)
check_code(sys.argv[1], 'c/arms', TWO_ARMS, 'E520')
check_code(sys.argv[1], 'c/arm-fallback', ARM_AND_FALLBACK, 'E520')
print('exclusions: the §24 exclusions are accepted and the rule still rejects its forms')
PYEOF
}

# --- 62-d: 既存の書き換えはそのまま ---
r62_existing() {
    python3 - "$workdir" "$hado" <<'PYEOF'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import check_code, check_fix, fmt

# E501: そのまま返す束縛。R57 の門番と同じ形。
check_fix(sys.argv[1], 'd/e501', 'def main =\n  1 as v ->\n  v\n', 'def main =\n  1\n')
# E503 と E501 を同じファイルに置くと、一括の fmt が両方を直す。
MIXED = 'type T = | A | B\ndef f (t: T): Int => if t | A => 1 | _ => 0\ndef main =\n  f A as v ->\n  v\n'
FIXED = 'type T = | A | B\ndef f (t: T): Int => if t | A => 1 | 0\ndef main =\n  f A\n'
check_code(sys.argv[1], 'd/mixed', MIXED, 'E503')
assert fmt(sys.argv[1], 'd/mixed-fmt', MIXED, FIXED) == FIXED
print('existing: an earlier rewrite still applies, and both settle together')
PYEOF
}

check '62-a the catch-all underscore becomes the fallback' r62_fallback
check '62-b a comment after the arrow survives' r62_comment
check '62-c the exclusions of §24 are accepted' r62_exclusions
check '62-d the earlier rewrites still apply' r62_existing

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
