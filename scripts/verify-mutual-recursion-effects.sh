#!/usr/bin/env bash
# mutual-recursion-effects: 相互再帰で片方の本体にだけ出る効果が、もう片方の
# 定義の効果の行にも届くことを測る。行を同一化せず、群の終わりにラベルだけを足す。
#
#   a. 割り算の Error が相手の h の行に届き、handler が捕まえて 42 になる。
#      main の実装では run が lowered return type differs で落ちていた。
#   b. 相手だけが出すユーザー効果は main まで届き、handler なしでは E706。
#      直す前は check が通り、実行時に unhandled operation ask になっていた。
#   c. 同じ呼び出しを handler の下で行うと Ask は main に残らず、腕の 3 が返る。
#   d. 書いた Pure の行は広がらず、E305 のまま（対照）。
#   e. 自己再帰の割り算は今までどおり 42 になる（対照）。
#   f. 書いた {Error} の行は今までどおり 42 になる（対照）。
#
# tier: quick
# round: mutual-recursion-effects
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
# `program <名前> <ソース>` は 1 つのソースを専用のディレクトリに書き、`run` の
# stdout/stderr/status をグローバルに置く。`check` は `checked <名前> <ソース>`。
program() {
    local name="$1" source="$2"
    local dir="$work/$name"
    mkdir -p "$dir"
    printf '%b' "$source" >"$dir/main.hd"
    "$hado_bin" run "$dir/main.hd" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

checked() {
    local name="$1" source="$2"
    local dir="$work/$name"
    mkdir -p "$dir"
    printf '%b' "$source" >"$dir/main.hd"
    "$hado_bin" check "$dir/main.hd" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

DIVISION='handler Error as Catch\nabort raise _ => 42\nreturn value => value\ndef rec h n => if n == 0 => g n | h (n - 1)\ndef rec g n => if n == 0 => 1 / n | h (n - 1)\ndef main = with Catch -> h 1\n'

# --- a. 相手の Error が行に届く ---
mutual_division_is_caught() {
    program division "$DIVISION" || return 1
    [ "$status" -eq 42 ] || { echo "  status=$status" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- b. 相手だけのユーザー効果は main まで届く ---
mutual_user_effect_reaches_main() {
    checked unhandled 'effect Ask\nonce ask: Int\ndef rec h n => if n == 0 => g n | h (n - 1)\ndef rec g n => if n == 0 => ask | h (n - 1)\ndef main = h 1\n' || return 1
    [ "$status" -eq 1 ] || { echo "  status=$status" >&2; return 1; }
    case "$stderr" in
        *'error[E706]'*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}

# --- c. handler の下では腕の値が返る ---
mutual_user_effect_is_handled() {
    program handled 'effect Ask\nonce ask: Int\nhandler Ask as H\nonce ask => 3\nreturn x => x\ndef rec h n => if n == 0 => g n | h (n - 1)\ndef rec g n => if n == 0 => ask | h (n - 1)\ndef main = with H -> h 1\n' || return 1
    [ "$status" -eq 3 ] || { echo "  status=$status" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- d. 書いた Pure の行は広がらない ---
pure_annotation_is_not_widened() {
    checked pure 'def rec h (n: Int): Int Pure => if n == 0 => g n | h (n - 1)\ndef rec g n => if n == 0 => 1 / n | h (n - 1)\ndef main = h 1\n' || return 1
    [ "$status" -eq 1 ] || { echo "  status=$status" >&2; return 1; }
    case "$stderr" in
        *'error[E305]'*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}

# --- e. 自己再帰は今までどおり ---
self_recursion_still_caught() {
    program self 'handler Error as Catch\nabort raise _ => 42\nreturn value => value\ndef rec h n => if n == 0 => 1 / n | h (n - 1)\ndef main = with Catch -> h 1\n' || return 1
    [ "$status" -eq 42 ] || { echo "  status=$status" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- f. 書いた {Error} の行は今までどおり ---
written_error_row_still_caught() {
    program annotated 'handler Error as Catch\nabort raise _ => 42\nreturn value => value\ndef rec h (n: Int): Int {Error} => if n == 0 => g n | h (n - 1)\ndef rec g n => if n == 0 => 1 / n | h (n - 1)\ndef main = with Catch -> h 1\n' || return 1
    [ "$status" -eq 42 ] || { echo "  status=$status" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
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
check 'a: the other function keeps the division error' mutual_division_is_caught
check 'b: the other function carries a user effect to main' mutual_user_effect_reaches_main
check 'c: a handled user effect from the other function returns its arm' mutual_user_effect_is_handled
check 'd: a written Pure row is not widened' pure_annotation_is_not_widened
check 'e: self recursion still catches the division error' self_recursion_still_caught
check 'f: a written Error row still catches the division error' written_error_row_still_caught

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
