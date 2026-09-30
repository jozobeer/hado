#!/usr/bin/env bash
# tier: full
# R46 正準形の診断と check --fix（`docs/rounds/r46.md`）。
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
import hashlib
import re
import subprocess
from pathlib import Path

HADO = None

def run(root, name, source, *args):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    (directory / 'main.hd').write_text(source)
    return subprocess.run([HADO, *args, str(directory / 'main.hd')], capture_output=True)

def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def check_ok(root, name, source):
    result = run(root, name, source, 'check')
    assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr.decode())

def check_code(root, name, source, code):
    result = run(root, name, source, 'check')
    assert result.returncode == 1, (name, result.returncode, result.stderr.decode())
    match = re.match(rb'[^:]+:(\d+):(\d+): error\[(E\d+)\]', result.stderr)
    assert match and match.group(3).decode() == code, (name, code, result.stderr.decode())
    return result

def check_fix(root, name, source):
    result = run(root, name, source, 'check', '--fix')
    return result

def build_run(root, name, expect_ok):
    directory = Path(root) / name
    build = subprocess.run([HADO, 'build', str(directory / 'main.hd')], capture_output=True)
    assert build.returncode == 0 and build.stderr == b'', (name, build.returncode, build.stderr.decode())
    result = subprocess.run([str(directory / 'main')], capture_output=True, stdin=subprocess.DEVNULL)
    assert result.returncode == 0, (name, result.returncode, result.stderr.decode())
    assert result.stderr == b'', (name, result.stderr.decode())
    # `Int` の `main` は印字せず終了コードになる（§54）。値の門番は
    # `Int.to_string` で包んだ入口を走らせ、印字された値を見る。
    assert result.stdout == expect_ok.encode(), (name, result.stdout, expect_ok)

def comments(source):
    return [line for line in source.splitlines() if line.strip().startswith('//')]

def control(root):
    """The comparisons must reject a wrong code and a wrong rewrite."""
    try:
        check_code(root, 'control-code', 'def main = x\n', 'E999')
    except AssertionError:
        pass
    else:
        raise AssertionError('check_code accepted a wrong code')
    try:
        check_ok(root, 'control-ok', 'def main = x\n')
    except AssertionError:
        pass
    else:
        raise AssertionError('check_ok accepted a rejected program')
    try:
        build_run(root, 'control-run', 'def main = 1\n', '2\n')
    except AssertionError:
        pass
    else:
        raise AssertionError('build_run accepted a wrong output')
PY

# --- 46-a: E517 の書き換え。診断 → --fix → check → build の出力一致 ---
r46_condition_arms() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix, comments

root = sys.argv[1] + '/a'
seed = int(os.environ.get('HADO_SEED', '46'))
rng = random.Random(seed)
Path(root).mkdir(parents=True, exist_ok=True)

for index in range(10):
    limit = rng.randint(-5, 5)
    yes, no = rng.randint(0, 9), rng.randint(0, 9)
    value = rng.randint(-9, 9)
    separator = rng.choice(['\n', ' '])
    comment = rng.choice(['', '// 日本語のコメント\n', '// 前書き\n'])
    source = comment + (
        f'def classify score => if{separator}'
        f'  | score >= {limit} => {yes}{separator}'
        f'  | {no}\n'
        f'def main = classify ({value}) as answer -> Int.to_string answer\n')
    name = f'long-{index}'
    caught = check_code(root, name, source, 'E517')
    assert b'canonical form:' in caught.stderr, (name, caught.stderr.decode())
    fixed = check_fix(root, name, source)
    assert fixed.returncode == 0 and fixed.stderr == b'', (name, fixed.returncode, fixed.stderr.decode())
    text = (Path(root) / name / 'main.hd').read_text()
    assert '=> if' not in text, (name, text)
    for line in comments(source):
        assert line in text, (name, line, text)
    build_run(root, name, f'{yes if value >= limit else no}\n')
    # 直した木は fix を持たない（2 回目は書き換えない）。
    before = (Path(root) / name / 'main.hd').read_bytes()
    again = check_fix(root, name, text)
    assert again.returncode == 0 and again.stderr == b'', (name, again.stderr.decode())
    assert (Path(root) / name / 'main.hd').read_bytes() == before, name
print(f'condition arms: seed {seed}, 10 rewritten programs run')
PY
}

# --- 46-b: E510 の書き換え（式の Nil / Cons）---
r46_expression_lists() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix

root = sys.argv[1] + '/b'
seed = int(os.environ.get('HADO_SEED', '46'))
rng = random.Random(seed)
Path(root).mkdir(parents=True, exist_ok=True)

def written(values):
    body = 'Nil'
    for value in reversed(values):
        body = f'Cons({value}, {body})'
    return body

for index in range(10):
    values = [rng.randint(-9, 9) for _ in range(rng.randint(1, 5))]
    source = f'def main = {written(values)}\n'
    name = f'cons-{index}'
    caught = check_code(root, name, source, 'E510')
    assert b'canonical form:' in caught.stderr, (name, caught.stderr.decode())
    fixed = check_fix(root, name, source)
    assert fixed.returncode == 0 and fixed.stderr == b'', (name, fixed.returncode, fixed.stderr.decode())
    text = (Path(root) / name / 'main.hd').read_text()
    # 正準形の印字は負の Int を括弧で包む（§54 の印字ではなく元の式の印字）。
    written_after = '[' + ', '.join(f'({value})' if value < 0 else str(value) for value in values) + ']'
    assert text == f'def main = {written_after}\n', (name, text)
    build_run(root, name, '[' + ', '.join(str(value) for value in values) + ']\n')
print(f'expression lists: seed {seed}, 10 rewritten lists run')
PY
}

# --- 46-c: E516 の書き換え（パターンの Nil / Cons）---
r46_pattern_lists() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import build_run, check_code, check_fix
import re

root = sys.argv[1] + '/c'
seed = int(os.environ.get('HADO_SEED', '46'))
rng = random.Random(seed)
Path(root).mkdir(parents=True, exist_ok=True)

def written(length):
    body = 'Nil'
    for offset in range(length):
        body = f'Cons(x{offset}, {body})'
    return body

for index in range(10):
    length = rng.randint(1, 4)
    source = 'def f (l: Int List): Int =>\n  if l\n'
    for arm in range(1, length + 1):
        names = ' + '.join(f'x{offset}' for offset in range(arm))
        source += f'  | {written(arm)} => {names}\n'
    source += ('  | 0\ndef main =\n  f ['
               + ', '.join(str(rng.randint(0, 9)) for _ in range(length))
               + '] as answer -> Int.to_string answer\n')
    name = f'pattern-{index}'
    caught = check_code(root, name, source, 'E516')
    assert b'canonical form:' in caught.stderr, (name, caught.stderr.decode())
    fixed = check_fix(root, name, source)
    assert fixed.returncode == 0 and fixed.stderr == b'', (name, fixed.returncode, fixed.stderr.decode())
    text = (Path(root) / name / 'main.hd').read_text()
    assert 'Cons' not in text and 'Nil' not in text, (name, text)
    # 書き換えた `main` は `Int.to_string` で包んである。印字される値を
    # ソースから読む（`f [..]` の並びは書き換えで変わらない）。
    listed = re.search(r'f \[([0-9, ]*)\]', text)
    assert listed, (name, text)
    values = [int(value) for value in listed.group(1).split(', ')]
    build_run(root, name, f'{sum(values)}\n')
print(f'pattern lists: seed {seed}, 10 rewritten patterns run')
PY
}

# --- 46-d: 既定の check / build はファイルを変えない ---
r46_no_write() {
    python3 - "$workdir" "$hado" <<'PY'
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import digest, run

root = Path(sys.argv[1]) / 'd'
root.mkdir(parents=True, exist_ok=True)
rejected = [
    'def main = Cons(1, Nil)\n',
    'def f x => if\n  | x > 0 => 1\n  | 0\ndef main = f 1\n',
    'def main = 5 as x -> x\n',
]
for index, source in enumerate(rejected):
    path = root / f'no-write-{index}' / 'main.hd'
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(source)
    before = digest(path)
    run(str(root), f'no-write-{index}', source, 'check')
    assert digest(path) == before, ('check', index)
    subprocess.run([harness.HADO, 'build', str(path)], capture_output=True)
    assert digest(path) == before, ('build', index)
print(f'read only: {len(rejected)} rejected files unchanged by check and build')
PY
}

# --- 46-e: 直せない診断では止まり、ファイルを変えない ---
r46_stops_without_rewrite() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import check_fix, digest

root = Path(sys.argv[1]) / 'e'
root.mkdir(parents=True, exist_ok=True)
# R47 で E501、R57 で E507 が直せるようになったので、直さないと
# 決めている未使用（wants.md の診断と正準形）の E505 を使う。
path = root / 'unfixable' / 'main.hd'
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text('def f (x: Int): Int Pure =>\n  1 as y ->\n  x\ndef main = f 1\n')
before = digest(path)
result = check_fix(str(root), 'unfixable', path.read_text())
assert result.returncode == 1, result.returncode
assert b'error[E505]' in result.stderr, result.stderr.decode()
assert digest(path) == before, 'a diagnostic without a rewrite must not change the file'

# コメントを含む範囲の書き換えは出ない。報告されるアンカーは外側なので
# ファイルは変わらず、コメントも残る。
commented = root / 'commented' / 'main.hd'
commented.parent.mkdir(parents=True, exist_ok=True)
source = 'def main = Cons(1, // keep\n  Nil)\n'
commented.write_text(source)
result = check_fix(str(root), 'commented', source)
assert result.returncode == 1, result.returncode
assert b'error[E510]' in result.stderr, result.stderr.decode()
assert commented.read_text() == source, commented.read_text()
print('unfixable: exit 1, E507 and a comment range both leave the file unchanged')
PY
}

check '46-a E517: diagnosis, rewrite, comments kept, execution' r46_condition_arms
check '46-b E510: written Cons chains become literals and run' r46_expression_lists
check '46-c E516: written patterns become literals and run' r46_pattern_lists
check '46-d check and build never write to a source file' r46_no_write
check '46-e a diagnostic without a local rewrite stops with exit 1' r46_stops_without_rewrite

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
