#!/usr/bin/env bash
# tier: full
# R55 `--print-type` を任意の `def` に広げる（`docs/rounds/r55.md`）。
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

# 入口ファイル。`answer` は値、`fail` は効果と型変数の残る値、`identity` は多相、
# `pure` は `Pure` を書いた多相、`twice` は効果多相、`reader` は効果行付きである。
# `lib::Util.helper` は契約が型を宣言する member である。
MAIN = (
    'def answer = 42\n'
    'def fail = raise { message = "boom" }\n'
    'def identity x => x\n'
    'def pure (x: Int): Int Pure => x\n'
    'def twice f x => f (f x)\n'
    'def reader (path: String): String {Error} => path\n'
    '\n'
    'def main = if 1 > 2 => fail | reader (Int.to_string (pure (lib::Util.helper (twice identity answer))))\n'
)
UTIL = 'library\nmodule Util\ndef helper(x: Int): Int Pure\n\nimplement Util as Util\ndef helper x => x\n'

def run(*args, cwd=None):
    return subprocess.run([HADO, *args], capture_output=True, cwd=cwd)

def project(directory, files):
    path = Path(directory)
    path.mkdir(parents=True, exist_ok=True)
    for name, text in files.items():
        target = path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
    return path

def root(directory):
    return project(directory, {'main.hd': MAIN, 'lib/util.hd': UTIL})

def printed(path, name=None):
    """The stdout of `check --print-type=<name>` for one definition."""
    flag = '--print-type' if name is None else f'--print-type={name}'
    result = run('check', flag, str(path))
    assert result.returncode == 0, (name, result.returncode, result.stderr)
    assert result.stderr == b'', (name, result.stderr)
    return result.stdout
PY

# --- 55-a: 値の def（§15 の nullary Fn は書けない形なので本体の型と効果を出す） ---
r55_value() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import printed, root

main = root(Path(sys.argv[1]) / 'a') / 'main.hd'
assert printed(main, 'answer') == b'Int {A}\n', printed(main, 'answer')
assert printed(main, 'fail') == b'A {Error | B}\n', printed(main, 'fail')
print('value: a value definition prints its type and its row')
PY
}

# --- 55-b: 多相の def の改名 ---
r55_polymorphic() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import printed, root

main = root(Path(sys.argv[1]) / 'b') / 'main.hd'
assert printed(main, 'identity') == b'Fn(A => A {B})\n', printed(main, 'identity')
print('polymorphic: quantified variables are renamed from A')
PY
}

# --- 55-c: 効果行と効果変数 ---
r55_effects() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import printed, root

directory = root(Path(sys.argv[1]) / 'c')
main = directory / 'main.hd'
assert printed(main, 'reader') == b'Fn(String => String {Error})\n', printed(main, 'reader')
assert printed(main, 'twice') == b'Fn(Fn(A => A {B}), A => A {B})\n', printed(main, 'twice')
# `Pure` を書いた def は行変数を残さない（§20）。
assert printed(main, 'pure') == b'Fn(Int => Int)\n', printed(main, 'pure')
print('effects: a concrete row, a row variable and Pure are told apart')
PY
}

# --- 55-d: 2 つの道が同じ型を出す ---
r55_two_ways() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import printed, project, run, root

# control: 値を付けない形は今までどおり `main` の型を出す。
plain = project(Path(sys.argv[1]) / 'd/plain', {'main.hd': 'def main = 42\n'})
result = run('check', '--print-type', str(plain / 'main.hd'))
assert result.returncode == 0, (result.returncode, result.stderr)
assert result.stdout == b'Int\n', result.stdout
assert result.stderr == b'', result.stderr

# 同じ main を 2 つの道で出した文字列が一致する。
directory = root(Path(sys.argv[1]) / 'd/two')
main = directory / 'main.hd'
assert printed(main, 'main') == printed(main), (printed(main, 'main'), printed(main))
print('two ways: --print-type=main and --print-type agree')
PY
}

# --- 55-e: 実装の member と見つからない名前 ---
r55_member() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import printed, root, run

directory = root(Path(sys.argv[1]) / 'e')
main = directory / 'main.hd'
# 契約が宣言する型を出す（実装の `def helper x => x` は `Fn(A => A {B})` である）。
assert printed(main, 'lib::Util.helper') == b'Fn(Int => Int)\n', printed(main, 'lib::Util.helper')

missing = run('check', '--print-type=nope', str(main))
assert missing.returncode == 1, (missing.returncode, missing.stdout)
assert missing.stdout == b'', missing.stdout
assert b'no definition named nope' in missing.stderr, missing.stderr
print('member: an implementation member is printed by name, an unknown name is an error')
PY
}

check '55-a a value definition prints its type' r55_value
check '55-b quantified variables are renamed from A' r55_polymorphic
check '55-c concrete and variable effect rows are printed' r55_effects
check '55-d the two ways to print main agree' r55_two_ways
check '55-e an implementation member is printed by name' r55_member

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
