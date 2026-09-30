#!/usr/bin/env bash
# tier: full
# R71: `as p? ->` の破棄の書き換え（E504）は、`?` の廃止で消えた。いまは E100 で
# 構文段階に落ち、書き換えを持たない。期待値は判定器側で導く。
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
pass=0
total=4
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1 ($2)"; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
hado() { cargo run --offline -q -- "$@"; }

# 検査の入力。旧 E504 のコーパスと同じ形をここで組み立てる（ファイル名に依存しない）。
mkdir -p "$work/a"
cat > "$work/a/main.hd" <<'HD'
def f x =>
  Some(x) as _? ->
  Some(1)
def main = f 1
HD

# 71-a: E100 になる。
line=$(hado check "$work/a/main.hd" 2>&1 | head -1)
case "$line" in
  *"error[E100]"*) ok "71-a the question mark is E100" ;;
  *) ng "71-a the question mark is E100" "$line" ;;
esac

# 71-b: check --fix は書き換えない（前後で同じ）。
cp "$work/a/main.hd" "$work/a/before"
hado check --fix "$work/a/main.hd" >/dev/null 2>&1
if cmp -s "$work/a/before" "$work/a/main.hd"; then
  ok "71-b check --fix leaves the removed spelling alone"
else
  ng "71-b check --fix leaves the removed spelling alone" "changed"
fi

# 71-c: fmt も書き換えず、冪等。
cp "$work/a/main.hd" "$work/a/once"
hado fmt "$work/a/main.hd" >/dev/null 2>&1
hado fmt "$work/a/main.hd" >/dev/null 2>&1
if cmp -s "$work/a/once" "$work/a/main.hd"; then
  ok "71-c fmt leaves the removed spelling alone and is idempotent"
else
  ng "71-c fmt leaves the removed spelling alone and is idempotent" "changed"
fi

# 71-d: 診断に正準形を出さない（書き換え先が無い）。
if printf '%s' "$line" | grep -q 'canonical form:'; then
  ng "71-d the diagnostic carries no canonical form" "$line"
else
  ok "71-d the diagnostic carries no canonical form"
fi

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
