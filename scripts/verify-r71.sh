#!/usr/bin/env bash
# R71: let _? = の破棄を do に書き換える（E504）。期待値は判定器側で導く。
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
pass=0
total=5
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1 ($2)"; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
hado() { cargo run --offline -q -- "$@"; }
cargo build --offline --bin hado >/dev/null 2>&1
bin="${CARGO_TARGET_DIR:-$HOME/.cargo/target}/debug/hado"

# 71-a: check --fix で受理される。
mkdir -p "$work/a" && cp tests/reject/E504/main.hd "$work/a/main.hd"
hado check --fix "$work/a/main.hd" >/dev/null 2>&1
if hado check "$work/a/main.hd" >/dev/null 2>&1; then
  ok "71-a check --fix makes E504 accepted"
else
  ng "71-a check --fix makes E504 accepted" "$(hado check "$work/a/main.hd" 2>&1 | head -1)"
fi

# 71-b: fmt でも受理され、2 回目が同じ（冪等）。
mkdir -p "$work/b" && cp tests/reject/E504/main.hd "$work/b/main.hd"
hado fmt "$work/b/main.hd" >/dev/null 2>&1
hado check "$work/b/main.hd" >/dev/null 2>&1
first=$?
cp "$work/b/main.hd" "$work/b/once.hd"
hado fmt "$work/b/main.hd" >/dev/null 2>&1
if [ "$first" = 0 ] && cmp -s "$work/b/once.hd" "$work/b/main.hd"; then
  ok "71-b fmt reaches accepted and is idempotent"
else
  ng "71-b fmt reaches accepted and is idempotent" "check=$first"
fi

# 71-c: 直した物の値が、手書きの do 版と同じ（期待値は再現から導く）。
python3 - "$work" <<'PY'
import pathlib, sys
w = pathlib.Path(sys.argv[1])
src = pathlib.Path('tests/reject/E504/main.hd').read_text()
plain = src.replace('let _? = ', 'do ')
(w / 'plain').mkdir(parents=True, exist_ok=True)
(w / 'plain' / 'main.hd').write_text(plain)
PY
got=$(cd "$work/a" && "$bin" run main.hd 2>&1)
want=$(cd "$work/plain" && "$bin" run main.hd 2>&1)
if [ -n "$want" ] && [ "$got" = "$want" ]; then
  ok "71-c the fixed file runs like the hand-written do version"
else
  ng "71-c the fixed file runs like the hand-written do version" "want=[$want] got=[$got]"
fi

# 71-d: 対象外は触らない（純粋値の let _ = と、名前のある束縛 let y? =）。
mkdir -p "$work/d" && cp tests/reject/E506/main.hd "$work/d/pure.hd"
cat > "$work/d/named.hd" <<'HD'
// spec: §35
def f x =>
  let y? = Some(x) ->
  Some(1)
def main = f 1
HD
cp "$work/d/pure.hd" "$work/d/pure.before"; cp "$work/d/named.hd" "$work/d/named.before"
hado check --fix "$work/d/pure.hd" >/dev/null 2>&1
hado check --fix "$work/d/named.hd" >/dev/null 2>&1
# R72 made the pure-discard shape (E506) fixable; its behaviour is covered by
# scripts/verify-r72.sh (72-a..72-d), so only the named shape is checked here.
if cmp -s "$work/d/named.before" "$work/d/named.hd"; then
  ok "71-d the excluded shapes are left alone"
else
  ng "71-d the excluded shapes are left alone" "pure/named changed"
fi

# 71-e: 層・系譜・手順書・コーパスのロック。
missing=""
grep -q "r71" scripts/lib/gate-tiers.sh || missing="$missing tier"
grep -q "verify-r71" docs/spec-trace.md || missing="$missing trace"
grep -q "^## 66\. " docs/plan.md || missing="$missing plan"
if [ -n "$(git status --porcelain tests/accept tests/reject stdlib tests/corpus.sha256)" ]; then
  missing="$missing corpus"
fi
if [ -z "$missing" ]; then
  ok "71-e tier, trace, plan, corpus lock"
else
  ng "71-e tier, trace, plan, corpus lock" "missing:$missing"
fi

echo "PASSED: $pass/$total"
