#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
python3 - <<'PY'
import os, pathlib, re, subprocess, tempfile, json
P = pathlib.Path
results = {}
locked = {p: p.read_bytes() for folder in ['tests/accept', 'tests/reject'] for p in P(folder).glob('*.shin')}
locked[P('crates/shin/tests/corpus.rs')] = P('crates/shin/tests/corpus.rs').read_bytes()
def run(*args):
    try:
        return subprocess.run(args, capture_output=True, text=True, timeout=180)
    except (OSError, subprocess.TimeoutExpired) as error:
        return subprocess.CompletedProcess(args, 127, '', str(error))
def report(number, label, ok):
    results[number] = bool(ok)
    print(f'{"OK" if ok else "NG"}: {number} {label}', flush=True)
build = run('cargo', 'build', '--offline', '-p', 'shin', '--bin', 'shin')
metadata = run('cargo', 'metadata', '--offline', '--format-version', '1', '--no-deps')
try:
    shin = str(P(json.loads(metadata.stdout)['target_directory']) / 'debug/shin')
except (ValueError, KeyError):
    shin = 'target/debug/shin'
def check(path, print_type=False):
    return run(shin, 'check', *(['--print-type'] if print_type else []), str(path))
def success(r): return r.returncode == 0 and not r.stdout and not r.stderr
usage = [run(shin), run(shin, 'check')]
report(1, 'cli', build.returncode == 0 and os.access(shin, os.X_OK) and all(r.returncode == 2 and not r.stdout and r.stderr.startswith('usage: shin ') and '<file>' in r.stderr for r in usage))
accept = [p for p in sorted(P('tests/accept').glob('*.shin')) if not re.search(r'^-- needs: prelude', p.read_text(), re.M)]
accepted = 0
for p in accept:
    r = check(p)
    if success(r):
        accepted += 1
        print(f'm3_accept_ok: {p}')
    else: print(r.stderr.strip())
report(2, 'accept-20', len(accept) == accepted == 20)
codes = ['E201','E203','E204','E301','E302','E303','E304','E601','E602']
reject = [P(f'tests/reject/{code}.shin') for code in codes]
lines = {}
for code,p in zip(codes,reject):
    r = check(p)
    match = re.fullmatch(re.escape(str(p))+r':([1-9][0-9]*):[1-9][0-9]*: error\['+code+r'\]: [^\n]+\n', r.stderr)
    if r.returncode == 1 and not r.stdout and match:
        lines[code] = int(match[1])
        print(f'm3_reject_ok: {p} {code}')
print(f'm3_reject_ok_n: {len(lines)}')
report(3, 'reject-9', len(lines) == 9)
p = reject[0]
original = p.read_bytes()
try:
    header = '\n'.join(original.decode().splitlines()[:2])
    p.write_text(header+'\ntype Nest<T> = Leaf(T) | Node(Nest<List<T>>)\ndef main = 1\n')
    r = check(p)
    report(4, 'swap', 'error[E302]' in r.stderr and 'error[E201]' not in r.stderr)
finally: p.write_bytes(original)
shifted = 0
for code,p in zip(codes,reject):
    original = p.read_bytes()
    try:
        p.write_text(re.sub(r'(^-- error:[^\n]*\n)', r'\1\n', original.decode(), count=1, flags=re.M))
        r = check(p)
        match = re.match(re.escape(str(p))+r':(\d+):',r.stderr)
        shifted += bool(match and code in lines and int(match[1]) == lines[code]+1)
    finally: p.write_bytes(original)
report(5, 'perturb', shifted == 9)
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
with tempfile.TemporaryDirectory(prefix='shin-m3-') as temp:
    rounds = errors = 0
    for i in range(20):
        ty = ['Int','Bool'][choice(2)]
        p = P(temp)/f'round_{i}.shin'
        p.write_text('def main = '+expression(ty,3))
        r = check(p,True)
        rounds += r.returncode == 0 and r.stdout == ty+'\n' and not r.stderr
    report(6, 'type-round-20', rounds == 20)
    for i in range(10):
        p = P(temp)/f'error_{i}.shin'
        p.write_text(f'def main = ({expression("Int",3)} + {expression("Bool",3)})')
        r = check(p)
        errors += r.returncode != 0 and 'error[E305]' in r.stderr
    report(7, 'outside-corpus', errors == 10)
source = '\n'.join(p.read_text() for p in P('crates/shin/src').rglob('*.rs'))
lib = P('crates/shin/src/lib.rs').read_text()
types = P('crates/shin/src/types.rs').read_text()
names = P('crates/shin/src/names.rs').read_text()
glue = all(word in lib for word in ['names::','types::']) and all('fn '+word in types for word in ['unify','generalize','occurs'])
glue &= all(word in names.lower() for word in ['type','module','constructor','value'])
glue &= not any('"'+p.name+'"' in source for p in reject)
for p in accept+reject:
    glue &= not any(line in source for line in p.read_text().splitlines() if len(line)>=20)
tests = run('cargo','test','--offline','--','--nocapture','--test-threads=1')
log = tests.stdout+tests.stderr
hm = all(re.search(r'test [^\n]*'+name+r'[^\n]* \.\.\. ok',log) for name in ['unify','generalize','occurs','signature_matching'])
report(8, 'glue+unit-hm', glue and hm)
gates = tests.returncode == 0
for script,total in [('m0',7),('m1-spec',6),('m1-corpus',7),('m2',8)]:
    r = run('bash',f'scripts/verify-{script}.sh')
    ok = r.returncode == 0 and r.stdout.rstrip().endswith(f'PASSED: {total}/{total}')
    print(f'gate_{script}: {int(ok)}',flush=True)
    gates &= ok
report(9,'gates',gates)
report(10,'corpus-lock',all(p.read_bytes() == content for p, content in locked.items()))
stdlib = [('option','Option'),('result','Result'),('list','List'),('division_error','DivisionError')]
report(11,'stdlib-types',all(P('stdlib/'+file+'.shin').is_file() and 'type '+ty in P('stdlib/'+file+'.shin').read_text() for file,ty in stdlib) and not any(re.search(r'^def ',p.read_text(),re.M) for p in P('stdlib').glob('*.shin')))
report(12,'extra-def',len(re.findall(r'(?<![A-Za-z_])lower_extra_def_ng: 1(?=\n)', log)) == 1)
passed = sum(results.values())
print(f'PASSED: {passed}/12')
raise SystemExit(0 if passed == 12 else 1)
PY
