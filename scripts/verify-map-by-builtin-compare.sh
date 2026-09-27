#!/usr/bin/env bash
# tier: full
# round: map-by-builtin-compare
# 連想配列 `Map` を組み込み `compare` の上に置いたことを測る門番
# （docs/rounds/map-by-builtin-compare.md の判定）。
#
# full にする理由: 検査 d がテストビルドの実測で、鍵の数だけ `Map.insert` を
# 並べたソースを作って実行ファイルを組み、検査 f・h は 20 万語の入力に対して
# 実行ファイルを warm-up 1 回 + 5 回ずつ走らせる。quick / core ではこの費用に
# 見合う変更が入らない。
#
# 検査と issue の完了条件の対応:
#   a 公開面          → 条件 4（内部 constructor は外から使えない、E204）
#   b 空と不在        → 条件 2（find が None、remove が no-op）
#   c fold の呼び出し順 → 条件 3（非可換の累算器、キーの昇順）
#   d compare 回数の上限 → 条件 4（赤黒木であることの観測）
#   e 公開面だけで順序が保たれる → 条件 4
#   f 頻度の独立照合   → 条件 5
#   g §55 の member と昇順 → 条件 1
#   h 頻度の速さ       → 条件 5
#   i 重複鍵の畳み     → 条件 6
#
# 判定器に暗記を持ち込まない: 期待値は毎回この門番の中で計算する。件数は
# §55 の関数集合と §56 の表から導く。コーパスのファイル名や行は埋め込まない。
#
# 時間の比は docs/issues/flaky-string-split-timing-gate.md の罠を踏まない:
# 対照を引いた中央値を使い、比が負や下限割れでも速さの下限では赤にしない。
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
hado="target/debug/hado"
hado_abs="$(cd "$(dirname "$hado")" && pwd)/$(basename "$hado")"
root="$(pwd)"

# --- 共有点: 派生 compare の primitive 呼び出しを数える道具 ---
#
# `crates/hado/src/mono.rs:5104` の `derive_scalar_compare` は、キーが String の
# とき `<` と `==` を直接 emit する。この 2 つは `crates/hado/src/emit.rs` から
# `hado_string_compare`（`runtime/hado_runtime.c:382`）を呼ぶ。`hado` 自身の
# リンクは `crates/hado/src/lib.rs` の `run_cc` が `cc <obj> <archive> -lgc -o <exe>`
# の形で行うので、`cc` を包んでこの呼び出しだけに `--wrap` を足す。
#
# `--wrap` は同名の定義と衝突するので、`hado_runtime.a` をアーカイブのまま
# 後ろに置くとリンカが wrap を無視する（実測）。代わりにアーカイブを展開して
# `hado_string_compare` のオブジェクトだけ落とし、残りを包む。
counter_dir="$workdir/counter"
mkdir -p "$counter_dir"
cat >"$counter_dir/counter.c" <<'C'
#include <stdio.h>
#include <stdint.h>

extern int64_t __real_hado_string_compare(const void *left, const void *right);

static unsigned long long hado_string_compare_calls = 0;

/* 計器は呼び出し 1 回ごとに 1 増やす（memcmp のバイト数では数えない）。 */
int64_t __wrap_hado_string_compare(const void *left, const void *right) {
  ++hado_string_compare_calls;
  return __real_hado_string_compare(left, right);
}

__attribute__((destructor)) static void hado_report_string_compare_calls(void) {
  fprintf(stderr, "HADO_STRING_COMPARE_CALLS=%llu\n", hado_string_compare_calls);
}
C
cc -c -O2 "$counter_dir/counter.c" -o "$counter_dir/counter.o" || exit 1

cat >"$counter_dir/cc" <<EOF
#!/usr/bin/env bash
# hado のリンク（アーカイブ名に hado_runtime.a が現れるとき）だけを包む。
if [[ " \$* " == *hado_runtime.a* ]]; then
  obj="\$1"; shift
  exec /usr/bin/cc "\$obj" "$counter_dir/counter.o" "\$@" -Wl,--wrap=hado_string_compare
fi
exec /usr/bin/cc "\$@"
EOF
chmod +x "$counter_dir/cc"

# --- 共有点: 鍵の列と、判定器が独立に計算する上限 ---
python3 - "$workdir" <<'PY'
import sys
from pathlib import Path

workdir = Path(sys.argv[1])

# String の compare 順で単調な昇順鍵。同じ桁に 0 埋めする（十進の "1" "2" "10"
# は String 順でないので使わない）。n は 2 の冪から 1 を引いた形にして、
# 素の二分探索木の最後の insert の primitive 回数が上限 4 * floor(log2(n + 1))
# を超える大きさ（n = 4095）を含める。
def keys(n):
    width = len(str(n - 1))
    return [str(i).zfill(width) for i in range(n)]


def bound(n):
    return 4 * ((n + 1).bit_length() - 1)


sizes = [255, 1023]
for n in sizes:
    (workdir / f'keys-{n}.txt').write_text('\n'.join(keys(n)) + '\n')
(workdir / 'bounds.json').write_text(
    '[' + ', '.join(f'{{"n": {n}, "bound": {bound(n)}}}' for n in sizes) + ']')
# 素の二分探索木を昇順に挿入したときの最後の insert の比較回数。
for n in sizes:
    print(f'plain bst last insert for n={n}: {n - 1} comparisons, '
          f'Map bound is {bound(n)}')
PY

# --- a: 公開面（内部 constructor は外から使えない。§47・E204）---
public_surface() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'a'
workdir.mkdir(parents=True, exist_ok=True)
hado = sys.argv[2]


def check(name, source):
    # 同じディレクトリに複数の `.hd` を置くと 1 つのプロジェクトとして読まれる。
    # 別の probe の誤りが混ざらないよう、1 本ずつ自分のディレクトリに置く。
    directory = workdir / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    done = subprocess.run([hado, 'check', str(path)], capture_output=True, text=True)
    return done.returncode, done.stdout + done.stderr


# §55 の関数集合と §47 の規則から、外から見える member を導く。内部の
# constructor と内部の def はどれも外から使えない。
section = Path('docs/spec.md').read_text()
start = section.index('## 55. prelude')
end = section.index('\n## 56. ', start)
block = section[start:end]
inventory = block[block.index('関数集合は次とする。'):]
inventory = inventory[inventory.index('```text') + len('```text'):]
inventory = inventory[:inventory.index('```')]
members = []
for line in inventory.splitlines():
    if not line.strip():
        continue
    names, functions = line.split(':', 1)
    for name in names.split('/'):
        members.extend(f'{name.strip()}.{member}' for member in functions.split())
map_members = sorted(m for m in members if m.startswith('Map.'))
assert map_members == [
    'Map.empty', 'Map.find', 'Map.fold', 'Map.insert', 'Map.remove', 'Map.to_list',
], map_members
# `MapEntry` は §55 が外に置く型として名指しする。`to_list` の要素を作る。
assert 'type (K, V) MapEntry' in block, 'MapEntry must be declared outside Map'

# 内部の名前は `stdlib/map.hd` の実装から列挙する。判定器はファイルの行を
# 名指ししないが、内部名を門番に書き写すと実装を変えたときに黙ってずれる。
# 実装の `implement Map as Map` の中の constructor と def をそのまま使う。
source = Path('stdlib/map.hd').read_text()
body = source[source.index('implement Map as Map'):]
internal_ctors = []
internal_defs = []
internal_types = []
for line in body.splitlines():
    line = line.rstrip()
    if line.startswith('type ') and '|' in line:
        # `type MapColor = | MapRed | MapBlack` と
        # `type (K, V) T = | MapLeaf | MapNode(...)` の両方から、型名と
        # constructor 名を取る。
        head, arms = line.split('=', 1)
        declared = head[len('type '):].strip()
        if declared.startswith('('):
            declared = declared[declared.index(')') + 1:].strip()
        internal_types.append(declared)
        for arm in arms.split('|'):
            name = arm.strip().split('(')[0].strip()
            if name:
                internal_ctors.append(name)
    elif line.startswith('type '):
        # constructor を持たない抽象型（`type (K, V) T`）は契約側にある。
        pass
    elif line.startswith('def rec '):
        internal_defs.append(line[len('def rec '):].split()[0])
    elif line.startswith('def '):
        internal_defs.append(line[len('def '):].split()[0])
assert internal_ctors, 'no internal constructors found'
assert internal_defs, 'no internal definitions found'
assert internal_types, 'no internal types found'
# 内部名は公開 member と重ならない（`empty` と `insert` などは実装側にもある）。
external = {m.split('.')[1] for m in map_members}
internal_ctors = sorted(set(internal_ctors))
internal_defs = sorted(set(internal_defs) - external)
assert not (set(internal_ctors) & external), 'internal constructor shares a public name'

# 外から内部 constructor を構築する。
for name in internal_ctors:
    code, text = check(f'ctor_{name}', f'def main = {name}\n')
    assert code != 0, f'{name} is constructible from outside'
    assert 'E204' in text, (name, text)
    # 公開 member の名前空間（`Map.`）からも届かない。
    code, text = check(f'qualified_{name}', f'def main = Map.{name}\n')
    assert code != 0 and ('E204' in text or 'E100' in text), (name, text)

# 外から内部 def を呼ぶ。E204（module member is not exported）。
for name in internal_defs:
    code, text = check(f'def_{name}', f'def main = Map.{name} Map.empty\n')
    assert code != 0, f'Map.{name} is callable from outside'
    assert 'E204' in text, (name, text)

# 内部型の名前が利用者の名前と衝突しないこと。`implement` の中の `type` も
# root の名前空間に出る（crates/hado/src/namespaces.rs の E205 は prelude の
# 名前も見る）ので、`Color` のような普通の名前を置くと、利用者が同じ名前の
# 型を宣言したときに E205 になる。prelude は名前空間を借りている側なので、
# 内部型には `Map` の前置きを付ける。契約が既に名指しする型（抽象型 `T`）は
# 契約側の名前なので除く。
contract_text = source[source.index('module Map'):source.index('implement Map as Map')]
contract_names = set()
for line in contract_text.splitlines():
    line = line.strip()
    if line.startswith('type '):
        declared = line[len('type '):].strip()
        if declared.startswith('('):
            declared = declared[declared.index(')') + 1:].strip()
        contract_names.add(declared)
for name in sorted((set(internal_ctors) | set(internal_types)) - contract_names):
    assert name.startswith('Map'), f'internal type name {name} is not prefixed with Map'

# 公開 member は全部外から見える。`MapEntry` も外の型である。
# 契約（`module Map` から `implement Map as Map` まで）の `def` の名前が §55 の
# 関数集合の `Map.` の行と一致し、内側の constructor が出ていないことを見る。
contract = source[source.index('module Map'):source.index('implement Map as Map')]
contract_defs = []
for line in contract.splitlines():
    line = line.strip()
    match = re.match(r'def\s+(?:\([^)]*\)\s+)?(\w+)', line)
    if match:
        contract_defs.append(match.group(1))
assert sorted(contract_defs) == sorted(external), (contract_defs, sorted(external))
assert 'MapEntry' in source
print(f'public surface: {len(map_members)} members reachable, '
      f'{len(internal_ctors)} internal constructors and {len(internal_defs)} internal defs rejected (E204)')
PY
}

# --- b: 空の写像と不在の鍵（条件 2）---
empty_and_absent() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'b'
workdir.mkdir(parents=True, exist_ok=True)
hado = sys.argv[2]


def build(name, source):
    directory = workdir / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    binary = directory / 'program'
    done = subprocess.run([hado, 'build', '-o', str(binary), str(path)],
                          capture_output=True, text=True)
    assert done.returncode == 0, (name, done.stderr[:600])
    return binary


def run(name, source):
    done = subprocess.run([str(build(name, source))], capture_output=True, text=True)
    assert done.returncode == 0, (name, done.returncode, done.stderr[:400])
    return done.stdout.strip()


# 空の写像。find は None、remove は to_list を変えない。
empty = run('empty', 'def main =\n'
                     '  let m = Map.empty ->\n'
                     '  let removed = Map.remove m "k" ->\n'
                     '  [ Option.unwrap_or (Map.find m "k") "none",\n'
                     '    Int.to_string (List.length (Map.to_list m)),\n'
                     '    Int.to_string (List.length (Map.to_list removed)) ]')
assert empty == '["none", "0", "0"]', empty

# 鍵を持つ写像に無い鍵。find は None、remove は同じ写像を返す。
absent = run('absent', 'def main =\n'
                       '  let m = Map.insert (Map.insert Map.empty "b" "2") "a" "1" ->\n'
                       '  let removed = Map.remove m "zzz" ->\n'
                       '  [ Option.unwrap_or (Map.find m "zzz") "none",\n'
                       '    Option.unwrap_or (Map.find removed "a") "gone",\n'
                       '    Int.to_string (List.length (Map.to_list m)),\n'
                       '    Int.to_string (List.length (Map.to_list removed)) ]')
assert absent == '["none", "1", "2", "2"]', absent

# 例外にしたり別の値にしたりする実装は、ここで `none` 以外を返して落ちる。
# 空の写像でだけ通る実装（`empty` を特別扱いする）は、鍵を持つ写像の
# 不在で落ちる。両方を測るので、片方だけでは通らない。
print('empty and absent: find is None and remove is a no-op for both the empty map '
      'and a missing key in a non-empty map')
PY
}

# --- c: fold の呼び出し順（条件 3）---
fold_order() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'c'
workdir.mkdir(parents=True, exist_ok=True)
hado = sys.argv[2]


def build(name, source):
    directory = workdir / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    binary = directory / 'program'
    done = subprocess.run([hado, 'build', '-o', str(binary), str(path)],
                          capture_output=True, text=True)
    assert done.returncode == 0, (name, done.stderr[:800])
    return binary


def run(name, source):
    done = subprocess.run([str(build(name, source))], capture_output=True, text=True)
    assert done.returncode == 0, (name, done.returncode, done.stderr[:400])
    return done.stdout.strip()


# 挿入順は昇順でない 3 件以上の相異なる鍵。判定器が `compare` により独立に
# 並べた列が期待値である（この門番の判定器が自分で並べる: String の昇順）。
insert_order = ['3', '1', '2', '0005', '0004']
expected = sorted(insert_order)

# 非可換の累算器。区切り `<` を挟んで訪問した鍵を積む。累算器の値そのものが
# 呼び出し順の証拠で、`Map.fold` の戻り値を `List.reverse` しても昇順の列には
# 戻らない（文字列を逆にすると `1<2<3<` になり `<1<2<3` にならない）。
# 効果の列（累算器の中で `log` する）は使わない: `Map.fold` の署名は抽象型
# `(K, V) T` を含むため効果行が一般化せず、効果を持つ累算器を渡すと E706 に
# なる（docs/rounds/map-by-builtin-compare.md の実測と仕様への疑義を参照）。
witness = 'def witness m => Map.fold m "" fn(acc, key, _ => String.concat acc (String.concat "<" key))\n'
source = witness + 'def main =\n'
source += '  let m = Map.empty ->\n'
for key in insert_order:
    source += f'  let m = Map.insert m "{key}" 0 ->\n'
source += '  IoDefault.log (witness m)\n'
got = run('order', source)
assert got == ''.join('<' + key for key in expected), (got, expected)

# `to_list` との一致だけでは通らない: 同じ写像の `to_list` は昇順なので、
# 降順に呼んで reverse する実装でも `to_list` は昇順になる。`fold` の
# 訪問列だけが降順を露わにする。ここでは `fold` の訪問列そのものを測る。
to_list = run('list', 'def main =\n'
                      '  let m = Map.insert (Map.insert (Map.insert Map.empty "3" 0) "1" 0) "2" 0 ->\n'
                      '  List.length (Map.to_list m)')
assert to_list == '3', to_list

# 空や 1 件では測らない（issue の完了条件）。3 件以上を測っていることを
# 判定器の中で確かめる。
assert len(insert_order) >= 3
print(f'fold order: the accumulator builds "{got}", which matches the independently '
      f'sorted keys {expected} for a non-ascending insertion order')
PY
}

# --- d: compare の primitive 呼び出し回数（条件 4）---
compare_counts() {
    python3 - "$workdir" "$hado_abs" "$counter_dir" "$root" <<'PY'
import json
import os
import random
import re
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'd'
workdir.mkdir(parents=True, exist_ok=True)
hado, counter = sys.argv[2], sys.argv[3]
bounds = {entry['n']: entry['bound']
          for entry in json.loads((Path(sys.argv[1]) / 'bounds.json').read_text())}
env = dict(os.environ, PATH=f'{counter}:{os.environ["PATH"]}')


def build(name, source):
    directory = workdir / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    binary = directory / 'program'
    done = subprocess.run([hado, 'build', '-o', str(binary), str(path)],
                          capture_output=True, text=True, env=env)
    assert done.returncode == 0, (name, done.stderr[:1500])
    return binary


def cumulative(binary):
    """計器はプロセスの終了時に累計を 1 回出す。プログラムごとに別プロセスで測る。"""
    done = subprocess.run([str(binary)], capture_output=True, text=True)
    assert done.returncode == 0, (binary, done.returncode, done.stderr[:400])
    found = re.findall(r'HADO_STRING_COMPARE_CALLS=(\d+)', done.stderr)
    assert len(found) == 1, (binary, done.stderr[-300:])
    return int(found[0])


def load_prefix(keys):
    """鍵を先頭から順に挿入する。モードで分岐しない。

    String の `==` や String の腕の照合はそれ自体が `hado_string_compare` を
    呼ぶ。argv でモードを選ぶと腕ごとに比較回数が変わり、増分に腕の差が混ざる
    （実測で 1 のずれが出た）。1 つの操作を 1 本のプログラムにして、腕を無くす。"""
    literal = ', '.join(f'"{key}"' for key in keys)
    return ('def rec load keys m =>\n'
            '  if keys\n'
            '  | [] => m\n'
            '  | [key, ...rest] => load rest (Map.insert m key "v")\n'
            'def main =\n'
            f'  let m = load [{literal}] Map.empty ->\n')


def base_source(keys):
    """n 件を入れて、長さを出す。find / remove の基準。"""
    return load_prefix(keys) + '  IoDefault.log (Int.to_string (List.length (Map.to_list m)))\n'


def last_insert_source(keys):
    """n - 1 件を入れた木に最後の 1 件を足す。基準は n - 1 件の木。"""
    return (load_prefix(keys[:-1])
            + f'  let grown = Map.insert m "{keys[-1]}" "v" ->\n'
            + '  IoDefault.log (Int.to_string (List.length (Map.to_list grown)))\n')


def find_source(keys, probe):
    return (load_prefix(keys)
            + f'  IoDefault.log (Option.unwrap_or (Map.find m "{probe}") "none")\n')


def remove_source(keys, probe):
    return (load_prefix(keys)
            + f'  let removed = Map.remove m "{probe}" ->\n'
            + '  IoDefault.log (Int.to_string (List.length (Map.to_list removed)))\n')


def search_primitives(root_key_and_children, key):
    """素の二分探索木の探索で `<` と `==` が何回呼ばれるか。`<` は各節で 1 回、
    `<` が偽のとき `==` が 1 回（1 回の派生 compare は 1 か 2 の primitive）。"""
    count = 0
    node = root_key_and_children
    while node is not None:
        count += 1
        if key < node[0]:
            node = node[1]
        else:
            count += 1
            if key == node[0]:
                return count
            node = node[2]
    return count


def plain_bst_last_insert(keys):
    """素の二分探索木（平衡化なし）に `keys` を順に入れたときの最後の insert の
    primitive 回数。判定器が独立に計算する（門番の中の定数にしない）。"""
    root = None
    for key in keys[:-1]:
        if root is None:
            root = [key, None, None]
            continue
        node = root
        while True:
            if key < node[0]:
                if node[1] is None:
                    node[1] = [key, None, None]
                    break
                node = node[1]
            elif key == node[0]:
                break
            else:
                if node[2] is None:
                    node[2] = [key, None, None]
                    break
                node = node[2]
    return search_primitives(root, keys[-1])


orders = {
    'ascending': lambda keys: keys,
    'descending': lambda keys: list(reversed(keys)),
    'random': lambda keys: random.Random(20260927).sample(keys, len(keys)),
}
# 昇順は小さい n と、素の二分探索木の最後の insert が上限を超える大きさ。
# 降順とランダムはその大きさだけを測る。
plan = [('ascending', 255), ('ascending', 1023), ('descending', 1023), ('random', 1023)]

for order, n in plan:
    limit = bounds[n]
    keys = (Path(sys.argv[1]) / f'keys-{n}.txt').read_text().split()
    assert keys == sorted(keys) and keys[0] < keys[-1], 'keys must be ascending'
    sequence = orders[order](keys)
    assert len(set(sequence)) == n
    if order != 'ascending':
        assert sequence != keys, order
    probe = sequence[-1]
    # 素の二分探索木の最後の insert の primitive 回数。昇順の最大 n では上限を
    # 超えることを判定器が確かめる（この大きさを含める理由）。
    plain = plain_bst_last_insert(sequence)
    if order == 'ascending' and n == max(size for kind, size in plan if kind == 'ascending'):
        assert plain > limit, (order, n, plain, limit)
    # n - 1 件の木（insert の基準）と n 件の木（find / remove の基準）。
    base = cumulative(build(f'{order}{n}_base', base_source(sequence[:-1])))
    inserted = cumulative(build(f'{order}{n}_insert', last_insert_source(sequence)))
    state = cumulative(build(f'{order}{n}_state', base_source(sequence)))
    # n - 1 件 + 1 件と n 件は同じ木なので、比較回数も同じでなければならない。
    assert inserted == state, (order, n, inserted, state)
    last_insert = state - base
    assert 0 < last_insert <= limit, (order, n, 'insert', last_insert, limit)
    increments = {'insert': last_insert}
    for mode, source in (('find', find_source(sequence, probe)),
                         ('remove', remove_source(sequence, probe))):
        measured = cumulative(build(f'{order}{n}_{mode}', source))
        increments[mode] = measured - state
        assert 0 < increments[mode] <= limit, (order, n, mode, increments[mode], limit)
    print(f'{order} n={n}: last insert {last_insert}, find {increments["find"]}, '
          f'remove {increments["remove"]} (limit {limit}; plain BST last insert {plain})')
PY
}

# --- e: 公開 member だけで挿入・削除後も順序が保たれる（条件 4）---
order_preserved() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import random
import re
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'e'
workdir.mkdir(parents=True, exist_ok=True)
hado = sys.argv[2]

# 判定器が入力を組み立て、期待値も自分で計算する。コーパスのファイル名や行は
# 使わない。公開 member（`empty` `insert` `remove` `to_list`）だけを使う。
rng = random.Random(20260927)
alphabet = [f'{i:03d}' for i in range(60)]
operations = []
for _ in range(400):
    key = rng.choice(alphabet)
    operations.append(('insert', key) if rng.random() < 0.6 else ('remove', key))


def expected(operations):
    model = {}
    for kind, key in operations:
        if kind == 'insert':
            model[key] = 'v'
        else:
            model.pop(key, None)
    return sorted(model)


source = ['def main =', '  let m0 = Map.empty ->']
name = 'm0'
for index, (kind, key) in enumerate(operations, start=1):
    nxt = f'm{index}'
    call = 'Map.insert' if kind == 'insert' else 'Map.remove'
    extra = f' "{key}" "v"' if kind == 'insert' else f' "{key}"'
    source.append(f'  let {nxt} = {call} {name}{extra} ->')
    name = nxt
source.append(f'  Map.to_list {name}')
path = workdir / 'main.hd'
path.write_text('\n'.join(source) + '\n')
binary = workdir / 'program'
done = subprocess.run([hado, 'build', '-o', str(binary), str(path)],
                      capture_output=True, text=True)
assert done.returncode == 0, done.stderr[:1500]
seen = subprocess.run([str(binary)], capture_output=True, text=True)
assert seen.returncode == 0, (seen.returncode, seen.stderr[:400])
want = expected(operations)
got = re.findall(r'MapEntry\("([^"]*)"', seen.stdout)
assert got == want, (got[:20], want[:20], len(got), len(want))
print(f'order: {len(operations)} insert/remove operations through the public members '
      f'leave {len(want)} keys in compare order')
PY
}

# --- f: 単語頻度の独立照合（条件 5）---
word_freq() {
    python3 - "$workdir" "$hado_abs" "$root" <<'PY'
import json
import re
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'f'
workdir.mkdir(parents=True, exist_ok=True)
hado, root = sys.argv[2], Path(sys.argv[3])
tools = root / 'tools' / 'wordfreq'

input_path = workdir / 'words.txt'
subprocess.run(['bash', str(tools / 'generate-input.sh'), str(input_path)], check=True)
words = input_path.read_text().split()
assert len(words) == 200000, len(words)
assert len(set(words)) == 500, len(set(words))
# 入れた生成手順で再生成してバイト一致させる。手順は決定的で、2 回目の走りも
# 1 回目と同じバイト列を返し、リポジトリに入れた digest とも一致する。
again = workdir / 'words-again.txt'
subprocess.run(['bash', str(tools / 'generate-input.sh'), str(again)], check=True)
assert again.read_bytes() == input_path.read_bytes(), 'generator is not deterministic'
digest = subprocess.run(['sha256sum', str(input_path)], capture_output=True,
                        text=True, check=True).stdout.split()[0]
recorded = (tools / 'input.sha256').read_text().split()[0]
assert digest == recorded, (digest, recorded)


def build(name, source_path):
    binary = workdir / name
    done = subprocess.run([hado, 'build', '-o', str(binary), str(source_path)],
                          capture_output=True, text=True)
    assert done.returncode == 0, (name, done.stderr[:800])
    return binary


def normalize(text):
    """両プログラムの出力を語 → 回数の対応に正規化する。"""
    table = {}
    for line in text.splitlines():
        if not line.strip():
            continue
        word, count = line.split()
        assert word not in table, word
        table[word] = int(count)
    return table


# 判定器が入力を空白区切りで独立に数えた語 → 回数。両者の一致ではなく、
# この対応と照合する。
independent = {}
for word in words:
    independent[word] = independent.get(word, 0) + 1
assert sum(independent.values()) == 200000
assert len(independent) == 500

for name in ('bump.hd', 'map.hd', 'control.hd'):
    binary = build(name.split('.')[0], tools / name)
    done = subprocess.run([str(binary)], input=input_path.read_bytes(),
                          capture_output=True)
    reason = done.stderr[:400].decode()
    assert done.returncode == 0, (name, done.returncode, reason)
    if name != 'control.hd':
        table = normalize(done.stdout.decode())
        assert table == independent, (
            name, {k: (table.get(k), v) for k, v in independent.items()
                   if table.get(k) != v})
        # 200000 回の更新をしたこと（頻度の総和が語数に一致する）。
        assert sum(table.values()) == 200000, (name, sum(table.values()))
    else:
        # 対照は頻度の更新をしない。同じ分割で先頭 500 語を回数 0 で出す。
        lines = [line for line in done.stdout.decode().splitlines() if line.strip()]
        assert len(lines) == 500, (name, len(lines))
        control_words = []
        for line in lines:
            word, count = line.split()
            assert count == '0', (name, line)
            control_words.append(word)
        assert control_words == words[:500], (name, control_words[:5], words[:5])
print(f'frequencies: both CLIs match the independent count of {len(words)} words over '
      f'{len(independent)} distinct words (sum {sum(independent.values())}), and the '
      f'control splits the same input without updating (digest {digest[:12]})')
PY
}

# --- g: §55 の member の型と昇順（条件 1）---
spec_members() {
    python3 - "$workdir" "$hado_abs" "$root" <<'PY'
import re
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'g'
workdir.mkdir(parents=True, exist_ok=True)
hado, root = sys.argv[2], Path(sys.argv[3])
spec = (root / 'docs/spec.md').read_text()
start = spec.index('## 55. prelude')
end = spec.index('\n## 56. ', start)
block = spec[start:end]

# §55 の member の型。関数集合の `Map:` の行と、型を並べた ```ocaml の
# `module Map` の節が両方あって、名前が一致することを見る。
inventory = block[block.index('関数集合は次とする。'):]
inventory = inventory[inventory.index('```text') + len('```text'):]
inventory = inventory[:inventory.index('```')]
declared = []
for line in inventory.splitlines():
    if not line.strip():
        continue
    names, functions = line.split(':', 1)
    if 'Map' in [n.strip() for n in names.split('/')]:
        declared = functions.split()
assert declared, 'Map is not in the §55 function set'
assert block.count('module Map') == 1, block.count('module Map')
typed = block[block.index('module Map'):]
typed = typed[:typed.index('```', typed.index('module Map'))]
# `def K insert(...)` の形から member 名を取る（型変数の前置きは member 名の前）。
found = re.findall(r'^def\s+(?:\([^)]*\)\s+)?(\w+)\s*[(:]', typed, re.M)
assert sorted(found) == sorted(declared), (found, declared)

# `find` は `V Option` を返す。
find_returns = None
for line in typed.splitlines():
    if line.strip().startswith('def') and re.search(r'\bfind\(', line):
        find_returns = line.rsplit('):', 1)[1].strip()
assert find_returns == 'V Option Pure', find_returns
# `fold` と `to_list` が昇順であることを §55 の本文が述べている。
assert '昇順' in block, '§55 must state the ascending order'

# §48 の functor は今は作らない（issue の案）。`MakeMap` が prelude に
# 無いことを測る（あったらこのラウンドのスコープ外の決定が入っている）。
assert 'MakeMap' not in spec[:start] + spec[end:], 'MakeMap is out of scope'

# 受理コーパスが §55 の member を実走して昇順を測っている。実行して照合する。
cases = {
    'empty_find': 'def main =\n'
                  '  let m = Map.empty ->\n'
                  '  let removed = Map.remove m "k" ->\n'
                  '  [Option.unwrap_or (Map.find m "k") "none",\n'
                  '   Int.to_string (List.length (Map.to_list removed))]',
    'ascending': 'def key_of | MapEntry(k, _) => k\n'
                 'def main =\n'
                 '  let m = Map.insert (Map.insert (Map.insert Map.empty "3" 0) "1" 0) "2" 0 ->\n'
                 '  List.map (Map.to_list m) key_of',
    'fold_ascending': 'def main =\n'
                      '  let m = Map.insert (Map.insert (Map.insert Map.empty "3" 0) "1" 0) "2" 0 ->\n'
                      '  Map.fold m [] fn(acc, k, _ => List.append acc [k])',
}


def run(name, source):
    directory = workdir / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    binary = directory / 'program'
    done = subprocess.run([hado, 'build', '-o', str(binary), str(path)],
                          capture_output=True, text=True)
    assert done.returncode == 0, (name, done.stderr[:400])
    return subprocess.run([str(binary)], capture_output=True, text=True).stdout.strip()


empty = run('empty_find', cases['empty_find'])
assert empty == '["none", "0"]', empty
ascending = run('ascending', cases['ascending'])
assert ascending == '["1", "2", "3"]', ascending
folded = run('fold_ascending', cases['fold_ascending'])
assert folded == '["1", "2", "3"]', folded
print(f'§55: {len(declared)} Map members typed, find returns V Option, '
      f'fold and to_list are ascending')
PY
}

# --- h: 単語頻度の速さ（条件 5）---
word_freq_timing() {
    python3 - "$workdir" "$hado_abs" "$root" <<'PY'
import resource
import statistics
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'h'
workdir.mkdir(parents=True, exist_ok=True)
hado, root = sys.argv[2], Path(sys.argv[3])
tools = root / 'tools' / 'wordfreq'
RUNS = 5

input_path = workdir / 'words.txt'
subprocess.run(['bash', str(tools / 'generate-input.sh'), str(input_path)], check=True)
payload = input_path.read_bytes()


def build(name):
    binary = workdir / name
    done = subprocess.run([hado, 'build', '-o', str(binary), str(tools / f'{name}.hd')],
                          capture_output=True, text=True)
    assert done.returncode == 0, (name, done.stderr[:800])
    return binary


def child_cpu_seconds():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def medians(control, target):
    """対照と対象を 1 回ずつ交互に測り、組ごとの差の中央値を返す。"""
    def once():
        start = child_cpu_seconds()
        subprocess.run([str(control)], input=payload, capture_output=True, check=True)
        control_seconds = child_cpu_seconds() - start
        start = child_cpu_seconds()
        subprocess.run([str(target)], input=payload, capture_output=True, check=True)
        return control_seconds, child_cpu_seconds() - start

    once()
    pairs = [once() for _ in range(RUNS)]
    controls = [c for c, _ in pairs]
    targets = [t for _, t in pairs]
    return (statistics.median(controls), statistics.median(targets),
            statistics.median([t - c for c, t in pairs]))


control = build('control')
bump = build('bump')
map_binary = build('map')
control_total, bump_total, bump_extra = medians(control, bump)
_, map_total, map_extra = medians(control, map_binary)
print(f'bump: total {bump_total:.3f}s control {control_total:.3f}s '
      f'subtracted {bump_extra:.3f}s')
print(f'map:  total {map_total:.3f}s control {control_total:.3f}s '
      f'subtracted {map_extra:.3f}s')

# 条件 5 の「Map 版が速いことを測る」。対照を引いた差の中央値で判定する
# （docs/issues/flaky-string-split-timing-gate.md: 中央値で判定し、閾値は
# 据え置く。退行を測る力は落とさない）。負荷で偽赤を出さない形は保つ:
# warm-up 1 回 + 5 組、対照減算、組ごとの差の中央値。片方が対照と同じか
# 速い（差が 0 以下）なら、それは測れていないということなので通さない。
assert bump_extra > 0, (
    f'the linear scan is inside the noise floor: subtracted {bump_extra:.3f}s')
assert map_extra > 0, (
    f'the tree update is inside the noise floor: subtracted {map_extra:.3f}s')
ratio = bump_extra / map_extra
assert ratio > 1.0, (
    f'the Map CLI is not faster than the association list: '
    f'bump {bump_extra:.3f}s, map {map_extra:.3f}s, ratio {ratio:.2f}')
print(f'ratio bump/map of the subtracted time: {ratio:.2f}')
PY
}

# --- i: 重複する鍵の畳み（条件 6）---
duplicate_keys() {
    python3 - "$workdir" "$hado_abs" <<'PY'
import subprocess
import sys
from pathlib import Path

workdir = Path(sys.argv[1]) / 'i'
workdir.mkdir(parents=True, exist_ok=True)
hado = sys.argv[2]


def run(name, source):
    directory = workdir / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    binary = directory / 'program'
    done = subprocess.run([hado, 'build', '-o', str(binary), str(path)],
                          capture_output=True, text=True)
    assert done.returncode == 0, (name, done.stderr[:600])
    got = subprocess.run([str(binary)], capture_output=True, text=True)
    assert got.returncode == 0, (name, got.returncode, got.stderr[:400])
    return got.stdout.strip()


# `-0.0` の後に `+0.0`。`to_list` の長さは 1、値は後から入れたもの、
# 両方の鍵で `find` がその値を返し、両方の鍵で `remove` がその 1 件を消す。
# 鍵は IEEE の `==` では区別できないので、`to_list` の鍵の
# `Float.to_string` が `"-0.0"` であることを測る（§54）。
key_of = 'def key_of | MapEntry(k, _) => k\n'
first_of = ('def first_of\n'
            '| [] => MapEntry(0.0, "")\n'
            '| [e, ..._] => e\n')
zero = run('zero', key_of + first_of + 'def main =\n'
           '  let m = Map.insert (Map.insert Map.empty (-0.0) "neg") 0.0 "pos" ->\n'
           '  [ Int.to_string (List.length (Map.to_list m)),\n'
           '    Option.unwrap_or (Map.find m (-0.0)) "none",\n'
           '    Option.unwrap_or (Map.find m 0.0) "none",\n'
           '    Float.to_string (key_of (first_of (Map.to_list m))),\n'
           '    Option.unwrap_or (Map.find (Map.remove m 0.0) (-0.0)) "none",\n'
           '    Int.to_string (List.length (Map.to_list (Map.remove m (-0.0)))) ]')
assert zero == '["1", "pos", "pos", "-0.0", "none", "0"]', zero

# NaN 同士は `==` が false なので、長さ 1 と find/remove で畳みを測り、
# 鍵の `==` では照合しない。
nan = run('nan', 'def main =\n'
                 '  let nan = 0.0 / 0.0 ->\n'
                 '  let m = Map.insert (Map.insert Map.empty nan "first") nan "second" ->\n'
                 '  [ Int.to_string (List.length (Map.to_list m)),\n'
                 '    Option.unwrap_or (Map.find m nan) "none",\n'
                 '    Int.to_string (List.length (Map.to_list (Map.remove m nan))) ]')
assert nan == '["1", "second", "0"]', nan
print('duplicates: -0.0 then +0.0 and NaN then NaN fold to one key, the later value '
      'wins, find returns it for both keys, remove deletes it through both, and the '
      'key prints as -0.0')
PY
}

check 'a surface: internal constructors and defs are unreachable (E204)' public_surface
check 'b empty: find is None and remove is a no-op on the empty map and a missing key' empty_and_absent
check 'c fold: the accumulator visits keys in compare order' fold_order
check 'd counts: derived compare stays under 4 * floor(log2(n + 1)) primitives' compare_counts
check 'e order: insert and remove through the public members keep compare order' order_preserved
check 'f frequencies: both CLIs match an independent word count' word_freq
check 'g spec: §55 types the Map members and fold/to_list are ascending' spec_members
check 'h timing: the control-subtracted time of the Map CLI' word_freq_timing
check 'i duplicates: equivalent keys fold to one entry with the later value' duplicate_keys

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
