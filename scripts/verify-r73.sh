#!/usr/bin/env bash
# tier: full
# R73: E501 の書き換え（`docs/rounds/r73.md`）。
# 判定は「E501 が消えた」ではなく「直したファイルが check を通る」まで見る。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF73'
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
E501 = Path('tests/reject/E501/main.hd').read_text()
# The expectation is derived from the source: the value the binding returns.
value = E501.split('let x = ', 1)[1].split(' ->', 1)[0].strip()
PLAIN = 'def f %s =>\n  %s\ndef main = f 1\n' % (value, value)
# A body that is not the bound name is not E501, so the fix must leave it alone.
OTHER = 'def f v =>\n  let x = v ->\n  x + 1\ndef main = f 1\n'


def r73_fix_accepts():
    path = write('a', E501)
    first = run([hado, 'check', path])
    assert first.returncode == 1 and 'E501' in first.stderr, first.stderr[:200]
    fix = run([hado, 'check', '--fix', path])
    assert fix.returncode == 0, (fix.returncode, fix.stderr[:200])
    assert run([hado, 'check', path]).returncode == 0, 'the fixed file is not accepted'
    assert 'let x =' not in Path(path).read_text(), 'the binding did not go away'


def r73_fmt_accepts_and_is_idempotent():
    path = write('b', E501)
    first = run([hado, 'fmt', path])
    assert first.returncode == 0, (first.returncode, first.stderr[:200])
    text = Path(path).read_text()
    assert run([hado, 'check', path]).returncode == 0, 'the formatted file is not accepted'
    second = run([hado, 'fmt', path])
    assert second.stdout == '', ('not idempotent', second.stdout[:120])
    assert Path(path).read_text() == text, 'the second fmt changed the file'


def r73_value_is_unchanged():
    fixed = write('c', E501)
    assert run([hado, 'check', '--fix', fixed]).returncode == 0
    plain = write('d', PLAIN)
    assert run([hado, 'check', plain]).returncode == 0, 'the derived plain version is not accepted'
    assert build_and_run(fixed, 'c-bin') == build_and_run(plain, 'd-bin')


def r73_leaves_other_bodies_alone():
    path = write('e', OTHER)
    assert run([hado, 'check', path]).returncode == 0, 'the other-body shape is not accepted'
    before = Path(path).read_text()
    run([hado, 'check', '--fix', path])
    assert Path(path).read_text() == before, 'the other-body shape changed'


for name, fn in (
    ('73-a check --fix makes E501 accepted', r73_fix_accepts),
    ('73-b fmt does the same and is idempotent', r73_fmt_accepts_and_is_idempotent),
    ('73-c the value is unchanged', r73_value_is_unchanged),
    ('73-d a different body is left alone', r73_leaves_other_bodies_alone),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF73
