#!/usr/bin/env bash
# tier: full
# R81: 型名 Unit を Void に改名する。値の綴り `()` と印字は動かさない。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF81'
import os, re, shutil, subprocess, sys, tempfile
from pathlib import Path

hado = sys.argv[1]
tmp = tempfile.mkdtemp()
passed, total = [], []


def place(root, name, source):
    path = os.path.join(tmp, root, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    Path(path).write_text(source)
    return path


def run(args, cwd=None):
    return subprocess.run([hado] + args, capture_output=True, text=True, timeout=180, cwd=cwd)


def check(name, fn):
    total.append(name)
    try:
        fn()
        passed.append(name)
        print('OK: %s' % name)
    except Exception as exc:
        print('NG: %s (%s)' % (name, str(exc)[:240]))


def r81_type_name():
    path = place('print', 'main.hd', 'def main = ()\n')
    r = run(['check', '--print-type', path])
    assert r.returncode == 0, (r.returncode, r.stderr[:200])
    assert r.stdout.strip() == 'Void', r.stdout
    path = place('annot', 'main.hd', 'def f (x: Void) => x\ndef main = f ()\n')
    r = run(['check', path])
    assert r.returncode == 0, (r.returncode, r.stderr[:200])
    path = place('old', 'main.hd', 'def f (x: Unit) => x\ndef main = f ()\n')
    r = run(['check', path])
    assert r.returncode == 1, (r.returncode, r.stderr[:200])
    assert 'error[E204]' in r.stderr, r.stderr[:200]


def r81_spec():
    spec = Path('docs/spec.md').read_text()
    assert not re.search(r'\bUnit\b', spec), 'old type name remains in docs/spec.md'
    for needle in (
        '\nVoid\n',
        'Void は `()` とする。',
        'Void は i64 の 0 とする。',
        '| Void | 入れ子では `()` |',
        'Void は何も出さない',
        'void            := "()"',
        'bool | void',
        'once log(contents: String): Void {Error}',
    ):
        assert needle in spec, needle
    assert '()`' in spec or '`()`' in spec


def r81_print():
    nested = place('nested', 'main.hd', 'def main = Some(())\n')
    r = run(['run', nested])
    assert r.returncode == 0, (r.returncode, r.stderr[:200])
    assert r.stdout == 'Some(())\n', r.stdout
    top = place('top', 'main.hd', 'def main = ()\n')
    r = run(['run', top])
    assert r.returncode == 0, (r.returncode, r.stderr[:200])
    assert r.stdout == '', repr(r.stdout)


def r81_old_spelling():
    root = Path(tmp) / 'old-spellings'
    root.mkdir()
    mutated = 0
    control = False
    for case in sorted(Path('tests/accept').iterdir()):
        if not case.is_dir():
            continue
        sources = [p for p in case.rglob('*') if p.is_file() and p.suffix in ('.hd', '.hdi')]
        if not sources:
            continue
        if not any(re.search(r'\bVoid\b', p.read_text()) for p in sources):
            continue
        dest = root / case.name
        shutil.copytree(case, dest)
        for path in dest.rglob('*'):
            if path.is_file() and path.suffix in ('.hd', '.hdi'):
                path.write_text(re.sub(r'\bVoid\b', 'Unit', path.read_text()))
        r = run(['check', str(dest / 'main.hd')])
        assert r.returncode != 0, ('alias remains', case.name, r.stdout[:120], r.stderr[:120])
        mutated += 1
        if not control:
            kept = root / 'control'
            shutil.copytree(case, kept)
            r = run(['check', str(kept / 'main.hd')])
            assert r.returncode == 0, ('control failed', case.name, r.stderr[:160])
            control = True
    assert mutated > 0, 'no accept case mentioned Void'
    assert control, 'control copy was not checked'


def r81_docs_lock():
    full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], capture_output=True, text=True, check=True).stdout.split()
    assert 'r81' in full, 'r81 is not in the full tier'
    assert 'verify-r81' in Path('docs/spec-trace.md').read_text(), 'the trace has no r81 row'
    assert Path('docs/rounds/r81.md').is_file(), 'docs/rounds/r81.md is missing'
    coverage = Path('docs/spec-coverage.md').read_text()
    assert '| 2.2 | Void payload |' in coverage, 'coverage still uses the old item name'
    assert subprocess.run(['sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256']).returncode == 0
    ledger = Path('docs/ledger.md').read_text()
    assert re.search(r'\bUnit\b', ledger), 'historical ledger lost the old type name'


for name, fn in (
    ('81-a print-type is Void; :Void accepts; :Unit is E204', r81_type_name),
    ('81-b spec uses Void and keeps ()', r81_spec),
    ('81-c nested print is Some(()); top-level Void prints nothing', r81_print),
    ('81-d restoring the old type name in accept copies fails check', r81_old_spelling),
    ('81-e docs, lock, and the historical ledger', r81_docs_lock),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF81
