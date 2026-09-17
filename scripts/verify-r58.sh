#!/usr/bin/env bash
# tier: full
# R58 E205 / E602 の関連位置（docs/plan.md 54 節）。
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
import re
import subprocess
from pathlib import Path

HADO = None
FIRST = 'first declared'
CONTRACT = 'declared in the contract'


def project(root, name, files, *args, entry='main.hd'):
    """Write `files` under `<root>/<name>` and run the CLI on the entry."""
    directory = Path(root) / name
    for relative, text in files.items():
        path = directory / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    return subprocess.run([HADO, *args, str(directory / entry)], capture_output=True)


def source_of(root, name, relative='main.hd'):
    return (Path(root) / name / relative).read_text()


def diagnostic(result, code, status=1):
    """The run's single diagnostic line, split into its parts."""
    assert result.returncode == status, (result.returncode, result.stderr.decode())
    lines = [line for line in result.stderr.decode().splitlines() if line]
    assert len(lines) == 1, ('one diagnostic per run', lines)
    match = re.fullmatch(r'([^\s:]+):(\d+):(\d+): error\[(E\d+)\]: (.*)', lines[0])
    assert match, lines[0]
    assert match.group(4) == code, (code, lines[0])
    return {'path': match.group(1), 'line': int(match.group(2)), 'col': int(match.group(3)),
            'message': match.group(5)}


def related(found, label):
    """The related position of a diagnostic, or None when it has none."""
    match = re.search(r'; ' + re.escape(label) + r' at ([^\s:]+):(\d+):(\d+)$', found['message'])
    return None if match is None else (match.group(1), int(match.group(2)), int(match.group(3)))


def position(text, needle, occurrence=1):
    """The 1-based (line, col) of the nth occurrence of `needle`."""
    index = -1
    for _ in range(occurrence):
        index = text.index(needle, index + 1)
    before = text[:index]
    return before.count('\n') + 1, index - (before.rfind('\n') + 1) + 1


def anchor(found):
    return found['path'], found['line'], found['col']
PY

# --- 58-a: 同一ファイルの 2 回目は 1 回目を指す ---
r58_same_file() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import FIRST, anchor, diagnostic, position, project, related

import random
rng = random.Random(58)
name = 'D' + str(rng.randint(1, 9))
blank = rng.randint(0, 3)
indent = rng.randint(0, 4)
pad = '\n' * blank + ' ' * indent
source = (f'{pad}type {name} = | Red(Int)\n'
          f'{pad}type {name} = | Blue(Int)\n'
          'def main = 1\n')
root = Path(sys.argv[1]) / 'a'

# control: 欠陥の無いプログラムは何も出さない。
clean = project(root, 'clean', {'main.hd': 'def main = 1\n'}, 'check')
assert clean.returncode == 0 and clean.stderr == b'', (clean.returncode, clean.stderr.decode())

found = diagnostic(project(root, 'dup', {'main.hd': source}, 'check'), 'E205')
assert anchor(found) == ('main.hd', *position(source, name, 2)), found
assert related(found, FIRST) == ('main.hd', *position(source, name, 1)), found
print('same file: the second declaration names the first')
PY
}

# --- 58-b: ファイルをまたぐ 2 回目は entry を指す ---
r58_cross_file() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import FIRST, anchor, diagnostic, position, project, related

import random
rng = random.Random(59)
name = 'D' + str(rng.randint(1, 9))
entry = f'type {name} = | Red(Int)\ndef main = 1\n'
other = f'type {name} = | Blue(Int)\n'
root = Path(sys.argv[1]) / 'b'

found = diagnostic(project(root, 'cross', {'main.hd': entry, 'other.hd': other}, 'check'), 'E205')
assert anchor(found) == ('other.hd', *position(other, name, 1)), found
assert related(found, FIRST) == ('main.hd', *position(entry, name, 1)), found
print('cross file: the later file points into the entry')
PY
}

# --- 58-c: 値の空間と record の field の重複 ---
r58_repeats() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import FIRST, anchor, diagnostic, position, project, related

root = Path(sys.argv[1]) / 'c'

# 値の空間（ファイルごと）
source = 'def color (x: Int): Int Pure => x\n' * 2 + 'def main = color 1\n'
found = diagnostic(project(root, 'value', {'main.hd': source}, 'check'), 'E205')
assert anchor(found) == ('main.hd', *position(source, 'color', 2)), found
assert related(found, FIRST) == ('main.hd', *position(source, 'color', 1)), found

# record の field（`a:` の位置を数える）
source = 'type R = { a: Int; a: Int }\ndef main = 1\n'
found = diagnostic(project(root, 'field', {'main.hd': source}, 'check'), 'E205')
assert anchor(found) == ('main.hd', *position(source, 'a:', 2)), found
assert related(found, FIRST) == ('main.hd', *position(source, 'a:', 1)), found
print('repeats: a value and a record field point at their first declaration')
PY
}

# --- 58-d: 根の名前との衝突と library の 2 つ目 ---
r58_conflicts() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import FIRST, anchor, diagnostic, position, project, related

import random
rng = random.Random(60)
root = Path(sys.argv[1]) / 'd'

# サブディレクトリの宣言が根の名前と同じ
name = 'D' + str(rng.randint(1, 9))
entry = f'type {name} = | Red(Int)\ndef main = 1\n'
sub = f'type {name} = | Blue(Int)\n'
found = diagnostic(project(root, 'root', {'main.hd': entry, 'sub/other.hd': sub}, 'check'), 'E205')
assert anchor(found) == ('sub/other.hd', *position(sub, name, 1)), found
assert related(found, FIRST) == ('main.hd', *position(entry, name, 1)), found

# library の 2 つ目
first = 'library\ntype A1 = | Red(Int)\n'
second = 'library\ntype A2 = | Blue(Int)\n'
found = diagnostic(project(root, 'library', {'main.hd': first, 'other.hd': second}, 'check'), 'E205')
assert anchor(found) == ('other.hd', 1, 1), found
assert related(found, FIRST) == ('main.hd', 1, 1), found
print('conflicts: the root name and the first library are named')
PY
}

# --- 58-e: 契約が別ファイルの E602 ---
r58_contract() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import CONTRACT, anchor, diagnostic, position, project, related

root = Path(sys.argv[1]) / 'e'
contract = 'module Store\ndef read (x: Int): Int\ndef write (x: Int): Int\n'

# 後ろの member が欠けている
missing = 'implement Store as Store\ndef read (x: Int): Int Pure => x\ndef main = 1\n'
found = diagnostic(project(root, 'tail', {'main.hd': missing, 'store.hdi': contract}, 'check'), 'E602')
assert anchor(found) == ('main.hd', 1, 1), found
assert related(found, CONTRACT) == ('store.hdi', *position(contract, 'write', 1)), found

# 前の member が欠けている
missing = 'implement Store as Store\ndef write (x: Int): Int Pure => x\ndef main = 1\n'
found = diagnostic(project(root, 'head', {'main.hd': missing, 'store.hdi': contract}, 'check'), 'E602')
assert related(found, CONTRACT) == ('store.hdi', *position(contract, 'read', 1)), found

# member の型が違う
mismatch = ('implement Store as Store\ndef read (x: Int): Bool Pure => true\n'
            'def write (x: Int): Int Pure => x\ndef main = 1\n')
found = diagnostic(project(root, 'type', {'main.hd': mismatch, 'store.hdi': contract}, 'check'), 'E602')
assert anchor(found) == ('main.hd', 1, 1), found
assert related(found, CONTRACT) == ('store.hdi', *position(contract, 'read', 1)), found
print('hdi: the contract member is named from the hdi file')
PY
}

# --- 58-f: 契約が entry ファイルにある E602 ---
r58_same_file_contract() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import CONTRACT, anchor, diagnostic, position, project, related

root = Path(sys.argv[1]) / 'f'
source = ('def main = M.inc 1\n'
          'module S\n'
          'def inc (x: Int): Int\n'
          'def dec (x: Int): Int\n'
          'implement S as M\n'
          'def inc x => x + 1\n')
found = diagnostic(project(root, 'same', {'main.hd': source}, 'check'), 'E602')
assert anchor(found) == ('main.hd', *position(source, 'implement', 1)), found
assert related(found, CONTRACT) == ('main.hd', *position(source, 'dec', 1)), found
print('same file: the contract sits in the entry file')
PY
}

# --- 58-g: 1 回目の宣言が無い形 ---
r58_negatives() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import CONTRACT, FIRST, diagnostic, project, related

root = Path(sys.argv[1]) / 'g'

# prelude の操作名と衝突する宣言: 1 回目は利用者のソースに無い。
source = 'def raise (x: Int): Int Pure => x\ndef main = 1\n'
found = diagnostic(project(root, 'prelude', {'main.hd': source}, 'check'), 'E205')
assert related(found, FIRST) is None, found
assert ' at ' not in found['message'], found

# 抽象型の循環: 1 回目の宣言が無い。
cyclic = 'def main = M.of 1\nmodule S\ntype T\ndef of (x: Int): T Pure\nimplement S as M\ntype T = T\n'
found = diagnostic(project(root, 'cycle', {'main.hd': cyclic}, 'check'), 'E602')
assert related(found, CONTRACT) is None, found
assert ' at ' not in found['message'], found

# 正常なプログラム
clean = project(root, 'clean', {'main.hd': 'def main = 1\n'}, 'check')
assert clean.returncode == 0 and clean.stderr == b'', (clean.returncode, clean.stderr.decode())
print('negatives: prelude conflicts and cycles carry no related position')
PY
}

# --- 58-h: E205 / E602 は書き換えを持たない ---
r58_untouched() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, source_of

root = Path(sys.argv[1]) / 'h'
source = 'type D1 = | Red(Int)\ntype D1 = | Blue(Int)\ndef main = 1\n'

result = project(root, 'fix', {'main.hd': source}, 'check', '--fix')
assert result.returncode == 1, (result.returncode, result.stderr.decode())
assert source_of(root, 'fix') == source, 'check --fix must not write'

result = project(root, 'fmt', {'main.hd': source}, 'fmt')
assert result.returncode == 0 and result.stderr == b'', (result.returncode, result.stderr.decode())
assert source_of(root, 'fmt') == source, 'fmt must not write'
print('untouched: E205 and E602 have no rewrite')
PY
}

# --- 58-i: 仕様と層と lock ---
r58_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert '関連位置' in spec, '§56 does not mention the related position'
assert 'first declared' in spec, '§56 does not write the E205 label'
assert 'declared in the contract' in spec, '§56 does not write the E602 label'
rows = {line.split('|')[1].strip(): line for line in spec.splitlines() if line.startswith('| E')}
assert 'first declared' in rows['E205'], rows['E205']
assert 'declared in the contract' in rows['E602'], rows['E602']
tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r58' in full.split(), 'r58 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert 'verify-r58' in trace, 'spec-trace does not name verify-r58'
wants = Path('docs/wants.md').read_text()
assert 'R58' in wants, 'wants.md does not record R58'
print('docs: §56, the error table, the trace rows, the tier and wants')
PY
}

r58_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '58-a the second declaration names the first' r58_same_file
check '58-b the later file points into the entry' r58_cross_file
check '58-c a repeated value and a repeated record field' r58_repeats
check '58-d the root name and the first library are named' r58_conflicts
check '58-e the contract member is named from the hdi' r58_contract
check '58-f the contract in the entry file' r58_same_file_contract
check '58-g prelude conflicts and cycles carry no position' r58_negatives
check '58-h E205 and E602 have no rewrite' r58_untouched
check '58-i spec, trace, tier, wants' r58_docs
check '58-i corpus lock' r58_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
