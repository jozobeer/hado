#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo 'usage: run-gates.sh [--tier quick|core|full] <output-directory> [verify-<name> ...]' >&2
    echo '       run-gates.sh --print-tier quick|core|full' >&2
}

tier=
print_tier=
output=
selectors=()
while [ "$#" -gt 0 ]; do
    case "$1" in
        --tier)
            [ "$#" -ge 2 ] || { usage; exit 1; }
            tier=$2
            shift 2
            ;;
        --print-tier)
            [ "$#" -ge 2 ] || { usage; exit 1; }
            print_tier=$2
            shift 2
            ;;
        --*)
            echo "unknown option: $1" >&2
            exit 1
            ;;
        *)
            if [ -z "$output" ]; then
                output=$1
            else
                selectors+=("$1")
            fi
            shift
            ;;
    esac
done

# --print-tier は層の一覧を出すだけなので、出力ディレクトリも走行スコープも作らない。
if [ -n "$print_tier" ]; then
    case "$print_tier" in
        quick | core | full) ;;
        *)
            echo "unknown tier: $print_tier" >&2
            exit 1
            ;;
    esac
    cd "$(dirname "$0")/.."
    if [ ! -f scripts/lib/gate-tiers.sh ]; then
        echo 'tier list is not available in this tree' >&2
        exit 1
    fi
    # shellcheck source=scripts/lib/gate-tiers.sh
    . scripts/lib/gate-tiers.sh
    hado_tier_gates "$print_tier"
    exit 0
fi

if [ -z "$output" ]; then
    usage
    exit 1
fi

# 層の名前は出力ディレクトリを作る前に検める。
if [ -n "$tier" ]; then
    case "$tier" in
        quick | core | full) ;;
        *)
            echo "unknown tier: $tier" >&2
            exit 1
            ;;
    esac
fi

# 出力ディレクトリは呼び出し側の作業ディレクトリ基準で解決する（元の挙動）。
mkdir -p -- "$output"
output=$(cd -- "$output" && pwd)

cd "$(dirname "$0")/.."

# 門番の層別（docs/plan.md 33 節）。門番名の一覧は scripts/lib/gate-tiers.sh が
# 唯一の正。lib の無い木（判定器が作る合成木）では層を使わない。
if [ -f scripts/lib/gate-tiers.sh ]; then
    # shellcheck source=scripts/lib/gate-tiers.sh
    . scripts/lib/gate-tiers.sh
fi
if [ -n "$tier" ] && ! declare -F hado_tier_gates >/dev/null; then
    echo "tier list is not available in this tree: $tier" >&2
    exit 1
fi

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

status=0

# 層の門番に、コマンド行で明示した門番を足す（重複は落とす）。--tier を付けても
# 変更に関係する門番を足して回せるようにする（docs/plan.md 33 節）。
# 層も明示も無い引数なしの全走は full と同じ扱いにする（文書層も回す）。
default_all=0
if [ -z "$tier" ] && [ "${#selectors[@]}" -eq 0 ]; then
    default_all=1
fi
all=()
if [ -n "$tier" ]; then
    mapfile -t all < <(hado_tier_gates "$tier")
fi
all+=("${selectors[@]}")
if [ "${#all[@]}" -eq 0 ]; then
    all=(scripts/verify-*.sh)
fi
gates=()
for gate in "${all[@]}"; do
    name=${gate##*/}
    name=${name%.sh}
    # 層の一覧は素の名前（m0, r9, ...）で持つ。門番名としても verify- 付きでも受ける。
    if [[ "$name" != verify-* ]]; then
        name="verify-$name"
    fi
    if [ ! -f "scripts/$name.sh" ]; then
        echo "unknown gate: $gate" >&2
        exit 1
    fi
    skip=0
    for existing in "${gates[@]}"; do
        if [ "$existing" = "$name" ]; then
            skip=1
            break
        fi
    done
    if [ "$skip" -eq 0 ]; then
        gates+=("$name")
    fi
done

# quick 層は「保存ごと」なので、門番の前にビルドを 1 回だけ実走する。
# ログは tools/gates/main.hd の集計に混ざるので、末尾を PASSED: n/N に揃える。
if [ "$tier" = "quick" ]; then
    echo "RUN: build -> $output/build.log"
    started=$SECONDS
    build_status=0
    if ! cargo build --offline >"$output/build.log" 2>&1; then
        build_status=1
        status=1
    fi
    printf 'PASSED: %s/1\n' "$((1 - build_status))" >>"$output/build.log"
    if [ -n "$gate_times" ]; then
        { printf '%s\t%s\t%s\n' "$((SECONDS - started))" "$build_status" build >>"$gate_times"; } 2>/dev/null || true
    fi
fi

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

# core の終わりに、full でだけ回す門番を見せる。層をまたいだ取り違えで赤を
# 見逃さないため（docs/plan.md 33 節）。
if [ "$tier" = "core" ]; then
    echo "full-only gates (not run in core):"
    comm -23 <(hado_tier_all_gates | LC_ALL=C sort) <(hado_tier_gates core | LC_ALL=C sort) | sed 's/^/  /'
fi

# 文書の整合は処理系の門番ではないので、full 層（と引数なしの全走）の最後に
# 文書層として回す（docs/plan.md 33 節）。verify-*.sh ではないので 38 本には数えない。
if { [ "$tier" = "full" ] || [ "$default_all" -eq 1 ]; } && [ -f scripts/lint-docs.sh ]; then
    echo "RUN: lint-docs -> $output/lint-docs.log"
    started=$SECONDS
    lint_status=0
    if ! bash scripts/lint-docs.sh >"$output/lint-docs.log" 2>&1; then
        lint_status=1
        status=1
    fi
    if [ -n "$gate_times" ]; then
        { printf '%s\t%s\t%s\n' "$((SECONDS - started))" "$lint_status" lint-docs >>"$gate_times"; } 2>/dev/null || true
    fi
fi

exit "$status"
