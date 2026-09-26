#!/usr/bin/env bash
# scripts/gate-tiers.lock を scripts/lib/gate-tiers.sh の導出から作り直す。
#
# 層の所属は各 scripts/verify-*.sh の `# tier:` 行が唯一の正で、
# scripts/lib/gate-tiers.sh がそれを走査して導く。この道具は導出の結果を
# `<門番名>\t<層>` の行として記録し、`--check` がバイト一致を見る。
# 門番の層を書き換えると導出が変わり、照合が赤になる（層を後ろへ動かすと
# 前の層から門番が消えるのに、層の所属を誰も見ていなかった）。
#
#   bash scripts/gate-tiers-lock.sh           作り直す（行数を出す）
#   bash scripts/gate-tiers-lock.sh --check   照合する（食い違えば差分を出して非 0）
#
# 形は scripts/corpus-lock.sh と同じ（引数なしで作り直し、--check で照合、
# 作り直しの最後に行数を出す）。門番名の綴りは `run-gates.sh --print-tier` と
# 同じく `verify-` と `.sh` を落とした名である。
set -euo pipefail
cd "$(dirname "$0")/.."

# 導出は scripts/lib/gate-tiers.sh の 1 か所だけ。門番の一覧はここに書かない。
# shellcheck source=scripts/lib/gate-tiers.sh
. scripts/lib/gate-tiers.sh

gate_tiers_bytes() {
    local tier names name
    for tier in $(hado_tiers); do
        # 層の走査が落ちる（`# tier:` の欠落・不正）なら、門番を黙って落とさずに
        # ここで止める。代入の終了コードは command substitution のそれである。
        names=$(hado_tier_minimal_gates "$tier") || return 1
        while IFS= read -r name; do
            [ -n "$name" ] || continue
            printf '%s\t%s\n' "$name" "$tier"
        done <<<"$names"
    done | LC_ALL=C sort
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
gate_tiers_bytes >"$generated"

if [ "$check" -eq 1 ]; then
    diff -u scripts/gate-tiers.lock "$generated"
    exit 0
fi

cp "$generated" scripts/gate-tiers.lock
grep -c '^' scripts/gate-tiers.lock
