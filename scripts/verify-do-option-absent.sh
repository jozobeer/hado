#!/usr/bin/env bash
# tier: core
# round: do-option-absent
#
# `Option` の不在を効果 `Absent` に一本化する（D2）。測るのは 3 つ。
#   a. 受理コーパスと stdlib に `?` の伝播の綴り（`as p? ->`）が無い。
#   b. `Option` に対する `->` は E303、`as p? ->` は E100。
#   c. prelude の `Absent` と `Maybe` だけで書いた効果版が、自分の `// expect:`
#      と同じ出力を出し、そのファイル自身は効果も handler も宣言しない。
# 期待値はタグと仕様のエラー表から導き、ファイル名や行を書き込まない。
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
pass=0
total=3
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1 ($2)"; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
hado() { cargo run --offline -q -- "$@"; }

# --- a: `as p? ->` の綴りが受理側に残っていない ---
spelled=$(grep -rnE 'as [^ ]*\? *->' tests/accept stdlib 2>/dev/null || true)
if [ -z "$spelled" ]; then
    ok "a no accept corpus or stdlib file spells \`as p? ->\`"
else
    ng "a no accept corpus or stdlib file spells \`as p? ->\`" "$spelled"
fi

# --- b: Option に対する `->` は E303、`?` は E100 ---
mkdir -p "$work/b1" "$work/b2"
cat > "$work/b1/option_discard.hd" <<'HD'
def find (x: Int): Int Option => Some(x)
def main =
  find 0 ->
  1
HD
cat > "$work/b2/question.hd" <<'HD'
def main = Some(1) as x? -> x
HD
discard_code=$(hado check "$work/b1/option_discard.hd" 2>&1 | sed -n 's/.*error\[\(E[0-9]*\)\].*/\1/p' | head -1)
question_code=$(hado check "$work/b2/question.hd" 2>&1 | sed -n 's/.*error\[\(E[0-9]*\)\].*/\1/p' | head -1)
if [ "$discard_code" = E303 ] && [ "$question_code" = E100 ]; then
    ok "b an Option discard is E303 and \`?\` is E100"
else
    ng "b an Option discard is E303 and \`?\` is E100" "discard=$discard_code question=$question_code"
fi

# --- c: prelude の効果版が expect と同じ出力を出し、自分では宣言しない ---
failures=""
seen=0
for path in tests/accept/*/main.hd; do
    [ -e "$path" ] || continue
    grep -qE '(^|[^A-Za-z_])absent([^A-Za-z_]|$)' "$path" || continue
    seen=$((seen + 1))
    if grep -qE '^[[:space:]]*(effect|handler)[[:space:]]' "$path"; then
        failures="$failures declared:$path"
        continue
    fi
    want=$(sed -n 's|^// expect: ||p' "$path" | head -1)
    got=$(hado run "$path" 2>/dev/null | head -1)
    [ "$got" = "$want" ] || failures="$failures output:$path"
done
if [ "$seen" -ge 1 ] && [ -z "$failures" ]; then
    ok "c the prelude-only absent program runs as its expect tag says"
else
    ng "c the prelude-only absent program runs as its expect tag says" "seen=$seen$failures"
fi

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
