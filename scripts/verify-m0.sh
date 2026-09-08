#!/bin/bash
# M0 acceptance checks 1..7 (see docs/plan.md, M0). Run from the repository root.
# Each check is independent; all run to the end. Non-zero exit if any fails.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=7
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

# --- shared cargo test output for checks 2..4 ---
cargo test >"$TMPD/test.log" 2>&1
test_status=$?
passed=$(grep -E "test result:" "$TMPD/test.log" | grep -oE "[0-9]+ passed" | grep -oE "[0-9]+" | awk '{s+=$1} END{print s+0}')
walk_ok=$(awk '/tests.walk.rs/{f=1} f && /test result: ok/{ok=1; f=0} f && /test result: FAILED/{f=0} END{print ok+0}' "$TMPD/test.log")
tail_ok=$(awk '/tests.tail.rs/{f=1} f && /test result: ok/{ok=1; f=0} f && /test result: FAILED/{f=0} END{print ok+0}' "$TMPD/test.log")

# --- check 2: cargo test passes >= 2 with exit 0 ---
if [ "$test_status" -eq 0 ] && [ "${passed:-0}" -ge 2 ]; then
    pass 2 "cargo-test passed=$passed"
else
    fail 2 "cargo-test status=$test_status passed=${passed:-0} (<2 or nonzero)"
fi

# --- check 3: walk ---
walk_reasons=""
grep -Fq '"3"' crates/shin/tests/walk.rs || walk_reasons="$walk_reasons missing-expected-stdout-3"
[ "$walk_ok" = "1" ] || walk_reasons="$walk_reasons walk-test-not-passing"
if [ -z "$walk_reasons" ]; then
    pass 3 "walk"
else
    fail 3 "walk:$walk_reasons"
fi

# --- check 4: tail ---
tail_reasons=""
grep -Eq '100_000_000|100000000' crates/shin/tests/tail.rs || tail_reasons="$tail_reasons missing-depth-const"
[ "$tail_ok" = "1" ] || tail_reasons="$tail_reasons tail-test-not-passing"
if [ -z "$tail_reasons" ]; then
    pass 4 "tail"
else
    fail 4 "tail:$tail_reasons"
fi

# --- check 5: dependencies ---
dep_reasons=""
for c in cranelift-codegen cranelift-frontend cranelift-module cranelift-object cranelift-native; do
    grep -Eq "^${c}[[:space:]]*=[[:space:]]*\"0\.135\"" crates/shin/Cargo.toml \
        || dep_reasons="$dep_reasons $c!=0.135"
done
if awk '/^\[/{sec=$0} sec=="[build-dependencies]" && /^cc[[:space:]]*=[[:space:]]*"1"/{found=1} END{exit !found}' crates/shin/Cargo.toml; then
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
check_set crates/shin/src/emit.rs is_pic true
check_set crates/shin/src/emit.rs preserve_frame_pointers true
check_set crates/shin/src/emit.rs opt_level none
check_set crates/shin/tests/tail.rs is_pic true
check_set crates/shin/tests/tail.rs preserve_frame_pointers true
check_set crates/shin/tests/tail.rs opt_level none
if grep -rEq 'set\([[:space:]]*"enable_verifier"[[:space:]]*,[[:space:]]*"false"[[:space:]]*\)' crates/shin; then
    flag_reasons="$flag_reasons enable_verifier=false-present"
fi
if [ -z "$flag_reasons" ]; then
    pass 6 "isa-flags"
else
    fail 6 "isa-flags:$flag_reasons"
fi

# --- check 7: runtime words, each independently ---
rt_reasons=""
for w in GC_INIT shin_entry shin_alloc GC_malloc; do
    grep -rq -- "$w" runtime/ || rt_reasons="$rt_reasons missing-$w"
done
if [ -z "$rt_reasons" ]; then
    pass 7 "runtime"
else
    fail 7 "runtime:$rt_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
