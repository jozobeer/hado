#!/usr/bin/env bash
# R64 fmt のレイアウトの正規化（docs/plan.md 60 節）。
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

# --- 64-a: 行末の空白と末尾の改行を整える ---
r64_layout() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import fmt_check, run, source_of

TAB = 'def main =\n\t1   '
FIXED = 'def main =\n\t1\n'
result = run(sys.argv[1], 'a', TAB, 'fmt', '--check')
assert result.returncode == 1, (result.returncode, result.stdout, result.stderr)
assert b'is not canonical (2 rewrites)' in result.stdout, result.stdout
assert source_of(sys.argv[1], 'a') == TAB, 'fmt --check must not write'
result = run(sys.argv[1], 'a', TAB, 'fmt')
assert result.returncode == 0 and result.stderr == b'', (result.returncode, result.stderr)
assert b'fmt: main.hd (2 rewrites)' in result.stdout, result.stdout
text = source_of(sys.argv[1], 'a')
assert text == FIXED, (repr(text), repr(FIXED))
result = run(sys.argv[1], 'a', FIXED, 'fmt')
assert result.returncode == 0 and result.stdout == b'', (result.returncode, result.stdout)
assert source_of(sys.argv[1], 'a') == FIXED, 'fmt must be idempotent'
fmt_check(sys.argv[1], 'a', FIXED, 0)
print('layout: the trailing blanks go and one final newline stays, the tab indent remains')
PYEOF3
}

# --- 64-b: 正準なファイルには触れない ---
r64_canonical() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import fmt_check, run, source_of

CANON = 'def main =\n  1\n'
result = run(sys.argv[1], 'b', CANON, 'fmt')
assert result.returncode == 0 and result.stdout == b'' and result.stderr == b'', (result.returncode, result.stdout)
assert source_of(sys.argv[1], 'b') == CANON, 'fmt must not write a canonical file'
fmt_check(sys.argv[1], 'b', CANON, 0)
print('canonical: fmt stays silent')
PYEOF3
}

# --- 64-c: 空のファイル ---
r64_empty() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import run, source_of

result = run(sys.argv[1], 'c', '', 'fmt')
assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
assert result.stdout == b'' and result.stderr == b'', (result.stdout, result.stderr)
assert source_of(sys.argv[1], 'c') == '', 'the empty file must stay empty'
print('empty: nothing to lay out')
PYEOF3
}

# --- 64-d: check --fix はレイアウトを当てない ---
r64_check_fix() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import run, source_of

TAB = 'def main =\n\t1   '
result = run(sys.argv[1], 'd', TAB, 'check', '--fix')
assert result.returncode == 0 and result.stderr == b'', (result.returncode, result.stderr)
assert source_of(sys.argv[1], 'd') == TAB, 'check --fix must not lay out'
print('check --fix: a diagnostic rewrite only, no layout')
PYEOF3
}

# --- 64-e: 仕様と層 ---
r64_docs() {
    python3 - <<'PYEOF3'
from pathlib import Path

tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r64' in full.split(), 'r64 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert 'verify-r64' in trace, 'spec-trace does not name verify-r64'
plan = Path('docs/plan.md').read_text()
assert '## 60. ' in plan, 'plan has no section 60'
spec = Path('docs/spec.md').read_text()
section = spec.split('## 58.1. ', 1)[1].split('\n## ', 1)[0]
assert '行末' in section, '§58.1 does not record the layout normalization'
print('docs: §58.1, the trace row, the tier and the plan')
PYEOF3
}

# --- 64-f: コーパスロック ---
r64_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '64-a trailing blanks and the final newline' r64_layout
check '64-b a canonical file is untouched' r64_canonical
check '64-c an empty file is untouched' r64_empty
check '64-d check --fix does not lay out' r64_check_fix
check '64-e spec, trace, tier' r64_docs
check '64-f corpus lock' r64_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
