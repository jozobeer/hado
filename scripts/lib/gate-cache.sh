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

if [ -z "${HADO_GATE_CACHE_RUN:-}" ]; then
    HADO_GATE_CACHE_RUN="pid$$-$(date +%s)-${RANDOM:-0}"
    export HADO_GATE_CACHE_RUN
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

# cargo の出力と終了コードを、同じ内容・同じ引数のあいだで再利用する。
cargo_cached() {
    gate_cached "cargo:$*" cargo "$@"
}
