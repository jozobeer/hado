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
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

payload_a=$(( (RANDOM << 15) | RANDOM ))
payload_a2=$(( (RANDOM << 15) | RANDOM ))
payload_e=$(( (RANDOM << 15) | RANDOM ))
payload_h=$(( (RANDOM << 15) | RANDOM ))
expected_h=$(( payload_h + 1 + 2 ))

write_source() {
    local kind=$1
    local source=$2
    local dir="$workdir/$kind"
    mkdir -p "$dir"
    printf '%s\n' "$source" >"$dir/main.hd"
}

build_kind() {
    local kind=$1
    "$compiler" build "$workdir/$kind/main.hd"
}

prints_expected() {
    local bin=$1 expected=$2
    local status=0
    "$bin" >"$workdir/got" || status=$?
    [ "$status" -eq 0 ] || return 1
    printf '%s\n' "$expected" >"$workdir/want"
    cmp -s "$workdir/got" "$workdir/want"
}

check_prints() {
    local kind=$1 expected=$2 label=$3
    if build_kind "$kind"; then
        check "$label" prints_expected "$workdir/$kind/main" "$expected"
    else
        check "$label" false
    fi
}

write_source a "$(cat <<EOF
effect Y
again y(value: Int): Void
effect B
once b: Int
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
handler B as Inner
once b => 2
def main = with Collect -> with Inner -> y $payload_a -> y b -> ()
EOF
)"

write_source a2 "$(cat <<EOF
effect Y
again y(value: Int): Void
effect W
again w(value: Int): Void
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return x => x
handler W as Inner
again w value => resume () as rest -> [value + 100, ...rest]
return _ => []
def main = with Collect -> with Inner -> y $payload_a2 -> w 2 -> ()
EOF
)"

write_source b "$(cat <<'EOF'
effect Y
again y(value: Int): Void
effect B
once b: Int
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return x => [x]
handler B as Inner
once b => 2
return _ => 42
def main = with Collect -> with Inner -> y 1 -> ()
EOF
)"

write_source c "$(cat <<'EOF'
effect Y
again y(value: Int): Void
effect B
once b: Int
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
handler B as Inner
once b => 2
def main = with Collect -> with Inner -> y b -> ()
EOF
)"

write_source d "$(cat <<'EOF'
effect A
once a: Int
effect Y
again y(value: Int): Void
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
handler A as Inner
once a => 1
def main = with Collect -> with Inner -> y 3 -> ()
EOF
)"

write_source e "$(cat <<EOF
effect A
once a: Int
effect Y
again y(value: Int): Void
handler A as Outer
once a => 1
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> y $payload_e -> ()
EOF
)"

write_source f "$(cat <<'EOF'
effect Y
again y(value: Int): Void
effect Z
again z(value: Int): Void
handler Y as CollectY
again y value => resume () as rest -> [value, ...rest]
return _ => []
handler Z as CollectZ
again z value => resume () as rest -> [value, ...rest]
return _ => []
def main = with CollectY -> with CollectZ -> y 3 -> ()
EOF
)"

write_source g "$(cat <<'EOF'
effect Y
again y(value: Int): Void
effect E
abort e(reason: Int)
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
handler E as Trap
abort e reason => [reason]
return _ => []
def main = with Collect -> with Trap -> y 7 -> ()
EOF
)"

write_source h "$(cat <<EOF
effect A
once a: Int
effect Y
again y(value: Int): Void
effect B
once b: Int
handler A as Outer
once a => 1
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
handler B as Inner
once b => 2
def main = with Outer -> with Collect -> with Inner -> y ($payload_h + a + b) -> ()
EOF
)"

write_source i "$(cat <<'EOF'
effect Y
again y(value: Int): Void
handler Y as CollectOuter
again y value => resume () as rest -> [value, ...rest]
return x => x
handler Y as CollectInner
again y value => resume () as rest -> [value + 100, ...rest]
return _ => []
def main = with CollectOuter -> with CollectInner -> y 3 -> ()
EOF
)"

write_source j1 "$(cat <<'EOF'
effect Y
again y(value: Int): Void
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
def main = with Collect -> y 3 -> ()
EOF
)"

write_source j2 "$(cat <<'EOF'
effect Y
again y(value: Int): Void
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
def main = with Collect -> y 1 -> y 2 -> ()
EOF
)"

write_source k2 "$(cat <<'EOF'
effect Z
again z(value: Int): Void
effect Y
again y(value: Int): Void
handler Z as Outer
again z value => resume () as rest -> [value, ...rest]
return _ => []
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> y 1 -> z 2 -> ()
EOF
)"

write_source k3 "$(cat <<'EOF'
effect Z
again z(value: Int): Void
effect Y
again y(value: Int): Void
effect B
once b: Int
handler Z as Outer
again z value => resume () as rest -> [value, ...rest]
return _ => []
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
handler B as Inner
once b => 2
def main = with Outer -> with Collect -> with Inner -> y b -> z 5 -> ()
EOF
)"

write_source ok_outer "$(cat <<'EOF'
effect Z
again z(value: Int): Void
effect Y
again y(value: Int): Void
handler Z as Outer
again z value => resume () as rest -> [value, ...rest]
return _ => []
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> z 4 -> ()
EOF
)"

write_source ok_once "$(cat <<'EOF'
effect A
once a: Int
effect Y
again y(value: Int): Void
handler A as Outer
once a => 1
handler Y as Collect
again y value => resume () as rest -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> y 9 -> y a -> ()
EOF
)"

check_prints a "[$payload_a, 2]" "30-a(1) Collect then Inner once yields generated payload then b"
check_prints a2 "[$payload_a2, 102]" "30-a(2) Collect then Inner again yields generated payload then wrapped w"
check_prints b "[1, 42]" "30-b Inner return remains after resume"
check_prints c "[2]" "30-c Collect then Inner yields b"
if build_kind d; then
    check "30-d Collect then Inner A builds" true
else
    check "30-d Collect then Inner A builds" false
fi
check_prints e "[$payload_e]" "30-e Outer then Collect yields generated payload"
check_prints f "[3]" "30-f nested again handlers"
check_prints g "[7]" "30-g Collect then abort handler"
check_prints h "[$expected_h]" "30-h three handlers yield generated payload plus a and b"
check_prints i "[103]" "30-i inner again wins"
check_prints j1 "[3]" "30-j solo Collect"
check_prints j2 "[1, 2]" "30-j solo Collect yields twice"
# R31 は拒否を値の一致に置き換える。クラッシュと黙った誤答を防ぐ
# 30-k の保証に正しい実行結果を加えるため、門番を緩めてはいない。
check_prints k2 "[2]" "30-k two-level resume captures outer"
check_prints k3 "[5]" "30-k three-level resume captures outer"
check_prints ok_outer "[4]" "30-k control outer again without resume"
check_prints ok_once "[9, 1]" "30-k control resume may call outer once"

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
