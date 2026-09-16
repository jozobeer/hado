#!/usr/bin/env bash
# R67: 受理コーパスを実際に動かす（docs/plan.md 62 節）。
# 検査だけでは「check は通るが実行できない」を見つけられない。受理ファイルの
# ヘッダが宣言する `// expect:`（stdout）と `// expect-error:`（終了とエラー名）を、
# 実際に build して走らせて比べる。件数も名前も判定器には書かない。
set -eu
cd "$(dirname "$0")/.."
cargo build --offline --bin hado >/dev/null 2>&1
target=$(cargo metadata --offline --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$target/debug/hado" <<'PYEOF'
import os, subprocess, sys, tempfile
from pathlib import Path

hado = sys.argv[1]
files = sorted(Path('tests/accept').glob('*/main.hd'))
tmp = tempfile.mkdtemp()
reds, compared, errored, untagged = [], 0, 0, 0


def run(args, timeout=30):
    try:
        return subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return None


def tag(header, name):
    for line in header:
        if line.startswith('// ' + name + ':'):
            return line.split(':', 1)[1].strip()
    return None


for path in files:
    header = path.read_text().splitlines()[:6]
    expect, expect_error = tag(header, 'expect'), tag(header, 'expect-error')
    name = path.parent.name
    check = run([hado, 'check', str(path)])
    if check is None or check.returncode != 0:
        reds.append((name, 'check', (check.stderr.strip()[:90] if check else 'timeout')))
        continue
    binary = os.path.join(tmp, name)
    build = run([hado, 'build', '-o', binary, str(path)])
    if build is None or build.returncode != 0:
        reds.append((name, 'build', (build.stderr.strip()[:90] if build else 'timeout')))
        continue
    result = run([binary])
    if result is None:
        reds.append((name, 'run timeout', ''))
    elif result.returncode < 0:
        reds.append((name, 'run died by signal %d' % -result.returncode, result.stderr.strip()[:90]))
    elif expect_error is not None:
        if result.returncode == 0:
            reds.append((name, 'expected the error %s, exit 0' % expect_error, result.stdout.strip()[:90]))
        elif expect_error not in result.stderr:
            reds.append((name, 'the error is not %s' % expect_error, result.stderr.strip()[:90]))
        else:
            errored += 1
    elif expect is None:
        untagged += 1
    elif result.returncode != 0:
        reds.append((name, 'exit %d' % result.returncode, result.stderr.strip()[:90]))
    elif result.stdout.strip() != expect:
        reds.append((name, 'stdout %r' % result.stdout.strip()[:70], 'expected %r' % expect))
    else:
        compared += 1

print('R67: fixtures=%d compared=%d errored=%d untagged=%d red=%d'
      % (len(files), compared, errored, untagged, len(reds)))
for name, what, detail in reds:
    print('NG: %s: %s | %s' % (name, what, detail))
print('PASSED: %d/1' % (0 if reds else 1))
sys.exit(1 if reds else 0)
PYEOF
