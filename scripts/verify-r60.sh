#!/usr/bin/env bash
# tier: full
# R60 E305 の役割と関連位置（`docs/rounds/r60.md`）。
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

def source_of(root, name, relative='main.hd'):
    return (Path(root) / name / relative).read_text()

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

def parts(found):
    """Split the message into the prose and the optional related position (§56)."""
    import re
    match = re.fullmatch(r'(.*?); (.+) at ([^\s:]+):(\d+):(\d+)', found['message'])
    if not match:
        return found['message'], None
    return match.group(1), {'label': match.group(2), 'path': match.group(3),
                            'line': int(match.group(4)), 'col': int(match.group(5))}

def position(text, needle, occurrence=1):
    """The 1-based (line, col) of the nth occurrence of `needle`."""
    index = -1
    for _ in range(occurrence):
        index = text.index(needle, index + 1)
    before = text[:index]
    return before.count('\n') + 1, index - (before.rfind('\n') + 1) + 1

def anchor(found):
    return found['path'], found['line'], found['col']

def expected_at(source, needle, occurrence=1):
    return {'label': 'expected here', 'path': 'main.hd', 'line': position(source, needle, occurrence)[0],
            'col': position(source, needle, occurrence)[1]}
PY

# --- 60-a: 定義の本体と宣言された戻り型 ---
r60_return_type() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, expected_at, parts, position, project

import random
rng = random.Random(61)
pad = '\n' * rng.randint(0, 3) + ' ' * rng.randint(0, 4)
source = f'{pad}def f (x: Int): Int => "a"\n{pad}def main = 0\n'
root = Path(sys.argv[1]) / 'a'

found = diagnostic(project(root, 'body', {'main.hd': source}, 'check'), 'E305')
message, related = parts(found)
assert message == 'expected Int, found String', message
assert anchor(found) == ('main.hd', *position(source, 'def f')), found
assert related == expected_at(source, 'Int =>'), related
print('return type: the body is compared with the declared type')
PY
}

# --- 60-b: 条件式の腕どうし ---
r60_arms() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, expected_at, parts, position, project

import random
rng = random.Random(62)
pad = '\n' * rng.randint(0, 3) + ' ' * rng.randint(0, 4)
source = f'{pad}def f (x: Int): Int => if true => 1 | "a"\n{pad}def main = 0\n'
root = Path(sys.argv[1]) / 'b'

found = diagnostic(project(root, 'arms', {'main.hd': source}, 'check'), 'E305')
message, related = parts(found)
assert message == 'expected Int, found String', message
assert anchor(found) == ('main.hd', *position(source, '"a"')), found
assert related == expected_at(source, '1 |'), related
print('arms: the later arm is compared with the first arm')
PY
}

# --- 60-c: 条件式の条件 ---
r60_condition() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, parts, position, project

import random
rng = random.Random(63)
pad = '\n' * rng.randint(0, 3) + ' ' * rng.randint(0, 4)
source = f'{pad}def f (x: Int): Int => if x => 1 | 2\n{pad}def main = 0\n'
root = Path(sys.argv[1]) / 'c'

found = diagnostic(project(root, 'condition', {'main.hd': source}, 'check'), 'E305')
message, related = parts(found)
assert message == 'expected Bool, found Int', message
assert anchor(found) == ('main.hd', *position(source, 'x =>')), found
assert related is None, related
print('condition: the condition is compared with Bool')
PY
}

# --- 60-d: レコード literal のフィールド ---
r60_field() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, parts, position, project

import random
rng = random.Random(64)
pad = '\n' * rng.randint(0, 3) + ' ' * rng.randint(0, 4)
source = (f'{pad}type T = {{ a: Int; b: String }}\n'
          f'{pad}def f (x: Int): T => {{ a = "x"; b = "y" }}\n'
          f'{pad}def main = 0\n')
root = Path(sys.argv[1]) / 'd'

found = diagnostic(project(root, 'field', {'main.hd': source}, 'check'), 'E305')
message, related = parts(found)
assert message == 'expected Int, found String', message
assert anchor(found) == ('main.hd', *position(source, '"x"')), found
assert related is None, related
print('field: the value is compared with the declared field type')
PY
}

# --- 60-e: 適用の引数（R56 の形） ---
r60_argument() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, parts, position, project

import random
rng = random.Random(65)
pad = '\n' * rng.randint(0, 3) + ' ' * rng.randint(0, 4)
source = f'{pad}def f (x: Int): Int => x\n{pad}def main = f "a"\n'
root = Path(sys.argv[1]) / 'e'

found = diagnostic(project(root, 'argument', {'main.hd': source}, 'check'), 'E305')
message, related = parts(found)
assert message == 'expected Int, found String', message
assert anchor(found) == ('main.hd', *position(source, '"a"')), found
assert related is None, related
print('argument: the actual is compared with the formal (R56)')
PY
}

# --- 60-f: 対称な文面の残りと、食い違いの無い対照 ---
r60_symmetric() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, anchor, diagnostic, parts, position, project

import random
rng = random.Random(66)
pad = '\n' * rng.randint(0, 3) + ' ' * rng.randint(0, 4)
root = Path(sys.argv[1]) / 'f'

logical = f'{pad}def f (x: Int): Bool => true && x\n{pad}def main = 0\n'
found = diagnostic(project(root, 'logical', {'main.hd': logical}, 'check'), 'E305')
message, related = parts(found)
assert message == 'cannot unify Bool with Int', message
assert anchor(found) == ('main.hd', *position(logical, 'true')), found
assert related is None, related

compare = f'{pad}def f (x: Int): Int => x == "a"\n{pad}def main = 0\n'
found = diagnostic(project(root, 'compare', {'main.hd': compare}, 'check'), 'E305')
message, related = parts(found)
assert message == 'cannot unify Int with String', message
assert related is None, related

control = f'{pad}def f (x: Int): Int => x + 1\n{pad}def main = f 1\n'
accepted(project(root, 'control', {'main.hd': control}, 'check'))
print('symmetric: the operators keep cannot unify and the anchor of the expression')
PY
}

# --- 60-g: 1 件報告 ---
r60_one_report() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, position, project

root = Path(sys.argv[1]) / 'g'
source = ('type T = { a: Int; b: Int }\n'
          'def f (x: Int): T => { a = "x"; b = "y" }\n'
          'def main = 0\n')
found = diagnostic(project(root, 'two', {'main.hd': source}, 'check'), 'E305')
assert anchor(found) == ('main.hd', *position(source, '"x"')), found
print('one report: two mismatching fields still print one line')
PY
}

# --- 60-h: 書き換えを持たない ---
r60_untouched() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, source_of

root = Path(sys.argv[1]) / 'h'
source = 'def f (x: Int): Int => "a"\ndef main = 0\n'

result = project(root, 'fix', {'main.hd': source}, 'check', '--fix')
assert result.returncode == 1, (result.returncode, result.stderr.decode())
assert source_of(root, 'fix') == source, 'check --fix must not write'

result = project(root, 'fmt', {'main.hd': source}, 'fmt')
assert result.returncode == 0 and result.stderr == b'', (result.returncode, result.stderr.decode())
assert source_of(root, 'fmt') == source, 'fmt must not write'
print('untouched: E305 has no rewrite')
PY
}

check '60-a the declared return type' r60_return_type
check '60-b the arms of a conditional' r60_arms
check '60-c the condition of a conditional' r60_condition
check '60-d the field of a record literal' r60_field
check '60-e the argument of a call' r60_argument
check '60-f the symmetric sites and the control' r60_symmetric
check '60-g one report per run' r60_one_report
check '60-h E305 has no rewrite' r60_untouched

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
