#!/bin/bash
# tier: core
# M0 acceptance checks (see docs/plan.md, M0). Run from the repository root.
# Each check is independent; all run to the end. Non-zero exit if any fails.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=5
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

# --- check 2: the full test passes with exit 0 ---
# テストごとの結果は scripts/nextest-full.sh の `nextest-case` 行から読む。
bash scripts/nextest-full.sh >"$TMPD/test.log" 2>&1
test_status=$?
passed=$(awk -F'\t' '$1 == "nextest-case" && $2 == "PASS" { n++ } END { print n + 0 }' "$TMPD/test.log")
if [ "$test_status" -eq 0 ] && [ "${passed:-0}" -ge 2 ]; then
    pass 2 "nextest passed=$passed"
else
    tail -n 40 "$TMPD/test.log"
    fail 2 "nextest status=$test_status passed=${passed:-0} (<2 or nonzero)"
fi

# --- check 3: dependencies ---
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
    pass 3 "deps"
else
    fail 3 "deps:$dep_reasons"
fi

# --- check 4: ISA flags (values, not mere words) ---
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
    pass 4 "isa-flags"
else
    fail 4 "isa-flags:$flag_reasons"
fi

# --- check 5: runtime words, each independently ---
rt_reasons=""
for w in GC_INIT hado_entry hado_alloc GC_malloc; do
    grep -rq -- "$w" runtime/ || rt_reasons="$rt_reasons missing-$w"
done
if [ -z "$rt_reasons" ]; then
    pass 5 "runtime"
else
    fail 5 "runtime:$rt_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
