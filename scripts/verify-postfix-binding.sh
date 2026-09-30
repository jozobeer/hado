#!/usr/bin/env bash
# postfix-binding: 後置の束縛（`e as p ->`・`e ->`）と `if`・`fn`・`module`・`implement`
# の閉じを測る。受理コーパス・stdlib・tools が 1 通りの正準形で往復し、閉じの
# 不足・余分・食い違いと旧形が E100 になること、印字器を壊すと赤になることを見る。
#
# tier: full
# round: postfix-binding
set -uo pipefail
cd "$(dirname "$0")/.."

checks=()
statuses=()

record() {
    checks+=("$1")
    statuses+=("$2")
    printf '%s: %s\n' "$1" "$2"
}

# ---------- J1: 往復と閉じの正例・負例 ----------
printf '== J1 ==\n'
if out=$(cargo test --offline --test postfix_close 2>&1); then
    summary=$(grep -m1 'test result:' <<<"$out")
    record "J1:往復と閉じ(${summary#test result: })" PASS
else
    printf '%s\n' "$out" | tail -25
    record "J1:往復と閉じ" FAIL
fi

# ---------- J2: 受理の実行と拒否のコード ----------
printf '== J2 ==\n'
if out=$(cargo test --offline --test corpus 2>&1); then
    summary=$(grep -m1 'test result:' <<<"$out")
    record "J2:コーパス(${summary#test result: })" PASS
else
    printf '%s\n' "$out" | tail -25
    record "J2:コーパス" FAIL
fi

# ---------- J3: 印字器の変異 ----------
printf '== J3 ==\n'
mutated=(crates/hado/src/postfix_print.rs crates/hado/src/parser.rs)
backup_dir=$(mktemp -d)
for file in "${mutated[@]}"; do
    cp "$file" "$backup_dir/$(basename "$file")"
done
# 変異のたびに元へ戻す。控えは消さない（最後の後片付けだけが消す）。
restore() {
    local file
    for file in "${mutated[@]}"; do
        cp "$backup_dir/$(basename "$file")" "$file"
    done
}
cleanup() {
    restore
    rm -rf "$backup_dir"
}
trap cleanup EXIT

mutations=0
caught=0
mutate() {
    local label=$1 file=$2 old=$3 new=$4
    restore
    if ! MUT_OLD="$old" MUT_NEW="$new" python3 - "$file" <<'PY'
import os, sys
path = sys.argv[1]
old, new = os.environ['MUT_OLD'], os.environ['MUT_NEW']
text = open(path, encoding='utf-8').read()
if text.count(old) != 1:
    sys.exit('anchor is not unique')
open(path, 'w', encoding='utf-8').write(text.replace(old, new))
PY
    then
        printf 'J3 %s: anchor error\n' "$label"
        mutations=$((mutations + 1))
        restore
        return
    fi
    mutations=$((mutations + 1))
    if cargo build --offline -q 2>/dev/null && cargo test --offline --test postfix_close >/dev/null 2>&1; then
        printf 'J3 %s: mutation stayed green\n' "$label"
    else
        printf 'J3 %s: caught\n' "$label"
        caught=$((caught + 1))
    fi
    restore
}

# `/if` を落とす。
mutate "drop-rail-if" crates/hado/src/postfix_print.rs 'format!("if {inner}\n{pad}/if")' 'format!("if {inner}")'
# Option 束縛の `?` を落とす。
mutate "drop-option-question" crates/hado/src/postfix_print.rs 'binding(format!(" as {pat}?"), value, body, indent, tail)' 'binding(format!(" as {pat}"), value, body, indent, tail)'
# 続きのある `if` を括弧で閉じない（`if(a => 1 | 2) + 1` を `if a => 1 | 2 + 1` と書く）。
mutate "drop-if-parens" crates/hado/src/postfix_print.rs 'format!("if({inner})")' 'format!("if {inner}")'
# 続きの無い `if` にも括弧を付ける（末尾の `if a => 1 | 2` を `if(...)` と書く）。
mutate "add-if-parens" crates/hado/src/postfix_print.rs '        format!("if {inner}")' '        format!("if({inner})")'
# `if` の直後の `(` を無条件に閉じと読む（`if((a+b)*2>c => 1 | 0)` が読めなくなる）。
mutate "always-close-if-paren" crates/hado/src/parser.rs 'TokKind::Arrow if depth == 0 => return true,' 'TokKind::Arrow => return true,'

cleanup
trap - EXIT
if ! cargo build --offline -q 2>/dev/null; then
    record "J3:復旧のビルド" FAIL
elif [ "$mutations" -gt 0 ] && [ "$caught" -eq "$mutations" ]; then
    record "J3:印字器の変異(cases=$mutations)" PASS
else
    record "J3:印字器の変異(cases=$mutations,caught=$caught)" ISSUES
fi

# ---------- J4: 文書の形 ----------
printf '== J4 ==\n'
if out=$(bash scripts/lint-docs.sh 2>&1); then
    record "J4:lint-docs($(tail -1 <<<"$out"))" PASS
else
    printf '%s\n' "$out" | tail -10
    record "J4:lint-docs" FAIL
fi

# ---------- 集計 ----------
passed=0
for status in "${statuses[@]}"; do
    [ "$status" = PASS ] && passed=$((passed + 1))
done
printf 'PASSED: %d/%d\n' "$passed" "${#checks[@]}"
[ "$passed" -eq "${#checks[@]}" ]
