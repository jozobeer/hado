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
payload_a=$(( (RANDOM << 15) | RANDOM ))
payload_b=$(( payload_a + 1 + RANDOM ))

write_source() {
    local kind=$1 body=$2
    mkdir -p "$workdir/$kind"
    cat >"$workdir/$kind/main.hd" <<EOF
effect Y
capture y(value: Int): Unit
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
$body
EOF
}

checks_type() {
    cargo run --offline -q -- check "$workdir/$1/main.hd"
}

rejects_build() {
    local kind=$1
    checks_type "$kind" || return 1
    if cargo run --offline -q -- build "$workdir/$kind/main.hd" 2>"$workdir/$kind/err"; then
        return 1
    fi
    cat "$workdir/$kind/err"
    rg -q -F 'capture requires do or tail position' "$workdir/$kind/err" &&
        [ ! -e "$workdir/$kind/main" ]
}

check_prints() {
    local kind=$1 expected=$2
    checks_type "$kind" || return 1
    cargo run --offline -q -- build "$workdir/$kind/main.hd" || return 1
    "$workdir/$kind/main" >"$workdir/$kind/got" || return 1
    cat "$workdir/$kind/got"
    printf '%s\n' "$expected" >"$workdir/$kind/want"
    cmp -s "$workdir/$kind/got" "$workdir/$kind/want"
}

write_source list "def main = with Collect -> [y $payload_a, y $payload_b]"
write_source let "def main = with Collect -> let u = y $payload_a -> [u]"
write_source argument "def f (u: Unit) => [u]
def main = with Collect -> f (y $payload_a)"
write_source tail "def main = with Collect -> y $payload_a"
write_source do "def main = with Collect -> do y $payload_a -> y $payload_b"
write_source branch "def main = with Collect -> if true => y $payload_a | y $payload_b"

check '31-0 拒否 (list)' rejects_build list
check '31-0 拒否 (let)' rejects_build let
check '31-0 拒否 (argument)' rejects_build argument
check '31-0 負の対照 (tail)' check_prints tail "[$payload_a]"
check '31-0 負の対照 (do)' check_prints do "[$payload_a, $payload_b]"
check '31-0 負の対照 (branch tail)' check_prints branch "[$payload_a]"

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
