#!/usr/bin/env bash
# 凍結した文書（docs/ledger.md・docs/plan.md）のハッシュを docs/frozen-docs.sha256 に
# 記録する。対象は丸ごとで、節を切り出すパーサは持たない（決定 D4。
# docs/plans/issue-backlog.md）。
#
#   bash scripts/frozen-docs-lock.sh           作り直す（行数を出す）
#   bash scripts/frozen-docs-lock.sh --check   照合する（食い違えば差分を出して非 0）
#
# 形は scripts/corpus-lock.sh と同じ（引数なしで作り直し、--check で照合、
# 作り直しの最後に行数を出す）。凍結する対象の一覧は frozen_docs の 1 か所だけが持つ。
set -euo pipefail
cd "$(dirname "$0")/.."

frozen_docs() {
    printf '%s\n' docs/ledger.md docs/plan.md
}

frozen_docs_bytes() {
    frozen_docs | LC_ALL=C sort | xargs -r sha256sum
}

check=0
if [ "${1:-}" = "--check" ]; then
    [ "$#" -eq 1 ]
    check=1
elif [ "$#" -ne 0 ]; then
    printf 'usage: %s [--check]\n' "$0" >&2
    exit 2
fi

generated=$(mktemp)
trap 'rm -f "$generated"' EXIT
frozen_docs_bytes >"$generated"

if [ "$check" -eq 1 ]; then
    diff -u docs/frozen-docs.sha256 "$generated"
    exit 0
fi

cp "$generated" docs/frozen-docs.sha256
grep -c '^' docs/frozen-docs.sha256
