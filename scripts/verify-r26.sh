#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh

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

check '26-a/b/c/e reply abort capture E706 probes' cargo_test_cached test --offline --test r26_effects
check '26-d reject codes match §56' reject_codes_match

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
