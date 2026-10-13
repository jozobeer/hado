#!/usr/bin/env bash
# tier: quick
# round: nextest
# 全体テストの入口 scripts/nextest-full.sh の判定器。network を使わない。
#
# nextest の代わりに、決めた junit を書いて決めた終了コードで終わる stub を
# HADO_NEXTEST_BIN に渡し、合成木で入口を走らせる。期待値は stub に渡した junit
# から決まる。測るのは次のとおり。
#   1. junit の各テストが `nextest-case` 行になり、失敗・エラーは FAIL、skipped は
#      SKIP になる。終了コードは nextest のものを返し、nextest へ渡す引数は固定。
#   2. 全部が通る junit なら全行 PASS で 0（1 の対照）。
#   3. 前の走行の junit を、書かなかった走行の結果として読まない。
#   4. nextest が 0 で終わっても junit が無ければ赤。
#   5. junit の置き場所が .config/nextest.toml と scripts/nextest-full.sh で一致する。
#   6. 全体テストを cargo test で走らせる門番が無い（nextest と二重に走らない）。
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0
check() {
    local label=$1
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tree="$work/tree"
mkdir -p "$tree/scripts"
cp scripts/nextest-full.sh "$tree/scripts/"
junit="$tree/target/nextest/gate/junit.xml"

stub="$work/stub-nextest"
cat >"$stub" <<'EOS'
#!/usr/bin/env bash
printf '%s\n' "$*" >"$HADO_STUB_ARGS"
if [ -n "${HADO_STUB_JUNIT:-}" ]; then
    mkdir -p target/nextest/gate
    cp "$HADO_STUB_JUNIT" target/nextest/gate/junit.xml
fi
exit "${HADO_STUB_STATUS:-0}"
EOS
chmod +x "$stub"

# run_full <junit の元（空なら書かない）> <stub の終了コード>
# 出力を $work/out、終了コードを $work/rc に残す。
run_full() {
    rm -f "$work/args"
    HADO_NEXTEST_BIN="$stub" HADO_STUB_JUNIT="$1" HADO_STUB_STATUS="$2" \
        HADO_STUB_ARGS="$work/args" bash "$tree/scripts/nextest-full.sh" >"$work/out" 2>&1
    echo "$?" >"$work/rc"
}
cases_of() {
    grep '^nextest-case' "$work/out"
}

cat >"$work/mixed.xml" <<'EOX'
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="5" failures="1" errors="1">
  <testsuite name="hado" tests="1">
    <testcase name="parser::tests::left_assoc_add_and_mul" classname="hado"/>
  </testsuite>
  <testsuite name="hado::walk" tests="2" failures="1">
    <testcase name="walk" classname="hado::walk"/>
    <testcase name="walk_fails" classname="hado::walk"><failure type="test failure">boom</failure><system-out>x</system-out></testcase>
  </testsuite>
  <testsuite name="hado::tail" tests="1" errors="1">
    <testcase name="tail" classname="hado::tail"><error type="SIGSEGV">crash</error></testcase>
  </testsuite>
  <testsuite name="hado::r9_order" tests="1">
    <testcase name="later" classname="hado::r9_order"><skipped/></testcase>
  </testsuite>
</testsuites>
EOX
cat >"$work/mixed.expected" <<'EOT'
nextest-case	PASS	hado	parser::tests::left_assoc_add_and_mul
nextest-case	PASS	hado::walk	walk
nextest-case	FAIL	hado::walk	walk_fails
nextest-case	FAIL	hado::tail	tail
nextest-case	SKIP	hado::r9_order	later
EOT

cat >"$work/green.xml" <<'EOX'
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="nextest-run" tests="2">
  <testsuite name="hado::walk" tests="2">
    <testcase name="walk" classname="hado::walk"/>
    <testcase name="walk_more" classname="hado::walk"><system-out>printed</system-out></testcase>
  </testsuite>
</testsuites>
EOX
cat >"$work/green.expected" <<'EOT'
nextest-case	PASS	hado::walk	walk
nextest-case	PASS	hado::walk	walk_more
EOT

mixed_case() {
    run_full "$work/mixed.xml" 100
    [ "$(cat "$work/rc")" = 100 ] || return 1
    diff <(cases_of) "$work/mixed.expected" >/dev/null || return 1
    [ "$(cat "$work/args")" = "nextest run --offline --no-fail-fast --profile gate" ]
}

green_case() {
    run_full "$work/green.xml" 0
    [ "$(cat "$work/rc")" = 0 ] || return 1
    diff <(cases_of) "$work/green.expected" >/dev/null
}

stale_junit_case() {
    mkdir -p "$(dirname "$junit")"
    cp "$work/green.xml" "$junit"
    run_full "" 101
    [ "$(cat "$work/rc")" = 101 ] || return 1
    [ -z "$(cases_of)" ]
}

missing_junit_case() {
    rm -f "$junit"
    run_full "" 0
    [ "$(cat "$work/rc")" != 0 ] || return 1
    [ -z "$(cases_of)" ]
}

# profile.gate.junit.path は profile の出力先 target/nextest/gate/ からの相対である。
junit_path_agrees() {
    local configured scripted
    configured=$(python3 -c '
import tomllib
with open(".config/nextest.toml", "rb") as f:
    print("target/nextest/gate/" + tomllib.load(f)["profile"]["gate"]["junit"]["path"])
') || return 1
    scripted=$(sed -n 's/^junit=//p' scripts/nextest-full.sh)
    [ -n "$scripted" ] && [ "$configured" = "$scripted" ]
}

# 全体テスト（cargo test --offline --no-fail-fast）を呼ぶ門番が無いこと。gate-cache の
# 証拠の仕組み（scripts/lib/gate-cache.sh）とその判定器は、合成木でその形を使う。
no_libtest_full_run() {
    ! grep -lE -- "(test --offline --no-fail-fast|'test','--offline','--no-fail-fast')" \
        scripts/*.sh |
        grep -vxE 'scripts/(verify-gate-cache|verify-nextest)\.sh' |
        grep -q .
}

check 'junit cases become nextest-case lines with the nextest status' mixed_case
check 'an all-passing junit gives only PASS lines and status 0' green_case
check 'a stale junit is not read as the result of a run that wrote none' stale_junit_case
check 'a zero exit without a junit is red' missing_junit_case
check 'the junit path agrees between .config/nextest.toml and nextest-full.sh' junit_path_agrees
check 'no gate runs the full suite through cargo test' no_libtest_full_run

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
