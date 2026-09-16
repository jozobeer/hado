#!/usr/bin/env bash
# R59 未使用の効果と handler の E513（docs/plan.md 55 節）。
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


def position(text, needle, occurrence=1):
    """The 1-based (line, col) of the nth occurrence of `needle`."""
    index = -1
    for _ in range(occurrence):
        index = text.index(needle, index + 1)
    before = text[:index]
    return before.count('\n') + 1, index - (before.rfind('\n') + 1) + 1


def anchor(found):
    return found['path'], found['line'], found['col']


def build_run(root, name, files, expect):
    """Build the program and check the value `main` prints."""
    directory = write(root, name, files)
    build = subprocess.run([HADO, 'build', str(directory / 'main.hd')], capture_output=True)
    assert build.returncode == 0 and build.stderr == b'', (name, build.returncode, build.stderr.decode())
    result = subprocess.run([str(directory / 'main')], capture_output=True, cwd=directory, stdin=subprocess.DEVNULL)
    assert result.returncode == 0, (name, result.returncode, result.stderr.decode())
    assert result.stdout.decode() == expect + '\n', (name, result.stdout.decode(), expect)
PY

# --- 59-a: 未使用の効果 ---
r59_unused_effect() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, anchor, diagnostic, position, project

import random
rng = random.Random(59)
effect = 'Ef' + str(rng.randint(1, 9))
op = 'op' + str(rng.randint(1, 9))
pad = '\n' * rng.randint(0, 3) + ' ' * rng.randint(0, 4)
body = 'def main = 1\n'
source = f'{pad}effect {effect}\n{pad}once {op}: Int\n{body}'
root = Path(sys.argv[1]) / 'a'

accepted(project(root, 'clean', {'main.hd': body}, 'check'))
found = diagnostic(project(root, 'unused', {'main.hd': source}, 'check'), 'E513')
assert anchor(found) == ('main.hd', *position(source, f'effect {effect}')), found
print('unused effect: E513 at the head of the effect')
PY
}

# --- 59-b: 未使用の handler と、with で使う形 ---
r59_handler() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, build_run, diagnostic, position, project

import random
rng = random.Random(60)
effect = 'Ef' + str(rng.randint(1, 9))
op = 'op' + str(rng.randint(1, 9))
handler = 'H' + str(rng.randint(1, 9))
n = rng.randint(10, 99)
root = Path(sys.argv[1]) / 'b'

decls = (f'effect {effect}\n'
         f'once {op}: Int\n'
         f'handler {effect} as {handler}\n'
         f'once {op} => {n}\n')
source = 'def main = 1\n' + decls
found = diagnostic(project(root, 'unused', {'main.hd': source}, 'check'), 'E513')
assert anchor(found) == ('main.hd', *position(source, f'handler {effect} as {handler}')), found

used = decls + f'def main = with {handler} -> {op}\n'
build_run(root, 'with', {'main.hd': used}, str(n))
print('handler: unused is E513, used by with runs')
PY
}

# --- 59-c: 操作の参照と、操作の型にだけ現れる型 ---
r59_operation() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, anchor, build_run, diagnostic, position, project

import random
rng = random.Random(61)
effect = 'Ef' + str(rng.randint(1, 9))
op = 'op' + str(rng.randint(1, 9))
handler = 'H' + str(rng.randint(1, 9))
ty = 'T' + str(rng.randint(1, 9))
n = rng.randint(10, 99)
root = Path(sys.argv[1]) / 'c'

# (1) 到達できる def から操作を呼ぶ
decls = (f'effect {effect}\n'
         f'once {op}: Int\n'
         f'handler {effect} as {handler}\n'
         f'once {op} => {n}\n')
reached = decls + f'def f (x: Int): Int {{{effect}}} => x + {op}\ndef main = with {handler} -> f 0\n'
build_run(root, 'reached', {'main.hd': reached}, str(n))

# (2) 効果の操作の型にだけ現れる型は使われる（bug.md の 2 つ目の穴）
only = (f'type {ty} = | Mk(Int)\n'
        f'effect {effect}\n'
        f'once {op} (m: {ty}): Unit\n'
        f'handler {effect} as {handler}\n'
        f'once {op} _ =>\n'
        f'  with {handler} ->\n'
        f'  ()\n'
        f'def main = 1\n')
accepted(project(root, 'optype', {'main.hd': only}, 'check'))

# control: 効果を外すと、その型は未使用になる
control = f'type {ty} = | Mk(Int)\ndef main = ()\n'
found = diagnostic(project(root, 'optype-control', {'main.hd': control}, 'check'), 'E513')
assert anchor(found) == ('main.hd', *position(control, f'type {ty}')), found

# (3) 効果行に現れるだけでも効果は使われる（tests/reject/E522 が要求する）。
# 効果が使われていれば、報告されるのは行の重複の E522 であって E513 ではない。
row = (f'effect {effect}\n'
       f'once {op}: Int\n'
       f'def g (x: Int): Int {{{effect}, {effect}}} => x\n'
       f'def main = g 1\n')
found = diagnostic(project(root, 'row', {'main.hd': row}, 'check'), 'E522')
assert anchor(found) == ('main.hd', *position(row, effect, 3)), found
print('operation: a reference uses the effect, and its types come along')
PY
}

# --- 59-d: ライブラリの公開面 ---
r59_library() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, anchor, diagnostic, position, project

import random
rng = random.Random(62)
effect = 'Ef' + str(rng.randint(1, 9))
op = 'op' + str(rng.randint(1, 9))
handler = 'H' + str(rng.randint(1, 9))
n = rng.randint(10, 99)
root = Path(sys.argv[1]) / 'd'

decls = (f'effect {effect}\n'
         f'once {op}: Int\n'
         f'handler {effect} as {handler}\n'
         f'once {op} => {n}\n')
accepted(project(root, 'library', {'store.hd': 'library\n' + decls}, 'check', entry='store.hd'))

entry = decls + 'def main = 1\n'
found = diagnostic(project(root, 'entry', {'main.hd': entry}, 'check'), 'E513')
assert anchor(found) == ('main.hd', *position(entry, f'handler {effect} as {handler}')), found
print('library: the public surface keeps both, an entry file does not')
PY
}

# --- 59-e: prelude の免除 ---
r59_prelude() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import accepted, project

root = Path(sys.argv[1]) / 'e'
accepted(project(root, 'plain', {'main.hd': 'def main = 1\n'}, 'check'))
accepted(project(root, 'raise', {'main.hd': 'def main = raise { message = "stopped" }\n'}, 'check'))
print('prelude: the prelude effects stay exempt')
PY
}

# --- 59-f: 到達しない def から操作を呼んでも効果は使われない ---
r59_reachability() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, position, project

import random
rng = random.Random(63)
effect = 'Ef' + str(rng.randint(1, 9))
op = 'op' + str(rng.randint(1, 9))
root = Path(sys.argv[1]) / 'f'

source = (f'effect {effect}\n'
          f'once {op}: Int\n'
          f'def unused_f = {op}\n'
          f'def main = 1\n')
found = diagnostic(project(root, 'unreachable', {'main.hd': source}, 'check'), 'E513')
assert anchor(found) == ('main.hd', *position(source, f'effect {effect}')), found
print('reachability: an unreachable def does not save the effect')
PY
}

# --- 59-g: 1 件報告 ---
r59_one_report() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import anchor, diagnostic, position, project

import random
rng = random.Random(64)
first = 'Ef' + str(rng.randint(1, 9))
second = 'Eg' + str(rng.randint(1, 9))
root = Path(sys.argv[1]) / 'g'

source = (f'effect {first}\nonce first_op: Int\n'
          f'effect {second}\nonce second_op: Int\n'
          'def main = 1\n')
found = diagnostic(project(root, 'two', {'main.hd': source}, 'check'), 'E513')
assert anchor(found) == ('main.hd', *position(source, f'effect {first}')), found
print('one report: two unused effects still print one line')
PY
}

# --- 59-h: 書き換えを持たない ---
r59_untouched() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, source_of

root = Path(sys.argv[1]) / 'h'
source = 'effect Unused\nonce unused: Int\ndef main = 1\n'

result = project(root, 'fix', {'main.hd': source}, 'check', '--fix')
assert result.returncode == 1, (result.returncode, result.stderr.decode())
assert source_of(root, 'fix') == source, 'check --fix must not write'

result = project(root, 'fmt', {'main.hd': source}, 'fmt')
assert result.returncode == 0 and result.stderr == b'', (result.returncode, result.stderr.decode())
assert source_of(root, 'fmt') == source, 'fmt must not write'
print('untouched: E513 has no rewrite')
PY
}

# --- 59-i: 仕様と層と lock ---
r59_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert '操作を参照するか' in spec, '§37 does not say what using an effect means'
rows = {line.split('|')[1].strip(): line for line in spec.splitlines() if line.startswith('| E')}
assert 'handler' in rows['E513'], rows['E513']
tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r59' in full.split(), 'r59 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert 'verify-r59' in trace, 'spec-trace does not name verify-r59'
bug = Path('docs/bug.md').read_text()
assert '未使用の効果に E513 が出ない' not in bug, 'bug.md still records the unused effect'
print('docs: §37, the error table, the trace rows, the tier and bug.md')
PY
}

r59_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '59-a an unused effect is E513' r59_unused_effect
check '59-b an unused handler is E513, with runs' r59_handler
check '59-c an operation reference and its types' r59_operation
check '59-d the library surface keeps effects and handlers' r59_library
check '59-e the prelude stays exempt' r59_prelude
check '59-f an unreachable def does not save the effect' r59_reachability
check '59-g one report per run' r59_one_report
check '59-h E513 has no rewrite' r59_untouched
check '59-i spec, trace, tier, bug' r59_docs
check '59-i corpus lock' r59_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
