#!/usr/bin/env bash
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

completion_count() {
    local n
    n=$(grep -c "self.completion(" crates/hado/src/mono.rs)
    [ "$n" = 2 ]
}

check '28-a/b/c nested handler probes' cargo test --offline --test r28_effects
check 'completion() stays at with and resume' completion_count

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
