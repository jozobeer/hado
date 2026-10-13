#!/usr/bin/env bash
# tier: full
# R51 実用 CLI を門番の走行に組み込む（`docs/rounds/r51.md`）。
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
import os
import subprocess

HADO = None

def run_tally(directory):
    return subprocess.run(
        [HADO, 'run', 'tools/gates/main.hd', '--', str(directory)], capture_output=True)

def tally_expected(entries):
    """`entries` is a list of (name, passed, total); read_dir returns byte order."""
    total = sum(entry[2] for entry in entries)
    passed = sum(entry[1] for entry in entries)
    red = [name for name, got, want in entries if got < want]
    return f"{passed}/{total} passed; red: " + (" ".join(red) if red else "none")

def expect_tally(directory, expected, entries):
    before = sorted(os.listdir(directory))
    result = run_tally(directory)
    assert result.returncode == 0, (directory, result.returncode, result.stderr.decode())
    assert result.stderr == b'', (directory, result.stderr.decode())
    assert result.stdout == f'{expected}\n'.encode(), (directory, result.stdout, expected)
    assert sorted(os.listdir(directory)) == before, (directory, 'the tally wrote into the directory')

def expect_failure(directory, message):
    result = run_tally(directory)
    assert result.returncode == 1, (directory, result.returncode, result.stdout.decode())
    assert result.stdout == b'', (directory, result.stdout.decode())
    assert result.stderr == f'{message}\n'.encode(), (directory, result.stderr.decode())

PY

# --- 51-a: 生成したログの集計 ---
r51_tally() {
    python3 - "$workdir" "$hado" <<'PY'
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import expect_tally, tally_expected

root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
rng = random.Random(51)
cases = 0
for case in range(6):
    directory = root / f'case-{case}'
    directory.mkdir(parents=True, exist_ok=True)
    entries = []
    for index in range(rng.randint(0, 5)):
        name = f'gate{index}.log'
        total = rng.randint(0, 6)
        got = total if rng.random() < 0.6 else rng.randint(0, total)
        (directory / name).write_text(f'body\nPASSED: {got}/{total}\n')
        entries.append((name, got, total))
    expect_tally(directory, tally_expected(entries), entries)
    cases += 1
print(f'tally: {cases} generated log directories match the harness tally')
PY
}

# --- 51-b: 入口の契約の 4 形 ---
r51_contract() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import expect_failure

root = Path(sys.argv[1]) / 'b'
root.mkdir(parents=True, exist_ok=True)

def directory(name, files):
    path = root / name
    path.mkdir(parents=True, exist_ok=True)
    for file, contents in files.items():
        (path / file).write_text(contents)
    return path

expect_failure(directory('no-verdict', {'a.log': 'body\n'}), 'InvalidLine')
expect_failure(directory('non-log', {'notes.tsv': 'timings\n'}), 'InvalidLine')
expect_failure(directory('bad-int', {'a.log': 'PASSED: 1/x\n'}), 'InvalidInt')
expect_failure(root / 'absent', 'NotFound')
print('contract: a log without a verdict, a non-log, a bad count, and a missing directory fail')
PY
}

check '51-a generated log directories match the harness tally' r51_tally
check '51-b the entry contract fails loudly' r51_contract

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
