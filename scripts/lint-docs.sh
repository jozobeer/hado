#!/usr/bin/env bash
# 文書層の検査（docs/plan.md 33 節）。docs/spec.md を「表として読む（処理系との
# 契約）」検査は門番に残し、「文章・見出し・対応表を逐語や個数で固定する（文書管理）」
# 検査だけをここへ移した。処理系を守らないので scripts/verify-*.sh には数えず、
# full 層の最後にだけ回す（scripts/run-gates.sh）。
#
# 移した元:
#   verify-m1-spec.sh の検査 1（見出し）・2（新設節）・4（対応表）・5（条件列の逐語）
#   verify-r38.sh の 38-c の逐語 regex・38-e の版文字列と ledger 節
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

# --- 検査 1: 既存の節番号（§1〜§42 が 1 回ずつ、見出しを消していない） ---
doc_headings() {
    local del missing n c
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
    [ "$del" -eq 0 ] && [ -z "$missing" ]
}

# --- 検査 2: 新設節 §43〜§59 が §42 より後にある ---
doc_new_sections() {
    local line42 ns_reasons n hits h ln
    line42=$(grep -nE '^# 42\.' "$SPEC" | head -n 1 | cut -d: -f1)
    line42=${line42:-0}
    ns_reasons=""
    n=43
    while [ "$n" -le 59 ]; do
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
    [ -z "$ns_reasons" ]
}

# --- 検査 3: 対応表（docs/spec-coverage.md）が計画の表 2.2/2.3/9.2/11.2/13.2/18.4/24.2 と一致 ---
doc_coverage() {
    local cov_reasons="" nrows nplan item citem found name secs ref num s
    if [ ! -f "$COV" ]; then
        return 1
    fi
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
    awk -F '|' '
        /^### 11\.2/ { p=1; next }
        /^### 11\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan112.txt"
    awk -F '|' '
        /^### 13\.2/ { p=1; next }
        /^### 13\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan132.txt"
    # Why not /^### 18\.4/: 19節に「18.4 の項目の割り当て」があり、同じ項目名が二重に入る。
    awk -F '|' '
        /^### 18\.4 仕様の項目/ { p=1; next }
        /^### 18\.5/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan184.txt"
    awk -F '|' '
        /^### 24\.2 仕様の項目/ { p=1; next }
        /^### 24\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan242.txt"
    cat "$TMPD/plan22.txt" "$TMPD/plan23.txt" "$TMPD/plan92.txt" "$TMPD/plan112.txt" "$TMPD/plan132.txt" "$TMPD/plan184.txt" "$TMPD/plan242.txt" >"$TMPD/plan_items.txt"
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

    [ -z "$cov_reasons" ]
}

# --- 検査 4: 条件列の逐語（新しい改行要求が入っていない） ---
doc_condition_column() {
    local a b
    a=$(awk 'index($0, "条件列") && index($0, "改行") { n++ } END { print n+0 }' "$SPEC")
    b=$(awk 'index($0, "パーサは改行を見ない") { n++ } END { print n+0 }' "$SPEC")
    [ "$a" -eq 0 ] && [ "$b" -ge 1 ]
}

# --- 検査 5: 版（spec と spec-coverage が同じ版を名乗る） ---
doc_version() {
    local sv cv
    sv=$(head -n 1 "$SPEC" | grep -oE 'v[0-9]+(\.[0-9]+)+' | head -n 1)
    cv=$(head -n 1 "$COV" | grep -oE 'v[0-9]+(\.[0-9]+)+' | head -n 1)
    [ -n "$sv" ] && [ "$sv" = "$cv" ]
}

# --- 検査 6: 最新ラウンドの台帳の節がある ---
doc_ledger_round() {
    local max
    max=$(for f in scripts/verify-r[0-9]*.sh; do
        f=${f##*/}
        f=${f%.sh}
        printf '%s\n' "${f#verify-r}"
    done | LC_ALL=C sort -n | tail -n 1)
    [ -n "$max" ] && grep -qE "^## R${max} " docs/ledger.md
}

# --- 検査 7: R38 が足した規則の文が spec に逐語で 1 回ずつある ---
# r38 の構造検査（probe の key と §番号の対応・見出しの実在）は verify-r38.sh に残る。
doc_r38_sentences() {
    python3 - <<'PY'
import re
from pathlib import Path
rules = {
    '38-c(1)': (45, r'^Array の `compare` は要素を先頭から比較して最初に異なる要素の順序を返し、共通部分がすべて等しいときだけ短い方を先とする。$'),
    '38-c(2)': (49, r'^`Int\.parse` の受理文法は、往復から導いた下限として `Int\.to_string` が返す全ての Int の10進表記を含み、'
                   r'本節が決めた上限として ASCII の `-\?\[0-9\]\+` に一致して値が -9223372036854775808 以上 9223372036854775807 以下である文字列だけを受理し、'
                   r'先行ゼロの個数は制限せず、`\+`・空白・`_`・小数点・全角数字は拒否する。$'),
    '38-c(3)': (49, r'^`read_dir` は指定したディレクトリの各項目の名前だけを返し、`\.` と `\.\.` を除いて UTF-8 のバイト順に整列する。$'),
    '38-c(4)': (49, r'^`read_stdin` は標準入力を EOF まで読み、同じプロセスでの2回目の呼び出しは空の `Bytes` を返す。$'),
    '38-c(5)': (49, r'^`env` の値が非 UTF-8 のときも、`message` が `"InvalidUtf8"` の `Error` を raise する。$'),
    '38-c(6)': (47, r'^既定実装の契約名 `EnvDefault` と `IoDefault` は契約の名前空間で予約し、利用者の同名の契約宣言は E205 とする。$'),
}
def sections(text):
    return dict(re.findall(r'^# (\d+)\.[^\n]*\n(.*?)(?=^# \d+\.|\Z)', text, re.M | re.S))
def present(text):
    found = sections(text)
    for key, (section, pattern) in rules.items():
        assert str(section) in found, f'{key}: missing section {section}'
        assert len(re.findall(pattern, found[str(section)], re.M)) == 1, key
spec = Path('docs/spec.md').read_text()
present(spec)
for key, (_, pattern) in rules.items():
    try:
        present(re.sub(pattern, '', spec, flags=re.M))
    except AssertionError:
        pass
    else:
        raise AssertionError(f'{key}: missing sentence accepted')
print('coverage: ' + ' '.join(f'§{section}={sum(1 for s, _ in rules.values() if s == section)}' for section in (45, 47, 49)))
PY
}

check 'spec headings §1..§42 exist once and none deleted' doc_headings
check 'new sections §43..§59 follow §42' doc_new_sections
check 'spec-coverage matches the plan tables 2.2/2.3/9.2/11.2/13.2/18.4/24.2' doc_coverage
check 'condition column prose does not require a newline' doc_condition_column
check 'spec and spec-coverage name the same version' doc_version
check 'the latest round has a ledger section' doc_ledger_round
check 'R38 spec sentences are pinned verbatim once each' doc_r38_sentences

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
