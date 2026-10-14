#!/usr/bin/env bash
# fmt-layout: §58.1 の一括整形が、配置を正準形へ寄せ、コメントと空行を保つことを測る。
# 中核は「印字し直しがコメントを消す／本体の外へ出す」欠陥である（§57）。
#   a. 配置の違う 2 つの綴りが、`fmt` の後に同じバイト列になる。
#   b. 受理コーパス・stdlib・tools の全ファイルが、印字し直しで 1 バイトも変わらない。
#   c. 本体の中のコメントが、印字し直しの後もその定義の中にあり、本体の前に出ない。
#   d. 空行が残る。
#   f. 空の本体を持つ `implement` が印字し直しで増えない（冪等）。
# 期待値は元のファイルから読み、ファイル名や行を書き込まない。
#
# tier: core
# round: fmt-layout
set -uo pipefail
cd "$(dirname "$0")/.."

pass=0
total=5
ok() { echo "OK: $1"; pass=$((pass + 1)); }
ng() { echo "NG: $1 ($2)"; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado_bin="$target_dir/debug/hado"

# --- a. 配置の違う 2 つの綴りが同じバイト列になるか ---
check_a() {
    mkdir -p "$work/flat" "$work/ragged"
    printf 'def size x => if x > 10 => "large" | "small"\ndef main = size 3\n' \
        >"$work/flat/main.hd"
    printf 'def size x =>\n      if x > 10\n  => "large"\n            | "small"\ndef main = size 3\n' \
        >"$work/ragged/main.hd"
    "$hado_bin" fmt "$work/flat/main.hd" >/dev/null 2>&1
    "$hado_bin" fmt "$work/ragged/main.hd" >/dev/null 2>&1
    if cmp -s "$work/flat/main.hd" "$work/ragged/main.hd"; then
        ok "a: 配置の違う 2 つの綴りが同じバイト列になる"
    else
        ng "a: 配置の違う 2 つの綴りが同じバイト列になる" "flat と ragged が違う"
        diff -u "$work/flat/main.hd" "$work/ragged/main.hd" | sed -n '1,10p'
    fi
}

# --- b. コーパス全体が印字し直しで変わらないか ---
check_b() {
    local bad=0 count=0
    for root in tests/accept stdlib tools; do
        [ -d "$root" ] || continue
        local out="$work/out-$(printf '%s' "$root" | tr / _)"
        mkdir -p "$out"
        "$hado_bin" print-postfix "$root" "$out" >/dev/null 2>&1
        while IFS= read -r file; do
            count=$((count + 1))
            local printed="$out/${file#"$root"/}"
            if [ ! -f "$printed" ] || ! cmp -s "$file" "$printed"; then
                bad=$((bad + 1))
                [ "$bad" -le 3 ] && echo "  変わる: $file"
            fi
        done < <(find "$root" -type f \( -name '*.hd' -o -name '*.hdi' \) | LC_ALL=C sort)
    done
    if [ "$count" -eq 0 ]; then
        ng "b: コーパスが印字し直しで変わらない" "測るファイルが無い"
    elif [ "$bad" -eq 0 ]; then
        ok "b: コーパス $count ファイルが印字し直しで変わらない"
    else
        ng "b: コーパスが印字し直しで変わらない" "$count 中 $bad ファイルが変わる"
    fi
}

# --- c. 本体の中のコメントが定義の中に残るか ---
check_c() {
    mkdir -p "$work/inner"
    cat >"$work/inner/main.hd" <<'EOF'
def outer y =>
  // 本体の中の印
  if y > 0 => 1 | 0
def main = outer 1
EOF
    "$hado_bin" fmt "$work/inner/main.hd" >/dev/null 2>&1
    # コメントがその定義の中にあり、かつ本体より前に来るか。
    if python3 - "$work/inner/main.hd" <<'PY'
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text()
body = text[text.index("def outer"):text.index("def main")]
mark = "// 本体の中の印"
inside = mark in body and body.index(mark) < body.index("if y > 0")
sys.exit(0 if inside else 1)
PY
    then
        ok "c: 本体の中のコメントが定義の中に残り、本体の前に来る"
    else
        ng "c: 本体の中のコメントが定義の中に残る" "定義の外へ出た"
        cat "$work/inner/main.hd"
    fi
}

# --- d. 空行が残るか ---
check_d() {
    mkdir -p "$work/blank"
    cat >"$work/blank/main.hd" <<'EOF'
effect Absent
abort absent

handler Absent as Maybe
abort absent => None
return value => Some(value)
EOF
    cp "$work/blank/main.hd" "$work/blank/before.hd"
    "$hado_bin" fmt "$work/blank/main.hd" >/dev/null 2>&1
    if cmp -s "$work/blank/before.hd" "$work/blank/main.hd"; then
        ok "d: 空行が残る"
    else
        ng "d: 空行が残る" "整形で変わった"
        diff -u "$work/blank/before.hd" "$work/blank/main.hd" | sed -n '1,10p'
    fi
}

# --- f. 空の本体を持つ implement が印字し直しで増えないか ---
# 空行をファイルの末尾へ置くと、印字し直すたびに空行が増える。
# `print-postfix` で測る。`fmt` は元のソースと比べるので、この増加を隠す。
check_f() {
    mkdir -p "$work/empty1" "$work/empty2" "$work/empty3"
    printf 'implement Foo as Bar\n\n/implement\n' >"$work/empty1/main.hd"
    "$hado_bin" print-postfix "$work/empty1" "$work/empty2" >/dev/null 2>&1
    "$hado_bin" print-postfix "$work/empty2" "$work/empty3" >/dev/null 2>&1
    if cmp -s "$work/empty2/main.hd" "$work/empty3/main.hd"; then
        ok "f: 空の本体を持つ implement が印字し直しで増えない"
    else
        ng "f: 空の本体を持つ implement が印字し直しで増えない" "2 度目で変わった"
        diff -u "$work/empty2/main.hd" "$work/empty3/main.hd" | sed -n '1,10p'
    fi
}

check_a
check_b
check_c
check_d
check_f

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
