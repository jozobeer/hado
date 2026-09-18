#!/usr/bin/env bash
# CI の入口（docs/rounds/ci.md）。GitHub Actions から event と ref を受け取り、
# 回す層を選んで門番と文書層を回す。層の中身と門番の一覧は
# scripts/lib/gate-tiers.sh が唯一の正で、この script は層の名前だけを決める。
#
#   bash scripts/ci-run.sh <event> <ref> [requested-tier]
#
# pull_request は core 以上、push は refs/heads/main だけを full、
# workflow_dispatch は requested-tier（空なら full）とする。知らない event・
# 知らない ref・知らない層は回さずに落ちる。
#
# 文書層（scripts/lint-docs.sh）は run-gates の層に含まれない（full の最後に
# run-gates が回すだけ）ので、どの層でもここで必ず回す。full では 2 回になるが、
# 判定は同じで所要は数秒である。
set -uo pipefail
cd "$(dirname "$0")/.."

usage() {
    echo 'usage: ci-run.sh <event> <ref> [requested-tier]' >&2
}

if [ "$#" -lt 2 ]; then
    usage
    exit 1
fi
event=$1
ref=$2
requested=${3:-}

# shellcheck source=scripts/lib/gate-tiers.sh
. scripts/lib/gate-tiers.sh

tier=
case "$event" in
    pull_request)
        # PR は core 以上。quick だけでは m0 の全走テストとコーパスが落ちる。
        tier=core
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

echo "ci-run: event=$event ref=$ref tier=$tier jobs=$jobs out=$out"
status=0
if ! bash scripts/run-gates.sh --jobs "$jobs" --tier "$tier" "$out"; then
    status=1
fi
# 門番が赤でも文書層は回す（赤を隠さない。判定は status に畳む）。
if ! bash scripts/lint-docs.sh; then
    status=1
fi
exit "$status"
