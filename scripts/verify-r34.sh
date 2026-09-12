#!/usr/bin/env bash
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

if ! cargo build --offline --bin hado; then
    echo 'PASSED: 0/0'
    exit 1
fi

hado_target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
compiler=$(readlink -f "$hado_target_dir/debug/hado")
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

for module in Bytes String; do
    label='34-c(0)'
    [ "$module" != String ] || label='34-b(6)'
    check "$label $module の member の引数型・戻り型・効果行は §49 と一致する" \
        python3 - docs/spec.md "stdlib/${module,,}.hd" "$workdir" "$module" <<'PY'
from pathlib import Path
import re
import sys

module = sys.argv[4]


def only_block(pattern, text):
    blocks = re.findall(pattern, text, flags=re.M | re.S)
    if len(blocks) != 1:
        raise ValueError(f"expected one block, found {len(blocks)}: {pattern}")
    return blocks[0]


def member_block(spec):
    section = only_block(r"^# 49\.[^\n]*\n(.*?)(?=^# |\Z)", spec)
    return only_block(r"member の型は次のとおり[^\n]*\n\s*```ocaml\n(.*?)```", section)


def argument_type(argument):
    match = re.fullmatch(r"\s*\w+\s*:\s*(.+)", argument)
    if not match:
        raise ValueError(f"invalid argument: {argument}")
    return " ".join(match[1].split())


def members_of(source):
    body = only_block(rf"^module {module}[ \t]*\n(.*?)(?=^(?:module|implement)\b|\Z)", source)
    members = {}
    for line in body.splitlines():
        line = line.split("//", 1)[0].strip()
        if not line:
            continue
        match = re.fullmatch(r"def\s+(\w+)\s*\(([^()]*)\)\s*:\s*(.+?)\s+(Pure|\{[^{}]+\})", line)
        if not match or match[1] in members:
            raise ValueError(f"invalid or duplicate member: {line}")
        members[match[1]] = (
            tuple(argument_type(arg) for arg in match[2].split(",")) if match[2].strip() else (),
            " ".join(match[3].split()),
            re.sub(r"\s+", "", match[4]),
        )
    if not members:
        raise ValueError(f"empty {module} contract")
    return members


def differences(expected, actual):
    return {name: (expected.get(name), actual.get(name))
            for name in expected.keys() | actual.keys()
            if expected.get(name) != actual.get(name)}


spec = Path(sys.argv[1]).read_text()
block = member_block(spec)
expected = members_of(block)
assert not differences(expected, members_of(block)), "equal contracts must match"
contract_block = only_block(rf"^(module {module}[ \t]*\n.*?)(?=^module|\Z)", block)

# 仕様の写しを再読して照合する。期待値を被験側から取る判定では、この対照が落ちる。
changed, count = re.subn(r"^(def length\([^\n]*\):[^\n]*) Pure$", r"\1 {Error}", contract_block, flags=re.M)
assert count == 1, "spec mutation must change member length exactly once"
copy = Path(sys.argv[3]) / "spec-effect.md"
copy.write_text(spec.replace(contract_block, changed, 1))
changed_members = members_of(member_block(copy.read_text()))
assert set(differences(changed_members, expected)) == {"length"}, "spec effect mutation escaped"
print("   (対照: §49 の写しの member length Pure → {Error} を検出)")

for pattern, replacement in (
    (r"^(def length\([^:]+: [^)]*)(\))", r"\1 List\2"),
    (r"^(def length\([^)]*\): \w+)", r"\1 List"),
):
    changed, count = re.subn(pattern, replacement, contract_block, flags=re.M)
    assert count == 1, "type mutation must change member length exactly once"
    assert set(differences(expected, members_of(changed))) == {"length"}, "type mutation escaped"

for empty in ("", f"module {module}\n"):
    try:
        members_of(empty)
    except ValueError:
        pass
    else:
        raise AssertionError("empty contract accepted")
try:
    member_block("")
except ValueError:
    pass
else:
    raise AssertionError("empty spec accepted")
print("   (対照: 全件一致は受理、空の仕様・契約と引数型・戻り型の不一致は拒否)")

actual = members_of(Path(sys.argv[2]).read_text())
mismatches = differences(expected, actual)
for name in sorted(expected.keys() | actual.keys()):
    if name in mismatches:
        print(f"   {module}.{name}: expected={expected.get(name)}, actual={actual.get(name)}")
    else:
        print(f"   {module}.{name}: args={actual[name][0]}, result={actual[name][1]}, effect={actual[name][2]}")
sys.exit(1 if mismatches else 0)
PY
done

mkdir -p "$workdir/cases"

# 探針の値は実行時に生成し、期待値は生成器の側で計算する。被験側のコードには
# 期待値を書かない（暗記で通せないようにする）。
if ! SEED=$(( (RANDOM << 15) | RANDOM )) python3 - "$workdir/cases" <<'PY'
import os, random, sys

base = sys.argv[1]
random.seed(int(os.environ["SEED"]))


def write(kind, source, expect=None, expect_error=None):
    directory = os.path.join(base, kind)
    os.makedirs(directory, exist_ok=True)
    with open(os.path.join(directory, "main.hd"), "w") as handle:
        handle.write(source)
    if expect is not None:
        with open(os.path.join(directory, "expect"), "wb") as handle:
            handle.write(expect.encode("utf-8"))
    if expect_error is not None:
        with open(os.path.join(directory, "expect_error"), "wb") as handle:
            handle.write((expect_error + "\n").encode("utf-8"))


manifest = []


def add(kind, desc, mode):
    manifest.append((kind, desc, mode))


def lit(values):
    return ", ".join(str(value) for value in values)


def plist(values):
    return "[" + lit(values) + "]"


ascii_alphabet = "abcdefghijklmnop"
multi = ["あ", "い", "😀", "é", "ß", "Ω"]


def rand_string():
    length = random.randint(3, 8)
    # 3 バイトの `あ` と 4 バイトの `😀` を必ず 1 つずつ含める。ASCII だけの列で
    # バイト数とスカラ値の個数が偶然一致して assert で生成器が落ちるのを防ぐ。
    parts = ["あ", "😀"]
    while len(parts) < length:
        parts.append(
            random.choice(ascii_alphabet) if random.random() < 0.5 else random.choice(multi)
        )
    random.shuffle(parts)
    return "".join(parts)


# 34-c(1): Bytes.length はバイト数、String.length はスカラ値の個数。
text = rand_string()
text_bytes = len(text.encode("utf-8"))
text_scalars = len(text)
assert text_bytes != text_scalars, (text, text_bytes, text_scalars)
write(
    "c1",
    f'def main = [Bytes.length (Bytes.of_string "{text}"), String.length "{text}"]\n',
    expect=f"[{text_bytes}, {text_scalars}]\n",
)
add("c1", "34-c(1) of_string の length はバイト数で String.length と異なる", "print")

# 34-c(2a): of_list → to_list の往復。区切りの判定力を保つため 2 要素以上にする。
payload = [random.randint(0, 255) for _ in range(random.randint(2, 9))]
write("c2a", f"def main = Bytes.to_list (Bytes.of_list [{lit(payload)}])\n", expect=plist(payload) + "\n")
add("c2a", "34-c(2a) of_list → to_list の往復", "print")
write("c2a_empty", "def main = Bytes.to_list (Bytes.of_list [])\n", expect="[]\n")
add("c2a_empty", "34-c(2a) 空の of_list → to_list", "print")
for index, values in enumerate(([0], [0, 65])):
    kind = f"c2a_nul_{index}"
    write(kind, f"def main = Bytes.to_list (Bytes.of_list {plist(values)})\n",
          expect=plist(values) + "\n")
    add(kind, f"34-c(2a) 0x00 を含む of_list → to_list は {plist(values)}", "print")

# 34-c(2b): get が生成した要素を返す。
index = random.randrange(len(payload))
write("c2b", f"def main = Bytes.get (Bytes.of_list [{lit(payload)}]) {index}\n", expect=f"{payload[index]}\n")
add("c2b", "34-c(2b) get が生成した要素を返す", "print")

# 34-c(2c): of_string → to_string の往復。
write("c2c", f'def main = Bytes.to_string (Bytes.of_string "{text}")\n', expect=text + "\n")
add("c2c", "34-c(2c) of_string → to_string の往復", "print")

# 34-c(3): 3 万要素の of_list。構築が O(n²) なら 2 秒に収まらない。
# 3 万要素のリテラルはコンパイラの深い再帰でスタックを溢れさせるため、リストは
# hado 側の純粋な生成器で実行時に作る（Bytes.of_list が見る要素数は同じ 3 万）。
large_count = 30000
large_indices = [0, large_count // 2, large_count - 1]
large_expect = plist([large_count] + [(large_count - 1 - index) % 256 for index in large_indices]) + "\n"
large_source = (
    "def rec mod256 n => if n < 256 => n | mod256 (n - 256)\n"
    "def rec gen n => if n <= 0 => [] | [mod256 (n - 1), ...gen (n - 1)]\n"
)
write(
    "c3",
    large_source + f"def main = let b = Bytes.of_list (gen {large_count}) -> "
    "[Bytes.length b, " + ", ".join(f"Bytes.get b {index}" for index in large_indices) + "]\n",
    expect=large_expect,
)
add("c3", "34-c(3) 3 万要素の of_list が 2 秒以内に一致", "timeout")
write(
    "c3_to_list",
    large_source
    + "def rec at values index => if values | [] => -1 | [head, ...tail] => at_head head tail index\n"
    "def rec at_head head tail index => if index == 0 => head | at tail (index - 1)\n"
    f"def main = let values = Bytes.to_list (Bytes.of_list (gen {large_count})) -> "
    "[List.length values, " + ", ".join(f"at values {index}" for index in large_indices) + "]\n",
    expect=large_expect,
)
add("c3_to_list", "34-c(3) 3 万要素の to_list の長さ・先頭・中間・末尾が 2 秒以内に一致", "timeout")

# 34-c(4a): of_list の 0..255 の範囲外。
write("c4a_256", "def main = Bytes.of_list [256]\n", expect_error="ByteOutOfRange")
add("c4a_256", "34-c(4a) of_list [256] は ByteOutOfRange", "error")
write("c4a_neg1", "def main = Bytes.of_list [-1]\n", expect_error="ByteOutOfRange")
add("c4a_neg1", "34-c(4a) of_list [-1] は ByteOutOfRange", "error")

range_values = [0, 255] + [random.randint(0, 255) for _ in range(random.randint(3, 10))]
for limit_name, limit in (("upper", 256), ("lower", -1)):
    for location, index in (("middle", random.randrange(2, len(range_values) - 1)),
                            ("last", len(range_values) - 1)):
        invalid_values = range_values.copy()
        invalid_values[index] = limit
        kind = f"c4a_{limit_name}_{location}"
        write(
            kind,
            f"def main = Bytes.of_list {plist(invalid_values)}\n",
            expect_error="ByteOutOfRange",
        )
        add(kind, f"34-c(4a) of_list {plist(invalid_values)} の位置 {index + 1} は ByteOutOfRange", "error")
write(
    "c4a_valid",
    f"def main = Bytes.of_list {plist(range_values)}\n",
    expect=f"Bytes.of_list {plist(range_values)}\n",
)
add("c4a_valid", f"34-c(4a) 負の対照 同じ長さの範囲内列 {plist(range_values)} は受理・値一致", "print")

# 34-c(4b): 分類から生成し、Python の strict decoder で期待値を導く。
# BEGIN UTF8 CASE GENERATOR
import random


def utf8_bytes(scalar, width):
    if width == 1:
        return [scalar]
    return [((0xFF << (8 - width)) & 0xFF) | (scalar >> (6 * (width - 1)))] + [
        0x80 | ((scalar >> shift) & 0x3F) for shift in range(6 * (width - 2), -1, -6)
    ]


def accepts_utf8(seq):
    try:
        bytes(seq).decode("utf-8", errors="strict")
        return True
    except UnicodeDecodeError:
        return False


def invalid_utf8_classes(rng, boundaries):
    for width, lower, upper in ((2, 0, 0x7F), (3, 0x80, 0x7FF), (4, 0x800, 0xD7FF)):
        yield f"overlong{width}", utf8_bytes(rng.randint(lower, upper), width)
    for scalar in (0xD800, rng.randint(0xD800, 0xDFFF), 0xDFFF):
        yield f"surrogate_{scalar:x}", utf8_bytes(scalar, 3)
    for scalar in (0x110000, rng.randint(0x110000, 0x1FFFFF)):
        yield f"above_max_{scalar:x}", utf8_bytes(scalar, 4)
    for lower, upper in ((0x80, 0xBF), (0xF8, 0xFF)):
        for lead in sorted({lower, rng.randint(lower + 1, upper - 1), upper}):
            yield f"invalid_lead_{lead:x}", [lead]
    for scalar, width in boundaries:
        for position in range(1, width):
            encoded = utf8_bytes(scalar, width)
            yield f"truncated_{scalar:x}_{position}", encoded[:position]
            # 下位 6 bit を保存し、継続バイト検査の欠落を他の検査が隠さないようにする。
            encoded[position] &= 0x7F
            yield f"bad_continuation_{scalar:x}_{position}", encoded


def utf8_cases(rng):
    boundaries = [(0, 1), (0x7F, 1), (0x80, 2), (0x7FF, 2),
                  (0x800, 3), (0xFFFF, 3), (0x10000, 4), (0x10FFFF, 4)]
    valid = [(f"boundary_{scalar:x}", list(chr(scalar).encode("utf-8")))
             for scalar, _ in boundaries]
    for count in (2, 3):
        valid.append((f"concat{count}", sum([rng.choice(valid[:len(boundaries)])[1]
                                           for _ in range(count)], [])))
    valid.extend([("empty", []), ("nul_concat", [0] + valid[-1][1])])
    cases = []
    for name, seq in valid:
        assert accepts_utf8(seq), ("invalid acceptance class", name, seq)
        cases.append((name, seq, accepts_utf8(seq)))
    for name, seq in invalid_utf8_classes(rng, boundaries):
        for form, combined in (("alone", seq), ("valid_before", valid[-1][1] + seq),
                               ("valid_after", seq + valid[-1][1])):
            assert not accepts_utf8(combined), ("valid rejection class", name, form, combined)
            cases.append((f"{name}_{form}", combined, accepts_utf8(combined)))
    return cases
# END UTF8 CASE GENERATOR

utf8_probes = utf8_cases(random)
print(f"   (UTF-8: {len(utf8_probes)} cases, "
      f"{sum(valid for _, _, valid in utf8_probes)} accepted by Python strict decoder)")
for index, (name, encoded, accepted) in enumerate(utf8_probes):
    kind = f"c4b_{index}_{name}"
    if accepted:
        write(kind,
              f"def main = Bytes.of_string (Bytes.to_string (Bytes.of_list {plist(encoded)}))\n",
              expect=f"Bytes.of_list {plist(encoded)}\n")
        add(kind, f"34-c(4b) {name} {plist(encoded)} は受理・往復一致", "print")
    else:
        write(kind, f"def main = Bytes.to_string (Bytes.of_list {plist(encoded)})\n",
              expect_error="InvalidUtf8")
        add(kind, f"34-c(4b) {name} {plist(encoded)} は InvalidUtf8", "error")

# 34-c(4c): get の範囲外。
write(
    "c4c_end",
    f"def main = let b = Bytes.of_list [{lit(payload)}] -> Bytes.get b (Bytes.length b)\n",
    expect_error="IndexOutOfRange",
)
add("c4c_end", "34-c(4c) get (length) は IndexOutOfRange", "error")
write(
    "c4c_neg",
    f"def main = let b = Bytes.of_list [{lit(payload)}] -> Bytes.get b (-1)\n",
    expect_error="IndexOutOfRange",
)
add("c4c_neg", "34-c(4c) get (-1) は IndexOutOfRange", "error")

# 34-c(4d): 負の対照（境界の内側は受理）。
write(
    "c4d_last",
    f"def main = let b = Bytes.of_list [{lit(payload)}] -> Bytes.get b (Bytes.length b - 1)\n",
    expect=f"{payload[-1]}\n",
)
add("c4d_last", "34-c(4d) 負の対照 get は境界内で受理", "print")
write("c4d_zero255", "def main = Bytes.of_list [0, 255]\n", expect="Bytes.of_list [0, 255]\n")
add("c4d_zero255", "34-c(4d) 負の対照 of_list [0, 255] は受理", "print")
write("c4d_empty", "def main = Bytes.to_string (Bytes.of_list [])\n", expect="\n")
add("c4d_empty", "34-c(4d) 負の対照 空の to_string は受理", "print")

# 34-c(5): handler で失敗を捕まえ message を取り出す。abort の腕の型は with 全体の
# 結果型（String）に合う必要があるため、String を返す Bytes.to_string の失敗を使う。
# 不正 UTF-8 の Bytes は handler の外で作り、handler の中は prelude の raise だけにする。
write(
    "c5",
    "def make (n: Int): Bytes {Error} => Bytes.of_list [n]\n"
    "def main = let invalid = make 255 -> with H -> Bytes.to_string invalid\n"
    "handler Error as H\n"
    "abort raise e => e.message\n",
    expect="InvalidUtf8\n",
)
add("c5", "34-c(5) handler が Bytes の失敗を Effect として捕まえる", "print")

# 34-d(1): 最上位の印字。
write("d1", f"def main = Bytes.of_list [{lit(payload)}]\n", expect="Bytes.of_list [" + lit(payload) + "]\n")
add("d1", "34-d(1) 最上位の Bytes 印字", "print")

# 34-d(2): 入れ子と record field の印字。
write("d2_list", "def main = [Bytes.of_list [1], Bytes.of_list []]\n", expect="[Bytes.of_list [1], Bytes.of_list []]\n")
add("d2_list", "34-d(2) 入れ子の Bytes 印字", "print")
write("d2_record", "type Box = { b: Bytes }\ndef main = { b = Bytes.of_list [1] }\n", expect="{ b = Bytes.of_list [1] }\n")
add("d2_record", "34-d(2) record field の Bytes 印字", "print")

# 34-e: == != compare がバイト列の構造比較。Bool は if で Int に落として同じリストに置く。
# 同値の比較（== が真・compare が 0）を確実に測るため、同じ内容を別インスタンスで
# 構築した探針と、空列どうしの探針を必ず置く。大小は最後のバイトだけを 1 増やした
# 列で測り、compare の両方向の符号が逆になることまで見る（乱数で偶然に頼らない）。
same = [random.randint(0, 255) for _ in range(random.randint(1, 6))]
same[-1] = same[-1] % 255
bumped = same[:-1] + [same[-1] + 1]
left = [random.randint(0, 255) for _ in range(random.randint(0, 6))]
right = [random.randint(0, 255) for _ in range(random.randint(0, 6))]
order = -1 if left < right else (0 if left == right else 1)
write(
    "e",
    f"def main = let x = Bytes.of_list [{lit(left)}] -> let y = Bytes.of_list [{lit(right)}] -> "
    f"[if x == y => 1 | 0, if x != y => 1 | 0, compare x y]\n",
    expect=f"[{1 if left == right else 0}, {1 if left != right else 0}, {order}]\n",
)
add("e", "34-e == != compare がバイト列の構造比較", "print")
write(
    "e_same",
    f"def main = let x = Bytes.of_list [{lit(same)}] -> let y = Bytes.of_list [{lit(same)}] -> "
    "[if x == y => 1 | 0, if x != y => 1 | 0, compare x y]\n",
    expect="[1, 0, 0]\n",
)
add("e_same", "34-e 同内容の別インスタンスは == が真で compare 0", "print")
write(
    "e_empty",
    "def main = let x = Bytes.of_list [] -> let y = Bytes.of_list [] -> "
    "[if x == y => 1 | 0, if x != y => 1 | 0, compare x y]\n",
    expect="[1, 0, 0]\n",
)
add("e_empty", "34-e 空列どうしは == が真で compare 0", "print")
write(
    "e_order",
    f"def main = let x = Bytes.of_list [{lit(same)}] -> let y = Bytes.of_list [{lit(bumped)}] -> "
    "[if x == y => 1 | 0, if x != y => 1 | 0, compare x y, compare y x]\n",
    expect="[0, 1, -1, 1]\n",
)
add("e_order", "34-e 大小両方向で compare の符号が逆", "print")
sequence = [random.randint(0, 255) for _ in range(random.randint(2, 9))]
for length in (0, random.randint(1, len(sequence) - 1)):
    prefix = sequence[:length]
    order = (prefix > sequence) - (prefix < sequence)
    kind = f"e_prefix_{length}"
    write(
        kind,
        f"def main = let x = Bytes.of_list [{lit(prefix)}] -> let y = Bytes.of_list [{lit(sequence)}] -> "
        "[compare x y, compare y x]\n",
        expect=plist([order, -order]) + "\n",
    )
    add(kind, f"34-e 長さ {length}/{len(sequence)} の接頭辞対は compare の両方向で符号が逆", "print")

for index, (left, right) in enumerate((
    ([0, 0, 1], [0, 0, 2]),
    ([0], [0, 0]),
    ([0, 255], [0, 255, 0]),
)):
    expected = []
    for a, b in ((left, right), (right, left)):
        expected.extend([int(a == b), int(a != b), (a > b) - (a < b)])
    kind = f"e_nul_{index}"
    write(
        kind,
        f"def main = let x = Bytes.of_list {plist(left)} -> let y = Bytes.of_list {plist(right)} -> "
        "[if x == y => 1 | 0, if x != y => 1 | 0, compare x y, "
        "if y == x => 1 | 0, if y != x => 1 | 0, compare y x]\n",
        expect=plist(expected) + "\n",
    )
    add(kind, f"34-e 0x00 を含む {plist(left)}/{plist(right)} の == != compare 両方向は {plist(expected)}", "print")

# 34-f: Bytes 注釈の関数は受理される。
write(
    "f_annot",
    f'def main = Bytes.length (f (Bytes.of_string "{text}"))\ndef f (b: Bytes) => b\n',
    expect=f"{text_bytes}\n",
)
add("f_annot", "34-f Bytes 注釈の関数が受理され実行できる", "print")

# 34-f2: shared type T = Bytes の等式が型の登録より前に収集される。
store = (
    "module Store\n"
    "type T\n"
    "def make(value: Bytes): T Pure\n"
)
write(
    "f2_shared",
    f'def main = read ({{ value = Store.make (Bytes.of_string "{text}") }})\n'
    "def read box => Bytes.length box.value\n"
    "type Box = { value: Store.T }\n"
    + store
    + "implement Store as Store\nshared type T = Bytes\ndef make value => value\n",
    expect=f"{text_bytes}\n",
)
add("f2_shared", "34-f2 shared type T = Bytes が record field に届く", "print")

# 34-f2 負の対照: shared を消すと等式が無く、record field が不透明なまま落ちる。
write(
    "f2_opaque",
    f'def main = read ({{ value = Store.make (Bytes.of_string "{text}") }})\n'
    "def read box => Bytes.length box.value\n"
    "type Box = { value: Store.T }\n"
    + store
    + "implement Store as Store\ntype T = Bytes\ndef make value => value\n",
)
add("f2_opaque", "34-f2 負の対照 shared 無しは E305", "code:E305")

# 34-a / 34-b: Unicode の索引と境界を分類から生成し、Python を oracle にする。
def quoted(value):
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n').replace('\t', '\\t') + '"'


def string_probe(label, expression, value=None, error=None, handler=None):
    kind = 'string_' + str(len(manifest))
    source = f'def main = {expression}\n'
    if handler is not None:
        source = (f'def main = with H -> {expression}\n'
                  'handler Error as H\n'
                  f'abort raise e => {handler}\n')
    if error is not None:
        source = f'// expect-error: {error}\n' + source
    write(kind, source, expect=None if error else value + '\n', expect_error=error)
    add(kind, label, 'error' if error else 'print')


def indices(s):
    return sorted({-1, 0, (random.randrange(1, len(s) - 1) if len(s) > 2 else 0), len(s) - 1, len(s), len(s) + 1})


def check_indices(category, s):
    bounds = indices(s)
    for start in bounds:
        for end in bounds:
            valid = 0 <= start <= end <= len(s)
            string_probe(f'34-{ "a(1,2)" if valid else "b(1)"} {category} slice {start}:{end}',
                         f'String.slice {quoted(s)} ({start}) ({end})',
                         value=s[start:end] if valid else None,
                         error=None if valid else 'IndexOutOfRange')
    for index in bounds:
        valid = 0 <= index < len(s)
        string_probe(f'34-{ "a(3)" if valid else "b(2)"} {category} get {index}',
                     f'String.get {quoted(s)} ({index})',
                     value=s[index] if valid else None,
                     error=None if valid else 'IndexOutOfRange')
    # 各失敗のすぐ隣を、成功として独立に測る。
    for index in (0, len(s)):
        string_probe(f'34-b(4) {category} slice {index}:{index}',
                     f'String.slice {quoted(s)} {index} {index}', value=s[index:index])
    if s:
        string_probe(f'34-b(4) {category} last get',
                     f'String.get {quoted(s)} {len(s) - 1}', value=s[-1])
    for width in sorted({len(char.encode('utf-8')) for char in s}):
        index = next(i for i, char in enumerate(s) if len(char.encode('utf-8')) == width)
        label = f'utf8-{width} get {index} expected {s[index]!r}'
        string_probe(f'34-a(3) {category} {label}',
                     f'String.get {quoted(s)} {index}', value=s[index])
        string_probe(f'34-b(5) {category} handler get {label}',
                     f'String.get {quoted(s)} {index}', value=s[index],
                     handler='String.get e.message (String.length e.message - 1)')
    if category in ('multibyte', 'mixed'):
        for scalar_kind, char in scalar_classes:
            index = s.index(char)
            string_probe(f'34-a(3) {category} {scalar_kind} get {index}',
                         f'String.get {quoted(s)} {index}', value=char)
            string_probe(f'34-a(1,2) {category} {scalar_kind} slice {index}:{index + 1}',
                         f'String.slice {quoted(s)} {index} {index + 1}', value=s[index:index + 1])


def check_split(category, s):
    separators = [('ascii', random.choice(',;:')), ('utf8-2', random.choice('éΩ')),
                  ('utf8-3', random.choice('あい')), ('utf8-4', '😀'),
                  ('sequence', ''.join(random.choices('aéΩあ😀', k=random.randint(2, 4)))),
                  ('nul', '\0')]
    if category in ('multibyte', 'mixed'):
        separators.extend(scalar_classes)
    for sep_kind, sep in separators:
        middle = random.randrange(1, len(s)) if len(s) > 1 else 0
        shapes = {'leading': sep + s, 'trailing': s + sep, 'consecutive': s + sep + sep + s,
                  'middle': s[:middle] + sep + s[middle:], 'absent': s.replace(sep, ''),
                  'whole': sep, 'original': s, 'empty': ''}
        # 重なり得るセパレータを消したあとに新しい一致ができても、oracle で答えを取る。
        while sep in shapes['absent']:
            shapes['absent'] = shapes['absent'].replace(sep, '')
        for position, text in shapes.items():
            value = '[' + ', '.join(map(quoted, text.split(sep))) + ']'
            string_probe(f'34-a(4) {category} {sep_kind} {position}',
                         f'String.split {quoted(text)} {quoted(sep)}', value=value)
            string_probe(f'34-a(5,6) {category} {sep_kind} {position}',
                         f'[String.starts_with {quoted(text)} {quoted(sep)}, String.contains {quoted(text)} {quoted(sep)}]',
                         value='[' + ', '.join(str(v).lower() for v in (text.startswith(sep), sep in text)) + ']')
    string_probe(f'34-b(3) {category} empty separator',
                 f'String.split {quoted(s)} ""', error='EmptySeparator')
    string_probe(f'34-b(4) {category} split valid separator',
                 f'String.split {quoted(s)} ","',
                 value='[' + ', '.join(map(quoted, s.split(','))) + ']')
    longer = s + 'Z'
    string_probe(f'34-a(4) {category} separator longer than string',
                 f'String.split {quoted(s)} {quoted(longer)}',
                 value='[' + ', '.join(map(quoted, s.split(longer))) + ']')


def check_internal_parts(category, s):
    positions = [j for j in range(1, len(s) - 1)
                 if s[j] != s[0] and (category == 'ascii' or ord(s[j]) > 127)]
    start = random.choice(positions)
    for width in (1, random.randint(2, len(s) - start)):
        part = s[start:start + width]
        value = '[' + ', '.join(str(v).lower() for v in (s.startswith(part), part in s)) + ']'
        assert not s.startswith(part) and part in s, (category, s, part)
        string_probe(f'34-a(5,6) {category} internal {start}:{start + width} expected {value}',
                     f'[String.starts_with {quoted(s)} {quoted(part)}, String.contains {quoted(s)} {quoted(part)}]',
                     value=value)


def check_longer_parts(category, s):
    suffixes = [('nul', '\0'),
                ('utf8-1', chr(random.randint(0x61, 0x7a))),
                ('utf8-2', chr(random.randint(0x80, 0x7ff))),
                ('utf8-3', chr(random.randint(0x800, 0xd7ff))),
                ('utf8-4', chr(random.randint(0x40000, 0x10ffff)))]
    candidates = [(kind, s + char) for kind, char in suffixes]
    candidates.append(('arbitrary', ''.join(random.choices('XYZ', k=len(s) + random.randint(2, 5)))))
    for kind, part in candidates:
        part.encode('utf-8')
        assert len(part) > len(s), (category, kind, s, part)
        string_probe(f'34-a(5,6) {category} longer {kind} expected [false, false]',
                     f'[String.starts_with {quoted(s)} {quoted(part)}, String.contains {quoted(s)} {quoted(part)}]',
                     value='[' + ', '.join(str(v).lower() for v in (s.startswith(part), part in s)) + ']')


def check_predicates(category, s):
    check_longer_parts(category, s)
    for length in sorted({0, 1, len(s), len(s) + 1}):
        prefix = (s + 'Z')[:length]
        candidates = [('matching', prefix), ('different', 'Z' * length)]
        if 0 < length <= len(s):
            candidates.append(('different_tail', prefix[:-1] + 'Z'))
        for match_kind, candidate in candidates:
            if match_kind == 'different_tail':
                assert len(candidate) == len(prefix) and candidate[:-1] == prefix[:-1] and candidate != prefix, (category, length, prefix, candidate)
            string_probe(f'34-a(5) {category} prefix {length} {match_kind}',
                         f'String.starts_with {quoted(s)} {quoted(candidate)}',
                         value=str(s.startswith(candidate)).lower())
    if s:
        check_internal_parts(category, s)
    middle = random.randrange(1, len(s) - 1) if len(s) > 2 else 0
    parts = {'leading': s[:1], 'middle': s[middle:middle + 1], 'trailing': s[-1:],
             'absent': 'Z', 'empty': '', 'whole': s, 'longer': s + 'Z',
             'reversed': s[::-1], 'repeated': s + s, 'subsequence': s[::2]}
    for position, part in parts.items():
        string_probe(f'34-a(6) {category} contains {position}',
                     f'String.contains {quoted(s)} {quoted(part)}', value=str(part in s).lower())


# 乱数だけでは先頭バイトの payload の上位ビットを落とす故障を見逃し得る。
scalar_classes = [(f'utf8-{width}-maximum', chr(maximum))
                  for width, maximum in enumerate((0x7f, 0x7ff, 0xffff, 0x10ffff), 1)]
scalar_classes += [(f'utf8-{width}-upper', chr(random.randint(lower, maximum - 1)))
                   for width, lower, maximum in ((2, 0x400, 0x7ff), (3, 0x8000, 0xd800),
                                                 (4, 0x40000, 0x10ffff))]
scalar_text = ''.join(char for _, char in scalar_classes)
scalar_text.encode('utf-8')
strings = {
    'empty': '',
    'ascii': ''.join(random.sample('abcde', 2) + random.choices('abcde', k=random.randint(1, 7))),
    'multibyte': scalar_text + 'éあ😀a' + ''.join(random.choices('éΩあ😀', k=random.randint(1, 7))),
    'mixed': 'a' + scalar_text + 'éあ😀' + ''.join(random.choices('abcéΩあ😀', k=random.randint(1, 7))),
    'nul': '\0a' + scalar_text + 'éあ😀' + ''.join(random.choices('aéΩあ😀\0', k=random.randint(1, 7))) + '\0',
}
for category, s in strings.items():
    check_indices(category, s)
    check_split(category, s)
    check_predicates(category, s)
    string_probe(f'34-a length unchanged {category}', f'String.length {quoted(s)}', value=str(len(s)))
    string_probe(f'34-b(5) {category} handler slice',
                 f'String.slice {quoted(s)} 0 {len(s) + 1}',
                 value='IndexOutOfRange', handler='e.message')
    # get の abort の腕も Char。message の末尾は両エラーで異なる。
    string_probe(f'34-b(5) {category} handler get',
                 f'String.get {quoted(s)} {len(s)}',
                 value='IndexOutOfRange'[-1],
                 handler='String.get e.message (String.length e.message - 1)')
    string_probe(f'34-b(5) {category} handler split',
                 f'String.split {quoted(s)} ""',
                 value='[' + quoted('EmptySeparator') + ']', handler='[e.message]')

# 非重複の分割と、候補の部分一致のあとにある一致も回す。
for sep in ['a' * random.randint(2, 4), 'あ😀あ']:
    text = sep + sep[:-1] + sep + sep
    string_probe('34-a(4) overlapping separator', f'String.split {quoted(text)} {quoted(sep)}',
                 value='[' + ', '.join(map(quoted, text.split(sep))) + ']')
print('   (String classes: ' + ', '.join(f'{name}={value!r}' for name, value in strings.items()) + ')')


with open(os.path.join(base, "manifest.tsv"), "w") as handle:
    for kind, desc, mode in manifest:
        handle.write(f"{kind}\t{desc}\t{mode}\n")
PY
then
    echo "NG: 探針の生成器が失敗した（判定を実行できない）" >&2
    exit 1
fi

if [ ! -s "$workdir/cases/manifest.tsv" ]; then
    echo "NG: 探針の manifest が無いか空（判定を実行できない）" >&2
    exit 1
fi

case_dir() { printf '%s/cases/%s' "$workdir" "$1"; }

build_case() {
    local kind="$1" dir
    dir=$(case_dir "$kind")
    "$compiler" build "$dir/main.hd" >/dev/null 2>"$workdir/build-$kind.err"
}

# 標準出力 / 標準エラーが空であることは、ファイルに保存してバイト数で測る
# （コマンド置換は末尾改行を落とすので使わない）。
prints_match() {
    local kind="$1" dir status=0
    dir=$(case_dir "$kind")
    build_case "$kind" || return 1
    "$dir/main" >"$workdir/out-$kind" 2>"$workdir/err-$kind" || status=$?
    [ "$status" -eq 0 ] || return 1
    [ "$(wc -c <"$workdir/err-$kind")" -eq 0 ] || return 1
    cmp -s "$workdir/out-$kind" "$dir/expect"
}

error_matches() {
    local kind="$1" dir status=0
    dir=$(case_dir "$kind")
    build_case "$kind" || return 1
    "$dir/main" >"$workdir/out-$kind" 2>"$workdir/err-$kind" || status=$?
    [ "$status" -eq 1 ] || return 1
    [ "$(wc -c <"$workdir/out-$kind")" -eq 0 ] || return 1
    cmp -s "$workdir/err-$kind" "$dir/expect_error"
}

output_controls() {
    local compiler=true dir
    dir=$(case_dir output-control)
    mkdir -p "$dir"
    cat >"$dir/main" <<'SH'
#!/usr/bin/env bash
cd "$(dirname "$0")"
cat stdout
cat stderr >&2
exit "$(cat status)"
SH
    chmod +x "$dir/main"
    printf 'value\n' >"$dir/expect"
    printf 'error\n' >"$dir/expect_error"
    cp "$dir/expect" "$dir/stdout"
    : >"$dir/stderr"
    echo 0 >"$dir/status"
    prints_match output-control || return 1
    printf 'different\n' >"$dir/stdout"
    if prints_match output-control; then return 1; fi
    : >"$dir/stdout"
    if prints_match output-control; then return 1; fi
    cp "$dir/expect" "$dir/stdout"
    printf '\n' >"$dir/stderr"
    if prints_match output-control; then return 1; fi
    : >"$dir/stdout"
    cp "$dir/expect_error" "$dir/stderr"
    echo 1 >"$dir/status"
    error_matches output-control || return 1
    printf '\n' >"$dir/stdout"
    if error_matches output-control; then return 1; fi
    : >"$dir/stdout"
    printf 'different\n' >"$dir/stderr"
    if error_matches output-control; then return 1; fi
    : >"$dir/stderr"
    if error_matches output-control; then return 1; fi
}

check '34-a/34-b 出力照合の対照: 全件一致は受理、空・値違い・余分な改行は拒否' output_controls

# build と実行を合わせて 1 つの期限で測る。build だけ遅い注入も落ちるよう、
# build と実行の両方に timeout を掛け、経過時間の合計も seconds 以下を要求する。
builds_runs_within() {
    local kind="$1" seconds="$2" dir start end status=0
    dir=$(case_dir "$kind")
    start=$(date +%s%N)
    if ! timeout "$seconds" "$compiler" build "$dir/main.hd" >/dev/null 2>"$workdir/build-$kind.err"; then
        echo "   ($kind: build timed out or failed)"
        return 1
    fi
    timeout "$seconds" "$dir/main" >"$workdir/out-$kind" 2>"$workdir/err-$kind" || status=$?
    end=$(date +%s%N)
    echo "   ($kind: build+run=$(( (end - start) / 1000000 ))ms run_status=$status)"
    [ "$status" -eq 0 ] || return 1
    [ "$(wc -c <"$workdir/err-$kind")" -eq 0 ] || return 1
    cmp -s "$workdir/out-$kind" "$dir/expect" || return 1
    [ $(( end - start )) -le $(( seconds * 1000000000 )) ]
}

code_present() {
    local kind="$1" code="$2" dir status
    dir=$(case_dir "$kind")
    "$compiler" check "$dir/main.hd" >"$workdir/check-$kind.out" 2>"$workdir/check-$kind.err"; status=$?
    [ "$status" -eq 1 ] || return 1
    [ "$(wc -c <"$workdir/check-$kind.out")" -eq 0 ] || return 1
    [[ "$(cat "$workdir/check-$kind.err")" == *"error[$code]:"* ]]
}

while IFS=$'\t' read -r kind desc mode; do
    case "$mode" in
        print) check "$desc" prints_match "$kind" ;;
        error) check "$desc" error_matches "$kind" ;;
        timeout) check "$desc" builds_runs_within "$kind" 2 ;;
        code:*) check "$desc" code_present "$kind" "${mode#code:}" ;;
    esac
done <"$workdir/cases/manifest.tsv"

# 34-f: 予約名。BUILTIN_TYPES に Bytes を足すと type Bytes は E205 になる。診断の
# 行:列は探針の行頭の空白と先頭の空行から計算する（定数で書かない）。
spaces() { printf '%*s' "$1" ''; }

random_padding() {
    local file="$1" blank pad tmp
    tmp="$file.pad"
    blank=$((1 + RANDOM % 4))
    pad=$((1 + RANDOM % 8))
    {
        for ((i = 0; i < blank; i++)); do printf '\n'; done
        while IFS= read -r line; do
            if [ -n "$line" ]; then
                printf '%s%s\n' "$(spaces "$pad")" "$line"
            else
                printf '\n'
            fi
        done <"$file"
    } >"$tmp"
    mv "$tmp" "$file"
}

position_of() {
    local file="$1" needle="$2"
    awk -v needle="$needle" '{ where = index($0, needle); if (where > 0) { print NR ":" where; exit } }' "$file"
}

expect_error_at() {
    local label="$1" entry="$2" code="$3" needle="$4"
    total=$((total + 1))
    local want err status
    want=$(position_of "$entry" "$needle")
    if [ -z "$want" ]; then
        echo "NG: $label (needle not found: $needle)"
        return
    fi
    "$compiler" check "$entry" >"$workdir/error-at.out" 2>"$workdir/error-at.err"; status=$?
    err=$(cat "$workdir/error-at.err")
    local prefix
    prefix="$(basename "$entry"):$want: error[$code]: "
    if [ "$status" -eq 1 ] && [ "$(wc -c <"$workdir/error-at.out")" -eq 0 ] && [[ "$err" == "$prefix"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$prefix', got status=$status '$err')"
    fi
}

mkdir -p "$workdir/e205"
printf 'type Bytes = | A\ndef main = 1\n' >"$workdir/e205/main.hd"
random_padding "$workdir/e205/main.hd"
expect_error_at '34-f type Bytes は E205、2 回目の宣言の名前' "$workdir/e205/main.hd" E205 'Bytes = | A'

# 34-g: m7 全体は走らせず、既存の inventory の判定だけを実行する。
check '34-g m7 inventory の String と Bytes の全 member が OK' python3 - <<'PY'
from pathlib import Path
import re
import subprocess

source = Path('scripts/verify-m7.sh').read_text()
program = re.search(r"^python3 - <<'PY'\n(.*?)^PY$", source, re.M | re.S)
assert program, 'm7 inventory program missing'
result = subprocess.run(['python3', '-'], input=program[1], text=True, capture_output=True)
assert result.returncode in (0, 1) and not result.stderr, result.stderr
lines = result.stdout.splitlines()
for module in ('String', 'Bytes'):
    entries = [line for line in lines if re.match(rf'^(?:OK|NG): {module}(?:\.| )', line)]
    assert len(entries) > 1 and all(line.startswith('OK: ') for line in entries), entries
    print('\n'.join('   ' + line for line in entries))
assert all(re.match(r'^NG: (?:Array(?:\.| )|Int\.parse )', line)
           for line in lines if line.startswith('NG: ')), result.stdout
assert re.fullmatch(r'prelude inventory: \d+/\d+', lines[-1]), result.stdout
print('   ' + lines[-1])
PY

# 34-h: 検査 6 と同じ導出関数を、仕様の写しで実行する。
check '34-h §49 の追加・削除に member 一覧が追随し、空入力は拒否する' \
    python3 - "$workdir" <<'PY'
from pathlib import Path
import re
import subprocess
import sys

source = Path('scripts/verify-m1-corpus.sh').read_text()
function = re.search(r'^prelude_members\(\) \{\n.*?^\}', source, re.M | re.S)
assert function, 'missing prelude_members function in check 6'
assert re.search(r'prelude_members "\$SPEC"', source), 'check 6 must call the tested function'
base = Path(sys.argv[1])
spec = Path('docs/spec.md').read_text()


def derive(text):
    path = base / 'members-spec.md'
    path.write_text(text)
    return subprocess.run(['bash', '-c', function[0] + '\nprelude_members "$1"', 'bash', str(path)],
                          capture_output=True)


original = derive(spec)
assert original.returncode == 0 and original.stdout and not original.stderr
members = original.stdout.decode().splitlines()
assert len(members) == len(set(members))
added = spec.replace('関数集合は次とする。\n\n```text\n',
                     '関数集合は次とする。\n\n```text\nFoo: bar\n', 1)
assert added != spec
changed = derive(added)
assert changed.returncode == 0 and not changed.stderr
assert set(changed.stdout.decode().splitlines()) == set(members) | {'Foo.bar'}
assert len(changed.stdout.decode().splitlines()) == len(members) + 1
removed = derive(added.replace('Foo: bar\n', ''))
assert removed.returncode == 0 and removed.stdout == original.stdout and not removed.stderr
for empty in ('', '# 49. prelude\n', '# 49. prelude\n関数集合は次とする。\n\n```text\n```\n'):
    result = derive(empty)
    assert result.returncode != 0 and not result.stdout, 'empty inventory accepted'
print(f'   (member count: {len(members)} → {len(members) + 1} → {len(members)}; added Foo.bar)')
print('   (対照: 同値・削除は追随、空の仕様・関数集合は拒否)')
PY

# 34-j: ディスク上のタグと lock を列挙し、件数は実数から導く。
check '34-j Bytes と String 操作の受理・失敗形と lock' python3 - <<'PY'
from pathlib import Path
import hashlib


def coverage(sources, item):
    tagged = [text for text in sources if f'// item: {item}' in text.splitlines()]
    return (len(tagged) >= 3
            and any('// expect-error: ' in text for text in tagged)
            and any('// expect: ' in text for text in tagged)
            and all('// needs: prelude' in text.splitlines() for text in tagged))


sources = [path.read_text() for path in Path('tests/accept').glob('*/main.hd')]
for item in ('Bytes', 'String 操作'):
    assert not coverage([], item), 'empty corpus accepted'
    assert not coverage([s for s in sources if '// expect-error:' not in s], item), 'all-success corpus accepted'
    assert coverage(sources, item), f'missing accepted/error corpus: {item}'
    count = sum(f'// item: {item}' in s.splitlines() for s in sources)
    failures = sum(f'// item: {item}' in s.splitlines() and '// expect-error:' in s for s in sources)
    print(f'   ({item}: accept={count}, expect-error={failures})')
paths = sorted(p for folder in ('tests/accept', 'tests/reject', 'tests/library', 'stdlib')
               for p in Path(folder).rglob('*') if p.suffix in ('.hd', '.hdi'))
expected = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}
lines = Path('tests/corpus.sha256').read_text().splitlines()
actual = {line.split('  ', 1)[1]: line.split('  ', 1)[0] for line in lines}
assert len(lines) == len(actual) == len(paths) and actual == expected, 'lock mismatch'
assert expected and actual != dict(list(actual.items())[1:]), 'missing lock entry escaped'
changed = dict(actual)
changed[str(paths[0])] = '0' * 64
assert changed != expected, 'wrong digest escaped'
print(f'   (lock: {len(lines)} entries = {len(paths)} source files; missing/wrong digest rejected)')
PY

# 34-i: 名前の UTF-8 幅 × 数字の桁と大小関係から入力と期待値を生成する。
if ! SEED=$(( (RANDOM << 15) | RANDOM )) python3 - "$workdir/cases" <<'PY'
from pathlib import Path
import os
import random
import shutil
import sys

base = Path(sys.argv[1])
rng = random.Random(int(os.environ['SEED']))
manifest = []


def quoted(value):
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n').replace('\t', '\\t') + '"'


def strings(values):
    return '[' + ', '.join(map(quoted, values)) + ']'


def probe(label, lines, expected=None):
    kind = f'fields_{len(manifest)}'
    directory = base / kind
    directory.mkdir()
    shutil.copytree('tools/gates/report', directory / 'report')
    expression = (f'List.map {strings(lines)} report::Report.fields' if expected is not None
                  else f'report::Report.fields {quoted(lines[0])}')
    header = '' if expected is not None else '// expect-error: InvalidLine\n'
    (directory / 'main.hd').write_text(header + f'def main = {expression}\n')
    if expected is None:
        (directory / 'expect_error').write_bytes(b'InvalidLine\n')
    else:
        (directory / 'expect').write_bytes(('[' + ', '.join(map(strings, expected)) + ']\n').encode())
    manifest.append(f'{kind}\t34-i {label}\t{"print" if expected is not None else "error"}\n')


ascii_name = ''.join(rng.choices('abcdefghijklmnopqrstuvwxyz', k=7)) + chr(0x7f) + '/\\"\t\n\0'
names = [('empty', ''), ('ascii-max', ascii_name)]
for width, low, high in ((2, 0x80, 0x7ff), (3, 0xe000, 0xffff), (4, 0x10000, 0x10ffff)):
    names.append((f'utf8-{width}-max', chr(rng.randint(low, high)) + chr(high)))
names.append(('mixed', ''.join(name for _, name in names)))
small, large = rng.randint(1, 8), rng.randint(100, 900)
numbers = [('single-equal', small, small), ('single-less', small, small + 1),
           ('multiple-equal', large, large), ('multiple-less', large, large + rng.randint(1, 99)),
           ('zero-equal', 0, 0), ('zero-less', 0, large)]
for category, name in names:
    expected = [[name, str(n), str(total)] for _, n, total in numbers]
    lines = [f'{name} PASSED: {n}/{total}' for _, n, total in numbers]
    probe(category + ' × ' + ','.join(label for label, _, _ in numbers), lines, expected)
    invalid = [('no-passed', f'{name} {small}/{large}'),
               ('no-slash', f'{name} PASSED: {small}'),
               ('empty-line', ''), ('name-only', name), ('digits-only', str(large))]
    for repeats in (2, 3):
        invalid.extend([(f'passed-{repeats}', name + ' PASSED: ' * repeats + f'{small}/{large}'),
                        (f'slash-{repeats}', f'{name} PASSED: {small}' + '/' * repeats + str(large))])
    for shape, line in invalid:
        probe(category + '/' + shape, [line])
probe('empty-input-list', [], [])
assert manifest, 'empty fields manifest'
(base / 'fields-manifest.tsv').write_text(''.join(manifest))
print(f'   (fields: names={len(names)}, number classes={len(numbers)}, probes={len(manifest)}; UTF-8 maxima=U+007F/U+07FF/U+FFFF/U+10FFFF)')
PY
then
    echo 'NG: 34-i generator failed'
    exit 1
fi
while IFS=$'\t' read -r kind desc mode; do
    case "$mode" in
        print) check "$desc" prints_match "$kind" ;;
        error) check "$desc" error_matches "$kind" ;;
        *) echo "NG: 34-i invalid mode: $mode"; exit 1 ;;
    esac
done <"$workdir/cases/fields-manifest.tsv"

# §47・§57: prelude の操作だけがディレクトリ境界を越える。
for layout in directory library; do
    for mode in success handled unhandled; do
        kind="prelude-$layout-$mode"
        dir=$(case_dir "$kind")
        mkdir -p "$dir/sub"
        value=$((1 + RANDOM))
        message="probe-$RANDOM-$RANDOM"
        {
            if [ "$layout" = library ]; then printf 'library\n'; fi
            cat <<HD
module Helper
def pick(x: Int): String {Error}
implement Helper as Helper
def pick x => if x > 0 => Int.to_string x | raise { message = "$message" }
HD
        } >"$dir/sub/helper.hd"
        case "$mode" in
            success)
                printf 'def main = sub::Helper.pick %s\n' "$value" >"$dir/main.hd"
                printf '%s\n' "$value" >"$dir/expect"
                ;;
            handled)
                printf 'def main = with H -> sub::Helper.pick 0\nhandler Error as H\nabort raise e => e.message\n' >"$dir/main.hd"
                printf '%s\n' "$message" >"$dir/expect"
                ;;
            unhandled)
                printf 'def main = sub::Helper.pick 0\n' >"$dir/main.hd"
                printf '%s\n' "$message" >"$dir/expect_error"
                ;;
        esac
        if [ "$mode" = unhandled ]; then
            check "34-prelude $layout/$mode" error_matches "$kind"
        else
            check "34-prelude $layout/$mode" prints_match "$kind"
        fi
    done
done

for visibility in local unknown; do
    dir=$(case_dir "operation-$visibility")
    mkdir -p "$dir/sub"
    operation="probe_$RANDOM"
    printf 'def main = %s\n' "$operation" >"$dir/main.hd"
    if [ "$visibility" = local ]; then
        printf 'effect Probe\nreply %s: Int\n' "$operation" >"$dir/sub/effect.hd"
    fi
    random_padding "$dir/main.hd"
    expect_error_at "34-prelude $visibility の操作は親から未束縛" "$dir/main.hd" E204 "$operation"
done

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
