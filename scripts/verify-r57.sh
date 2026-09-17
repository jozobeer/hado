#!/usr/bin/env bash
# tier: full
# R57 E507 / E508 の書き換え（docs/plan.md 53 節）。
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

# --- 57-c: E508 の本体ちょうどの `if` は let p? = になる ---
r57_repropagate_if() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix, fmt, fmt_check

import random
rng = random.Random(59)
id_value = rng.randint(1, 9)
printed = f'Some({id_value + 1})'

head = 'def find_user (id: Int): Int Option Pure => if id > 0 => Some(id) | None\n'
body = 'if find_user id\n  | Some(user) => Some(user + 1)\n  | None => None'
source = head + f'def g (id: Int): Int Option Pure =>\n  {body}\ndef main = g {id_value}\n'
expected = head + f'def g (id: Int): Int Option Pure =>\n  let user? = find_user id -> Some(user + 1)\ndef main = g {id_value}\n'
root = Path(sys.argv[1]) / 'c'
check_code(root, 'some', source, 'E508', 'let user? = find_user id -> Some(user + 1)')
check_fix(root, 'some', source, expected)
build_run(root, 'some', printed)
fmt(root, 'some2', source, expected)
fmt_check(root, 'some', source, 1)

# 不在は伝播する: 囲む関数が None を返す。
absent = head + f'def g (id: Int): Int Option Pure =>\n  {body}\ndef main = g (-{id_value})\n'
expected_absent = head + f'def g (id: Int): Int Option Pure =>\n  let user? = find_user id -> Some(user + 1)\ndef main = g (-{id_value})\n'
check_fix(root, 'none', absent, expected_absent)
build_run(root, 'none', 'None')
print('repropagate: the tail branch becomes let p? = and None propagates')
PY
}

# --- 57-d: E508 の第31節の形は引数を value にした関数になる ---
r57_repropagate_arms() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix, fmt

import random
rng = random.Random(60)
id_value = rng.randint(1, 9)
printed = f'Some({id_value + 1})'

head = 'def find_user (id: Int): Int Option Pure => if id > 0 => Some(id) | None\n'
source = head + f'def g\n| Some(user) => Some(user + 1)\n| None => None\ndef main = g (find_user {id_value})\n'
expected = head + f'def g value => let user? = value -> Some(user + 1)\ndef main = g (find_user {id_value})\n'
root = Path(sys.argv[1]) / 'd'
check_code(root, 'def', source, 'E508', 'def g value => let user? = value -> Some(user + 1)')
check_fix(root, 'def', source, expected)
build_run(root, 'def', printed)
fmt(root, 'def2', source, expected)

source = head + f'def main = (fn(| Some(user) => Some(user + 1) | None => None)) (find_user {id_value})\n'
expected = head + f'def main = (fn(value => let user? = value -> Some(user + 1))) (find_user {id_value})\n'
check_code(root, 'fn', source, 'E508', 'fn(value => let user? = value -> Some(user + 1))')
check_fix(root, 'fn', source, expected)
build_run(root, 'fn', printed)
print('repropagate arms: the parameter is named value and the body is the let form')
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

# 本体でない位置の E508: 伝播先が違うので書き換えない。
consume = 'def unwrap_or (x: Int Option, default: Int): Int Pure => if x | Some(v) => v | default\n'
no_rewrite(root, 'nested', head + consume + 'def g (id: Int): Int Pure =>\n  let y = if find_user id\n  | Some(user) => Some(user + 1)\n  | None => None ->\n  unwrap_or y 0\ndef main = g 1\n', 'E508')

# fallback を持つ E507: scrutinee を 2 回評価し得る。
no_rewrite(root, 'fallback', 'def f (x: Int Option): Int Option Pure =>\n  if x\n  | Some(v) => Some(v)\n  | x\ndef main = f (Some(1))\n', 'E507')

# 束縛名が 1 つも無い E508: do が正準形になる。
no_rewrite(root, 'binderless', 'type Wrap = | Wrap\ndef f (x: Wrap Option): Int Option Pure =>\n  if x\n  | Some(Wrap) => Some(1)\n  | None => None\ndef main = f (Some(Wrap))\n', 'E508')

# 本体が `value` を自由に参照する E508（第31節の形）: 引数の名前が捕捉する。
capture = ('def merge (user: Int, rest: Int Option): Int Option Pure =>\n'
           '  if rest\n  | Some(n) => Some(user + n)\n  | None => Some(user)\n'
           'def main =\n  let value = Some(100) ->\n  (fn(| Some(user) => merge user value | None => None)) (Some(1))\n')
no_rewrite(root, 'capture', capture, 'E508')

# 範囲にコメントを含む E507: コメントを消さない。
no_rewrite(root, 'comment', 'def f (x: Int Option): Int Option Pure =>\n  if x\n  // keep\n  | Some(v) => Some(v)\n  | None => None\ndef main = f (Some(1))\n', 'E507')
print('declines: nested E508, fallback E507, binderless E508 and a comment range are left alone')
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
head = 'def find_user (id: Int): Int Option Pure => if id > 0 => Some(id) | None\n'
shapes = [
    ('if', 'def f (x: Int Option): Int Option Pure =>\n  if x\n  | Some(v) => Some(v)\n  | None => None\ndef main = f (Some(2))\n',
     'def f (x: Int Option): Int Option Pure =>\n  x\ndef main = f (Some(2))\n'),
    ('def', 'def f\n| Some(v) => Some(v)\n| None => None\ndef main = f (Some(2))\n',
     'def f value => value\ndef main = f (Some(2))\n'),
    ('repropagate', head + 'def g (id: Int): Int Option Pure =>\n  if find_user id\n  | Some(user) => Some(user + 1)\n  | None => None\ndef main = g 2\n',
     head + 'def g (id: Int): Int Option Pure =>\n  let user? = find_user id -> Some(user + 1)\ndef main = g 2\n'),
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

# --- 57-g: 仕様と層と lock ---
r57_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
section = spec.split('## 57. ', 1)[1].split('\n## ', 1)[0]
for word in ('E507', 'E508', 'def f value => value', 'let', '本体ちょうど', 'fallback'):
    assert word in section, f'§57 does not mention {word}'
assert '局所的な書き換えを持たない正準形違反（E507 / E508）' not in spec, '§58.1 still calls E507 / E508 unfixable'
tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r57' in full.split(), 'r57 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert 'verify-r57' in trace, 'spec-trace does not name verify-r57'
wants = Path('docs/wants.md').read_text()
assert 'R57' in wants, 'wants.md does not record R57'
print('docs: §57, §58.1, the trace rows, the tier and wants')
PY
}

r57_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '57-a the if form becomes its scrutinee' r57_if
check '57-b the arm forms become identity functions' r57_arms
check '57-c the tail E508 becomes let p? = and propagates' r57_repropagate_if
check '57-d the E508 arm forms name the parameter value' r57_repropagate_arms
check '57-e forms without a local rewrite are left alone' r57_declines
check '57-f check --fix, fmt and build --fix share one rewrite' r57_shared
check '57-g spec, trace, tier, wants' r57_docs
check '57-g corpus lock' r57_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
