#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh

passed=0
total=0
check() {
    local label="$1"
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}

e1xx_match() {
    local spec disk
    spec=$(sed -n 's/^| \(E1[0-9][0-9]\) |.*/\1/p' docs/spec.md | sort -u)
    disk=$(find tests/reject -mindepth 1 -maxdepth 1 -type d -name 'E1[0-9][0-9]' | sed 's|.*/||' | sort)
    [ -n "$spec" ] && [ "$spec" = "$disk" ]
}

# Why not verify-r9.sh: R9-1 は parse_corpus の corpus_counts を同梱し、計画はそれを R26 まで赤とする。
check '23-a §59 生成プログラムの往復' cargo_test_cached test --offline --test r9_grammar
check '23-b E1xx が §56 の表と一致' e1xx_match
check '23-c §37/§38 の例と E123/E124 の位置摂動' cargo_test_cached test --offline --test r23_syntax

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
