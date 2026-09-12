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

# 位置を検査する探針は、先頭の空行と行頭の空白で診断の行:列をずらし、
# 期待値を探針側で計算する（位置を定数で書かない）。
spaces() { printf '%*s' "$1" ''; }
blank_lines() { local i; for ((i = 0; i < $1; i++)); do printf '\n'; done; }

mkdir -p "$workdir/32a" "$workdir/32b" "$workdir/32b2" "$workdir/32b3" "$workdir/32c1" "$workdir/32c2" "$workdir/32c3" \
    "$workdir/32d" "$workdir/32d1" "$workdir/32d2" "$workdir/32d3" "$workdir/32d4" "$workdir/32d5" "$workdir/32e" \
    "$workdir/32g1" "$workdir/32g2" "$workdir/32g3"

cat >"$workdir/32a/lib.hd" <<'EOF'
def helper value => value + 1

module Store
def read(value: Int): Int

implement Store as Memory
def read value => helper value
EOF

cat >"$workdir/32b/lib.hd" <<'EOF'
library

module Store
def read(value: Int): Int

implement Store as MemoryStore
def read value => value
EOF
printf 'def main = 1\n' >"$workdir/32c3/main.hd"
printf 'def library = 1\n' >"$workdir/32g1/main.hd"
printf 'def shared = 1\n' >"$workdir/32g2/main.hd"
printf 'type shared = | A\n' >"$workdir/32g3/main.hd"

cat >"$workdir/32d/lib.hd" <<'EOF'
library

type Outer = | Outer(Int)

effect E
abort fail

module S
def read(value: Int): Int

module Empty
def done(x: Int): Int

implement S as Memory
def read value => value

implement S as Make(P: S)
def read value => P.read value

handler E as H
abort fail => 0
return x => x
EOF

cat >"$workdir/32d1/lib.hd" <<'EOF'
library

module S
def read(value: Int): Int

implement S as Memory
def read value => value
def extra x => x
EOF

cat >"$workdir/32d2/lib.hd" <<'EOF'
library

module S
def read(value: Int): Int

implement S as Memory
type Local = | Local(Int)
def read value => value
EOF

cat >"$workdir/32d3/lib.hd" <<'EOF'
library

def helper x => x

module S
def read(value: Int): Int

implement S as Memory
def read value => value
EOF

printf 'def main = 1\n' >"$workdir/32e/a.hd"
cat >"$workdir/32e/lib.hd" <<'EOF'
module Store
def read(value: Int): Int

implement Store as Memory
def read value => value
EOF

expect_error() {
    local label=$1 entry=$2 file=$3 code=$4 line=$5 col=$6
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$file:$line:$col: error[$code]: "
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$prefix', got status=$status '$err')"
    fi
}

expect_command_error() {
    local label=$1 command=$2 entry=$3 file=$4 code=$5 line=$6 col=$7
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- "$command" "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$file:$line:$col: error[$code]: "
    local artifact="${entry%.hd}"
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]] && [ ! -e "$artifact" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$prefix' without artifact, got status=$status '$err')"
    fi
}

accept() {
    local label=$1 entry=$2
    total=$((total + 1))
    local out err status
    out=$(cargo run --offline -q -- check "$entry" 2>"$workdir/stderr"); status=$?
    err=$(cat "$workdir/stderr")
    if [ "$status" -eq 0 ] && [ -z "$out" ] && [ -z "$err" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (status=$status stdout='$out' stderr='$err')"
    fi
}

accept_print() {
    local label=$1 entry=$2
    total=$((total + 1))
    local out="$workdir/print-stdout" err="$workdir/print-stderr" status
    cargo run --offline -q -- check --print-type "$entry" >"$out" 2>"$err"; status=$?
    # 末尾の改行まで含めて測る（コマンド置換は末尾改行を落とすので使わない）。
    local out_bytes err_bytes
    out_bytes=$(wc -c <"$out"); err_bytes=$(wc -c <"$err")
    if [ "$status" -eq 0 ] && [ "$out_bytes" -eq 0 ] && [ "$err_bytes" -eq 0 ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (status=$status stdout_bytes=$out_bytes stderr_bytes=$err_bytes)"
    fi
}

expect_entry_error() {
    local label=$1 command=$2 entry=$3 file=$4
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- "$command" "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$file:1:1: error[E206]: "
    local artifact="${entry%.hd}"
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]] \
        && [[ "$err" == *"check"* ]] && [[ "$err" == *"library"* ]] && [ ! -e "$artifact" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want entry-oriented E206 with check/library without artifact, got status=$status '$err')"
    fi
}

# 32-a: main の無い .hd は check ではライブラリとして受理、build / dump-mono は E206 1:1。
accept '32-a check (main 無しはライブラリとして受理)' "$workdir/32a/lib.hd"
accept_print '32-a print-type (stdout も stderr も空)' "$workdir/32a/lib.hd"
expect_entry_error '32-a build (E206 1:1、entry 向けメッセージ)' build "$workdir/32a/lib.hd" lib.hd
expect_entry_error '32-a dump-mono (E206 1:1、entry 向けメッセージ)' dump-mono "$workdir/32a/lib.hd" lib.hd

# 32-b: library を宣言したディレクトリの main は E206。行頭の空白で列を、
# 先頭の空行で行をずらし、期待する行:列を探針の中で計算する。
indent_b=$((1 + RANDOM % 8))
printf '%sdef main = 1\n' "$(spaces "$indent_b")" >"$workdir/32b/main.hd"
expect_error '32-b (check, library ディレクトリの main、行頭の空白)' "$workdir/32b/main.hd" main.hd E206 1 $((indent_b + 1))
expect_command_error '32-b (build, library ディレクトリの main、行頭の空白)' build "$workdir/32b/main.hd" main.hd E206 1 $((indent_b + 1))
expect_error '32-b (check, lib.hd から他ファイルの main、行頭の空白)' "$workdir/32b/lib.hd" main.hd E206 1 $((indent_b + 1))
blank_b=$((1 + RANDOM % 4))
{ blank_lines "$blank_b"; printf 'def main = 1\n'; } >"$workdir/32b/main.hd"
expect_error '32-b (check, 先頭の空行)' "$workdir/32b/main.hd" main.hd E206 $((blank_b + 1)) 1

# 32-b (library 宣言ディレクトリの引数付き main): 定義の形によらず E206。別ファイルの
# 引数付き main でも、位置はその def を指す（値定義だけを探してはいけない）。
printf 'library\n' >"$workdir/32b2/lib.hd"
blank_b2=$((1 + RANDOM % 4))
{
    blank_lines "$blank_b2"
    printf 'def main x => x\n\n'
    printf 'module S\n'
    printf 'def read(value: Int): Int\n\n'
    printf 'implement S as Memory\n'
    printf 'def read value => main value\n'
} >"$workdir/32b2/main.hd"
expect_error '32-b (library の引数付き main、別ファイル、先頭の空行)' "$workdir/32b2/lib.hd" main.hd E206 $((blank_b2 + 1)) 1

# 32-b (引数付き main の build / dump-mono): 早期拒否の 1:1 ではなく、entry_form の
# E206 が check と同じ宣言位置になる。
blank_b3=$((1 + RANDOM % 4))
{ blank_lines "$blank_b3"; printf 'def main x => x\n'; } >"$workdir/32b3/main.hd"
expect_error '32-b (引数付き main、check は宣言位置)' "$workdir/32b3/main.hd" main.hd E206 $((blank_b3 + 1)) 1
expect_command_error '32-b (引数付き main、build は宣言位置)' build "$workdir/32b3/main.hd" main.hd E206 $((blank_b3 + 1)) 1
expect_command_error '32-b (引数付き main、dump-mono は宣言位置)' dump-mono "$workdir/32b3/main.hd" main.hd E206 $((blank_b3 + 1)) 1

# 32-c (1): 同一ファイルの 2 つ目は E205。
indent_c1=$((1 + RANDOM % 8))
{ printf 'library\n'; printf '%slibrary\n' "$(spaces "$indent_c1")"; } >"$workdir/32c1/main.hd"
expect_error '32-c (1a) 同一ファイルの 2 つ目、行頭の空白' "$workdir/32c1/main.hd" main.hd E205 2 $((indent_c1 + 1))
blank_c1=$((1 + RANDOM % 4))
{ printf 'library\n'; blank_lines "$blank_c1"; printf 'library\n'; } >"$workdir/32c1/main.hd"
expect_error '32-c (1b) 同一ファイルの 2 つ目、先頭の空行' "$workdir/32c1/main.hd" main.hd E205 $((blank_c1 + 2)) 1

# 32-c (2): 別ファイルの 2 つ目は E205。
printf 'library\n' >"$workdir/32c2/a.hd"
indent_c2=$((1 + RANDOM % 8))
printf '%slibrary\n' "$(spaces "$indent_c2")" >"$workdir/32c2/b.hd"
expect_error '32-c (2a) 別ファイルの 2 つ目、行頭の空白' "$workdir/32c2/a.hd" b.hd E205 1 $((indent_c2 + 1))
blank_c2=$((1 + RANDOM % 4))
{ blank_lines "$blank_c2"; printf 'library\n'; } >"$workdir/32c2/b.hd"
expect_error '32-c (2b) 別ファイルの 2 つ目、先頭の空行' "$workdir/32c2/a.hd" b.hd E205 $((blank_c2 + 1)) 1

# 32-c (3): .hdi の library は E100。
indent_c3=$((1 + RANDOM % 8))
printf '%slibrary\n' "$(spaces "$indent_c3")" >"$workdir/32c3/sig.hdi"
expect_error '32-c (3a) .hdi の library、行頭の空白' "$workdir/32c3/main.hd" sig.hdi E100 1 $((indent_c3 + 1))
blank_c3=$((1 + RANDOM % 4))
{ blank_lines "$blank_c3"; printf 'library\n'; } >"$workdir/32c3/sig.hdi"
expect_error '32-c (3b) .hdi の library、先頭の空行' "$workdir/32c3/main.hd" sig.hdi E100 $((blank_c3 + 1)) 1
accept '32-d (未参照の公開面でも受理)' "$workdir/32d/lib.hd"
expect_error '32-d (1) 契約に無い実装の def' "$workdir/32d1/lib.hd" lib.hd E514 8 1
expect_error '32-d (2) 実装局所の ADT が未使用' "$workdir/32d2/lib.hd" lib.hd E513 7 1
expect_error '32-d (3) ライブラリ根の外の def が未使用' "$workdir/32d3/lib.hd" lib.hd E513 3 1

# 32-d (4): handler の腕から参照される外の def は、その先も辿って根にする。
cat >"$workdir/32d4/lib.hd" <<'EOF'
library

effect E
abort fail

handler E as H
abort fail => helper 0
return x => x

def helper x => leaf x
def leaf x => x
EOF
accept '32-d (4) handler から間接参照される外の def も根から辿る' "$workdir/32d4/lib.hd"

# 32-d (5): 負の対照。handler が直接呼ぶ def から辿れない def は E513。
cat >"$workdir/32d5/lib.hd" <<'EOF'
library

effect E
abort fail

handler E as H
abort fail => helper 0
return x => x

def helper x => x
def leaf x => x
EOF
expect_error '32-d (5) handler から辿れない外の def は E513' "$workdir/32d5/lib.hd" lib.hd E513 11 1
expect_error '32-e (宣言の無い main は普通の def)' "$workdir/32e/lib.hd" a.hd E513 1 1
rm "$workdir/32e/a.hd"
accept '32-e (a.hd を消せば受理)' "$workdir/32e/lib.hd"
expect_error '32-g def library' "$workdir/32g1/main.hd" main.hd E100 1 5
expect_error '32-g def shared' "$workdir/32g2/main.hd" main.hd E100 1 5
expect_error '32-g type shared' "$workdir/32g3/main.hd" main.hd E100 1 6

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
