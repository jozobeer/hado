#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
if cargo test --offline --test r23_syntax; then
    echo 'OK: R23 generated effect syntax, E123/E124 anchors and grammar examples'
    echo 'PASSED: 1/1'
else
    echo 'NG: R23 generated effect syntax, E123/E124 anchors and grammar examples'
    echo 'PASSED: 0/1'
    exit 1
fi
