#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
if cargo test --offline --test r19_contracts; then
    echo 'OK: R19 generated contracts, diagnostics and positions'
    echo 'PASSED: 1/1'
else
    echo 'NG: R19 generated contracts, diagnostics and positions'
    echo 'PASSED: 0/1'
    exit 1
fi
