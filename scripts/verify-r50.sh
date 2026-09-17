#!/usr/bin/env bash
# tier: full
# R50 スタックの枯渇を診断にする（`docs/rounds/r50.md`）。
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
import subprocess
from pathlib import Path

HADO = None
OVERFLOW = b'hado: stack overflow\n'


def again_source(n):
    return (
        'effect A Yield\n'
        'again yield(value: A): Void\n'
        'handler A Yield as Collect\n'
        'again yield value =>\n'
        '  let rest = resume () ->\n'
        '  [value, ...rest]\n'
        'return _ => []\n'
        'def rec loop (n: Int): Void {Int Yield} =>\n'
        '  if n <= 0 => ()\n'
        '  | do yield n -> loop (n - 1)\n'
        f'def main = List.length (with Collect -> loop {n})\n')


def build_program(root, name, n):
    """Generate an `again` loop, build it to a private path, and return the binary."""
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    source = directory / 'main.hd'
    source.write_text(again_source(n))
    binary = directory / 'program'
    built = subprocess.run([HADO, 'build', '-o', str(binary), str(source)], capture_output=True)
    assert built.returncode == 0, (name, built.returncode, built.stderr.decode())
    return binary


def run_with_stack(binary, stack_kb, timeout=300):
    """Run with RLIMIT_STACK set, so the depth is the harness's choice, not the shell's."""
    return subprocess.run(
        ['bash', '-c', f'ulimit -s {stack_kb}; exec "$1"', '_', str(binary)],
        capture_output=True, timeout=timeout)


def expect_run(binary, stack_kb, code, stdout, stderr):
    result = run_with_stack(binary, stack_kb)
    assert result.returncode == code, (binary, result.returncode, result.stderr.decode())
    assert result.stdout == stdout, (binary, result.stdout, stdout)
    assert result.stderr == stderr, (binary, result.stderr, stderr)


def control(root):
    """The comparisons must reject a wrong exit code, a wrong stdout, and a wrong stderr."""
    binary = build_program(root, 'control', 1000)
    for code, stdout, stderr in [(0, b'1000\n', OVERFLOW), (1, b'1000\n', b''), (0, b'999\n', b'')]:
        try:
            expect_run(binary, 65536, code, stdout, stderr)
        except AssertionError:
            pass
        else:
            raise AssertionError(f'control accepted a wrong expectation: {code} {stdout} {stderr}')
PY

# --- 50-a: 深い again ループを 1 MB のスタックで走らせる ---
r50_overflow() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import OVERFLOW, build_program, control, expect_run

root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
control(str(root / 'control'))
binary = build_program(str(root), 'deep', 300000)
expect_run(binary, 1024, 1, b'', OVERFLOW)
print('overflow: 300000 iterations on a 1 MB stack exit 1 with the stack overflow line')
PY
}

# --- 50-b: 同じ実行ファイルを 64 MB で走らせると完走する ---
r50_completes() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_program, expect_run

root = Path(sys.argv[1]) / 'b'
root.mkdir(parents=True, exist_ok=True)
binary = build_program(str(root), 'deep', 300000)
expect_run(binary, 65536, 0, b'300000\n', b'')
print('completes: 300000 iterations on a 64 MB stack print 300000 and exit 0')
PY
}

# --- 50-c: 浅いループは 1 MB でも完走する ---
r50_shallow() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_program, expect_run

root = Path(sys.argv[1]) / 'c'
root.mkdir(parents=True, exist_ok=True)
binary = build_program(str(root), 'shallow', 1000)
expect_run(binary, 1024, 0, b'1000\n', b'')
print('shallow: 1000 iterations on a 1 MB stack print 1000 and exit 0')
PY
}

# --- 50-e: 枯渇でないフォールトを握り潰さない ---
# The guard's statics are not reachable from a guest program, so compile the
# runtime into a probe and drive it with a fault whose address is not the
# stack floor (a raise has no address). It must stay a real signal death.
r50_fault_not_masked() {
    cat >"$workdir/probe.c" <<'C'
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

int32_t hado_entry(void) { return 0; }
#define main hado_runtime_main
#include "hado_runtime.c"
#undef main

int main(int argc, char **argv) {
  (void)argc;
  (void)argv;
  install_stack_guard();
  raise(SIGSEGV);
  return 0;
}
C
    cc -I runtime -o "$workdir/probe" "$workdir/probe.c" -lgc || return 1
    local output status
    output=$("$workdir/probe" 2>&1) 2>/dev/null
    status=$?
    if [ "$status" -ne 139 ]; then
        echo "probe exited $status, not 139"
        return 1
    fi
    if [ -n "$output" ]; then
        echo "probe wrote to stderr: $output"
        return 1
    fi
    echo "not masked: a fault away from the stack floor stays a signal death"
}

# --- 50-d: 仕様と追跡表と層別と corpus lock ---
r50_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert '## 53.1. スタックの枯渇' in spec, 'spec has no §53.1'
assert 'hado: stack overflow' in spec, 'spec does not name the message'
trace = Path('docs/spec-trace.md').read_text()
assert '| §53.1 |' in trace, 'spec-trace has no §53.1 row'
import subprocess
full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], capture_output=True, text=True, check=True).stdout.split()
assert 'r50' in full, 'r50 is not in the full tier'
print('docs: §53.1, the trace row, and r50 in the full tier')
PY
}

r50_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '50-a a deep again loop reports a stack overflow' r50_overflow
check '50-b a deep again loop completes with enough stack' r50_completes
check '50-c a shallow again loop completes on a small stack' r50_shallow
check '50-e a fault away from the stack floor is not masked' r50_fault_not_masked
check '50-d spec, trace, tier' r50_docs
check '50-d corpus lock' r50_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
