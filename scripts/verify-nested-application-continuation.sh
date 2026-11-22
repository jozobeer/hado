#!/usr/bin/env bash
# nested-application-continuation: 効果付き閉包を返す呼び出しをその場で呼んでも、
# `as` の続きが resume の継続に載ることを測る。
#
#   a. `(get 0) 0 as n -> n + 10` は resume の値 1 に続きを足して 11 になる。
#      呼び手が適用だと効果を見逃し、続きを飛ばして 1 になっていた。
#   b. 先に束ねた `get 0 as f -> f 0 as n -> n + 10` も 11 になる（対照）。
#   c. 同じ入れ子の呼び出しで 8 / 0 が起きれば Error の腕が 0 を返す（対照）。
#   d. 効果を運ばない閉包の入れ子は、handler の下でも素の呼び出しのまま 14 になる（対照）。
#
# tier: quick
# round: nested-application-continuation
set -uo pipefail
cd "$(dirname "$0")/.."

pass=0
total=4
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
# stdout/stderr/status をグローバルに置く。読み込みは入力根の全 .hd を見るので、
# 検体ごとにディレクトリを分ける（§50）。
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

# 効果付き閉包を返す呼び出しを、束ねずにその場で呼ぶ。
NESTED='effect Choice\nagain pick: Int\nhandler Choice as C\nagain pick => resume 1\nreturn value => value\ndef get x => fn y =>\n  pick as n ->\n  n + x * 0 + y * 0\n/fn\ndef main =\n  with C ->\n  (get 0) 0 as n ->\n  n + 10\n'

# --- a. 入れ子の適用でも続きが走る ---
nested_application_keeps_the_continuation() {
    program nested "$NESTED" || return 1
    [ "$status" -eq 11 ] || { echo "  status=$status" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- b. 束ねた形は変わらない ---
bound_form_still_keeps_the_continuation() {
    program bound 'effect Choice\nagain pick: Int\nhandler Choice as C\nagain pick => resume 1\nreturn value => value\ndef get x => fn y =>\n  pick as n ->\n  n + x * 0 + y * 0\n/fn\ndef main =\n  with C ->\n  get 0 as f ->\n  f 0 as n ->\n  n + 10\n' || return 1
    [ "$status" -eq 11 ] || { echo "  status=$status" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- c. abort の腕は入れ子でも効く ---
abort_arm_through_the_nested_call() {
    program abort 'handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef get x => fn(y => if y == 0 => 8 / 0 | x + y)\n\ndef main =\n  with Catch ->\n  (get 0) 0 as n ->\n  n + 10\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- d. 効果を運ばない閉包は outcome にしない ---
pure_closure_stays_a_plain_call() {
    program pure 'effect Choice\nagain pick: Int\nhandler Choice as C\nagain pick => resume 1\nreturn value => value\ndef add x => fn(y => x + y)\n\ndef main =\n  with C ->\n  pick as n ->\n  (add 1) 2 as m ->\n  n + m + 10\n' || return 1
    [ "$status" -eq 14 ] || { echo "  status=$status" >&2; return 1; }
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
check 'a: a nested application keeps the continuation' nested_application_keeps_the_continuation
check 'b: the bound form still keeps the continuation' bound_form_still_keeps_the_continuation
check 'c: an abort arm works through the nested call' abort_arm_through_the_nested_call
check 'd: a pure closure stays a plain call' pure_closure_stays_a_plain_call

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
