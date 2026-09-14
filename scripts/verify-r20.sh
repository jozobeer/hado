#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh
if cargo_test_cached test --offline --test r20_entries; then
    echo 'OK: R20 generated entries, file-local values and diagnostics'
    echo 'PASSED: 1/1'
else
    echo 'NG: R20 generated entries, file-local values and diagnostics'
    echo 'PASSED: 0/1'
    exit 1
fi
