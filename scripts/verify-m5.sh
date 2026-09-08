#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --offline --bin hado
hado_target_dir=$(cargo metadata --offline --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
python3 - "$hado_target_dir/debug/hado" <<'PY'
from pathlib import Path
import re
import subprocess
import sys
import tempfile

compiler = str(Path(sys.argv[1]).resolve())
passed = total = 0

def report(name, ok, detail=""):
    global passed, total
    total += 1
    passed += bool(ok)
    print(f"{'OK' if ok else 'NG'}: {name}" + (f": {detail}" if not ok else ""))

def dump(path):
    try:
        return subprocess.run([compiler, "dump-mono", str(path)], capture_output=True, text=True, timeout=60)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess([], 124, "", "timeout")

def valid_ir(result):
    if result.returncode != 0 or result.stderr or not result.stdout.startswith("fn main() -> "):
        return False
    if "main printer unavailable" in result.stdout:
        return False
    # Ignore string literals and debug names when inspecting structural fields.
    structure = re.sub(r'"(?:\\.|[^"\\])*"', '""', result.stdout)
    if re.search(r'\b(?:Var\(\d|offsets?:|byte_size:|size:|alignment:)', structure):
        return False
    for call in re.finditer(r'\bTailCall(?:Closure)?\s*\{\s*(\w+):\s*(\w+)', structure):
        if call[1] == "function" and call[2] != "FunctionId":
            return False
        if call[1] not in ("function", "closure"):
            return False
    return True

for path in sorted(Path("tests/accept").glob("*.hd")):
    if re.search(r"^-- needs: prelude\s*$", path.read_text(), re.M):
        continue
    result = dump(path)
    report(path, valid_ir(result), result.stderr.strip() or result.stdout)

for path in sorted(Path("tests/reject").glob("*.hd")):
    expected = re.search(r"^-- error: (E\d+)\s*$", path.read_text(), re.M)
    result = dump(path)
    diagnostic = re.fullmatch(re.escape(str(path)) + r":\d+:\d+: error\[(E\d+)\]: [^\n]+\n", result.stderr)
    report(path, result.returncode == 1 and not result.stdout and expected and diagnostic and expected[1] == diagnostic[1], result.stderr.strip())

with tempfile.TemporaryDirectory(prefix="hado-m5-") as temporary:
    probes = [
        ("equality", "def main = 1 == 2", ['name: "eq_Int"', 'name: "print_Bool"']),
        ("comparison", "def main = compare 1 2", ['name: "compare_Int"', 'name: "print_Int"']),
        ("runtime-call", 'def main = String.length "hello"', ['CallRuntime { function: StringLength']),
        ("direct-tail", "def rec down n => if n == 0 => 42 | down (n - 1)\ndef main = down 3", ['TailCall { function: FunctionId']),
        ("closure-tail", "def apply f x => f x\ndef main = apply (fn x => x + 1) 41", ['TailCallClosure {']),
    ]
    for name, source, required in probes:
        path = Path(temporary) / (name + ".hd")
        path.write_text(source)
        result = dump(path)
        report(name, valid_ir(result) and all(text in result.stdout for text in required), result.stderr.strip() or result.stdout)
print(f"PASSED: {passed}/{total}")
sys.exit(0 if total and passed == total else 1)
PY
