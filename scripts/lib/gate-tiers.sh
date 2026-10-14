#!/usr/bin/env bash
# 門番の層別（docs/plan.md 33 節）。層は「いつ回すか」だけを決め、判定は変えない。
#
# 各門番は最小の層に 1 回だけ属する。所属は各 scripts/verify-*.sh の
# `# tier: quick|core|full` 行が宣言し、このファイルはそれを走査して層ごとの
# 集合を導く。3 層の最小の層の集合は、実在の門番（scripts/verify-*.sh）を
# 重複なく覆う。実行は累積で、
#   quick -> quick のみ
#   core  -> quick + core（保存ごとに回す軽い検査も含める）
#   full  -> quick + core + full（実在の門番すべて）
# とする。docs/plan.md には一覧を書かない。
#
# 層を足す・門番を足すときは、門番自身の `# tier:` 行を直す。`# tier:` 行が無いか
# 2 つ以上ある門番は、走査が理由を出して失敗する。
#
# 走査の基準は source した時点のこのファイルの位置であり、呼び出し時の cwd には
# 依存しない。

_hado_verify_scripts_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

hado_tier_minimal_gates() {
    case "$1" in
        quick | core | full) ;;
        *)
            printf 'unknown tier: %s\n' "$1" >&2
            return 1
            ;;
    esac
    local f name count value invalid=0
    local names=()
    for f in "$_hado_verify_scripts_dir"/verify-*.sh; do
        [ -f "$f" ] || continue
        count=$(grep -c '^# tier: ' "$f" || true)
        if [ "$count" -ne 1 ]; then
            printf 'invalid tier header: %s\n' "${f##*/}" >&2
            invalid=1
            continue
        fi
        value=$(grep '^# tier: ' "$f")
        value=${value#\# tier: }
        case "$value" in
            quick | core | full) ;;
            *)
                printf 'invalid tier header: %s\n' "${f##*/}" >&2
                invalid=1
                continue
                ;;
        esac
        if [ "$value" = "$1" ]; then
            name=${f##*/}
            name=${name%.sh}
            names+=("${name#verify-}")
        fi
    done
    [ "$invalid" -eq 0 ] || return 1
    if [ "${#names[@]}" -eq 0 ]; then
        return 0
    fi
    printf '%s\n' "${names[@]}" | LC_ALL=C sort
}

# その層を回すときに実走する門番（累積）。
hado_tier_gates() {
    case "$1" in
        quick) hado_tier_minimal_gates quick ;;
        core)
            hado_tier_minimal_gates quick || return
            hado_tier_minimal_gates core
            ;;
        full)
            hado_tier_minimal_gates quick || return
            hado_tier_minimal_gates core || return
            hado_tier_minimal_gates full
            ;;
        *)
            printf 'unknown tier: %s\n' "$1" >&2
            return 1
            ;;
    esac
}

# 最小の層の割り当て（重複なく全門番を覆うことを機械で見るための一覧）。
hado_tier_all_gates() {
    hado_tier_minimal_gates quick || return
    hado_tier_minimal_gates core || return
    hado_tier_minimal_gates full
}

hado_tiers() {
    printf '%s\n' quick core full
}
