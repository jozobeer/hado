#!/usr/bin/env bash
# tier: core
set -uo pipefail
cd "$(dirname "$0")/.."

# 全走 1 回のあいだ、同じ内容の実走を 1 度に畳む（docs/plan.md 32 節）。
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh
failed=0
passed=0
total=2
python3 - <<'PY'
from pathlib import Path
import re
import sys

spec = Path("docs/spec.md").read_text().splitlines()
section, on = [], False
for line in spec:
    if line.startswith("## 55."):
        on = True
    elif on and (line.startswith("## ") or line.startswith("# ")) and not line.startswith("## 55."):
        break
    if on:
        section.append(line)
block = re.search(r"関数集合は次とする。\s*```text\n(.*?)```", "\n".join(section), flags=re.S)
required = {}
if block:
    for raw in block.group(1).splitlines():
        line = raw.strip()
        if not line or ":" not in line:
            continue
        names, members = line.split(":", 1)
        member_list = members.split()
        for name in re.split(r"\s*/\s*", names.strip()):
            required[name] = member_list
passed = total = 0
if not required:
    total = 1
    print("NG: prelude inventory: missing function set in §55")
for module, members in required.items():
    path = Path("stdlib") / (module.lower() + ".hd")
    source = path.read_text() if path.exists() else ""
    total += 1
    ok = re.search(r"^module " + module + r"$", source, flags=re.M) and re.search(r"^implement " + module + " as " + module + r"$", source, flags=re.M)
    passed += bool(ok)
    print(f"{'OK' if ok else 'NG'}: {module} signature")
    for member in members:
        total += 1
        ok = len(re.findall(r"\bdef\s+(?:rec\s+)?(?:[A-Z]\w*\s+|\([A-Z]\w*(?:\s*,\s*[A-Z]\w*)+\)\s+)?" + re.escape(member) + r"\b", source)) >= 2
        passed += ok
        print(f"{'OK' if ok else 'NG'}: {module}.{member} declaration and definition")
# §55 の関数集合のブロックに、R43 で足した 9 つの member が載っている（旧 verify-r43 の
# 43-a(1)）。ブロックから名前を落とすと上の走査が静かに縮むので、ここで名前を固定する。
r43_members = {"List": ["head", "nth", "take", "drop"],
               "String": ["join", "trim"],
               "Int": ["abs", "min", "max"]}
for module, names in r43_members.items():
    for name in names:
        total += 1
        ok = name in required.get(module, [])
        passed += bool(ok)
        print(f"{'OK' if ok else 'NG'}: {module}.{name} in the §55 function set")
# §55 の署名と stdlib の署名が一致する（旧 verify-r43 の 43-a をここへ移した）。
section_text = "\n".join(section)
signatures = {
    "stdlib/list.hd": ["def A head (values: A List): A Option Pure",
                       "def A nth (values: A List, index: Int): A Option Pure",
                       "def A take (values: A List, count: Int): A List Pure",
                       "def A drop (values: A List, count: Int): A List Pure"],
    "stdlib/string.hd": ["def join (values: String List, separator: String): String Pure",
                         "def trim (s: String): String Pure"],
    "stdlib/int.hd": ["def abs (value: Int): Int Pure",
                      "def min (left: Int, right: Int): Int Pure",
                      "def max (left: Int, right: Int): Int Pure"],
    "stdlib/io.hd": ["once log(contents: String): Void {Error}"],
}
for path, lines in signatures.items():
    source = Path(path).read_text()
    for line in lines:
        total += 1
        ok = line in source and line in section_text
        passed += bool(ok)
        print(f"{'OK' if ok else 'NG'}: {path} {line}")
print(f"prelude inventory: {passed}/{total}")
sys.exit(0 if passed == total else 1)
PY
if [ "$?" -eq 0 ]; then passed=$((passed + 1)); else failed=1; fi
if cargo_cached test --offline --no-fail-fast; then passed=$((passed + 1)); else failed=1; fi
echo "PASSED: $passed/$total"
exit "$failed"
