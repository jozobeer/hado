#!/usr/bin/env bash
# tier: quick
# round: repl-abstract-map-binding
# #109: 公開型が抽象型の値（Map.T）を REPL で束縛できず、次の入力が E204 に
# なる欠陥を固定する。合成した `main` の E705 は E702 と同じ REPL の産物として
# 落とすが、`hado check` の E705 は残す。値の印字は実装の表現に開かず断り、
# `MapNode` を漏らさない（§10・§47）。
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
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado="$target_dir/debug/hado"

# `session <名前> <入力の列>` は入力をパイプで与え、status/stdout/stderr を
# グローバルに置く。
status=0
stdout=''
stderr=''
session() {
    local name="$1" script="$2"
    local dir="$workdir/$name"
    mkdir -p "$dir"
    printf '%b' "$script" >"$dir/in"
    "$hado" repl <"$dir/in" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

# --- 1. 束縛した写像を次の入力から使える（現象の直接の再現） ---
binds_and_finds() {
    session find 'Map.insert Map.empty 1 "a" as m ->\nMap.find m 1\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = 'Some("a")' ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
    session list 'Map.insert Map.empty 1 "a" as m ->\nMap.to_list m\n' || return 1
    [ "$status" -eq 0 ] || { echo "  list status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = '[MapEntry(1, "a")]' ] || { echo "  list stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  list stderr=$stderr" >&2; return 1; }
}
check '1 a bound abstract map is usable from the next input' binds_and_finds

# --- 2. 写像そのものは印字できない。実装の constructor を漏らさず、セッションは続く ---
refusues_to_print_without_leaking() {
    session print 'Map.insert Map.empty 1 "a" as m ->\nm\n7\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status want=0 stderr=$stderr" >&2; return 1; }
    [ "$stdout" = '7' ] || { echo "  stdout=$stdout" >&2; return 1; }
    case "$stderr" in
        *印字できない*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
    case "$stderr" in
        *MapNode*|*MapLeaf*) echo "  Map internals leaked: $stderr" >&2; return 1 ;;
    esac
}
check '2 printing an abstract map is refused without leaking its representation' refusues_to_print_without_leaking

# --- 3. REPL の産物だけを落とす。`hado check` の E705 は残る ---
check_still_rejects() {
    local source="$workdir/main.hd"
    printf 'def main = Map.insert Map.empty 1 "a"\n' >"$source"
    local out err rc
    out=$("$hado" check "$source" 2>"$workdir/check.err")
    rc=$?
    err=$(cat "$workdir/check.err")
    [ "$rc" -eq 1 ] || { echo "  check rc=$rc want=1" >&2; return 1; }
    [ -z "$out" ] || { echo "  check stdout=$out" >&2; return 1; }
    case "$err" in
        *E705*) ;;
        *) echo "  check stderr=$err" >&2; return 1 ;;
    esac
}
check '3 hado check still rejects a main that returns an abstract map' check_still_rejects

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
