#!/usr/bin/env bash
# tier: quick
# round: repl
# §11 の継続としての REPL（`docs/issues/repl.md` 段階 1、方式は `docs/plans/repl.md` の A）。
# 完了条件の 2 つ（対話の入口があること・前の入力の効果が再実行されないこと）を
# そのまま測る。値は §54 の `main` の規則で出る（Int を終了コードにする規則だけは
# 当てない）。入力はファイル名ではなく、ここで組み立てた列をそのまま与える。
#
# 効果の再実行は `log` の出力だけでは数えられない（出力を隠してやり直す実装が
# 緑になる）。検査 3 はファイルへの書き込みの回数で数える。
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

# --- 2. 完了条件: 前の入力の効果は再実行されない（`log` の出力） ---
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

# --- 3. 効果の回数をファイルで数える。1 入力につき 1 文字増える入力を 2 回、
#        束縛と長さの読み出しを挟んで 8 入力流す。期待は実装から独立に決まる ---
counts_file_writes() {
    local dir="$workdir/count"
    mkdir -p "$dir"
    : >"$dir/count.txt"
    local path="$dir/count.txt"
    local script
    script=$(printf 'read_file "%s" as old -> write_file "%s" (Bytes.of_string (String.concat (Bytes.to_string old) "x"))\n1 as y ->\nread_file "%s" as now -> Bytes.length now\ny + 1\nread_file "%s" as now -> Bytes.length now\nread_file "%s" as old -> write_file "%s" (Bytes.of_string (String.concat (Bytes.to_string old) "x"))\ny + 2\nread_file "%s" as now -> Bytes.length now\n' \
        "$path" "$path" "$path" "$path" "$path" "$path" "$path")
    session count "$script" || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '1\n2\n1\n3\n2')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ "$(cat "$dir/count.txt")" = "xx" ] || { echo "  file=$(cat "$dir/count.txt") want=xx" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
}
check '3 the file counts one write per input, not one per replay' counts_file_writes

# --- 4. 束縛の持ち越しと、入力の完結 ---
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
check '4 repl carries bindings and waits for a balanced input' carries_bindings

# --- 5. §52: `<dir>` のライブラリ。実装は 1 つの入力がその一部しか使わなくても
#        未使用ではない（1 歩ごとに E513 を当てると全部の入力が断られる） ---
loads_library() {
    local dir="$workdir/library"
    mkdir -p "$dir/two/a" "$dir/two/b"
    printf 'library\n' >"$dir/two/two.hd"
    printf 'module A\ndef one (n: Int): Int\n/module\n' >"$dir/two/a/a.hdi"
    printf 'implement A as A\ndef one n => n + 1\n/implement\n' >"$dir/two/a/a.hd"
    printf 'module B\ndef two (n: Int): Int\n/module\n' >"$dir/two/b/b.hdi"
    printf 'implement B as B\ndef two n => n + 2\n/implement\n' >"$dir/two/b/b.hd"
    session a-only 'two::a::A.one 1\n' "$dir/two" || return 1
    [ "$status" -eq 0 ] || { echo "  a-only status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '2')" ] || { echo "  a-only stdout=$stdout" >&2; return 1; }
    session b-only 'two::b::B.two 1\n' "$dir/two" || return 1
    [ "$stdout" = "$(printf '3')" ] || { echo "  b-only stdout=$stdout" >&2; return 1; }
    session neither '1 + 1\n' "$dir/two" || return 1
    [ "$stdout" = "$(printf '2')" ] || { echo "  neither stdout=$stdout" >&2; return 1; }
    session mixed 'two::a::A.one 1\ntwo::b::B.two 1\n1 + 1\n' "$dir/two" || return 1
    [ "$stdout" = "$(printf '2\n3\n2')" ] || { echo "  mixed stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  mixed stderr=$stderr" >&2; return 1; }
    # 名前空間にならないディレクトリ名は受け取らない（§51）。
    local bad="$dir/Bad"
    mkdir -p "$bad"
    session bad-name '1\n' "$bad" || return 1
    [ "$status" -eq 1 ] || { echo "  bad name status=$status want=1" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  bad name stdout=$stdout" >&2; return 1; }
    # `library` を宣言しないディレクトリはライブラリではない（§52）。
    local plain="$dir/plain"
    mkdir -p "$plain"
    printf 'def value => 1\n' >"$plain/plain.hd"
    session no-library '1\n' "$plain" || return 1
    [ "$status" -eq 1 ] || { echo "  no library status=$status want=1" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  no library stdout=$stdout" >&2; return 1; }
    # issue の現象にあった `repl stdlib` も入口で断る。stdlib は §52 のライブラリ
    # ではなく、根の宣言として読むと prelude と衝突して入力ごとに E205 が出る。
    session stdlib '1\n' stdlib || return 1
    [ "$status" -eq 1 ] || { echo "  stdlib status=$status want=1" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdlib stdout=$stdout" >&2; return 1; }
    [ "$(printf '%s\n' "$stderr" | wc -l)" -le 2 ] || { echo "  stdlib stderr has too many lines" >&2; return 1; }
}
check '5 hado repl <dir> loads that library and rejects a bad name' loads_library

# --- 6. §10 の規則: 単相でない値は受けない。診断のあともセッションは続く ---
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
check '6 a non-monomorphic input is diagnosed and the session continues' monomorphic

# --- 7. 最上位に届いた raise は、その入力だけを捨ててセッションを続ける ---
raise_continues() {
    session raise 'Bytes.get (Bytes.of_string "") 5\n7\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status" >&2; return 1; }
    [ "$stdout" = "$(printf '7')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    case "$stderr" in
        *IndexOutOfRange*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}
check '7 a raise at the top abandons that input and the session continues' raise_continues

# --- 8. §53.1: スタックの枯渇は診断で、シグナル死ではない ---
stack_diagnostic() {
    local dir="$workdir/deep"
    mkdir -p "$dir/deep/d"
    printf 'library\n' >"$dir/deep/deep.hd"
    printf 'module D\ndef down (n: Int): Int\n/module\n' >"$dir/deep/d/d.hdi"
    printf 'implement D as D\ndef rec down n => if n == 0 => 0 | 1 + down (n - 1)\n/implement\n' >"$dir/deep/d/d.hd"
    session stack 'deep::d::D.down 1000000\n7\n' "$dir/deep" || return 1
    [ "$status" -eq 1 ] || { echo "  status=$status want=1（シグナル死なら 132/134/139）" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  stdout=$stdout" >&2; return 1; }
    case "$stderr" in
        *"hado: stack overflow"*) ;;
        *) echo "  stderr=$stderr" >&2; return 1 ;;
    esac
}
check '8 stack exhaustion is a diagnostic, not a signal death' stack_diagnostic

# --- 9. 束縛した値は、割り当ての多い入力を挟んでも生きている（Boehm GC の根） ---
bound_value_survives_gc() {
    session gc 'List.map [1, 2, 3, 4, 5, 6, 7, 8] fn(v => String.concat "value-" (Int.to_string v)) as xs ->\n[0, 1, 2, 3, 4, 5, 6, 7, 8, 9] as a ->\nList.fold a [] fn(acc, i => List.append (List.map a fn(j => i * 10 + j)) acc) as b ->\nList.fold b 0 fn(s, i => s + i + List.length (List.map b fn(j => String.concat "zzzzzzzzzzzzzzzz" (Int.to_string j))))\nList.fold b 0 fn(s, i => s + i + List.length (List.map b fn(j => String.concat "yyyyyyyyyyyyyyyy" (Int.to_string j))))\nxs\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status want=0（回収されるとシグナルで落ちる）" >&2; return 1; }
    [ "$stdout" = "$(printf '14950\n14950\n["value-1", "value-2", "value-3", "value-4", "value-5", "value-6", "value-7", "value-8"]')" ] || { echo "  stdout=$stdout" >&2; return 1; }
}
check '9 a bound value survives the allocations of later inputs' bound_value_survives_gc

# --- 10. 使い方: `repl` は <dir> を高々 1 つ取る ---
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
check '10 repl takes at most one library directory' repl_usage

# --- 11. 束縛の仮引数は自由な名前だけ。字句の同綴り（ラムダの仮引数・field）は使わない ---
binding_not_a_token_match() {
    session lambda-param '1 as x ->\nfn(x => x + 1) 2\n' || return 1
    [ "$status" -eq 0 ] || { echo "  lambda status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '3')" ] || { echo "  lambda stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  lambda stderr=$stderr" >&2; return 1; }
    session member '1 as map ->\nList.map [1, 2] fn(v => v + 1)\n' || return 1
    [ "$status" -eq 0 ] || { echo "  member status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '[2, 3]')" ] || { echo "  member stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  member stderr=$stderr" >&2; return 1; }
}
check '11 a later input may reuse a binding name as a parameter or field' binding_not_a_token_match

# --- 12. 行コメントは束縛の印にならない。`->` の後ろのコメントは束縛を隠さない ---
comment_is_not_a_binding() {
    session comment-arrow '0 as x ->\n1 // as x ->\nx\n' || return 1
    [ "$status" -eq 0 ] || { echo "  status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '1\n0')" ] || { echo "  stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  stderr=$stderr" >&2; return 1; }
    session comment-after '1 as x -> // keep\nx\n' || return 1
    [ "$status" -eq 0 ] || { echo "  after status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '1')" ] || { echo "  after stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  after stderr=$stderr" >&2; return 1; }
    session comment-discard '1 -> // gone\n2\n' || return 1
    [ "$status" -eq 0 ] || { echo "  discard status=$status stderr=$stderr" >&2; return 1; }
    [ "$stdout" = "$(printf '2')" ] || { echo "  discard stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  discard stderr=$stderr" >&2; return 1; }
}
check '12 a line comment does not create or hide a binding' comment_is_not_a_binding

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
