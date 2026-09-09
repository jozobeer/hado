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

reject_codes_match() {
    local spec disk
    spec=$(sed -n 's/^| \(E[1-7][0-9][0-9]\) |.*/\1/p' docs/spec.md | sort -u)
    disk=$(find tests/reject -mindepth 1 -maxdepth 1 -type d -name 'E[0-9][0-9][0-9]' | sed 's|.*/||' | sort)
    [ -n "$spec" ] && [ "$spec" = "$disk" ]
}

# Why not 26-c: capture の evidence passing はこのラウンドでは未実装。
check '26-a/b/e reply abort E706 probes' cargo test --offline --test r26_effects
check '26-d reject codes match §50' reject_codes_match

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
