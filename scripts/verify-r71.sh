#!/usr/bin/env bash
# tier: full
# R71: 束縛した `Option` の中身を破棄して伝播する形。`?` の破棄の書き換え（E504）は
# 戻さないので、同じ入力形（`as _? ->`）を「受理され、書き換えを持たない正準形」と
# して測る。期待値は判定器側で導く。
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

# 検査の入力。旧 E504 のコーパスと同じ形（`as _? ->`）をここで組み立てる
# （ファイル名に依存しない）。
mkdir -p "$work/a"
cat > "$work/a/main.hd" <<'HD'
def f (x: Int): Int Option Pure => if x == 0 => None | Some(x)
def main = f 1 as _? ->
  Some(2)
HD

# 71-a: 受理される（E504 は無い）。
line=$(hado check "$work/a/main.hd" 2>&1 | head -1)
if [ -z "$line" ]; then
  ok "71-a the discarded Option binding is accepted"
else
  ng "71-a the discarded Option binding is accepted" "$line"
fi

# 71-b: check --fix は書き換えない（前後で同じ）。
cp "$work/a/main.hd" "$work/a/before"
hado check --fix "$work/a/main.hd" >/dev/null 2>&1
if cmp -s "$work/a/before" "$work/a/main.hd"; then
  ok "71-b check --fix leaves the canonical spelling alone"
else
  ng "71-b check --fix leaves the canonical spelling alone" "changed"
fi

# 71-c: fmt も書き換えず、冪等。
cp "$work/a/main.hd" "$work/a/once"
hado fmt "$work/a/main.hd" >/dev/null 2>&1
hado fmt "$work/a/main.hd" >/dev/null 2>&1
if cmp -s "$work/a/once" "$work/a/main.hd"; then
  ok "71-c fmt leaves the canonical spelling alone and is idempotent"
else
  ng "71-c fmt leaves the canonical spelling alone and is idempotent" "changed"
fi

# 71-d: 実行。`Some` の腕は中身を捨て、`None` の腕は `None` を伝播する。
mkdir -p "$work/some" "$work/none"
cat > "$work/some/main.hd" <<'HD'
def f (x: Int): Int Option Pure => if x == 0 => None | Some(x)
def main = f 1 as _? ->
  Some(2)
HD
cat > "$work/none/main.hd" <<'HD'
def f (x: Int): Int Option Pure => if x == 0 => None | Some(x)
def main = f 0 as _? ->
  Some(2)
HD
some=$(hado run "$work/some/main.hd" 2>/dev/null | head -1)
none=$(hado run "$work/none/main.hd" 2>/dev/null | head -1)
if [ "$some" = "Some(2)" ] && [ "$none" = "None" ]; then
  ok "71-d Some discards its payload and None propagates"
else
  ng "71-d Some discards its payload and None propagates" "some=$some none=$none"
fi

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
