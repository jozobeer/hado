#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --offline --bin shin
shin_target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$shin_target_dir/debug/shin" "$@" <<'PY'
from pathlib import Path
import re
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
        run = subprocess.run([str(copied.with_suffix(""))], capture_output=True, timeout=60)
        if run.returncode == status and run.stdout == stdout and run.stderr == stderr:
            print(f"OK: {name}")
            return True
        print(f"NG: {name}: status={run.returncode} stdout={run.stdout!r} stderr={run.stderr!r}")
    except subprocess.TimeoutExpired:
        print(f"NG: {name}: timeout")
    return False

with tempfile.TemporaryDirectory(prefix="shin-m6-") as temporary:
    for source in sorted(Path("tests/accept").glob("*.shin")):
        text = source.read_text()
        if not include_prelude and re.search(r"^-- needs: prelude\s*$", text, re.M):
            continue
        total += 1
        expected = re.search(r"^-- expect: (.*)$", text, re.M)
        copied = Path(temporary) / source.name
        copied.write_text(text)
        if expected is None:
            print(f"NG: {source}: missing expected stdout")
            continue
        passed += verify(source, copied, (expected[1] + "\n").encode())
    probes = [
        ("result-error", "def main = 7 / 0", b"", b"DivisionByZero\n", 1),
        ("tail-self", "def rec down n => if n == 0 => 42 | down (n - 1)\ndef main = down 100000000", b"42\n", b"", 0),
        ("tail-mutual", "def rec ping n => if n == 0 => 42 | pong (n - 1) 1 2 3 4 5 6 7 8\ndef rec pong n a b c d e f g h => if n == 0 => a + b + c + d + e + f + g + h | ping (n - 1)\ndef main = ping 100000000", b"42\n", b"", 0),
        ("tail-closure", "def rec loop offset n => if n == 0 => offset | let f = fn x => loop offset x -> f (n - 1)\ndef main = loop 42 100000000", b"42\n", b"", 0),
    ]
    for name, text, stdout, stderr, status in probes:
        total += 1
        copied = Path(temporary) / ("probe-" + name + ".shin")
        copied.write_text(text)
        passed += verify(name, copied, stdout, stderr, status)
print(f"PASSED: {passed}/{total}")
sys.exit(0 if total > 0 and passed == total else 1)
PY
