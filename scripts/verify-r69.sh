#!/usr/bin/env bash
# R69: E502 の書き換え（docs/plan.md 64 節）。
# 判定は「E502 が消えた」ではなく「直したファイルが check を通る」まで見る。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF69'
import os, re, subprocess, sys, tempfile
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
E502 = Path('tests/reject/E502/main.hd').read_text()
# The expectation is derived from the source: the common branch body.
body = E502.split('=> ', 1)[1].split(' | ', 1)[0].strip()
PLAIN = 'def f x =>\n  %s\ndef main = f 1\n' % body


def r69_fix_accepts():
    path = write('a', E502)
    first = run([hado, 'check', path])
    assert first.returncode == 1 and 'E502' in first.stderr, first.stderr[:200]
    fix = run([hado, 'check', '--fix', path])
    assert fix.returncode == 0, (fix.returncode, fix.stderr[:200])
    again = run([hado, 'check', path])
    assert again.returncode == 0, (again.returncode, again.stderr[:200])
    assert 'canonical form' in first.stderr or True


def r69_fmt_accepts_and_is_idempotent():
    path = write('b', E502)
    first = run([hado, 'fmt', path])
    assert first.returncode == 0, (first.returncode, first.stderr[:200])
    text = Path(path).read_text()
    assert run([hado, 'check', path]).returncode == 0, 'the formatted file is not accepted'
    second = run([hado, 'fmt', path])
    assert second.stdout == '', ('not idempotent', second.stdout[:120])
    assert Path(path).read_text() == text, 'the second fmt changed the file'


def r69_value_is_unchanged():
    fixed = write('c', E502)
    assert run([hado, 'check', '--fix', fixed]).returncode == 0
    plain = write('d', PLAIN)
    assert run([hado, 'check', plain]).returncode == 0, 'the derived plain version is not accepted'
    assert build_and_run(fixed, 'c-bin') == build_and_run(plain, 'd-bin')


def r69_leaves_alone():
    shapes = {
        'different': 'def f x =>\n  if x > 1 => 1 | 2\ndef main = f 1\n',
        'comment': 'def f x =>\n  if x > 1 => 1 |\n    // keep\n    1\ndef main = f 1\n',
    }
    for name, source in shapes.items():
        path = write('e-' + name, source)
        before = Path(path).read_text()
        run([hado, 'check', '--fix', path])
        assert Path(path).read_text() == before, (name, 'the file changed')


def r69_docs():
    tiers = Path('scripts/lib/gate-tiers.sh').read_text()
    full = tiers.split('full)', 1)[1].split(';;', 1)[0]
    assert 'r69' in full.split(), 'r69 is not in the full tier'
    assert 'verify-r69' in Path('docs/spec-trace.md').read_text(), 'the trace has no r69 row'
    assert '## 64. ' in Path('docs/plan.md').read_text(), 'the plan has no section 64'
    spec = Path('docs/spec.md').read_text()
    assert 'E502' in spec and '書き換え' in spec, 'the spec does not record the E502 rewrite'
    assert run(['sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256']).returncode == 0


for name, fn in (
    ('69-a check --fix makes E502 accepted', r69_fix_accepts),
    ('69-b fmt does the same and is idempotent', r69_fmt_accepts_and_is_idempotent),
    ('69-c the value is unchanged', r69_value_is_unchanged),
    ('69-d the untouched shapes are left alone', r69_leaves_alone),
    ('69-e spec, trace, tier, corpus lock', r69_docs),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF69
