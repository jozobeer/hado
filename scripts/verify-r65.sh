#!/usr/bin/env bash
# R65 診断の機械可読な出力（docs/plan.md 61 節）。
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

# --- 65-a: 1 行 1 診断の JSON ---
r65_json_line() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import json
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import run

SOURCE = 'def f (b: Bool): Int => if b | true => 1 | false => 2\ndef main = f true\n'
result = run(sys.argv[1], 'a', SOURCE, 'check', '--format', 'json')
assert result.returncode == 1, (result.returncode, result.stdout, result.stderr)
lines = result.stderr.decode().strip().split('\n')
assert len(lines) == 1, lines
report = json.loads(lines[0])
assert report['path'].endswith('main.hd'), report
assert (report['line'], report['col']) == (1, 25), report
assert report['code'] == 'E520', report
assert report['message'].startswith('use a Boolean branch'), report
print('json: one line, parsed, the position and the code match')
PYEOF3
}

# --- 65-b: 正準形は別のフィールド ---
r65_canonical() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import json
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import run

SOURCE = 'def f (b: Bool): Int => if b | true => 1 | false => 2\ndef main = f true\n'
result = run(sys.argv[1], 'b', SOURCE, 'check', '--format', 'json')
report = json.loads(result.stderr.decode().strip())
assert report['canonical'] == 'if b => 1 | 2', report
assert 'canonical form' not in report['message'], report
assert report['related'] is None, report
print('json: the canonical form is a field of its own')
PYEOF3
}

# --- 65-c: 関連位置 ---
r65_related() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import json
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import run

SOURCE = 'def f (n: Int): Int => n\ndef f (n: Int): Int => n\ndef main = f 1\n'
result = run(sys.argv[1], 'c', SOURCE, 'check', '--format', 'json')
assert result.returncode == 1, (result.returncode, result.stderr)
report = json.loads(result.stderr.decode().strip())
assert report['code'] == 'E205', report
assert report['related'] is not None, report
assert report['related']['line'] == 1, report
assert report['related']['label'] == 'first declared', report
print('json: the related position and its label are fields')
PYEOF3
}

# --- 65-d: 既定は今の 1 行のまま ---
r65_default() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import run

SOURCE = 'def f (b: Bool): Int => if b | true => 1 | false => 2\ndef main = f true\n'
result = run(sys.argv[1], 'd', SOURCE, 'check')
text = result.stderr.decode()
assert result.returncode == 1, (result.returncode, text)
assert text.startswith('main.hd'), text
assert 'error[E520]' in text and '; canonical form: if b => 1 | 2' in text, text
assert not text.startswith('{'), text
print('default: the human line is unchanged')
PYEOF3
}

# --- 65-e: 用法の誤りは終了コード 2 ---
r65_usage() {
    python3 - "$workdir" "$hado" <<'PYEOF3'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import run

SOURCE = 'def main = 1\n'
for name, args in (('wrong', ('--format', 'yaml')), ('missing', ('--format',))):
    result = run(sys.argv[1], 'e-' + name, SOURCE, 'check', *args)
    assert result.returncode == 2, (name, result.returncode, result.stderr)
print('usage: an unknown or missing format is exit 2')
PYEOF3
}

# --- 65-f: 仕様と層 ---
r65_docs() {
    python3 - <<'PYEOF3'
from pathlib import Path

tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r65' in full.split(), 'r65 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert 'verify-r65' in trace, 'spec-trace does not name verify-r65'
plan = Path('docs/plan.md').read_text()
assert '## 61. ' in plan, 'plan has no section 61'
spec = Path('docs/spec.md').read_text()
section = spec.split('## 58.4. ', 1)[1].split('\n## ', 1)[0]
assert 'json' in section, '§58.4 does not record the JSON format'
print('docs: §58.4, the trace row, the tier and the plan')
PYEOF3
}

# --- 65-g: コーパスロック ---
r65_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '65-a one line of JSON per diagnostic' r65_json_line
check '65-b the canonical form is a field' r65_canonical
check '65-c the related position is a field' r65_related
check '65-d the default line is unchanged' r65_default
check '65-e an unknown format is exit 2' r65_usage
check '65-f spec, trace, tier' r65_docs
check '65-g corpus lock' r65_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
