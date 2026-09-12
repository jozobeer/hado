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
        with open(os.path.join(directory, "expect_error"), "w") as handle:
            handle.write(expect_error)


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
write(
    "c3",
    "def rec mod256 n => if n < 256 => n | mod256 (n - 256)\n"
    "def rec gen n => if n <= 0 => [] | [mod256 (n - 1), ...gen (n - 1)]\n"
    "def main = let b = Bytes.of_list (gen 30000) -> "
    "[Bytes.length b, Bytes.get b 0, Bytes.get b 29999]\n",
    expect=f"[30000, {(30000 - 1) % 256}, {0 % 256}]\n",
)
add("c3", "34-c(3) 3 万要素の of_list が 2 秒以内に一致", "timeout")

# 34-c(4a): of_list の 0..255 の範囲外。
write("c4a_256", "def main = Bytes.of_list [256]\n", expect_error="ByteOutOfRange")
add("c4a_256", "34-c(4a) of_list [256] は ByteOutOfRange", "error")
write("c4a_neg1", "def main = Bytes.of_list [-1]\n", expect_error="ByteOutOfRange")
add("c4a_neg1", "34-c(4a) of_list [-1] は ByteOutOfRange", "error")

# 34-c(4b): 不正 UTF-8。
write("c4b", "def main = Bytes.to_string (Bytes.of_list [255])\n", expect_error="InvalidUtf8")
add("c4b", "34-c(4b) to_string 255 は InvalidUtf8", "error")

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
    [ "$(cat "$dir/expect_error")" = "$(cat "$workdir/err-$kind")" ]
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
