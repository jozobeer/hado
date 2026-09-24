#!/usr/bin/env bash
# tier: full
# R47 正準形の修正を E515 と E501 へ広げる（`docs/rounds/r47.md`）。
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
hado="${CARGO_TARGET_DIR:-target}/debug/hado"

cat >"$workdir/harness.py" <<'PY'
import hashlib
import re
import subprocess
from pathlib import Path

HADO = None

def run(root, name, source, *args):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    (directory / 'main.hd').write_text(source)
    return subprocess.run([HADO, *args, str(directory / 'main.hd')], capture_output=True)

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def check_ok(root, name, source):
    result = run(root, name, source, 'check')
    assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr.decode())

def check_code(root, name, source, code):
    result = run(root, name, source, 'check')
    assert result.returncode == 1, (name, result.returncode, result.stderr.decode())
    match = re.match(rb'[^:]+:(\d+):(\d+): error\[(E\d+)\]', result.stderr)
    assert match and match.group(3).decode() == code, (name, code, result.stderr.decode())
    return result

def check_fix(root, name, source):
    return run(root, name, source, 'check', '--fix')

def build_run(root, name, expected):
    directory = Path(root) / name
    build = subprocess.run([HADO, 'build', str(directory / 'main.hd')], capture_output=True)
    assert build.returncode == 0 and build.stderr == b'', (name, build.returncode, build.stderr.decode())
    result = subprocess.run([str(directory / 'main')], capture_output=True, cwd=directory, stdin=subprocess.DEVNULL)
    assert result.returncode == 0, (name, result.returncode, result.stderr.decode())
    assert result.stderr == b'', (name, result.stderr.decode())
    assert result.stdout == expected.encode(), (name, result.stdout, expected)

def comments(source):
    return [line for line in source.splitlines() if line.strip().startswith('//')]

def control(root):
    """The comparisons must reject a wrong code and a wrong output."""
    try:
        check_code(root, 'control-code', 'def main = x\n', 'E999')
    except AssertionError:
        pass
    else:
        raise AssertionError('check_code accepted a wrong code')
    try:
        check_ok(root, 'control-ok', 'def main = x\n')
    except AssertionError:
        pass
    else:
        raise AssertionError('check_ok accepted a rejected program')
    try:
        build_run(root, 'control-run', 'def main = 1\n', '2\n')
    except AssertionError:
        pass
    else:
        raise AssertionError('build_run accepted a wrong output')
PY

# --- 47-a: E515 の書き換え。診断 → --fix → check → build の出力一致 ---
r47_sole_parameter() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix, comments

root = sys.argv[1] + '/a'
seed = int(os.environ.get('HADO_SEED', '47'))
rng = random.Random(seed)
Path(root).mkdir(parents=True, exist_ok=True)

for index in range(10):
    added = rng.randint(-9, 9)
    fallback = rng.randint(-9, 9)
    value = rng.randint(-9, 9)
    rec = rng.choice(['', 'rec '])
    comment = rng.choice(['', '// 前書き\n', '// 日本語のコメント\n'])
    source = comment + (
        f'def {rec}f x =>\n'
        f'  if x\n'
        f'  | Some(v) => v + ({added})\n'
        f'  | {fallback}\n'
        f'def main = f (Some({value}))\n')
    name = f'sole-{index}'
    caught = check_code(root, name, source, 'E515')
    assert b'canonical form:' in caught.stderr, (name, caught.stderr.decode())
    fixed = check_fix(root, name, source)
    assert fixed.returncode == 0 and fixed.stderr == b'', (name, fixed.returncode, fixed.stderr.decode())
    text = (Path(root) / name / 'main.hd').read_text()
    assert 'if x' not in text, (name, text)
    assert '| Some(v) =>' in text, (name, text)
    for line in comments(source):
        assert line in text, (name, line, text)
    build_run(root, name, f'{value + added}\n')
    before = (Path(root) / name / 'main.hd').read_bytes()
    again = check_fix(root, name, text)
    assert again.returncode == 0 and again.stderr == b'', (name, again.stderr.decode())
    assert (Path(root) / name / 'main.hd').read_bytes() == before, name
print(f'sole parameter: seed {seed}, 10 rewritten functions run')
PY
}

# --- 47-b: E501 の書き換え（リテラルと関数適用）---
r47_identity_binding() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix

root = sys.argv[1] + '/b'
seed = int(os.environ.get('HADO_SEED', '47'))
rng = random.Random(seed)
Path(root).mkdir(parents=True, exist_ok=True)

for index in range(10):
    added = rng.randint(-9, 9)
    value = rng.randint(-9, 9)
    name = f'identity-{index}'
    if index % 2 == 0:
        source = f'def g n => n + ({added})\ndef main =\n  let x = g ({value}) -> x\n'
        expected = value + added
        expected_text = f'g ({value})'
    else:
        source = f'def main =\n  let x = {value} -> x\n'
        expected = value
        expected_text = str(value)
    caught = check_code(root, name, source, 'E501')
    assert b'canonical form:' in caught.stderr, (name, caught.stderr.decode())
    fixed = check_fix(root, name, source)
    assert fixed.returncode == 0 and fixed.stderr == b'', (name, fixed.returncode, fixed.stderr.decode())
    text = (Path(root) / name / 'main.hd').read_text()
    assert 'let x' not in text, (name, text)
    assert expected_text in text, (name, expected_text, text)
    build_run(root, name, f'{expected}\n')
print(f'identity binding: seed {seed}, 10 rewritten bindings run')
PY
}

# --- 47-c: E501 の入れ子。値の `->` を分離点と取り違えない ---
r47_nested_binding() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix

root = sys.argv[1] + '/c'
seed = int(os.environ.get('HADO_SEED', '47'))
rng = random.Random(seed)
Path(root).mkdir(parents=True, exist_ok=True)

for index in range(8):
    value = rng.randint(-9, 9)
    source = f'def main =\n  let x = let y = {value} -> y -> x\n'
    name = f'nested-{index}'
    check_code(root, name, source, 'E501')
    fixed = check_fix(root, name, source)
    assert fixed.returncode == 0 and fixed.stderr == b'', (name, fixed.returncode, fixed.stderr.decode())
    text = (Path(root) / name / 'main.hd').read_text()
    assert 'let' not in text, (name, text)
    build_run(root, name, f'{value}\n')
print(f'nested binding: seed {seed}, 8 nested lets reduced from the outside in')
PY
}

# --- 47-d: コメントを含む範囲ではファイルが変わらない ---
r47_comment_ranges() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import check_fix

root = Path(sys.argv[1]) / 'd'
root.mkdir(parents=True, exist_ok=True)
cases = {
    'branch': 'def f x\n  // keep\n  => if x\n  | Some(v) => v\n  | 0\ndef main = f (Some(7))\n',
    'binding': 'def main =\n  let x = 5 // keep\n    -> x\n',
}
for name, source in cases.items():
    path = root / name / 'main.hd'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(source)
    result = check_fix(str(root), name, source)
    assert result.returncode == 1, (name, result.returncode)
    assert b'error[E5' in result.stderr, (name, result.stderr.decode())
    assert path.read_text() == source, (name, path.read_text())
print(f'comment ranges: {len(cases)} files with a comment inside the range unchanged')
PY
}

check '47-a E515: diagnosis, rewrite, comments kept, execution' r47_sole_parameter
check '47-b E501: identity binding becomes its value and runs' r47_identity_binding
check '47-c E501: nested let reduced from the outside in' r47_nested_binding
check '47-d E515 and E501: a comment in the range leaves the file unchanged' r47_comment_ranges

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
