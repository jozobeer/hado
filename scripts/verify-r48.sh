#!/usr/bin/env bash
# tier: full
# R48 `hado run` と安全な出力先（docs/plan.md 44 節）。
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
hado="${CARGO_TARGET_DIR:-target}/debug/hado"

cat >"$workdir/harness.py" <<'PY'
import hashlib
import os
import re
import subprocess
from pathlib import Path

HADO = None


def write(root, name, source):
    directory = Path(root) / name
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / 'main.hd'
    path.write_text(source)
    return path


def hado(args, env=None, stdin=None):
    return subprocess.run([HADO, *args], capture_output=True, input=stdin, env=env)


def execute(path, args=(), stdin=None):
    """Run a built executable directly, not through the CLI."""
    return subprocess.run([str(path), *args], capture_output=True, input=stdin)


def temp_env(path):
    """Run with TMPDIR inside the gate so a leftover run directory is visible."""
    path.mkdir(parents=True, exist_ok=True)
    return {**os.environ, 'TMPDIR': str(path)}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def check_code(path, code):
    result = hado(['check', str(path)])
    assert result.returncode == 1, (path, result.returncode, result.stderr.decode())
    match = re.match(rb'[^:]+:(\d+):(\d+): error\[(E\d+)\]', result.stderr)
    assert match and match.group(3).decode() == code, (path, code, result.stderr.decode())


def run_value(root, name, value):
    """`run` a literal main and require the value on stdout with exit code 0."""
    path = write(root, name, f'def main = {value}\n')
    temporary = Path(root) / name / 'tmp'
    result = hado(['run', str(path)], env=temp_env(temporary))
    assert result.returncode == 0, (name, result.returncode, result.stderr.decode())
    assert result.stderr == b'', (name, result.stderr.decode())
    assert result.stdout == f'{value}\n'.encode(), (name, result.stdout)
    # 一時ディレクトリは消え、入力の隣にも実行ファイルは残らない。
    assert list(temporary.iterdir()) == [], (name, list(temporary.iterdir()))
    assert sorted(entry.name for entry in path.parent.iterdir()) == ['main.hd', 'tmp'], name
    return path


def control(root):
    """The comparisons must reject a wrong value and a wrong exit code."""
    path = write(root, 'control-value', 'def main = 1\n')
    result = hado(['run', str(path)], env=temp_env(Path(root) / 'control-value' / 'tmp'))
    try:
        assert result.stdout == b'2\n'
    except AssertionError:
        pass
    else:
        raise AssertionError('run_value accepted a wrong value')
    try:
        check_code(path, 'E999')
    except AssertionError:
        pass
    else:
        raise AssertionError('check_code accepted a wrong code')
PY

# --- 48-a: main の値を出し、入力の隣に何も残さない ---
r48_value() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import control, run_value
from pathlib import Path

root = Path(sys.argv[1]) / 'a'
root.mkdir(parents=True, exist_ok=True)
seed = int(os.environ.get('HADO_SEED', '48'))
rng = random.Random(seed)
control(str(root / 'control'))
for index in range(6):
    run_value(str(root), f'value-{index}', rng.randint(-20, 20))
print(f'run: seed {seed}, 6 values printed, no binary next to the source')
PY
}

# --- 48-b: `--` の後の引数だけが args に見える ---
r48_arguments() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import hado, temp_env, write

root = Path(sys.argv[1]) / 'b'
root.mkdir(parents=True, exist_ok=True)
seed = int(os.environ.get('HADO_SEED', '48'))
rng = random.Random(seed)
for index in range(6):
    count = rng.randint(1, 6)
    arguments = [rng.choice(['a', 'b', 'c', 'x y', '-n', '日本語']) for _ in range(count)]
    path = write(str(root), f'args-{index}', 'def main =\n  let argv = args ->\n  List.length argv\n')
    result = hado(['run', str(path), '--', *arguments], env=temp_env(path.parent / 'tmp'))
    assert result.returncode == 0, (index, result.returncode, result.stderr.decode())
    assert result.stdout == f'{count}\n'.encode(), (index, result.stdout, arguments)
print(f'run: seed {seed}, 6 argument lists seen by args without the program name')
PY
}

# --- 48-c: raise の message と終了コード 1 ---
r48_exit_code() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import hado, temp_env, write

root = Path(sys.argv[1]) / 'c'
root.mkdir(parents=True, exist_ok=True)
seed = int(os.environ.get('HADO_SEED', '48'))
rng = random.Random(seed)
for index in range(4):
    message = f'boom-{rng.randint(0, 999)}'
    path = write(str(root), f'raise-{index}', f'def main = raise {{ message = "{message}" }}\n')
    result = hado(['run', str(path)], env=temp_env(path.parent / 'tmp'))
    assert result.returncode == 1, (index, result.returncode, result.stderr.decode())
    assert result.stderr.strip() == message.encode(), (index, result.stderr)
    assert result.stdout == b'', (index, result.stdout)
print(f'run: seed {seed}, 4 raised messages on stderr with exit code 1')
PY
}

# --- 48-d: 標準入力がそのままプログラムへ渡る ---
r48_stdin() {
    python3 - "$workdir" "$hado" <<'PY'
import os
import random
import string
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import hado, temp_env, write

root = Path(sys.argv[1]) / 'd'
root.mkdir(parents=True, exist_ok=True)
seed = int(os.environ.get('HADO_SEED', '48'))
rng = random.Random(seed)
source = 'def main =\n  let bytes = read_stdin ->\n  Bytes.to_string bytes\n'
for index in range(4):
    line = ''.join(rng.choice(string.ascii_lowercase) for _ in range(rng.randint(1, 20)))
    path = write(str(root), f'stdin-{index}', source)
    result = hado(['run', str(path)], env=temp_env(path.parent / 'tmp'), stdin=line.encode())
    assert result.returncode == 0, (index, result.returncode, result.stderr.decode())
    assert result.stdout == f'{line}\n'.encode(), (index, result.stdout, line)
print(f'run: seed {seed}, 4 standard inputs reached the program')
PY
}

# --- 48-e: 出力先と仕様・層・単体テスト・lock ---
r48_output_path() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import execute, hado, write

root = Path(sys.argv[1]) / 'e'
root.mkdir(parents=True, exist_ok=True)
path = write(str(root), 'output', 'def main = 42\n')
target = root / 'out' / 'prog'
target.parent.mkdir(parents=True, exist_ok=True)
built = hado(['build', '-o', str(target), str(path)])
assert built.returncode == 0, (built.returncode, built.stderr.decode())
assert target.is_file(), 'build -o did not write the requested path'
assert not path.with_suffix('').exists(), 'build -o also wrote next to the source'
result = execute(target)
assert result.stdout == b'42\n', result.stdout
# 既定の出力先は入力の隣のまま。
default = hado(['build', str(path)])
assert default.returncode == 0, default.stderr.decode()
assert path.with_suffix('').is_file(), 'the default build no longer writes next to the source'
print('build: -o writes only the requested path, the default is unchanged')
PY
}

r48_docs() {
    python3 - <<'PY'
from pathlib import Path

spec = Path('docs/spec.md').read_text()
body = spec.split('## 58. ', 1)[1].split('\n# 付録', 1)[0]
for fragment in ['hado run', '-o <path>', '一時ディレクトリ', '終了コード', 'canonical form:']:
    assert fragment in body, fragment
wants = Path('docs/wants.md').read_text()
assert 'R48' in wants, 'wants does not mention R48'
import subprocess
full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], capture_output=True, text=True, check=True).stdout.split()
assert 'r48' in full, 'r48 is not in the full tier'
print('docs: §58, wants points at R48, r48 in the full tier')
PY
}

r48_unit_tests() {
    cargo test --offline -q --test run_cli
}

r48_lock() {
    sha256sum --strict --status -c tests/corpus.sha256
}

check '48-a run prints the main value and leaves no binary' r48_value
check '48-b run forwards the arguments after --' r48_arguments
check '48-c run returns the program exit code' r48_exit_code
check '48-d run passes standard input through' r48_stdin
check '48-e build -o writes only the requested path' r48_output_path
check '48-e spec §58, wants, full tier' r48_docs
check '48-e run_cli tests' r48_unit_tests
check '48-e corpus lock' r48_lock

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
