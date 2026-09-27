#!/usr/bin/env bash
# tier: core
# round: frozen-ledger-guard
#
# 凍結した文書（docs/ledger.md・docs/plan.md）の書き換えを止める門番。
# ハッシュは docs/frozen-docs.sha256 が持ち、scripts/frozen-docs-lock.sh --check が
# バイト一致を diff -u で見る（scripts/corpus-lock.sh と同じ形。決定 D4）。
# 節を切り出すパーサは持たず、丸ごとのバイト一致だけを見る。
#
# tier は core: 文書 2 本と lock を読むだけで、PR を回す層に要る検出力だから
# （scripts/ci-run.sh は pull_request を core で回す。決定 D3 が
# scripts/verify-spec-contracts.sh を core にしたのと同じ）。
set -uo pipefail
cd "$(dirname "$0")/.."

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

frozen_docs_lock() {
    bash scripts/frozen-docs-lock.sh --check
}

check 'the frozen documents match the recorded hashes' frozen_docs_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
