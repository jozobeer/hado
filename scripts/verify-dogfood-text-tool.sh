#!/usr/bin/env bash
# dogfood-text-tool: tools/dogfood-text-tool のテキスト処理ツールが、第56節のエラーコード表と
# tests/reject の // error: タグを突き合わせ、欠け・余り・重複を印字して差分があれば非 0 で
# 終わることを測る。行数・期待値は実ファイルと入力から導き、コードや件数を判定器へ書き込まない。
# 常に成功を返す実装は、入力の変異（拒否ケースを隠す・表に無い // error: を足す・重複させる）で落ちる。
#
# tier: core
# round: dogfood-text-tool
set -uo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

pass=0
total=0
ok() { total=$((total + 1)); pass=$((pass + 1)); echo "OK: $1"; }
ng() { total=$((total + 1)); echo "NG: $1 ($2)"; }

tool=tools/dogfood-text-tool/main.hd
spec=docs/spec.md
reject=tests/reject

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# 道具の起動は 1 回のビルドに寄せる。並列の層では cargo の錠を握らない。
if ! cargo build --offline -q --bin hado; then
    echo "NG: build (cargo build --bin hado)"
    echo "PASSED: 0/1"
    exit 1
fi
hado=target/debug/hado

# 道具を走らせ、標準出力と標準エラーをまとめて out に、終了コードを rc に入れる。
out=
rc=0
run_tool() {
    out=$("$hado" run "$1" -- "$2" "$3" 2>&1)
    rc=$?
}

# 拒否コーパスのディレクトリを名前順に並べる。隠す 1 件も重複させる 2 件もここから選ぶ。
reject_dirs() {
    find "$1" -mindepth 1 -maxdepth 1 -type d | LC_ALL=C sort
}

# そのディレクトリの main.hd の // error: タグの値。ファイル名は見ない。
tag_of_dir() {
    sed -n 's|^// error: *\([^ ]*\).*$|\1|p' "$1/main.hd" | head -1
}

# --- a: ツールの行数は tools/ の実ファイルから導く（下限 100 は issue の完了条件） ---
lines=$(find tools/dogfood-text-tool -name '*.hd' -exec cat {} + | wc -l)
if [ "$lines" -ge 100 ]; then
    ok "a lines: the tool is $lines lines (>= 100)"
else
    ng "a lines" "$lines < 100"
fi

# --- b: 実木では表とコーパスが一致し、差分を出さず 0 で終わる ---
run_tool "$tool" "$spec" "$reject"
if [ "$rc" -eq 0 ] && grep -q '^dogfood-text-tool: ' <<<"$out" && ! grep -qE '^(missing|extra|duplicate|empty):' <<<"$out"; then
    ok "b clean: the code table and tests/reject agree ($(grep '^dogfood-text-tool: ' <<<"$out"))"
else
    ng "b clean" "rc=$rc out=$(tr '\n' ' ' <<<"$out" | cut -c1-200)"
fi

# --- c: 拒否ケースを 1 つ隠すと、そのコードが欠けとして出て非 0 になる ---
mkdir -p "$work/hide"
cp "$spec" "$work/hide/spec.md"
cp -r "$reject" "$work/hide/reject"
hidden=$(reject_dirs "$work/hide/reject" | head -1)
hidden_code=$(tag_of_dir "$hidden")
rm -rf "$hidden"
run_tool "$tool" "$work/hide/spec.md" "$work/hide/reject"
if [ "$rc" -ne 0 ] && grep -qx "missing: $hidden_code" <<<"$out"; then
    ok "c hidden: removing $hidden_code is reported as missing"
else
    ng "c hidden" "rc=$rc want=missing:$hidden_code out=$(tr '\n' ' ' <<<"$out" | cut -c1-200)"
fi

# --- d: 表に無い // error: を 1 つ足すと、そのコードが余りとして出て非 0 になる ---
extra_code=$(python3 - "$spec" <<'PY'
import re, sys
text = open(sys.argv[1], encoding='utf-8').read()
codes = set(re.findall(r'^\| (E\d+) \|', text, re.M))
for n in range(990, 1000):
    if f'E{n}' not in codes:
        print(f'E{n}')
        break
PY
)
mkdir -p "$work/extra"
cp "$spec" "$work/extra/spec.md"
cp -r "$reject" "$work/extra/reject"
extra_file=$(reject_dirs "$work/extra/reject" | head -1)/main.hd
printf '// error: %s\n' "$extra_code" >>"$extra_file"
run_tool "$tool" "$work/extra/spec.md" "$work/extra/reject"
if [ "$rc" -ne 0 ] && grep -qx "extra: $extra_code" <<<"$out"; then
    ok "d extra: adding $extra_code is reported as extra"
else
    ng "d extra" "rc=$rc want=extra:$extra_code out=$(tr '\n' ' ' <<<"$out" | cut -c1-200)"
fi

# --- e: 同じタグの拒否ケースを 2 つにすると、重複として件数付きで出て非 0 になる ---
mkdir -p "$work/dup"
cp "$spec" "$work/dup/spec.md"
cp -r "$reject" "$work/dup/reject"
dup_dirs=$(reject_dirs "$work/dup/reject")
dup_first=$(sed -n 1p <<<"$dup_dirs")
dup_second=$(sed -n 2p <<<"$dup_dirs")
dup_code=$(tag_of_dir "$dup_first")
cp "$dup_first/main.hd" "$dup_second/main.hd"
run_tool "$tool" "$work/dup/spec.md" "$work/dup/reject"
if [ "$rc" -ne 0 ] && grep -qx "duplicate: $dup_code 2" <<<"$out"; then
    ok "e duplicate: two cases with $dup_code are reported as a duplicate"
else
    ng "e duplicate" "rc=$rc want=duplicate:$dup_code 2 out=$(tr '\n' ' ' <<<"$out" | cut -c1-200)"
fi

# --- f: split・trim・Char・Map がツールのソース（コメントを除く）に現れる ---
code_only=$(grep -v '^[[:space:]]*//' "$tool")
missing_apis=
for api in 'String.split' 'String.trim' 'Char.' 'Map.'; do
    grep -qF "$api" <<<"$code_only" || missing_apis="$missing_apis $api"
done
if [ -z "$missing_apis" ]; then
    ok "f api: split, trim, Char and Map appear in the tool source"
else
    ng "f api" "not found:$missing_apis"
fi

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
