#!/usr/bin/env bash
# tier: core
# round: do-option-absent
#
# `Option` の不在を効果 `Absent` に一本化する（D2）。測るのは 4 つ。
#   a. 受理コーパスと stdlib に `?` の伝播の綴り（`as p? ->`。pattern は空白を含みうる）
#      が無い。
#   b. `Option` に対する `->` は E303、`as p? ->` は E100。
#   c. prelude の `Absent` と `Maybe` だけで書いた効果版が、自分の `// expect:`
#      と同じ出力を出し、そのファイル自身は効果も handler も宣言しない。
#   d. `main` に残った `Absent` は操作名を stderr に出して exit 1。
# 期待値はタグと仕様のエラー表から導き、ファイル名や行を書き込まない。
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

# --- a: `as p? ->` の綴りが受理側に残っていない ---
# `as` と `?` の間には pattern（`{ value = Box(n) }` など空白を含む）が入る。
# 行コメントと文字列を外してから `?` + `->` を探し、記録用の `?`（`// item: let p? =`）や
# 文字列の中の `?` を綴りと数えない。
spelled=$(python3 - <<'PYA'
from pathlib import Path
import re

def code_only(text):
    """行コメントと文字列リテラルを空白に置き換えた行を返す。"""
    lines = []
    for line in text.splitlines():
        out = []
        i = 0
        in_string = False
        while i < len(line):
            ch = line[i]
            if in_string:
                if ch == "\\" and i + 1 < len(line):
                    i += 2
                    continue
                if ch == '"':
                    in_string = False
                i += 1
                continue
            if ch == '"':
                in_string = True
                out.append(" ")
                i += 1
                continue
            if ch == "/" and i + 1 < len(line) and line[i + 1] == "/":
                break
            out.append(ch)
            i += 1
        lines.append("".join(out))
    return lines

for root in ("tests/accept", "stdlib"):
    for path in sorted(Path(root).rglob("*.hd")):
        for number, line in enumerate(code_only(path.read_text()), 1):
            if re.search(r"\?[ \t]*->", line):
                print(f"{path}:{number}:{line.strip()}")
PYA
)
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

# --- d: `main` に残った `Absent` は操作名を stderr に出して exit 1 ---
mkdir -p "$work/d"
cat > "$work/d/main.hd" <<'HD'
// needs: prelude
def main = absent
HD
hado run "$work/d/main.hd" >"$work/d/out" 2>"$work/d/err"
status=$?
stderr=$(cat "$work/d/err")
if [ "$status" -eq 1 ] && printf '%s\n' "$stderr" | grep -qx 'absent'; then
    ok "d an unhandled absent exits 1 and prints the operation name"
else
    ng "d an unhandled absent exits 1 and prints the operation name" "status=$status stderr=$stderr"
fi

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
