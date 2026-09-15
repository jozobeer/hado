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

# 並列（--jobs>1）は同一門番の実走を flock で 1 回にする。gate-cache.sh がある
# 木だけが対象で、合成木のように gate_cached が素通しの木は flock 無しでも動く。
if [ "$gate_jobs" -gt 1 ] && [ -f "$(dirname "$0")/../scripts/lib/gate-cache.sh" ]; then
    command -v flock >/dev/null 2>&1 || {
        echo 'run-gates: --jobs requires flock' >&2
        exit 1
    }
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
    # 入れ子の run-gates（合成木・部分走）には診断の記録を継がせない。
    unset HADO_GATE_RUNS
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
else
    gate_cached() {
        local label=$1
        shift
        "$@"
    }
fi

# 走行自身がログを書く出力先は門番の入力ではない。内容ハッシュから外し、並列の
# 安定性 guard とキャッシュの鍵がログの増加で揺れないようにする（出力先がツリーの
# 外なら何もしない）。symlink 越しでも取り違えないよう実体のパスで比べる。
if declare -F hado_gate_repo_root >/dev/null 2>&1; then
    repo_root=$(cd -- "$(hado_gate_repo_root)" && pwd -P)
    output_real=$(cd -- "$output" && pwd -P)
    case "$output_real" in
        "$repo_root"/*)
            HADO_GATE_FINGERPRINT_EXCLUDE=${output_real#"$repo_root"/}
            export HADO_GATE_FINGERPRINT_EXCLUDE
            ;;
    esac
fi

# 並列のワーカーの pid と作業ディレクトリ。中断時にワーカーを止めてから
# 走行スコープと作業ディレクトリを消す。kill は親の subshell にしか当たらず、
# flock を握る subshell や門番本体が孤児で残るので、子孫を葉から止める。
gate_scratch=""
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
    if [ -n "$gate_scratch" ]; then rm -rf -- "$gate_scratch"; fi
    if [ -n "$gate_run_dir" ]; then rm -rf -- "$gate_run_dir"; fi
    return 0
}
trap hado_cleanup EXIT

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

# ワーカー（または前段）として門番 1 本を走らせ、終了コードと所要を作業
# ディレクトリに残す。ログは平坦な $output/<名>.log に出す（--jobs>1）。
hado_run_gate() {
    local name=$1
    echo "RUN: $name -> $output/$name.log"
    local started=$SECONDS gate_status=0
    # label は入れ子の呼び出しと同じ `verify-<名>` にする(下の直列のループと同じ理由)。
    if ! gate_cached "$name" bash "scripts/$name.sh" >"$output/$name.log" 2>&1; then
        gate_status=1
    fi
    printf '%s\n' "$gate_status" >"$gate_scratch/$name.status"
    printf '%s\n' "$((SECONDS - started))" >"$gate_scratch/$name.seconds"
}

# 終了したワーカーを追跡から外す（reap 済みの pid を cleanup が殺さないため）。
hado_forget_pid() {
    local pid=$1 kept=() p
    for p in "${gate_worker_pids[@]}"; do
        if [ "$p" != "$pid" ]; then
            kept+=("$p")
        fi
    done
    gate_worker_pids=("${kept[@]}")
}

# 前段（直列の一覧 ∩ 走行する門番）を完走させ、残りを N 並列でファンアウトし、
# 終了コードと所要を 1 か所に集約する。status は呼び出し側の大域変数。
hado_run_parallel() {
    local mode=parallel running=0 name s seconds done_pid= stable_before= stable_after=
    gate_scratch=$(mktemp -d)
    # m0 がビルドと全走テストを完走し、full-test の証拠を確定させる。
    for name in "${prelude_gates[@]}"; do
        hado_run_gate "$name"
    done
    # 前段に赤があれば後段は直列に落とす。赤い門番は probe を戻し切らずに終わり
    # 得るので、その状態で並列に走らせると鍵が揺れて入れ子が再走する。判定は既に
    # 赤で確定しているので、緩めずに直列へ落とすだけにする。
    for name in "${prelude_gates[@]}"; do
        s=$(cat "$gate_scratch/$name.status" 2>/dev/null) || s=1
        if [ "$s" -ne 0 ]; then
            mode=serial
        fi
    done
    if [ "$mode" = serial ]; then
        echo 'run-gates: a serial gate failed; running the rest serially' >&2
    fi
    # 前段の後はツリーが静止しているはず。後段が恒久的に変えたら赤にする。
    if [ "$mode" = parallel ] && declare -F hado_gate_fingerprint >/dev/null 2>&1; then
        stable_before=$(hado_gate_fingerprint)
    fi
    for name in "${fanout_gates[@]}"; do
        if [ "$mode" = parallel ]; then
            while [ "$running" -ge "$gate_jobs" ]; do
                wait -n -p done_pid 2>/dev/null || true
                if [ -n "$done_pid" ]; then hado_forget_pid "$done_pid"; fi
                running=$((running - 1))
            done
            hado_run_gate "$name" &
            gate_worker_pids+=("$!")
            running=$((running + 1))
        else
            hado_run_gate "$name"
        fi
    done
    wait || true
    # reap 済みの pid を残すと、再利用された無関係のプロセスを殺し得る。
    gate_worker_pids=()
    if [ -n "$stable_before" ]; then
        stable_after=$(hado_gate_fingerprint)
        if [ "$stable_before" != "$stable_after" ]; then
            echo 'run-gates: the working tree changed during the parallel phase' >&2
            status=1
        fi
    fi
    # 集約。門番の順は走行する門番の一覧のまま。status の無いワーカーは赤。
    for name in "${gates[@]}"; do
        s=$(cat "$gate_scratch/$name.status" 2>/dev/null) || s=
        if [ -z "$s" ]; then
            echo "run-gates: no status from gate worker: $name" >&2
            s=1
        fi
        if [ "$s" -ne 0 ]; then status=1; fi
        if [ -n "$gate_times" ]; then
            seconds=$(cat "$gate_scratch/$name.seconds" 2>/dev/null) || seconds=0
            { printf '%s\t%s\t%s\n' "${seconds:-0}" "$s" "$name" >>"$gate_times"; } 2>/dev/null || true
        fi
    done
}

if [ "$gate_jobs" -gt 1 ]; then
    # 直列にする門番の一覧（scripts/lib/gate-tiers.sh が唯一の正）。これらは共有
    # ツリーを改変するので、ファンアウトの前に 1 本ずつ完走させる（docs/plan.md 34 節）。
    # 一覧の無い木では m0 だけを前段に置く。
    serial_names=()
    if declare -F hado_tier_serial_gates >/dev/null 2>&1; then
        mapfile -t serial_names < <(hado_tier_serial_gates)
    else
        serial_names=(m0)
    fi
    prelude_gates=()
    fanout_gates=()
    for name in "${serial_names[@]}"; do
        case "$name" in verify-*) ;; *) name="verify-$name" ;; esac
        for gate in "${gates[@]}"; do
            if [ "$gate" = "$name" ]; then
                prelude_gates+=("$gate")
                break
            fi
        done
    done
    for gate in "${gates[@]}"; do
        skip=0
        for name in "${prelude_gates[@]}"; do
            if [ "$name" = "$gate" ]; then
                skip=1
                break
            fi
        done
        if [ "$skip" -eq 0 ]; then
            fanout_gates+=("$gate")
        fi
    done
    hado_run_parallel
else
    for name in "${gates[@]}"; do
        echo "RUN: $name -> $output/$name.log"
        started=$SECONDS
        gate_status=0
        # label は入れ子の呼び出しと同じ `verify-<名>` にする（m1-corpus/m2/m3 は
        # `gate_cached verify-m0 bash scripts/verify-m0.sh` の形で引く）。ここが
        # ずれると同じ門番を走行内で 2 回以上実走する（docs/plan.md 34 節）。
        if ! gate_cached "$name" bash "scripts/$name.sh" >"$output/$name.log" 2>&1; then
            gate_status=1
            status=1
        fi
        # HADO_GATE_TIMES を設定すると、門番ごとの所要と終了コードを追記する（任意）。
        if [ -n "$gate_times" ]; then
            { printf '%s\t%s\t%s\n' "$((SECONDS - started))" "$gate_status" "$name" >>"$gate_times"; } 2>/dev/null || true
        fi
    done
fi

# core の終わりに、この走行で回さなかった門番を見せる。層をまたいだ取り違えで
# 赤を見逃さないため（docs/plan.md 33 節）。明示的に足した門番は差し引く。
if [ "$tier" = "core" ]; then
    echo "full-only gates (not run in this run):"
    comm -23 <(hado_tier_all_gates | LC_ALL=C sort) \
        <(for name in "${gates[@]}"; do printf '%s\n' "${name#verify-}"; done | LC_ALL=C sort) |
        sed 's/^/  /'
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
