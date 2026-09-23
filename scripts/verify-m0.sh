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

# --- shared cargo test output for checks 2..4 ---
cargo_cached test --offline --no-fail-fast >"$TMPD/test.log" 2>&1
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
grep -Fq '"3"' crates/hado/tests/walk.rs || walk_reasons="$walk_reasons missing-expected-stdout-3"
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
check_set crates/hado/src/emit.rs is_pic true
check_set crates/hado/src/emit.rs preserve_frame_pointers true
check_set crates/hado/src/emit.rs opt_level none
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
# 削除したラッパー門番（`cargo test --test rN_...` を呼んでいた verify-r13.sh など）が持って
# いた検査。`cargo test` は対象ごと消えても残りを走らせて緑になるので、対象の名前が全走の
# 出力に現れることを見る（審査の指摘）。名前はディスクから導くと、消えた対象が期待からも
# 消えて素通りするため、削除した門番が守っていた組をここに固定する。
target_reasons=""
for name in \
    r9_grammar r13_conditions r14_rails r16_sections r17_files r18_spelling \
    r19_contracts r20_entries r21_functors r22_namespaces r23_syntax \
    r24_effects r25_effects r26_effects r31_effects r37_world
do
    if [ ! -f "crates/hado/tests/$name.rs" ]; then
        target_reasons="$target_reasons absent-$name"
    elif ! grep -qF "Running tests/$name.rs" "$TMPD/test.log"; then
        target_reasons="$target_reasons not-run-$name"
    else
        # 中身を空にすると `Running` は出るが 0 件になる。1 件以上を要求する。
        count=$(awk -v n="tests/$name.rs" '
            index($0, "Running " n) { seen = 1; next }
            seen && /^running [0-9]+ tests?$/ { print $2; exit }
        ' "$TMPD/test.log")
        [ "${count:-0}" -ge 1 ] || target_reasons="$target_reasons empty-$name"
    fi
done
if [ -z "$target_reasons" ]; then
    pass 8 "test-targets"
else
    fail 8 "test-targets:$target_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
