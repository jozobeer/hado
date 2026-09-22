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

# 呼び出し箇所の本数は R28 の実装の形であって約束ではない。R30 が
# make_continuation の completion() を正当に外したので落ちるようになった。28-a〜28-c の保証は
# tests/accept の nested_unrelated_abort / nested_matching_abort / abort_arm_* が持ち、corpus 判定が
# 実走で status と stdout を照合する。r28_effects の 6 件は corpus と重複していたので test-prune で削った。
check '28-a/b/c nested handler probes' cargo_test_cached test --offline --test corpus

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
