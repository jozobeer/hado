#!/usr/bin/env bash
# nested-application-round-trip: 適用の呼び手が適用のときの括弧（`(f a) b`）が、
# `fmt` と `print-postfix` の往復で保たれ、意味が変わらないことを測る（§13・§57・§58.1）。
#
#   a. `fmt` の前後で `run` の出力（終了コードと stdout/stderr）が同じ。
#   b. `print-postfix` の出力を読み直した `run` が、元のソースの `run` と同じ。
#   c. `fmt` の直後の `fmt --check` が終了コード 0（冪等）。
#
# 検体は `tests/accept/` の `(f a) b` の形（適用の呼び手が括弧で囲まれた適用）を
# ディスクから列挙して数える。ファイル名は書き込まない。検体の `// expect:` との
# 一致は tests/corpus.rs が見る。
#
# tier: core
# round: nested-application-round-trip
set -uo pipefail
cd "$(dirname "$0")/.."

pass=0
total=0
ok() { total=$((total + 1)); pass=$((pass + 1)); echo "OK: $1"; }
ng() { total=$((total + 1)); echo "NG: $1 ($2)"; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

cargo build --offline -q --bin hado || exit 1
target_dir=$(cd "${CARGO_TARGET_DIR:-target}" && pwd)
hado_bin="$target_dir/debug/hado"
mkdir -p "$work/run" "$work/print" "$work/fmt"

# --- 検体の列挙（ディスクから数える） ---
# 適用の呼び手が括弧で囲まれた適用である形を探す。テキストの形だけを見る。
#   1. 対応する `)` を持つ `(` の群を取る。
#   2. その群の中身が適用（空白で区切られた呼び手と引数）である。
#   3. 群が呼び手の位置にあり（直前が式の続きでない）、同じ行の直後に引数が続く。
# 受理コーパスは正準形なので、印字の形をそのまま探せる。
specimens=$(python3 - <<'PY'
from pathlib import Path

OPERATORS = {
    "+", "-", "*", "/", "%", "==", "!=", "<", ">", "<=", ">=", "&&", "||",
    "|>", "=", "|", "=>", "->",
}


def code_lines(path):
    for line in path.read_text().split("\n"):
        out = []
        i = 0
        in_str = False
        while i < len(line):
            c = line[i]
            if in_str:
                if c == "\\" and i + 1 < len(line):
                    i += 2
                    continue
                if c == '"':
                    in_str = False
                i += 1
                continue
            if c == '"':
                in_str = True
                i += 1
                continue
            if c == "/" and i + 1 < len(line) and line[i + 1] == "/":
                break
            out.append(c)
            i += 1
        yield "".join(out)


def match_paren(line, start):
    depth = 0
    i = start
    while i < len(line):
        c = line[i]
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return None


def top_level_tokens(inner):
    tokens = []
    current = ""
    depth = 0
    for c in inner:
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        if depth == 0 and c in " \t":
            if current:
                tokens.append(current)
                current = ""
        else:
            current += c
    if current:
        tokens.append(current)
    return tokens


def is_callee_position(line, start):
    j = start - 1
    while j >= 0 and line[j] in " \t":
        j -= 1
    if j < 0:
        return True
    return line[j] in "=([{,|>"


def has_nested_caller(path):
    for line in code_lines(path):
        i = 0
        while i < len(line):
            if line[i] != "(":
                i += 1
                continue
            j = match_paren(line, i)
            if j is None:
                break
            tokens = top_level_tokens(line[i + 1 : j])
            application = len(tokens) >= 2 and tokens[0] not in OPERATORS
            k = j + 1
            while k < len(line) and line[k] in " \t":
                k += 1
            argument_follows = k < len(line) and (
                line[k].isalnum() or line[k] in '_("[{'
            )
            if application and is_callee_position(line, i) and argument_follows:
                return True
            i += 1
    return False


for path in sorted(Path("tests/accept").rglob("*")):
    if not path.is_file() or path.suffix not in (".hd", ".hdi"):
        continue
    if has_nested_caller(path):
        print(path.parent)
PY
)

# --- run の出力（終了コードと stdout/stderr）を 1 つのファイルへ書く ---
capture_run() {
    local file=$1 out=$2 status
    "$hado_bin" run "$file" >"$out.body" 2>&1
    status=$?
    {
        printf '%s\n' "$status"
        cat "$out.body"
    } >"$out"
    rm -f "$out.body"
}

specimen_count=$(printf '%s\n' "$specimens" | grep -c . || true)

# --- 検体ごとの検査 ---
check_specimen() {
    local case_dir=$1 name=${1#tests/accept/}
    capture_run "$case_dir/main.hd" "$work/run/original"

    rm -rf "$work/fmt/$name"
    cp -R "$case_dir" "$work/fmt/$name"
    "$hado_bin" fmt "$work/fmt/$name/main.hd" >/dev/null 2>&1
    capture_run "$work/fmt/$name/main.hd" "$work/run/formatted"
    if cmp -s "$work/run/original" "$work/run/formatted"; then
        ok "a: $name: fmt の前後で run の出力が同じ"
    else
        ng "a: $name: fmt の前後で run の出力が同じ" "fmt が意味を変えた"
        diff -u "$work/run/original" "$work/run/formatted" | sed -n '1,6p'
    fi

    if "$hado_bin" fmt --check "$work/fmt/$name/main.hd" >/dev/null 2>&1; then
        ok "c: $name: fmt の直後の fmt --check が 0（冪等）"
    else
        ng "c: $name: fmt の直後の fmt --check が 0（冪等）" "2 度目で変わる"
    fi

    rm -rf "$work/print/$name"
    if "$hado_bin" print-postfix "$case_dir" "$work/print/$name" >/dev/null 2>&1; then
        capture_run "$work/print/$name/main.hd" "$work/run/printed"
        if cmp -s "$work/run/original" "$work/run/printed"; then
            ok "b: $name: print-postfix の出力を読み直した run が同じ"
        else
            ng "b: $name: print-postfix の出力を読み直した run が同じ" "往復で意味が変わった"
            diff -u "$work/run/original" "$work/run/printed" | sed -n '1,6p'
        fi
    else
        ng "b: $name: print-postfix の出力を読み直した run が同じ" "印字に失敗した"
    fi
}

if [ "$specimen_count" -eq 0 ]; then
    ng "検体: tests/accept に (f a) b の形がある" "0 本"
else
    while IFS= read -r case_dir; do
        [ -n "$case_dir" ] || continue
        check_specimen "$case_dir"
    done <<<"$specimens"
fi

echo "PASSED: $pass/$total"
[ "$pass" -eq "$total" ]
