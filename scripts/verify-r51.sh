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


def control(root):
    """The comparison must reject a wrong tally."""
    from pathlib import Path
    directory = Path(root) / 'control'
    directory.mkdir(parents=True, exist_ok=True)
    (directory / 'a.log').write_text('PASSED: 1/2\n')
    entries = [('a.log', 1, 2)]
    try:
        expect_tally(directory, '2/2 passed; red: none', entries)
    except AssertionError:
        pass
    else:
        raise AssertionError('expect_tally accepted a wrong tally')
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
from harness import control, expect_tally, tally_expected

root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
control(str(root / 'control'))
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

# --- 51-c: 走行への組み込み（合成木） ---
r51_wiring() {
    local tree="$workdir/tree"
    mkdir -p "$tree/scripts/lib" "$tree/tools/gates/report"
    cp scripts/run-gates.sh "$tree/scripts/run-gates.sh"
    cp tools/gates/main.hd "$tree/tools/gates/main.hd"
    cp tools/gates/report/report.hd "$tree/tools/gates/report/report.hd"
    cat >"$tree/scripts/lib/gate-tiers.sh" <<'SH'
hado_tier_gates() {
    printf '%s\n' ${STUB_GATES:-stub-a}
}
SH
    cat >"$tree/scripts/verify-stub-a.sh" <<'SH'
echo "OK: a"
echo "PASSED: 1/1"
SH
    cat >"$tree/scripts/verify-stub-b.sh" <<'SH'
echo "NG: b"
echo "PASSED: 0/1"
exit 1
SH
    cat >"$tree/scripts/verify-stub-c.sh" <<'SH'
echo "no verdict here"
exit 1
SH
    python3 - "$tree" "$target_dir" <<'PY'
import os
import subprocess
import sys
from pathlib import Path

tree = Path(sys.argv[1])
target = sys.argv[2]
env = {**os.environ, 'CARGO_TARGET_DIR': target, 'STUB_GATES': 'stub-a stub-b'}
result = subprocess.run(
    ['bash', str(tree / 'scripts/run-gates.sh'), '--jobs', '1', '--tier', 'full', str(tree / 'red')],
    capture_output=True, env=env)
stdout = result.stdout.decode()
assert result.returncode == 1, (result.returncode, stdout, result.stderr.decode())
assert 'TALLY: 1/2 passed; red: verify-stub-b.log\n' in stdout, (stdout, result.stderr.decode())
assert sorted(p.name for p in (tree / 'red').glob('*.log')) == ['verify-stub-a.log', 'verify-stub-b.log']

env['STUB_GATES'] = 'stub-c'
result = subprocess.run(
    ['bash', str(tree / 'scripts/run-gates.sh'), '--jobs', '1', '--tier', 'full', str(tree / 'broken')],
    capture_output=True, env=env)
assert result.returncode == 1, (result.returncode, result.stdout.decode(), result.stderr.decode())
assert 'TALLY: failed' in result.stderr.decode(), result.stderr.decode()
assert 'InvalidLine' in result.stderr.decode(), result.stderr.decode()
print('wiring: the run prints the Hado tally and fails when a log has no verdict')
PY
}

r51_docs() {
    python3 - <<'PY'
from pathlib import Path

import subprocess
full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], capture_output=True, text=True, check=True).stdout.split()
assert 'r51' in full, 'r51 is not in the full tier'
run_gates = Path('scripts/run-gates.sh').read_text()
assert 'run tools/gates/main.hd --' in run_gates, 'run-gates does not call the tally'
wants = Path('docs/wants.md').read_text()
assert 'R51' in wants, 'wants.md does not record R51'
print('docs: r51 in the full tier, the tally is wired, wants.md updated')
PY
}

r51_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '51-a generated log directories match the harness tally' r51_tally
check '51-b the entry contract fails loudly' r51_contract
check '51-c the run prints the Hado tally' r51_wiring
check '51-d tier, wiring, wants' r51_docs
check '51-d corpus lock' r51_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
