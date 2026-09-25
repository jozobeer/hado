#!/usr/bin/env bash
# tier: full
# round: canonical-in-syntax-errors
# 構文と分岐の拒否に正準の綴りを出す（`docs/rounds/canonical-in-syntax-errors.md`）。
# 検体は 1 つずつ別ディレクトリに置く（読み込みは入力根の全 .hd を見るため。§50）。
set -uo pipefail
cd "$(dirname "$0")/.."

cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

python3 - "$target_dir/debug/hado" "$workdir" <<'PYEOF'
import json
import subprocess
import sys
from pathlib import Path

HADO, ROOT = sys.argv[1], Path(sys.argv[2])
DEF_FORM = 'def f(a: T, b: U): R =>'
ARM_FORM = '| p => if | c => a | b'
FIELDS = {'path', 'line', 'col', 'end_line', 'end_col', 'code', 'message',
          'canonical', 'suggestion', 'related'}

SPLIT = 'def rec bump (entries: Entry List) (word: String): Entry List =>\n  entries\n'
NESTED = 'def label n =>\n  if n\n    | 0 => "zero"\n    | _ => if n > 0 => "positive" | "negative"\n'
TARGETS = (('split', SPLIT, 'E100', DEF_FORM), ('nested', NESTED, 'E101', ARM_FORM))

OTHER_E101 = (
    ('fn-arms', Path('tests/reject/E101/main.hd').read_text()),
    ('pattern-in-arm', 'def label n =>\n  if n\n    | 0 => "zero"\n    | m => if m\n'
                       '      | 1 => "one"\n      | default => "many"\n'),
    ('branch-in-column', 'def label n =>\n  if n\n    | 0 => "zero"\n    | m => if\n'
                         '      | m > 0 => if m > 9 => "big" | "small"\n      | "negative"\n'),
)
OTHER_E100 = (('annotation', Path('tests/reject/E100/main.hd').read_text()),)

passed = 0
total = 0
serial = 0


def place(name, source, file='main.hd'):
    global serial
    serial += 1
    directory = ROOT / f'{serial}-{name}'
    directory.mkdir()
    path = directory / file
    path.write_text(source)
    return path


def hado(*args):
    return subprocess.run([HADO, *map(str, args)], capture_output=True, text=True)


def one_json_line(result):
    lines = result.stderr.strip().split('\n')
    assert len(lines) == 1, lines
    return json.loads(lines[0])


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


def human_line():
    for name, source, code, form in TARGETS:
        result = hado('check', place(name, source))
        assert result.returncode == 1, (name, result.returncode)
        assert f'error[{code}]' in result.stderr, (name, result.stderr)
        assert f'; suggestion: {form}' in result.stderr, (name, result.stderr)
        assert 'canonical form:' not in result.stderr, (name, result.stderr)


def json_report(report, name, code, form):
    assert set(report) == FIELDS, (name, sorted(report))
    assert report['code'] == code, (name, report)
    assert report['suggestion'] is not None and form in report['suggestion'], (name, report)
    assert report['canonical'] is None, (name, report)
    assert 'suggestion' not in report['message'], (name, report)


def json_from(command):
    def body():
        for name, source, code, form in TARGETS:
            path = place(name, source)
            extra = ['-o', path.with_suffix('')] if command == 'build' else []
            result = hado(command, '--format', 'json', *extra, path)
            assert result.returncode == 1, (name, command, result.returncode)
            json_report(one_json_line(result), name, code, form)
    return body


def json_from_test():
    # `test` は入口を解析しないので、同じ本文を根の *_test.hd に置く（§58.2）。
    for name, source, code, form in TARGETS:
        path = place(name, source, 'broken_test.hd')
        result = hado('test', '--format', 'json', path)
        assert result.returncode == 1, (name, result.returncode)
        assert 'broken_test.hd failed (build)' in result.stdout, (name, result.stdout)
        json_report(one_json_line(result), name, code, form)


def not_applied():
    for name, source, _, _ in TARGETS:
        for args in (('fmt',), ('check', '--fix')):
            path = place(name, source)
            hado(*args, path)
            assert path.read_text() == source, (name, args, path.read_text())


def rewrite_control():
    source = Path('tests/reject/E520/main.hd').read_text()
    report = one_json_line(hado('check', '--format', 'json', place('e520', source)))
    assert report['code'] == 'E520', report
    assert report['canonical'] is not None and 'if b => 1 | 0' in report['canonical'], report
    assert report['suggestion'] is None, report
    for args in (('fmt',), ('check', '--fix')):
        path = place('e520', source)
        hado(*args, path)
        text = path.read_text()
        assert text != source and 'if b => 1 | 0' in text, (args, text)


def no_suggestion(cases, code):
    def body():
        for name, source in cases:
            report = one_json_line(hado('check', '--format', 'json', place(name, source)))
            assert report['code'] == code, (name, report)
            assert report['suggestion'] is None, (name, report)
            assert report['canonical'] is None, (name, report)
    return body


check('the default line shows the canonical spelling', human_line)
for command in ('check', 'build', 'run'):
    check(f'{command} --format json reports the suggestion', json_from(command))
check('test --format json reports the suggestion', json_from_test)
check('fmt and check --fix do not apply a suggestion', not_applied)
check('E520 keeps a canonical rewrite and no suggestion', rewrite_control)
check('other E101 shapes have no suggestion', no_suggestion(OTHER_E101, 'E101'))
check('other E100 shapes have no suggestion', no_suggestion(OTHER_E100, 'E100'))
print(f'PASSED: {passed}/{total}')
sys.exit(0 if passed == total else 1)
PYEOF
