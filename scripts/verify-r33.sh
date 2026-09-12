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

# 共通の探針。契約 Monoid と実装 StringMonoid。
monoid_contract() {
    printf 'module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\n'
}
monoid_implementation() {
    printf 'implement Monoid as StringMonoid\nshared type T = String\ndef combine left _ => left\ndef of value => value\n'
}

mkdir -p "$workdir/33c1" "$workdir/33c2" "$workdir/33c3" "$workdir/33c3neg" "$workdir/33c4" "$workdir/33c5" "$workdir/33c6" \
    "$workdir/33o1" "$workdir/33o2" "$workdir/33accept"

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

# 33-0 意味なし: 工程 0 では shared は意味を持たない。combine "a" "b" は E305 のまま。
{
    printf 'def main = StringMonoid.combine "a" "b"\n\n'
    monoid_contract
    printf '\n'
    monoid_implementation
} >"$workdir/33o1/main.hd"

# 33-0 受理は工程 1: 工程 0 では of の結果は不透明のままなので E705。
# （発注書の表は受理を期待するが、発注書の本文「この工程では shared は意味を持たない」と
#   手順書 26 節の工程 0 に従い、工程 0 の実測として E705 を固定する。工程 1 で受理に書き換える。）
{
    printf 'def main = StringMonoid.of "a"\n\n'
    monoid_contract
    printf '\n'
    monoid_implementation
} >"$workdir/33o2/main.hd"

# 33-0 構文受理: main の無い .hd はライブラリとして受理。shared 実体化があっても
# 抽象型が main に出ないので受理し、stdout / stderr は空。
{
    monoid_contract
    printf '\n'
    monoid_implementation
} >"$workdir/33accept/main.hd"

for probe in "$workdir/33c1/main.hd" "$workdir/33c2/main.hd" "$workdir/33c3/main.hd" "$workdir/33c3neg/main.hd" \
    "$workdir/33c4/main.hd" "$workdir/33c5/main.hd" "$workdir/33c6/main.hd" "$workdir/33o1/main.hd" "$workdir/33o2/main.hd" \
    "$workdir/33accept/main.hd"; do
    random_padding "$probe"
done

expect_error_at '33-c (1) ADT 本体への shared は E100、| の位置' "$workdir/33c1/main.hd" E100 '| A'
expect_error_at '33-c (2) record 本体への shared は E100、{ の位置' "$workdir/33c2/main.hd" E100 '{ x'
expect_error_at '33-c (3) 契約に無い型への shared は E204、型名の位置' "$workdir/33c3/main.hd" E204 'U = Int'
expect_not_code '33-c (3) 負の対照 (契約が U を宣言)' "$workdir/33c3neg/main.hd" E204
expect_error_at '33-c (4) 外の shared は E100' "$workdir/33c4/main.hd" E100 'shared'
expect_error_at '33-c (5) 契約の中の shared は E100' "$workdir/33c5/main.hd" E100 'shared'
expect_error_at '33-c (6) shared の後の def は E100' "$workdir/33c6/main.hd" E100 'def f'
expect_code '33-0 意味なし (combine は E305 のまま)' "$workdir/33o1/main.hd" E305
expect_code '33-0 受理は工程 1 (of は E705 のまま)' "$workdir/33o2/main.hd" E705
accept_quiet '33-0 構文受理 (main 無しのライブラリ、stdout/stderr 空)' "$workdir/33accept/main.hd"

# 33-j: 構文の往復と生成。r9_grammar が shared を生成して往復し、keywords は緑のまま。
grammar_and_keywords() {
    cargo test --offline -q --test r9_grammar >/dev/null 2>&1 || return 1
    cargo test --offline -q --test r23_syntax >/dev/null 2>&1 || return 1
}
check '33-j (r9_grammar と r23_syntax が緑)' grammar_and_keywords

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
