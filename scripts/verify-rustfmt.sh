#!/usr/bin/env bash
# tier: quick
# round: rust-lint
# Rust の整形を守る門番（docs/rounds/rust-lint.md の決定 1・4）。rustfmt の設定は
# 既定で、`rustfmt.toml` を置かない。木全体が差分を出さないことだけを見る。
# 直すのは `cargo fmt --all`。
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

rustfmt_clean() {
    cargo fmt --all --check
}

check 'cargo fmt --all --check reports no diff' rustfmt_clean

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
