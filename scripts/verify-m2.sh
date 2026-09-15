#!/bin/bash
# M2 acceptance checks 1..8 (see docs/plan.md, M2). Run from the repository root.
# Each check is independent; all run to the end. Non-zero exit if any fails.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# 全走 1 回のあいだ、同じ内容の実走を 1 度に畳む（docs/plan.md 32 節）。
# shellcheck source=scripts/lib/gate-cache.sh
. scripts/lib/gate-cache.sh

ok=0
total=8
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

TMPD="$(mktemp -d)"
tar -cf "$TMPD/corpus.tar" tests/accept tests/reject
find tests/accept tests/reject tests/library crates/hado/tests/corpus.rs -type f -print0 | sort -z | xargs -0 sha256sum > "$TMPD/corpus.sha"
find docs runtime scripts -type f -print0 | sort -z | xargs -0 sha256sum > "$TMPD/protected.sha"

# Any exit path restores the corpus and drops probe files.
cleanup() {
    rm -f tests/accept/__probe_bad__/main.hd
    rmdir tests/accept/__probe_bad__ 2>/dev/null || true
    tar -xf "$TMPD/corpus.tar"
    rm -rf "$TMPD"
}
trap cleanup EXIT

# Base run of the acceptance harness against the committed corpus.
# --test-threads=1 serialises the four tests so marker lines are not
# interleaved with harness progress lines (grep below is substring-safe
# either way).
cargo_cached test --test parse_corpus -- --nocapture --test-threads=1 >"$TMPD/base.log" 2>&1
base_status=$?

# --- check 1: all accept files parse; broken probe is not accepted ---
c1_reasons=""
[ "$base_status" -eq 0 ] || c1_reasons="$c1_reasons base-exit=$base_status"
n_accept=$(ls tests/accept/*/main.hd 2>/dev/null | wc -l | tr -d ' ')
[ "$n_accept" -gt 0 ] || c1_reasons="$c1_reasons disk-accept-count=$n_accept"
for f in tests/accept/*/main.hd; do
    [ -e "$f" ] || continue
    rel="$f"
    n=$(grep -cF "parse_accept_ok: $rel" "$TMPD/base.log")
    [ "$n" -eq 1 ] || c1_reasons="$c1_reasons $rel:lines=$n"
done
[ "$(grep -cF "parse_accept_ok_n: $n_accept" "$TMPD/base.log")" -ge 1 ] || c1_reasons="$c1_reasons accept-n!=$n_accept"
# Probe: `def main = )` must not be accepted.
mkdir -p tests/accept/__probe_bad__
printf -- '// spec: 付録C\n// expect: 0\ndef main = )\n' > tests/accept/__probe_bad__/main.hd
cargo_cached test --test parse_corpus -- --nocapture --test-threads=1 >"$TMPD/probe1.log" 2>&1
rm -f tests/accept/__probe_bad__/main.hd
    rmdir tests/accept/__probe_bad__ 2>/dev/null || true
n_probe=$(grep -cF 'parse_accept_ok: tests/accept/__probe_bad__/main.hd' "$TMPD/probe1.log")
[ "$n_probe" -eq 0 ] || c1_reasons="$c1_reasons probe-bad-accepted($n_probe)"
n_ok_probe=$(grep -cF 'parse_accept_ok:' "$TMPD/probe1.log")
[ "$n_ok_probe" -eq "$n_accept" ] || c1_reasons="$c1_reasons probe-lost-originals($n_ok_probe)"
if [ -z "$c1_reasons" ]; then
    pass 1 "accept-$n_accept"
else
    fail 1 "accept-$n_accept:$c1_reasons"
fi

# --- check 2: specification E1xx codes and swap probe ---
e1xx_codes=$(sed -n 's/^| \(E1[0-9][0-9]\) |.*/\1/p' docs/spec.md | sort -u)
n_e1xx=$(printf '%s\n' "$e1xx_codes" | wc -l)
n_non_e1xx=$(sed -n 's/^| \(E[2-9][0-9][0-9]\) |.*/\1/p' docs/spec.md | sort -u | wc -l)
c2_reasons=""
for code in $e1xx_codes; do
    f="tests/reject/$code/main.hd"
    [ "$(grep -cF "parse_e1xx_ok: $f $code" "$TMPD/base.log")" -eq 1 ] \
        || c2_reasons="$c2_reasons $f:no-ok-line"
    d=$(grep -F "parse_e1xx_diag: $f " "$TMPD/base.log" | head -n 1 | sed "s|.*parse_e1xx_diag: $f ||")
    printf '%s' "$d" | grep -qE "^$f:[0-9]+:[1-9][0-9]*: error\[$code\]: .+" \
        || c2_reasons="$c2_reasons $f:bad-diag($d)"
done
[ "$(grep -cF "parse_e1xx_ok_n: $n_e1xx" "$TMPD/base.log")" -ge 1 ] || c2_reasons="$c2_reasons e1xx-n!=$n_e1xx"
# Content-swap probe: keep the E103 header, put the E104 defect in the body.
cp tests/reject/E103/main.hd "$TMPD/E103.orig"
printf -- '// spec: §5\n// error: E103\n// item: リテラル\ndef main = 1\n# trailing comment in the wrong form\n' > tests/reject/E103/main.hd
cargo_cached test --test parse_corpus -- --nocapture --test-threads=1 >"$TMPD/swap.log" 2>&1
want_line=$(grep -nF '# trailing comment in the wrong form' tests/reject/E103/main.hd | head -n 1 | cut -d: -f1)
want_col=$(awk '/# trailing comment/{print index($0, "#"); exit}' tests/reject/E103/main.hd)
cp "$TMPD/E103.orig" tests/reject/E103/main.hd
n_ok=$(grep -cF 'parse_e1xx_ok: tests/reject/E103/main.hd E103' "$TMPD/swap.log")
[ "$n_ok" -eq 0 ] || c2_reasons="$c2_reasons swap-e103-still-ok($n_ok)"
d=$(grep -F 'parse_e1xx_diag: tests/reject/E103/main.hd ' "$TMPD/swap.log" | head -n 1 | sed 's|.*parse_e1xx_diag: tests/reject/E103/main.hd ||')
printf '%s' "$d" | grep -qE '^tests/reject/E103/main\.hd:[0-9]+:[0-9]+: error\[E104\]: ' \
    || c2_reasons="$c2_reasons swap-not-e104($d)"
dline=$(printf '%s' "$d" | cut -d: -f2)
dcol=$(printf '%s' "$d" | cut -d: -f3)
[ "$dline" = "$want_line" ] && [ "$dcol" = "$want_col" ] \
    || c2_reasons="$c2_reasons swap-pos(diag=$dline:$dcol want=$want_line:$want_col)"
if [ -z "$c2_reasons" ]; then
    pass 2 "e1xx-$n_e1xx+swap"
else
    fail 2 "e1xx-$n_e1xx+swap:$c2_reasons"
fi

# --- check 3: non-E1xx files parse; E1xx never counts as non-E1xx ---
c3_reasons=""
[ "$(grep -cF "parse_non_e1xx_ok_n: $n_non_e1xx" "$TMPD/base.log")" -ge 1 ] || c3_reasons="$c3_reasons non-e1xx-n!=$n_non_e1xx"
for f in tests/reject/*/main.hd; do
    [ -e "$f" ] || continue
    rel="$f"
    code=$(grep -m1 '^// error: ' "$f" | sed 's|^// error: ||')
    n=$(grep -cF "parse_non_e1xx_ok: $rel" "$TMPD/base.log")
    if printf '%s' "$code" | grep -qE '^E1[0-9][0-9]$'; then
        [ "$n" -eq 0 ] || c3_reasons="$c3_reasons e1xx-has-non($rel:$n)"
    else
        [ "$n" -eq 1 ] || c3_reasons="$c3_reasons $rel:lines=$n"
    fi
done
if [ -z "$c3_reasons" ]; then
    pass 3 "non-e1xx-$n_non_e1xx"
else
    fail 3 "non-e1xx-$n_non_e1xx:$c3_reasons"
fi

# --- check 4: E1xx positions, perturbation probes, gen/decoy/tree, emit ---
c4_reasons=""
diag_line() { # log rel
    grep -F "parse_e1xx_diag: $2 " "$1" | head -n 1 | sed "s|.*parse_e1xx_diag: $2 ||" | cut -d: -f2
}
diag_col() { # log rel
    grep -F "parse_e1xx_diag: $2 " "$1" | head -n 1 | sed "s|.*parse_e1xx_diag: $2 ||" | cut -d: -f3
}
expect_line() { # derive the anchor from the syntax prescribed by the error code
    python3 - "$1" "$2" <<'PYANCHOR'
import re, sys
patterns = {
    'E101': r'\bfn\s*\(\s*\|',
    'E102': r'\brec\b',
    'E103': r'(?<![\w.])\.\d',
    'E104': r'--|#',
    'E105': r'\(\s*\d+\s*,',
    'E106': r'\b[A-Z]\w*\s+\d',
    'E107': r'\(\s*[+*/%-]',
}
with open(sys.argv[2]) as source:
    for line, text in enumerate(source, 1):
        if re.search(patterns[sys.argv[1]], text.split('//', 1)[0]):
            print(line)
            break
    else:
        raise SystemExit('expected diagnostic anchor is absent')
PYANCHOR
}
for n in 101 102 103 104 105 106 107; do
    code="E$n"
    f="tests/reject/$code/main.hd"
    line=$(diag_line "$TMPD/base.log" "$f")
    want=$(expect_line "$code" "$f")
    [ "$line" = "$want" ] || c4_reasons="$c4_reasons $code:line=$line(want=$want)"
done
for pair in "E103:12" "E105:12" "E107:6"; do
    code="${pair%%:*}"
    want="${pair##*:}"
    f="tests/reject/$code/main.hd"
    col=$(diag_col "$TMPD/base.log" "$f")
    [ "$col" = "$want" ] || c4_reasons="$c4_reasons $code:col=$col(want=$want)"
done
n_cols=$(for n in 101 102 103 104 105 106 107; do diag_col "$TMPD/base.log" "tests/reject/E$n/main.hd"; done | sort -u | wc -l | tr -d ' ')
[ "$n_cols" -ge 2 ] || c4_reasons="$c4_reasons distinct-cols=$n_cols"
# Line-shift probe: a blank line after `// error:` moves <line> by exactly 1.
for n in 101 102 103 104 105 106 107; do
    f="tests/reject/E$n/main.hd"
    cp "$f" "$TMPD/ls$n.orig"
    awk '/^\/\/ error: E[0-9]/ { print; print ""; next } { print }' "$f" >"$TMPD/ls$n.new"
    cp "$TMPD/ls$n.new" "$f"
    cargo_cached test --test parse_corpus -- --nocapture --test-threads=1 >"$TMPD/ls$n.log" 2>&1
    cp "$TMPD/ls$n.orig" "$f"
    base=$(diag_line "$TMPD/base.log" "$f")
    new=$(diag_line "$TMPD/ls$n.log" "$f")
    want=$((base + 1))
    [ "$new" -eq "$want" ] || c4_reasons="$c4_reasons shift-$n:line=$new(want=$want)"
done
# Column-shift probe: `def main = .5` -> `def main =   .5` moves <col> 12 -> 14.
cp tests/reject/E103/main.hd "$TMPD/cs.orig"
sed 's/^def main = \.5$/def main =   .5/' tests/reject/E103/main.hd >"$TMPD/cs.new"
cp "$TMPD/cs.new" tests/reject/E103/main.hd
cargo_cached test --test parse_corpus -- --nocapture --test-threads=1 >"$TMPD/cs.log" 2>&1
cp "$TMPD/cs.orig" tests/reject/E103/main.hd
base_col=$(diag_col "$TMPD/base.log" "tests/reject/E103/main.hd")
new_col=$(diag_col "$TMPD/cs.log" "tests/reject/E103/main.hd")
[ "$base_col" -eq 12 ] || c4_reasons="$c4_reasons colshift-base=$base_col"
[ "$new_col" -eq 14 ] || c4_reasons="$c4_reasons colshift-new=$new_col"
# Gen / decoy / tree probes and the emit.rs lexical dependency.
n_gen=$(grep -cF 'parse_gen_ok:' "$TMPD/base.log")
[ "$n_gen" -eq 5 ] || c4_reasons="$c4_reasons gen-count=$n_gen"
while read -r a b s; do
    sum=$((a + b))
    [ "$sum" -eq "$s" ] || c4_reasons="$c4_reasons gen-sum($a $b want=$sum got=$s)"
done < <(grep -F 'parse_gen_ok:' "$TMPD/base.log" | sed 's/.*parse_gen_ok: //' | awk '{print $1, $2, $3}')
n_decoy_ok=$(grep -cF 'parse_decoy_ok:' "$TMPD/base.log")
n_decoy_ng=$(grep -cF 'parse_decoy_ng:' "$TMPD/base.log")
[ "$n_decoy_ok" -eq 3 ] || c4_reasons="$c4_reasons decoy-ok=$n_decoy_ok"
[ "$n_decoy_ng" -eq 3 ] || c4_reasons="$c4_reasons decoy-ng=$n_decoy_ng"
n_tree=$(grep -cF 'parse_tree_ok:' "$TMPD/base.log")
[ "$n_tree" -eq 20 ] || c4_reasons="$c4_reasons tree-count=$n_tree"
[ "$(grep -cF 'parse_tree_n: 20' "$TMPD/base.log")" -ge 1 ] || c4_reasons="$c4_reasons tree-n!=20"
n_lex=$(grep -o 'lexer::' crates/hado/src/emit.rs | wc -l | tr -d ' ')
[ "$n_lex" -eq 0 ] || c4_reasons="$c4_reasons emit-lexer::=$n_lex"
if [ -z "$c4_reasons" ]; then
    pass 4 "diag-positions+probes"
else
    fail 4 "diag-positions+probes:$c4_reasons"
fi

# --- check 5: M0 and M1 gates keep their completion values ---
c5_reasons=""
cargo_cached test --offline --no-fail-fast >"$TMPD/cargo.log" 2>&1
[ "$?" -eq 0 ] || c5_reasons="$c5_reasons cargo-test-exit!=0"
cargo_cached test --test walk >"$TMPD/walk.log" 2>&1
[ "$?" -eq 0 ] || c5_reasons="$c5_reasons walk-exit!=0"
# Why not 6/6: R25 が m1-spec の検査を落としたあと、分母の直書きが追従しない。
for script in verify-m0.sh verify-m1-spec.sh verify-m1-corpus.sh; do
    want=$(awk -F= '/^total=/ {print $2; exit}' "scripts/$script")
    # label は run-gates と同じ `verify-<名>`。`.sh` を付けると別の id になり、
    # 同じ門番を走行内で 2 回実走する（docs/plan.md 34 節）。
    gate_cached "${script%.sh}" bash "scripts/$script" >"$TMPD/$script.log" 2>&1
    st=$?
    last=$(tail -n 1 "$TMPD/$script.log")
    [ -n "$want" ] && [ "$st" -eq 0 ] && [ "$last" = "PASSED: $want/$want" ] \
        || c5_reasons="$c5_reasons $script(status=$st,last=$last)"
done
if [ -z "$c5_reasons" ]; then
    pass 5 "gates"
else
    fail 5 "gates:$c5_reasons"
fi

# --- check 6: verification preserves documentation, runtime, and scripts ---
if sha256sum --status -c "$TMPD/protected.sha"; then
    pass 6 "protected-files-unchanged"
else
    fail 6 "protected-files-changed"
fi

# --- check 7: corpus and corpus.rs match their entry state ---
c7_reasons=""
sha256sum --status -c "$TMPD/corpus.sha" || c7_reasons="corpus-changed"
if [ -z "$c7_reasons" ]; then
    pass 7 "corpus-unchanged"
else
    fail 7 "corpus-unchanged:$c7_reasons"
fi

# --- check 8: glue — public compilation uses the full pipeline ---
c8_reasons=""
n=$(grep -c 'pub fn check' crates/hado/src/types.rs)
[ "$n" -eq 1 ] || c8_reasons="$c8_reasons types-check=$n"
c=$(grep -c 'pub fn compile_to_object' crates/hado/src/lib.rs)
[ "$c" -eq 1 ] || c8_reasons="$c8_reasons lib-pub-fn=$c"
c=$(grep -c 'pub use emit::compile_to_object' crates/hado/src/lib.rs)
[ "$c" -eq 0 ] || c8_reasons="$c8_reasons lib-pub-use=$c"
c=$(grep -c 'compile_source_to_object' crates/hado/src/lib.rs)
[ "$c" -ge 1 ] || c8_reasons="$c8_reasons lib-full-pipeline=0"
c=$(grep -c 'lexer::lex' crates/hado/src/emit.rs)
[ "$c" -eq 0 ] || c8_reasons="$c8_reasons emit-lex::lex=$c"
c=$(grep -c 'parser::parse' crates/hado/src/emit.rs)
[ "$c" -eq 0 ] || c8_reasons="$c8_reasons emit-parser::parse=$c"
if [ -z "$c8_reasons" ]; then
    pass 8 "glue"
else
    fail 8 "glue:$c8_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]