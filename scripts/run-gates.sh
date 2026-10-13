#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo 'usage: run-gates.sh [--jobs N] [--tier quick|core|full] <output-directory> [verify-<name> ...]' >&2
    echo '       run-gates.sh --print-tier quick|core|full' >&2
}

tier=
print_tier=
output=
gate_jobs=1
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
        --jobs)
            [ "$#" -ge 2 ] || { usage; exit 1; }
            gate_jobs=$2
            shift 2
            ;;
        --jobs=*)
            gate_jobs=${1#--jobs=}
            shift
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

# --jobs は正の整数（先頭 0 なし）だけ。層の一覧より先に検める。
case "$gate_jobs" in
    '' | *[!0-9]* | 0*)
        echo "invalid --jobs: $gate_jobs" >&2
        usage
        exit 1
        ;;
esac

for name in "$tier" "$print_tier"; do
    case "$name" in
        '' | quick | core | full) ;;
        *)
            echo "unknown tier: $name" >&2
            exit 1
            ;;
    esac
done

# 門番の層別（docs/plan.md 33 節）。門番名の一覧は scripts/lib/gate-tiers.sh が唯一の正。
here=$(cd "$(dirname "$0")/.." && pwd)
# shellcheck source=scripts/lib/gate-tiers.sh
. "$here/scripts/lib/gate-tiers.sh"

# --print-tier は層の一覧を出すだけなので、出力ディレクトリを作らない。
if [ -n "$print_tier" ]; then
    hado_tier_gates "$print_tier"
    exit 0
fi

if [ -z "$output" ]; then
    usage
    exit 1
fi

# 出力ディレクトリは呼び出し側の作業ディレクトリ基準で解決する。
mkdir -p -- "$output"
output=$(cd -- "$output" && pwd)
cd "$here"

# 門番ごとの所要の記録先（任意）。
gate_times=${HADO_GATE_TIMES:-}
unset HADO_GATE_TIMES
record_time() { # seconds status name
    if [ -n "$gate_times" ]; then
        { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$gate_times"; } 2>/dev/null || true
    fi
}

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
    tier_list=$(hado_tier_gates "$tier") || {
        echo "tier scan failed: $tier" >&2
        exit 1
    }
    if [ -n "$tier_list" ]; then
        mapfile -t all <<<"$tier_list"
    fi
fi
all+=("${selectors[@]}")
if [ "${#all[@]}" -eq 0 ]; then
    all=(scripts/verify-*.sh)
fi
gates=()
for gate in "${all[@]}"; do
    name=${gate##*/}
    name=${name%.sh}
    # 層の一覧は素の名前（m0, r29, ...）で持つ。門番名としても verify- 付きでも受ける。
    if [[ "$name" != verify-* ]]; then
        name="verify-$name"
    fi
    if [ ! -f "scripts/$name.sh" ]; then
        echo "unknown gate: $gate" >&2
        exit 1
    fi
    case " ${gates[*]} " in
        *" $name "*) ;;
        *) gates+=("$name") ;;
    esac
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
    record_time "$((SECONDS - started))" "$build_status" build
fi

# 門番 1 本を走らせ、終了コードと所要を作業ディレクトリに残す。ログは $output/<名>.log。
gate_scratch=$(mktemp -d)
gate_worker_pids=()
hado_kill_tree() {
    local pid=$1 child
    if command -v pgrep >/dev/null 2>&1; then
        for child in $(pgrep -P "$pid" 2>/dev/null || true); do
            hado_kill_tree "$child"
        done
    fi
    kill "$pid" 2>/dev/null || true
}
hado_cleanup() {
    local worker
    for worker in "${gate_worker_pids[@]}"; do
        hado_kill_tree "$worker"
    done
    rm -rf -- "$gate_scratch"
}
trap hado_cleanup EXIT

hado_run_gate() {
    local name=$1
    echo "RUN: $name -> $output/$name.log"
    local started=$SECONDS gate_status=0
    if ! bash "scripts/$name.sh" >"$output/$name.log" 2>&1; then
        gate_status=1
    fi
    printf '%s\n' "$gate_status" >"$gate_scratch/$name.status"
    printf '%s\n' "$((SECONDS - started))" >"$gate_scratch/$name.seconds"
}

# 並列（--jobs>1）では m0 を先に完走させる。m0 がビルドと全体テストを済ませるので、
# 後段の門番は cargo の錠を奪い合わずにビルド済みの処理系を使う。門番は木を書かない。
prelude=()
fanout=("${gates[@]}")
if [ "$gate_jobs" -gt 1 ]; then
    fanout=()
    for name in "${gates[@]}"; do
        if [ "$name" = verify-m0 ]; then
            prelude+=("$name")
        else
            fanout+=("$name")
        fi
    done
fi
for name in "${prelude[@]}"; do
    hado_run_gate "$name"
done
running=0
for name in "${fanout[@]}"; do
    if [ "$gate_jobs" -eq 1 ]; then
        hado_run_gate "$name"
        continue
    fi
    while [ "$running" -ge "$gate_jobs" ]; do
        done_pid=
        wait -n -p done_pid 2>/dev/null || true
        if [ -n "$done_pid" ]; then
            kept=()
            for pid in "${gate_worker_pids[@]}"; do
                [ "$pid" = "$done_pid" ] || kept+=("$pid")
            done
            gate_worker_pids=("${kept[@]}")
        fi
        running=$((running - 1))
    done
    hado_run_gate "$name" &
    gate_worker_pids+=("$!")
    running=$((running + 1))
done
wait || true
# reap 済みの pid を残すと、再利用された無関係のプロセスを殺し得る。
gate_worker_pids=()

# 集約。門番の順は走行する門番の一覧のまま。status の無い門番は赤。
for name in "${gates[@]}"; do
    s=$(cat "$gate_scratch/$name.status" 2>/dev/null) || s=
    if [ -z "$s" ]; then
        echo "run-gates: no status from gate: $name" >&2
        s=1
    fi
    if [ "$s" -ne 0 ]; then status=1; fi
    seconds=$(cat "$gate_scratch/$name.seconds" 2>/dev/null) || seconds=0
    record_time "${seconds:-0}" "$s" "$name"
done

# core の終わりに、この走行で回さなかった門番を見せる。層をまたいだ取り違えで
# 赤を見逃さないため（docs/plan.md 33 節）。明示的に足した門番は差し引く。
if [ "$tier" = "core" ]; then
    echo "full-only gates (not run in this run):"
    LC_ALL=C comm -23 <(hado_tier_all_gates | LC_ALL=C sort) \
        <(for name in "${gates[@]}"; do printf '%s\n' "${name#verify-}"; done | LC_ALL=C sort) |
        sed 's/^/  /'
fi

# 文書の整合は処理系の門番ではないので、full 層（と引数なしの全走）の最後に
# 文書層として回す（docs/plan.md 33 節）。
if [ "$tier" = "full" ] || [ "$default_all" -eq 1 ]; then
    echo "RUN: lint-docs -> $output/lint-docs.log"
    started=$SECONDS
    lint_status=0
    if ! bash scripts/lint-docs.sh >"$output/lint-docs.log" 2>&1; then
        lint_status=1
        status=1
    fi
    record_time "$((SECONDS - started))" "$lint_status" lint-docs
fi

# 走行のログの集計は Hado の道具で出す（`docs/rounds/r51.md`）。出力先には書かない。
# 集計自身がそのディレクトリを読むので、自分のログを混ぜると入口の契約（全ての項目が
# PASSED: n/N の行を持つ）を破る。読めないログがあれば走行を赤にする。
hado_binary="${CARGO_TARGET_DIR:-target}/debug/hado"
if [ -x "$hado_binary" ]; then
    tally_status=0
    tally=$("$hado_binary" run tools/gates/main.hd -- "$output" 2>&1) || tally_status=$?
    if [ "$tally_status" -eq 0 ]; then
        printf 'TALLY: %s\n' "$tally"
    else
        printf 'TALLY: failed (%s): %s\n' "$tally_status" "$tally" >&2
        status=1
    fi
fi

exit "$status"
