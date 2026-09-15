#!/usr/bin/env bash
# 門番キャッシュ（scripts/lib/gate-cache.sh）の判定器。
# 目的は 6 つ。内容が同じなら実走は 1 回。内容が変われば実走する。失敗は保存しない。
# 引数が違えば別の結果になる。走行スコープが入れ子の run-gates に漏れない。
# 古い走行のスコープを残さない。
# 加えて 33 節の層別を 2 つ（一覧が実在の門番と一致して重複なく覆う、run-gates が
# 同じ一覧を解決する）と、全走の証拠がある部分 cargo test だけ実走を省く規則を 3 つ測る。
# 34 節の並列実行では、同一コマンドの並行呼び出しが 1 回だけ実走すること、公開が
# 原子的であること、run-gates と入れ子が同じ label で 1 つの鍵を共有すること、
# 直列の一覧の門番が fan-out より先に完走すること、--jobs が直列と同じ PASSED を
# 出し赤を隠さないこと、並列度の上限を守ること、HADO_GATE_TIMES が門番ごとに 1 行に
# なること、不正な --jobs を拒否することを測る。
set -uo pipefail
cd "$(dirname "$0")/.."
# 自己判定は外側の走行の診断（34-g の HADO_GATE_RUNS）に混ざらない。
unset HADO_GATE_RUNS

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
    local actual listed dup serial
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
    [ -z "$dup" ] || return 1
    # 直列の一覧（--jobs の前段）は空でなく、実在の門番の部分集合である。
    serial=$(
        . scripts/lib/gate-tiers.sh
        hado_tier_serial_gates | LC_ALL=C sort
    )
    [ -n "$serial" ] || return 1
    [ -z "$(comm -23 <(printf '%s\n' "$serial") <(printf '%s\n' "$listed"))" ]
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

# 同じ内容・同じ引数の gate_cached を 8 本並行に呼んでも、実走は 1 回だけ。
# flock が無い（check-then-run の）実装では 8 回走る（docs/plan.md 34 節）。
concurrent_once_case() {
    local runs="$workdir/concurrent-runs"
    : >"$runs"
    HADO_GATE_CACHE_RUN="$runid-concurrent" bash -c '
        source scripts/lib/gate-cache.sh
        runs=$1
        outdir=$2
        bump() {
            printf "run\n" >>"$runs"
            sleep 0.3
            printf "out\n"
        }
        for i in 1 2 3 4 5 6 7 8; do
            gate_cached demo bump >"$outdir/concurrent.$i" 2>&1 &
        done
        wait
    ' _ "$runs" "$workdir" || return 1
    [ "$(wc -l <"$runs")" -eq 1 ] || return 1
    local i
    for i in 1 2 3 4 5 6 7 8; do
        [ "$(cat "$workdir/concurrent.$i")" = "out" ] || return 1
    done
}

# 公開は out → status の順の rename。読み手は部分的な out を一度も見ない。
# 実走は 1 回だけ（読み手が先に lock を取っても同じ）。
atomic_publish_case() {
    local runs="$workdir/atomic-runs" bad="$workdir/atomic-bad" order="$workdir/atomic-order"
    : >"$runs"
    : >"$order"
    rm -f "$bad"
    HADO_GATE_CACHE_RUN="$runid-atomic" bash -c '
        source scripts/lib/gate-cache.sh
        runs=$1
        bad=$2
        order=$3
        # mv を包んで、公開の順（out → status）を記録する。順序が逆だと読み手が
        # status=0 を見た瞬間に空の out を読む。
        mv() {
            printf "%s\n" "${!#}" >>"$order"
            command mv "$@"
        }
        emit() {
            printf "run\n" >>"$runs"
            local i
            for i in $(seq 30); do printf "line-%s\n" "$i"; sleep 0.01; done
        }
        gate_cached big emit >/dev/null 2>&1 &
        writer=$!
        i=0
        while [ "$i" -lt 30 ]; do
            got=$(gate_cached big emit 2>/dev/null)
            if [ -n "$got" ]; then
                n=$(printf "%s\n" "$got" | grep -c "^line-")
                [ "$n" -eq 30 ] || printf "partial\n" >"$bad"
            fi
            i=$((i + 1))
        done
        wait "$writer"
        [ "$(basename "$(sed -n 1p "$order")")" = out ] || printf "order\n" >"$bad"
        [ "$(basename "$(sed -n 2p "$order")")" = status ] || printf "order\n" >"$bad"
    ' _ "$runs" "$bad" "$order" || true
    [ ! -f "$bad" ] && [ "$(wc -l <"$runs")" -eq 1 ] && [ "$(wc -l <"$order")" -eq 2 ]
}

# run-gates が門番 x を走らせた結果を、x を再入する門番 y が同じ label で引ける。
# label が割れている（verify-verify-x と verify-x）と x が 2 回実走する。
label_agreement_case() {
    local tree="$workdir/label-tree" out="$workdir/label-out" runs="$workdir/label-runs"
    rm -rf -- "$tree" "$out" "$runs"
    : >"$runs"
    mkdir -p "$tree/scripts/lib"
    cp scripts/run-gates.sh "$tree/scripts/run-gates.sh"
    cp scripts/lib/gate-cache.sh "$tree/scripts/lib/gate-cache.sh"
    cat >"$tree/scripts/verify-x.sh" <<EOS
#!/usr/bin/env bash
printf 'run\n' >>"$runs"
echo 'PASSED: 1/1'
EOS
    cat >"$tree/scripts/verify-y.sh" <<EOS
#!/usr/bin/env bash
. scripts/lib/gate-cache.sh
gate_cached verify-x bash scripts/verify-x.sh >/dev/null 2>&1
echo 'PASSED: 1/1'
EOS
    (cd "$tree" && bash scripts/run-gates.sh "$out" verify-x verify-y) >/dev/null 2>&1 || return 1
    [ "$(wc -l <"$runs")" -eq 1 ]
}

# 並列の門番を合成木に用意する。呼び出し側が -out と --jobs を渡す。
make_parallel_tree() {
    local tree=$1 runs=$2
    rm -rf -- "$tree" "$runs"
    : >"$runs"
    mkdir -p "$tree/scripts/lib"
    cp scripts/run-gates.sh "$tree/scripts/run-gates.sh"
    cp scripts/lib/gate-cache.sh "$tree/scripts/lib/gate-cache.sh"
}

# --jobs 2 が直列と同じ結果を出し、各門番が 1 回ずつ走る。出力先は平坦な .log だけ。
parallel_matches_serial_case() {
    local tree="$workdir/parallel-tree" runs="$workdir/parallel-runs" g f
    make_parallel_tree "$tree" "$runs" || return 1
    for g in a b c d; do
        cat >"$tree/scripts/verify-$g.sh" <<EOS
#!/usr/bin/env bash
printf '%s\n' '$g' >>"$runs"
sleep 0.05
echo 'PASSED: 1/1'
EOS
    done
    (cd "$tree" && bash scripts/run-gates.sh "$workdir/parallel-serial" verify-a verify-b verify-c verify-d) >/dev/null 2>&1 || return 1
    (cd "$tree" && bash scripts/run-gates.sh --jobs 2 "$workdir/parallel-jobs2" verify-a verify-b verify-c verify-d) >/dev/null 2>&1 || return 1
    for g in a b c d; do
        [ "$(grep -c "^$g$" "$runs")" -eq 2 ] || return 1
    done
    [ "$(ls "$workdir/parallel-jobs2" | wc -l)" -eq 4 ] || return 1
    for f in "$workdir/parallel-jobs2"/*; do
        case "${f##*/}" in *.log) ;; *) return 1 ;; esac
        grep -qx 'PASSED: 1/1' "$f" || return 1
    done
}

# 後段の同時実行数が --jobs を超えない。6 本を 2 並列で走らせ、同時実行の最大が 2。
parallel_pool_case() {
    local tree="$workdir/pool-tree" state="$workdir/pool-state" g
    make_parallel_tree "$tree" "$workdir/pool-runs" || return 1
    mkdir -p "$state"
    echo 0 >"$state/cur"
    echo 0 >"$state/max"
    for g in a b c d e f; do
        cat >"$tree/scripts/verify-$g.sh" <<EOS
#!/usr/bin/env bash
{
    flock 9
    n=\$(cat "$state/cur")
    n=\$((n + 1))
    echo "\$n" >"$state/cur"
    m=\$(cat "$state/max")
    [ "\$n" -gt "\$m" ] && echo "\$n" >"$state/max"
} 9>"$state/lock"
sleep 0.4
{
    flock 9
    n=\$(cat "$state/cur")
    echo "\$((n - 1))" >"$state/cur"
} 9>"$state/lock"
echo 'PASSED: 1/1'
EOS
    done
    (cd "$tree" && bash scripts/run-gates.sh --jobs 2 "$workdir/pool-out" \
        verify-a verify-b verify-c verify-d verify-e verify-f) >/dev/null 2>&1 || return 1
    [ "$(cat "$state/max")" -eq 2 ]
}

# 直列の一覧（hado_tier_serial_gates）の門番が fan-out より先に完走する（前段の barrier）。
parallel_barrier_case() {
    local tree="$workdir/barrier-tree" runs="$workdir/barrier-runs" g rest
    make_parallel_tree "$tree" "$runs" || return 1
    cat >"$tree/scripts/lib/gate-tiers.sh" <<'EOS'
hado_tier_serial_gates() { printf '%s\n' m0 mutator; }
EOS
    cat >"$tree/scripts/verify-m0.sh" <<EOS
#!/usr/bin/env bash
printf 'm0\n' >>"$runs"
sleep 0.3
: >"$workdir/barrier-marker"
echo 'PASSED: 1/1'
EOS
    cat >"$tree/scripts/verify-mutator.sh" <<EOS
#!/usr/bin/env bash
printf 'mutator\n' >>"$runs"
sleep 0.3
: >"$workdir/barrier-marker"
echo 'PASSED: 1/1'
EOS
    for g in a b; do
        cat >"$tree/scripts/verify-$g.sh" <<EOS
#!/usr/bin/env bash
[ -f "$workdir/barrier-marker" ] || exit 1
printf '%s\n' '$g' >>"$runs"
echo 'PASSED: 1/1'
EOS
    done
    (cd "$tree" && bash scripts/run-gates.sh --jobs 3 "$workdir/barrier-out" verify-a verify-b verify-m0 verify-mutator) >/dev/null 2>&1 || return 1
    [ "$(sed -n '1p' "$runs")" = m0 ] || return 1
    [ "$(sed -n '2p' "$runs")" = mutator ] || return 1
    rest=$(sed -n '3,4p' "$runs" | LC_ALL=C sort | tr '\n' ' ')
    [ "$rest" = "a b " ]
}

# 後段がツリーを恒久的に変えたら、guard が分類漏れとして赤にする（34-h の対照）。
parallel_tree_guard_case() {
    local tree="$workdir/guard-tree" out="$workdir/guard-out" status=0
    make_parallel_tree "$tree" "$workdir/guard-runs" || return 1
    cat >"$tree/scripts/verify-a.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 1/1'
EOS
    cat >"$tree/scripts/verify-leak.sh" <<EOS
#!/usr/bin/env bash
printf 'leak\n' >"$tree/scripts/leak.txt"
echo 'PASSED: 1/1'
EOS
    (cd "$tree" && bash scripts/run-gates.sh --jobs 2 "$out" verify-a verify-leak) \
        >"$workdir/guard-stdout" 2>"$workdir/guard-stderr" || status=$?
    [ "$status" -eq 1 ] || return 1
    grep -q 'the working tree changed during the parallel phase' "$workdir/guard-stderr"
}

# 出力先がツリーの中でも、走行自身のログで安定性 guard を誤爆させない。
parallel_output_in_tree_case() {
    local tree="$workdir/intree-tree" out="$workdir/intree-tree/gate-out" g
    make_parallel_tree "$tree" "$workdir/intree-runs" || return 1
    for g in a b c d; do
        cat >"$tree/scripts/verify-$g.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 1/1'
EOS
    done
    (cd "$tree" && bash scripts/run-gates.sh --jobs 2 gate-out verify-a verify-b verify-c verify-d) \
        >"$workdir/intree-stdout" 2>"$workdir/intree-stderr" || return 1
    [ "$(ls "$out" | wc -l)" -eq 4 ] || return 1
    [ ! -s "$workdir/intree-stderr" ]
}

# 前段に赤があれば後段は直列に落ちる（赤い門番が残した probe で鍵が揺れても
# 入れ子を再走させない）。判定は既に赤なので、緩めずに直列へ落ちることだけを見る。
parallel_prelude_red_case() {
    local tree="$workdir/redprel-tree" out="$workdir/redprel-out" status=0 g
    make_parallel_tree "$tree" "$workdir/redprel-runs" || return 1
    cat >"$tree/scripts/lib/gate-tiers.sh" <<'EOS'
hado_tier_serial_gates() { printf '%s\n' bad; }
EOS
    cat >"$tree/scripts/verify-bad.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 0/1'
exit 1
EOS
    for g in a b; do
        cat >"$tree/scripts/verify-$g.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 1/1'
EOS
    done
    (cd "$tree" && bash scripts/run-gates.sh --jobs 3 "$out" verify-a verify-b verify-bad) \
        >"$workdir/redprel-stdout" 2>"$workdir/redprel-stderr" || status=$?
    [ "$status" -eq 1 ] || return 1
    grep -q 'running the rest serially' "$workdir/redprel-stderr" || return 1
    [ "$(ls "$out" | wc -l)" -eq 3 ]
}

# 赤を隠さない。1 本でも赤なら exit 1 で、赤のログは平坦な場所に残る。
parallel_red_case() {
    local tree="$workdir/red-tree" out="$workdir/red-out" g status=0
    make_parallel_tree "$tree" "$workdir/red-runs" || return 1
    for g in a b c; do
        cat >"$tree/scripts/verify-$g.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 1/1'
EOS
    done
    cat >"$tree/scripts/verify-red.sh" <<'EOS'
#!/usr/bin/env bash
echo 'PASSED: 0/1'
exit 1
EOS
    (cd "$tree" && bash scripts/run-gates.sh --jobs 4 "$out" verify-a verify-b verify-c verify-red) >/dev/null 2>&1 || status=$?
    [ "$status" -eq 1 ] || return 1
    [ "$(ls "$out" | wc -l)" -eq 4 ] || return 1
    grep -qx 'PASSED: 0/1' "$out/verify-red.log"
}

# HADO_GATE_TIMES は並列でも門番ごとに 1 行だけ。最外の run-gates が記録する。
parallel_times_case() {
    local tree="$workdir/times-tree" times="$workdir/times.tsv"
    make_parallel_tree "$tree" "$workdir/times-runs" || return 1
    local g
    for g in a b c; do
        cat >"$tree/scripts/verify-$g.sh" <<'EOS'
#!/usr/bin/env bash
sleep 0.05
echo 'PASSED: 1/1'
EOS
    done
    (cd "$tree" && env -u HADO_GATE_RUN_ACTIVE -u HADO_GATE_RUNS HADO_GATE_TIMES="$times" \
        bash scripts/run-gates.sh --jobs 3 "$workdir/times-out" verify-a verify-b verify-c) >/dev/null 2>&1 || return 1
    [ "$(wc -l <"$times")" -eq 3 ] || return 1
    [ "$(cut -f2 "$times" | LC_ALL=C sort -u)" = "0" ] || return 1
    [ "$(cut -f3 "$times" | LC_ALL=C sort | tr '\n' ' ')" = "verify-a verify-b verify-c " ]
}

# --jobs は正の整数だけ。0・負・非整数・欠落・--jobs=0 は usage で落ち、
# 出力ディレクトリを作らない。
jobs_invalid_case() {
    local bad out
    for bad in 0 -1 x 1.5 ""; do
        out="$workdir/jobs-invalid-$bad"
        rm -rf -- "$out"
        if bash scripts/run-gates.sh --jobs "$bad" "$out" verify-a >/dev/null 2>"$workdir/jobs-err"; then
            return 1
        fi
        [ ! -e "$out" ] || return 1
    done
    if bash scripts/run-gates.sh --jobs >/dev/null 2>"$workdir/jobs-err"; then return 1; fi
    out="$workdir/jobs-invalid-eq"
    rm -rf -- "$out"
    if bash scripts/run-gates.sh --jobs=0 "$out" verify-a >/dev/null 2>"$workdir/jobs-err"; then return 1; fi
    [ ! -e "$out" ]
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
check 'concurrent callers of one command run it once' concurrent_once_case
check 'cache publishes complete entries only' atomic_publish_case
check 'runner and nested gates share one cache label' label_agreement_case
check '--jobs 2 matches the serial results and runs each gate once' parallel_matches_serial_case
check 'the parallel fan-out respects the --jobs limit' parallel_pool_case
check 'serial gates finish before the parallel fan-out' parallel_barrier_case
check 'a gate that changes the tree trips the stability guard' parallel_tree_guard_case
check 'an output directory inside the tree does not trip the guard' parallel_output_in_tree_case
check 'a red serial gate drops the fan-out back to serial' parallel_prelude_red_case
check 'a red gate is not hidden under --jobs' parallel_red_case
check 'HADO_GATE_TIMES has one line per gate under --jobs' parallel_times_case
check '--jobs rejects values that are not positive integers' jobs_invalid_case

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
