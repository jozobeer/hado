#!/usr/bin/env bash
# tests/corpus.sha256 を docs/corpus.md の定義どおりに作り直す。
set -euo pipefail
cd "$(dirname "$0")/.."

corpus_lock_bytes() {
    {
        find tests/accept tests/reject tests/library -type f \( -name '*.hd' -o -name '*.hdi' \)
        printf '%s\n' stdlib/*.hd
    } | LC_ALL=C sort | xargs -r sha256sum
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
corpus_lock_bytes >"$generated"

if [ "$check" -eq 1 ]; then
    diff -u tests/corpus.sha256 "$generated"
    exit 0
fi

cp "$generated" tests/corpus.sha256
grep -c '^' tests/corpus.sha256
