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
    local kind=$1 body=$2 handler=${3:-Collect}
    mkdir -p "$workdir/$kind"
    cat >"$workdir/$kind/main.hd" <<EOF
effect Y
capture y(value: Int): Unit
handler Y as $handler
capture y value => let rest = resume () -> [value, ...rest]
$(if [ "$handler" = Keep ]; then echo "return x => x"; else echo "return _ => []"; fi)
$body
EOF
}

checks_type() {
    cargo run --offline -q -- check "$workdir/$1/main.hd"
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
write_source let "def f u => if u == () => [7] | []
def main = with Keep -> let u = y $payload_a -> f u" Keep
write_source argument "def f u => if u == () => [9] | []
def main = with Keep -> f (y $payload_a)" Keep
write_source record "type P = { a: Unit; b: Int }
def main = with Keep -> let p = { a = y $payload_a; b = 4 } -> [p.b]" Keep
write_source pipe "def f u => if u == () => [8] | []
def main = with Keep -> y $payload_a |> f" Keep
write_source operand "def n u => if u == () => 5 | 0
def main = with Keep -> [n (y $payload_a) + n (y $payload_b)]" Keep
write_source tail "def main = with Collect -> y $payload_a"
write_source do "def main = with Collect -> do y $payload_a -> y $payload_b"
write_source branch "def main = with Collect -> if true => y $payload_a | y $payload_b"
write_source short_circuit "def g u => if u == () => true | false
def main = with Collect -> let b = false && g (y $payload_a) -> if b => y $payload_b | y 3"
write_source nested "def main = with Collect -> let inner = with Collect -> y $payload_a -> do y $payload_b -> inner"

check '31-d (list, generated payload)' check_prints list "[$payload_a, $payload_b]"
check '31-d (let)' check_prints let "[$payload_a, 7]"
check '31-d (argument)' check_prints argument "[$payload_a, 9]"
check '31-d (record)' check_prints record "[$payload_a, 4]"
check '31-d (pipe)' check_prints pipe "[$payload_a, 8]"
check '31-d (operand, generated payload)' check_prints operand "[$payload_a, $payload_b, $((5 + 5))]"
check '31-d 負の対照 (tail)' check_prints tail "[$payload_a]"
check '31-d 負の対照 (do)' check_prints do "[$payload_a, $payload_b]"
check '31-d 負の対照 (branch tail)' check_prints branch "[$payload_a]"
check '31-d 負の対照 (short circuit)' check_prints short_circuit "[3]"
check '31-d 負の対照 (with body)' check_prints nested "[$payload_b]"

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
