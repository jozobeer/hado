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

r24_reject_match() {
    local spec disk
    spec=$(sed -n 's/^| \(E308\|E522\|E605\) |.*/\1/p' docs/spec.md | sort -u)
    disk=$(printf '%s\n' $spec | while read -r code; do
        [ -d "tests/reject/$code" ] && printf '%s\n' "$code"
    done | sort)
    [ -n "$spec" ] && [ "$spec" = "$disk" ]
}

check '24-a..e generated effect-row probes' cargo_test_cached test --offline --test r24_effects
check '24-f E308 E522 E605 が §57 の表と一致' r24_reject_match

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
