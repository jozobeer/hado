#!/usr/bin/env bash
# CI の入口（docs/rounds/ci.md）。GitHub Actions から event と ref を受け取り、
# 回す層を選んで門番と文書層を回す。層の中身と門番の一覧は
# scripts/lib/gate-tiers.sh が唯一の正で、この script は層の名前だけを決める。
#
#   bash scripts/ci-run.sh <event> <ref> [requested-tier] [shard]
#
# pull_request は core 以上、push は refs/heads/main だけを full、
# workflow_dispatch は requested-tier（空なら full）とする。知らない event・
# 知らない ref・知らない層は回さずに落ちる。
#
# ただし pull_request の差分が docs_only_path の文書だけなら、門番を回さず文書層
# だけを回す（docs/rounds/ci-docs-only.md）。差分は checkout した merge commit の
# 1 番目の親（取り込み先の先端）との比較で取り、取れなければ core に倒す。
#
# 文書層（scripts/lint-docs.sh）は run-gates の層に含まれない（full の最後に
# run-gates が回すだけ）ので、どの層でもここで必ず回す。full では 2 回になるが、
# 判定は同じで所要は数秒である。
#
# shard（tests:I/N か gates:I/N）を渡すと、その取り分だけを回す（scripts/run-gates.sh の
# --shard、docs/rounds/ci-shards.md）。ワークフローは tests と gates の各 I = 1..N を別々の
# job で同時に回す。文書層は gates:1 だけが回す。
set -uo pipefail
cd "$(dirname "$0")/.."

usage() {
    echo 'usage: ci-run.sh <event> <ref> [requested-tier] [shard]' >&2
}

if [ "$#" -lt 2 ]; then
    usage
    exit 1
fi
event=$1
ref=$2
requested=${3:-}
shard=${4:-}
shard_args=()
shard_first=1
if [ -n "$shard" ]; then
    # 形は run-gates が検める。文書だけの PR では run-gates を呼ばないので、ここでも落とす。
    if ! bash scripts/run-gates.sh --shard "$shard" --print-tier quick >/dev/null; then
        exit 1
    fi
    shard_args=(--shard "$shard")
    [ "${shard%%/*}" = gates:1 ] || shard_first=0
fi

# shellcheck source=scripts/lib/gate-tiers.sh
. scripts/lib/gate-tiers.sh

# core 以上の門番が読まず、文書層（scripts/lint-docs.sh）だけが読む文書。
docs_only_path() {
    case "$1" in
        README.md | CONTRIBUTING.md | docs/rounds/*.md | docs/issues/*.md | docs/plans/*.md) return 0 ;;
    esac
    return 1
}

# PR に core が要る理由を出す。差分がすべて docs_only_path なら何も出さずに偽。
# merge commit でない・差分が取れない・空なら core とする。リネームは
# --no-renames で削除と追加に分け、元のパスも見る。
pr_core_reason() {
    local changed path
    if [ -z "$(git rev-parse -q --verify 'HEAD^2' 2>/dev/null)" ] ||
        ! changed=$(git diff --no-renames --name-only 'HEAD^1' HEAD 2>/dev/null); then
        echo 'the merge commit and its first parent are unavailable'
        return 0
    fi
    if [ -z "$changed" ]; then
        echo 'the diff is empty'
        return 0
    fi
    while IFS= read -r path; do
        if ! docs_only_path "$path"; then
            echo "$path is not a document-only path"
            return 0
        fi
    done <<<"$changed"
    return 1
}

tier=
docs_only=0
case "$event" in
    pull_request)
        # PR は core 以上。quick だけでは m0 の全走テストとコーパスが落ちる。
        tier=core
        if reason=$(pr_core_reason); then
            echo "ci-run: pull_request runs core: $reason"
        else
            docs_only=1
        fi
        ;;
    push)
        if [ "$ref" != "refs/heads/main" ]; then
            echo "ci-run: push is wired only for refs/heads/main: $ref" >&2
            exit 1
        fi
        tier=full
        ;;
    workflow_dispatch)
        if [ -n "$requested" ]; then
            tier=$requested
        else
            tier=full
        fi
        ;;
    *)
        echo "ci-run: unknown event: $event" >&2
        exit 1
        ;;
esac

# 層が実在することを run-gates と同じ一覧で確かめる（fail closed）。
known=0
while IFS= read -r name; do
    if [ "$name" = "$tier" ]; then
        known=1
    fi
done < <(hado_tiers)
if [ "$known" -ne 1 ]; then
    echo "ci-run: unknown tier: $tier" >&2
    exit 1
fi

# 門番の並列度。runner の CPU 数をそのまま使う（run-gates の既定は 1 = 直列）。
jobs=${HADO_CI_JOBS:-}
if [ -z "$jobs" ]; then
    jobs=$(nproc 2>/dev/null || echo 1)
fi
out=${HADO_CI_OUT:-target/ci-gates}

if [ "$docs_only" -eq 1 ]; then
    echo "ci-run: event=$event ref=$ref docs-only: the gates are skipped, the document layer runs${shard:+ (shard $shard)}"
else
    echo "ci-run: event=$event ref=$ref tier=$tier jobs=$jobs out=$out${shard:+ shard=$shard}"
fi
status=0
if [ "$docs_only" -eq 0 ] && ! bash scripts/run-gates.sh --jobs "$jobs" --tier "$tier" "${shard_args[@]}" "$out"; then
    status=1
fi
# 門番が赤でも文書層は回す（赤を隠さない。判定は status に畳む）。
if [ "$shard_first" -eq 1 ] && ! bash scripts/lint-docs.sh; then
    status=1
fi
exit "$status"
