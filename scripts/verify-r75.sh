#!/usr/bin/env bash
# R75: E505（未使用束縛）の書き換え —— 検査器が印字する正準形が受理されること。
# 検体は 1 つずつ別ディレクトリに置く（check は入力ルート配下の全 .hd を見るため）。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF75'
import json, os, subprocess, sys, tempfile
from pathlib import Path

hado = sys.argv[1]
tmp = tempfile.mkdtemp()
passed = []
total = []


def run(args):
    return subprocess.run(args, capture_output=True, text=True, timeout=120)


def place(name, source):
    path = os.path.join(tmp, name, 'main.hd')
    os.makedirs(os.path.dirname(path), exist_ok=True)
    Path(path).write_text(source)
    return path


def diagnostics(path):
    out = run([hado, 'check', '--format', 'json', path])
    # The CLI writes diagnostics to stderr (§58.4).
    return [json.loads(line) for line in out.stderr.splitlines() if line.strip().startswith('{')]


def fix_to_fixpoint(path, limit=4):
    for _ in range(limit):
        text = Path(path).read_text()
        run([hado, 'check', '--fix', path])
        if run([hado, 'check', path]).returncode == 0:
            return True
        if Path(path).read_text() == text:
            break
    return run([hado, 'check', path]).returncode == 0


E515_FLOW = 'def usage = "usage"\ndef message args =>\n  if args\n    | [] => usage\n    | [name, ...rest] => name\ndef main =\n  let args = EnvDefault.args ->\n  IoDefault.log (message args)\n'
E505_ALONE = 'def message\n  | [] => "usage"\n  | [name, ...rest] => name\ndef main = IoDefault.log (message [])\n'
USED_TAIL = 'def first_of\n  | [] => "none"\n  | [y, ..._] => y\ndef message\n  | [] => "usage"\n  | [name, ...rest] => first_of rest\ndef main = IoDefault.log (message ["a"])\n'


def check(name, fn):
    total.append(name)
    try:
        fn()
        passed.append(name)
        print('OK: %s' % name)
    except Exception as exc:
        print('NG: %s (%s)' % (name, str(exc)[:240]))


def r75_e515_flow_reaches_acceptance():
    path = place('a', E515_FLOW)
    codes = [d['code'] for d in diagnostics(path)]
    assert codes and codes[0] == 'E515', codes
    assert fix_to_fixpoint(path), 'the E515 flow did not reach acceptance'
    assert '..._' in Path(path).read_text(), 'the unreferenced spread was not renamed'


def r75_e505_alone_reaches_acceptance():
    path = place('b', E505_ALONE)
    found = diagnostics(path)
    assert found and found[0]['code'] == 'E505', found
    # The diagnostic itself must carry the canonical form; the gate does not assume it.
    assert found[0]['canonical'] == '_', found[0]
    assert fix_to_fixpoint(path), 'E505 alone did not reach acceptance'
    assert '..._' in Path(path).read_text(), 'the unreferenced spread was not renamed'


def r75_leaves_used_bindings_alone():
    path = place('c', USED_TAIL)
    found = diagnostics(path)
    assert [d['code'] for d in found] == ['E505'], found
    # The one reported binding is the unused one; the tail is referenced by the body.
    unused = found[0]['message'].split('binding ')[1].split(' ')[0]
    assert unused != 'rest', 'the referenced tail was reported as unused'
    assert fix_to_fixpoint(path), 'the used-tail file did not reach acceptance'
    after = Path(path).read_text()
    assert '...rest' in after, 'a referenced spread binding was renamed'
    assert '..._ =>' not in after, 'the referenced spread binding was renamed'


def r75_fmt_shares_the_rewrite_and_is_idempotent():
    path = place('d', E515_FLOW)
    first = run([hado, 'fmt', path])
    assert first.returncode == 0, (first.returncode, first.stderr[:200])
    assert run([hado, 'check', path]).returncode == 0, 'the formatted file is not accepted'
    text = Path(path).read_text()
    second = run([hado, 'fmt', path])
    assert second.stdout == '', ('not idempotent', second.stdout[:120])
    assert Path(path).read_text() == text, 'the second fmt changed the file'


def r75_docs():
    tiers = Path('scripts/lib/gate-tiers.sh').read_text()
    full = tiers.split('full)', 1)[1].split(';;', 1)[0]
    assert 'r75' in full.split(), 'r75 is not in the full tier'
    assert 'verify-r75' in Path('docs/spec-trace.md').read_text(), 'the trace has no r75 row'
    assert '## 70. ' in Path('docs/plan.md').read_text(), 'the plan has no section 70'
    assert 'E505' in Path('docs/spec.md').read_text(), 'the spec does not mention E505'
    assert run(['sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256']).returncode == 0


for name, fn in (
    ('75-a the E515 flow reaches acceptance', r75_e515_flow_reaches_acceptance),
    ('75-b E505 alone reaches acceptance and carries its canonical form', r75_e505_alone_reaches_acceptance),
    ('75-c a referenced spread binding is left alone', r75_leaves_used_bindings_alone),
    ('75-d fmt shares the rewrite and is idempotent', r75_fmt_shares_the_rewrite_and_is_idempotent),
    ('75-e spec, trace, tier, corpus lock', r75_docs),
):
    check(name, fn)

print('PASSED: %d/%d' % (len(passed), len(total)))
sys.exit(0 if len(passed) == len(total) else 1)
PYEOF75
