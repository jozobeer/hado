#!/usr/bin/env bash
# build-json-diagnostics: `build`・`run`・`test` の検査の診断が `--format json` で
# `check --format json` と同じ 1 行になることを測る（§58.4）。既定の 1 行が
# 変わらないことは、同じ入力の `check` の既定の 1 行との一致で対照する。
# 検体はこの門番が作り、期待値は同じ入力の `check` の出力から読む。命令名・
# コード・位置は埋め込まない。読み込みの後で止まる 2 つの段階（冗長の E520・型の
# E305）と、`build` だけが持つ入口の検査（E206）を測る。`check --format json` が
# 空振りしていないこと（診断を JSON で出していること）も見るので、両側が同じ
# 既定の 1 行に戻る退行でも赤になる。
#
# tier: core
# round: build-json-diagnostics
set -uo pipefail
cd "$(dirname "$0")/.."

checks=()
statuses=()

record() {
    checks+=("$1")
    statuses+=("$2")
    printf '%s: %s\n' "$1" "$2"
}

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

# ---------- K1: 4 命令の JSON と既定の 1 行（issue の完了条件） ----------
printf '== K1 ==\n'
if out=$(python3 - "$workdir" <<'PYEOF' 2>&1
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(sys.argv[1])
# 入力はこの門番が作る。コーパスの名前も行も借りない。
REDUNDANT = 'def f (b: Bool) => if b | true => 1 | false => 0\ndef main = f true\n'
TYPED = 'def main = 1 + true\n'
LIBRARY = 'library\n\ndef x = 1\n'

passed = 0
total = 0


def place(name, source, file='main.hd'):
    directory = ROOT / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / file
    path.write_text(source)
    return path


def hado(*args):
    return subprocess.run(
        ['cargo', 'run', '--offline', '-q', '--', *map(str, args)],
        capture_output=True, text=True)


def one_line(text, what):
    lines = text.rstrip('\n').split('\n')
    assert len(lines) == 1 and lines[0], (what, 'stderr must be one line', lines)
    return lines[0]


def check(label, body):
    global passed, total
    total += 1
    try:
        body()
    except (AssertionError, ValueError, KeyError) as error:
        print(f'NG: {label}: {error}')
        return
    passed += 1
    print(f'OK: {label}')


def json_probe(command, path, extra=(), canonical_contains=None):
    """同じ入力の `check --format json` の 1 行を期待値にする。"""
    def body():
        baseline = hado('check', '--format', 'json', path)
        assert baseline.returncode == 1, ('check', baseline.returncode, baseline.stderr)
        expected = one_line(baseline.stderr, 'check')
        report = json.loads(expected)
        # 対照が空振りしないこと: check 自身が診断を JSON で出している。
        assert report['code'] is not None, report
        if canonical_contains is not None:
            assert report['canonical'] is not None, report
            assert canonical_contains in report['canonical'], report
        result = hado(command, '--format', 'json', *extra, path)
        assert result.returncode == 1, (command, result.returncode, result.stderr)
        got = one_line(result.stderr, command)
        assert got == expected, (command, expected, got)
    return body


def default_probe(command, path, extra=()):
    """既定の 1 行が同じ入力の `check` の既定の 1 行と一致する（不変の対照）。"""
    def body():
        baseline = hado('check', path)
        assert baseline.returncode == 1, ('check', baseline.returncode, baseline.stderr)
        expected = one_line(baseline.stderr, 'check')
        assert not expected.startswith('{'), expected
        result = hado(command, *extra, path)
        assert result.returncode == 1, (command, result.returncode, result.stderr)
        got = one_line(result.stderr, command)
        assert got == expected, (command, expected, got)
    return body


def test_probe():
    """`test` は根の `*_test.hd` を入口にする（§58.2）。診断のパスが変わるので、
    同じファイルを `check` した 1 行と比べ、続きの報告も測る。"""
    path = place('broken-tests', REDUNDANT, 'broken_test.hd')
    body = json_probe('test', path, canonical_contains='if b => 1 | 0')

    def run():
        body()
        result = hado('test', '--format', 'json', path)
        assert 'test: broken_test.hd failed (build)' in result.stdout, result.stdout
        assert 'test: 0/1 passed' in result.stdout, result.stdout
    return run


def library_probe(command):
    """`build` だけが持つ入口の検査（E206）。既定の 1 行からコードを読む。"""
    def body():
        path = ROOT / 'lib' / 'lib.hd'
        human = hado(command, path)
        assert human.returncode == 1, (command, human.returncode)
        line = one_line(human.stderr, command)
        assert 'error[' in line, line
        code = line.split('error[', 1)[1].split(']', 1)[0]
        result = hado(command, '--format', 'json', path)
        assert result.returncode == 1, (command, result.returncode, result.stderr)
        report = json.loads(one_line(result.stderr, command))
        assert report['code'] == code, (report, code)
        assert report['message'] in line, (report, line)
    return body


redundant = place('redundant-arms', REDUNDANT)
typed = place('type-mismatch', TYPED)
place('lib', LIBRARY, 'lib.hd')
build_out = ROOT / 'program'

check('build --format json matches check (E520 canonical)',
      json_probe('build', redundant, ('-o', build_out), canonical_contains='if b => 1 | 0'))
check('run --format json matches check (E520 canonical)',
      json_probe('run', redundant, canonical_contains='if b => 1 | 0'))
check('test --format json matches check (E520 canonical)', test_probe())
check('build --format json matches check (E305 type)',
      json_probe('build', typed, ('-o', build_out)))
check('run --format json matches check (E305 type)', json_probe('run', typed))
check('build keeps the default line (E520)', default_probe('build', redundant, ('-o', build_out)))
check('run keeps the default line (E520)', default_probe('run', redundant))
check('test keeps the default line (E520)', default_probe('test', place('plain-tests', REDUNDANT, 'plain_test.hd')))
check('build reports its entry diagnostic as JSON',
      library_probe('build'))
check('run reports its entry diagnostic as JSON',
      library_probe('run'))
print(f'PASSED: {passed}/{total}')
sys.exit(0 if passed == total else 1)
PYEOF
); then
    summary=$(grep -m1 'PASSED:' <<<"$out")
    printf '%s\n' "$out"
    record "K1:4命令の診断(${summary})" PASS
else
    printf '%s\n' "$out" | tail -30
    record "K1:4命令の診断" FAIL
fi

# ---------- K2: 文書の形（round ファイルの実測フェンス） ----------
printf '== K2 ==\n'
if out=$(bash scripts/lint-docs.sh 2>&1); then
    record "K2:lint-docs($(tail -1 <<<"$out"))" PASS
else
    printf '%s\n' "$out" | tail -10
    record "K2:lint-docs" FAIL
fi

# ---------- 集計 ----------
passed=0
for status in "${statuses[@]}"; do
    [ "$status" = PASS ] && passed=$((passed + 1))
done
printf 'PASSED: %d/%d\n' "$passed" "${#checks[@]}"
[ "$passed" -eq "${#checks[@]}" ]
