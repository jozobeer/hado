#!/usr/bin/env bash
# tier: full
# R72: E506 の書き換え（`docs/rounds/r72.md`）。
# 判定は「E506 が消えた」ではなく「直したファイルが check を通る」まで見る。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF72'
import os, subprocess, sys, tempfile
from pathlib import Path

hado = sys.argv[1]
tmp = tempfile.mkdtemp()
root = os.path.join(tmp, 'root')
os.makedirs(root)
passed = []
total = []


def run(args, cwd=None):
    return subprocess.run(args, capture_output=True, text=True, timeout=90, cwd=cwd)


def write(name, source):
    path = os.path.join(root, name, 'main.hd')
    os.makedirs(os.path.dirname(path), exist_ok=True)
    Path(path).write_text(source)
    return path


def build_and_run(path, name):
    binary = os.path.join(tmp, name)
    built = run([hado, 'build', '-o', binary, path])
    assert built.returncode == 0, (name, built.stderr[:200])
    work = os.path.join(tmp, name + '-work')
    os.makedirs(work, exist_ok=True)
    result = run([binary], cwd=work)
    assert result.returncode == 0, (name, result.returncode, result.stderr[:200])
    return result.stdout


def check(name, fn):
    total.append(name)
    try:
        fn()
        passed.append(name)
        print('OK: %s' % name)
    except Exception as exc:
        print('NG: %s (%s)' % (name, str(exc)[:240]))


# The sample is the corpus fixture itself, so there is one source of truth.
E506 = Path('tests/reject/E506/main.hd').read_text()
# The expectation is derived from the source: the binding goes, the body stays.
PLAIN = 'def f x =>\n  x\ndef main = f 2\n'
# E506 fires for an effectful discard too (measured in R72), so the fix must
# leave this shape alone.
EFFECTFUL = 'def f x =>\n  let _ = IoDefault.log "x" ->\n  x\ndef main = f 1\n'


def r72_fix_accepts():
    path = write('a', E506)
    first = run([hado, 'check', path])
    assert first.returncode == 1 and 'E506' in first.stderr, first.stderr[:200]
    fix = run([hado, 'check', '--fix', path])
    assert fix.returncode == 0, (fix.returncode, fix.stderr[:200])
    assert run([hado, 'check', path]).returncode == 0, 'the fixed file is not accepted'
    assert 'let _' not in Path(path).read_text(), 'the binding did not go away'


def r72_fmt_accepts_and_is_idempotent():
    path = write('b', E506)
    first = run([hado, 'fmt', path])
    assert first.returncode == 0, (first.returncode, first.stderr[:200])
    text = Path(path).read_text()
    assert run([hado, 'check', path]).returncode == 0, 'the formatted file is not accepted'
    second = run([hado, 'fmt', path])
    assert second.stdout == '', ('not idempotent', second.stdout[:120])
    assert Path(path).read_text() == text, 'the second fmt changed the file'


def r72_value_is_unchanged():
    fixed = write('c', E506)
    assert run([hado, 'check', '--fix', fixed]).returncode == 0
    plain = write('d', PLAIN)
    assert run([hado, 'check', plain]).returncode == 0, 'the derived plain version is not accepted'
    assert build_and_run(fixed, 'c-bin') == build_and_run(plain, 'd-bin')


def r72_leaves_effectful_alone():
    path = write('e', EFFECTFUL)
    before = Path(path).read_text()
    result = run([hado, 'check', '--fix', path])
    assert result.returncode == 1, 'the effectful discard must still be rejected'
    assert Path(path).read_text() == before, 'the effectful shape changed'
    assert 'E506' in result.stderr, result.stderr[:200]


for name, fn in (
    ('72-a check --fix makes E506 accepted', r72_fix_accepts),
    ('72-b fmt does the same and is idempotent', r72_fmt_accepts_and_is_idempotent),
    ('72-c the value is unchanged', r72_value_is_unchanged),
    ('72-d the effectful discard is left alone', r72_leaves_effectful_alone),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF72
