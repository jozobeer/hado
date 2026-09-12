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

mkdir -p "$workdir/32b" "$workdir/32c1" "$workdir/32c2" "$workdir/32c3" \
    "$workdir/32g1" "$workdir/32g2" "$workdir/32g3" "$workdir/32neg"

cat >"$workdir/32b/lib.hd" <<'EOF'
library

module Store
def read(value: Int): Int

implement Store as MemoryStore
def read value => value
EOF
printf 'def main = 1\n' >"$workdir/32b/main.hd"

printf 'library\nlibrary\n' >"$workdir/32c1/main.hd"
printf 'library\n' >"$workdir/32c2/a.hd"
printf 'library\n' >"$workdir/32c2/b.hd"
printf 'library\n' >"$workdir/32c3/sig.hdi"
printf 'def main = 1\n' >"$workdir/32c3/main.hd"
printf 'def library = 1\n' >"$workdir/32g1/main.hd"
printf 'def shared = 1\n' >"$workdir/32g2/main.hd"
printf 'type shared = | A\n' >"$workdir/32g3/main.hd"

# 32-c 負の対照: library 1 つと契約・実装（main 無し）。
# 工程 1 でライブラリ受理に変わる。この行は工程 1 で書き換える。
cp "$workdir/32b/lib.hd" "$workdir/32neg/main.hd"

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

expect_build_error() {
    local label=$1 entry=$2 file=$3 code=$4 line=$5 col=$6
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- build "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$file:$line:$col: error[$code]: "
    local artifact="${entry%.hd}"
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]] && [ ! -e "$artifact" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$prefix' without artifact, got status=$status '$err')"
    fi
}

expect_error '32-b (check, library ディレクトリの main)' "$workdir/32b/main.hd" main.hd E206 1 1
expect_build_error '32-b (build, library ディレクトリの main)' "$workdir/32b/main.hd" main.hd E206 1 1
expect_error '32-c (1) 同一ファイルの 2 つ目' "$workdir/32c1/main.hd" main.hd E205 2 1
expect_error '32-c (2) 別ファイルの 2 つ目' "$workdir/32c2/a.hd" b.hd E205 1 1
expect_error '32-c (3) .hdi の library' "$workdir/32c3/main.hd" sig.hdi E100 1 1
expect_error '32-g def library' "$workdir/32g1/main.hd" main.hd E100 1 5
expect_error '32-g def shared' "$workdir/32g2/main.hd" main.hd E100 1 5
expect_error '32-g type shared' "$workdir/32g3/main.hd" main.hd E100 1 6
expect_error '32-c 負の対照 (main 無しの library ディレクトリ)' "$workdir/32neg/main.hd" main.hd E206 1 1

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
