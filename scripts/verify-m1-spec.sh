#!/bin/bash
# M1 spec acceptance check 3 (see docs/plan.md, M1 and 33 節). Run from the repository root.
#
# 検査 1（見出し）・2（新設節）・4（対応表）・5（条件列の逐語）は文書管理であって
# 処理系との契約ではない。docs/plan.md 33 節に従って scripts/lint-docs.sh（full 層）
# へ移した。ここに残るのは §56 のエラーコード表＝処理系との契約だけである。
# total= は先頭に数値リテラルで置く（verify-m2.sh と verify-m3.sh が読む）。
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=1
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

SPEC="docs/spec.md"

TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT

heading_count() {
    grep -cE "^## ${1}\\." "$SPEC" 2>/dev/null || true
}

heading_exists() {
    [ "$(heading_count "$1")" -ge 1 ]
}

# --- check 3: error code table ---
awk '
    /^## 56\./ { on=1 }
    on { print }
' "$SPEC" >"$TMPD/sec50.txt"
awk -F '|' '
    {
        code=$2
        name=$3
        secs=$4
        gsub(/^ +| +$/, "", code)
        gsub(/^ +| +$/, "", name)
        gsub(/^ +| +$/, "", secs)
        if (code ~ /^E[1-7][0-9][0-9]$/) {
            print code "\t" name "\t" secs
        }
    }
' "$TMPD/sec50.txt" >"$TMPD/codes.tsv"

ec_reasons=""
if [ ! -s "$TMPD/codes.tsv" ]; then
    ec_reasons="$ec_reasons empty-table"
else
    while IFS=$'\t' read -r code name secs; do
        if ! printf '%s' "$code" | grep -qE '^E[1-7][0-9][0-9]$'; then
            ec_reasons="$ec_reasons bad-form-$code"
        fi
        if [ -z "$name" ]; then
            ec_reasons="$ec_reasons empty-name-$code"
        fi
        if [ -z "$secs" ]; then
            ec_reasons="$ec_reasons empty-sec-$code"
        else
            printf '%s\n' "$secs" | grep -oE '§[0-9]+' | while read -r ref; do
                num=${ref#§}
                if ! heading_exists "$num"; then
                    echo " missing-sec-$code-$ref"
                fi
            done >"$TMPD/secmiss.$$"
            if [ -s "$TMPD/secmiss.$$" ]; then
                ec_reasons="$ec_reasons$(cat "$TMPD/secmiss.$$")"
            fi
            rm -f "$TMPD/secmiss.$$"
        fi
    done <"$TMPD/codes.tsv"

    cut -f1 "$TMPD/codes.tsv" | sort | uniq -d >"$TMPD/dups.txt"
    if [ -s "$TMPD/dups.txt" ]; then
        while read -r d; do
            ec_reasons="$ec_reasons dup-$d"
        done <"$TMPD/dups.txt"
    fi

    s=1
    while [ "$s" -le 7 ]; do
        if ! grep -qE "^E${s}[0-9][0-9]" "$TMPD/codes.tsv"; then
            ec_reasons="$ec_reasons missing-E${s}xx"
        fi
        s=$((s + 1))
    done
fi
if [ -z "$ec_reasons" ]; then
    pass 3 "error-codes"
else
    fail 3 "error-codes:$ec_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
