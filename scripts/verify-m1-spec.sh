#!/bin/bash
# M1 spec acceptance checks 1..6 (see docs/plan.md, M1). Run from the repository root.
# Each check is independent; all run to the end. Non-zero exit if any fails.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=6
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

SPEC="docs/spec.md"
PLAN="docs/plan.md"
COV="docs/spec-coverage.md"

TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT

heading_count() {
    grep -cE "^#+ ${1}\\." "$SPEC" 2>/dev/null || true
}

heading_exists() {
    [ "$(heading_count "$1")" -ge 1 ]
}

# --- check 1: existing section numbers ---
del=$(git diff -- "$SPEC" | grep -c '^-#' || true)
missing=""
n=1
while [ "$n" -le 42 ]; do
    c=$(heading_count "$n")
    if [ "$c" -ne 1 ]; then
        missing="$missing #$n:$c"
    fi
    n=$((n + 1))
done
if [ "$del" -eq 0 ] && [ -z "$missing" ]; then
    pass 1 "headings"
else
    fail 1 "deleted-headings=$del count-mismatch=$missing"
fi

# --- check 2: new sections 43..53 after 42 ---
line42=$(grep -nE '^# 42\.' "$SPEC" | head -n 1 | cut -d: -f1)
line42=${line42:-0}
ns_reasons=""
n=43
while [ "$n" -le 53 ]; do
    mapfile -t hits < <(grep -nE "^# ${n}\\." "$SPEC" || true)
    if [ "${#hits[@]}" -eq 0 ]; then
        ns_reasons="$ns_reasons missing-#$n"
    else
        for h in "${hits[@]}"; do
            ln=${h%%:*}
            if [ "$ln" -le "$line42" ]; then
                ns_reasons="$ns_reasons #$n-before-42"
            fi
        done
    fi
    n=$((n + 1))
done
if [ -z "$ns_reasons" ]; then
    pass 2 "new-sections"
else
    fail 2 "new-sections:$ns_reasons"
fi

# --- check 3: DivisionError ---
arith=$(grep -cF 'ArithmeticError' "$SPEC" || true)
div=$(grep -cF 'DivisionError' "$SPEC" || true)
if [ "$arith" -eq 0 ] && [ "$div" -ge 1 ]; then
    pass 3 "DivisionError"
else
    fail 3 "ArithmeticError=$arith DivisionError=$div"
fi

# --- check 4: error code table ---
awk '
    /^# 50\./ { on=1 }
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
    pass 4 "error-codes"
else
    fail 4 "error-codes:$ec_reasons"
fi

# --- check 5: coverage table (row count derived from the plan tables 2.2, 2.3, 9.2) ---
cov_reasons=""
if [ ! -f "$COV" ]; then
    cov_reasons="$cov_reasons missing-file"
else
    awk -F '|' '
        {
            t=$2; name=$3; secs=$4
            gsub(/^ +| +$/, "", t)
            gsub(/^ +| +$/, "", name)
            gsub(/^ +| +$/, "", secs)
            if (t=="計画の表" || t=="" || t ~ /^-+$/) next
            if (name=="" || name=="項目名") next
            print name "\t" secs
        }
    ' "$COV" >"$TMPD/cov.tsv"

    awk -F '|' '
        /^### 2\.2/ { p=1; next }
        /^### 2\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan22.txt"
    awk -F '|' '
        /^### 2\.3/ { p=1; next }
        /^## 3\./ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan23.txt"
    awk -F '|' '
        /^### 9\.2/ { p=1; next }
        /^### 9\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan92.txt"
    cat "$TMPD/plan22.txt" "$TMPD/plan23.txt" "$TMPD/plan92.txt" >"$TMPD/plan_items.txt"
    nrows=$(wc -l <"$TMPD/cov.tsv" | tr -d ' ')
    nplan=$(wc -l <"$TMPD/plan_items.txt" | tr -d ' ')
    if [ "$nrows" -ne "$nplan" ]; then
        cov_reasons="$cov_reasons rows=$nrows(want-$nplan)"
    fi
    cut -f1 "$TMPD/cov.tsv" >"$TMPD/cov_items.txt"

    while IFS= read -r item || [ -n "$item" ]; do
        found=0
        while IFS= read -r citem || [ -n "$citem" ]; do
            if [ "$item" = "$citem" ]; then
                found=1
                break
            fi
        done <"$TMPD/cov_items.txt"
        if [ "$found" -eq 0 ]; then
            cov_reasons="$cov_reasons missing-item"
            break
        fi
    done <"$TMPD/plan_items.txt"

    while IFS= read -r citem || [ -n "$citem" ]; do
        found=0
        while IFS= read -r item || [ -n "$item" ]; do
            if [ "$citem" = "$item" ]; then
                found=1
                break
            fi
        done <"$TMPD/plan_items.txt"
        if [ "$found" -eq 0 ]; then
            cov_reasons="$cov_reasons extra-item"
            break
        fi
    done <"$TMPD/cov_items.txt"

    while IFS=$'\t' read -r name secs; do
        printf '%s\n' "$secs" | grep -oE '§[0-9]+' | while read -r ref; do
            num=${ref#§}
            if ! heading_exists "$num"; then
                echo " missing-sec-$ref"
            fi
        done >"$TMPD/covsec.$$"
        if [ -s "$TMPD/covsec.$$" ]; then
            cov_reasons="$cov_reasons$(cat "$TMPD/covsec.$$")"
        fi
        rm -f "$TMPD/covsec.$$"
    done <"$TMPD/cov.tsv"
fi
if [ -z "$cov_reasons" ]; then
    pass 5 "coverage"
else
    fail 5 "coverage:$cov_reasons"
fi

# --- check 6: condition-column newline requirement gone ---
# (a) lines containing 条件列 AND 改行 == 0
# (b) lines containing パーサは改行を見ない verbatim >= 1
a=$(awk 'index($0, "条件列") && index($0, "改行") { n++ } END { print n+0 }' "$SPEC")
b=$(awk 'index($0, "パーサは改行を見ない") { n++ } END { print n+0 }' "$SPEC")
if [ "$a" -eq 0 ] && [ "$b" -ge 1 ]; then
    pass 6 "condition-column"
else
    fail 6 "cooccur-条件列-改行=$a verbatim-パーサは改行を見ない=$b"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
