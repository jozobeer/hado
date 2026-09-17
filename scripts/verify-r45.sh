#!/usr/bin/env bash
# tier: full
# R45 複数の scrutinee とパターンの腕の条件列（docs/plan.md 41 節）。
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

# 生成する探針の共通の道具。check / build と比較を 1 箇所に閉じ、
# 比較が差を検出できることを control で先に確かめる（docs/plan.md 39 節）。
cat >"$workdir/harness.py" <<'PY'
import re
import subprocess
from pathlib import Path


def run_check(root, name, source):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=False)
    (directory / 'main.hd').write_text(source)
    return subprocess.run(
        ['cargo', 'run', '--offline', '-q', '--', 'check', str(directory / 'main.hd')],
        capture_output=True)


def expect_ok(root, name, source):
    result = run_check(root, name, source)
    assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr.decode())


def expect_code(root, name, source, code, line=None, col=None):
    result = run_check(root, name, source)
    assert result.returncode == 1, (name, result.returncode, result.stderr.decode())
    match = re.match(rb'[^:]+:(\d+):(\d+): error\[(E\d+)\]', result.stderr)
    assert match, (name, result.stderr.decode())
    assert match.group(3).decode() == code, (name, code, result.stderr.decode())
    if line is not None:
        assert int(match.group(1)) == line, (name, line, result.stderr.decode())
    if col is not None:
        assert int(match.group(2)) == col, (name, col, result.stderr.decode())


def build_printed(root, name, source, expected):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=False)
    (directory / 'main.hd').write_text(source)
    build = subprocess.run(
        ['cargo', 'run', '--offline', '-q', '--', 'build', str(directory / 'main.hd')],
        capture_output=True)
    assert build.returncode == 0 and build.stderr == b'', (name, build.returncode, build.stderr.decode())
    result = subprocess.run([str(directory / 'main')], capture_output=True, cwd=directory, stdin=subprocess.DEVNULL)
    assert result.returncode == 0, (name, result.returncode, result.stderr.decode())
    assert result.stderr == b'', (name, result.stderr.decode())
    assert result.stdout == expected.encode(), (name, result.stdout, expected)


def hado_list(values):
    return '[' + ', '.join(str(value) for value in values) + ']'


def control(root):
    """The comparison must reject a wrong code and a wrong output."""
    try:
        expect_code(root, 'control-code', 'def main = x\n', 'E999')
    except AssertionError:
        pass
    else:
        raise AssertionError('expect_code accepted a wrong code')
    try:
        expect_ok(root, 'control-ok', 'def main = x\n')
    except AssertionError:
        pass
    else:
        raise AssertionError('expect_ok accepted a rejected program')
    try:
        build_printed(root, 'control-printed', 'def main = 1\n', '2\n')
    except AssertionError:
        pass
    else:
        raise AssertionError('build_printed accepted a wrong output')
PY

# --- 45-a: 網羅性の行列。判定器が同じ行列で網羅性を計算し、E402 / E403 / 受理と照合する ---
r45_coverage_matrix() {
    python3 - "$workdir" <<'PY'
import itertools
import os
import random
import sys

sys.path.insert(0, sys.argv[1])
from harness import control, expect_code, expect_ok

control(sys.argv[1] + '/a')
seed = int(os.environ.get('HADO_SEED', '45'))
rng = random.Random(seed)

# 列に使う型。名前 -> (constructor, payload の数)
TYPES = {
    'C2': [('A', 0), ('B', 0)],
    'C3': [('P', 0), ('Q', 0), ('R', 0)],
    'W': [('K', 1), ('L', 0)],
}
VALUES = {'C2': 'A', 'C3': 'P', 'W': 'L'}


def decls(columns):
    # 使う型だけ宣言する（使わない型は E513 になる）
    return ''.join(
        f"type {name} = " + ' '.join(f'| {ctor}' + ('(Int)' if arity else '') for ctor, arity in TYPES[name]) + '\n'
        for name in TYPES if name in columns)


def cell_text(column, cell):
    if cell is None:
        return '_'
    arity = dict(TYPES[column])[cell]
    return cell + ('(_)' if arity else '')


def exhaustive(columns, rows):
    for combo in itertools.product(*(list(dict(TYPES[column])) for column in columns)):
        if not any(all(cell is None or cell == value for cell, value in zip(row, combo)) for row in rows):
            return False
    return True


def subsumed(earlier, later):
    return all(a is None or a == b for a, b in zip(earlier, later))


counts = {'ok': 0, 'E402': 0, 'E403': 0}
index = 0
attempts = 0
while sum(counts.values()) < 36 and attempts < 2000:
    attempts += 1
    columns = [rng.choice(list(TYPES)) for _ in range(rng.choice([2, 2, 3]))]
    rows = []
    for _ in range(rng.randint(1, 5)):
        row = tuple(rng.choice([None] + list(dict(TYPES[column]))) for column in columns)
        if all(cell is None for cell in row):
            continue
        if any(subsumed(earlier, row) for earlier in rows):
            continue
        rows.append(row)
    if not rows:
        continue
    fallback = rng.random() < 0.5
    complete = exhaustive(columns, rows)
    expected = 'E402' if not complete and not fallback else 'E403' if complete and fallback else 'ok'
    if counts[expected] >= 16:
        continue
    params = ', '.join(f'{chr(97 + i)}: {column}' for i, column in enumerate(columns))
    scrutinees = ', '.join(chr(97 + i) for i in range(len(columns)))
    arms = ''.join(
        f'  | {", ".join(cell_text(column, cell) for column, cell in zip(columns, row))} => {number}\n'
        for number, row in enumerate(rows))
    if fallback:
        arms += f'  | {len(rows)}\n'
    declared = decls(columns)
    source = declared + f'def f ({params}): Int =>\n  if {scrutinees}\n{arms}'
    source += 'def main = f ' + ' '.join(VALUES[column] for column in columns) + '\n'
    name = f'matrix-{index}'
    index += 1
    if expected == 'ok':
        expect_ok(sys.argv[1] + '/a', name, source)
    else:
        # E402 は `if`、E403 は到達不能な fallback の `|` を指す（§56）
        line = declared.count('\n') + 2 + (len(rows) + 1 if expected == 'E403' else 0)
        expect_code(sys.argv[1] + '/a', name, source, expected, line=line, col=3)
    counts[expected] += 1
assert all(count >= 4 for count in counts.values()), counts
print(f'coverage matrix: seed {seed}, {counts}')
PY
}

# --- 45-b: 腕の条件列 ---
r45_arm_conditions() {
    python3 - "$workdir" <<'PY'
import os
import random
import sys

sys.path.insert(0, sys.argv[1])
from harness import build_printed, expect_code, expect_ok, hado_list

root = sys.argv[1] + '/b'
seed = int(os.environ.get('HADO_SEED', '45'))
rng = random.Random(seed)

MERGE = '''def rec merge left right comparator =>
  if left, right
  | [], _ => right
  | _, [] => left
  | [a, ...rest_a], [b, ...rest_b] => if
    | comparator a b <= 0 => [a, ...merge rest_a right comparator]
    | [b, ...merge left rest_b comparator]
'''


def int_list(values):
    # 空のリストは filter で型を決めて作る（リテラル [] は型が決まらない）
    return hado_list(values) if values else 'List.filter [0] fn(v => v > 0)'


cases = 0
for index in range(12):
    left = sorted(rng.randint(-5, 9) for _ in range(rng.randint(0, 5)))
    right = sorted(rng.randint(-5, 9) for _ in range(rng.randint(0, 5)))
    if index == 0:
        left, right = [], []
    source = MERGE + f'def main = merge ({int_list(left)}) ({int_list(right)}) fn(a, b => a - b)\n'
    build_printed(root, f'merge-{index}', source, hado_list(sorted(left + right)) + '\n')
    cases += 1

# (2) fallback の無い腕の条件列は E401。アンカーは腕の `if`
source = 'def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if\n    | v > 0 => v\ndef main = f (Some(1))\n'
expect_code(root, 'missing-fallback', source, 'E401', line=4, col=16)

# (3) 条件が 1 本の腕の条件列は E520 にならない
source = 'def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if\n    | v > 0 => v\n    | 0\ndef main = f (Some(1))\n'
expect_ok(root, 'single-condition', source)

# (4) 腕の中の Boolean 二分岐、pattern 分岐、条件列の腕の中の条件列は E101
forms = {
    'boolean-in-arm': 'def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if v > 0 => v | 0\ndef main = f (Some(1))\n',
    'pattern-in-arm': 'def f (x: Int Option Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if v | None => 1 | Some(w) => w\ndef main = f (Some(Some(1)))\n',
    'conditions-in-condition': 'def f (x: Int Option): Int =>\n  if x\n  | None => 0\n  | Some(v) => if\n    | v > 0 => if\n      | v > 5 => 5\n      | v\n    | 0\ndef main = f (Some(1))\n',
}
for name, source in forms.items():
    expect_code(root, name, source, 'E101')
print(f'arm conditions: {cases} merge cases, E401 anchor, single condition, {len(forms)} nested forms')
PY
}

# --- 45-c: 列の規則 ---
r45_columns() {
    python3 - "$workdir" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
from harness import expect_code, expect_ok

root = sys.argv[1] + '/c'
head = 'def f (a: Int List, b: Int List): Int =>\n  if a, b\n'
tail = 'def main = f [] []\n'
expect_code(root, 'fewer-columns', head + '  | [] => 0\n  | [_, ..._], [_, ..._] => 1\n  | 2\n' + tail, 'E125', line=3, col=3)
expect_code(root, 'more-columns', head + '  | [], _, _ => 0\n  | 1\n' + tail, 'E125', line=3, col=3)
expect_code(root, 'all-variables', head + '  | x, y => List.length x + List.length y\n' + tail, 'E512')
expect_code(root, 'all-underscore', head + '  | _, _ => 0\n' + tail, 'E503')
expect_ok(root, 'partial-underscore',
          'def f (a: Int List, b: Int List): Int List =>\n  if a, b\n  | _, [] => a\n  | [], _ => b\n  | [_, ..._], [_, ..._] => a\n' + tail)
expect_ok(root, 'bool-columns',
          'def f (a: Bool, b: Bool): Int =>\n  if a, b\n  | true, true => 1\n  | true, false => 2\n  | false, _ => 3\ndef main = f true false\n')
expect_code(root, 'bool-column',
            'def f (a: Bool): Int =>\n  if a\n  | true => 1\n  | false => 2\ndef main = f true\n', 'E520')
print('columns: E125 both directions, E512, E503, partial underscore, Bool columns')
PY
}

# --- 45-d: 既存の読みが動かない ---
r45_existing_reading() {
    python3 - "$workdir" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
from harness import expect_code, expect_ok

root = sys.argv[1] + '/d'
# fallback の None は戻り型と合わせる（型の E305 は網羅性の E404 より先に当たる）
expect_code(root, 'fallback-not-last', 'def f (x: Int Option): Int Option =>\n  if x\n  | None\n  | Some(v) => Some(v + 1)\ndef main = f None\n', 'E404', line=4, col=3)
expect_ok(root, 'fallback-last', 'def f (x: Int Option): Int =>\n  if x\n  | Some(v) => v\n  | 0\ndef main = f None\n')
expect_code(root, 'sole-parameter', 'def f x =>\n  if x\n  | Some(v) => v\n  | 0\ndef main = f None\n', 'E515')
print('existing reading: E404 at the arm after a pattern-shaped fallback, fallback last, E515')
PY
}

# --- 45-e: 仕様と文法 ---
r45_spec_grammar() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
sentence = '腕の本体に置ける分岐は条件列だけで、条件列の腕には分岐を置けない'
assert spec.count(sentence) == 1, spec.count(sentence)
grammar = spec.split('## 付録C ')[1]
for fragment in ['pattern_branch  := "if" expression ("," expression)* pattern_arms',
                 'pattern_arm     := "|" pattern ("," pattern)* "=>" (arm_body | arm_conditions)',
                 'arm_conditions  := "if" condition_arms']:
    assert fragment in grammar, fragment
assert '| E125 | pattern-column-count | §26 | 構文 |' in spec
assert '## 26.1. パターンの腕の条件列' in spec
print('spec: §30 sentence once, 付録C rules, E125 row, §26.1')
PY
}

r45_grammar_generator() {
    cargo test --offline -q --test r9_grammar >"$workdir/r9.log" 2>&1 || { tail -5 "$workdir/r9.log"; return 1; }
}

# --- 45-f: stdlib の merge と List.sort の実行 ---
r45_stdlib_merge() {
    python3 - "$workdir" <<'PY'
import os
import random
import re
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
from harness import build_printed, hado_list

source = Path('stdlib/list.hd').read_text()
assert len(re.findall(r'^def rec merge ', source, re.M)) == 1, 'merge must be one definition'
assert not re.search(r'merge_right|merge_heads', source), 'merge helpers must be gone'

root = sys.argv[1] + '/f'
seed = int(os.environ.get('HADO_SEED', '45'))
rng = random.Random(seed)
lists = [[], [1], [2, 1], [3, 8, -2, 3, 1], [5, 5, 5], list(range(20, 0, -1))]
lists += [[rng.randint(-9, 9) for _ in range(rng.randint(0, 12))] for _ in range(6)]
for index, values in enumerate(lists):
    literal = hado_list(values) if values else 'List.filter [0] fn(v => v > 0)'
    source = f'def main = List.sort ({literal}) fn(a, b => a - b)\n'
    build_printed(root, f'sort-{index}', source, hado_list(sorted(values)) + '\n')
    source = f'def main = List.sort ({literal}) fn(a, b => b - a)\n'
    build_printed(root, f'sort-desc-{index}', source, hado_list(sorted(values, reverse=True)) + '\n')
print(f'stdlib merge: one definition, {len(lists)} lists sorted both ways')
PY
}

r45_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '45-a coverage matrix over 2-3 columns' r45_coverage_matrix
check '45-b arm conditions: merge runs, E401 anchor, single condition, E101 forms' r45_arm_conditions
check '45-c column rules: E125, E512, E503, partial underscore, Bool columns' r45_columns
check '45-d existing reading: E404, fallback last, E515' r45_existing_reading
check '45-e spec: §30 sentence, 付録C rules, E125 row, §26.1' r45_spec_grammar
check '45-e grammar generator visits the new rules' r45_grammar_generator
check '45-f stdlib merge is one definition and List.sort sorts' r45_stdlib_merge
check '45-f corpus lock' r45_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
