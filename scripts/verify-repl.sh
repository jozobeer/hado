#!/usr/bin/env bash
# tier: quick
# round: repl
# §11 の継続としての REPL（`docs/issues/repl.md` 段階 1、方式は `docs/plans/repl.md` の A）。
# 完了条件の 2 つ（対話の入口があること・前の入力の効果が再実行されないこと）を
# そのまま測る。値は §54 の `main` の規則で出る（Int を終了コードにする規則だけは
# 当てない）。入力はファイル名ではなく、ここで組み立てた列をそのまま与える。
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

# `session <名前> <入力の列> [<ライブラリのディレクトリ>]` は入力の列をパイプで
# 与え、status/stdout/stderr をグローバルに置く。標準入力は端末ではないので
# 前口上は出ない。
status=0
stdout=''
stderr=''
session() {
    local name="$1" script="$2" library="${3:-}"
    local dir="$workdir/$name"
    mkdir -p "$dir"
    printf '%b' "$script" >"$dir/in"
    if [ -n "$library" ]; then
        "$hado" repl "$library" <"$dir/in" >"$dir/out" 2>"$dir/err"
    else
        "$hado" repl <"$dir/in" >"$dir/out" 2>"$dir/err"
    fi
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

# --- 1. 対話の入口: 1 入力 1 歩を順に評価し、値を §54 の規則で印字する ---
prints_each_step() {
    session print '40 + 2\n"hi"\ntrue\n1.5\n()\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "$(printf '42\nhi\ntrue\n1.5')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}
check '1 repl evaluates each complete input and prints its value' prints_each_step

# --- 2. 完了条件: 前の入力の効果は再実行されない ---
effect_once() {
    session effect 'log "a"\nlog "b"\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "$(printf 'a\nb')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    # 効果の入力の間に別の入力を挟んでも、前の効果は繰り返さない。
    session effect-binding 'log "a"\n1 as x ->\nx\nlog "b"\n' || return 1
    [ "$stdout" = "$(printf 'a\n1\nb')" ] || { echo "  binding stdout=$stdout" >&2; return 1; }
    # `e ->` の継続でも効果は 1 回だけである。
    session effect-discard 'log "a" ->\nlog "b"\n' || return 1
    [ "$stdout" = "$(printf 'a\nb')" ] || { echo "  discard stdout=$stdout" >&2; return 1; }
}
check '2 an earlier input effect is not re-executed' effect_once

# --- 3. 束縛の持ち越しと、入力の完結 ---
carries_bindings() {
    session binding '1 as x ->\nx + 1\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "$(printf '2')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    # 前の入力の束縛を捕まえた閉包も次の入力へ持ち越す。
    session closure '1 as x ->\n(fn(a: Int => a + x)) as f ->\nf 4\n' || return 1
    [ "$stdout" = "$(printf '5')" ] || { echo "  closure stdout=$stdout" >&2; return 1; }
    # 括弧が釣り合うまで送らない。1 行目だけで評価すると E100 になる入力。
    session complete '[1, 2\n, 3]\n' || return 1
    [ "$stdout" = "$(printf '[1, 2, 3]')" ] || { echo "  complete stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  complete stderr=$stderr" >&2; return 1; }
}
check '3 repl carries bindings and waits for a balanced input' carries_bindings

# --- 4. §52: `hado repl <dir>` はそのライブラリを読み込む ---
loads_library() {
    local dir="$workdir/library"
    mkdir -p "$dir/counter/box"
    printf 'library\n' >"$dir/counter/counter.hd"
    printf 'module Box\ndef value (n: Int): Int\n/module\n' >"$dir/counter/box/box.hdi"
    printf 'implement Box as Box\ndef value n => n + 41\n/implement\n' >"$dir/counter/box/box.hd"
    session library 'counter::box::Box.value 1\n' "$dir/counter" || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '42')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
    # 名前空間にならないディレクトリ名は受け取らない（§51）。
    local bad="$dir/Bad"
    mkdir -p "$bad"
    session bad-name '1\n' "$bad" || return 1
    [ "$status" -eq 1 ] || { echo "  bad name status=$status want=1" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  bad name stdout=$stdout" >&2; return 1; }
}
check '4 hado repl <dir> loads that library and rejects a bad name' loads_library

# --- 5. §10 の規則: 単相でない値は受けない。診断のあともセッションは続く ---
monomorphic() {
    session polymorphic '[]\n7\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "$(printf '7')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -n "$stderr" ] || { echo "  no diagnostic on stderr" >&2; return 1; }
    # 診断は標準エラーへ、値は標準出力へ。診断のあとの入力も評価する。
    session typed '"x"\n1 + true\n2\n' || return 1
    [ "$status" -eq 0 ] || { echo "  typed status=$status" >&2; return 1; }
    [ "$stdout" = "$(printf 'x\n2')" ] || { echo "  typed stdout=$stdout" >&2; return 1; }
    [ -n "$stderr" ] || { echo "  typed stderr is empty" >&2; return 1; }
}
check '5 a non-monomorphic input is diagnosed and the session continues' monomorphic

# --- 6. 使い方: `repl` は <dir> を高々 1 つ取る ---
repl_usage() {
    local dir="$workdir/usage"
    mkdir -p "$dir"
    "$hado" repl a b >"$dir/out" 2>"$dir/err"
    local rc=$?
    [ "$rc" -eq 2 ] || { echo "  rc=$rc want=2" >&2; return 1; }
    [ ! -s "$dir/out" ] || { echo "  stdout=$(cat "$dir/out")" >&2; return 1; }
    grep -q '^usage: hado ' "$dir/err" || { echo "  stderr=$(cat "$dir/err")" >&2; return 1; }
    grep -q 'repl \[<dir>\]' "$dir/err" || { echo "  usage does not name repl" >&2; return 1; }
}
check '6 repl takes at most one library directory' repl_usage

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
