#!/usr/bin/env bash
# tier: full
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

if ! cargo build --offline --bin hado; then
    echo 'PASSED: 0/0'
    exit 1
fi

hado_target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
compiler=$(readlink -f "$hado_target_dir/debug/hado")
helper=$(readlink -f scripts/exec-stdout-epipe.py)
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

alphabet=abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ
int_value=$(( (RANDOM << 15) | RANDOM ))
string_value=
i=0
while [ "$i" -lt 12 ]; do
    string_value=${string_value}${alphabet:$((RANDOM % 52)):1}
    i=$((i + 1))
done
char_value=${alphabet:$((RANDOM % 26)):1}

status_of() {
    local status=0
    "$@" || status=$?
    echo "$status"
}

closed_stdout_is() {
    local want=$1 bin=$2
    local status=0
    "$bin" >&- || status=$?
    [ "$status" -eq "$want" ]
}

closed_both_is() {
    local want=$1 bin=$2
    local status=0
    "$bin" >&- 2>&- || status=$?
    [ "$status" -eq "$want" ]
}

redirected_is() {
    local want=$1 bin=$2 dest=$3
    local status=0
    "$bin" >"$dest" || status=$?
    [ "$status" -eq "$want" ]
}

prints_expected() {
    local bin=$1 expected=$2
    local status=0
    "$bin" >"$workdir/got" || status=$?
    [ "$status" -eq 0 ] || return 1
    printf '%s\n' "$expected" >"$workdir/want"
    cmp -s "$workdir/got" "$workdir/want"
}

prints_expected_pipe() {
    local bin=$1 expected=$2
    local status=0
    local got
    got=$("$bin") || status=$?
    [ "$status" -eq 0 ] && [ "$got" = "$expected" ]
}

build_kind() {
    local kind=$1 source=$2
    local dir="$workdir/$kind"
    mkdir -p "$dir"
    printf '%s\n' "$source" >"$dir/main.hd"
    "$compiler" build "$dir/main.hd"
}

exercise() {
    local kind=$1 expected=$2
    local bin="$workdir/$kind/main"
    check "29-a $kind closed stdout is 1" closed_stdout_is 1 "$bin"
    check "29-b $kind /dev/full is 1" redirected_is 1 "$bin" /dev/full
    local epipe
    epipe=$(status_of python3 "$helper" "$bin")
    check "29-c $kind epipe is 1" [ "$epipe" -eq 1 ]
    check "29-c $kind epipe is not 141" [ "$epipe" -ne 141 ]
    check "29-d $kind stdout and stderr closed is 1" closed_both_is 1 "$bin"
    check "29-e $kind prints generated value" prints_expected "$bin" "$expected"
    check "29-e $kind pipe prints generated value" prints_expected_pipe "$bin" "$expected"
    check "29-e $kind /dev/null is 0" redirected_is 0 "$bin" /dev/null
}

if ! build_kind int "def main = $int_value" \
    || ! build_kind string "def main = \"$string_value\"" \
    || ! build_kind char "def main = '$char_value'"; then
    echo 'NG: build fixture'
    exit 1
fi

exercise int "$int_value"
exercise string "$string_value"
exercise char "$char_value"

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
