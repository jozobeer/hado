#!/bin/bash
# tier: core
# M2 acceptance checks (see docs/plan.md, M2). Run from the repository root.
# 受理・E1xx・非 E1xx の件数と合否は crates/hado/tests/parse_corpus.rs の corpus_counts が
# assert し、m0 の全体テストが走らせる。ここに残るのは診断の位置と glue だけである。
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=2
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT

cargo test --offline --test compiler -- parse_corpus:: --nocapture --test-threads=1 >"$TMPD/base.log" 2>&1

# --- check 1: E1xx の診断の位置（行は error code が定める構文から導く） ---
c1_reasons=""
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
    [ "$line" = "$want" ] || c1_reasons="$c1_reasons $code:line=$line(want=$want)"
done
for pair in "E103:12" "E105:12" "E107:6"; do
    code="${pair%%:*}"
    want="${pair##*:}"
    f="tests/reject/$code/main.hd"
    col=$(diag_col "$TMPD/base.log" "$f")
    [ "$col" = "$want" ] || c1_reasons="$c1_reasons $code:col=$col(want=$want)"
done
n_cols=$(for n in 101 102 103 104 105 106 107; do diag_col "$TMPD/base.log" "tests/reject/E$n/main.hd"; done | sort -u | wc -l | tr -d ' ')
[ "$n_cols" -ge 2 ] || c1_reasons="$c1_reasons distinct-cols=$n_cols"
n_lex=$(grep -o 'lexer::' crates/hado/src/emit.rs | wc -l | tr -d ' ')
[ "$n_lex" -eq 0 ] || c1_reasons="$c1_reasons emit-lexer::=$n_lex"
if [ -z "$c1_reasons" ]; then
    pass 1 "diag-positions"
else
    fail 1 "diag-positions:$c1_reasons"
fi

# --- check 2: glue — public compilation uses the full pipeline ---
c2_reasons=""
n=$(grep -c 'pub fn check' crates/hado/src/types.rs)
[ "$n" -eq 1 ] || c2_reasons="$c2_reasons types-check=$n"
c=$(grep -c 'pub fn compile_to_object' crates/hado/src/lib.rs)
[ "$c" -eq 1 ] || c2_reasons="$c2_reasons lib-pub-fn=$c"
c=$(grep -c 'pub use emit::compile_to_object' crates/hado/src/lib.rs)
[ "$c" -eq 0 ] || c2_reasons="$c2_reasons lib-pub-use=$c"
c=$(grep -c 'compile_source_to_object' crates/hado/src/lib.rs)
[ "$c" -ge 1 ] || c2_reasons="$c2_reasons lib-full-pipeline=0"
c=$(grep -c 'lexer::lex' crates/hado/src/emit.rs)
[ "$c" -eq 0 ] || c2_reasons="$c2_reasons emit-lex::lex=$c"
c=$(grep -c 'parser::parse' crates/hado/src/emit.rs)
[ "$c" -eq 0 ] || c2_reasons="$c2_reasons emit-parser::parse=$c"
if [ -z "$c2_reasons" ]; then
    pass 2 "glue"
else
    fail 2 "glue:$c2_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
