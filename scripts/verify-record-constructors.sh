#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
if cargo test --offline --test named_record_constructors --test named_record_order; then
    echo 'OK: named-field constructors, patterns, abstraction and diagnostics'
    echo 'PASSED: 1/1'
else
    echo 'NG: named-field constructors, patterns, abstraction and diagnostics'
    echo 'PASSED: 0/1'
    exit 1
fi
