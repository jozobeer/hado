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


write_outer_source() {
    local kind=$1 first=$2 outer=$3 last=$4 returns=$5
    mkdir -p "$workdir/$kind"
    cat >"$workdir/$kind/main.hd" <<EOF
effect Z
capture z(value: Int): Unit
effect Y
capture y(value: Int): Unit
handler Z as Outer
capture z value => let rest = resume () -> [value, ...rest]
return $returns
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> do y $first -> do z $outer -> $last
EOF
}

write_spread_source() {
    local kind=$1 inner=$2 outer=$3
    mkdir -p "$workdir/$kind"
    cat >"$workdir/$kind/main.hd" <<EOF
effect Z
capture z(value: Int): Unit
effect W
capture w(value: Int): Unit
handler Z as Outer
capture z value => let rest = resume () -> [value, ...rest]
return x => x
handler W as Inner
capture w value => [value + 100, ...resume ()]
return _ => []
def main = with Outer -> with Inner -> do w $inner -> do z $outer -> ()
EOF
}

write_outer_source a 1 2 "()" "_ => []"
write_outer_source b 1 2 "()" "x => x"
write_outer_source c 1 2 "do y 3 -> ()" "x => x"
write_spread_source f 2 3
write_outer_source generated_a "$payload_a" "$payload_b" "()" "_ => []"
payload_c=$((payload_b + 1 + RANDOM))
write_outer_source generated_c "$payload_a" "$payload_b" "do y $payload_c -> ()" "x => x"
write_spread_source generated_f "$payload_a" "$payload_b"

mkdir -p "$workdir/e" "$workdir/ok_outer" "$workdir/ok_reply"
cat >"$workdir/e/main.hd" <<'EOF'
effect Z
capture z(value: Int): Unit
effect Y
capture y(value: Int): Unit
effect B
reply b: Int
handler Z as Outer
capture z value => let rest = resume () -> [value, ...rest]
return _ => []
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
handler B as Inner
reply b => 2
def main = with Outer -> with Collect -> with Inner -> do y b -> do z 5 -> ()
EOF
cat >"$workdir/ok_outer/main.hd" <<'EOF'
effect Z
capture z(value: Int): Unit
effect Y
capture y(value: Int): Unit
handler Z as Outer
capture z value => let rest = resume () -> [value, ...rest]
return _ => []
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> do z 4 -> ()
EOF
cat >"$workdir/ok_reply/main.hd" <<'EOF'
effect A
reply a: Int
effect Y
capture y(value: Int): Unit
handler A as Outer
reply a => 1
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> do y 9 -> do y a -> ()
EOF

check '31-a outer return discards inner answer' check_prints a "[2]"
check '31-b outer return keeps inner answer' check_prints b "[2, 1]"
check '31-c inner handler is reinstalled after outer resume' check_prints c "[2, 1, 3]"
check '31-e reply handler between captures' check_prints e "[5]"
check '31-f resume in list spread' check_prints f "[3, 102]"
check '31-g outer capture without resume' check_prints ok_outer "[4]"
check '31-g resume calls outer reply' check_prints ok_reply "[9, 1]"
check '31-k generated 31-a' check_prints generated_a "[$payload_b]"
check '31-k generated 31-c' check_prints generated_c "[$payload_b, $payload_a, $payload_c]"
check '31-k generated 31-f' check_prints generated_f "[$payload_b, $((payload_a + 100))]"

write_ask_source() {
    local kind=$1 seed=$2
    mkdir -p "$workdir/$kind"
    cat >"$workdir/$kind/main.hd" <<EOF
effect Ask
capture ask(seed: Int): Int
handler Ask as Twice
capture ask seed => resume (seed * 2)
return x => x
def main = with Twice -> let v = ask $seed -> [v]
EOF
}

write_mixed_source() {
    local kind=$1 body=$2 prefix=$3
    mkdir -p "$workdir/$kind"
    cat >"$workdir/$kind/main.hd" <<EOF
effect S
capture s(value: String): Unit
effect Y
capture y(value: Int): Unit
handler S as Names
capture s value => let rest = resume () -> [value, ...rest]
return x => x
handler Y as Collect
capture y value => let rest = resume () -> [String.concat "$prefix" (Int.to_string value), ...rest]
return _ => []
def main = with Names -> with Collect -> $body
EOF
}

write_ask_source h 21
write_ask_source generated_h "$payload_a"
write_mixed_source i 'do s "a" -> do y 1 -> ()' n
write_mixed_source j 'do y 1 -> do s "b" -> ()' ''
check '31-h non-Unit capture result' check_prints h '[42]'
check '31-i mixed payloads' check_prints i '["a", "n1"]'
check '31-j mixed payloads through resume' check_prints j '["b", "1"]'
check '31-k generated 31-h' check_prints generated_h "[$((payload_a * 2))]"

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
