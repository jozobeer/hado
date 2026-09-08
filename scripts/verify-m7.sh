#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
failed=0
passed=0
total=3
if bash scripts/verify-m6.sh --with-prelude; then passed=$((passed + 1)); else failed=1; fi
python3 - <<'PY'
from pathlib import Path
import re
import sys

required = {
    "List": "map filter fold length reverse append sort",
    "Option": "map unwrap_or is_some ok_or",
    "Result": "map map_error is_ok",
    "String": "length concat",
    "Int": "to_string",
    "Float": "to_string",
}
passed = total = 0
for module, members in required.items():
    path = Path("stdlib") / (module.lower() + ".hd")
    source = path.read_text() if path.exists() else ""
    total += 1
    ok = re.search(r"^module " + module + r"$", source, flags=re.M) and re.search(r"^implement " + module + " as " + module + r"$", source, flags=re.M)
    passed += bool(ok)
    print(f"{'OK' if ok else 'NG'}: {module} signature")
    for member in members.split():
        total += 1
        ok = len(re.findall(r"\bdef\s+(?:rec\s+)?(?:[A-Z]\w*\s+|\([A-Z]\w*(?:\s*,\s*[A-Z]\w*)+\)\s+)?" + re.escape(member) + r"\b", source)) >= 2
        passed += ok
        print(f"{'OK' if ok else 'NG'}: {module}.{member} declaration and definition")
print(f"prelude inventory: {passed}/{total}")
sys.exit(0 if passed == total else 1)
PY
if [ "$?" -eq 0 ]; then passed=$((passed + 1)); else failed=1; fi
if cargo test --offline --no-fail-fast; then passed=$((passed + 1)); else failed=1; fi
echo "PASSED: $passed/$total"
exit "$failed"
