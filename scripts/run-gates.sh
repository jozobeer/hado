#!/usr/bin/env bash
set -euo pipefail
if [ "$#" -eq 0 ]; then
    echo 'usage: run-gates.sh <output-directory> [verify-<name> ...]' >&2
    exit 1
fi
mkdir -p -- "$1"
output=$(cd -- "$1" && pwd)
shift
cd "$(dirname "$0")/.."

# 門番ごとの所要の記録先（任意）。最外の run-gates だけが記録し、入れ子の
# run-gates（r38 の合成木や部分走）には継がせない。
if [ -n "${HADO_GATE_RUN_ACTIVE:-}" ]; then
    gate_times=
else
    gate_times=${HADO_GATE_TIMES:-}
fi
HADO_GATE_RUN_ACTIVE=1
export HADO_GATE_RUN_ACTIVE
unset HADO_GATE_TIMES

# 全走 1 回のあいだ、同じ内容に対する門番と cargo test の実走を 1 度に畳む
# （docs/plan.md 32 節）。lib の無い木（判定器が作る合成木）では素通しする。
gate_run_dir=""
if [ -f scripts/lib/gate-cache.sh ]; then
    # 入れ子の run-gates が外側の走行スコープを消さないよう、自分の走行を新しく始める。
    unset HADO_GATE_CACHE_RUN
    # shellcheck source=scripts/lib/gate-cache.sh
    . scripts/lib/gate-cache.sh
    gate_run_dir=$(hado_gate_cache_run_dir)
    trap 'rm -rf -- "$gate_run_dir"' EXIT
else
    gate_cached() {
        local label=$1
        shift
        "$@"
    }
fi

if [ "$#" -eq 0 ]; then
    set -- scripts/verify-*.sh
fi
gates=()
for gate in "$@"; do
    name=${gate##*/}
    name=${name%.sh}
    if [[ "$name" != verify-* ]] || [ ! -f "scripts/$name.sh" ]; then
        echo "unknown gate: $gate" >&2
        exit 1
    fi
    gates+=("$name")
done
status=0
for name in "${gates[@]}"; do
    echo "RUN: $name -> $output/$name.log"
    started=$SECONDS
    gate_status=0
    if ! gate_cached "verify-$name" bash "scripts/$name.sh" >"$output/$name.log" 2>&1; then
        gate_status=1
        status=1
    fi
    # HADO_GATE_TIMES を設定すると、門番ごとの所要と終了コードを追記する（任意）。
    if [ -n "$gate_times" ]; then
        { printf '%s\t%s\t%s\n' "$((SECONDS - started))" "$gate_status" "$name" >>"$gate_times"; } 2>/dev/null || true
    fi
done
exit "$status"
