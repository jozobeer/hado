#!/usr/bin/env bash
# 全体テストを固定した版の nextest で 1 回走らせ、テストごとの結果を 1 行ずつ出す
# （docs/rounds/nextest.md）。門番はこの行だけを読み、nextest の人向けの出力は読まない。
#
#   bash scripts/nextest-full.sh
#
# 出力は nextest の出力に続けて、テストごとに次の 1 行（区切りはタブ）:
#   nextest-case  PASS|FAIL|SKIP  <バイナリ ID>  <テスト名>
# バイナリ ID は lib の単体テストが `hado`、tests/walk.rs が `hado::walk` である。
# 終了コードは nextest の終了コード。nextest が 0 で終わっても結果を読めなければ 1。
#
# nextest は scripts/nextest-setup.sh が target/nextest-setup に取ってくる。
# HADO_NEXTEST_BIN があればそれを使う（scripts/verify-nextest.sh の stub）。
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

# .config/nextest.toml の profile.gate.junit（verify-nextest が突き合わせる）。
junit=target/nextest/gate/junit.xml

bin=${HADO_NEXTEST_BIN:-}
if [ -z "$bin" ]; then
    bin=$(bash scripts/nextest-setup.sh --cache target/nextest-setup) || exit 1
fi

# 同じ木で別々に起動した門番どうしが junit を取り合わないよう、走行を直列にする。
mkdir -p target/nextest
exec 8>target/nextest/gate.lock
if command -v flock >/dev/null 2>&1; then
    flock 8
fi

# 前の走行の junit を、この走行の結果として読まない。
rm -f -- "$junit"
status=0
"$bin" nextest run --offline --no-fail-fast --profile gate 8>&- || status=$?

if [ ! -f "$junit" ]; then
    echo "nextest-full: $junit was not written" >&2
    [ "$status" -ne 0 ] || status=1
    exit "$status"
fi
python3 - "$junit" <<'PY' || status=1
import sys
import xml.etree.ElementTree as ET

for suite in ET.parse(sys.argv[1]).getroot().iter('testsuite'):
    for case in suite.iter('testcase'):
        tags = {child.tag for child in case}
        if tags & {'failure', 'error'}:
            status = 'FAIL'
        elif 'skipped' in tags:
            status = 'SKIP'
        else:
            status = 'PASS'
        print(f"nextest-case\t{status}\t{suite.get('name')}\t{case.get('name')}")
PY
exit "$status"
