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

check '34-c(0) Bytes の member の引数型・戻り型・効果行は §49 と一致する' \
    python3 - docs/spec.md stdlib/bytes.hd "$workdir" <<'PY'
from pathlib import Path
import re
import sys


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


def bytes_members(source):
    body = only_block(r"^module Bytes[ \t]*\n(.*?)(?=^(?:module|implement)\b|\Z)", source)
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
        raise ValueError("empty Bytes contract")
    return members


def differences(expected, actual):
    return {name: (expected.get(name), actual.get(name))
            for name in expected.keys() | actual.keys()
            if expected.get(name) != actual.get(name)}


spec = Path(sys.argv[1]).read_text()
block = member_block(spec)
expected = bytes_members(block)
assert not differences(expected, bytes_members(block)), "equal contracts must match"
bytes_block = only_block(r"^(module Bytes[ \t]*\n.*?)(?=^module|\Z)", block)

# 仕様の写しを再読して照合する。期待値を被験側から取る判定では、この対照が落ちる。
changed, count = re.subn(r"^(def length\([^\n]*\):[^\n]*) Pure$", r"\1 {Error}", bytes_block, flags=re.M)
assert count == 1, "spec mutation must change Bytes.length exactly once"
copy = Path(sys.argv[3]) / "spec-effect.md"
copy.write_text(spec.replace(bytes_block, changed, 1))
changed_members = bytes_members(member_block(copy.read_text()))
assert set(differences(changed_members, expected)) == {"length"}, "spec effect mutation escaped"
print("   (対照: §49 の写しの Bytes.length Pure → {Error} を検出)")

for pattern, replacement in (
    (r"^(def length\([^:]+: [^)]*)(\))", r"\1 List\2"),
    (r"^(def length\([^)]*\): \w+)", r"\1 List"),
):
    changed, count = re.subn(pattern, replacement, bytes_block, flags=re.M)
    assert count == 1, "type mutation must change Bytes.length exactly once"
    assert set(differences(expected, bytes_members(changed))) == {"length"}, "type mutation escaped"

for empty in ("", "module Bytes\n"):
    try:
        bytes_members(empty)
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

actual = bytes_members(Path(sys.argv[2]).read_text())
mismatches = differences(expected, actual)
for name in sorted(expected.keys() | actual.keys()):
    if name in mismatches:
        print(f"   Bytes.{name}: expected={expected.get(name)}, actual={actual.get(name)}")
    else:
        print(f"   Bytes.{name}: args={actual[name][0]}, result={actual[name][1]}, effect={actual[name][2]}")
sys.exit(1 if mismatches else 0)
PY

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

# 34-c(4b): 不正 UTF-8。
write("c4b", "def main = Bytes.to_string (Bytes.of_list [255])\n", expect_error="InvalidUtf8")
add("c4b", "34-c(4b) to_string 255 は InvalidUtf8", "error")
for scalar in (0x10FFFF, 0x110000):
    encoded = [0xF0 | (scalar >> 18)] + [
        0x80 | ((scalar >> shift) & 0x3F) for shift in (12, 6, 0)
    ]
    kind = f"c4b_{scalar:x}"
    source = f"def main = Bytes.to_string (Bytes.of_list [{lit(encoded)}])\n"
    if scalar <= 0x10FFFF:
        write(kind, source, expect=chr(scalar) + "\n")
        add(kind, "34-c(4b) U+10FFFF は受理", "print")
    else:
        write(kind, source, expect_error="InvalidUtf8")
        add(kind, "34-c(4b) U+110000 は InvalidUtf8", "error")

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

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
