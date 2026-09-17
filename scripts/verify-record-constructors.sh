#!/usr/bin/env bash
# tier: full
set -uo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh
if cargo_test_cached test --offline --test named_record_constructors --test named_record_order; then
    echo 'OK: named-field constructors, patterns, abstraction and diagnostics'
    echo 'PASSED: 1/1'
else
    echo 'NG: named-field constructors, patterns, abstraction and diagnostics'
    echo 'PASSED: 0/1'
    exit 1
fi
