#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
if cargo test --offline --test r16_sections; then
    echo 'PASSED: 1/1'
else
    echo 'PASSED: 0/1'
    exit 1
fi
