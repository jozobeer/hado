#!/usr/bin/env bash
# tier: full
# round: embed-runtime
#
# embed-runtime: `cargo install --path crates/hado` で入れた `hado` が、ビルド先を
# 消した後も build / run できることを測る（docs/rounds/embed-runtime.md）。一時の
# --target-dir でビルドして --root へ入れ、--target-dir を消してから、受理コーパスの
# 各ケースを別のディレクトリへコピーして走らせる。期待値は各ケースの
# `// expect:` / `// expect-error:` から判定器側で読む（docs/corpus.md）。ケース名は
# 埋め込まず、ディスクの実数を数える。0 件と、全件の期待値が空の判定は赤にする。
set -u
cd "$(dirname "$0")/.."

work=$(mktemp -d)
# 途中成果（cranelift を含む debug のビルド）は worktree の ./target に置く。/tmp は
# tmpfs で、全走のあいだ他の作業と RAM と inode を分け合う。ここにビルド先を置くと
# 書き込みが失敗して偽の赤になる（2026-10-10 の全走で実測）。受理ケースを走らせる
# 場所は、チェックアウトの外であることを測るため /tmp のままにする。
mkdir -p target
build=$(mktemp -d "$PWD/target/embed-runtime.XXXXXX")
trap 'rm -rf "$work" "$build"' EXIT

passed=0
total=0
check() {
    local label=$1
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}

# --- 1: 一時の --target-dir でビルドし、--root へ入れる ---
install_from_path() {
    if ! cargo install --offline --debug --path crates/hado --root "$work/root" \
        --target-dir "$build/target" >"$work/install.log" 2>&1; then
        tail -5 "$work/install.log"
        return 1
    fi
    [ -x "$work/root/bin/hado" ]
}
check 'cargo install --path crates/hado into a private root' install_from_path

# --- 2: ビルド先を消す。以後の判定はこの状態で走る ---
remove_build_dir() {
    rm -rf "$build/target" && [ ! -e "$build/target" ]
}
check 'the --target-dir of the install is removed' remove_build_dir

# --- 3, 4: 受理コーパスの各ケースを別のディレクトリで run / build する ---
corpus_judge() {
    HADO="$work/root/bin/hado" WORK="$work/cases" python3 - "$1" <<'PY'
import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

mode = sys.argv[1]
hado = Path(os.environ['HADO'])
work = Path(os.environ['WORK']) / mode
work.mkdir(parents=True)


def expected(text):
    """corpus.rs の expected_output と同じ読み方。"""
    heads = []
    for line in text.splitlines():
        if line.startswith('// expect:'):
            heads.append((0, line[len('// expect:'):]))
        elif line.startswith('// expect-error:'):
            heads.append((1, line[len('// expect-error:'):]))
    assert len(heads) == 1, heads
    status, value = heads[0]
    value = value.strip()
    out = (value + '\n').encode() if value else b''
    return (0, out, b'') if status == 0 else (1, b'', out)


def observe(case):
    common = dict(cwd=case, stdin=subprocess.DEVNULL, capture_output=True)
    if mode == 'run':
        return subprocess.run([hado, 'run', 'main.hd'], **common)
    built = subprocess.run([hado, 'build', '-o', 'program', 'main.hd'], **common)
    if built.returncode != 0:
        return built
    return subprocess.run([case / 'program'], **common)


def judge(src):
    """1 件を走らせ、(期待値が空でないか, 食い違いの説明か None) を返す。"""
    case = work / src.name
    shutil.copytree(src, case)
    status, stdout, stderr = expected((src / 'main.hd').read_text())
    got = observe(case)
    shutil.rmtree(case)
    if (got.returncode, got.stdout, got.stderr) == (status, stdout, stderr):
        return bool(stdout or stderr), None
    return bool(stdout or stderr), (f'{src.name}: status={got.returncode} '
                                    f'stdout={got.stdout[:80]!r} stderr={got.stderr[:200]!r}')


cases = sorted(p for p in Path('tests/accept').iterdir() if (p / 'main.hd').is_file())
assert cases, 'no accept cases on disk'
# ケースは互いに別のディレクトリで走るので並べる。結果はケースの順に集める。
with ThreadPoolExecutor(max_workers=os.cpu_count() or 1) as pool:
    results = list(pool.map(judge, cases))
nonempty = sum(flag for flag, _ in results)
bad = [why for _, why in results if why]
assert nonempty, 'every expectation is empty; a silent binary would pass'
assert not bad, f'{len(bad)} of {len(cases)} failed: ' + ' | '.join(bad[:3])
print(f'{mode}: {len(cases)} accept cases match their headers ({nonempty} expect output)')
PY
}
check 'hado run of every accept case matches its header outside the checkout' corpus_judge run
check 'hado build of every accept case links and the executable matches' corpus_judge build

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
