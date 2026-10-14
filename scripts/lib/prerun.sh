# 門番のケースごとの判定を先に並列で走らせ、結果を元の順で読む（docs/rounds/ci-shards.md）。
#
#   prerun_init <dir>            結果の置き場を作る
#   prerun_start <key> <cmd...>  cmd を背景で走らせ、出力と終了コードを <dir>/<key>.* に残す
#   prerun_wait                  全部の終わりを待つ
#   prerun_result <key>          出力を流し、cmd の終了コードを返す（check から呼ぶ）
#
# 同時に走らせる数は HADO_GATE_PARALLEL（既定は nproc）。cmd は subshell で走るので、
# 呼び出し側の変数を書き換えない関数だけを渡す。標準入力は /dev/null にする。

prerun_init() {
    _prerun_dir=$1
    mkdir -p -- "$_prerun_dir"
    _prerun_limit=${HADO_GATE_PARALLEL:-$(nproc 2>/dev/null || echo 1)}
}

prerun_start() {
    local key=$1
    shift
    while [ "$(jobs -rp | wc -l)" -ge "$_prerun_limit" ]; do
        wait -n 2>/dev/null || true
    done
    (
        status=0
        "$@" </dev/null >"$_prerun_dir/$key.log" 2>&1 || status=$?
        printf '%s\n' "$status" >"$_prerun_dir/$key.status"
    ) &
}

prerun_wait() {
    wait
}

prerun_result() {
    local key=$1 status
    cat -- "$_prerun_dir/$key.log" 2>/dev/null
    status=$(cat -- "$_prerun_dir/$key.status" 2>/dev/null) || status=
    [ "$status" = 0 ]
}
