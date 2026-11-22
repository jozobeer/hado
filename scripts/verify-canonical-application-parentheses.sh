#!/usr/bin/env bash
# canonical-application-parentheses: 診断の `canonical form:` と `fmt` が、適用の
# 呼び手が適用のときの括弧（`(f a) b`）を落とさないことを測る（§36・§57・§58.1）。
# 正準形が `f a b` と書くと、2 回の適用が 1 回の 2 引数呼び出しに変わり、
# 書き換えたソースは E301 になる。
#
#   a. E510 の `canonical form:` を `main` の本体に書いたソースが、元のソースと
#      同じ値を出す（`hado run` が `[3]`）。
#   b. `fmt` の出力がその `canonical form:` と同じ綴りで、`run` が同じ値を出す。
#   c. E520 の `canonical form:` も同じ（Boolean branch の腕の中の入れ子の適用）。
#   d. 平坦化した `[add 1 2]` は E301 で拒否される（2 つの綴りは同じ意味でない）。
#   e. `check --fix` と `fmt` が同じバイト列を残す（§57）。
#
# 期待値は診断から読む。平坦化した綴りだけを対照としてここに書く。
#
# tier: quick
# round: canonical-application-parentheses
set -uo pipefail
cd "$(dirname "$0")/.."

pass=0
total=5
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1 ($2)"; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado_bin="$target_dir/debug/hado"

# 1 つの検体を 1 つのディレクトリに置く（`check` は入力根の .hd を全部読む。§50）。
specimen() {
    local dir=$1 source=$2
    mkdir -p "$work/$dir"
    printf '%s' "$source" >"$work/$dir/main.hd"
}

# 診断の `canonical form:` を 1 行で取り出す。
hint_of() {
    "$hado_bin" check "$work/$1/main.hd" 2>&1 | sed -n 's/.*canonical form: //p'
}

# `def add` と `def main = <hint>` を新しいディレクトリへ書き、`run` の出力を残す。
# hint が読めない綴りなら `run` が落ち、終了コードが残る。
run_hint() {
    local dir=$1 hint=$2 out=$3
    mkdir -p "$work/$dir"
    printf 'def add x => fn(y => x + y)\ndef main = %s\n' "$hint" >"$work/$dir/main.hd"
    "$hado_bin" run "$work/$dir/main.hd" >"$out" 2>&1
    echo $? >"$out.exit"
}

CONS='def add x => fn(y => x + y)
def main = Cons((add 1) 2, Nil)
'
ARM='def add x => fn(y => x + y)
def main = if true | true => (add 1) 2 | false => 0
'

# --- a. E510 の canonical form を本体に書いたソースが同じ値を出す ---
specimen cons "$CONS"
hint=$(hint_of cons)
run_hint cons-hint "$hint" "$work/a.out"
if [ "$(cat "$work/a.out")" = "[3]" ] && [ "$(cat "$work/a.out.exit")" = "0" ]; then
    ok "a: E510 の canonical form を本体に書いたソースが [3] を出す"
else
    ng "a: E510 の canonical form を本体に書いたソースが [3] を出す" \
        "exit=$(cat "$work/a.out.exit") hint=$hint"
fi

# --- b. fmt の出力が同じ canonical form で、同じ値を出す ---
mkdir -p "$work/cons-fmt"
printf '%s' "$CONS" >"$work/cons-fmt/main.hd"
if "$hado_bin" fmt "$work/cons-fmt/main.hd" >/dev/null 2>&1; then
    printed=$(sed -n 's/^def main = //p' "$work/cons-fmt/main.hd")
    "$hado_bin" run "$work/cons-fmt/main.hd" >"$work/b.out" 2>&1
    status=$?
    if [ "$printed" = "$hint" ] && [ "$(cat "$work/b.out")" = "[3]" ] && [ "$status" = "0" ]; then
        ok "b: fmt の出力が同じ canonical form で [3] を出す"
    else
        ng "b: fmt の出力が同じ canonical form で [3] を出す" \
            "printed=$printed exit=$status"
    fi
else
    ng "b: fmt の出力が同じ canonical form で [3] を出す" "fmt が落ちた"
fi

# --- c. E520 の canonical form も同じ ---
specimen arm "$ARM"
arm_hint=$(hint_of arm)
# E520 の書き換えは Boolean branch 1 つなので、リストの要素として置き直す。
mkdir -p "$work/arm-hint"
printf 'def add x => fn(y => x + y)\ndef main = [%s]\n' "$arm_hint" >"$work/arm-hint/main.hd"
"$hado_bin" run "$work/arm-hint/main.hd" >"$work/c.out" 2>&1
arm_status=$?
if [ "$(cat "$work/c.out")" = "[3]" ] && [ "$arm_status" = "0" ]; then
    ok "c: E520 の canonical form をリストに置いたソースが [3] を出す"
else
    ng "c: E520 の canonical form をリストに置いたソースが [3] を出す" \
        "exit=$arm_status hint=$arm_hint"
fi

# --- d. 平坦化した綴りは同じ意味ではない ---
mkdir -p "$work/flat"
printf 'def add x => fn(y => x + y)\ndef main = [add 1 2]\n' >"$work/flat/main.hd"
if "$hado_bin" run "$work/flat/main.hd" >"$work/d.out" 2>&1; then
    ng "d: 平坦化した [add 1 2] は E301 で拒否される" "受理された"
elif grep -q 'error\[E301\]' "$work/d.out"; then
    ok "d: 平坦化した [add 1 2] は E301 で拒否される"
else
    ng "d: 平坦化した [add 1 2] は E301 で拒否される" "$(tail -1 "$work/d.out")"
fi

# --- e. check --fix と fmt が同じバイト列を残す ---
mkdir -p "$work/cons-fix"
printf '%s' "$CONS" >"$work/cons-fix/main.hd"
if "$hado_bin" check --fix "$work/cons-fix/main.hd" >"$work/e.out" 2>&1; then
    if cmp -s "$work/cons-fix/main.hd" "$work/cons-fmt/main.hd"; then
        ok "e: check --fix と fmt が同じバイト列を残す"
    else
        ng "e: check --fix と fmt が同じバイト列を残す" "check --fix と fmt が食い違う"
        diff -u "$work/cons-fmt/main.hd" "$work/cons-fix/main.hd" | sed -n '1,6p'
    fi
else
    ng "e: check --fix と fmt が同じバイト列を残す" "$(tail -1 "$work/e.out")"
fi

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
