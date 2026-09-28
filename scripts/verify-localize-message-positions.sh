#!/usr/bin/env bash
# localize-message-positions: 診断本文が指す位置をファイルの座標で測る。入口以外の
# ファイルに置いた E501・E502・E507・E508・E406・E403・E405 の本文が、同じ欠陥を
# 単独ファイルとして check した出力と一致し、入口の行数を 1 行と 8 行に変えても
# 動かないことを見る。期待値は単独ファイルの出力から計算し、ケース名と行番号は
# 埋め込まない。位置の localize と穴埋めを外す変異が赤になることも見る。公開 API
# `hado::dump_mono`・`hado::check_source` の本文に穴が残らないことも見る。
#
# tier: core
# round: localize-message-positions
set -uo pipefail
cd "$(dirname "$0")/.."

checks=()
statuses=()

record() {
    checks+=("$1")
    statuses+=("$2")
    printf '%s: %s\n' "$1" "$2"
}

# ---------- J1: 本文の位置（issue の完了条件 1・2） ----------
printf '== J1 ==\n'
if out=$(cargo test --offline --test localize_message_positions 2>&1); then
    summary=$(grep -m1 'test result:' <<<"$out")
    record "J1:診断位置(${summary#test result: })" PASS
else
    printf '%s\n' "$out" | tail -25
    record "J1:診断位置" FAIL
fi

# ---------- J2: localize と穴埋めを外す変異 ----------
printf '== J2 ==\n'
mutated=(crates/hado/src/project.rs crates/hado/src/lib.rs)
backup_dir=$(mktemp -d)
for file in "${mutated[@]}"; do
    cp "$file" "$backup_dir/$(basename "$file")"
done
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
        printf 'J2 %s: anchor error\n' "$label"
        mutations=$((mutations + 1))
        restore
        return
    fi
    mutations=$((mutations + 1))
    if cargo test --offline --test localize_message_positions >/dev/null 2>&1; then
        printf 'J2 %s: mutation stayed green\n' "$label"
    else
        printf 'J2 %s: caught\n' "$label"
        caught=$((caught + 1))
    fi
    restore
}

# 本文が指す位置をファイルの座標へ直さない（連結後の行のまま）。
mutate "skip-mention-localize" crates/hado/src/project.rs \
    '            localize_line(files, &mut mention.0);' '            let _ = mention;'
# 入口以外のファイルの穴を埋めない（本文に {0} が残る）。
mutate "skip-mention-fill" crates/hado/src/project.rs \
    '    error.fill_mentions();' '    let _ = &mut error;'
# 単独ファイルの穴を埋めない（公開 API の本文に {0} が残る）。
mutate "skip-single-source-fill" crates/hado/src/lib.rs \
    '    error.fill_mentions();' '    let _ = &mut error;'

cleanup
trap - EXIT
if ! cargo build --offline -q 2>/dev/null; then
    record "J2:復旧のビルド" FAIL
elif [ "$mutations" -gt 0 ] && [ "$caught" -eq "$mutations" ]; then
    record "J2:変異(cases=$mutations)" PASS
else
    record "J2:変異(cases=$mutations,caught=$caught)" ISSUES
fi

# ---------- J3: 文書の形 ----------
printf '== J3 ==\n'
if out=$(bash scripts/lint-docs.sh 2>&1); then
    record "J3:lint-docs($(tail -1 <<<"$out"))" PASS
else
    printf '%s\n' "$out" | tail -10
    record "J3:lint-docs" FAIL
fi

# ---------- 集計 ----------
passed=0
for status in "${statuses[@]}"; do
    [ "$status" = PASS ] && passed=$((passed + 1))
done
printf 'PASSED: %d/%d\n' "$passed" "${#checks[@]}"
[ "$passed" -eq "${#checks[@]}" ]
