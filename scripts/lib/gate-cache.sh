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
# 実走を省く（docs/plan.md 33 節）。
#
# 並列（docs/plan.md 34 節）:
# - 同じ id の gate_cached を並行に呼んでも実走は 1 回である。$dir/lock を flock
#   で取り、取ってから結果を再確認する。flock が無い環境ではその保護だけを省く
#   （直列は安全。run-gates.sh --jobs>1 は flock を要求する）。
# - 公開は out → status の順の rename。読み手は status=0 を見たとき完全な out が
#   あることを前提にできる。
# - 全走のログも成功したときだけ temp → rename で公開し、既に公開済みなら
#   再公開しない。書きかけのログを証拠として読ませない。証拠が無ければ cargo_cached と同じく実走する。

hado_gate_repo_root() {
    local here
    here=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
    printf '%s' "$here"
}

# 作業ツリーの内容のハッシュ。target/ と .git/ と __pycache__/ は除外する。
# HADO_GATE_FINGERPRINT_EXCLUDE があれば、そのリポジトリ相対のパス以下も除く。
# run-gates は走行自身がログを書く出力先をここに入れる（ログは門番の入力では
# ないので、鍵と並列の安定性 guard を走行の書き込みで揺らさない）。
hado_gate_fingerprint() {
    local repo exclude="${HADO_GATE_FINGERPRINT_EXCLUDE:-}"
    repo=$(hado_gate_repo_root)
    (
        cd "$repo" || exit 1
        if [ -n "$exclude" ]; then
            find . -type f \
                -not -path './target/*' -not -path './.git/*' -not -path '*/__pycache__/*' \
                -not -path "./$exclude/*" \
                -print0
        else
            find . -type f \
                -not -path './target/*' -not -path './.git/*' -not -path '*/__pycache__/*' \
                -print0
        fi |
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
    # 新しい走行スコープに、前のスコープの保持 id は関係ない。
    unset HADO_GATE_HELD_IDS
    hado_gate_cache_prune
fi

hado_gate_cache_run_dir() {
    printf '%s/target/gate-cache/%s' "$(hado_gate_repo_root)" "$HADO_GATE_CACHE_RUN"
}

# gate_cached <label> <command> [args...]
# 成功した結果だけを、内容ハッシュと label + 引数のハッシュで再利用する。
# 同じ id を並行に呼んでも実走は 1 回である（$dir/lock の flock）。公開は
# out → status の順の rename で、読み手に部分的な結果を見せない。
hado_gate_cache_hit() {
    [ -f "$1/status" ] && [ "$(cat "$1/status")" = "0" ]
}

# HADO_GATE_RUNS（任意）に実走と再利用を 1 行ずつ残す。並列の実走回数を数える診断。
hado_gate_note() {
    [ -n "${HADO_GATE_RUNS:-}" ] || return 0
    printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$HADO_GATE_RUNS" 2>/dev/null || true
}

# lock の中身。既に結果があれば再生し、無ければ実走して成功だけを公開する。
# 呼び出し側が flock を取る（取れない環境では直列前提で直接呼ぶ）。
hado_gate_cache_exec() {
    local dir=$1 label=$2
    shift 2
    if hado_gate_cache_hit "$dir"; then
        hado_gate_note hit "$label" "${*}"
        cat "$dir/out"
        return 0
    fi
    printf '%s\n' "$label" >"$dir/label"
    printf '%s\n' "$*" >"$dir/argv"
    hado_gate_note run "$label" "$*"
    local tmp="$dir/out.$BASHPID.tmp" status=0
    # lock の fd（9）は実走するコマンドへ継がせない。
    ( "$@" ) >"$tmp" 2>&1 9>&- || status=$?
    cat "$tmp"
    if [ "$status" -eq 0 ]; then
        mv -- "$tmp" "$dir/out"
        printf '0\n' >"$dir/status.$BASHPID.tmp"
        mv -- "$dir/status.$BASHPID.tmp" "$dir/status"
    else
        rm -f -- "$tmp"
    fi
    return "$status"
}

gate_cached() {
    local label=$1
    shift
    local base id dir rc saved_held
    base=$(hado_gate_cache_run_dir)
    id=$(printf '%s\0' "$label" "$@" | sha256sum | cut -c1-32)
    dir="$base/$(hado_gate_fingerprint)/$id"
    if hado_gate_cache_hit "$dir"; then
        hado_gate_note hit "$label" "$*"
        cat "$dir/out"
        return 0
    fi
    # 同じ id を保持したまま再入すると flock が永久に待つ。門番が自分自身を
    # 引く循環を、待たずに赤で止める（保持 id は子へ継がせる）。
    case " ${HADO_GATE_HELD_IDS:-} " in
        *" $id "*)
            echo "gate_cached: recursive cache re-entry: $label" >&2
            return 1
            ;;
    esac
    mkdir -p "$dir"
    saved_held=${HADO_GATE_HELD_IDS:-}
    HADO_GATE_HELD_IDS="$saved_held $id"
    export HADO_GATE_HELD_IDS
    if command -v flock >/dev/null 2>&1; then
        (flock 9 && hado_gate_cache_exec "$dir" "$label" "$@") 9>"$dir/lock"
        rc=$?
    else
        hado_gate_cache_exec "$dir" "$label" "$@"
        rc=$?
    fi
    HADO_GATE_HELD_IDS=$saved_held
    if [ -n "$saved_held" ]; then
        export HADO_GATE_HELD_IDS
    else
        unset HADO_GATE_HELD_IDS
    fi
    return "$rc"
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
# ログは成功したときだけ temp → rename で公開する。書き手が書いている途中の
# ログを読み手に読ませず、赤い全走のログは証拠として残さない。
cargo_cached() {
    if ! hado_gate_is_full_test "$@"; then
        gate_cached "cargo:$*" cargo "$@"
        return $?
    fi
    local log status=0 tmp
    log=$(hado_gate_full_test_log)
    mkdir -p "$(dirname "$log")"
    # 公開済みなら再公開しない（rename 済みの完全なログを上書きしない）。
    if [ -f "$log" ]; then
        gate_cached "cargo:$*" cargo "$@"
        return $?
    fi
    tmp=$(mktemp "$(dirname "$log")/full-test.XXXXXX")
    # パイプを使わない。呼び出し側が set -e と pipefail でも、赤のログを
    # 消す前に抜けないためである。
    if gate_cached "cargo:$*" cargo "$@" >"$tmp" 2>&1; then
        status=0
    else
        status=$?
    fi
    cat "$tmp"
    if [ "$status" -eq 0 ]; then
        mv -- "$tmp" "$log"
    else
        # 赤は公開しない。$log は成功時にしか作られず、あれば手前で返っている。
        rm -f -- "$tmp"
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
