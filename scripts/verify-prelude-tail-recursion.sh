#!/usr/bin/env bash
# tier: core
# round: prelude-tail-recursion
# prelude の非末尾再帰を末尾形にしたことを、スタックを尽くす深さで測る門番
# （docs/rounds/prelude-tail-recursion.md の判定）。
#
# 深さはスタック上限で決める。既定の 8 MB のままだと非末尾の形が 15 万件まで
# 粘り、String.split の 100 万件は O(n^2) で分単位になる。`ulimit -s 512` の
# 下では非末尾が 1 万件で尽きるので、深さの測定には 3 万件を使う。3 万件が
# 尽きる深さであることは、同じ 512 KB で非末尾の probe（リスト形と String 形）
# が `hado: stack overflow` で終了コード 1 になることで先に確かめる。
# 100 万件は List・Array・Bytes の正しさに使う（末尾形なので 512 KB でも通る）。
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
import subprocess
from pathlib import Path

HADO = None
STACK_KB = 512
DEPTH = 30000
# Env.args は argv の上限（RLIMIT_STACK/4）が深さを決める。512 KB では 9000 件で
# exec 自体が E2BIG になり、直す前の実装の 1 万 2000 件に届かない。4 MB と 8 万件
# なら exec でき、直す前の実装はそこで `hado: stack overflow` になる
# （docs/rounds/prelude-tail-recursion.md の実測）。
ARG_STACK_KB = 4096
ARG_DEPTH = 80000
OVERFLOW = b'hado: stack overflow\n'
BUILD = 'def rec build_go n acc => if n <= 0 => acc | build_go (n - 1) [n, ...acc]\n'


def build(root, name, source):
    """Generate a program, build it to a private path, and return the binary."""
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    source_path = directory / 'main.hd'
    source_path.write_text(source)
    binary = directory / 'program'
    done = subprocess.run([HADO, 'build', '-o', str(binary), str(source_path)], capture_output=True)
    assert done.returncode == 0, (name, done.stderr.decode())
    return binary


def run(binary, stack_kb=STACK_KB, args=()):
    """Run with RLIMIT_STACK set, so the depth is the harness's choice, not the shell's."""
    return subprocess.run(
        ['bash', '-c', f'ulimit -s {stack_kb}; exec "$@"', '_', str(binary), *args],
        capture_output=True, timeout=600)


def expect(binary, stdout, code=0, stderr=b'', stack_kb=STACK_KB, args=()):
    done = run(binary, stack_kb, args)
    assert done.returncode == code, (binary, done.returncode, done.stderr.decode())
    assert done.stdout == stdout, (binary, done.stdout, stdout)
    assert done.stderr == stderr, (binary, done.stderr, stderr)


def value(root, name, expression, expected, prelude=BUILD):
    binary = build(root, name, prelude + f'def main = {expression}')
    expect(binary, f'{expected}\n'.encode())


def control(root):
    """The comparisons must reject a wrong exit code, a wrong stdout, and a wrong stderr."""
    binary = build(root, 'control', BUILD + 'def main = List.length (build_go 100 [])')
    for kwargs in (dict(stdout=b'99\n'), dict(stdout=b'100\n', code=1),
                   dict(stdout=b'100\n', stderr=b'x\n')):
        try:
            expect(binary, **kwargs)
        except AssertionError:
            pass
        else:
            raise AssertionError(f'control accepted a wrong expectation: {kwargs}')
PY

# --- a: 深さがスタックを尽くすこと（赤の探針）---
ptr_depth() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import BUILD, DEPTH, OVERFLOW, build, expect

root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
list_shape = build(root, 'red_list',
                   'def rec map_old values f => if values | [] => [] | [head, ...tail] => [f head, ...map_old tail f]\n'
                   + BUILD + f'def main = List.length (map_old (build_go {DEPTH} []) fn(x => x))')
expect(list_shape, b'', code=1, stderr=OVERFLOW)
string_shape = build(root, 'red_string',
                     f'def rec walk s start count => if start >= count => [] | [s, ...walk s (start + 1) count]\n'
                     f'def main = List.length (walk "0123456789" 0 {DEPTH})')
expect(string_shape, b'', code=1, stderr=OVERFLOW)
print(f'depth: a list-shaped and a String-shaped non-tail recursion at {DEPTH} exhaust a 512 KB stack')
PY
}

# --- b: List の 100 万件が末尾形で通ること ---
ptr_list() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import value

root = Path(sys.argv[1]) / 'b'
root.mkdir(parents=True, exist_ok=True)
value(root, 'map', 'List.fold (List.map (build_go 1000000 []) fn(x => x + 1)) 0 fn(a, x => a + x)', 500001500000)
value(root, 'filter', 'List.fold (List.filter (build_go 1000000 []) fn(x => x > 500000)) 0 fn(a, x => a + x)', 375000250000)
value(root, 'length', 'List.length (build_go 1000000 [])', 1000000)
value(root, 'take', 'List.fold (List.take (build_go 1000000 []) 1000000) 0 fn(a, x => a + x)', 500000500000)
value(root, 'append', 'List.fold (List.append (build_go 500000 []) (build_go 500000 [])) 0 fn(a, x => a + x)', 250000500000)
print('list: map/filter/length/take/append at 1000000 elements')
PY
}

# --- c: sort が 100 万件を並べ替えること（再帰の深さは log n）---
ptr_sort() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import BUILD, build, expect

root = Path(sys.argv[1]) / 'c'
root.mkdir(parents=True, exist_ok=True)
source = BUILD + '''def main =
  List.sort (List.reverse (build_go 1000000 [])) fn(a, b => a - b) as sorted ->
  [List.fold sorted 0 fn(a, x => a + x), List.fold sorted 0 fn(a, x => (a * 31 + x) % 1000000007)]
'''
binary = build(root, 'sort', source)
order = 0
for x in range(1, 1000001):
    order = (order * 31 + x) % 1000000007
expect(binary, f'[500000500000, {order}]\n'.encode())
print('sort: 1000000 reversed elements sort to 1..1000000')
PY
}

# --- d: Array と Bytes の 100 万件 ---
ptr_array_bytes() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import value

root = Path(sys.argv[1]) / 'd'
root.mkdir(parents=True, exist_ok=True)
value(root, 'array', 'List.fold (Array.to_list (Array.of_list (build_go 1000000 []))) 0 fn(a, x => a + x)', 500000500000)
value(root, 'array_set', 'Array.get (Array.set (Array.of_list (build_go 1000000 [])) 999999 42) 999999', 42)
value(root, 'bytes', 'List.length (Bytes.to_list (Bytes.of_list (bytes_go 1000000 [])))', 1000000,
      prelude='def rec bytes_go n acc => if n <= 0 => acc | bytes_go (n - 1) [n % 256, ...acc]\n')
print('array: to_list/set at 1000000 elements, bytes: to_list at 1000000 bytes')
PY
}

# --- e: String の深さ（split は O(n^2) なので 3 万件）---
ptr_string() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import BUILD, DEPTH, value

root = Path(sys.argv[1]) / 'e'
root.mkdir(parents=True, exist_ok=True)
commas = root / 'commas.txt'
commas.write_text(',' * DEPTH)
value(root, 'split', f'List.length (read_file "{commas}" |> Bytes.to_string |> String.split ",")', DEPTH + 1, prelude='')
value(root, 'join', f'String.length (String.join (List.map (build_go {DEPTH} []) fn(_ => "a")) ",")', 2 * DEPTH - 1)
print(f'string: split/join at {DEPTH} parts')
PY
}

# --- f: Env と Io の深さ ---
ptr_env_io() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import ARG_DEPTH, ARG_STACK_KB, DEPTH, build, expect, value

root = Path(sys.argv[1]) / 'f'
root.mkdir(parents=True, exist_ok=True)
entries = root / 'entries'
entries.mkdir(parents=True, exist_ok=True)
for i in range(DEPTH):
    (entries / f'e{i:05d}').touch()
value(root, 'read_dir', f'List.length (read_dir "{entries}")', DEPTH, prelude='')
args = build(root, 'args', 'def main = List.length args')
expect(args, f'{ARG_DEPTH}\n'.encode(), stack_kb=ARG_STACK_KB, args=['a'] * ARG_DEPTH)

# 効果を持つ呼び出しを `|>` で書くと、末尾呼び出しの引数のリストで単相化が
# `list element type mismatch` になる（docs/issues/pipe-in-tail-argument.md）。
# handler の下で両方の公開経路が単相化を通ることを測る。
caught = 'handler Error as H\nabort raise e => e.message\nreturn _ => "ok"\ndef main = with H -> '
expect(build(root, 'caught_dir', caught + f'read_dir "{root}/nope"'), b'NotFound\n')
expect(build(root, 'caught_args', caught + 'List.length args'), b'ok\n')
print(f'env/io: read_dir at {DEPTH} entries, args at {ARG_DEPTH}, both under a handler')
PY
}

# --- g: 判定器そのものの較正 ---
ptr_control() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import control

root = Path(sys.argv[1]) / 'g'
root.mkdir(parents=True, exist_ok=True)
control(root)
print('control: a wrong exit code, stdout, or stderr is rejected')
PY
}

# --- h: 仕様とコーパス ---
ptr_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert '`map` は関数を、`filter` は述語を、先頭の要素から順に適用する。' in spec, 'spec has no application order'
assert '比較関数の型は `Pure` とする' in spec, 'spec does not make the comparator Pure'
assert '`List.length` `reverse` `append` `sort` `head`' in spec, 'sort is not in the Pure list'
stdlib = Path('stdlib/list.hd').read_text()
assert 'comparator: Fn(A, A => Int Pure)): A List Pure' in stdlib, 'List.sort is not Pure in stdlib'
for case in ('tests/accept/list_map_head_order/main.hd', 'tests/accept/list_filter_head_order/main.hd'):
    assert Path(case).exists(), case
print('docs: §55 has the application order and the Pure comparator, and the corpus cases exist')
PY
}

check 'a depth: non-tail probes overflow a 512 KB stack' ptr_depth
check 'b list: 1000000 elements' ptr_list
check 'c sort: 1000000 elements' ptr_sort
check 'd array and bytes: 1000000 elements' ptr_array_bytes
check 'e string: split and join at the depth' ptr_string
check 'f env and io: args and read_dir at the depth' ptr_env_io
check 'g control: wrong expectations are rejected' ptr_control
check 'h docs: spec and corpus' ptr_docs

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
