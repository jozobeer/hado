#!/usr/bin/env bash
# 門番キャッシュ（scripts/lib/gate-cache.sh）の判定器。
# 目的は 6 つ。内容が同じなら実走は 1 回。内容が変われば実走する。失敗は保存しない。
# 引数が違えば別の結果になる。走行スコープが入れ子の run-gates に漏れない。
# 古い走行のスコープを残さない。
# 加えて 33 節の層別を 2 つ（一覧が実在の門番と一致して重複なく覆う、run-gates が
# 同じ一覧を解決する）と、全走の証拠がある部分 cargo test だけ実走を省く規則を 3 つ測る。
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
runid=""
cleanup() {
    rm -rf "$workdir"
    [ -n "$runid" ] && rm -rf target/gate-cache/"$runid"-*
}
trap cleanup EXIT
runid="gate-cache-selftest-$$-$RANDOM"

replay_case() {
    local counter="$workdir/replay-counter"
    printf '0\n' >"$counter"
    HADO_GATE_CACHE_RUN="$runid-replay" bash -c '
        source scripts/lib/gate-cache.sh
        counter=$1
        bump() {
            local n
            n=$(cat "$counter")
            printf "%s\n" "$((n + 1))" >"$counter"
            printf "out-%s\n" "$n"
        }
        gate_cached demo bump
        gate_cached demo bump
    ' _ "$counter" >"$workdir/replay.out"
    [ "$(cat "$counter")" = "1" ] &&
        [ "$(wc -l <"$workdir/replay.out")" -eq 2 ] &&
        [ "$(sort -u "$workdir/replay.out")" = "out-0" ]
}

argv_case() {
    local counter="$workdir/argv-counter"
    printf '0\n' >"$counter"
    HADO_GATE_CACHE_RUN="$runid-argv" bash -c '
        source scripts/lib/gate-cache.sh
        counter=$1
        bump() {
            local n
            n=$(cat "$counter")
            printf "%s\n" "$((n + 1))" >"$counter"
        }
        gate_cached demo bump a
        gate_cached demo bump b
        gate_cached demo bump a
    ' _ "$counter"
    [ "$(cat "$counter")" = "2" ]
}

failure_case() {
    local counter="$workdir/failure-counter"
    printf '0\n' >"$counter"
    HADO_GATE_CACHE_RUN="$runid-failure" bash -c '
        source scripts/lib/gate-cache.sh
        counter=$1
        boom() {
            local n
            n=$(cat "$counter")
            printf "%s\n" "$((n + 1))" >"$counter"
            printf "boom\n"
            return 3
        }
        gate_cached demo boom
        first=$?
        gate_cached demo boom
        second=$?
        [ "$first" -eq 3 ] && [ "$second" -eq 3 ] && [ "$(cat "$counter")" = "2" ]
    ' _ "$counter" >"$workdir/failure.out"
    [ "$(cat "$counter")" = "2" ] && [ "$(wc -l <"$workdir/failure.out")" -eq 2 ]
}

content_case() {
    HADO_GATE_CACHE_RUN="$runid-content" bash -c '
        source scripts/lib/gate-cache.sh
        probe=.gate-cache-selftest-probe
        rm -f "$probe"
        before=$(hado_gate_fingerprint)
        printf "x\n" >"$probe"
        during=$(hado_gate_fingerprint)
        rm -f "$probe"
        after=$(hado_gate_fingerprint)
        [ "$before" != "$during" ] && [ "$before" = "$after" ]
    '
}

scope_case() {
    local base="$workdir/scope"
    mkdir -p "$base/scripts/lib"
    cp scripts/run-gates.sh "$base/scripts/run-gates.sh"
    cp scripts/lib/gate-cache.sh "$base/scripts/lib/gate-cache.sh"
    cat >"$base/scripts/verify-scope.sh" <<'EOS'
#!/usr/bin/env bash
printf '%s\n' "${HADO_GATE_CACHE_RUN:-unset}" >"$SCOPE_OUT"
echo 'PASSED: 1/1'
EOS
    SCOPE_OUT="$workdir/scope.out" HADO_GATE_CACHE_RUN=inherited \
        bash "$base/scripts/run-gates.sh" "$workdir/scope-logs" verify-scope >/dev/null 2>&1
    local recorded
    recorded=$(cat "$workdir/scope.out" 2>/dev/null || true)
    [ -n "$recorded" ] && [ "$recorded" != "inherited" ] && [ "$recorded" != "unset" ]
}

prune_case() {
    local base=target/gate-cache
    local old="$base/selftest-old-$$"
    local fresh="$base/selftest-fresh-$$"
    mkdir -p "$old" "$fresh"
    touch -d '2 days ago' "$old"
    HADO_GATE_CACHE_RUN= bash -c 'source scripts/lib/gate-cache.sh' >/dev/null 2>&1
    [ ! -d "$old" ] && [ -d "$fresh" ]
    local ok=$?
    rm -rf "$old" "$fresh"
    return "$ok"
}

# 層の一覧（scripts/lib/gate-tiers.sh）が実在の門番と一致し、3 層が
# 重複なく全門番を覆う（docs/plan.md 33-f）。
tier_list_case() {
    local actual listed dup
    actual=$(for f in scripts/verify-*.sh; do
        f=${f##*/}
        f=${f%.sh}
        printf '%s\n' "${f#verify-}"
    done | LC_ALL=C sort)
    listed=$(
        # shellcheck source=scripts/lib/gate-tiers.sh
        . scripts/lib/gate-tiers.sh
        hado_tier_all_gates | LC_ALL=C sort
    )
    [ "$actual" = "$listed" ] || return 1
    dup=$(
        . scripts/lib/gate-tiers.sh
        for t in quick core full; do hado_tier_minimal_gates "$t"; done | LC_ALL=C sort | uniq -d
    )
    [ -z "$dup" ]
}

# run-gates が層の一覧から同じ門番を解決し、quick ⊆ core ⊆ full、
# full == 実在の門番すべてになる（docs/plan.md 33-f）。
tier_resolve_case() {
    local actual quick core full
    actual=$(for f in scripts/verify-*.sh; do
        f=${f##*/}
        f=${f%.sh}
        printf '%s\n' "${f#verify-}"
    done | LC_ALL=C sort -u)
    quick=$(bash scripts/run-gates.sh --print-tier quick | LC_ALL=C sort -u) || return 1
    core=$(bash scripts/run-gates.sh --print-tier core | LC_ALL=C sort -u) || return 1
    full=$(bash scripts/run-gates.sh --print-tier full | LC_ALL=C sort -u) || return 1
    [ "$full" = "$actual" ] || return 1
    [ -z "$(comm -23 <(printf '%s\n' "$quick") <(printf '%s\n' "$core"))" ] || return 1
    [ -z "$(comm -23 <(printf '%s\n' "$core") <(printf '%s\n' "$full"))" ] || return 1
    [ "$quick" != "$full" ]
}

# 全走の証拠がある対象だけ、部分 cargo test の実走を省く（docs/plan.md 33 節）。
# 証拠が無い対象、-- 付き、-q 付き、混在は実走する。
narrow_reuse_case() {
    local runs="$workdir/narrow-runs" reused
    : >"$runs"
    HADO_GATE_CACHE_RUN="$runid-narrow" bash -c '
        source scripts/lib/gate-cache.sh
        runs=$1
        cargo() {
            printf "run\n" >>"$runs"
            case "$*" in
                "test --offline --no-fail-fast")
                    printf "   Running tests/r11_comments.rs (target/debug/deps/r11_comments-1)\n"
                    printf "test result: ok. 3 passed; 0 failed; 0 ignored\n"
                    printf "   Running tests/corpus.rs (target/debug/deps/corpus-1)\n"
                    printf "test result: ok. 5 passed; 0 failed; 0 ignored\n"
                    ;;
                *)
                    printf "   Running tests/x.rs (target/debug/deps/x-1)\n"
                    printf "test result: ok. 1 passed; 0 failed; 0 ignored\n"
                    ;;
            esac
            return 0
        }
        cargo_cached test --offline --no-fail-fast >/dev/null
        cargo_test_cached test --offline --test r11_comments >/dev/null
        cargo_test_cached test --offline --test r11_comments -- --nocapture >/dev/null
        cargo_test_cached test --offline --test r99_missing >/dev/null
        cargo_test_cached test --offline -q --test r11_comments >/dev/null
        cargo_test_cached test --offline --test r11_comments --test r99_missing >/dev/null
    ' _ "$runs"
    # 実走は全走 + nocapture + r99 + -q + 混在 = 5 回だけ。
    [ "$(wc -l <"$runs")" -eq 5 ] || return 1
    reused=$(HADO_GATE_CACHE_RUN="$runid-narrow" bash -c '
        source scripts/lib/gate-cache.sh
        cargo() { return 0; }
        cargo_test_cached test --offline --test r11_comments
    ' 2>/dev/null)
    printf '%s\n' "$reused" | grep -q 'Running tests/r11_comments.rs' &&
        printf '%s\n' "$reused" | grep -q 'test result: ok.'
}

# 全走が赤なら証拠は残らず、部分テストは実走する。
narrow_without_full_case() {
    local runs="$workdir/narrow-fail-runs"
    : >"$runs"
    HADO_GATE_CACHE_RUN="$runid-narrow-fail" bash -c '
        source scripts/lib/gate-cache.sh
        runs=$1
        cargo() {
            printf "run\n" >>"$runs"
            printf "   Running tests/r11_comments.rs (target/debug/deps/r11_comments-1)\n"
            printf "test result: FAILED. 0 passed; 1 failed\n"
            return 1
        }
        cargo_cached test --offline --no-fail-fast >/dev/null 2>&1
        cargo_test_cached test --offline --test r11_comments >/dev/null 2>&1
    ' _ "$runs" || true
    [ "$(wc -l <"$runs")" -eq 2 ]
}

# 対象が結果行を出さずに落ち、続く Doc-tests の ok だけがある全走は証拠にしない。
narrow_crash_case() {
    local runs="$workdir/narrow-crash-runs"
    : >"$runs"
    HADO_GATE_CACHE_RUN="$runid-narrow-crash" bash -c '
        source scripts/lib/gate-cache.sh
        runs=$1
        cargo() {
            printf "run\n" >>"$runs"
            case "$*" in
                "test --offline --no-fail-fast")
                    printf "   Running tests/r11_comments.rs (target/debug/deps/r11_comments-1)\n"
                    printf "   Doc-tests hado\n"
                    printf "test result: ok. 0 passed; 0 failed; 0 ignored\n"
                    ;;
                *)
                    printf "   Running tests/x.rs (target/debug/deps/x-1)\n"
                    printf "test result: ok. 1 passed; 0 failed; 0 ignored\n"
                    ;;
            esac
            return 0
        }
        cargo_cached test --offline --no-fail-fast >/dev/null
        cargo_test_cached test --offline --test r11_comments >/dev/null
    ' _ "$runs"
    [ "$(wc -l <"$runs")" -eq 2 ]
}

# 引数なしの全走は full と同じ扱いで、文書層（lint-docs.sh）まで回す。
default_all_docs_case() {
    local tree="$workdir/default-all" out="$workdir/default-all-out" ran="$workdir/default-all-ran"
    rm -rf -- "$tree" "$out" "$ran"
    mkdir -p "$tree/scripts/lib"
    cp scripts/run-gates.sh "$tree/scripts/run-gates.sh"
    cp scripts/lib/gate-cache.sh "$tree/scripts/lib/gate-cache.sh"
    cat >"$tree/scripts/verify-a.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 1/1'
EOS
    cat >"$tree/scripts/lint-docs.sh" <<EOS
#!/usr/bin/env bash
printf 'ran\\n' >>"$ran"
echo 'PASSED: 1/1'
EOS
    (cd "$tree" && bash scripts/run-gates.sh "$out" >/dev/null 2>&1) || return 1
    [ -f "$out/verify-a.log" ] && [ -f "$out/lint-docs.log" ] && [ "$(wc -l <"$ran")" -eq 1 ]
}

# --tier に明示した門番を足す（重複は落とす）。層だけを回して終わらない。
tier_extra_case() {
    local tree="$workdir/tier-extra" out="$workdir/tier-extra-out" log="$workdir/tier-extra-run.txt"
    rm -rf -- "$tree" "$out" "$log"
    mkdir -p "$tree/scripts/lib"
    cp scripts/run-gates.sh "$tree/scripts/run-gates.sh"
    cp scripts/lib/gate-cache.sh "$tree/scripts/lib/gate-cache.sh"
    cat >"$tree/scripts/lib/gate-tiers.sh" <<'EOS'
hado_tier_minimal_gates() {
    case "$1" in
        quick) echo quick-gate ;;
        core) echo core-gate ;;
        full) echo extra-gate ;;
    esac
}
hado_tier_gates() {
    case "$1" in
        quick) hado_tier_minimal_gates quick ;;
        core)
            hado_tier_gates quick
            hado_tier_minimal_gates core
            ;;
        full)
            hado_tier_gates core
            hado_tier_minimal_gates full
            ;;
    esac
}
hado_tier_all_gates() { hado_tier_gates full; }
EOS
    for n in quick-gate core-gate extra-gate; do
        cat >"$tree/scripts/verify-$n.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 1/1'
EOS
    done
    # quick は合成木に Cargo.toml が無いと build で落ちるので core で測る。
    (cd "$tree" && bash scripts/run-gates.sh --tier core "$out" quick-gate extra-gate) >"$log" 2>&1 || return 1
    [ "$(grep -c 'RUN: verify-quick-gate ' "$log")" -eq 1 ] &&
        [ "$(grep -c 'RUN: verify-core-gate ' "$log")" -eq 1 ] &&
        [ "$(grep -c 'RUN: verify-extra-gate ' "$log")" -eq 1 ] &&
        [ -f "$out/verify-quick-gate.log" ] && [ -f "$out/verify-extra-gate.log" ]
}

# 出力ディレクトリの相対パスは、呼び出し側の作業ディレクトリ基準で解決する。
relative_output_case() {
    local tree="$workdir/relative-tree" sub="$workdir/relative-cwd"
    rm -rf -- "$tree" "$sub"
    mkdir -p "$tree/scripts/lib" "$sub"
    cp scripts/run-gates.sh "$tree/scripts/run-gates.sh"
    cp scripts/lib/gate-cache.sh "$tree/scripts/lib/gate-cache.sh"
    cat >"$tree/scripts/verify-a.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 1/1'
EOS
    (cd "$sub" && bash "$tree/scripts/run-gates.sh" relout verify-a >/dev/null 2>&1) || return 1
    [ -f "$sub/relout/verify-a.log" ] && [ ! -e "$tree/relout" ]
}

check 'cache replays identical content once' replay_case
check 'cache separates different arguments' argv_case
check 'cache never stores failure' failure_case
check 'fingerprint follows content' content_case
check 'run-gates gives each run its own scope' scope_case
check 'stale run scopes are pruned' prune_case
check 'tier list matches the gate scripts and partitions them' tier_list_case
check 'run-gates resolves the tiers from the single list' tier_resolve_case
check 'narrow cargo tests reuse the full test only with evidence' narrow_reuse_case
check 'narrow cargo tests run when the full test failed' narrow_without_full_case
check 'narrow cargo tests ignore a Doc-tests ok after a crash' narrow_crash_case
check 'argument-less run also runs the docs layer' default_all_docs_case
check 'explicit gates are added to a tier without duplicates' tier_extra_case
check 'a relative output directory is resolved from the caller cwd' relative_output_case

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
