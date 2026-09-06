#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --offline --bin shin
shin_target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$shin_target_dir/debug/shin" <<'PY'
from pathlib import Path
import re
import subprocess
import sys

failed = False
for kind, files in [
    ("reject", sorted(Path("tests/reject").glob("E[45]*.shin"))),
    ("accept", sorted(Path("tests/accept").glob("*.shin"))),
]:
    total = passed = 0
    for path in files:
        source = path.read_text()
        if kind == "accept" and re.search(r"^-- needs: prelude\s*$", source, re.M):
            continue
        total += 1
        result = subprocess.run([sys.argv[1], "check", str(path)], capture_output=True, text=True)
        if kind == "reject":
            expected = re.search(r"^-- error: (E\d+)\s*$", source, re.M)
            diagnostic = re.fullmatch(re.escape(str(path)) + r":\d+:\d+: error\[(E\d+)\]: [^\n]+\n", result.stderr)
            ok = result.returncode == 1 and not result.stdout and expected and diagnostic and expected[1] == diagnostic[1]
        else:
            ok = result.returncode == 0 and not result.stdout and not result.stderr
        if ok:
            passed += 1
        else:
            failed = True
            print(f"FAIL {path}: exit={result.returncode} stdout={result.stdout!r} stderr={result.stderr!r}")
    print(f"{kind}: {passed}/{total}")
    if total == 0:
        failed = True
sys.exit(1 if failed else 0)
PY
