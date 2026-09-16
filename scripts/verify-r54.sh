#!/usr/bin/env bash
# R54 `hado test`（docs/plan.md 50 節）。
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
import subprocess
from pathlib import Path

HADO = None


def run(*args, cwd=None):
    return subprocess.run([HADO, *args], capture_output=True, cwd=cwd)


def project(directory, files):
    path = Path(directory)
    path.mkdir(parents=True, exist_ok=True)
    for name, text in files.items():
        target = path / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text)
    return path
PY

# --- 54-a: 合格するテストが順に走る ---
r54_passing() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

root = project(Path(sys.argv[1]) / 'a', {
    'main.hd': 'def main = 0\n',
    'b_test.hd': 'def main = 22\n',
    'a_test.hd': 'def main = 11\n',
    'c_test.hd': 'def main = 33\n',
})
result = run('test', str(root / 'main.hd'))
assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
expected = (b'11\ntest: a_test.hd ok\n'
            b'22\ntest: b_test.hd ok\n'
            b'33\ntest: c_test.hd ok\n'
            b'test: 3/3 passed\n')
assert result.stdout == expected, result.stdout
assert result.stderr == b'', result.stderr
print('passing: three tests run in path order and the summary is 3/3')
PY
}

# --- 54-b: 落ちても残りを走らせる ---
r54_failing() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

root = project(Path(sys.argv[1]) / 'b', {
    'main.hd': 'def main = 0\n',
    'a_test.hd': 'def main = 11\n',
    'b_test.hd': 'def main = raise { message = "boom" }\n',
    'c_test.hd': 'def main = 33\n',
})
result = run('test', str(root / 'main.hd'))
assert result.returncode == 1, (result.returncode, result.stdout)
expected = (b'11\ntest: a_test.hd ok\n'
            b'test: b_test.hd failed (exit 1)\n'
            b'33\ntest: c_test.hd ok\n'
            b'test: 2/3 passed\n')
assert result.stdout == expected, result.stdout
assert b'boom' in result.stderr, result.stderr
print('failing: the raise is an exit 1, the later test still runs, the summary is 2/3')
PY
}

# --- 54-c: ビルドが通らないテスト ---
r54_build_failure() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

# §50 のとおり根は 1 つのプログラムなので、1 ファイルの誤りは他の
# テストのビルドも落とす。
root = project(Path(sys.argv[1]) / 'c', {
    'main.hd': 'def main = 0\n',
    'a_test.hd': 'def main = 1 + true\n',
    'b_test.hd': 'def main = 33\n',
})
result = run('test', str(root / 'main.hd'))
assert result.returncode == 1, (result.returncode, result.stdout)
expected = (b'test: a_test.hd failed (build)\n'
            b'test: b_test.hd failed (build)\n'
            b'test: 0/2 passed\n')
assert result.stdout == expected, result.stdout
assert b'E305' in result.stderr, result.stderr

# `main` を持たない `*_test.hd` は E206（§52）。
nomain = project(Path(sys.argv[1]) / 'c/nomain', {
    'no_main_test.hd': 'def helper = 0\n',
})
result = run('test', str(nomain / 'no_main_test.hd'))
assert result.returncode == 1, (result.returncode, result.stdout)
assert result.stdout == b'test: no_main_test.hd failed (build)\ntest: 0/1 passed\n', result.stdout
assert b'E206' in result.stderr, result.stderr
print('build failure: a type error and a missing main are reported as build failures')
PY
}

# --- 54-d: テストが 1 本も無い ---
r54_no_tests() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

# control: `_test.hd` で終わらない名前はテストではない。無言で 0 にしない。
root = project(Path(sys.argv[1]) / 'd', {
    'main.hd': 'def main = 0\n',
    'mytest.hd': 'def main = 11\n',
    'main_tests.hd': 'def main = 22\n',
})
result = run('test', str(root / 'main.hd'))
assert result.returncode == 1, (result.returncode, result.stdout)
assert result.stdout == b'', result.stdout
assert b'no *_test.hd' in result.stderr, result.stderr
print('no tests: a name that is not *_test.hd is not a test, and the exit code is 1')
PY
}

# --- 54-e: 選ぶのは根の直下だけ ---
r54_root_only() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

# 下位ディレクトリの `util_test.hd` は根の直下ではないので走らない。
# 名前は `*_test.hd` なので、根の直下の規則が無ければ選ばれてしまう。
root = project(Path(sys.argv[1]) / 'e', {
    'main.hd': 'def main = 0\n',
    'main_test.hd': 'def main = sub::Util.helper 7\n',
    'sub/util_test.hd': 'library\nmodule Util\ndef helper(x: Int): Int Pure\n\nimplement Util as Util\ndef helper x => x\n',
})
result = run('test', str(root / 'main.hd'))
assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
assert result.stdout == b'7\ntest: main_test.hd ok\ntest: 1/1 passed\n', result.stdout
print('root only: a subdirectory *_test.hd is a project member but is not run')
PY
}

# --- 54-f: 仕様と層と lock ---
r54_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
assert '## 58.2. ' in spec, 'no §58.2'
tail = spec.split('## 58. ', 1)[1].split('## 58.1. ', 1)[0]
assert 'hado test <file.hd>' in tail, '§58 does not list test'
section = spec.split('## 58.2. ', 1)[1].split('\n## ', 1)[0]
for word in ('*_test.hd', 'failed (build)', 'passed'):
    assert word in section, f'§58.2 does not define {word}'
tiers = Path('scripts/lib/gate-tiers.sh').read_text()
full = tiers.split('full)', 1)[1].split(';;', 1)[0]
assert 'r54' in full.split(), 'r54 is not in the full tier'
trace = Path('docs/spec-trace.md').read_text()
assert '§58.2' in trace, 'spec-trace has no §58.2 row'
wants = Path('docs/wants.md').read_text()
assert 'R54' in wants, 'wants.md does not record R54'
print('docs: §58.2, the command list, the trace row, the tier and wants')
PY
}

r54_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '54-a passing tests run in path order' r54_passing
check '54-b a failing test does not stop the run' r54_failing
check '54-c build failures are reported as build failures' r54_build_failure
check '54-d no test files is an error' r54_no_tests
check '54-e only the root directory is selected' r54_root_only
check '54-f spec, trace, tier, wants' r54_docs
check '54-f corpus lock' r54_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
