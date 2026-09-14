#!/usr/bin/env bash
# 門番キャッシュ（scripts/lib/gate-cache.sh）の判定器。
# 目的は 6 つ。内容が同じなら実走は 1 回。内容が変われば実走する。失敗は保存しない。
# 引数が違えば別の結果になる。走行スコープが入れ子の run-gates に漏れない。
# 古い走行のスコープを残さない。
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
runid=""
cleanup() {
    rm -rf "$workdir"
    [ -n "$runid" ] && rm -rf target/gate-cache/"$runid"-*
}
trap cleanup EXIT
runid="gate-cache-selftest-$$-$RANDOM"

replay_case() {
    local counter="$workdir/replay-counter"
    printf '0\n' >"$counter"
    HADO_GATE_CACHE_RUN="$runid-replay" bash -c '
        source scripts/lib/gate-cache.sh
        counter=$1
        bump() {
            local n
            n=$(cat "$counter")
            printf "%s\n" "$((n + 1))" >"$counter"
            printf "out-%s\n" "$n"
        }
        gate_cached demo bump
        gate_cached demo bump
    ' _ "$counter" >"$workdir/replay.out"
    [ "$(cat "$counter")" = "1" ] &&
        [ "$(wc -l <"$workdir/replay.out")" -eq 2 ] &&
        [ "$(sort -u "$workdir/replay.out")" = "out-0" ]
}

argv_case() {
    local counter="$workdir/argv-counter"
    printf '0\n' >"$counter"
    HADO_GATE_CACHE_RUN="$runid-argv" bash -c '
        source scripts/lib/gate-cache.sh
        counter=$1
        bump() {
            local n
            n=$(cat "$counter")
            printf "%s\n" "$((n + 1))" >"$counter"
        }
        gate_cached demo bump a
        gate_cached demo bump b
        gate_cached demo bump a
    ' _ "$counter"
    [ "$(cat "$counter")" = "2" ]
}

failure_case() {
    local counter="$workdir/failure-counter"
    printf '0\n' >"$counter"
    HADO_GATE_CACHE_RUN="$runid-failure" bash -c '
        source scripts/lib/gate-cache.sh
        counter=$1
        boom() {
            local n
            n=$(cat "$counter")
            printf "%s\n" "$((n + 1))" >"$counter"
            printf "boom\n"
            return 3
        }
        gate_cached demo boom
        first=$?
        gate_cached demo boom
        second=$?
        [ "$first" -eq 3 ] && [ "$second" -eq 3 ] && [ "$(cat "$counter")" = "2" ]
    ' _ "$counter" >"$workdir/failure.out"
    [ "$(cat "$counter")" = "2" ] && [ "$(wc -l <"$workdir/failure.out")" -eq 2 ]
}

content_case() {
    HADO_GATE_CACHE_RUN="$runid-content" bash -c '
        source scripts/lib/gate-cache.sh
        probe=.gate-cache-selftest-probe
        rm -f "$probe"
        before=$(hado_gate_fingerprint)
        printf "x\n" >"$probe"
        during=$(hado_gate_fingerprint)
        rm -f "$probe"
        after=$(hado_gate_fingerprint)
        [ "$before" != "$during" ] && [ "$before" = "$after" ]
    '
}

scope_case() {
    local base="$workdir/scope"
    mkdir -p "$base/scripts/lib"
    cp scripts/run-gates.sh "$base/scripts/run-gates.sh"
    cp scripts/lib/gate-cache.sh "$base/scripts/lib/gate-cache.sh"
    cat >"$base/scripts/verify-scope.sh" <<'EOS'
#!/usr/bin/env bash
printf '%s\n' "${HADO_GATE_CACHE_RUN:-unset}" >"$SCOPE_OUT"
echo 'PASSED: 1/1'
EOS
    SCOPE_OUT="$workdir/scope.out" HADO_GATE_CACHE_RUN=inherited \
        bash "$base/scripts/run-gates.sh" "$workdir/scope-logs" verify-scope >/dev/null 2>&1
    local recorded
    recorded=$(cat "$workdir/scope.out" 2>/dev/null || true)
    [ -n "$recorded" ] && [ "$recorded" != "inherited" ] && [ "$recorded" != "unset" ]
}

prune_case() {
    local base=target/gate-cache
    local old="$base/selftest-old-$$"
    local fresh="$base/selftest-fresh-$$"
    mkdir -p "$old" "$fresh"
    touch -d '2 days ago' "$old"
    HADO_GATE_CACHE_RUN= bash -c 'source scripts/lib/gate-cache.sh' >/dev/null 2>&1
    [ ! -d "$old" ] && [ -d "$fresh" ]
    local ok=$?
    rm -rf "$old" "$fresh"
    return "$ok"
}

check 'cache replays identical content once' replay_case
check 'cache separates different arguments' argv_case
check 'cache never stores failure' failure_case
check 'fingerprint follows content' content_case
check 'run-gates gives each run its own scope' scope_case
check 'stale run scopes are pruned' prune_case

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
