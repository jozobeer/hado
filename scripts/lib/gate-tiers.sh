#!/usr/bin/env bash
# 門番の層別（docs/plan.md 33 節）。層は「いつ回すか」だけを決め、判定は変えない。
#
# 各門番は最小の層に 1 回だけ属する。3 層の最小の層の集合は、実在の門番
# （scripts/verify-*.sh）を重複なく覆う。実行は累積で、
#   quick -> quick のみ
#   core  -> quick + core（保存ごとに回す軽い検査も含める）
#   full  -> quick + core + full（実在の門番すべて）
# とする。門番名の一覧はここが唯一の正であり、docs/plan.md には一覧を書かない。
#
# 層を足す・門番を足すときは、ここだけを直す。scripts/verify-gate-cache.sh の
# 自己判定が「一覧 == 実在の門番」「重複なく覆う」「run-gates が同じ一覧を解決する」
# を機械で照合するので、実在の門番とのずれは門番の赤になる。

hado_tier_minimal_gates() {
    case "$1" in
        quick)
            # 保存ごと。ビルド（run-gates が先に 1 回だけ実走する）と仕様表の契約。
            printf '%s\n' gate-cache m1-spec
            ;;
        core)
            # コミット前。マイルストーンの完了条件と仕上げの判定。
            printf '%s\n' m0 m1-corpus m2 m3 m4 m5 m6 m7 r38
            ;;
        full)
            # 節目・夜間。ラウンドごとの生成探針と棚卸しの対象。
            printf '%s\n' record-constructors \
                r9 r11 r12 r13 r14 r16 r17 r18 r19 r20 r21 r22 \
                r23 r24 r25 r26 r28 r29 r30 r31 r32 r33 r34 r35 r36 r37 r42 r43 r44 r45 r46 r47 r48 r49 r50 r51 r52 r53 r54 r55 r56 r57 r58 r59 r60 r61 r62
            ;;
        *)
            printf 'unknown tier: %s\n' "$1" >&2
            return 1
            ;;
    esac
}

# その層を回すときに実走する門番（累積）。
hado_tier_gates() {
    case "$1" in
        quick) hado_tier_minimal_gates quick ;;
        core)
            hado_tier_minimal_gates quick
            hado_tier_minimal_gates core
            ;;
        full)
            hado_tier_minimal_gates quick
            hado_tier_minimal_gates core
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
    hado_tier_minimal_gates quick
    hado_tier_minimal_gates core
    hado_tier_minimal_gates full
}

# 並列（--jobs>1）の前段で直列に完走させる門番（docs/plan.md 34 節）。
# キャッシュの鍵は作業ツリー全体のハッシュなので、共有ツリーを書き換える門番を
# ファンアウトで走らせると他の門番の結果が揺れる。ここに挙げた門番は
#   gate-cache      : リポジトリ直下に probe を置いて消す（内容ハッシュを揺らす）
#   m0              : ビルドと全走テスト。full-test の証拠を確定させる barrier
#   m1-corpus/m2/m3 : tests/accept・tests/reject に probe を置き、入れ替える
# ため直列にする。run-gates はこの一覧 ∩ 走行する門番を前段に置く。
# ツリーを書く門番を足したら、ここにも足す。verify-gate-cache.sh が一覧が
# 実在の門番の部分集合であることを機械で見る。
hado_tier_serial_gates() {
    printf '%s\n' gate-cache m0 m1-corpus m2 m3
}

hado_tiers() {
    printf '%s\n' quick core full
}
