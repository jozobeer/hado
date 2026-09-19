#!/usr/bin/env bash
# tier: core
# round: cognitive-complexity
# 認知複雑度を 7 以下に保つ門番（docs/rounds/cognitive-complexity.md の判定 cc-a・cc-c）。
# `clippy.toml` の cognitive-complexity-threshold = 7 を本番コードに掛け、警告を
# 1 つも許さない。対象は library と binary（`--lib --bins`）で、`crates/hado/src/main.rs`
# の `main` のような bin 側の本番コードも見る。テストコードは対象外なので
# `--all-targets` にはしない（`tests/` の 14 件と `src` 内のテストの 3 件が引っかかる）。
# レベルは `-F` にする。`-D` はコード中の内側属性 `#![allow(clippy::cognitive_complexity)]`
# で緩められるが、`-F` は `error[E0453]` にして通さない。逃げ道の綴りは
# 下の grep でも禁じる。
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

complexity_clean() {
    cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity
}

# 本番の関数に allow を付けて閾値を回避していないこと。外側 `#[allow(...)]` と
# 内側 `#![allow(...)]` の両方を見る。付いていれば空一致で赤になる。
no_cognitive_allow() {
    ! grep -rnE '^[[:space:]]*#!?\[allow\([^]]*cognitive_complexity' crates/hado/src
}

check 'cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity is clean' complexity_clean
check 'no #[allow] / #![allow] for cognitive_complexity under crates/hado/src' no_cognitive_allow

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
