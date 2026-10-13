#!/bin/bash
# tier: core
# M0 acceptance checks 1..7 (see docs/plan.md, M0). Run from the repository root.
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
trap 'rm -rf "$TMPD"' EXIT

# --- check 1: cargo build ---
if cargo build >"$TMPD/build.log" 2>&1; then
    pass 1 "cargo-build"
else
    fail 1 "cargo-build exit!=0"
fi

# --- shared full test output for checks 2..4 and 8 ---
# テストごとの結果は scripts/nextest-full.sh の `nextest-case` 行から読む。
nextest_full_cached >"$TMPD/test.log" 2>&1
test_status=$?
passed=$(awk -F'\t' '$1 == "nextest-case" && $2 == "PASS" { n++ } END { print n + 0 }' "$TMPD/test.log")
# バイナリのテストが 1 件以上あり、すべて PASS なら 1。
binary_ok() {
    awk -F'\t' -v b="$1" '
        $1 == "nextest-case" && $3 == b { n++; if ($2 != "PASS") bad = 1 }
        END { print (n > 0 && !bad) ? 1 : 0 }
    ' "$TMPD/test.log"
}
walk_ok=$(binary_ok hado::walk)
tail_ok=$(binary_ok hado::tail)

# --- check 2: the full test passes >= 2 with exit 0 ---
if [ "$test_status" -eq 0 ] && [ "${passed:-0}" -ge 2 ]; then
    pass 2 "nextest passed=$passed"
else
    fail 2 "nextest status=$test_status passed=${passed:-0} (<2 or nonzero)"
fi

# --- check 3: walk ---
walk_reasons=""
grep -Fq '"def main = 1 + 2", 3' crates/hado/tests/walk.rs || walk_reasons="$walk_reasons missing-expected-exit-code-3"
[ "$walk_ok" = "1" ] || walk_reasons="$walk_reasons walk-test-not-passing"
if [ -z "$walk_reasons" ]; then
    pass 3 "walk"
else
    fail 3 "walk:$walk_reasons"
fi

# --- check 4: tail ---
tail_reasons=""
grep -Eq '100_000_000|100000000' crates/hado/tests/tail.rs || tail_reasons="$tail_reasons missing-depth-const"
[ "$tail_ok" = "1" ] || tail_reasons="$tail_reasons tail-test-not-passing"
if [ -z "$tail_reasons" ]; then
    pass 4 "tail"
else
    fail 4 "tail:$tail_reasons"
fi

# --- check 5: dependencies ---
dep_reasons=""
for c in cranelift-codegen cranelift-frontend cranelift-module cranelift-object cranelift-native; do
    grep -Eq "^${c}[[:space:]]*=[[:space:]]*\"0\.135\"" crates/hado/Cargo.toml \
        || dep_reasons="$dep_reasons $c!=0.135"
done
if awk '/^\[/{sec=$0} sec=="[build-dependencies]" && /^cc[[:space:]]*=[[:space:]]*"1"/{found=1} END{exit !found}' crates/hado/Cargo.toml; then
    :
else
    dep_reasons="$dep_reasons cc!=1-in-build-deps"
fi
if [ -z "$dep_reasons" ]; then
    pass 5 "deps"
else
    fail 5 "deps:$dep_reasons"
fi

# --- check 6: ISA flags (values, not mere words) ---
flag_reasons=""
check_set() {
    grep -Eq "set\([[:space:]]*\"$2\"[[:space:]]*,[[:space:]]*\"$3\"[[:space:]]*\)" "$1" \
        || flag_reasons="$flag_reasons $1:$2!=$3"
}
# opt_level は make_isa の引数になり、既定の値は emit.rs の定数が持つ
# （cranelift-opt-level）。値の錠はその定数に当てる。
check_const() {
    grep -Eq "pub const $2: &str = \"$3\";" "$1" \
        || flag_reasons="$flag_reasons $1:$2!=$3"
}
check_set crates/hado/src/emit.rs is_pic true
check_set crates/hado/src/emit.rs preserve_frame_pointers true
check_const crates/hado/src/emit.rs BUILD_OPT_LEVEL none
check_const crates/hado/src/emit.rs RUN_OPT_LEVEL none
grep -Eq 'set\([[:space:]]*"opt_level"[[:space:]]*,[[:space:]]*opt_level[[:space:]]*\)' \
    crates/hado/src/emit.rs || flag_reasons="$flag_reasons emit.rs:opt_level-not-applied"
check_set crates/hado/tests/tail.rs is_pic true
check_set crates/hado/tests/tail.rs preserve_frame_pointers true
check_set crates/hado/tests/tail.rs opt_level none
if grep -rEq 'set\([[:space:]]*"enable_verifier"[[:space:]]*,[[:space:]]*"false"[[:space:]]*\)' crates/hado; then
    flag_reasons="$flag_reasons enable_verifier=false-present"
fi
if [ -z "$flag_reasons" ]; then
    pass 6 "isa-flags"
else
    fail 6 "isa-flags:$flag_reasons"
fi

# --- check 7: runtime words, each independently ---
rt_reasons=""
for w in GC_INIT hado_entry hado_alloc GC_malloc; do
    grep -rq -- "$w" runtime/ || rt_reasons="$rt_reasons missing-$w"
done
if [ -z "$rt_reasons" ]; then
    pass 7 "runtime"
else
    fail 7 "runtime:$rt_reasons"
fi

# --- check 8: 専用のテストターゲットが実在し、走っている ---
# 削除したラッパー門番（`cargo test --test rN_...` を呼んでいた verify-r13.sh や
# verify-record-constructors.sh）が持っていた検査。全体テストは対象ごと消えても残りを
# 走らせて緑になるので、対象の名前が全走の出力に現れることを見る（審査の指摘）。名前は
# ディスクから導くと、消えた対象が期待からも消えて素通りするため、削除した門番が守っていた
# 組をここに固定する。
target_reasons=""
for name in \
    named_record_constructors named_record_order \
    r9_coverage r9_grammar r9_module_types r9_names r9_order r9_print r9_syntax r9_unused \
    r13_conditions r14_rails r16_sections r17_files r18_spelling \
    r19_contracts r20_entries r21_functors r22_namespaces r23_syntax \
    r24_effects r25_effects r26_effects r31_effects r37_world
do
    if [ ! -f "crates/hado/tests/$name.rs" ]; then
        target_reasons="$target_reasons absent-$name"
    else
        # 走らなかった対象と中身を空にした対象は、どちらも結果の行が 0 件になる。
        count=$(awk -F'\t' -v b="hado::$name" '
            $1 == "nextest-case" && $3 == b { n++ } END { print n + 0 }
        ' "$TMPD/test.log")
        [ "$count" -ge 1 ] || target_reasons="$target_reasons not-run-or-empty-$name"
    fi
done
if [ -z "$target_reasons" ]; then
    pass 8 "test-targets"
else
    fail 8 "test-targets:$target_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
