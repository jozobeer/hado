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

# code の診断が出て、needles を全て含み、forbidden を含まない。
# 引数: label entry code forbidden needle...
expect_code_contains() {
    local label=$1 entry=$2 code=$3 forbidden=$4
    shift 4
    total=$((total + 1))
    local err status ok=1 needle
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    [ "$status" -eq 1 ] && [[ "$err" == *"error[$code]:"* ]] || ok=0
    for needle in "$@"; do
        [[ "$err" == *"$needle"* ]] || ok=0
    done
    [ -z "$forbidden" ] || { [[ "$err" != *"$forbidden"* ]] || ok=0; }
    if [ "$ok" -eq 1 ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want $code with '$*' without '$forbidden', got status=$status '$err')"
    fi
}

# code の診断が、許容コードのいずれかで出る（負の対照がどの欠陥コードで落ちるかは
# 問わないが、終了 1 と型の診断であることは要求する）。
expect_code_one_of() {
    local label=$1 entry=$2
    shift 2
    total=$((total + 1))
    local err status ok=0 code
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    if [ "$status" -eq 1 ]; then
        for code in "$@"; do
            [[ "$err" == *"error[$code]:"* ]] && ok=1
        done
    fi
    if [ "$ok" -eq 1 ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want one of $*, got status=$status '$err')"
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
rand_box="w${RANDOM}"
rand_other=$(( (RANDOM << 15) | RANDOM ))
rand_rec="r${RANDOM}"
rand_pat="p${RANDOM}"
rand_named="m${RANDOM}"
rand_proj="j${RANDOM}"
rand_ip="i${RANDOM}"
rand_rp="q${RANDOM}"
rand_eff="e${RANDOM}"
rand_harm="h${RANDOM}"
rand_fbody="f${RANDOM}"
rand_fe_name="g${RANDOM}"
rand_fe_prefix="p${RANDOM}"
rand_fe_suffix="s${RANDOM}"

mkdir -p "$workdir/33c1" "$workdir/33c2" "$workdir/33c3" "$workdir/33c3neg" "$workdir/33c4" "$workdir/33c5" "$workdir/33c6" \
    "$workdir/33a2" "$workdir/33b2" "$workdir/33d" "$workdir/33dneg" "$workdir/33e" "$workdir/33eneg" \
    "$workdir/33f2" "$workdir/33h" "$workdir/33i1" "$workdir/33i2" "$workdir/33i3" "$workdir/33accept" \
    "$workdir/33wrap" "$workdir/33wrapneg" "$workdir/33dist" "$workdir/33distpos" "$workdir/33lrec" "$workdir/33lrecneg" \
    "$workdir/33ident" "$workdir/33cyc" "$workdir/33pat" "$workdir/33patneg" "$workdir/33proj" "$workdir/33projneg" \
    "$workdir/33named" "$workdir/33namedneg" \
    "$workdir/33ip" "$workdir/33ipneg" "$workdir/33rp" "$workdir/33rpneg" \
    "$workdir/33eff" "$workdir/33effneg" "$workdir/33harm" "$workdir/33harmneg" \
    "$workdir/33fn" "$workdir/33fnneg" "$workdir/33fneff" "$workdir/33fneffneg"

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
    printf 'module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\ndef empty: T Pure\ndef show(value: T): String Pure\n\n'
    printf 'module Fold\ntype T\ndef fold(values: T List): T Pure\n'
}
functor_body() {
    printf '\nimplement Fold as MakeFold(M: Monoid)\n%s type T = M.T\ndef fold values => List.fold values M.empty M.combine\n' "$1"
}
string_monoid() {
    printf 'implement Monoid as StringMonoid\nshared type T = String\ndef combine left right => String.concat left right\ndef of value => value\ndef empty = ""\ndef show value => value\n'
}
opaque_monoid() {
    printf 'implement Monoid as OpaqueMonoid\ntype T = String\ndef combine left right => String.concat left right\ndef of value => value\ndef empty = ""\ndef show value => value\n'
}

mkdir -p "$workdir/33a1" "$workdir/33a1c" "$workdir/33a3" "$workdir/33a1b" "$workdir/33a1bneg" \
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

# 33-a(1c): 適用結果の抽象型 StringFold.T は引数の shared に従い String に解ける。
# 注釈 StringFold.T に String を渡せることで測る（適用の写しで shared の登録を
# 落とすと、StringFold.T が不透明に戻り E305 になる）。
{
    printf 'def main = squash (StringFold.fold ["%s", "%s"])\ndef squash (x: StringFold.T): String => x\n' "$rand_a" "$rand_b"
    functor_contracts; string_monoid
    functor_body 'shared'
    printf 'implement Fold as StringFold = MakeFold(StringMonoid)\n'
} >"$workdir/33a1c/main.hd"

# 33-a(1b): shared functor を印なしの引数に適用する。OpaqueFold.T は OpaqueMonoid.T と
# 同一なので、OpaqueMonoid.of の値が fold を通り、実装の公開関数 show で String に戻って
# 実行値が一致する（印なしの T は main から直接は返せないので公開関数を挟む）。
{
    printf 'def main = OpaqueMonoid.show (OpaqueFold.fold [OpaqueMonoid.of "%s"])\n' "$rand_a"
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

# 33-a(1b) 同一性: OpaqueFold.T が OpaqueMonoid.T と同一であることを、両者の間の
# 注釈付き関数 squash を挟んでも通ることで測る。適用で等式を登録し落とすと、2 つの
# 抽象型が揃わず squash の注釈で E305 になる（OpaqueMonoid.of を直接 fold に渡す
# だけでは型変数の単一化で隠れるため、注釈で固定する）。
{
    printf 'def main = OpaqueMonoid.show (squash (OpaqueFold.fold [OpaqueMonoid.of "%s"]))\ndef squash (x: OpaqueFold.T): OpaqueMonoid.T => x\n' "$rand_a"
    functor_contracts; opaque_monoid
    functor_body 'shared'
    printf 'implement Fold as OpaqueFold = MakeFold(OpaqueMonoid)\n'
} >"$workdir/33ident/main.hd"

# functor の shared 適用の輪: functor の shared 適用が輪になるときは E602 で止まる。循環防止を
# 外すと検査が停止せず SIGABRT になるため、終了コードと stderr の error[E602] の
# 両方を見る（expect_code は status=1 とコードの出現を要求する）。
{
    printf 'def main = A.read (A.make 7)\n'
    printf 'module Id\ntype T\ndef make(n: Int): T Pure\ndef read(value: T): Int Pure\n\n'
    printf 'implement Id as I\nshared type T = Int\ndef make n => n\ndef read value => value\n\n'
    printf 'implement Id as Make(Arg: Id)\nshared type T = Arg.T\ndef make n => Arg.make n\ndef read value => Arg.read value\n\n'
    printf 'implement Id as A = Make(B)\nimplement Id as B = Make(A)\n'
} >"$workdir/33cyc/main.hd"

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

# --- 工程 1 の修正: ADT payload・別実装の局所型・局所 record ---

# ADT payload の型を shared が展開する。外の ADT Box の constructor の payload が
# 契約修飾型 Store.T で、唯一の実装が shared type T = String のとき Box に String を
# 渡せる（探針 wrapper）。
{
    printf 'def main = Box("%s")\ntype Box = | Box(Store.T)\n' "$rand_box"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
} >"$workdir/33wrap/main.hd"

# 負の対照: shared を外すと payload は不透明のまま E305。
{
    printf 'def main = Box("%s")\ntype Box = | Box(Store.T)\n' "$rand_box"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
} >"$workdir/33wrapneg/main.hd"

# 別実装の局所型は混同されない。2 つの実装 A B が同名の局所型 Hidden を持ち、
# どちらも shared type T = Hidden でも A.T と B.T は別の型（探針 distinct）。
{
    printf 'def main = A.read (B.make %s)\n' "$rand_other"
    printf 'module Store\ntype T\ndef make(id: Int): T Pure\ndef read(value: T): Int Pure\n\n'
    printf 'implement Store as A\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id\n\n'
    printf 'implement Store as B\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id\n'
} >"$workdir/33dist/main.hd"

# 正の対照: 同じ実装の中では局所型は一致するので受理して実行できる。
{
    printf 'def main = A.read (A.make %s)\n' "$rand_int"
    printf 'module Store\ntype T\ndef make(id: Int): T Pure\ndef read(value: T): Int Pure\n\n'
    printf 'implement Store as A\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id\n'
} >"$workdir/33distpos/main.hd"

# 局所 record への shared は field 参照を許す（探針 localrecord）。
{
    printf 'def main = name (Store.make "%s")\ndef name (t: Store.T) => t.name\n' "$rand_rec"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype Hidden = { name: String }\nshared type T = Hidden\ndef make name => { name = name }\n'
} >"$workdir/33lrec/main.hd"

# 負の対照: shared を外すと局所 record でも field 参照は E601。
{
    printf 'def main = name (Store.make "%s")\ndef name (t: Store.T) => t.name\n' "$rand_rec"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype Hidden = { name: String }\ntype T = Hidden\ndef make name => { name = name }\n'
} >"$workdir/33lrecneg/main.hd"

# --- 工程 1 の修正 2: パターン payload と field 射影への shared の展開 ---

# パターン payload の型を shared が展開する。外の ADT Box の constructor の payload が
# 契約修飾型 Store.T で、唯一の実装が shared type T = String のとき、パターンで束縛した
# 値を String として使える（探針 pattern）。
{
    printf 'def main = read (Box(Store.make "%s"))\ndef read | Box(value) => String.concat value "!"\ntype Box = | Box(Store.T)\n' "$rand_pat"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
} >"$workdir/33pat/main.hd"

# 負の対照: shared を外すとパターンで束縛した payload は不透明のまま E305。
{
    printf 'def main = read (Box(Store.make "%s"))\ndef read | Box(value) => String.concat value "!"\ntype Box = | Box(Store.T)\n' "$rand_pat"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
} >"$workdir/33patneg/main.hd"

# field 射影の型を shared が展開する。record の field の型が契約修飾型 Store.T で、
# 唯一の実装が shared type T = String のとき、射影した値を String として使える（探針 projection）。
{
    printf 'def main = String.concat ({ name = Store.make "%s" }).name "!"\ntype Box = { name: Store.T }\n' "$rand_proj"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
} >"$workdir/33proj/main.hd"

# 負の対照: shared を外すと射影した field 型は不透明のまま E305。
{
    printf 'def main = String.concat ({ name = Store.make "%s" }).name "!"\ntype Box = { name: Store.T }\n' "$rand_proj"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
} >"$workdir/33projneg/main.hd"

# named constructor の構築とパターンの payload の型を shared が展開する。外の ADT Box の
# 名前付き constructor の payload が契約修飾型 Store.T で、唯一の実装が shared type T =
# String のとき、構築で String を渡せ、パターンで束縛した値を String として使える（探針 named）。
{
    printf 'def main = read (Box { value = Store.make "%s" })\ndef read\n| Box { value } => String.concat value "!"\ntype Box = | Box { value: Store.T }\n' "$rand_named"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
} >"$workdir/33named/main.hd"

# 負の対照: shared を外すと named constructor の payload は不透明のまま E305。
{
    printf 'def main = read (Box { value = Store.make "%s" })\ndef read\n| Box { value } => String.concat value "!"\ntype Box = | Box { value: Store.T }\n' "$rand_named"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
} >"$workdir/33namedneg/main.hd"

# --- 工程 1 の修正 4: 等式の収集を型と効果の登録より前に置く 4 形 ---
# いずれも「Store.T が型の位置に現れ、それを消費する定義が Store に構文的に依存しない」形。
# shared の等式が register_type / register_effects より前に入っていないと、record schema と
# 効果の操作型が不透明のまま登録され、推論・実行が通らない。

# 正 1 (inferred-projection): 未注釈の関数が record の field を読む。record schema 登録時に
# Store.T が解けていないと box.value が不透明のまま E305 になる（探針 inferred-projection）。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read box => String.concat box.value "!"\ntype Box = { value: Store.T }\n' "$rand_ip"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
} >"$workdir/33ip/main.hd"

# 負 1: shared を外すと record schema の payload は不透明のまま型検査が落ちる。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read box => String.concat box.value "!"\ntype Box = { value: Store.T }\n' "$rand_ip"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
} >"$workdir/33ipneg/main.hd"

# 正 2 (record-pattern): record パターンで束縛した値を String に使う。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read\n| { value = v } => String.concat v "!"\ntype Box = { value: Store.T }\n' "$rand_rp"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
} >"$workdir/33rp/main.hd"

# 負 2: shared を外すとパターンで束縛した payload は不透明のまま型検査が落ちる。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read\n| { value = v } => String.concat v "!"\ntype Box = { value: Store.T }\n' "$rand_rp"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
} >"$workdir/33rpneg/main.hd"

# 正 3 (effect): 効果 Send の操作 send の payload が Store.T。register_effects の時点で
# 等式が解けていないと payload が不透明になり、handler の腕の String.concat が E305。
{
    printf 'def main = with Handle -> send (Store.make "%s")\n' "$rand_eff"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
    printf 'effect Send\ncapture send(value: Store.T): String\n'
    printf 'handler Send as Handle\ncapture send value => resume (String.concat value "!")\n'
} >"$workdir/33eff/main.hd"

# 負 3: shared を外すと操作型の payload は不透明のまま handler の腕で落ちる。
{
    printf 'def main = with Handle -> send (Store.make "%s")\n' "$rand_eff"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
    printf 'effect Send\ncapture send(value: Store.T): String\n'
    printf 'handler Send as Handle\ncapture send value => resume (String.concat value "!")\n'
} >"$workdir/33effneg/main.hd"

# 正 4 (handler-arm): 効果 Ask の操作 get の結果型が Store.T。register_effects の時点で
# 等式が解けていないと結果型が不透明になり、腕の resume value が E305。
{
    printf 'def main = with Handle -> get (Store.make "%s")\n' "$rand_harm"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = String\ndef make name => name\n'
    printf 'effect Ask\ncapture get(value: String): Store.T\n'
    printf 'handler Ask as Handle\ncapture get value => resume value\n'
} >"$workdir/33harm/main.hd"

# 負 4: shared を外すと操作の結果型は不透明のまま腕の resume で落ちる。
{
    printf 'def main = with Handle -> get (Store.make "%s")\n' "$rand_harm"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = String\ndef make name => name\n'
    printf 'effect Ask\ncapture get(value: String): Store.T\n'
    printf 'handler Ask as Handle\ncapture get value => resume value\n'
} >"$workdir/33harmneg/main.hd"

# --- 工程 1 の修正 5: 関数型を本体とする shared の等式 ---
# record schema の field の型が関数型の抽象型 Store.T のとき、record 登録より前に
# 等式が解けていないと E305 になる。関数型と効果行も前処理で解く。

# 正 1 (function-body): 未注釈の関数が record の field の関数を呼ぶ。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read box => box.value "!"\ntype Box = { value: Store.T }\n' "$rand_fbody"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = Fn(String => String)\ndef make name => fn(suffix => String.concat name suffix)\n'
} >"$workdir/33fn/main.hd"

# 負 1: shared を外すと record schema の field は不透明のまま型検査が落ちる。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read box => box.value "!"\ntype Box = { value: Store.T }\n' "$rand_fbody"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = Fn(String => String)\ndef make name => fn(suffix => String.concat name suffix)\n'
} >"$workdir/33fnneg/main.hd"

# 正 2 (function-with-effects): 効果行を持つ関数型。効果は実際に起きる形にする
# (go は {Error} の parse を呼ぶ)。等式の効果行が解けていないと E301 / E305 になる。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read box => box.value "%s"\ntype Box = { value: Store.T }\n' "$rand_fe_name" "$rand_fe_suffix"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\nshared type T = Fn(String => String {Error})\ndef go (suffix: String): String {Error} => String.concat "%s" (parse suffix)\ndef parse (input: String): String {Error} => if input == "" => raise { message = "empty" } | input\ndef make _ => go\n' "$rand_fe_prefix"
} >"$workdir/33fneff/main.hd"

# 負 2: shared を外すと効果行付きの field は不透明のまま型検査が落ちる。
{
    printf 'def main = read ({ value = Store.make "%s" })\ndef read box => box.value "%s"\ntype Box = { value: Store.T }\n' "$rand_fe_name" "$rand_fe_suffix"
    printf 'module Store\ntype T\ndef make(name: String): T Pure\n\n'
    printf 'implement Store as Store\ntype T = Fn(String => String {Error})\ndef go (suffix: String): String {Error} => String.concat "%s" (parse suffix)\ndef parse (input: String): String {Error} => if input == "" => raise { message = "empty" } | input\ndef make _ => go\n' "$rand_fe_prefix"
} >"$workdir/33fneffneg/main.hd"

for probe in "$workdir/33c1/main.hd" "$workdir/33c2/main.hd" "$workdir/33c3/main.hd" "$workdir/33c3neg/main.hd" \
    "$workdir/33c4/main.hd" "$workdir/33c5/main.hd" "$workdir/33c6/main.hd" \
    "$workdir/33a2/main.hd" "$workdir/33b2/main.hd" "$workdir/33d/main.hd" "$workdir/33dneg/main.hd" \
    "$workdir/33e/main.hd" "$workdir/33eneg/main.hd" "$workdir/33f2/main.hd" "$workdir/33h/main.hd" \
    "$workdir/33i1/main.hd" "$workdir/33i2/main.hd" "$workdir/33i3/main.hd" "$workdir/33accept/main.hd" \
    "$workdir/33a1/main.hd" "$workdir/33a1c/main.hd" "$workdir/33a3/main.hd" "$workdir/33a1b/main.hd" "$workdir/33a1bneg/main.hd" \
    "$workdir/33b1/main.hd" "$workdir/33b1b/main.hd" "$workdir/33g1/main.hd" "$workdir/33g2/main.hd" "$workdir/33g3/main.hd" \
    "$workdir/33wrap/main.hd" "$workdir/33wrapneg/main.hd" "$workdir/33dist/main.hd" "$workdir/33distpos/main.hd" \
    "$workdir/33lrec/main.hd" "$workdir/33lrecneg/main.hd" "$workdir/33ident/main.hd" "$workdir/33cyc/main.hd" \
    "$workdir/33pat/main.hd" "$workdir/33patneg/main.hd" "$workdir/33proj/main.hd" "$workdir/33projneg/main.hd" \
    "$workdir/33named/main.hd" "$workdir/33namedneg/main.hd" \
    "$workdir/33ip/main.hd" "$workdir/33ipneg/main.hd" "$workdir/33rp/main.hd" "$workdir/33rpneg/main.hd" \
    "$workdir/33eff/main.hd" "$workdir/33effneg/main.hd" "$workdir/33harm/main.hd" "$workdir/33harmneg/main.hd" \
    "$workdir/33fn/main.hd" "$workdir/33fnneg/main.hd" "$workdir/33fneff/main.hd" "$workdir/33fneffneg/main.hd"; do
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
expect_code_contains '33-f (2) E305 は cannot unify と String / Int を診断し .T を含まない' "$workdir/33f2/main.hd" E305 '.T' 'cannot unify' 'String' 'Int'
accept_build_run '33-h 型引数付き shared で push と注釈が通る' "$workdir/33h/main.hd" '[2, 1]'
expect_error_at '33-i (1) shared type T = T は E602' "$workdir/33i1/main.hd" E602 'implement'
expect_error_at '33-i (2) shared の相互参照は E602' "$workdir/33i2/main.hd" E602 'implement'
expect_error_at '33-i (3) 負の対照 (印なし type T = T) は E602' "$workdir/33i3/main.hd" E602 'implement'
accept_quiet '33-0 構文受理 (main 無しのライブラリ、stdout/stderr 空)' "$workdir/33accept/main.hd"

accept_build_run '33-a(1) shared functor の fold が String List を取る' "$workdir/33a1/main.hd" "${rand_a}${rand_b}"
accept_build_run '33-a(1c) 適用結果の StringFold.T が引数の shared で String に解ける' "$workdir/33a1c/main.hd" "${rand_a}${rand_b}"
accept_build_run '33-a(3) shared functor の fold が of の String を取る' "$workdir/33a3/main.hd" "${rand_a}${rand_b}"
accept_build_run '33-a(1b) shared functor を印なしの引数に通し公開関数で実行値が一致' "$workdir/33a1b/main.hd" "$rand_a"
expect_code '33-a(1b) 負の対照: 印なしの引数の T に String は渡せない' "$workdir/33a1bneg/main.hd" E305
accept_build_run '33-a(1b) 同一性: OpaqueFold.T を OpaqueMonoid.T に注釈で通す' "$workdir/33ident/main.hd" "$rand_a"
expect_code 'shared functor の適用の輪は E602 で止まる' "$workdir/33cyc/main.hd" E602
expect_code '33-b(1) 印なし functor の 2 適用は別の抽象型' "$workdir/33b1/main.hd" E305
expect_code_message '33-b(1b) 印なし functor を shared 引数に適用すると T は不透明' "$workdir/33b1b/main.hd" E305 'StringFold.T'
accept_build_run '33-g(1) 契約修飾型 Id.T が shared で Int に解ける' "$workdir/33g1/main.hd" "$rand_int"
expect_error_at '33-g(2) Id.T の実装が 2 つなら E603' "$workdir/33g2/main.hd" E603 'Id.T'
expect_code '33-g(3) 印なしの実装では Int を Id.T に渡せない' "$workdir/33g3/main.hd" E305
print_type_is '33-f(3) shared functor の適用結果が String と印字' "$workdir/33a1/main.hd" 'String'

# 工程 1 の修正: ADT payload・別実装の局所型・局所 record。
accept_build_run 'shared が ADT payload を展開し Box(String) を印字' "$workdir/33wrap/main.hd" "Box(\"$rand_box\")"
expect_code_message 'shared 無しの ADT payload は E305 で Store.T が出る' "$workdir/33wrapneg/main.hd" E305 'Store.T'
expect_code_contains '別実装の局所型 A.Hidden と B.Hidden は単一化できない' "$workdir/33dist/main.hd" E305 '' 'cannot unify' 'A.Hidden' 'B.Hidden'
accept_build_run '同じ実装の局所型は一致し整数を印字' "$workdir/33distpos/main.hd" "$rand_int"
accept_build_run 'shared が局所 record を展開し field を読める' "$workdir/33lrec/main.hd" "$rand_rec"
expect_code 'shared 無しの局所 record は E601' "$workdir/33lrecneg/main.hd" E601

# 工程 1 の修正 2: パターン payload と field 射影。
accept_build_run 'shared がパターン payload を展開し束縛値を String に使える' "$workdir/33pat/main.hd" "${rand_pat}!"
expect_code 'shared 無しのパターン payload は E305' "$workdir/33patneg/main.hd" E305
accept_build_run 'shared が field 射影を展開し値を String に使える' "$workdir/33proj/main.hd" "${rand_proj}!"
expect_code 'shared 無しの field 射影は E305' "$workdir/33projneg/main.hd" E305

accept_build_run 'shared が named constructor の payload を展開し束縛値を String に使える' "$workdir/33named/main.hd" "${rand_named}!"
expect_code 'shared 無しの named constructor の payload は E305' "$workdir/33namedneg/main.hd" E305

# 工程 1 の修正 4: 等式の収集が登録より前にあることを 4 形で測る。
accept_build_run '等式の前処理: 未注釈関数が record の field を読める' "$workdir/33ip/main.hd" "${rand_ip}!"
expect_code_one_of '前処理の負 1: shared 無しの record field は型検査で落ちる' "$workdir/33ipneg/main.hd" E305 E601 E705
accept_build_run '等式の前処理: record パターンで束縛した値を使える' "$workdir/33rp/main.hd" "${rand_rp}!"
expect_code_one_of '前処理の負 2: shared 無しの record パターンは型検査で落ちる' "$workdir/33rpneg/main.hd" E305 E601 E705
accept_build_run '等式の前処理: 効果の操作 payload が解ける' "$workdir/33eff/main.hd" "${rand_eff}!"
expect_code_one_of '前処理の負 3: shared 無しの効果の操作 payload は型検査で落ちる' "$workdir/33effneg/main.hd" E305 E601 E705
accept_build_run '等式の前処理: handler の腕で操作の結果型が解ける' "$workdir/33harm/main.hd" "$rand_harm"
expect_code_one_of '前処理の負 4: shared 無しの handler の腕は型検査で落ちる' "$workdir/33harmneg/main.hd" E305 E601 E705

# 工程 1 の修正 5: 関数型を本体とする shared の等式を前処理で解く。
accept_build_run '前処理: 関数型の shared が record の field に届き呼べる' "$workdir/33fn/main.hd" "${rand_fbody}!"
expect_code '前処理の負: 関数型の shared 無しは型検査で落ちる' "$workdir/33fnneg/main.hd" E305
accept_build_run '前処理: 効果行付きの関数型の shared が record の field に届く' "$workdir/33fneff/main.hd" "${rand_fe_prefix}${rand_fe_suffix}"
expect_code '前処理の負: 効果行付きの shared 無しは型検査で落ちる' "$workdir/33fneffneg/main.hd" E305

# 33-j: 構文の往復と生成。r9_grammar が shared を生成して往復し、keywords は緑のまま。
grammar_and_keywords() {
    cargo test --offline -q --test r9_grammar >/dev/null 2>&1 || return 1
    cargo test --offline -q --test r23_syntax >/dev/null 2>&1 || return 1
}
check '33-j (r9_grammar と r23_syntax が緑)' grammar_and_keywords

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
