#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
python3 - <<'PY'
import os, pathlib, re, subprocess, tempfile, json
import sys
sys.path.insert(0, "scripts")
from corpus_paths import source_files, diagnostic
P = pathlib.Path
results = {}
def run(*args, timeout=180):
    try:
        return subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired) as error:
        if isinstance(error, subprocess.TimeoutExpired):
            print(f'NG: timeout ({timeout}s): {" ".join(args)}', file=sys.stderr, flush=True)
        return subprocess.CompletedProcess(args, 127, '', str(error))
def report(number, label, ok):
    results[number] = bool(ok)
    print(f'{"OK" if ok else "NG"}: {number} {label}', flush=True)
build = run('cargo', 'build', '--offline', '-p', 'hado', '--bin', 'hado')
metadata = run('cargo', 'metadata', '--offline', '--format-version', '1', '--no-deps')
try:
    hado = str(P(json.loads(metadata.stdout)['target_directory']) / 'debug/hado')
except (ValueError, KeyError):
    hado = 'target/debug/hado'
def check(path, print_type=False):
    return run(hado, 'check', *(['--print-type'] if print_type else []), str(path))
def success(r): return r.returncode == 0 and not r.stdout and not r.stderr
usage = [run(hado), run(hado, 'check')]
report(1, 'cli', build.returncode == 0 and os.access(hado, os.X_OK) and all(r.returncode == 2 and not r.stdout and r.stderr.startswith('usage: hado ') and '<file>' in r.stderr for r in usage))
accept = sorted(P('tests/accept').glob('*/main.hd'))
accepted = 0
for p in accept:
    r = check(p)
    if success(r):
        accepted += 1
        print(f'm3_accept_ok: {p}')
    else: print(r.stderr.strip())
report(2, f'accept-{len(accept)}', bool(accept) and len(accept) == accepted)
codes = ['E201','E203','E204','E301','E302','E303','E304','E601','E602']
reject = [P(f'tests/reject/{code}/main.hd') for code in codes]
anchors = {}
for code,p in zip(codes,reject):
    r = check(p)
    match = diagnostic(r.stderr, p, code)
    if r.returncode == 1 and not r.stdout and match:
        anchors[code] = match
        print(f'm3_reject_ok: {p} {code}')
print(f'm3_reject_ok_n: {len(anchors)}')
report(3, f'reject-{len(codes)}', len(anchors) == len(codes))
p = reject[0]
original = p.read_bytes()
try:
    header = '\n'.join(original.decode().splitlines()[:2])
    p.write_text(header+'\ntype T Nest = | Leaf(T) | Node(T List Nest)\ndef main = 1\n')
    r = check(p)
    report(4, 'swap', 'error[E302]' in r.stderr and 'error[E201]' not in r.stderr)
finally: p.write_bytes(original)
shifted = 0
for code,p in zip(codes,reject):
    if code not in anchors:
        continue
    target, line, col = anchors[code]
    original = target.read_bytes()
    try:
        target.write_bytes(b'\n' + original)
        r = check(p)
        shifted += r.returncode == 1 and not r.stdout and diagnostic(r.stderr, p, code) == (target, line + 1, col)
    finally: target.write_bytes(original)
report(5, 'perturb', shifted == len(codes))
state = 0x543E5EED
def choice(n):
    global state
    state = (state*6364136223846793005+1442695040888963407) & ((1<<64)-1)
    return (state>>32)%n
def expression(kind,depth):
    if not depth: return str(choice(1000)) if kind == 'Int' else ['true','false'][choice(2)]
    if kind == 'Int':
        op = ['+','-','*'][choice(3)]
        return f'({expression(kind,depth-1)} {op} {expression(kind,depth-1)})'
    if choice(2):
        op = ['<','<=','>','>='][choice(4)]
        return f'({expression("Int",depth-1)} {op} {expression("Int",depth-1)})'
    return f'(not {expression("Bool",depth-1)})'
with tempfile.TemporaryDirectory(prefix='hado-m3-') as temp:
    rounds = errors = 0
    for i in range(20):
        ty = ['Int','Bool'][choice(2)]
        p = P(temp)/f'round_{i}'/'main.hd'
        p.parent.mkdir()
        value = expression(ty,3)
        for _ in range(1 + choice(3)):
            # Why not Result: §49 から型が消えており、生成すると E204 になる。
            container = choice(2)
            if container == 0:
                value, ty = f'[{value}]', f'{ty} List'
            else:
                value, ty = f'Some({value})', f'{ty} Option'
        p.write_text(f'def typed(value: {ty}): {ty} => value\ndef main = typed ({value})')
        r = check(p,True)
        rounds += r.returncode == 0 and r.stdout == ty+'\n' and not r.stderr
    report(6, 'type-round-20', rounds == 20)
    for i in range(10):
        p = P(temp)/f'error_{i}'/'main.hd'
        p.parent.mkdir()
        p.write_text(f'def main = ({expression("Int",3)} + {expression("Bool",3)})')
        r = check(p)
        errors += r.returncode != 0 and 'error[E305]' in r.stderr
    report(7, 'outside-corpus', errors == 10)
source = '\n'.join(p.read_text() for p in [*P('crates/hado/src').rglob('*.rs'), P('crates/hado/tests/corpus.rs')])
corpus = [p for folder in ['tests/accept', 'tests/reject', 'tests/library'] for p in source_files(folder)]
lib = P('crates/hado/src/lib.rs').read_text()
types = P('crates/hado/src/types.rs').read_text()
names = P('crates/hado/src/names.rs').read_text()
glue = all(word in lib for word in ['names::','types::']) and all('fn '+word in types for word in ['unify','generalize','occurs'])
glue &= all(word in names.lower() for word in ['type','module','constructor','value'])
glue &= not any('"'+(p.parent.name+'/'+p.name)+'"' in source or str(p) in source for p in corpus)
for folder in ['tests/accept', 'tests/reject', 'tests/library']:
    for case in P(folder).iterdir():
        if case.is_dir():
            glue &= not any('"'+p.relative_to(case).as_posix()+'"' in source
                            for p in source_files(case) if p.parent != case)
for p in corpus:
    glue &= not any(line in source for line in p.read_text().splitlines() if len(line)>=20)
tests = run('cargo','test','--offline','--no-fail-fast','--','--nocapture','--test-threads=1', timeout=600)
log = tests.stdout+tests.stderr
hm = all(re.search(r'test [^\n]*'+name+r'[^\n]* \.\.\. ok',log) for name in ['unify','generalize','occurs','signature_matching'])
report(8, 'glue+unit-hm', glue and hm)
gates = tests.returncode == 0
def script_total(name):
    match = re.search(r'^total=(\d+)', P(f'scripts/verify-{name}.sh').read_text(), flags=re.M)
    return int(match.group(1)) if match else None
for script in ['m0', 'm1-spec', 'm1-corpus', 'm2']:
    total = script_total(script)
    r = run('bash', f'scripts/verify-{script}.sh', timeout=None)
    ok = total is not None and r.returncode == 0 and r.stdout.rstrip().endswith(f'PASSED: {total}/{total}')
    print(f'gate_{script}: {int(ok)}', flush=True)
    gates &= ok
report(9,'gates',gates)
lock_entries = P('tests/corpus.sha256').read_text().splitlines()
lock_paths = [line.split(maxsplit=1)[1].lstrip('*') for line in lock_entries if len(line.split(maxsplit=1)) == 2]
expected_paths = {str(p) for p in corpus} | {str(p) for p in P('stdlib').glob('*.hd')}
lock_check = run('sha256sum', '--strict', '--status', '-c', 'tests/corpus.sha256')
report(10,'corpus-lock', lock_check.returncode == 0 and len(lock_paths) == len(expected_paths) and set(lock_paths) == expected_paths)
def spec_section(number):
    lines, on, heading = [], False, f'# {number}.'
    for line in P('docs/spec.md').read_text().splitlines():
        if line.startswith(heading):
            on = True
        elif on and line.startswith('# ') and not line.startswith(heading):
            break
        if on:
            lines.append(line)
    return '\n'.join(lines)
known = re.search(r'型の((?:\s*`[A-Za-z]+`)+)\s*と効果の\s*`([A-Za-z]+)`', spec_section(49))
known_types = re.findall(r'`([A-Za-z]+)`', known.group(1)) if known else []
known_effect = known.group(2) if known else None
stdlib_files = list(P('stdlib').glob('*.hd'))
stdlib_text = '\n'.join(path.read_text() for path in stdlib_files)
type_pat = r'^type\s+(?:[A-Z]\w*\s+|\([A-Z]\w*(?:\s*,\s*[A-Z]\w*)+\)\s+)?'
types_ok = bool(known_types) and all(re.search(type_pat + re.escape(ty) + r'\s*=', stdlib_text, re.M) for ty in known_types)
effect_ok = bool(known_effect) and any(re.search(r'^effect\s+' + re.escape(known_effect) + r'\b', path.read_text(), re.M) for path in stdlib_files)
no_early_def = not any(re.search(r'\bdef\b', re.split(r'\b(?:module|implement)\b', re.sub(r'//[^\n]*', '', path.read_text()), maxsplit=1)[0]) for path in stdlib_files)
report(11,'stdlib-types', types_ok and effect_ok and no_early_def)
report(12,'extra-def',len(re.findall(r'(?<![A-Za-z_])lower_extra_def_ng: 1(?=\n)', log)) == 1)
passed = sum(results.values())
print(f'PASSED: {passed}/12')
raise SystemExit(0 if passed == 12 else 1)
PY
