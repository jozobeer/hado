#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0

check() {
    local label=$1
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

spaces() { printf '%*s' "$1" ''; }
blank_lines() { local i; for ((i = 0; i < $1; i++)); do printf '\n'; done; }

# 探針の行頭の空白と先頭の空行を乱数で作り、診断の行:列が定数にならないようにする。
random_padding() {
    local file=$1
    local blank pad
    local tmp="$file.pad"
    blank=$((1 + RANDOM % 4))
    pad=$((1 + RANDOM % 8))
    {
        blank_lines "$blank"
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

# 期待する行:列は、探針のファイルから探針側で計算する（定数で書かない）。
position_of() {
    local file=$1 needle=$2
    awk -v needle="$needle" '
        { where = index($0, needle); if (where > 0) { print NR ":" where; exit } }
    ' "$file"
}

# entry の診断が file:line:col: error[code] で始まることを確かめる。line:col は
# entry の中の needle の位置から計算する。
expect_error_at() {
    local label=$1 entry=$2 code=$3 needle=$4
    total=$((total + 1))
    local want err status
    want=$(position_of "$entry" "$needle")
    if [ -z "$want" ]; then
        echo "NG: $label (needle not found: $needle)"
        return
    fi
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$(basename "$entry"):$want: error[$code]: "
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$prefix', got status=$status '$err')"
    fi
}

# 診断のコードが code で、code 以外の位置は問わない（E305 など）。
expect_code() {
    local label=$1 entry=$2 code=$3
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    if [ "$status" -eq 1 ] && [[ "$err" == *"error[$code]:"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want $code, got status=$status '$err')"
    fi
}

expect_not_code() {
    local label=$1 entry=$2 code=$3
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    if [[ "$err" != *"error[$code]:"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label ($code appeared: '$err')"
    fi
}

# code の診断が出て、そのメッセージに needle を含む。
expect_code_message() {
    local label=$1 entry=$2 code=$3 needle=$4
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    if [ "$status" -eq 1 ] && [[ "$err" == *"error[$code]:"* ]] && [[ "$err" == *"$needle"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want $code containing '$needle', got status=$status '$err')"
    fi
}

# code の診断が出て、そのメッセージに forbidden を含まない。
expect_code_absent() {
    local label=$1 entry=$2 code=$3 forbidden=$4
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    if [ "$status" -eq 1 ] && [[ "$err" == *"error[$code]:"* ]] && [[ "$err" != *"$forbidden"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want $code without '$forbidden', got status=$status '$err')"
    fi
}

# stdout / stderr が空であることは、ファイルに保存してバイト数で測る
# （コマンド置換は末尾改行を落とすので使わない）。
accept_quiet() {
    local label=$1 entry=$2
    total=$((total + 1))
    local out="$workdir/accept-stdout" err="$workdir/accept-stderr" status out_bytes err_bytes
    cargo run --offline -q -- check "$entry" >"$out" 2>"$err"; status=$?
    out_bytes=$(wc -c <"$out"); err_bytes=$(wc -c <"$err")
    if [ "$status" -eq 0 ] && [ "$out_bytes" -eq 0 ] && [ "$err_bytes" -eq 0 ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (status=$status stdout_bytes=$out_bytes stderr_bytes=$err_bytes)"
    fi
}

# check 受理 → build → 実行。実行ファイルの印字が expected と一致すること。
accept_build_run() {
    local label=$1 entry=$2 expected=$3
    total=$((total + 1))
    local artifact="${entry%.hd}" got status
    if ! cargo run --offline -q -- check "$entry" >/dev/null 2>"$workdir/abr-check-err"; then
        echo "NG: $label (check failed: $(cat "$workdir/abr-check-err"))"
        return
    fi
    if ! cargo run --offline -q -- build "$entry" >/dev/null 2>"$workdir/abr-build-err"; then
        echo "NG: $label (build failed: $(cat "$workdir/abr-build-err"))"
        return
    fi
    got=$("$artifact" 2>"$workdir/abr-run-err"); status=$?
    if [ "$status" -eq 0 ] && [ "$got" = "$expected" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$expected' got status=$status '$got')"
    fi
}

# check --print-type の stdout が expected と一致し、抽象型の修飾名 `.T` を含まない。
print_type_is() {
    local label=$1 entry=$2 expected=$3
    total=$((total + 1))
    local out="$workdir/print-type-out" err="$workdir/print-type-err" status got
    cargo run --offline -q -- check --print-type "$entry" >"$out" 2>"$err"; status=$?
    got=$(cat "$out")
    if [ "$status" -eq 0 ] && [ "$got" = "$expected" ] && [[ "$got" != *".T"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$expected' without .T, got status=$status '$got' err='$(cat "$err")')"
    fi
}

# 共通の探針。契約 Monoid と実装 StringMonoid。
monoid_contract() {
    printf 'module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\n'
}
monoid_realization() {
    printf 'implement Monoid as StringMonoid\n%s type T = String\ndef combine left right => String.concat left right\ndef of value => value\n' "$1"
}

# 実行で印字する値は実行時に生成し、期待値は判定器側で計算する（定数で書かない）。
rand_a="a${RANDOM}"
rand_b="b${RANDOM}"
rand_name="n${RANDOM}"
rand_int=$(( (RANDOM << 15) | RANDOM ))

mkdir -p "$workdir/33c1" "$workdir/33c2" "$workdir/33c3" "$workdir/33c3neg" "$workdir/33c4" "$workdir/33c5" "$workdir/33c6" \
    "$workdir/33a2" "$workdir/33b2" "$workdir/33d" "$workdir/33dneg" "$workdir/33e" "$workdir/33eneg" \
    "$workdir/33f2" "$workdir/33h" "$workdir/33i1" "$workdir/33i2" "$workdir/33i3" "$workdir/33accept"

# 33-c (1): constructor 付き本体（ADT）への shared は E100、最初の | の位置。
{
    printf 'def main = 1\n\n'
    monoid_contract
    printf '\nimplement Monoid as StringMonoid\nshared type T = | A | B\n'
} >"$workdir/33c1/main.hd"

# 33-c (2): constructor 付き本体（record）への shared は E100、{ の位置。
{
    printf 'def main = 1\n\n'
    monoid_contract
    printf '\nimplement Monoid as StringMonoid\nshared type T = { x: Int }\n'
} >"$workdir/33c2/main.hd"

# 33-c (3): 契約に無い型への shared は E204、型名の位置。契約の T も実体化し、
# 実装は main から参照される形にする（E204 より先に E513 / E602 を出さない）。
{
    printf 'def main = StringMonoid.of "a"\n\n'
    monoid_contract
    printf '\nimplement Monoid as StringMonoid\nshared type T = String\nshared type U = Int\ndef combine left _ => left\ndef of value => value\n'
} >"$workdir/33c3/main.hd"

# 33-c (3) 負の対照: 契約が U を宣言していれば E204 にならない（別の欠陥は問わない）。
{
    printf 'def main = StringMonoid.of "a"\n\n'
    monoid_contract
    printf 'type U\ndef size(value: U): Int Pure\n\nimplement Monoid as StringMonoid\nshared type T = String\nshared type U = Int\ndef combine left _ => left\ndef of value => value\ndef size _ => 0\n'
} >"$workdir/33c3neg/main.hd"

# 33-c (4): 外の shared は E100。
printf 'shared type T = Int\n\ndef main = 1\n' >"$workdir/33c4/main.hd"

# 33-c (5): 契約（module）の中の shared は E100。
{
    printf 'def main = 1\n\n'
    printf 'module Monoid\nshared type T\ndef of(value: String): T Pure\n'
} >"$workdir/33c5/main.hd"

# 33-c (6): shared の後に type 以外（def）は E100。
{
    printf 'def main = 1\n\n'
    monoid_contract
    printf '\nimplement Monoid as StringMonoid\nshared def f = 1\ndef combine left _ => left\ndef of value => value\n'
} >"$workdir/33c6/main.hd"

# 33-a (2): shared の等式が公開面に出て、combine/of が String で単一化する。
{
    printf 'def main = StringMonoid.combine (StringMonoid.of "%s") "%s"\n\n' "$rand_a" "$rand_b"
    monoid_contract
    printf '\n'
    monoid_realization shared
} >"$workdir/33a2/main.hd"

# 33-b (2): 同じ形で shared 無し。不透明のまま E305 で StringMonoid.T が出る。
{
    printf 'def main = StringMonoid.combine (StringMonoid.of "%s") "%s"\n\n' "$rand_a" "$rand_b"
    monoid_contract
    printf '\n'
    monoid_realization ''
} >"$workdir/33b2/main.hd"

# 33-d: 契約 Store の抽象型 T を shared type T = Int で実体化し、main が整数を返す。
{
    printf 'def main = Store.find 1\n\n'
    printf 'module Store\ntype T\ndef find(id: Int): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = Int\ndef find _ => %s\n' "$rand_int"
} >"$workdir/33d/main.hd"

# 33-d 緩和なし: 印なしは E705 で def main。
{
    printf 'def main = Store.find 1\n\n'
    printf 'module Store\ntype T\ndef find(id: Int): T Pure\n\n'
    printf 'implement Store as Store\ntype T = Int\ndef find _ => 1\n'
} >"$workdir/33dneg/main.hd"

# 33-e: shared で record に解け、実装の外の field 参照が通る。
{
    printf 'def main = describe (UserStore.make "%s")\ndef describe (t: UserStore.T) => t.name\n\n' "$rand_name"
    printf 'type User = { name: String }\n\n'
    printf 'module UserStore\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement UserStore as UserStore\nshared type T = User\ndef make name => { name = name }\n'
} >"$workdir/33e/main.hd"

# 33-e 緩和なし: 印なしは E601 で field 参照の `.` の位置。
{
    printf 'def main = describe (UserStore.make "%s")\ndef describe (t: UserStore.T) => t.name\n\n' "$rand_name"
    printf 'type User = { name: String }\n\n'
    printf 'module UserStore\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement UserStore as UserStore\ntype T = User\ndef make name => { name = name }\n'
} >"$workdir/33eneg/main.hd"

# 33-f (2): shared type T = String の値に Int を渡すと E305、両辺に `.T` は現れない。
{
    printf 'def main = StringMonoid.combine (StringMonoid.of "a") 1\n\n'
    monoid_contract
    printf '\n'
    monoid_realization shared
} >"$workdir/33f2/main.hd"

# 33-h: 型引数付きの shared type A T = A List。push と Int Stack.T の注釈が通る。
{
    printf 'def main = Stack.push (f [1]) 2\ndef f (s: Int Stack.T) => s\n\n'
    printf 'module Stack\ntype A T\ndef A push(stack: A T, value: A): A T Pure\ndef A empty: A T Pure\n\n'
    printf 'implement Stack as Stack\nshared type A T = A List\ndef push stack value => [value, ...stack]\ndef empty = []\n'
} >"$workdir/33h/main.hd"

# 33-i (1): shared type T = T は E602 で implement。
{
    printf 'def main = StringMonoid.of "a"\n\n'
    monoid_contract
    printf '\nimplement Monoid as StringMonoid\nshared type T = T\ndef combine left right => left\ndef of value => value\n'
} >"$workdir/33i1/main.hd"

# 33-i (2): shared type T = U と shared type U = T は E602 で implement。
{
    printf 'def main = StringMonoid.of "a"\n\n'
    printf 'module Monoid\ntype T\ntype U\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\n\n'
    printf 'implement Monoid as StringMonoid\nshared type T = U\nshared type U = T\ndef combine left right => left\ndef of value => value\n'
} >"$workdir/33i2/main.hd"

# 33-i (3) 負の対照: 印なしの type T = T も E602（変わらない）。
{
    printf 'def main = StringMonoid.of "a"\n\n'
    monoid_contract
    printf '\nimplement Monoid as StringMonoid\ntype T = T\ndef combine left right => left\ndef of value => value\n'
} >"$workdir/33i3/main.hd"

# 33-0 構文受理: main の無い .hd はライブラリとして受理。shared 実体化があっても
# 抽象型が main に出ないので受理し、stdout / stderr は空。
{
    monoid_contract
    printf '\n'
    monoid_realization shared
} >"$workdir/33accept/main.hd"

# --- 工程 2: functor の shared type T = M.T と契約修飾型 X.T ---

# 契約 Monoid と Fold。functor MakeFold の実体化を functor_body が作る。
functor_contracts() {
    printf 'module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\ndef empty: T Pure\n\n'
    printf 'module Fold\ntype T\ndef fold(values: T List): T Pure\n'
}
functor_body() {
    printf '\nimplement Fold as MakeFold(M: Monoid)\n%s type T = M.T\ndef fold values => List.fold values M.empty M.combine\n' "$1"
}
string_monoid() {
    printf 'implement Monoid as StringMonoid\nshared type T = String\ndef combine left right => String.concat left right\ndef of value => value\ndef empty = ""\n'
}
opaque_monoid() {
    printf 'implement Monoid as OpaqueMonoid\ntype T = String\ndef combine left right => String.concat left right\ndef of value => value\ndef empty = ""\n'
}

mkdir -p "$workdir/33a1" "$workdir/33a3" "$workdir/33a1b" "$workdir/33a1bneg" \
    "$workdir/33b1" "$workdir/33b1b" "$workdir/33g1" "$workdir/33g2" "$workdir/33g3"

# 33-a(1): shared functor を shared な引数に適用し、String の List を fold する。
{
    printf 'def main = StringFold.fold ["%s", "%s"]\n' "$rand_a" "$rand_b"
    functor_contracts; string_monoid
    functor_body 'shared'
    printf 'implement Fold as StringFold = MakeFold(StringMonoid)\n'
} >"$workdir/33a1/main.hd"

# 33-a(3): 同じ適用で fold に StringMonoid.of の値を渡す。
{
    printf 'def main = StringFold.fold [StringMonoid.of "%s", StringMonoid.of "%s"]\n' "$rand_a" "$rand_b"
    functor_contracts; string_monoid
    functor_body 'shared'
    printf 'implement Fold as StringFold = MakeFold(StringMonoid)\n'
} >"$workdir/33a3/main.hd"

# 33-a(1b): shared functor を印なしの引数に適用する。OpaqueFold.T は OpaqueMonoid.T と
# 同一なので、OpaqueMonoid.of の値を fold が受け取れる（E305 が出ない）。印なしの T は
# main から返せないので E705 になる（正の対照は E305 の不在で測る）。
{
    printf 'def main = OpaqueFold.fold [OpaqueMonoid.of "%s"]\n' "$rand_a"
    functor_contracts; opaque_monoid
    functor_body 'shared'
    printf 'implement Fold as OpaqueFold = MakeFold(OpaqueMonoid)\n'
} >"$workdir/33a1b/main.hd"

# 33-a(1b) 負の対照: 印なしの引数の T に String は渡せない。
{
    printf 'def main = OpaqueFold.fold ["%s"]\n' "$rand_a"
    functor_contracts; opaque_monoid
    functor_body 'shared'
    printf 'implement Fold as OpaqueFold = MakeFold(OpaqueMonoid)\n'
} >"$workdir/33a1bneg/main.hd"

# 33-b(1): 印なしの type T = M.T を 2 回適用すると別の抽象型（A.read (B.make 7) が E305）。
{
    printf 'def main = A.read (B.make 7)\n'
    printf 'module Id\ntype T\ndef make(x: Int): T Pure\ndef read(x: T): Int Pure\n\n'
    printf 'module Box\ntype T\ndef make(x: Int): T Pure\ndef read(x: T): Int Pure\n\n'
    printf 'implement Id as I\ntype T = Int\ndef make x => x\ndef read x => x\n\n'
    printf 'implement Id as J\ntype T = Int\ndef make x => x\ndef read x => x\n\n'
    printf 'implement Box as Make(Arg: Id)\ntype T = Arg.T\ndef make x => Arg.make x\ndef read x => Arg.read x\n\n'
    printf 'implement Box as A = Make(I)\nimplement Box as B = Make(J)\n'
} >"$workdir/33b1/main.hd"

# 33-b(1b): 印なしの type T = M.T を shared な引数に適用すると、適用結果の T は
# 引数の T に解けず不透明のまま（StringFold.fold ["a"] が E305、StringFold.T と String）。
{
    printf 'def main = StringFold.fold ["%s"]\n' "$rand_a"
    functor_contracts; string_monoid
    functor_body ''
    printf 'implement Fold as StringFold = MakeFold(StringMonoid)\n'
} >"$workdir/33b1b/main.hd"

# 33-g: 契約修飾型 X.T。契約 Store の member の型 Id.T は唯一の実装 IntId の
# shared type T = Int で解け、Int を直接渡せる。
{
    printf 'def main = Store.find %s\n' "$rand_int"
    printf 'module Id\ntype T\ndef make(n: Int): T Pure\n\n'
    printf 'module Store\ndef find(id: Id.T): Int Pure\n\n'
    printf 'implement Id as IntId\nshared type T = Int\ndef make n => n\n\n'
    printf 'implement Store as Store\ndef find id => id\n'
} >"$workdir/33g1/main.hd"

# 33-g (2): 実装が 2 つなら E603（変わらない）。
{
    printf 'def main = Store.find %s\n' "$rand_int"
    printf 'module Id\ntype T\ndef make(n: Int): T Pure\n\n'
    printf 'module Store\ndef find(id: Id.T): Int Pure\n\n'
    printf 'implement Id as IntId\nshared type T = Int\ndef make n => n\n\n'
    printf 'implement Id as OtherId\nshared type T = Int\ndef make n => n\n\n'
    printf 'implement Store as Store\ndef find id => id\n'
} >"$workdir/33g2/main.hd"

# 33-g (3): 唯一の実装が印なしなら Id.T は不透明で、Int を渡すと E305。
{
    printf 'def main = Store.find %s\n' "$rand_int"
    printf 'module Id\ntype T\ndef make(n: Int): T Pure\n\n'
    printf 'module Store\ndef find(id: Id.T): Int Pure\n\n'
    printf 'implement Id as IntId\ntype T = Int\ndef make n => n\n\n'
    printf 'implement Store as Store\ndef find id => id\n'
} >"$workdir/33g3/main.hd"

for probe in "$workdir/33c1/main.hd" "$workdir/33c2/main.hd" "$workdir/33c3/main.hd" "$workdir/33c3neg/main.hd" \
    "$workdir/33c4/main.hd" "$workdir/33c5/main.hd" "$workdir/33c6/main.hd" \
    "$workdir/33a2/main.hd" "$workdir/33b2/main.hd" "$workdir/33d/main.hd" "$workdir/33dneg/main.hd" \
    "$workdir/33e/main.hd" "$workdir/33eneg/main.hd" "$workdir/33f2/main.hd" "$workdir/33h/main.hd" \
    "$workdir/33i1/main.hd" "$workdir/33i2/main.hd" "$workdir/33i3/main.hd" "$workdir/33accept/main.hd" \
    "$workdir/33a1/main.hd" "$workdir/33a3/main.hd" "$workdir/33a1b/main.hd" "$workdir/33a1bneg/main.hd" \
    "$workdir/33b1/main.hd" "$workdir/33b1b/main.hd" "$workdir/33g1/main.hd" "$workdir/33g2/main.hd" "$workdir/33g3/main.hd"; do
    random_padding "$probe"
done

expect_error_at '33-c (1) ADT 本体への shared は E100、| の位置' "$workdir/33c1/main.hd" E100 '| A'
expect_error_at '33-c (2) record 本体への shared は E100、{ の位置' "$workdir/33c2/main.hd" E100 '{ x'
expect_error_at '33-c (3) 契約に無い型への shared は E204、型名の位置' "$workdir/33c3/main.hd" E204 'U = Int'
expect_not_code '33-c (3) 負の対照 (契約が U を宣言)' "$workdir/33c3neg/main.hd" E204
expect_error_at '33-c (4) 外の shared は E100' "$workdir/33c4/main.hd" E100 'shared'
expect_error_at '33-c (5) 契約の中の shared は E100' "$workdir/33c5/main.hd" E100 'shared'
expect_error_at '33-c (6) shared の後の def は E100' "$workdir/33c6/main.hd" E100 'def f'

accept_build_run '33-a(2) shared で combine が String に解け実行値が一致' "$workdir/33a2/main.hd" "${rand_a}${rand_b}"
expect_code_message '33-b(2) 印なしは E305 で StringMonoid.T が出る' "$workdir/33b2/main.hd" E305 'StringMonoid.T'
accept_build_run '33-d shared で E705 が緩和され整数を印字' "$workdir/33d/main.hd" "$rand_int"
expect_error_at '33-d 緩和なしは E705 で def main' "$workdir/33dneg/main.hd" E705 'def main'
accept_build_run '33-e shared で E601 が緩和され field が読める' "$workdir/33e/main.hd" "$rand_name"
expect_error_at '33-e 緩和なしは E601 で field の .' "$workdir/33eneg/main.hd" E601 '.name'
print_type_is '33-f (1) --print-type が String を出す' "$workdir/33a2/main.hd" 'String'
expect_code_absent '33-f (2) E305 の両辺に .T が現れない' "$workdir/33f2/main.hd" E305 '.T'
accept_build_run '33-h 型引数付き shared で push と注釈が通る' "$workdir/33h/main.hd" '[2, 1]'
expect_error_at '33-i (1) shared type T = T は E602' "$workdir/33i1/main.hd" E602 'implement'
expect_error_at '33-i (2) shared の相互参照は E602' "$workdir/33i2/main.hd" E602 'implement'
expect_error_at '33-i (3) 負の対照 (印なし type T = T) は E602' "$workdir/33i3/main.hd" E602 'implement'
accept_quiet '33-0 構文受理 (main 無しのライブラリ、stdout/stderr 空)' "$workdir/33accept/main.hd"

accept_build_run '33-a(1) shared functor の fold が String List を取る' "$workdir/33a1/main.hd" "${rand_a}${rand_b}"
accept_build_run '33-a(3) shared functor の fold が of の String を取る' "$workdir/33a3/main.hd" "${rand_a}${rand_b}"
expect_code_absent '33-a(1b) 正の対照: fold が印なしの引数の T を取り、引数に E305 は出ない' "$workdir/33a1b/main.hd" E705 'E305'
expect_code '33-a(1b) 負の対照: 印なしの引数の T に String は渡せない' "$workdir/33a1bneg/main.hd" E305
expect_code '33-b(1) 印なし functor の 2 適用は別の抽象型' "$workdir/33b1/main.hd" E305
expect_code_message '33-b(1b) 印なし functor を shared 引数に適用すると T は不透明' "$workdir/33b1b/main.hd" E305 'StringFold.T'
accept_build_run '33-g(1) 契約修飾型 Id.T が shared で Int に解ける' "$workdir/33g1/main.hd" "$rand_int"
expect_error_at '33-g(2) Id.T の実装が 2 つなら E603' "$workdir/33g2/main.hd" E603 'Id.T'
expect_code '33-g(3) 印なしの実装では Int を Id.T に渡せない' "$workdir/33g3/main.hd" E305
print_type_is '33-f(3) shared functor の適用結果が String と印字' "$workdir/33a1/main.hd" 'String'

# 33-j: 構文の往復と生成。r9_grammar が shared を生成して往復し、keywords は緑のまま。
grammar_and_keywords() {
    cargo test --offline -q --test r9_grammar >/dev/null 2>&1 || return 1
    cargo test --offline -q --test r23_syntax >/dev/null 2>&1 || return 1
}
check '33-j (r9_grammar と r23_syntax が緑)' grammar_and_keywords

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
