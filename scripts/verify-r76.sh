#!/usr/bin/env bash
# R76: 終了コードの体験 —— 失敗は raise が main まで届くことで表し、stderr と 1 を返す（§54）。
# 終了コードは 0 と 1 の二値である（spec.md:1846）。検体は 1 つずつ別ディレクトリに置く。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF76'
import os, subprocess, sys, tempfile
from pathlib import Path

hado = sys.argv[1]
tmp = tempfile.mkdtemp()
passed, total = [], []

CLI = ('def usage = "usage: cli <name>"\n'
       'def message\n'
       '  | [] => raise { message = usage }\n'
       '  | [name, ..._] => name\n'
       'def main =\n'
       '  let args = EnvDefault.args ->\n'
       '  IoDefault.log (message args)\n')
RAISE = 'def main =\n  raise { message = "boom" }\n'


def place(name, source):
    path = os.path.join(tmp, name, 'main.hd')
    os.makedirs(os.path.dirname(path), exist_ok=True)
    Path(path).write_text(source)
    return path


def hado_run(path, args):
    return subprocess.run([hado, 'run', path] + (['--'] + args if args else []),
                          capture_output=True, text=True, timeout=120)


def check(name, fn):
    total.append(name)
    try:
        fn()
        passed.append(name)
        print('OK: %s' % name)
    except Exception as exc:
        print('NG: %s (%s)' % (name, str(exc)[:240]))


def r76_usage_failure():
    path = place('a', CLI)
    assert subprocess.run([hado, 'check', path], capture_output=True).returncode == 0
    r = hado_run(path, [])
    assert r.returncode == 1, ('exit', r.returncode)
    assert r.stdout == '', ('stdout', r.stdout)
    assert 'usage: cli <name>' in r.stderr, ('stderr', r.stderr[:120])


def r76_success():
    path = place('b', CLI)
    r = hado_run(path, ['alice', 'bob'])
    assert r.returncode == 0, ('exit', r.returncode)
    assert r.stdout.strip() == 'alice', ('stdout', r.stdout)
    assert r.stderr == '', ('stderr', r.stderr[:120])


def r76_raise_reaches_main():
    path = place('c', RAISE)
    assert subprocess.run([hado, 'check', path], capture_output=True).returncode == 0
    r = hado_run(path, [])
    assert r.returncode == 1, ('exit', r.returncode)
    assert r.stdout == '', ('stdout', r.stdout)
    assert 'boom' in r.stderr, ('stderr', r.stderr[:120])


def r76_exit_codes_are_the_binary_pair():
    # §54: 0 と 1 の二値。成功と失敗が同じ検体から区別できることを測る。
    path = place('d', CLI)
    codes = {hado_run(path, []).returncode, hado_run(path, ['x']).returncode}
    assert codes == {0, 1}, codes


def r76_docs():
    tiers = Path('scripts/lib/gate-tiers.sh').read_text()
    full = tiers.split('full)', 1)[1].split(';;', 1)[0]
    assert 'r76' in full.split(), 'r76 is not in the full tier'
    assert 'verify-r76' in Path('docs/spec-trace.md').read_text(), 'the trace has no r76 row'
    assert '## 71. ' in Path('docs/plan.md').read_text(), 'the plan has no section 71'
    spec = Path('docs/spec.md').read_text()
    assert '終了コードは 0 と 1 の二値' in spec, 'the spec no longer fixes the exit codes'
    assert subprocess.run(['sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256']).returncode == 0


for name, fn in (
    ('76-a a usage failure is stderr + exit 1', r76_usage_failure),
    ('76-b success is stdout + exit 0', r76_success),
    ('76-c a raise reaching main is stderr + exit 1', r76_raise_reaches_main),
    ('76-d the exit codes are the binary pair of §54', r76_exit_codes_are_the_binary_pair),
    ('76-e spec, trace, tier, corpus lock', r76_docs),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF76
