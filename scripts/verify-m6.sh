#!/usr/bin/env bash
# tier: core
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --offline --bin hado
hado_target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$hado_target_dir/debug/hado" "$@" <<'PY'
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

compiler = str(Path(sys.argv[1]).resolve())
include_prelude = sys.argv[2:] == ["--with-prelude"]
if sys.argv[2:] and not include_prelude:
    sys.exit("usage: verify-m6.sh [--with-prelude]")
passed = total = 0

def verify(name, copied, stdout, stderr=b"", status=0):
    try:
        built = subprocess.run([compiler, "build", str(copied)], capture_output=True, timeout=60)
        if built.returncode:
            print(f"NG: {name}: {built.stderr.decode().strip()}")
            return False
        run = subprocess.run([str(copied.with_suffix(""))], cwd=str(case), stdin=subprocess.DEVNULL,
                             capture_output=True, timeout=60)
        if run.returncode == status and run.stdout == stdout and run.stderr == stderr:
            print(f"OK: {name}")
            return True
        print(f"NG: {name}: status={run.returncode} stdout={run.stdout!r} stderr={run.stderr!r}")
    except subprocess.TimeoutExpired:
        print(f"NG: {name}: timeout")
    return False

with tempfile.TemporaryDirectory(prefix="hado-m6-") as temporary:
    for source in sorted(Path("tests/accept").glob("*/main.hd")):
        text = source.read_text()
        if not include_prelude and re.search(r"^// needs: prelude\s*$", text, re.M):
            continue
        total += 1
        headers = [(line.startswith("// expect-error:"), line.split(":", 1)[1].strip())
                   for line in text.splitlines()
                   if line.startswith(("// expect:", "// expect-error:"))]
        if len(headers) != 1:
            print(f"NG: {source}: expected exactly one output header")
            continue
        is_error, expected = headers[0]
        output = (expected + "\n").encode() if expected else b""
        case = Path(temporary) / source.parent.name
        shutil.copytree(source.parent, case)
        copied = case / source.name
        passed += verify(source, copied, b"" if is_error else output,
                         output if is_error else b"", int(is_error))
print(f"PASSED: {passed}/{total}")
sys.exit(0 if total > 0 and passed == total else 1)
PY
