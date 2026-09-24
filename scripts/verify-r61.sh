#!/usr/bin/env bash
# tier: full
# R61 再帰定義の省略した効果行（`docs/rounds/r61.md`）。
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
import subprocess
from pathlib import Path

HADO = None

def write(root, name, files):
    directory = Path(root) / name
    for relative, text in files.items():
        path = directory / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    return directory

def project(root, name, files, *args, entry='main.hd'):
    """Write `files` under `<root>/<name>` and run the CLI on the entry."""
    directory = write(root, name, files)
    return subprocess.run([HADO, *args, str(directory / entry)], capture_output=True)

def accepted(result):
    assert result.returncode == 0 and result.stderr == b'', (result.returncode, result.stderr.decode())

def diagnostic(result, code, status=1):
    """The run's single diagnostic line, split into its parts."""
    import re
    assert result.returncode == status, (result.returncode, result.stderr.decode())
    lines = [line for line in result.stderr.decode().splitlines() if line]
    assert len(lines) == 1, ('one diagnostic per run', lines)
    match = re.fullmatch(r'([^\s:]+):(\d+):(\d+): error\[(E\d+)\]: (.*)', lines[0])
    assert match, lines[0]
    assert match.group(4) == code, (code, lines[0])
    return {'path': match.group(1), 'line': int(match.group(2)), 'col': int(match.group(3)),
            'message': match.group(5)}

def run_program(root, name, files, *args):
    """Build the entry and run it, returning the process result."""
    directory = write(root, name, files)
    entry = directory / 'main.hd'
    build = subprocess.run([HADO, 'build', str(entry)], capture_output=True)
    assert build.returncode == 0, build.stderr.decode()
    executable = entry.with_suffix('')
    assert executable.exists(), executable
    return subprocess.run([str(executable), *args], capture_output=True)

def printed(root, name, files, definition):
    """The type that `check --print-type=<definition>` prints for the entry."""
    import re
    directory = write(root, name, files)
    result = subprocess.run([HADO, 'check', f'--print-type={definition}', str(directory / 'main.hd')],
                            capture_output=True)
    assert result.returncode == 0, result.stderr.decode()
    text = (result.stdout + result.stderr).decode()
    match = re.search(r'^Fn\(.*\)$', text.strip(), re.M)
    assert match, text
    return match.group(0)
PY

# --- 61-a: bug.md の形。効果行を省いた再帰生成器 ---
r61_reproduction() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, printed, project, run_program

GENERATOR = '''def rec generate (n: Int): Int List =>
  if n == 0 => []
  | [n % 7, ...generate (n - 1)]
def main = List.length (generate 5)
'''
accepted(project(sys.argv[1], 'a/check', {'main.hd': GENERATOR}, 'check'))
result = run_program(sys.argv[1], 'a/build', {'main.hd': GENERATOR})
assert result.returncode == 0, result.stderr.decode()
assert result.stdout.strip() == b'5', result.stdout
# 推論された行は §20 の暗黙の効果変数なので、開いた行として出る。精度はこのラウンドの射程外である。
type_text = printed(sys.argv[1], 'a/type', {'main.hd': GENERATOR}, 'generate')
assert type_text.startswith('Fn(Int => Int List {') and 'Error' in type_text, type_text
print('reproduction: the omitted row is inferred and the program runs')
PY
}

# --- 61-b: 再帰が要る。相互再帰でも同じ ---
r61_isolation() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, project, run_program

PLAIN = 'def f (n: Int): Int List => [n % 7]\ndef main = List.length (f 5)\n'
MUTUAL = '''def rec g (n: Int): Int List =>
  if n == 0 => []
  | [n % 7, ...h (n - 1)]
def rec h (n: Int): Int List =>
  if n == 0 => []
  | g (n - 1)
def main = List.length (g 5)
'''
accepted(project(sys.argv[1], 'b/plain', {'main.hd': PLAIN}, 'check'))
accepted(project(sys.argv[1], 'b/mutual', {'main.hd': MUTUAL}, 'check'))
result = run_program(sys.argv[1], 'b/mutual-run', {'main.hd': MUTUAL})
assert result.returncode == 0, result.stderr.decode()
print('isolation: recursion is what the omission needs, and a mutual group is accepted')
PY
}

# --- 61-c: 書いた行はそのまま効く ---
r61_written_rows() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, diagnostic, project

MISMATCH = '''def rec f (n: Int): Int {Io} =>
  if n == 0 => 0
  | n % 7 + f (n - 1)
def main = f 5
'''
EXACT = '''def rec f (n: Int): Int {Error} =>
  if n == 0 => 0
  | n % 7 + f (n - 1)
def main = f 5
'''
found = diagnostic(project(sys.argv[1], 'c/mismatch', {'main.hd': MISMATCH}, 'check'), 'E305')
assert found['message'] == 'effect row mismatch', found
accepted(project(sys.argv[1], 'c/exact', {'main.hd': EXACT}, 'check'))
print('written rows: a written row still constrains the body')
PY
}

# --- 61-d: 省略した行の効果は消えない ---
r61_effects_survive() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, diagnostic, project, run_program

HEAD = 'effect Ask\nonce ask: Int\n'
BODY = '''def rec f (n: Int): Int =>
  if n == 0 => 0
  | ask + f (n - 1)
'''
unhandled = HEAD + BODY + 'def main = f 5\n'
handler = HEAD + 'handler Ask as H\nonce ask => 3\nreturn x => x\n'
handled = HEAD + 'handler Ask as H\nonce ask => 3\nreturn x => x\n' + BODY + 'def main = with H -> f 5\n'
found = diagnostic(project(sys.argv[1], 'd/plain', {'main.hd': unhandled}, 'check'), 'E706')
assert found['message'] == 'main cannot have a user-defined effect', found
accepted(project(sys.argv[1], 'd/handled', {'main.hd': handled}, 'check'))
result = run_program(sys.argv[1], 'd/handled-run', {'main.hd': handled})
assert result.returncode == 0, result.stderr.decode()
assert result.stdout.strip() == b'15', result.stdout
print('effects: an omitted row still reaches the caller and can be handled')
PY
}

# --- 61-e: E307 は残る ---
r61_e307() {
    python3 - "$workdir" "$hado" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import diagnostic, project

POLY = 'def rec poly x => poly [x]\ndef main = poly 1\n'
found = diagnostic(project(sys.argv[1], 'e/poly', {'main.hd': POLY}, 'check'), 'E307')
assert found['message'] == 'recursive calls change the argument type', found
print('E307: a polymorphic recursive argument is still rejected')
PY
}

check '61-a the reproduction from bug.md' r61_reproduction
check '61-b recursion, and a mutual group' r61_isolation
check '61-c a written row still constrains' r61_written_rows
check '61-d an omitted row still reaches the caller' r61_effects_survive
check '61-e E307 stays' r61_e307

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
