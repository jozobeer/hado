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
# 層を足す・門番を足すときは、門番自身の `# tier:` 行を直す。
# scripts/verify-gate-cache.sh の自己判定が「各門番が有効な `# tier:` 行を
# ちょうど 1 つ持つ」「走査結果 == 実在の門番」「重複なく覆う」
# 「run-gates が同じ一覧を解決する」を機械で照合するので、実在の門番との
# ずれは門番の赤になる。
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

# 並列（--jobs>1）の前段で直列に完走させる門番（docs/plan.md 34 節）。
# キャッシュの鍵は作業ツリー全体のハッシュなので、共有ツリーを書き換える門番を
# ファンアウトで走らせると他の門番の結果が揺れる。ここに挙げた門番は
#   gate-cache      : リポジトリ直下に probe を置いて消す（内容ハッシュを揺らす）
#   m0              : ビルドと全走テスト。full-test の証拠を確定させる barrier
#   m1-corpus/m2/m3 : tests/accept・tests/reject に probe を置き、入れ替える
#   nested-application-round-trip : postfix_print.rs へ変異を当ててビルドし直す
# ため直列にする。run-gates はこの一覧 ∩ 走行する門番を前段に置く。
# ツリーを書く門番を足したら、ここにも足す。verify-gate-cache.sh が一覧が
# 実在の門番の部分集合であることを機械で見る。
hado_tier_serial_gates() {
    printf '%s\n' gate-cache m0 m1-corpus m2 m3 nested-application-round-trip
}

hado_tiers() {
    printf '%s\n' quick core full
}
