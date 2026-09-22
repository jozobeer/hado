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

def r81_print():
    nested = place('nested', 'main.hd', 'def main = Some(())\n')
    r = run(['run', nested])
    assert r.returncode == 0, (r.returncode, r.stderr[:200])
    assert r.stdout == 'Some(())\n', r.stdout
    top = place('top', 'main.hd', 'def main = ()\n')
    r = run(['run', top])
    assert r.returncode == 0, (r.returncode, r.stderr[:200])
    assert r.stdout == '', repr(r.stdout)

for name, fn in (
    ('81-a print-type is Void; :Void accepts; :Unit is E204', r81_type_name),
    ('81-c nested print is Some(()); top-level Void prints nothing', r81_print),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF81
