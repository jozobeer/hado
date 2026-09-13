#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0

check() {
    local label=$1
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}

workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

# 位置を検査する探針は、先頭の空行と行頭の空白で診断の行:列をずらし、
# 期待値を探針側で計算する（位置を定数で書かない）。
spaces() { printf '%*s' "$1" ''; }
blank_lines() { local i; for ((i = 0; i < $1; i++)); do printf '\n'; done; }

# ケースの代表ファイル（docs/corpus.md）: library を宣言したファイル。無ければ
# パスのバイト順の先頭。ファイル名は判定器に書かない。宣言は行コメント（`//` 以降）を
# 除いた上で、行頭の空白の後に library が単独で現れる行（runner の parse と同じ字句）。
library_rep() {
    local dir=$1 rep
    rep=$(
        find "$dir" -type f -name '*.hd' -print0 \
            | while IFS= read -r -d '' file; do
                if sed 's|//.*||' "$file" | grep -qE '^[[:space:]]*library([[:space:]]|$)'; then
                    printf '%s\n' "$file"
                fi
            done \
            | LC_ALL=C sort | head -n1
    )
    if [ -z "$rep" ]; then
        rep=$(find "$dir" -type f \( -name '*.hd' -o -name '*.hdi' \) | LC_ALL=C sort | head -n1)
    fi
    printf '%s' "$rep"
}

# 32-i 補助: 代表ファイルの選択が runner（corpus.rs）と同じ字句規則で決まることを
# 確かめる。library の行末コメント（`//` 以降）と行頭の空白を通す。library を宣言した
# ファイルをバイト順で後ろに置き、選択がそれと一致しなければ落ちる。探針が自分で
# 作るファイルだけを使い、tests/library の名前は判定器に書かない。
verify_library_rep() {
    local base="$workdir/32i-rep"
    mkdir -p "$base/comment" "$base/indent"
    for c in comment indent; do
        printf '// no library declaration here\n\nmodule Extra\ndef f(x: Int): Int\n\nimplement Extra as E\ndef f x => x\n' >"$base/$c/a.hd"
    done
    printf 'library // declaration\n' >"$base/comment/z.hd"
    printf '  library\n' >"$base/indent/z.hd"
    local c rep want
    for c in comment indent; do
        want="$base/$c/z.hd"
        rep=$(library_rep "$base/$c")
        if [ "$rep" != "$want" ]; then
            echo "  representative for $c: want $want got '$rep'"
            return 1
        fi
        if ! cargo run --offline -q -- check "$rep" >"$workdir/lib-out" 2>"$workdir/lib-err"; then
            echo "  library rejected: $rep ($(cat "$workdir/lib-err"))"
            return 1
        fi
    done
}

# 32-i: tests/library の全ケースを代表ファイルから check して受理。
verify_library() {
    local dir rep n=0
    for dir in tests/library/*/; do
        [ -d "$dir" ] || continue
        rep=$(library_rep "$dir")
        if [ -z "$rep" ]; then
            echo "  library no representative: $dir"
            return 1
        fi
        if ! cargo run --offline -q -- check "$rep" >"$workdir/lib-out" 2>"$workdir/lib-err"; then
            echo "  library rejected: $rep ($(cat "$workdir/lib-err"))"
            return 1
        fi
        if [ -s "$workdir/lib-out" ] || [ -s "$workdir/lib-err" ]; then
            echo "  library diagnostics: $rep"
            return 1
        fi
        n=$((n + 1))
    done
    [ "$n" -ge 1 ]
}

# 32-j 補助: base/main.hd を build して実行し、印字が expected と一致するか。
report_build_run() {
    local base=$1 expected=$2 got status
    if ! cargo run --offline -q -- build "$base/main.hd" >/dev/null 2>"$workdir/rep-build-err"; then
        echo "  report build failed: $(cat "$workdir/rep-build-err")"
        return 1
    fi
    got=$("$base/main" 2>"$workdir/rep-run-err"); status=$?
    if [ "$status" -ne 0 ] || [ "$got" != "$expected" ]; then
        echo "  report run want '$expected' got '$got' status=$status"
        return 1
    fi
}

# argv を使う実物の入口では、stdout と stderr をバイト単位で照合する。
report_build_run_args() {
    local base=$1 expected=$2
    shift 2
    cargo run --offline -q -- build "$base/main.hd" >"$base/build.out" 2>"$base/build.err" || return 1
    "$base/main" "$@" >"$base/out" 2>"$base/err" || return 1
    cat "$base/out"
    [ "$(wc -c <"$base/build.err")" -eq 0 ] &&
        [ "$(wc -c <"$base/err")" -eq 0 ] && cmp -s "$base/out" "$expected"
}

# 32-j 補助: base に report/ を写し、3 つ組（name:passed:total）を add で畳む
# main.hd を書いて build → 実行。期待値は呼び出し側で計算して渡す。
report_case() {
    local base=$1 state="report::Report.empty" step=0 name passed total triple
    mkdir -p "$base"
    cp -r tools/gates/report "$base/report"
    {
        printf 'def main =\n'
        for triple in "$3" "$4" "$5"; do
            IFS=: read -r name passed total <<<"$triple"
            printf '  let t%d = report::Report.add %s "%s" %s %s ->\n' "$step" "$state" "$name" "$passed" "$total"
            state="t$step"
            step=$((step + 1))
        done
        printf '  report::Report.render %s\n' "$state"
    } >"$base/main.hd"
    report_build_run "$base" "$2"
}

# 32-j (a): 実行時に生成した 3 つ組。少なくとも 1 つは赤になる。
report_case_random() {
    local names=("a$RANDOM" "b$RANDOM" "c$RANDOM")
    local totals=($((1 + RANDOM % 100)) $((1 + RANDOM % 100)) $((1 + RANDOM % 100)))
    local passed=("${totals[0]}" "$((RANDOM % totals[1]))" "$((RANDOM % (totals[2] + 1)))")
    local red=()
    [ "${passed[0]}" -lt "${totals[0]}" ] && red+=("${names[0]}")
    [ "${passed[1]}" -lt "${totals[1]}" ] && red+=("${names[1]}")
    [ "${passed[2]}" -lt "${totals[2]}" ] && red+=("${names[2]}")
    local red_str="none"
    [ "${#red[@]}" -gt 0 ] && red_str="${red[*]}"
    local sum_passed=$((passed[0] + passed[1] + passed[2]))
    local sum_total=$((totals[0] + totals[1] + totals[2]))
    report_case "$workdir/32j" "$sum_passed/$sum_total passed; red: $red_str" \
        "${names[0]}:${passed[0]}:${totals[0]}" \
        "${names[1]}:${passed[1]}:${totals[1]}" \
        "${names[2]}:${passed[2]}:${totals[2]}"
}

# 32-j (b): 全件緑の 3 つ組。赤が無いときの `red: none` を測る。
report_case_green() {
    local totals=($((1 + RANDOM % 100)) $((1 + RANDOM % 100)) $((1 + RANDOM % 100)))
    local sum=$((totals[0] + totals[1] + totals[2]))
    report_case "$workdir/32j-green" "$sum/$sum passed; red: none" \
        "g0:${totals[0]}:${totals[0]}" \
        "g1:${totals[1]}:${totals[1]}" \
        "g2:${totals[2]}:${totals[2]}"
}

# 32-j (c): 空のリスト。`0/0 passed; red: none` を測る。
report_case_empty() {
    local base="$workdir/32j-empty"
    mkdir -p "$base"
    cp -r tools/gates/report "$base/report"
    printf 'def main = report::Report.render report::Report.empty\n' >"$base/main.hd"
    report_build_run "$base" "0/0 passed; red: none"
}

# 32-j (d): 実物の入口を check → 写して build → 実行。実行ファイルは残さない。
report_case_real() {
    local base="$workdir/32j-real"
    mkdir -p "$base"
    cp tools/gates/main.hd "$base/main.hd"
    cp -r tools/gates/report "$base/report"
    if ! cargo run --offline -q -- check "$base/main.hd" >"$workdir/rep-out" 2>"$workdir/rep-err"; then
        echo "  real main.hd rejected: $(cat "$workdir/rep-err")"
        return 1
    fi
    if [ -s "$workdir/rep-out" ] || [ -s "$workdir/rep-err" ]; then
        echo "  real main.hd diagnostics"
        return 1
    fi
    mkdir "$base/logs"
    local name n limit sum_passed=0 sum_total=0 red=()
    for name in "a$RANDOM" "b$RANDOM" "c$RANDOM"; do
        limit=$((1 + RANDOM % 100))
        n=$((RANDOM % limit))
        printf 'noise\nPASSED: %d/%d\n' "$n" "$limit" >"$base/logs/$name"
        sum_passed=$((sum_passed + n))
        sum_total=$((sum_total + limit))
        red+=("$name")
    done
    printf '%d/%d passed; red: %s\n' "$sum_passed" "$sum_total" "${red[*]}" >"$base/expect"
    report_build_run_args "$base" "$base/expect" "$base/logs"
}

# 32-j: E2 の要約プログラム。report.hd を単体で check し、実行時に生成した
# 3 つ組で main.hd を書いて build → 実行。期待値は判定器側で計算する。
verify_report() {
    local rep="tools/gates/report/report.hd"
    if ! cargo run --offline -q -- check "$rep" >"$workdir/rep-out" 2>"$workdir/rep-err"; then
        echo "  report.hd rejected: $(cat "$workdir/rep-err")"
        return 1
    fi
    if [ -s "$workdir/rep-out" ] || [ -s "$workdir/rep-err" ]; then
        echo "  report.hd diagnostics"
        return 1
    fi
    report_case_random || return 1
    report_case_green || return 1
    report_case_empty || return 1
    report_case_real || return 1
}

mkdir -p "$workdir/32a" "$workdir/32b" "$workdir/32b2" "$workdir/32b3" "$workdir/32c1" "$workdir/32c2" "$workdir/32c3" \
    "$workdir/32d" "$workdir/32d1" "$workdir/32d2" "$workdir/32d3" "$workdir/32d4" "$workdir/32d5" "$workdir/32e" \
    "$workdir/32g1" "$workdir/32g2" "$workdir/32g3"

cat >"$workdir/32a/lib.hd" <<'EOF'
def helper value => value + 1

module Store
def read(value: Int): Int

implement Store as Memory
def read value => helper value
EOF

cat >"$workdir/32b/lib.hd" <<'EOF'
library

module Store
def read(value: Int): Int

implement Store as MemoryStore
def read value => value
EOF
printf 'def main = 1\n' >"$workdir/32c3/main.hd"
printf 'def library = 1\n' >"$workdir/32g1/main.hd"
printf 'def shared = 1\n' >"$workdir/32g2/main.hd"
printf 'type shared = | A\n' >"$workdir/32g3/main.hd"

cat >"$workdir/32d/lib.hd" <<'EOF'
library

type Outer = | Outer(Int)

effect E
abort fail

module S
def read(value: Int): Int

module Empty
def done(x: Int): Int

implement S as Memory
def read value => value

implement S as Make(P: S)
def read value => P.read value

handler E as H
abort fail => 0
return x => x
EOF

cat >"$workdir/32d1/lib.hd" <<'EOF'
library

module S
def read(value: Int): Int

implement S as Memory
def read value => value
def extra x => x
EOF

cat >"$workdir/32d2/lib.hd" <<'EOF'
library

module S
def read(value: Int): Int

implement S as Memory
type Local = | Local(Int)
def read value => value
EOF

cat >"$workdir/32d3/lib.hd" <<'EOF'
library

def helper x => x

module S
def read(value: Int): Int

implement S as Memory
def read value => value
EOF

printf 'def main = 1\n' >"$workdir/32e/a.hd"
cat >"$workdir/32e/lib.hd" <<'EOF'
module Store
def read(value: Int): Int

implement Store as Memory
def read value => value
EOF

expect_error() {
    local label=$1 entry=$2 file=$3 code=$4 line=$5 col=$6
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- check "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$file:$line:$col: error[$code]: "
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$prefix', got status=$status '$err')"
    fi
}

expect_command_error() {
    local label=$1 command=$2 entry=$3 file=$4 code=$5 line=$6 col=$7
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- "$command" "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$file:$line:$col: error[$code]: "
    local artifact="${entry%.hd}"
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]] && [ ! -e "$artifact" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$prefix' without artifact, got status=$status '$err')"
    fi
}

accept() {
    local label=$1 entry=$2
    total=$((total + 1))
    local out err status
    out=$(cargo run --offline -q -- check "$entry" 2>"$workdir/stderr"); status=$?
    err=$(cat "$workdir/stderr")
    if [ "$status" -eq 0 ] && [ -z "$out" ] && [ -z "$err" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (status=$status stdout='$out' stderr='$err')"
    fi
}

accept_print() {
    local label=$1 entry=$2
    total=$((total + 1))
    local out="$workdir/print-stdout" err="$workdir/print-stderr" status
    cargo run --offline -q -- check --print-type "$entry" >"$out" 2>"$err"; status=$?
    # 末尾の改行まで含めて測る（コマンド置換は末尾改行を落とすので使わない）。
    local out_bytes err_bytes
    out_bytes=$(wc -c <"$out"); err_bytes=$(wc -c <"$err")
    if [ "$status" -eq 0 ] && [ "$out_bytes" -eq 0 ] && [ "$err_bytes" -eq 0 ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (status=$status stdout_bytes=$out_bytes stderr_bytes=$err_bytes)"
    fi
}

expect_entry_error() {
    local label=$1 command=$2 entry=$3 file=$4
    total=$((total + 1))
    local err status
    err=$(cargo run --offline -q -- "$command" "$entry" 2>&1 >/dev/null); status=$?
    local prefix="$file:1:1: error[E206]: "
    local artifact="${entry%.hd}"
    if [ "$status" -eq 1 ] && [[ "$err" == "$prefix"* ]] \
        && [[ "$err" == *"check"* ]] && [[ "$err" == *"library"* ]] && [ ! -e "$artifact" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want entry-oriented E206 with check/library without artifact, got status=$status '$err')"
    fi
}

build_prints() {
    local label=$1 entry=$2 expected=$3
    total=$((total + 1))
    local artifact="${entry%.hd}" got status
    if ! cargo run --offline -q -- check "$entry" >/dev/null 2>"$workdir/check-stderr"; then
        echo "NG: $label (check failed: $(cat "$workdir/check-stderr"))"
        return
    fi
    if ! cargo run --offline -q -- build "$entry" >/dev/null 2>"$workdir/build-stderr"; then
        echo "NG: $label (build failed: $(cat "$workdir/build-stderr"))"
        return
    fi
    got=$("$artifact" 2>"$workdir/run-stderr"); status=$?
    if [ "$status" -eq 0 ] && [ "$got" = "$expected" ]; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label (want '$expected' got status=$status '$got')"
    fi
}

# 32-f: 埋め込み。印字する値は実行時に生成した整数をライブラリの def に埋め、
# 期待値は判定器側で計算する（生成した値はスクリプトに定数として現れない）。
payload_fa=$(( (RANDOM << 15) | RANDOM ))
payload_fb=$(( 1 + RANDOM ))
make_app() {
    local base=$1
    mkdir -p "$base/lib/store" "$base/lib/id"
    printf 'library\n\ntype Found = { value: Int }\n' >"$base/lib/lib.hd"
    printf 'module Store\ndef find(x: Int): Int\ndef wrap(f: Found): Int\n' >"$base/lib/store/store.hdi"
    cat >"$base/lib/id/int_id.hd" <<EOF
module IntId
def value(x: Int): Int

implement IntId as IntId
def value x => x + $payload_fa
EOF
    cat >"$base/lib/store/memory.hd" <<EOF
implement Store as Memory
def find id => id::IntId.value id + $payload_fb
def wrap f => f.value
EOF
    printf 'def main = lib::store::Memory.find 1\n' >"$base/main.hd"
}

# 32-a: main の無い .hd は check ではライブラリとして受理、build / dump-mono は E206 1:1。
accept '32-a check (main 無しはライブラリとして受理)' "$workdir/32a/lib.hd"
accept_print '32-a print-type (stdout も stderr も空)' "$workdir/32a/lib.hd"
expect_entry_error '32-a build (E206 1:1、entry 向けメッセージ)' build "$workdir/32a/lib.hd" lib.hd
expect_entry_error '32-a dump-mono (E206 1:1、entry 向けメッセージ)' dump-mono "$workdir/32a/lib.hd" lib.hd

# 32-b: library を宣言したディレクトリの main は E206。行頭の空白で列を、
# 先頭の空行で行をずらし、期待する行:列を探針の中で計算する。
indent_b=$((1 + RANDOM % 8))
printf '%sdef main = 1\n' "$(spaces "$indent_b")" >"$workdir/32b/main.hd"
expect_error '32-b (check, library ディレクトリの main、行頭の空白)' "$workdir/32b/main.hd" main.hd E206 1 $((indent_b + 1))
expect_command_error '32-b (build, library ディレクトリの main、行頭の空白)' build "$workdir/32b/main.hd" main.hd E206 1 $((indent_b + 1))
expect_error '32-b (check, lib.hd から他ファイルの main、行頭の空白)' "$workdir/32b/lib.hd" main.hd E206 1 $((indent_b + 1))
blank_b=$((1 + RANDOM % 4))
{ blank_lines "$blank_b"; printf 'def main = 1\n'; } >"$workdir/32b/main.hd"
expect_error '32-b (check, 先頭の空行)' "$workdir/32b/main.hd" main.hd E206 $((blank_b + 1)) 1

# 32-b (library 宣言ディレクトリの引数付き main): 定義の形によらず E206。別ファイルの
# 引数付き main でも、位置はその def を指す（値定義だけを探してはいけない）。
printf 'library\n' >"$workdir/32b2/lib.hd"
blank_b2=$((1 + RANDOM % 4))
{
    blank_lines "$blank_b2"
    printf 'def main x => x\n\n'
    printf 'module S\n'
    printf 'def read(value: Int): Int\n\n'
    printf 'implement S as Memory\n'
    printf 'def read value => main value\n'
} >"$workdir/32b2/main.hd"
expect_error '32-b (library の引数付き main、別ファイル、先頭の空行)' "$workdir/32b2/lib.hd" main.hd E206 $((blank_b2 + 1)) 1

# 32-b (引数付き main の build / dump-mono): 早期拒否の 1:1 ではなく、entry_form の
# E206 が check と同じ宣言位置になる。
blank_b3=$((1 + RANDOM % 4))
{ blank_lines "$blank_b3"; printf 'def main x => x\n'; } >"$workdir/32b3/main.hd"
expect_error '32-b (引数付き main、check は宣言位置)' "$workdir/32b3/main.hd" main.hd E206 $((blank_b3 + 1)) 1
expect_command_error '32-b (引数付き main、build は宣言位置)' build "$workdir/32b3/main.hd" main.hd E206 $((blank_b3 + 1)) 1
expect_command_error '32-b (引数付き main、dump-mono は宣言位置)' dump-mono "$workdir/32b3/main.hd" main.hd E206 $((blank_b3 + 1)) 1

# 32-c (1): 同一ファイルの 2 つ目は E205。
indent_c1=$((1 + RANDOM % 8))
{ printf 'library\n'; printf '%slibrary\n' "$(spaces "$indent_c1")"; } >"$workdir/32c1/main.hd"
expect_error '32-c (1a) 同一ファイルの 2 つ目、行頭の空白' "$workdir/32c1/main.hd" main.hd E205 2 $((indent_c1 + 1))
blank_c1=$((1 + RANDOM % 4))
{ printf 'library\n'; blank_lines "$blank_c1"; printf 'library\n'; } >"$workdir/32c1/main.hd"
expect_error '32-c (1b) 同一ファイルの 2 つ目、先頭の空行' "$workdir/32c1/main.hd" main.hd E205 $((blank_c1 + 2)) 1

# 32-c (2): 別ファイルの 2 つ目は E205。
printf 'library\n' >"$workdir/32c2/a.hd"
indent_c2=$((1 + RANDOM % 8))
printf '%slibrary\n' "$(spaces "$indent_c2")" >"$workdir/32c2/b.hd"
expect_error '32-c (2a) 別ファイルの 2 つ目、行頭の空白' "$workdir/32c2/a.hd" b.hd E205 1 $((indent_c2 + 1))
blank_c2=$((1 + RANDOM % 4))
{ blank_lines "$blank_c2"; printf 'library\n'; } >"$workdir/32c2/b.hd"
expect_error '32-c (2b) 別ファイルの 2 つ目、先頭の空行' "$workdir/32c2/a.hd" b.hd E205 $((blank_c2 + 1)) 1

# 32-c (3): .hdi の library は E100。
indent_c3=$((1 + RANDOM % 8))
printf '%slibrary\n' "$(spaces "$indent_c3")" >"$workdir/32c3/sig.hdi"
expect_error '32-c (3a) .hdi の library、行頭の空白' "$workdir/32c3/main.hd" sig.hdi E100 1 $((indent_c3 + 1))
blank_c3=$((1 + RANDOM % 4))
{ blank_lines "$blank_c3"; printf 'library\n'; } >"$workdir/32c3/sig.hdi"
expect_error '32-c (3b) .hdi の library、先頭の空行' "$workdir/32c3/main.hd" sig.hdi E100 $((blank_c3 + 1)) 1
accept '32-d (未参照の公開面でも受理)' "$workdir/32d/lib.hd"
expect_error '32-d (1) 契約に無い実装の def' "$workdir/32d1/lib.hd" lib.hd E514 8 1
expect_error '32-d (2) 実装局所の ADT が未使用' "$workdir/32d2/lib.hd" lib.hd E513 7 1
expect_error '32-d (3) ライブラリ根の外の def が未使用' "$workdir/32d3/lib.hd" lib.hd E513 3 1

# 32-d (4): handler の腕から参照される外の def は、その先も辿って根にする。
cat >"$workdir/32d4/lib.hd" <<'EOF'
library

effect E
abort fail

handler E as H
abort fail => helper 0
return x => x

def helper x => leaf x
def leaf x => x
EOF
accept '32-d (4) handler から間接参照される外の def も根から辿る' "$workdir/32d4/lib.hd"

# 32-d (5): 負の対照。handler が直接呼ぶ def から辿れない def は E513。
cat >"$workdir/32d5/lib.hd" <<'EOF'
library

effect E
abort fail

handler E as H
abort fail => helper 0
return x => x

def helper x => x
def leaf x => x
EOF
expect_error '32-d (5) handler から辿れない外の def は E513' "$workdir/32d5/lib.hd" lib.hd E513 11 1
expect_error '32-e (宣言の無い main は普通の def)' "$workdir/32e/lib.hd" a.hd E513 1 1
rm "$workdir/32e/a.hd"
accept '32-e (a.hd を消せば受理)' "$workdir/32e/lib.hd"
expect_error '32-g def library' "$workdir/32g1/main.hd" main.hd E100 1 5
expect_error '32-g def shared' "$workdir/32g2/main.hd" main.hd E100 1 5
expect_error '32-g type shared' "$workdir/32g3/main.hd" main.hd E100 1 6

# 32-f: 埋め込みの受理。ライブラリ根のファイルに外の型を置き、ライブラリの中の
# パス（id::IntId と、ルートの Found）をライブラリ根から解く。
make_app "$workdir/32f/app"
build_prints '32-f (check → build → 実行、生成値)' "$workdir/32f/app/main.hd" "$((1 + payload_fa + payload_fb))"

# 32-f (1): ライブラリの中から利用側の根の外の型を無修飾で参照すると E204。
make_app "$workdir/32f1/app"
printf 'type Config = { n: Int }\ndef main = lib::store::Memory.find 1\n' >"$workdir/32f1/app/main.hd"
printf 'implement Store as Memory\ndef find id => id::IntId.value id + %s\ndef wrap (f: Config) => f.n\n' "$payload_fb" >"$workdir/32f1/app/lib/store/memory.hd"
expect_error '32-f (1) 利用側の根の外の型を無修飾参照' "$workdir/32f1/app/main.hd" lib/store/memory.hd E204 3 14

# 32-f (2): ライブラリの中から兄弟ディレクトリを参照すると E204。
make_app "$workdir/32f2/app"
mkdir -p "$workdir/32f2/app/other"
printf 'module OtherStore\ndef f(x: Int): Int\n' >"$workdir/32f2/app/other/other_store.hdi"
printf 'implement OtherStore as Helper\ndef f x => x\n' >"$workdir/32f2/app/other/helper.hd"
printf 'implement Store as Memory\ndef find id => other::Helper.f id + %s\ndef wrap f => f.value\n' "$payload_fb" >"$workdir/32f2/app/lib/store/memory.hd"
expect_error '32-f (2) 兄弟ディレクトリの実装参照' "$workdir/32f2/app/main.hd" lib/store/memory.hd E204 2 16

# 32-f (3): 入口検査でライブラリの中の未参照の実装は E513。
make_app "$workdir/32f3/app"
mkdir -p "$workdir/32f3/app/lib/extra"
printf 'implement store::Store as Spare\ndef find x => x\ndef wrap f => f.value\n' >"$workdir/32f3/app/lib/extra/spare.hd"
expect_error '32-f (3) ライブラリ内の未参照の実装' "$workdir/32f3/app/main.hd" lib/extra/spare.hd E513 1 1

# 32-f 単体: ライブラリ根のファイルを単体で check すれば受理（パスはライブラリ根から解く）。
make_app "$workdir/32funit/app"
accept '32-f 単体 (ライブラリ根からパスを解く)' "$workdir/32funit/app/lib/lib.hd"

# 32-f 埋め込みの main: ライブラリ根の別ファイルの def main は E206。
make_app "$workdir/32fmain/app"
printf 'def main = 1\n' >"$workdir/32fmain/app/lib/main.hd"
expect_error '32-f 埋め込みの main (library ディレクトリの def main)' "$workdir/32fmain/app/main.hd" lib/main.hd E206 1 1

# 32-d (4): 根でもライブラリ根でもないサブディレクトリのファイルの外の def は E100。
make_app "$workdir/32d4b/app"
printf 'def helper x => x\n\nimplement Store as Memory\ndef find id => id::IntId.value id + %s\ndef wrap f => f.value\n' "$payload_fb" >"$workdir/32d4b/app/lib/store/memory.hd"
expect_error '32-d (4) 根でもライブラリ根でもない外の def' "$workdir/32d4b/app/main.hd" lib/store/memory.hd E100 1 1

# 32-d (3) 再確認: 外の def はファイル局所。別ファイルからは E204、同じファイルの
# 実装からは受理。
mkdir -p "$workdir/32d3b/lib/store" "$workdir/32d3c"
printf 'library\n\ndef helper x => x + 1\n' >"$workdir/32d3b/lib/lib.hd"
printf 'module Store\ndef find(x: Int): Int\n' >"$workdir/32d3b/lib/store/store.hdi"
printf 'implement store::Store as Memory\ndef find id => helper id\n' >"$workdir/32d3b/lib/store/memory.hd"
expect_error '32-d (3) 再確認 (外の def はファイル局所)' "$workdir/32d3b/lib/lib.hd" store/memory.hd E204 2 16
printf 'library\n\ndef helper x => x + 1\n\nmodule Local\ndef run(x: Int): Int\n\nimplement Local as L\ndef run x => helper x\n' >"$workdir/32d3c/lib.hd"
accept '32-d (3) 再確認 (同じファイルの実装から参照)' "$workdir/32d3c/lib.hd"

# 32-i: tests/library を代表ファイルから check して全件受理。
check '32-i (代表ファイルの選択が runner と一致)' verify_library_rep
check '32-i (tests/library 全件受理)' verify_library

# 32-j: E2 の要約プログラムを check / build / 実行。
check '32-j (E2 の要約を check / build / 実行)' verify_report

# ライブラリ根自身の宣言も prelude の名前と衝突する（§55, §59）。ライブラリ根は
# 利用側の根の名前とは比べないが、prelude はライブラリの中でも見える。単体検査と
# 埋め込みで同じ位置になる。
mkdir -p "$workdir/32f4/app/lib"
printf 'def main = 1\n' >"$workdir/32f4/app/main.hd"
blank_f4=$((1 + RANDOM % 4))
{ printf 'library\n'; blank_lines "$blank_f4"; printf 'type Option = | Local\n'; } >"$workdir/32f4/app/lib/lib.hd"
expect_error '32-f4 prelude 衝突 (埋め込み)' "$workdir/32f4/app/main.hd" lib/lib.hd E205 $((blank_f4 + 2)) 6
expect_error '32-f4 prelude 衝突 (単体)' "$workdir/32f4/app/lib/lib.hd" lib.hd E205 $((blank_f4 + 2)) 6

# ライブラリ根と配下の型の衝突（§55）。配下側の宣言を指す。
mkdir -p "$workdir/32f5/app/lib/store"
printf 'def main = 1\n' >"$workdir/32f5/app/main.hd"
printf 'library\n\ntype Found = | A\n' >"$workdir/32f5/app/lib/lib.hd"
blank_f5=$((1 + RANDOM % 4))
{ blank_lines "$blank_f5"; printf 'type Found = | B\n'; } >"$workdir/32f5/app/lib/store/store.hd"
expect_error '32-f5 ライブラリ根と配下の型の衝突' "$workdir/32f5/app/main.hd" lib/store/store.hd E205 $((blank_f5 + 1)) 6

# ライブラリ根と配下の契約の衝突（§55）。constructor と同じ名前空間の規則。
mkdir -p "$workdir/32f6/app/lib/store"
printf 'def main = 1\n' >"$workdir/32f6/app/main.hd"
printf 'library\n\nmodule Store\ndef read(x: Int): Int\n' >"$workdir/32f6/app/lib/lib.hd"
blank_f6=$((1 + RANDOM % 4))
{ blank_lines "$blank_f6"; printf 'module Store\ndef read(x: Int): Int\n'; } >"$workdir/32f6/app/lib/store/store.hd"
expect_error '32-f6 ライブラリ根と配下の契約の衝突' "$workdir/32f6/app/main.hd" lib/store/store.hd E205 $((blank_f6 + 1)) 8

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
