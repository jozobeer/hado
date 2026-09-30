#!/usr/bin/env bash
# tier: full
# R57 E507 の書き換え（`docs/rounds/r57.md`）。
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
import random
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

def build_fix(root, name, source, expected):
    result = run(root, name, source, 'build', '--fix', '-o', str(Path(root) / name / 'program'))
    assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr.decode())
    text = source_of(root, name)
    assert text == expected, (name, repr(text), repr(expected))
    return text

def no_rewrite(root, name, source, code):
    """The diagnostic fires, no rewrite is offered, and neither tool writes."""
    check_code(root, name, source, code, hint=None)
    result = run(root, name, source, 'check', '--fix')
    assert result.returncode == 1, (name, result.returncode)
    assert source_of(root, name) == source, (name, 'check --fix must not write')
    fmt_check(root, name, source, 0)

def build_run(root, name, expect):
    directory = Path(root) / name
    build = subprocess.run([HADO, 'build', str(directory / 'main.hd')], capture_output=True)
    assert build.returncode == 0 and build.stderr == b'', (name, build.returncode, build.stderr.decode())
    result = subprocess.run([str(directory / 'main')], capture_output=True, cwd=directory, stdin=subprocess.DEVNULL)
    assert result.returncode == 0, (name, result.returncode, result.stderr.decode())
    assert result.stdout.decode() == expect + '\n', (name, result.stdout.decode(), expect)

def value_of(rng):
    """A generated option argument and the text `main` prints for it."""
    n = rng.randint(1, 9)
    return (f'Some({n})', f'Some({n})') if rng.random() < 0.5 else ('None', 'None')
PY

# --- 57-a: E507 の `if` の形は scrutinee になる ---
r57_if() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix, fmt, fmt_check, source_of, value_of

import random
rng = random.Random(57)
value, printed = value_of(rng)

# 単純な scrutinee: 正準形は名前そのもの。
body = 'if x\n  | Some(v) => Some(v)\n  | None => None'
source = f'def f (x: Int Option): Int Option Pure =>\n  {body}\ndef main = f {value}\n'
expected = f'def f (x: Int Option): Int Option Pure =>\n  x\ndef main = f {value}\n'
root = Path(sys.argv[1]) / 'a'
check_code(root, 'plain', source, 'E507', 'x')
check_fix(root, 'plain', source, expected)
build_run(root, 'plain', printed)
fmt(root, 'plain2', source, expected)
fmt_check(root, 'plain', source, 1)

# 複合の scrutinee: 正準形は 1 行の括弧付きの形。
head = 'def id_opt (x: Int Option): Int Option Pure => x\n'
body = 'if (x |> id_opt)\n  | Some(v) => Some(v)\n  | None => None'
source = head + f'def f (x: Int Option): Int Option Pure =>\n  {body}\ndef main = f {value}\n'
expected = head + f'def f (x: Int Option): Int Option Pure =>\n  (x |> id_opt)\ndef main = f {value}\n'
check_code(root, 'pipe', source, 'E507', '(x |> id_opt)')
check_fix(root, 'pipe', source, expected)
build_run(root, 'pipe', printed)
print('if: the branch becomes the scrutinee, spelled canonically anywhere')
PY
}

# --- 57-b: E507 の第31節の形は def f value => value / fn(value => value) ---
r57_arms() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix, fmt, value_of

import random
rng = random.Random(58)
value, printed = value_of(rng)

source = f'def f\n| Some(v) => Some(v)\n| None => None\ndef main = f {value}\n'
expected = f'def f value => value\ndef main = f {value}\n'
root = Path(sys.argv[1]) / 'b'
check_code(root, 'def', source, 'E507', 'def f value => value')
check_fix(root, 'def', source, expected)
build_run(root, 'def', printed)
fmt(root, 'def2', source, expected)

source = f'def main = (fn(| Some(v) => Some(v) | None => None)) {value}\n'
expected = f'def main = (fn(value => value)) {value}\n'
check_code(root, 'fn', source, 'E507', 'fn(value => value)')
check_fix(root, 'fn', source, expected)
build_run(root, 'fn', printed)
print('arms: the def and fn arm forms become identity functions')
PY
}

# --- 57-c: 旧 E508 の `if` の形は、いまは正準形で受理される（`?` の廃止） ---
r57_removed_repropagate_if() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import check_fix

root = Path(sys.argv[1]) / 'c'
head = 'def find_user (id: Int): Int Option Pure => if id > 0 => Some(id) | None\n'
source = head + 'def g (id: Int): Int Option Pure =>\n  if find_user id\n  | Some(user) => Some(user + 1)\n  | None => None\ndef main = g 1\n'
check_fix(root, 'accepted', source, source)
print('removed repropagate: the tail branch shape is accepted unchanged')
PY
}

# --- 57-d: 旧 E508 の第31節の形も、いまは正準形で受理される ---
r57_removed_repropagate_arms() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import check_fix

root = Path(sys.argv[1]) / 'd'
source = 'def g\n| Some(user) => Some(user + 1)\n| None => None\ndef main = g (Some(1))\n'
check_fix(root, 'accepted', source, source)
print('removed repropagate arms: the arm form is accepted unchanged')
PY
}

# --- 57-e: 書き換えを出さない形 ---
r57_declines() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import no_rewrite

root = Path(sys.argv[1]) / 'e'
head = 'def find_user (id: Int): Int Option Pure => if id > 0 => Some(id) | None\n'


# fallback を持つ E507: scrutinee を 2 回評価し得る。
no_rewrite(root, 'fallback', 'def f (x: Int Option): Int Option Pure =>\n  if x\n  | Some(v) => Some(v)\n  | x\ndef main = f (Some(1))\n', 'E507')



# 範囲にコメントを含む E507: コメントを消さない。
no_rewrite(root, 'comment', 'def f (x: Int Option): Int Option Pure =>\n  if x\n  // keep\n  | Some(v) => Some(v)\n  | None => None\ndef main = f (Some(1))\n', 'E507')
print('declines: a fallback E507 and a comment range are left alone')
PY
}

# --- 57-f: fmt と check --fix と build --fix は同じ書き換えを共有する ---
r57_shared() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_fix, check_fix, fmt, source_of

root = Path(sys.argv[1]) / 'f'
shapes = [
    ('if', 'def f (x: Int Option): Int Option Pure =>\n  if x\n  | Some(v) => Some(v)\n  | None => None\ndef main = f (Some(2))\n',
     'def f (x: Int Option): Int Option Pure =>\n  x\ndef main = f (Some(2))\n'),
    ('def', 'def f\n| Some(v) => Some(v)\n| None => None\ndef main = f (Some(2))\n',
     'def f value => value\ndef main = f (Some(2))\n'),
]
for index, (name, source, expected) in enumerate(shapes):
    by_fix = check_fix(root, f'{name}-fix', source, expected)
    by_fmt = fmt(root, f'{name}-fmt', source, expected)
    by_build = build_fix(root, f'{name}-build', source, expected)
    assert by_fix == by_fmt == by_build == source_of(root, f'{name}-fmt'), (name, by_fix, by_fmt, by_build)
    assert index >= 0
print(f'shared: {len(shapes)} shapes have the same text from check --fix, fmt and build --fix')
PY
}

check '57-a the if form becomes its scrutinee' r57_if
check '57-b the arm forms become identity functions' r57_arms
check '57-c the removed repropagate shape is accepted unchanged' r57_removed_repropagate_if
check '57-d the removed repropagate arm form is accepted unchanged' r57_removed_repropagate_arms
check '57-e forms without a local rewrite are left alone' r57_declines
check '57-f check --fix, fmt and build --fix share one rewrite' r57_shared

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
