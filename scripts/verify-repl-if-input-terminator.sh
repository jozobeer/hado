#!/usr/bin/env bash
# repl-if-input-terminator: REPL が入力を未完と見る境界が、閉じのキーワードを
# 開きに数えないことを測る（§11 の 1 入力 1 歩、§12.1 の閉じの規則）。
#
#   a. 関数の中の `/if` は `fn` の閉じではない。`/if` で入力を割ると、
#      `/fn` より後ろが別の式になり、束縛した関数は腕の中の効果を走らせない。
#   b. `/fn` は開いた `fn` を閉じる。閉じた関数を `as` で束縛して次の入力から使える。
#   c. `handler` は `/handler` で閉じない。開きに数えると、見出しのあとの式が
#      EOF まで同じ未完の入力に吸い込まれ、評価されない。
#   d. `effect` も `/effect` で閉じない。同じく見出しのあとの式を吸い込まない。
#   e. `module` は `/module` で閉じる。閉じの `module` を開きに数えない。
#   f. `implement` は `/implement` で閉じる。閉じの `implement` を開きに数えない。
#
# 期待値は入力そのものから決まる（定義の入力は段階 1 では断られるので、
# その次の式が評価されることを見る）。
#
# tier: quick
# round: repl-if-input-terminator
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

# `session <名前> <入力の列>` は入力の列をパイプで与え、stdout/stderr/status を
# グローバルに置く。標準入力は端末ではないので前口上は出ない。
status=0
stdout=''
stderr=''
session() {
    local name="$1" script="$2"
    local dir="$work/$name"
    mkdir -p "$dir"
    printf '%b' "$script" >"$dir/in"
    "$hado_bin" repl <"$dir/in" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

# --- a. `/if` は `fn` の閉じではない ---
if_close_does_not_cut_a_function() {
    session if-close 'fn x =>\n  if x == 0 => log "z" -> 0 | x\n/if\n/fn as f ->\nf 1\nf 0\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "$(printf '1\nz\n0')" ] || { echo "  stdout=$stdout" >&2; return 1; }
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
check 'a: /if does not cut an open fn' if_close_does_not_cut_a_function

# --- b. `/fn` は開いた `fn` を閉じる ---
fn_close_finishes_the_function() {
    session fn-close 'fn x =>\n  log "a" -> x + 1\n/fn as f ->\nf 2\nf 3\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    # 効果付きの関数を 2 回呼ぶ。`/fn` で入力が割れなければ 1 回目と 2 回目が
    # 別の入力として走り、`log` は呼び出しごとに出る。
    [ "$stdout" = "$(printf 'a\n3\na\n4')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}
check 'b: /fn closes an open fn' fn_close_finishes_the_function

# --- c. `handler` 見出しは閉じを待たない ---
handler_heading_does_not_swallow() {
    session handler 'handler Error as Catch\n1\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "1" ] || { echo "  stdout=$stdout" >&2; return 1; }
    case "$stderr" in
        *定義の入力*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}
check 'c: a handler heading does not swallow the next input' handler_heading_does_not_swallow

# --- d. `effect` 見出しは閉じを待たない ---
effect_heading_does_not_swallow() {
    session effect 'effect E\n1\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "1" ] || { echo "  stdout=$stdout" >&2; return 1; }
    case "$stderr" in
        *定義の入力*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}
check 'd: an effect heading does not swallow the next input' effect_heading_does_not_swallow

# --- e. `/module` は `module` を閉じる ---
module_close_finishes_the_block() {
    session module-close 'module M\n/module\n1\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "1" ] || { echo "  stdout=$stdout" >&2; return 1; }
    case "$stderr" in
        *定義の入力*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}
check 'e: /module closes a module block' module_close_finishes_the_block

# --- f. `/implement` は `implement` を閉じる ---
implement_close_finishes_the_block() {
    session implement-close 'implement M as M\n/implement\n1\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "1" ] || { echo "  stdout=$stdout" >&2; return 1; }
    case "$stderr" in
        *定義の入力*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}
check 'f: /implement closes an implement block' implement_close_finishes_the_block

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
