#!/usr/bin/env bash
# R52 `hado fmt` と `build --fix`（docs/plan.md 48 節）。
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
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado="$target_dir/debug/hado"

cat >"$workdir/harness.py" <<'PY'
import re
import shutil
import subprocess
from pathlib import Path

HADO = None


def run(*args, cwd=None):
    return subprocess.run([HADO, *args], capture_output=True, cwd=cwd)


def header_code(path):
    match = re.search(r'^// error: (E[1-7]\d\d)$', Path(path).read_text(), re.M)
    return match.group(1) if match else None


def project(directory, files):
    path = Path(directory)
    path.mkdir(parents=True, exist_ok=True)
    for name, text in files.items():
        target = path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
    return path


def formatted_line(path, rewrites=None):
    pattern = rf'^fmt: {re.escape(path)} \(\d+ rewrites\)$'
    if rewrites is not None:
        pattern = rf'^fmt: {re.escape(path)} \({rewrites} rewrites\)$'
    return re.compile(pattern, re.M)


def control(root):
    """A canonical file must not look like it would change."""
    directory = project(Path(root) / 'control', {'main.hd': 'def main = 0\n'})
    result = run('fmt', '--check', str(directory / 'main.hd'))
    assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
    assert result.stdout == b'', result.stdout
    assert result.stderr == b'', result.stderr
PY

# --- 52-a: check --fix と同じ結果になる ---
r52_same_as_check_fix() {
    python3 - "$workdir" "$hado" <<'PY'
import shutil
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import header_code, run

fixable = {'E501', 'E510', 'E515', 'E516', 'E517'}
chosen = [path for path in sorted(Path('tests/reject').iterdir())
          if path.is_dir() and header_code(path / 'main.hd') in fixable]
assert chosen, 'no fixable reject fixture'
root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
for fixture in chosen:
    with_check = root / f'{fixture.name}-check'
    with_format = root / f'{fixture.name}-fmt'
    shutil.copytree(fixture, with_check)
    shutil.copytree(fixture, with_format)
    before = (with_check / 'main.hd').read_bytes()
    fix = run('check', '--fix', str(with_check / 'main.hd'))
    assert fix.returncode == 0, (fixture.name, fix.returncode, fix.stderr)
    first = run('fmt', str(with_format / 'main.hd'))
    assert first.returncode == 0, (fixture.name, first.returncode, first.stderr)
    assert first.stderr == b'', first.stderr
    assert harness.formatted_line('main.hd').search(first.stdout.decode()), first.stdout
    after = (with_check / 'main.hd').read_bytes()
    formatted = (with_format / 'main.hd').read_bytes()
    assert after != before, fixture.name
    assert formatted == after, (fixture.name, after, formatted)
    for directory in (with_check, with_format):
        checked = run('check', str(directory / 'main.hd'))
        assert checked.returncode == 0, (fixture.name, directory.name, checked.stderr)
print(f'fix parity: {len(chosen)} fixable fixtures match check --fix and become accepted')
PY
}

# --- 52-b: 診断を待たずに書く ---
r52_beyond_diagnostics() {
    python3 - "$workdir" "$hado" <<'PY'
import shutil
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

source = 'def f x => x + 1\ndef main = f true\ndef g = let y = 5 -> y\n'
# 2 つを別の入力根に置く。`fmt` は入力根のソース全部を見るので、同じ根に
# 並べると片方を整形したときにもう片方も直ってしまう。
root = Path(sys.argv[1]) / 'b'
formatted = project(root / 'format', {'main.hd': source})
with_check = project(root / 'check', {'main.hd': source})
fix = run('check', '--fix', str(with_check / 'main.hd'))
assert fix.returncode == 1, (fix.returncode, fix.stderr)
assert (with_check / 'main.hd').read_text() == source, 'check --fix changed a masked file'

format_result = run('fmt', str(formatted / 'main.hd'))
assert format_result.returncode == 0, (format_result.returncode, format_result.stderr)
assert (formatted / 'main.hd').read_text() != source, 'fmt left the masked rewrite alone'
still = run('fmt', '--check', str(with_check / 'main.hd'))
assert still.returncode == 1, (still.returncode, still.stdout)
done = run('fmt', '--check', str(formatted / 'main.hd'))
assert done.returncode == 0, (done.returncode, done.stdout)
checked = run('check', str(formatted / 'main.hd'))
assert checked.returncode == 1, checked.returncode
assert b'E305' in checked.stderr, checked.stderr
print('beyond diagnostics: fmt writes a rewrite that check --fix cannot reach')
PY
}

# --- 52-c: --check と冪等 ---
r52_check_mode() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import control, project, run

root = Path(sys.argv[1]) / 'c'
control(root)
canonical = project(root / 'canonical', {'main.hd': 'def main = 42\n'})
dry = run('fmt', '--check', str(canonical / 'main.hd'))
assert dry.returncode == 0, (dry.returncode, dry.stdout, dry.stderr)
assert dry.stdout == b'', dry.stdout

source = 'def main = let x = 7 -> x\n'
dirty = project(root / 'dirty', {'main.hd': source})
dry = run('fmt', '--check', str(dirty / 'main.hd'))
assert dry.returncode == 1, (dry.returncode, dry.stdout)
assert dry.stdout == b'fmt: main.hd is not canonical (1 rewrites)\n', dry.stdout
assert dry.stderr == b'', dry.stderr
assert (dirty / 'main.hd').read_text() == source, '--check wrote the file'

written = run('fmt', str(dirty / 'main.hd'))
assert written.returncode == 0, (written.returncode, written.stderr)
assert written.stdout == b'fmt: main.hd (1 rewrites)\n', written.stdout
once = (dirty / 'main.hd').read_bytes()
again = run('fmt', str(dirty / 'main.hd'))
assert again.returncode == 0, (again.returncode, again.stderr)
assert again.stdout == b'', again.stdout
assert (dirty / 'main.hd').read_bytes() == once, 'fmt is not idempotent'
quiet = run('fmt', '--check', str(dirty / 'main.hd'))
assert quiet.returncode == 0, (quiet.returncode, quiet.stdout)
print('check mode: --check writes nothing, fmt is idempotent')
PY
}

# --- 52-d: 入力根のソース全部が対象 ---
r52_project_scope() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

root = project(Path(sys.argv[1]) / 'd', {
    'main.hd': 'def main = lib::Util.helper 3\n',
    'lib/util.hd': 'library\nmodule Util\ndef helper(x: Int): Int Pure\n\nimplement Util as Util\ndef helper x => let y = x -> y\n',
})
before = (root / 'lib/util.hd').read_text()
result = run('fmt', str(root / 'main.hd'))
assert result.returncode == 0, (result.returncode, result.stderr)
assert result.stdout == b'fmt: lib/util.hd (1 rewrites)\n', result.stdout
assert (root / 'lib/util.hd').read_text() != before, 'the subdirectory file was not rewritten'
assert (root / 'main.hd').read_text() == 'def main = lib::Util.helper 3\n', 'an already canonical file changed'
checked = run('check', str(root / 'main.hd'))
assert checked.returncode == 0, checked.stderr
print('project scope: the input root is formatted, relative paths are printed')
PY
}

# --- 52-e: build --fix ---
r52_build_fix() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

source = 'def main = let value = 42 -> value\n'
root = Path(sys.argv[1]) / 'e'
plain = project(root / 'plain', {'main.hd': source})
refused = run('build', str(plain / 'main.hd'))
assert refused.returncode == 1, (refused.returncode, refused.stderr)
assert (plain / 'main.hd').read_text() == source, 'build without --fix wrote the file'

fixed = project(root / 'fixed', {'main.hd': source})
built = run('build', '--fix', str(fixed / 'main.hd'))
assert built.returncode == 0, (built.returncode, built.stderr)
assert (fixed / 'main.hd').read_text() != source, 'build --fix did not rewrite the source'
canonical = run('fmt', '--check', str(fixed / 'main.hd'))
assert canonical.returncode == 0, (canonical.returncode, canonical.stdout)
ran = run('run', str(fixed / 'main.hd'))
assert ran.returncode == 0, (ran.returncode, ran.stderr)
assert ran.stdout == b'42\n', ran.stdout
print('build --fix: the source is made canonical before the build and runs')
PY
}

r52_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert '## 58.1. ' in spec, 'no §58.1'
tail = spec.split('## 58. ', 1)[1].split('## 58.1. ', 1)[0]
assert 'hado fmt [--check] <file.hd>' in tail, '§58 does not list fmt'
assert 'hado build [-o <path>] [--fix] <file.hd>' in tail, '§58 does not list build --fix'
section = spec.split('## 58.1. ', 1)[1].split('\n## ', 1)[0]
assert 'hado fmt' in section and '--check' in section, '§58.1 does not define fmt'
tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r52' in full.split(), 'r52 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert '§58.1' in trace, 'spec-trace has no §58.1 row'
wants = Path('docs/wants.md').read_text()
assert 'R52' in wants, 'wants.md does not record R52'
print('docs: §58.1, the command list, the trace row, the tier and wants')
PY
}

r52_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '52-a fmt matches check --fix and becomes accepted' r52_same_as_check_fix
check '52-b fmt writes rewrites that check --fix cannot reach' r52_beyond_diagnostics
check '52-c --check writes nothing and fmt is idempotent' r52_check_mode
check '52-d the whole input root is formatted' r52_project_scope
check '52-e build --fix makes the source canonical' r52_build_fix
check '52-f spec, trace, tier, wants' r52_docs
check '52-f corpus lock' r52_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
