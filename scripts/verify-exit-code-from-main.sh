#!/usr/bin/env bash
# tier: core
# round: exit-code-from-main
# §54 の終了コードと §55 の log_error（`docs/rounds/exit-code-from-main.md`）。
# 完了条件をそのまま測る。値は仕様の境界（0・255・256・-1・i64 の両端）で選ぶ。
# 生成した実行ファイルと `hado run` の両方で測る（完了条件 2）。
# log_error の失敗は ENOSPC（/dev/full）と EPIPE（stderr の読み手が先に終わる）の
# 両方で測る。数値の `// expect:` を持つ入口はファイル名で選ばず全て走らせる。
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0
check() {
    local label="$1"
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado="$target_dir/debug/hado"

# `run_program <名前> <ソース>` はビルドして走らせ、status/stdout/stderr を
# グローバルに置く。ビルドが落ちたら status を 255 にして知らせる。
status=0
stdout=''
stderr=''
run_program() {
    local name="$1" source="$2"
    local dir="$workdir/$name"
    mkdir -p "$dir"
    printf '%b' "$source" >"$dir/main.hd"
    if ! "$hado" build "$dir/main.hd" >"$dir/build.out" 2>"$dir/build.err"; then
        cat "$dir/build.err" >&2
        status=255
        stdout=''
        stderr='(build failed)'
        return 1
    fi
    "$dir/main" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

# 同じプログラムを `hado run` でも走らせる。終了コードは guest のものを返す。
run_cli() {
    local name="$1" source="$2"
    local dir="$workdir/cli-$name"
    mkdir -p "$dir"
    printf '%b' "$source" >"$dir/main.hd"
    "$hado" run "$dir/main.hd" >"$dir/out" 2>"$dir/err"
    status=$?
    stdout=$(cat "$dir/out")
    stderr=$(cat "$dir/err")
    return 0
}

# --- 1. 範囲内の Int は終了コードそのもので、何も印字しない ---
in_range_status() {
    local label="$1" value="$2"
    run_program "in-$label" "def main = $value" || return 1
    [ "$status" -eq "$value" ] || { echo "  built status=$status want=$value" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  built stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  built stderr=$stderr" >&2; return 1; }
    run_cli "in-$label" "def main = $value" || return 1
    [ "$status" -eq "$value" ] || { echo "  run status=$status want=$value" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  run stdout=$stdout" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  run stderr=$stderr" >&2; return 1; }
}
for value in 0 1 3 42 255; do
    check "1 Int $value is the exit code and prints nothing" in_range_status "$value" "$value"
done

# Int 以外の main は今までどおり印字して 0 で終わる。
non_int() {
    local label="$1" source="$2" expected="$3"
    run_program "non-$label" "$source" || return 1
    [ "$status" -eq 0 ] || { echo "  built status=$status want=0" >&2; return 1; }
    [ "$stdout" = "$expected" ] || { echo "  built stdout=$stdout want=$expected" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  built stderr=$stderr" >&2; return 1; }
    run_cli "non-$label" "$source" || return 1
    [ "$status" -eq 0 ] || { echo "  run status=$status want=0" >&2; return 1; }
    [ "$stdout" = "$expected" ] || { echo "  run stdout=$stdout want=$expected" >&2; return 1; }
    [ -z "$stderr" ] || { echo "  run stderr=$stderr" >&2; return 1; }
}
check '1 String main prints and exits 0' non_int string 'def main = "hello"' 'hello'
check '1 Bool main prints and exits 0' non_int bool 'def main = 1 < 2' 'true'
check '1 Void main prints nothing and exits 0' non_int void 'def main = ()' ''

# raise は届いても 1 のまま。
raised() {
    run_program raise 'def main = raise { message = "boom" }' || return 1
    [ "$status" -eq 1 ] || { echo "  built status=$status want=1" >&2; return 1; }
    [ "$stderr" = 'boom' ] || { echo "  built stderr=$stderr" >&2; return 1; }
    run_cli raise 'def main = raise { message = "boom" }' || return 1
    [ "$status" -eq 1 ] || { echo "  run status=$status want=1" >&2; return 1; }
    [ "$stderr" = 'boom' ] || { echo "  run stderr=$stderr" >&2; return 1; }
}
check '1 raise reaching main exits 1 with the message' raised

# --- 2. 範囲外は縮める前に診断する。負数も 256 も OS に正規化させない ---
out_of_range() {
    local label="$1" expression="$2"
    run_program "out-$label" "def main = $expression" || return 1
    [ "$status" -eq 1 ] || { echo "  built status=$status want=1" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  built stdout=$stdout" >&2; return 1; }
    [ "$stderr" = 'hado: exit code out of range' ] || { echo "  built stderr=$stderr" >&2; return 1; }
    run_cli "out-$label" "def main = $expression" || return 1
    [ "$status" -eq 1 ] || { echo "  run status=$status want=1" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  run stdout=$stdout" >&2; return 1; }
    [ "$stderr" = 'hado: exit code out of range' ] || { echo "  run stderr=$stderr" >&2; return 1; }
}
check '2 -1 is diagnosed' out_of_range neg1 '-1'
check '2 256 is diagnosed' out_of_range over256 '256'
check '2 i64 max is diagnosed' out_of_range imax '9223372036854775807'
# i64 の最小値はリテラルに書けない（E112）ので引き算で作る。
check '2 i64 min is diagnosed' out_of_range imin '(-9223372036854775807 - 1)'

# --- 3. §55 の log_error は stderr へ、log は stdout へ ---
log_error_streams() {
    local source='def main =\n  log_error "x" ->\n  ()'
    run_program logerr "$source" || return 1
    [ "$status" -eq 0 ] || { echo "  built status=$status want=0" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  built stdout=$stdout" >&2; return 1; }
    [ "$stderr" = 'x' ] || { echo "  built stderr=$stderr" >&2; return 1; }
    run_cli logerr "$source" || return 1
    [ "$status" -eq 0 ] || { echo "  run status=$status want=0" >&2; return 1; }
    [ -z "$stdout" ] || { echo "  run stdout=$stdout" >&2; return 1; }
    [ "$stderr" = 'x' ] || { echo "  run stderr=$stderr" >&2; return 1; }
    # 対照: log は stdout、log_error は stderr に分かれる。
    run_program both 'def main =\n  log "out" ->\n  log_error "err" ->\n  ()' || return 1
    [ "$status" -eq 0 ] || { echo "  both status=$status want=0" >&2; return 1; }
    [ "$stdout" = 'out' ] || { echo "  both stdout=$stdout" >&2; return 1; }
    [ "$stderr" = 'err' ] || { echo "  both stderr=$stderr" >&2; return 1; }
}
check '3 log_error writes stderr only and log writes stdout' log_error_streams

# --- 4. 失敗した log_error は捕捉できる IoFailure。捕まえなければ 1 ---
# 捕捉の証拠は、handler がプログラムの隣に残す `observed` のファイル。
write_log_error_handler() {
    cat >"$1/main.hd" <<'HD'
handler Error as Catch
abort raise e =>
  write_file "observed" (Bytes.of_string e.message) ->
  ()
return value => value
def main =
  with Catch ->
  log_error "x"
HD
}

# ENOSPC: 書き込み先を /dev/full にする。
log_error_caught() {
    local dir="$workdir/logerr-caught"
    mkdir -p "$dir"
    write_log_error_handler "$dir"
    "$hado" build -o "$dir/program" "$dir/main.hd" >/dev/null 2>&1 || return 1
    (cd "$dir" && sh -c './program 2>/dev/full')
    [ $? -eq 0 ] || return 1
    [ "$(cat "$dir/observed" 2>/dev/null)" = 'IoFailure' ]
}
check '4 a failed log_error is a catchable IoFailure' log_error_caught

log_error_uncaught() {
    local dir="$workdir/logerr-uncaught"
    mkdir -p "$dir"
    printf 'def main =\n  log_error "x" ->\n  ()\n' >"$dir/main.hd"
    "$hado" build -o "$dir/program" "$dir/main.hd" >/dev/null 2>&1 || return 1
    (cd "$dir" && sh -c './program 2>/dev/full')
    [ $? -eq 1 ]
}
check '4 an uncaught log_error failure exits 1' log_error_uncaught

# EPIPE: stderr の読み手が先に終わったパイプへ書く。シグナル（141）で終わらせず、
# 他の書き込み失敗と同じにする（§54・§55）。
log_error_epipe_uncaught() {
    local dir="$workdir/logerr-epipe-uncaught" helper status=0
    mkdir -p "$dir"
    printf 'def main =\n  log_error "x" ->\n  ()\n' >"$dir/main.hd"
    "$hado" build -o "$dir/program" "$dir/main.hd" >/dev/null 2>&1 || return 1
    helper=$(readlink -f scripts/exec-stderr-epipe.py)
    (cd "$dir" && python3 "$helper" ./program) || status=$?
    [ "$status" -eq 1 ]
}
check '4 an uncaught stderr EPIPE exits 1, not 141' log_error_epipe_uncaught

log_error_epipe_caught() {
    local dir="$workdir/logerr-epipe-caught" helper status=0
    mkdir -p "$dir"
    write_log_error_handler "$dir"
    "$hado" build -o "$dir/program" "$dir/main.hd" >/dev/null 2>&1 || return 1
    helper=$(readlink -f scripts/exec-stderr-epipe.py)
    (cd "$dir" && python3 "$helper" ./program) || status=$?
    [ "$status" -eq 0 ] || return 1
    [ "$(cat "$dir/observed" 2>/dev/null)" = 'IoFailure' ]
}
check '4 a caught stderr EPIPE is an IoFailure' log_error_epipe_caught

# 印字の失敗も 1。
print_failure() {
    local dir="$workdir/printfail"
    mkdir -p "$dir"
    printf 'def main = "x"' >"$dir/main.hd"
    "$hado" build "$dir/main.hd" >/dev/null 2>&1 || return 1
    "$dir/main" >/dev/full 2>/dev/null
    [ $? -eq 1 ]
}
check '4 a failed print exits 1' print_failure

# Int の main は印字しないので、stdout が壊れていても終了コードは値のまま。
int_ignores_broken_stdout() {
    local dir="$workdir/int-print"
    mkdir -p "$dir"
    printf 'def main = 7' >"$dir/main.hd"
    "$hado" build "$dir/main.hd" >/dev/null 2>&1 || return 1
    "$dir/main" >/dev/full 2>/dev/null
    [ $? -eq 7 ]
}
check '4 an Int main never writes so a broken stdout does not matter' int_ignores_broken_stdout

# --- 5. `// expect:` を持つ入口は、その行の値を印字する ---
# 入口はファイル名で選ばない（§50 の第 2 入口を漏らさない）。`// expect:` は
# stdout 全体（`docs/corpus.md` の「trim 後」）。`// run-expect:` は 1 行ずつを
# 順に並べた stdout。タグの値がそのまま期待値で、判定器には書き写さない。
# 数値でも文字列でも構造でも同じ照合をする（`0001,0002,0003` のように複数値を
# 1 行で書く形も 1 つの値として丸ごと比べる）。印字の期待を持たない入口
# （`// expect-error:` だけの入口や、出力ヘッダの無い入口）は対象外。
expect_is_printed() {
    local out_dir="$workdir/expect" red=0 seen=0
    mkdir -p "$out_dir"
    local file want out got rc
    while IFS= read -r file; do
        grep -q '^def main' "$file" || continue
        if grep -qE '^// expect:' "$file"; then
            want=$(sed -nE 's|^// expect: ?(.*)$|\1|p' "$file" | head -1 | sed 's/[[:space:]]*$//')
        elif grep -qE '^// run-expect:' "$file"; then
            want=$(sed -nE 's|^// run-expect: ?(.*)$|\1|p' "$file" \
                | sed 's/[[:space:]]*$//' \
                | awk 'NR > 1 { printf "\n" } { printf "%s", $0 }')
        else
            continue
        fi
        seen=$((seen + 1))
        out="$out_dir/$(echo "$file" | tr '/.' '__')"
        run_dir="$out_dir/run-$(echo "$file" | tr '/.' '__')"
        mkdir -p "$run_dir"
        if ! "$hado" build -o "$out" "$file" >/dev/null 2>"$out_dir/build.err"; then
            echo "  $file: build failed" >&2
            red=$((red + 1))
            continue
        fi
        # `corpus.rs` と同じ条件で走らせる。作業ディレクトリを分けて、stdin は空
        # （`env_io_roundtrip` は `read_stdin` の長さを印字する）。
        got=$(cd "$run_dir" && "$out" </dev/null 2>/dev/null)
        rc=$?
        if [ "$got" != "$want" ] || [ "$rc" -ne 0 ]; then
            echo "  $file: want='$want' got='$got' rc=$rc" >&2
            red=$((red + 1))
        fi
    done < <(grep -rlE '^// (run-)?expect:' tests stdlib | sort)
    [ "$seen" -gt 0 ] || { echo "  no // expect: entry" >&2; return 1; }
    [ "$red" -eq 0 ]
}
check '5 every // expect: entry prints its line' expect_is_printed

# --- 7. `hado test` は guest の終了コードをそのまま報告する ---
# 期待する値はここで作り、門番に書き写さない。
test_reports_exit_code() {
    local dir="$workdir/tests"
    mkdir -p "$dir"
    local code other
    code=$((2 + RANDOM % 254))
    other=$((2 + (code + 100) % 254))
    printf 'def main = 0\n' >"$dir/main.hd"
    printf 'def main = %s\n' "$code" >"$dir/a_test.hd"
    printf 'def main = %s\n' "$other" >"$dir/b_test.hd"
    printf 'def main = 0\n' >"$dir/c_test.hd"
    local report
    report=$("$hado" test "$dir/main.hd" 2>"$dir/test.err")
    local rc=$?
    [ "$rc" -eq 1 ] || { echo "  rc=$rc want=1" >&2; return 1; }
    [ "$report" = "test: a_test.hd failed (exit $code)
test: b_test.hd failed (exit $other)
test: c_test.hd ok
test: 1/3 passed" ] || { echo "  report=$report" >&2; return 1; }
    [ ! -s "$dir/test.err" ]
}
check '7 hado test reports each guest exit code in path order' test_reports_exit_code

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
