#!/usr/bin/env bash
# tier: full
# R56 E305 に期待型と実型を載せる（docs/plan.md 52 節）。
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

# 形ごとの入口ファイル。食い違いの位置と対は判定器側で計算する。
ADD1 = 'def add1 (x: Int): Int Pure => x + 1\n\ndef main = add1 "%s"\n'
TWICE = 'def twice f x => f (f x)\n\ndef main = twice (fn(y => y + 1)) %s\n'
APPLY2 = 'def apply2 (f: Fn(Int => Int)): Int Pure => f 1\n\ndef main = apply2 fn(x => x > 0)\n'
SELF = 'def self_apply f => f f\n\ndef main = self_apply %d\n'
TWICE_VAR = 'def twice f x => f (f x)\n\ndef main = twice true (fn(y => y) 1)\n'
LIST = 'def main = [%d, true]\n'
ARMS = 'def main = if 1 > 2 => %d | "x"\n'
GOOD = 'def add1 (x: Int): Int Pure => x + 1\n\ndef main = add1 %d\n'

DIAGNOSTIC = re.compile(
    r'^(?P<path>.*?):(?P<line>\d+):(?P<col>\d+): error\[(?P<code>[A-Z]\d+)\]: (?P<message>.*)$',
    re.M,
)


def run(*args):
    return subprocess.run([HADO, *args], capture_output=True)


def project(directory, source):
    path = Path(directory)
    path.mkdir(parents=True, exist_ok=True)
    (path / 'main.hd').write_text(source)
    return path / 'main.hd'


def padded(source):
    """Random blank lines and indentation, so the anchor has to be computed."""
    blank = random.randint(1, 6)
    pad = random.randint(1, 8)
    lines = source.splitlines()
    text = '\n' * blank + '\n'.join((' ' * pad + line) if line else '' for line in lines) + '\n'
    return text


def anchor(text, needle):
    """The 1-based line and column of the needle's first occurrence."""
    index = text.index(needle)
    line = text.count('\n', 0, index) + 1
    col = index - text.rfind('\n', 0, index)
    return line, col


def diagnostic(entry):
    """The single diagnostic `check` reports, as (line, col, code, message)."""
    result = run('check', str(entry))
    assert result.returncode == 1, (result.returncode, result.stdout, result.stderr)
    match = DIAGNOSTIC.search(result.stderr.decode())
    assert match, result.stderr
    return int(match['line']), int(match['col']), match['code'], match['message']


def checked(entry):
    """The exit code of a program that passes."""
    result = run('check', str(entry))
    assert result.stderr == b'', result.stderr
    return result.returncode


def shape(name, template, *arguments):
    text = padded(template % arguments)
    entry = project(Path(name), text)
    return entry, text
PY

# --- 56-a: 適用の引数の食い違いは実引数と仮引数の対 ---
r56_application() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import ADD1, GOOD, anchor, checked, diagnostic, shape

random = __import__('random')
random.seed(56)
entry, text = shape(Path(sys.argv[1]) / 'a', ADD1, 'hi')
line, col = anchor(text, '"hi"')
assert diagnostic(entry) == (line, col, 'E305', 'expected Int, found String'), diagnostic(entry)
# control: 同じ形で実引数の型が合っていれば通る。
good, _ = shape(Path(sys.argv[1]) / 'a/good', GOOD, 1)
assert checked(good) == 0, good
print('application: the argument and its parameter are named at the argument')
PY
}

# --- 56-b: 仮引数の型が型変数のときの対 ---
r56_variable() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import APPLY2, TWICE, TWICE_VAR, diagnostic, shape

# 仮引数の型が未確定の型変数のまま残る形（§58.3 の改名、§20 の行の印字）。
entry, _ = shape(Path(sys.argv[1]) / 'b', TWICE_VAR)
assert diagnostic(entry)[2:] == ('E305', 'expected Fn(A => A {B}), found Bool'), diagnostic(entry)
# 実引数の側にだけ変数が残る形。
entry, _ = shape(Path(sys.argv[1]) / 'b/apply2', APPLY2)
assert diagnostic(entry)[2:] == ('E305', 'expected Fn(Int => Int), found Fn(Int => Bool {A})'), diagnostic(entry)
# 仮引数の型が第 1 引数で確定する形は、確定した型を出す。
entry, _ = shape(Path(sys.argv[1]) / 'b/twice', TWICE, 'true')
assert diagnostic(entry)[2:] == ('E305', 'expected Int, found Bool'), diagnostic(entry)
print('variable: a parameter type variable is renamed')
PY
}

# --- 56-c: 出現検査は cannot unify のままで、共有する変数は同じ名前 ---
r56_occurs() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import SELF, diagnostic, shape

entry, _ = shape(Path(sys.argv[1]) / 'c', SELF, 1)
assert diagnostic(entry)[2:] == ('E306', 'cannot unify A with Fn(A => B {C})'), diagnostic(entry)
print('occurs: one table per diagnostic keeps a shared variable one name')
PY
}

# --- 56-d: 引数以外の食い違い（役割が確定する箇所は期待と実、リストは対称な文面のまま） ---
r56_other() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import ARMS, LIST, anchor, diagnostic, shape

entry, _ = shape(Path(sys.argv[1]) / 'd/list', LIST, 1)
assert diagnostic(entry)[2:] == ('E305', 'cannot unify Bool with A'), diagnostic(entry)
# R60: 条件式の腕は期待の側が先の腕なので、期待と実の形に変わり、先の腕の位置が関連位置になる。
entry, text = shape(Path(sys.argv[1]) / 'd/arms', ARMS, 1)
line, col, code, message = diagnostic(entry)
assert (line, col, code) == (*anchor(text, '"x"'), 'E305'), (line, col, code)
related_line, related_col = anchor(text, '1 |')
assert message == f'expected Int, found String; expected here at main.hd:{related_line}:{related_col}', message
print('other: a determined site names the pair, a list keeps the symmetric wording')
PY
}

# --- 56-e: どの形も内部の番号を本文に出さない ---
r56_no_numbers() {
    python3 - "$workdir" "$hado" <<'PY'
import re
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import ADD1, APPLY2, ARMS, LIST, SELF, TWICE, TWICE_VAR, diagnostic, shape

shapes = [(ADD1, 'x'), (TWICE, 'true'), (TWICE_VAR,), (APPLY2,), (SELF, 1), (LIST, 1), (ARMS, 1)]
for index, (template, *arguments) in enumerate(shapes):
    entry, _ = shape(Path(sys.argv[1]) / f'e/{index}', template, *arguments)
    message = diagnostic(entry)[3]
    assert not re.search(r'\bt\d+\b', message), message
    assert message.startswith('expected ') or message.startswith('cannot unify '), message
print('numbers: no diagnostic prints an internal variable number')
PY
}

# --- 56-f: 仕様と層と lock ---
r56_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert '## 15.1. ' in spec, 'no §15.1'
section = spec.split('## 15.1. ', 1)[1].split('\n## ', 1)[0]
for word in ('expected', 'found', 'cannot unify', '`A` `B`', '内部の番号'):
    assert word in section, f'§15.1 does not define {word}'
tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r56' in full.split(), 'r56 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert '§15.1' in trace, 'spec-trace has no §15.1 row'
wants = Path('docs/wants.md').read_text()
assert 'R56' in wants, 'wants.md does not record R56'
print('docs: §15.1, the trace row, the tier and wants')
PY
}

r56_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '56-a an application mismatch names the argument and its parameter' r56_application
check '56-b a parameter type variable is renamed' r56_variable
check '56-c an occurs check keeps one name for a shared variable' r56_occurs
check '56-d a determined site names the pair, a list stays symmetric' r56_other
check '56-e no diagnostic prints an internal variable number' r56_no_numbers
check '56-f spec, trace, tier, wants' r56_docs
check '56-f corpus lock' r56_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
