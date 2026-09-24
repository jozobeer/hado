#!/usr/bin/env bash
# tier: full
# R53 入力根の走査から `.` 始まりを外す（`docs/rounds/r53.md`）。
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

# --- 53-a: 隠しディレクトリを持つ根 ---
r53_hidden_directory() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

root = project(Path(sys.argv[1]) / 'a/git', {
    'main.hd': 'def main = 42\n',
    '.git/config': '[core]\n\trepositoryformatversion = 0\n',
    '.git/HEAD': 'ref: refs/heads/main\n',
    '.git/objects/info/packs': '\n',
})
checked = run('check', str(root / 'main.hd'))
assert checked.returncode == 0, (checked.returncode, checked.stderr)
assert checked.stderr == b'', checked.stderr

built = run('build', str(root / 'main.hd'))
assert built.returncode == 0, (built.returncode, built.stderr)
executable = root / 'main'
assert executable.is_file(), 'build produced no executable'
ran = run('run', str(root / 'main.hd'))
assert ran.returncode == 0, (ran.returncode, ran.stderr)
assert ran.stdout == b'42\n', ran.stdout

# control: `.` 以外の不正な名前は今までどおり拒否する（決定 3、決定 7）。
for name in ('Bad', '1x'):
    refused = project(Path(sys.argv[1]) / f'a/refuse-{name}', {'main.hd': 'def main = 42\n'})
    (refused / name).mkdir(exist_ok=True)
    result = run('check', str(refused / 'main.hd'))
    assert result.returncode == 1, (name, result.returncode)
    message = f'invalid namespace directory: {name}\n'.encode()
    assert message in result.stderr, (name, result.stderr)
print('hidden directory: a root with .git checks and builds; Bad/ and 1x/ are still refused')
PY
}

# --- 53-b: 隠しファイルは読まれない ---
r53_hidden_file() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

broken = 'def broken = 1 + true\n'
hidden = project(Path(sys.argv[1]) / 'b/hidden', {'main.hd': 'def main = 1\n', '.probe.hd': broken})
checked = run('check', str(hidden / 'main.hd'))
assert checked.returncode == 0, (checked.returncode, checked.stderr)
assert checked.stderr == b'', checked.stderr
dry = run('fmt', '--check', str(hidden / 'main.hd'))
assert dry.returncode == 0, (dry.returncode, dry.stdout, dry.stderr)
assert dry.stdout == b'', dry.stdout
assert (hidden / '.probe.hd').read_text() == broken, 'fmt touched the isolated file'

# control: `.` 無しの同じファイルは読まれる（決定 1 が消したのは `.` 始まりだけ）。
visible = project(Path(sys.argv[1]) / 'b/visible', {'main.hd': 'def main = 1\n', 'probe.hd': broken})
checked = run('check', str(visible / 'main.hd'))
assert checked.returncode == 1, checked.returncode
assert b'E305' in checked.stderr, checked.stderr
assert b'probe.hd' in checked.stderr, checked.stderr
print('hidden file: .probe.hd is not read; probe.hd is')
PY
}

# --- 53-c: 隠しディレクトリの中身も読まれない ---
r53_hidden_tree() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

helper = 'library\ndef helper = 1 + true\n'
deep = 'def deep = 1 + true\n'
hidden = project(Path(sys.argv[1]) / 'c/hidden', {
    'main.hd': 'def main = 1\n',
    '.scratch/helper.hd': helper,
    '.scratch/sub/deep.hd': deep,
})
checked = run('check', str(hidden / 'main.hd'))
assert checked.returncode == 0, (checked.returncode, checked.stderr)

# control: 同じ中身を `.` 無しに置くと、どちらも読まれて落ちる。
visible = project(Path(sys.argv[1]) / 'c/visible', {
    'main.hd': 'def main = 1\n',
    'scratch/helper.hd': helper,
})
checked = run('check', str(visible / 'main.hd'))
assert checked.returncode == 1, checked.returncode
assert b'scratch/helper.hd' in checked.stderr, checked.stderr

nested = project(Path(sys.argv[1]) / 'c/nested', {
    'main.hd': 'def main = 1\n',
    'scratch/sub/deep.hd': deep,
})
checked = run('check', str(nested / 'main.hd'))
assert checked.returncode == 1, checked.returncode
assert b'scratch/sub/deep.hd' in checked.stderr, checked.stderr
print('hidden tree: .scratch/ (and its subdirectories) is not read')
PY
}

# --- 53-d: 明示して渡したものは読まれる ---
r53_explicit_entry() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

explicit = project(Path(sys.argv[1]) / 'd/explicit', {
    'main.hd': 'def main = 1\n',
    '.probe.hd': 'def broken = 1 + true\n',
})
checked = run('check', str(explicit / '.probe.hd'))
assert checked.returncode == 1, checked.returncode
assert b'E305' in checked.stderr, checked.stderr
assert b'.probe.hd' in checked.stderr, checked.stderr

# `.` 始まりのディレクトリを根にするときは、その中の普通の名前を読む。
inner = project(Path(sys.argv[1]) / 'd/inner', {
    '.scratch/main.hd': 'def main = inner::Util.helper 3\n',
    '.scratch/inner/util.hd': 'library\nmodule Util\ndef helper(x: Int): Int Pure\n\nimplement Util as Util\ndef helper x => x\n',
})
checked = run('check', str(inner / '.scratch/main.hd'))
assert checked.returncode == 0, (checked.returncode, checked.stderr)
ran = run('run', str(inner / '.scratch/main.hd'))
assert ran.returncode == 0, (ran.returncode, ran.stderr)
assert ran.stdout == b'3\n', ran.stdout
print('explicit entry: a named .probe.hd is read; .scratch/ works as an entry directory')
PY
}

# --- 53-e: 仕様と層と lock ---

# --- 53-f: 整形も隔離先を触らない ---
r53_format_scope() {
    python3 - "$workdir" "$hado" <<'PY'
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import harness
harness.HADO = sys.argv[2]
from harness import project, run

hidden = 'def helper = let y = 1 -> y\n'
root = project(Path(sys.argv[1]) / 'f', {
    'main.hd': 'def main = let x = 7 -> x\n',
    '.scratch/helper.hd': hidden,
})
formatted = run('fmt', str(root / 'main.hd'))
assert formatted.returncode == 0, (formatted.returncode, formatted.stderr)
assert formatted.stdout == b'fmt: main.hd (1 rewrites)\n', formatted.stdout
assert (root / '.scratch/helper.hd').read_text() == hidden, 'fmt wrote the isolated file'

source = 'def main = let x = 7 -> x\n'
fixed = project(Path(sys.argv[1]) / 'f/build', {'main.hd': source, '.scratch/helper.hd': hidden})
built = run('build', '--fix', str(fixed / 'main.hd'))
assert built.returncode == 0, (built.returncode, built.stderr)
assert (fixed / 'main.hd').read_text() != source, 'build --fix did not format the entry'
assert (fixed / '.scratch/helper.hd').read_text() == hidden, 'build --fix wrote the isolated file'
print('format scope: fmt and build --fix leave the isolated tree alone')
PY
}

check '53-a a hidden directory is skipped, Bad and 1x are still refused' r53_hidden_directory
check '53-b a hidden file is not read, a visible one is' r53_hidden_file
check '53-c a hidden tree is not read' r53_hidden_tree
check '53-d an explicitly named hidden file is read' r53_explicit_entry
check '53-f fmt and build --fix leave the isolated tree alone' r53_format_scope

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
