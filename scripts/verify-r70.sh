#!/usr/bin/env bash
# tier: full
# R70: 診断を JSON で読む小さな道具（tools/diagnostics/main.hd）の門番。
# 期待値は判定器が JSON から独立に数えて導く。ファイル名も件数も暗記しない。
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
pass=0
total=5
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1 ($2)"; }

count_json() {
  python3 -c '
import collections, json, sys
counts = collections.Counter()
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        d = json.loads(line)
    except Exception:
        continue
    counts[d["code"]] += 1
for code in sorted(counts):
    print(f"{code} {counts[code]}")
'
}

# 70-a: 道具自身が検査を通る。
if cargo run --offline -q -- check tools/diagnostics/main.hd >/dev/null 2>&1; then
  ok "70-a the tool passes check"
else
  ng "70-a the tool passes check" "$(cargo run --offline -q -- check tools/diagnostics/main.hd 2>&1 | head -2)"
fi

# 70-b/70-c: 拒否コーパスの 2 つの再現で、期待値（JSON から数えたもの）と一致する。
for fixture in tests/reject/E502/main.hd tests/reject/E511/main.hd; do
  name=$(basename "$(dirname "$fixture")")
  json=$(cargo run --offline -q -- check --format json "$fixture" 2>&1)
  want=$(printf '%s\n' "$json" | count_json | sort)
  got=$(printf '%s\n' "$json" | cargo run --offline -q -- run tools/diagnostics/main.hd 2>/dev/null | grep -v '^$' | sort)
  if [ "$want" = "$got" ]; then
    ok "70-$([ "$name" = E502 ] && echo b || echo c) the tool's counts match the JSON for $name"
  else
    ng "70-$([ "$name" = E502 ] && echo b || echo c) the tool's counts match the JSON for $name" "want=[$want] got=[$got]"
  fi
done

# 70-d: 診断が無ければ何も出さない（空行だけ）。
empty=$(printf '' | cargo run --offline -q -- run tools/diagnostics/main.hd 2>/dev/null | grep -v '^$' | wc -l | tr -d ' ')
if [ "$empty" = "0" ]; then
  ok "70-d no diagnostics prints nothing"
else
  ng "70-d no diagnostics prints nothing" "lines=$empty"
fi

# 70-e: 層・系譜・手順書・コーパスのロック。
missing=""
bash scripts/run-gates.sh --print-tier full | grep -qx r70 || missing="$missing tier"
grep -q "verify-r70" docs/spec-trace.md || missing="$missing trace"
[ -f docs/rounds/r70.md ] || missing="$missing plan"
if [ -n "$(git status --porcelain tests/accept tests/reject stdlib tests/corpus.sha256)" ]; then
  missing="$missing corpus"
fi
if [ -z "$missing" ]; then
  ok "70-e tier, trace, plan, corpus lock"
else
  ng "70-e tier, trace, plan, corpus lock" "missing:$missing"
fi

echo "PASSED: $pass/$total"
