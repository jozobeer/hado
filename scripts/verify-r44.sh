#!/usr/bin/env bash
# tier: full
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0
check() {
    local label="$1"
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT
cargo build --offline -q --bin hado || exit 1

# --- 44-a: once / again がキーワードで、reply / capture が名前である ---
r44_lexer() {
    python3 - "$workdir" <<'PY'
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]) / 'lexer'
root.mkdir(parents=True, exist_ok=False)


def check(name, source, expected_code):
    directory = root / name
    directory.mkdir()
    (directory / 'main.hd').write_text(source)
    result = subprocess.run(
        ['cargo', 'run', '--offline', '-q', '--', 'check', str(directory / 'main.hd')],
        capture_output=True)
    if expected_code is None:
        assert result.returncode == 0 and result.stderr == b'', (name, result.returncode, result.stderr)
        return
    assert result.returncode == 1 and result.stdout == b'', (name, result.returncode, result.stdout)
    assert f'error[{expected_code}]'.encode() in result.stderr, (name, result.stderr)


# 新しい綴りが操作の種別として通る。
check('once-declaration',
      'effect Ask\nonce ask: String\nhandler Ask as H\nonce ask => "x"\ndef main = with H -> ask\n', None)
check('again-declaration',
      'effect A Y\nagain y(value: Int): Void\nhandler A Y as H\nagain y value => resume ()\ndef main = with H -> do y 1 -> 0\n', None)
check('again-function-body',
      'effect A Y\nagain y(value: Int): Void\ndef emit x => y x\ndef main =\n  with H ->\n  emit 1\nhandler A Y as H\nagain y value => resume ()\n', None)
# 旧綴りは op_kind の位置で名前として読まれ、文法に合わない。
check('old-reply-is-a-name', 'effect E reply x: Int\ndef main = 0\n', 'E100')
check('old-capture-is-a-name', 'effect E capture x: Int\ndef main = 0\n', 'E100')
# 新しい綴りは予約語なので def の名前にならない。
check('once-is-reserved', 'def once = 1\ndef main = 0\n', 'E100')
check('again-is-reserved', 'def again = 2\ndef main = 0\n', 'E100')
# 旧綴りは普通の名前として使える。
check('old-spellings-are-names', 'def reply = 1\ndef capture = 2\ndef main = reply + capture\n', None)
print('lexer: once/again are keywords, reply/capture are ordinary names')
PY
}

# --- 44-b: 仕様が once / again を綴る ---
r44_spec() {
    python3 - <<'PY'
import re
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert not re.search(r'\breply\b', spec), 'reply remains in docs/spec.md'
assert not re.search(r'\bcapture\b', spec), 'capture remains in docs/spec.md'
needles = ['| `once` |', '| `again` |', 'once ask: String', 'again yield value =>',
           'once log(contents: String): Void {Error}', '`once` `abort` `again`',
           'op_kind         := "once" | "abort" | "again"']
missing = [needle for needle in needles if needle not in spec]
assert not missing, missing
print('spec: the kind table, the examples, the keyword list and op_kind use once/again')
PY
}

# --- 44-c: コーパスの件数・lock・実行 ---
r44_corpus() {
    python3 - <<'PY'
import re
import subprocess
from pathlib import Path

spec = Path('docs/spec.md').read_text()
table = spec.split('## 56. エラーコード', 1)[1]
codes = re.findall(r'^\| (E[1-7]\d\d) \|[^|]*\| (§\d+(?:\.\d+)?|付録[A-Z]) \|', table, flags=re.M)
assert codes, 'missing the error code table'
reject = [path for path in sorted(Path('tests/reject').iterdir()) if path.is_dir()]
assert len(reject) == len(codes), (len(reject), len(codes))
seen = {}
for path in reject:
    text = (path / 'main.hd').read_text()
    match = re.search(r'^// error: (E[1-7]\d\d)$', text, flags=re.M)
    assert match, path
    seen.setdefault(match.group(1), []).append(path.name)
duplicated = {code: names for code, names in seen.items() if len(names) != 1}
assert not duplicated, duplicated
lock = {}
for line in Path('tests/corpus.sha256').read_text().splitlines():
    digest, path = line.split('  ', 1)
    lock[path] = digest
files = []
for root in ('tests/accept', 'tests/reject', 'tests/library'):
    files += [path for path in Path(root).rglob('*') if path.is_file() and path.suffix in ('.hd', '.hdi')]
files += sorted(Path('stdlib').glob('*.hd'))
assert {str(path) for path in files} == set(lock), (len(files), len(lock))
result = subprocess.run(['cargo', 'test', '--offline', '--test', 'corpus'], capture_output=True)
assert result.returncode == 0, result.stdout[-2000:] + result.stderr[-2000:]
assert b'test result: ok' in result.stdout, result.stdout[-2000:]
print(f'corpus: {len(reject)} reject codes match the table, {len(lock)} files locked, accept and reject run green')
PY
}

# --- 44-d: 旧綴りに戻した写しは check を通らない ---
r44_old_spellings() {
    python3 - "$workdir" <<'PY'
import re
import shutil
import subprocess
import sys
from pathlib import Path

root = Path(sys.argv[1]) / 'old-spellings'
root.mkdir(parents=True, exist_ok=False)
mapper = (('once', 'reply'), ('again', 'capture'))


def check(directory):
    return subprocess.run(
        ['cargo', 'run', '--offline', '-q', '--', 'check', str(directory / 'main.hd')],
        capture_output=True)


def mutate(directory):
    for path in directory.rglob('*'):
        if path.is_file() and path.suffix in ('.hd', '.hdi'):
            text = path.read_text()
            for new, old in mapper:
                text = re.sub(rf'\b{new}\b', old, text)
            path.write_text(text)


mutated = 0
skipped = 0
control = False
for case in sorted(Path('tests/accept').iterdir()):
    if not case.is_dir():
        continue
    sources = [path for path in case.rglob('*') if path.is_file() and path.suffix in ('.hd', '.hdi')]
    if not sources:
        continue
    if not any(re.search(r'\b(?:once|again)\b', path.read_text()) for path in sources):
        skipped += 1
        continue
    if not control:
        kept = root / 'control'
        shutil.copytree(case, kept)
        result = check(kept)
        assert result.returncode == 0 and result.stderr == b'', ('control', case.name, result.stderr)
        control = True
    destination = root / case.name
    shutil.copytree(case, destination)
    mutate(destination)
    result = check(destination)
    assert result.returncode != 0, (case.name, result.stdout, result.stderr)
    mutated += 1
assert control, 'no accept case uses once or again'
assert mutated > 0, 'no accept case was mutated'
print(f'old spellings: {mutated} mutated accept cases rejected, {skipped} cases without the words')
PY
}

# --- 44-e: 文法生成器が新しい綴りだけを書く ---
r44_generator() {
    python3 - <<'PY'
from pathlib import Path

source = Path('crates/hado/tests/r9_grammar.rs').read_text()
assert '"once"' in source and '"again"' in source, 'the generator does not spell once/again'
assert '"reply"' not in source, 'the generator still spells reply'
assert '"capture"' not in source, 'the generator still spells capture'
print('grammar generator: once and again only')
PY
}

# --- 44-f: 効果を持つテストの実行 ---
r44_tests() {
    cargo test --offline --no-fail-fast \
        --test r9_grammar --test r23_syntax --test r24_effects --test r25_effects \
        --test r26_effects --test r31_effects --test r37_world
}

check '44-a lexer keywords and ordinary names' r44_lexer
check '44-b spec uses once and again' r44_spec
check '44-c corpus counts, lock and execution' r44_corpus
check '44-d old spellings are rejected' r44_old_spellings
check '44-e grammar generator spells once and again' r44_generator
check '44-f effect test binaries' r44_tests

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
