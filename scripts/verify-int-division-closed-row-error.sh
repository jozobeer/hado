#!/usr/bin/env bash
# int-division-closed-row-error: 整数除算の Error が、注釈の閉じた効果行から
# 落ちないことを測る（§19 は / と % を Int {Error} とし、§20 は書いた行を
# その通りとする）。
#
#   a. Int Pure の / は E305。閉じた行に Error が無いので通してはならない。
#   b. Int Pure の % も E305。
#   c. Int {Io} の / も E305（行に無い効果）。
#   d. 被演算子に注釈を付けた (x: Int, y: Int): Int Pure も E305。
#   e. Int {Error} は通り、f 8 2 は 4、f 1 0 は DivisionByZero を出す。
#   f. 行を書かない形は Error を推論する（--print-type が {Error} を出す）。
#
# 期待値は仕様の節と入力そのものから決まる（a〜d は拒否の理由が 1 つだけなので
# 先頭の診断コードを見る。e・f は受理の側）。
#
# tier: quick
# round: int-division-closed-row-error
set -uo pipefail
cd "$(dirname "$0")/.."

pass=0
total=6
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1${2:+ ($2)}"; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado_bin="$target_dir/debug/hado"

status=0
stdout=''
stderr=''
# run <名前> <ソース> <サブコマンド> はソースを 1 本のファイルとして走らせ、
# stdout・stderr・status をグローバルに置く。ソースの改行は %b で戻す。
run() {
    local name="$1" source="$2" sub="$3"
    local dir="$work/$name"
    mkdir -p "$dir"
    printf '%b' "$source" >"$dir/main.hd"
    "$hado_bin" "$sub" "$dir/main.hd" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

# rejects_with <名前> <ソース> <コード> は check がそのコードで止まることを見る。
rejects_with() {
    local name="$1" source="$2" code="$3"
    run "$name" "$source" check || return 1
    [ "$status" -eq 1 ] || { echo "  status=$status" >&2; return 1; }
    grep -qF "error[$code]" <<<"$stderr" || { echo "  stderr=$stderr" >&2; return 1; }
}
check() {
    local label="$1"
    shift
    if "$@"; then
        ok "$label"
    else
        ng "$label"
    fi
}

# --- a. Pure の / は E305 ---
pure_div_is_rejected() {
    rejects_with pure-div 'def f x y: Int Pure => x / y\ndef main = f 1 1\n' E305
}
check 'a: Pure division is E305' pure_div_is_rejected

# --- b. Pure の % も E305 ---
pure_mod_is_rejected() {
    rejects_with pure-mod 'def f x y: Int Pure => x % y\ndef main = f 1 1\n' E305
}
check 'b: Pure modulo is E305' pure_mod_is_rejected

# --- c. {Io} の / も E305 ---
closed_row_without_error_is_rejected() {
    rejects_with io-div 'def f x y: Int {Io} => x / y\ndef main = f 1 1\n' E305
}
check 'c: a closed row without Error is E305' closed_row_without_error_is_rejected

# --- d. 注釈つきの被演算子も E305 ---
annotated_operands_are_rejected() {
    rejects_with annotated 'def f (x: Int, y: Int): Int Pure => x / y\ndef main = f 1 1\n' E305
}
check 'd: annotated operands are E305' annotated_operands_are_rejected

# --- e. {Error} は通り、除算の端が走る ---
error_row_is_accepted_and_runs() {
    run error-div 'def f x y: Int {Error} => x / y\ndef main = f 8 2\n' check || return 1
    [ "$status" -eq 0 ] || { echo "  check status=$status" >&2; return 1; }
    [ -z "$stdout$stderr" ] || { echo "  check out=$stdout$stderr" >&2; return 1; }
    run error-div-run 'def f x y: Int {Error} => x / y\ndef main = f 8 2\n' run || return 1
    [ "$status" -eq 4 ] || { echo "  run status=$status" >&2; return 1; }
    run error-div-zero 'def f x y: Int {Error} => x / y\ndef main = f 1 0\n' run || return 1
    [ "$status" -eq 1 ] || { echo "  zero status=$status" >&2; return 1; }
    grep -qF 'DivisionByZero' <<<"$stderr" || { echo "  zero stderr=$stderr" >&2; return 1; }
}
check 'e: an {Error} row runs the division' error_row_is_accepted_and_runs

# --- f. 行を書かない形は Error を推論する ---
open_row_infers_error() {
    local dir="$work/open-row"
    mkdir -p "$dir"
    printf 'def f x y => x / y\ndef main = f 8 2\n' >"$dir/main.hd"
    "$hado_bin" check --print-type=f "$dir/main.hd" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    grep -qF 'Fn(Int, Int => Int {Error})' <<<"$stdout" || { echo "  stdout=$stdout" >&2; return 1; }
}
check 'f: an open row infers Error' open_row_infers_error

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
