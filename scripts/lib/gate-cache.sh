#!/usr/bin/env bash
# 門番の入れ子呼び出しと cargo test の結果を、1 回の走行の中で 1 度だけ実走する
# ための内容キー付きキャッシュ。
#
# なぜ判定を緩めないか:
# - 鍵は「作業ツリーの内容のハッシュ」と「コマンドの完全な引数」で決まる。内容が
#   同じなら同じ結果を返すコマンドだけを対象にする。
# - 成功（終了コード 0）だけを保存する。赤は毎回実走するので、失敗を古い結果で
#   覆い隠さない。
# - キャッシュは走行 1 回のスコープに閉じる（scripts/run-gates.sh が発行する）。
#   乱数で探針を作る門番を走行間で凍結せず、root / 非 root のような内容に現れない
#   環境差を結果に持ち越さないためである。
#
# 使い方:
#   source scripts/lib/gate-cache.sh
#   gate_cached <label> <command> [args...]
#   cargo_cached <cargo の引数...>
#   cargo_test_cached <cargo test の引数...>
#
# cargo_test_cached は、同じ走行の中で全走の全体テストが成功していて、その
# ログに対象の実行と ok の証拠があるときだけ、`--test <名前>` の部分テストの
# 実走を省く（docs/plan.md 33 節）。証拠が無ければ cargo_cached と同じく実走する。

hado_gate_repo_root() {
    local here
    here=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
    printf '%s' "$here"
}

# 作業ツリーの内容のハッシュ。target/ と .git/ と __pycache__/ は除外する。
hado_gate_fingerprint() {
    local repo
    repo=$(hado_gate_repo_root)
    (
        cd "$repo" || exit 1
        find . -type f \
            -not -path './target/*' -not -path './.git/*' -not -path '*/__pycache__/*' \
            -print0 |
            LC_ALL=C sort -z |
            xargs -0 -r sha256sum |
            sha256sum |
            cut -d' ' -f1
    )
}

# 死んだ走行のスコープを掃除する。生きている走行に触れないよう、既定 1 日より
# 古いものだけを消す。全走は分単位なので十分に安全である。走行を始めるときだけ走らす。
hado_gate_cache_prune() {
    local base
    base="$(hado_gate_repo_root)/target/gate-cache"
    [ -d "$base" ] || return 0
    find "$base" -mindepth 1 -maxdepth 1 -type d \
        -mmin +"${HADO_GATE_CACHE_TTL_MINUTES:-1440}" \
        -exec rm -rf -- {} + 2>/dev/null || true
}

if [ -z "${HADO_GATE_CACHE_RUN:-}" ]; then
    HADO_GATE_CACHE_RUN="pid$$-$(date +%s)-${RANDOM:-0}"
    export HADO_GATE_CACHE_RUN
    hado_gate_cache_prune
fi

hado_gate_cache_run_dir() {
    printf '%s/target/gate-cache/%s' "$(hado_gate_repo_root)" "$HADO_GATE_CACHE_RUN"
}

# gate_cached <label> <command> [args...]
# 成功した結果だけを、内容ハッシュと label + 引数のハッシュで再利用する。
gate_cached() {
    local label=$1
    shift
    local base id dir tmp status
    base=$(hado_gate_cache_run_dir)
    id=$(printf '%s\0' "$label" "$@" | sha256sum | cut -c1-32)
    dir="$base/$(hado_gate_fingerprint)/$id"
    if [ -f "$dir/status" ] && [ "$(cat "$dir/status")" = "0" ]; then
        cat "$dir/out"
        return 0
    fi
    mkdir -p "$dir"
    printf '%s\n' "$label" >"$dir/label"
    printf '%s\n' "$*" >"$dir/argv"
    tmp="$dir/out.tmp.$$"
    ( "$@" ) >"$tmp" 2>&1
    status=$?
    if [ "$status" -eq 0 ]; then
        cat "$tmp"
        mv "$tmp" "$dir/out"
        printf '0\n' >"$dir/status"
    else
        cat "$tmp"
        rm -f "$tmp"
    fi
    return "$status"
}

# 全走の全体テスト（cargo test --offline --no-fail-fast）だけを指す。
hado_gate_is_full_test() {
    [ "$*" = "test --offline --no-fail-fast" ]
}

# 全走の出力を、この内容の証拠として走行スコープに残す。
hado_gate_full_test_log() {
    printf '%s/full-test-%s.log' "$(hado_gate_cache_run_dir)" "$(hado_gate_fingerprint)"
}

# cargo の出力と終了コードを、同じ内容・同じ引数のあいだで再利用する。
# 全走の全体テストだけは、その出力を走行スコープに残す（cargo_test_cached が使う）。
cargo_cached() {
    if ! hado_gate_is_full_test "$@"; then
        gate_cached "cargo:$*" cargo "$@"
        return $?
    fi
    local log status=0
    log=$(hado_gate_full_test_log)
    mkdir -p "$(dirname "$log")"
    gate_cached "cargo:$*" cargo "$@" | tee "$log"
    status=${PIPESTATUS[0]}
    # 赤い全走は証拠として残さない（「成功だけを保存する」原則）。
    if [ "$status" -ne 0 ]; then
        rm -f -- "$log"
    fi
    return "$status"
}

# cargo_test_cached の再利用を許す形か。許すのは
#   cargo test [--offline] [--no-fail-fast] --test <名前> ...
# だけである。`--` 以降（--nocapture など）や -q / --lib / --features /
# --release などが 1 つでもあれば実走する。引数の解釈を広げない。
hado_gate_narrow_test_argv() {
    local -a args=("$@")
    local i=1 has=0 a
    [ "${#args[@]}" -ge 1 ] || return 1
    [ "${args[0]}" = test ] || return 1
    while [ "$i" -lt "${#args[@]}" ]; do
        a=${args[$i]}
        case "$a" in
            --offline | --no-fail-fast) ;;
            --test)
                i=$((i + 1))
                [ "$i" -lt "${#args[@]}" ] || return 1
                has=$((has + 1))
                ;;
            --test=*)
                [ -n "${a#--test=}" ] || return 1
                has=$((has + 1))
                ;;
            *) return 1 ;;
        esac
        i=$((i + 1))
    done
    [ "$has" -gt 0 ]
}

hado_gate_narrow_test_targets() {
    local -a args=("$@")
    local i=1 a
    while [ "$i" -lt "${#args[@]}" ]; do
        a=${args[$i]}
        case "$a" in
            --test)
                i=$((i + 1))
                printf '%s\n' "${args[$i]}"
                ;;
            --test=*) printf '%s\n' "${a#--test=}" ;;
        esac
        i=$((i + 1))
    done
}

# 全走のログに「tests/<名前>.rs を実行して ok」の証拠があるか。
# 証拠が無い（対象が改名・削除された、全走が赤だった）なら実走する。
hado_gate_full_log_has_target() {
    local log=$1 target=$2
    awk -v t="$target" '
        $0 ~ ("Running .*tests/" t "\\.rs ") { seen=1; next }
        seen && /test result: ok\./ { ok=1; exit }
        seen && (/test result: FAILED/ || /^ *Running / || /^ *Doc-tests / || /^error/ || /^failures:/) { exit }
        END { exit !(ok) }
    ' "$log"
}

# 再利用した根拠（対象の実行行と結果行）を出す。全走ログ 700 行を写さない。
hado_gate_full_log_evidence() {
    local log=$1 target=$2
    awk -v t="$target" '
        $0 ~ ("Running .*tests/" t "\\.rs ") { seen=1; print; next }
        seen && /test result:/ { print; exit }
    ' "$log"
}

# 部分テストの実走を全走に統合する。全走がこの内容で成功し、そのログに
# 対象の実行と ok の証拠があるときだけ、実走せず成功を再利用する。
# 「全走成功 ⇒ 部分成功」は対象が実在するときだけ成り立つので、証拠を要求する。
cargo_test_cached() {
    local -a targets=()
    local target log covered=1
    if ! hado_gate_narrow_test_argv "$@"; then
        cargo_cached "$@"
        return $?
    fi
    log=$(hado_gate_full_test_log)
    if [ ! -f "$log" ]; then
        cargo_cached "$@"
        return $?
    fi
    mapfile -t targets < <(hado_gate_narrow_test_targets "$@")
    for target in "${targets[@]}"; do
        if ! hado_gate_full_log_has_target "$log" "$target"; then
            covered=0
            break
        fi
    done
    if [ "$covered" -ne 1 ]; then
        cargo_cached "$@"
        return $?
    fi
    printf 'gate-cache: full test already covered: cargo %s\n' "$*"
    for target in "${targets[@]}"; do
        hado_gate_full_log_evidence "$log" "$target"
    done
    return 0
}
