#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh
if cargo_test_cached test --offline --test r22_namespaces; then
    echo 'OK: R22 generated namespaces, visibility and diagnostic positions'
    echo 'PASSED: 1/1'
else
    echo 'NG: R22 generated namespaces, visibility and diagnostic positions'
    echo 'PASSED: 0/1'
    exit 1
fi
