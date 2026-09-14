#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh
if cargo_test_cached test --offline --test r18_spelling; then
    echo 'OK: R18 generated spelling, execution, rejection and position probes'
    echo 'PASSED: 1/1'
else
    echo 'NG: R18 generated spelling, execution, rejection and position probes'
    echo 'PASSED: 0/1'
    exit 1
fi
