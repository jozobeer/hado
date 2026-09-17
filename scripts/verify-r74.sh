#!/usr/bin/env bash
# tier: full
# R74: E511 の書き換え（`docs/rounds/r74.md`）。
# 判定は「E511 が消えた」ではなく「直したファイルが check を通る」まで見る。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF74'
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
E511 = Path('tests/reject/E511/main.hd').read_text()
# The expectation is derived from the source: the shared variable moves into the
# arms, so the parameter goes away and every `x == ` prefix is dropped.
definition = None
arms = []
trailer = []
for line in E511.splitlines():
    stripped = line.strip()
    if stripped.startswith('def f '):
        definition = 'def ' + stripped.split()[1]
    elif stripped.startswith('|'):
        arms.append('  | ' + stripped[1:].strip())
    elif stripped.startswith('def '):
        trailer.append(stripped)
variable = arms[0].split('==', 1)[0].split('|', 1)[1].strip()
arms = ['  | ' + arm.split('|', 1)[1].strip().replace(variable + ' == ', '', 1) for arm in arms]
PLAIN = definition + '\n' + '\n'.join(arms) + '\n' + '\n'.join(trailer) + '\n'
# A column without a fallback is not rewritten (literal patterns need one, E402).
NO_FALLBACK = 'def f x\n  | x == 1 => "one"\n  | x == 2 => "two"\ndef main = f 1\n'


def r74_fix_accepts():
    path = write('a', E511)
    first = run([hado, 'check', path])
    assert first.returncode == 1 and 'E511' in first.stderr, first.stderr[:200]
    fix = run([hado, 'check', '--fix', path])
    assert fix.returncode == 0, (fix.returncode, fix.stderr[:200])
    assert run([hado, 'check', path]).returncode == 0, 'the fixed file is not accepted'
    assert 'x ==' not in Path(path).read_text(), 'the equality tests did not go away'


def r74_fmt_accepts_and_is_idempotent():
    path = write('b', E511)
    first = run([hado, 'fmt', path])
    assert first.returncode == 0, (first.returncode, first.stderr[:200])
    text = Path(path).read_text()
    assert run([hado, 'check', path]).returncode == 0, 'the formatted file is not accepted'
    second = run([hado, 'fmt', path])
    assert second.stdout == '', ('not idempotent', second.stdout[:120])
    assert Path(path).read_text() == text, 'the second fmt changed the file'


def r74_value_is_unchanged():
    fixed = write('c', E511)
    assert run([hado, 'check', '--fix', fixed]).returncode == 0
    plain = write('d', PLAIN)
    result = run([hado, 'check', plain])
    assert result.returncode == 0, ('the derived plain version is not accepted', result.stderr[:200])
    assert build_and_run(fixed, 'c-bin') == build_and_run(plain, 'd-bin')


def r74_leaves_no_fallback_alone():
    path = write('e', NO_FALLBACK)
    before = Path(path).read_text()
    run([hado, 'check', '--fix', path])
    assert Path(path).read_text() == before, 'a column without a fallback was rewritten'


def r74_docs():
    full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], capture_output=True, text=True, check=True).stdout.split()
    assert 'r74' in full, 'r74 is not in the full tier'
    assert 'verify-r74' in Path('docs/spec-trace.md').read_text(), 'the trace has no r74 row'
    assert Path('docs/rounds/r74.md').is_file(), 'docs/rounds/r74.md is missing'
    assert 'E511' in Path('docs/spec.md').read_text(), 'the spec does not mention E511'
    assert run(['sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256']).returncode == 0


for name, fn in (
    ('74-a check --fix makes E511 accepted', r74_fix_accepts),
    ('74-b fmt does the same and is idempotent', r74_fmt_accepts_and_is_idempotent),
    ('74-c the value is unchanged', r74_value_is_unchanged),
    ('74-d a column without a fallback is left alone', r74_leaves_no_fallback_alone),
    ('74-e spec, trace, tier, corpus lock', r74_docs),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF74
