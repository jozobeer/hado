#!/usr/bin/env bash
# tier: full
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

check 'R9-1 字句・全文法の往復・空白摂動' \
    cargo_test_cached test --offline --no-fail-fast --test r9_syntax --test r9_grammar --test parse_corpus
check 'R9-2 検査段階・位置順・全拒否コードの位置摂動' \
    cargo_test_cached test --offline --no-fail-fast --test r9_order --test check_cli
check 'R9-3 名前解決・未使用宣言・signature・prelude' \
    cargo_test_cached test --offline --no-fail-fast --test r9_names --test r9_unused --test r9_module_types --test prelude
check 'R9-4 網羅性・束縛の使用・冗長性' \
    cargo_test_cached test --offline --no-fail-fast --test r9_coverage
check 'R9-5 印字・標準出力と標準エラー・終了コード' \
    cargo_test_cached test --offline --no-fail-fast --test r9_print --test emit_mono --test corpus

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
