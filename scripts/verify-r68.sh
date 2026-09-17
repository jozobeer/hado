#!/usr/bin/env bash
# tier: full
# R68: E512 の書き換え（`docs/rounds/r68.md`）。
# 判定は「E512 が消えた」ではなく「直したファイルが check を通る」まで見る。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF68'
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


E512 = 'type T = | A | B\ndef f (t: T): T => if t | A => A | y => y\ndef main = f A\n'
PLAIN = 'type T = | A | B\ndef f (t: T): T => t\ndef main = f A\n'


def r68_fix_accepts():
    path = write('a', E512)
    first = run([hado, 'check', path])
    assert first.returncode == 1 and 'E512' in first.stderr, first.stderr[:200]
    fix = run([hado, 'check', '--fix', path])
    assert fix.returncode == 0, (fix.returncode, fix.stderr[:200])
    again = run([hado, 'check', path])
    assert again.returncode == 0, (again.returncode, again.stderr[:200])


def r68_fmt_accepts_and_is_idempotent():
    path = write('b', E512)
    first = run([hado, 'fmt', path])
    assert first.returncode == 0, (first.returncode, first.stderr[:200])
    text = Path(path).read_text()
    assert run([hado, 'check', path]).returncode == 0, 'the formatted file is not accepted'
    second = run([hado, 'fmt', path])
    assert second.stdout == '', ('not idempotent', second.stdout[:120])
    assert Path(path).read_text() == text, 'the second fmt changed the file'


def r68_value_is_unchanged():
    fixed = write('c', E512)
    assert run([hado, 'check', '--fix', fixed]).returncode == 0
    plain = write('d', PLAIN)
    assert run([hado, 'check', plain]).returncode == 0
    assert build_and_run(fixed, 'c-bin') == build_and_run(plain, 'd-bin')


def r68_leaves_alone():
    shapes = {
        'unused': 'type T = | A | B\ndef f (t: T): T => if t | A => A | y => 0\ndef main = f A\n',
        'call': 'type T = | A | B\ndef g (t: T): T => t\ndef f (t: T): T => if g t | A => A | y => y\ndef main = f A\n',
        'comment': 'type T = | A | B\ndef f (t: T): T => if t | A => A | y =>\n  // keep\n  y\ndef main = f A\n',
    }
    for name, source in shapes.items():
        path = write('e-' + name, source)
        before = Path(path).read_text()
        result = run([hado, 'check', '--fix', path])
        assert Path(path).read_text() == before, (name, 'the file changed', result.stdout[:120])


def r68_docs():
    full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], capture_output=True, text=True, check=True).stdout.split()
    assert 'r68' in full, 'r68 is not in the full tier'
    assert 'verify-r68' in Path('docs/spec-trace.md').read_text(), 'the trace has no r68 row'
    assert Path('docs/rounds/r68.md').is_file(), 'docs/rounds/r68.md is missing'
    spec = Path('docs/spec.md').read_text()
    assert 'E512' in spec and '書き換え' in spec, 'the spec does not record the E512 rewrite'
    assert run(['sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256']).returncode == 0


for name, fn in (
    ('68-a check --fix makes E512 accepted', r68_fix_accepts),
    ('68-b fmt does the same and is idempotent', r68_fmt_accepts_and_is_idempotent),
    ('68-c the value is unchanged', r68_value_is_unchanged),
    ('68-d the four shapes are left alone', r68_leaves_alone),
    ('68-e spec, trace, tier, corpus lock', r68_docs),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF68
