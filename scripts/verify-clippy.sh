#!/usr/bin/env bash
# tier: core
# round: rust-lint
# Rust の lint を守る門番（docs/rounds/rust-lint.md の決定 4）。`cargo clippy` の
# 警告を 1 つも許さない（`-D warnings`）。`--all-targets` は lib・bins・tests を覆う。
# 依存は `--offline` で解決する（AGENTS.md）。
# `crates/hado/float_format.rs` は build.rs が RUSTC を直接呼ぶ standalone の
# ソースで、Cargo のターゲットではないので `--all-targets` の外にある。
# build.rs と同じ `--crate-type staticlib -C panic=abort` で clippy-driver をかける。
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

clippy_standalone_clean() {
    local out status
    out=$(mktemp -d) || return 1
    clippy-driver \
        --crate-type staticlib \
        --edition=2021 \
        --crate-name hado_float_format \
        -C panic=abort \
        --emit=metadata \
        -D warnings \
        crates/hado/float_format.rs \
        -o "$out/float_format.rmeta"
    status=$?
    rm -rf "$out"
    return "$status"
}

check 'cargo clippy --offline --all-targets -- -D warnings is clean' clippy_clean
check 'clippy-driver --crate-type staticlib -D warnings on crates/hado/float_format.rs is clean' clippy_standalone_clean

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
