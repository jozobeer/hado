#!/usr/bin/env bash
# tier: core
# round: rust-lint
# Rust の lint を守る門番（docs/rounds/rust-lint.md の決定 4）。`cargo clippy` の
# 警告を 1 つも許さない（`-D warnings`）。`--all-targets` は lib・bins・tests を覆う。
# 依存は `--offline` で解決する（AGENTS.md）。
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0
check() {
    local label=$1
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}

clippy_clean() {
    cargo clippy --offline --all-targets -- -D warnings
}

check 'cargo clippy --offline --all-targets -- -D warnings is clean' clippy_clean

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
