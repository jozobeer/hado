#!/usr/bin/env bash
# resume-once: §38 の `resume` が捕まえた継続を 1 回だけ再開することを測る。
#
#   a. 同じ腕で 2 回 resume を呼んでも with の続きは 1 回だけ走る。2 回目は
#      resume was already used で止まり、終了コード 1 になる。
#   b. resume が 1 回だけなら継続は 1 回走り、値が返る（対照）。
#   c. resume が if の片側にしか無くても、選ばれた側の resume は 1 回走る（対照）。
#   d. again が 2 回起これば、それぞれの継続は自分の印を持つ。印を 1 つのセルに
#      共有すると 2 回目の resume が止まるので、両方が値を返すことを見る（対照）。
#
# tier: quick
# round: resume-once
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

# --- a. 2 回目の resume は継続を再実行しない ---
second_resume_does_not_rerun_the_continuation() {
    program second 'effect Choice\nagain pick: Int\nhandler Choice as C\nagain pick =>\n  resume 1 as a ->\n  resume 2 as b ->\n  a + b\nreturn value => value\ndef main =\n  with C ->\n  pick as n ->\n  log "after" ->\n  n\n' || return 1
    [ "$status" -eq 1 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "after" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ "$stderr" = "resume was already used" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- b. 1 回だけの resume は 1 回走る ---
one_resume_runs_the_continuation_once() {
    program single 'effect Choice\nagain pick: Int\nhandler Choice as C\nagain pick => resume 7\nreturn value => value\ndef main =\n  with C ->\n  log (Int.to_string (pick))\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "7" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- c. if の片側だけの resume も 1 回走る ---
resume_in_only_one_arm_still_runs_once() {
    program one-arm 'effect Choice\nagain pick: Int\nhandler Choice as C\nagain pick => if true => resume 7 | resume 8\nreturn value => value\ndef main = with C -> log (Int.to_string (pick))\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "7" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}

# --- d. again ごとに印が分かれる ---
each_again_gets_its_own_cell() {
    program two-yields 'effect Yield\nagain yield(value: Int): Int\nhandler Yield as Sum\nagain yield value =>\n  resume value\nreturn value => value\ndef main =\n  with Sum ->\n  yield 1 as a ->\n  yield 2 as b ->\n  log (Int.to_string (a + b))\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "3" ] || { echo "  stdout=$stdout" >&2; return 1; }
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
check 'a: a second resume does not rerun the continuation' second_resume_does_not_rerun_the_continuation
check 'b: one resume runs the continuation once' one_resume_runs_the_continuation_once
check 'c: a resume in one if arm still runs once' resume_in_only_one_arm_still_runs_once
check 'd: each again gets its own one-shot cell' each_again_gets_its_own_cell

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
