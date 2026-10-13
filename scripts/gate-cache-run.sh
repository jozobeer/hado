#!/usr/bin/env bash
# python で書かれた判定器からキャッシュ付きの実走を呼ぶための薄い入口。
# 使い方:
#   gate-cache-run.sh <label> <command> [args...]
#   gate-cache-run.sh --cargo <cargo の引数...>
#   gate-cache-run.sh --nextest-full
set -uo pipefail
cd "$(dirname "$0")/.."
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh
if [ "${1:-}" = "--cargo" ]; then
    shift
    cargo_cached "$@"
    exit $?
fi
if [ "${1:-}" = "--nextest-full" ]; then
    nextest_full_cached
    exit $?
fi
label=$1
shift
gate_cached "$label" "$@"
exit $?
