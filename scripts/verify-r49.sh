#!/usr/bin/env bash
# tier: full
# R49 効果行付き関数値の build 失敗を直す（`docs/rounds/r49.md`）。
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
import re
import subprocess
from pathlib import Path

HADO = None

def write(root, name, source):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    return path

def hado(args):
    return subprocess.run([HADO, *args], capture_output=True)

def check_ok(path):
    result = hado(['check', str(path)])
    assert result.returncode == 0 and result.stderr == b'', (path, result.returncode, result.stderr.decode())

def check_code(path, code):
    result = hado(['check', str(path)])
    assert result.returncode == 1, (path, result.returncode, result.stderr.decode())
    match = re.match(rb'[^:]+:(\d+):(\d+): error\[(E\d+)\]', result.stderr)
    assert match and match.group(3).decode() == code, (path, code, result.stderr.decode())

def build_run(root, name, source, expected):
    """Check, build to a private path, run, and require `expected` plus a newline."""
    path = write(root, name, source)
    check_ok(path)
    binary = path.parent / 'program'
    built = hado(['build', '-o', str(binary), str(path)])
    assert built.returncode == 0, (name, built.returncode, built.stderr.decode())
    result = subprocess.run([str(binary)], capture_output=True)
    assert result.returncode == 0, (name, result.returncode, result.stderr.decode())
    assert result.stderr == b'', (name, result.stderr.decode())
    assert result.stdout == f'{expected}\n'.encode(), (name, result.stdout, expected)

def control(root):
    """The comparisons must reject a wrong code and a wrong output."""
    path = write(root, 'control-ok', 'def main = 1\n')
    try:
        check_code(path, 'E999')
    except AssertionError:
        pass
    else:
        raise AssertionError('check_code accepted a wrong code')
    try:
        build_run(root, 'control-run', 'def main = 1\n', '2')
    except AssertionError:
        pass
    else:
        raise AssertionError('build_run accepted a wrong output')
PY

# --- 49-a: 効果行付きの関数値を返す def を多相関数へ渡す ---
r49_effectful_value() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, control

root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
seed = int(os.environ.get('HADO_SEED', '49'))
rng = random.Random(seed)
control(str(root / 'control'))

def word():
    return ''.join(rng.choice('abcxyz') for _ in range(rng.randint(1, 4)))

for index in range(6):
    prefix, infix = word(), word()
    digits = str(rng.randint(0, 999))
    fold = rng.choice(['direct', 'let'])
    make = (
        'def make(s: String): Fn(String => String {Error}) Pure =>\n'
        f'  fn(t => String.concat s (String.concat "{infix}" (Int.to_string (Int.parse t))))\n')
    if fold == 'direct':
        source = (
            'def apply f x => f x\n'
            + make
            + f'def main = apply (make "{prefix}") "{digits}"\n')
    else:
        source = (
            make
            + 'def main =\n'
            + f'  let f = make "{prefix}" ->\n'
            + f'  f "{digits}"\n')
    build_run(str(root), f'effect-{index}', source, f'{prefix}{infix}{digits}')
print(f'effectful value: seed {seed}, 6 definitions passed through a polymorphic function')
PY
}

# --- 49-c: 効果行の無い関数値と、効果行付き関数値をフィールドに持つ record ---
r49_unchanged_forms() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run

root = Path(sys.argv[1]) / 'c'
root.mkdir(parents=True, exist_ok=True)
cases = {
    'pure-value': (
        'def apply f x => f x\n'
        'def make(s: String): Fn(String => String) Pure => fn(t => String.concat s t)\n'
        'def main = apply (make "a") "b"\n',
        'ab'),
    'lambda-value': (
        'def apply f x => f x\n'
        'def main = apply (fn(t => String.concat "a" t)) "b"\n',
        'ab'),
    'record-field': (
        'type Box = { run: Fn(String => String {Error}) }\n'
        'def make(s: String): Fn(String => String {Error}) Pure => fn(t => String.concat s t)\n'
        'def main =\n'
        '  let box = { run = make "a" } ->\n'
        '  box.run "b"\n',
        'ab'),
}
for name, (source, expected) in cases.items():
    build_run(str(root), name, source, expected)
print(f'unchanged forms: {len(cases)} function values still build and run')
PY
}

check '49-a effectful function values build and run' r49_effectful_value
check '49-c pure and record function values are unchanged' r49_unchanged_forms

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
