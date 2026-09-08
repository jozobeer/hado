#!/bin/bash
# M1 corpus acceptance checks 1..7 (see docs/plan.md, M1). Run from the repository root.
# Each check is independent; all run to the end. Non-zero exit if any fails.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

ok=0
total=7
pass() { echo "OK: $1 $2"; ok=$((ok + 1)); }
fail() { echo "NG: $1 $2"; }

SPEC="docs/spec.md"
COV="docs/spec-coverage.md"
PLAN="docs/plan.md"
CORPUS_RS="crates/hado/tests/corpus.rs"

TMPD="$(mktemp -d)"
find docs runtime scripts -type f -print0 | sort -z | xargs -0 sha256sum > "$TMPD/protected.sha"
trap 'rm -rf "$TMPD"' EXIT

# --- shared: section-50 code table (same extraction as verify-m1-spec.sh:79-95) ---
awk '
    /^# 50\./ { on=1 }
    on { print }
' "$SPEC" >"$TMPD/sec50.txt"
awk -F '|' '
    {
        code=$2
        name=$3
        secs=$4
        gsub(/^ +| +$/, "", code)
        gsub(/^ +| +$/, "", name)
        gsub(/^ +| +$/, "", secs)
        if (code ~ /^E[1-7][0-9][0-9]$/) {
            print code "\t" name "\t" secs
        }
    }
' "$TMPD/sec50.txt" >"$TMPD/codes.tsv"

# --- shared: corpus item names (coverage column extraction, 2.2, 9.2, 11.2 and 13.2 rows) ---
awk -F '|' '
    {
        t=$2; name=$3
        gsub(/^ +| +$/, "", t)
        gsub(/^ +| +$/, "", name)
        if (t != "2.2" && t != "9.2" && t != "11.2" && t != "13.2") next
        if (name == "" || name == "項目名") next
        print name
    }
' "$COV" >"$TMPD/items22.txt"

# --- check 1: reject corpus is 1:1 with the section-50 codes (count derived from the table) ---
c1_reasons=""
n_reject=0
if [ -d tests/reject ]; then
    n_reject=$(ls tests/reject/*/main.hd 2>/dev/null | wc -l | tr -d ' ')
fi
n_codes=$(cut -f1 "$TMPD/codes.tsv" | sort -u | wc -l | tr -d ' ')
if [ "$n_reject" -ne "$n_codes" ]; then
    c1_reasons="$c1_reasons reject-count=$n_reject(want-$n_codes)"
fi
: >"$TMPD/corpus_codes.txt"
for f in tests/reject/*/main.hd; do
    [ -e "$f" ] || continue
    n_err=$(grep -cE '^// error: E[1-7][0-9][0-9]$' "$f" || true)
    if [ "$n_err" -ne 1 ]; then
        c1_reasons="$c1_reasons $f:error-lines=$n_err"
    fi
    grep -E '^// error: E[1-7][0-9][0-9]$' "$f" 2>/dev/null | sed 's|^// error: ||' >>"$TMPD/corpus_codes.txt" || true
    n_spec=$(grep -cE '^// spec: §[0-9]+$' "$f" || true)
    if [ "$n_spec" -ne 1 ]; then
        c1_reasons="$c1_reasons $f:spec-lines=$n_spec"
    else
        specval=$(grep -E '^// spec: §[0-9]+$' "$f" | sed 's|^// spec: ||')
        errcode=$(grep -E '^// error: E[1-7][0-9][0-9]$' "$f" 2>/dev/null | sed 's|^// error: ||' | head -n 1)
        if [ -n "$errcode" ]; then
            wantsec=$(awk -F '\t' -v c="$errcode" '$1==c {print $3}' "$TMPD/codes.tsv" | head -n 1)
            if [ -z "$wantsec" ]; then
                c1_reasons="$c1_reasons $f:code-$errcode-not-in-sec50"
            elif [ "$specval" != "$wantsec" ]; then
                c1_reasons="$c1_reasons $f:spec-$specval!=table-$wantsec"
            fi
        fi
    fi
done
cut -f1 "$TMPD/codes.tsv" | sort -u >"$TMPD/spec_codes.txt"
sort -u "$TMPD/corpus_codes.txt" >"$TMPD/corpus_uniq.txt" 2>/dev/null || : >"$TMPD/corpus_uniq.txt"
if [ -s "$TMPD/spec_codes.txt" ] || [ -s "$TMPD/corpus_uniq.txt" ]; then
    only_spec=$(comm -23 "$TMPD/spec_codes.txt" "$TMPD/corpus_uniq.txt" | tr '\n' ' ')
    only_corpus=$(comm -13 "$TMPD/spec_codes.txt" "$TMPD/corpus_uniq.txt" | tr '\n' ' ')
    [ -z "$only_spec" ] || c1_reasons="$c1_reasons missing-from-corpus:$only_spec"
    [ -z "$only_corpus" ] || c1_reasons="$c1_reasons extra-in-corpus:$only_corpus"
fi
if [ -z "$c1_reasons" ]; then
    pass 1 "reject-1:1"
else
    fail 1 "reject-1:1:$c1_reasons"
fi

# --- check 2: the section-39 examples (first line of each code block) are in the accept corpus ---
c2_reasons=""
awk '/^# 39\./{f=1} /^# 40\./{f=0} f && /^```/{inb=!inb; first=inb; next} f && inb && first {print; first=0}' "$SPEC" >"$TMPD/sec39.txt"
[ -s "$TMPD/sec39.txt" ] || c2_reasons="$c2_reasons no-sec39-examples"
while IFS= read -r s; do
    [ -n "$s" ] || continue
    found=0
    for f in tests/accept/*/main.hd; do
        [ -e "$f" ] || continue
        if grep -qxF -- "$s" "$f" && grep -qE '^// spec: §39$' "$f"; then
            found=1
            break
        fi
    done
    [ "$found" -eq 1 ] || c2_reasons="$c2_reasons missing-line:$s"
done <"$TMPD/sec39.txt"
for f in tests/accept/*/main.hd; do
    [ -e "$f" ] || continue
    if grep -qxF 'def display' "$f"; then
        grep -qF '| Guest => "guest"' "$f" || c2_reasons="$c2_reasons $f:missing-guest-branch"
        grep -qE '^// expect: guest$' "$f" || c2_reasons="$c2_reasons $f:missing-expect-guest"
    fi
    if grep -qxF 'def active_names users =>' "$f"; then
        grep -qE '^// needs: prelude$' "$f" || c2_reasons="$c2_reasons $f:missing-needs-prelude"
    fi
done
if [ -z "$c2_reasons" ]; then
    pass 2 "sec39-examples"
else
    fail 2 "sec39-examples:$c2_reasons"
fi

# --- check 3: accept covers all items of plan 2.2, 9.2, 11.2 and 13.2; reject covers all but the 2 exempt ---
c3_reasons=""
n_items=$(wc -l <"$TMPD/items22.txt" | tr -d ' ')
n_plan=$(awk -F '|' '
    /^### 2\.2/ || /^### 9\.2/ || /^### 11\.2/ || /^### 13\.2/ { p=1; next }
    /^### 2\.3/ || /^### 9\.3/ || /^### 11\.3/ || /^### 13\.3/ { p=0 }
    p {
        name=$2
        gsub(/^ +| +$/, "", name)
        if (name=="" || name=="項目" || name ~ /^-+$/) next
        if ($0 ~ /^\|/) n++
    }
    END { print n+0 }
' "$PLAN")
if [ "$n_items" -ne "$n_plan" ]; then
    c3_reasons="$c3_reasons item-count=$n_items(want-$n_plan)"
fi
while IFS= read -r name || [ -n "$name" ]; do
    [ -n "$name" ] || continue
    na=0
    nr=0
    for f in tests/accept/*/main.hd; do
        [ -e "$f" ] || continue
        grep -qFx -- "// item: $name" "$f" && na=$((na + 1))
    done
    for f in tests/reject/*/main.hd; do
        [ -e "$f" ] || continue
        grep -qFx -- "// item: $name" "$f" && nr=$((nr + 1))
    done
    [ "$na" -ge 1 ] || c3_reasons="$c3_reasons accept-missing:$name"
    # Reject-exempt (no rejectable form by definition, docs/plan.md M1):
    # - 冗長の対象外: permission ("eta expansion is not redundant"), §2
    # - 末尾呼び出し: guarantee ("tail calls consume no stack"), §48
    if [ "$name" != "冗長の対象外" ] && [ "$name" != "末尾呼び出し" ]; then
        [ "$nr" -ge 1 ] || c3_reasons="$c3_reasons reject-missing:$name"
    fi
done <"$TMPD/items22.txt"
: >"$TMPD/all_items.txt"
for f in tests/accept/*/main.hd tests/reject/*/main.hd; do
    [ -e "$f" ] || continue
    grep -E '^// item: ' "$f" 2>/dev/null | sed 's|^// item: ||' >>"$TMPD/all_items.txt" || true
done
sort -u "$TMPD/all_items.txt" >"$TMPD/all_items_uniq.txt"
sort -u "$TMPD/items22.txt" >"$TMPD/items22_uniq.txt"
while IFS= read -r v || [ -n "$v" ]; do
    [ -n "$v" ] || continue
    grep -qFx -- "$v" "$TMPD/items22_uniq.txt" || c3_reasons="$c3_reasons unknown-item:$v"
done <"$TMPD/all_items_uniq.txt"
if [ -z "$c3_reasons" ]; then
    pass 3 "pairs"
else
    fail 3 "pairs:$c3_reasons"
fi

# --- check 4: short-circuit accept corpus for && and || ---
c4_reasons=""
and_ok=0
or_ok=0
for f in tests/accept/*/main.hd; do
    [ -e "$f" ] || continue
    if grep -qE '^// short-circuit: &&$' "$f" && grep -qF 'def rec' "$f" && grep -qF 'false &&' "$f"; then
        and_ok=1
    fi
    if grep -qE '^// short-circuit: \|\|$' "$f" && grep -qF 'def rec' "$f" && grep -qF 'true ||' "$f"; then
        or_ok=1
    fi
done
[ "$and_ok" -eq 1 ] || c4_reasons="$c4_reasons missing-&&-file"
[ "$or_ok" -eq 1 ] || c4_reasons="$c4_reasons missing-||-file"
if [ -z "$c4_reasons" ]; then
    pass 4 "short-circuit"
else
    fail 4 "short-circuit:$c4_reasons"
fi

# --- check 5: cargo test --test corpus enumerates every file ---
c5_reasons=""
if [ ! -f "$CORPUS_RS" ]; then
    c5_reasons="$c5_reasons missing-corpus.rs"
else
    n_accept=0
    n_reject_disk=0
    if [ -d tests/accept ]; then
        n_accept=$(ls tests/accept/*/main.hd 2>/dev/null | wc -l | tr -d ' ')
    fi
    if [ -d tests/reject ]; then
        n_reject_disk=$(ls tests/reject/*/main.hd 2>/dev/null | wc -l | tr -d ' ')
    fi
    want_n=$((n_accept + n_reject_disk))
    (cd tests/accept 2>/dev/null && ls */main.hd 2>/dev/null | sed 's|^|tests/accept/|' | sort >"$TMPD/disk_accept.txt") || : >"$TMPD/disk_accept.txt"
    (cd tests/reject 2>/dev/null && ls */main.hd 2>/dev/null | sed 's|^|tests/reject/|' | sort >"$TMPD/disk_reject.txt") || : >"$TMPD/disk_reject.txt"
    cat "$TMPD/disk_accept.txt" "$TMPD/disk_reject.txt" | sort >"$TMPD/disk_all.txt"

    cargo test --test corpus -- --nocapture >"$TMPD/corpus.log" 2>&1
    corpus_status=$?
    if [ "$corpus_status" -ne 0 ]; then
        c5_reasons="$c5_reasons cargo-test-corpus-exit=$corpus_status"
    fi
    if ! grep -qFx "corpus_enumerated: $want_n" "$TMPD/corpus.log"; then
        got=$(grep -E '^corpus_enumerated: ' "$TMPD/corpus.log" | head -n 1)
        c5_reasons="$c5_reasons enumerated!=disk-$want_n(got:$got)"
    fi
    grep -E '^corpus_file: ' "$TMPD/corpus.log" | sed 's/^corpus_file: //' | sort -u >"$TMPD/out_files.txt"
    if [ -s "$TMPD/disk_all.txt" ] || [ -s "$TMPD/out_files.txt" ]; then
        d_only_disk=$(comm -23 "$TMPD/disk_all.txt" "$TMPD/out_files.txt" | tr '\n' ' ')
        d_only_out=$(comm -13 "$TMPD/disk_all.txt" "$TMPD/out_files.txt" | tr '\n' ' ')
        [ -z "$d_only_disk" ] || c5_reasons="$c5_reasons missing-from-output:$d_only_disk"
        [ -z "$d_only_out" ] || c5_reasons="$c5_reasons extra-in-output:$d_only_out"
    fi
    for lit in 'read_dir' 'tests/accept' 'tests/reject' 'corpus_enumerated:' 'corpus_file:' 'compile_source_to_object' 'link_executable' '// expect:' '// error:'; do
        grep -qF -- "$lit" "$CORPUS_RS" || c5_reasons="$c5_reasons corpus.rs-missing:$lit"
    done
    for f in tests/accept/*/main.hd tests/reject/*/main.hd; do
        [ -e "$f" ] || continue
        base=$(basename "$(dirname "$f")")
        if grep -qF -- "\"$base\"" "$CORPUS_RS"; then
            c5_reasons="$c5_reasons corpus.rs-quotes-basename:$base"
        fi
    done
    # predicate 7: probe behaviour (enumeration, not a hard-coded count)
    mkdir -p tests/accept/__probe__
    printf -- '// spec: §39\n// expect: 3\ndef main = 1 + 2\n' >tests/accept/__probe__/main.hd
    cargo test --test corpus -- --nocapture >"$TMPD/corpus_probe.log" 2>&1
    probe_status=$?
    rm -f tests/accept/__probe__/main.hd
    rmdir tests/accept/__probe__
    want_probe=$((want_n + 1))
    if [ "$probe_status" -ne 0 ]; then
        c5_reasons="$c5_reasons probe-exit=$probe_status"
    fi
    if ! grep -qFx "corpus_enumerated: $want_probe" "$TMPD/corpus_probe.log"; then
        got=$(grep -E '^corpus_enumerated: ' "$TMPD/corpus_probe.log" | head -n 1)
        c5_reasons="$c5_reasons probe-enumerated!=disk-$want_probe(got:$got)"
    fi
    if ! grep -qFx 'corpus_file: tests/accept/__probe__/main.hd' "$TMPD/corpus_probe.log"; then
        c5_reasons="$c5_reasons probe-path-missing"
    fi
    cargo test --test corpus -- --nocapture >"$TMPD/corpus_back.log" 2>&1
    if ! grep -qFx "corpus_enumerated: $want_n" "$TMPD/corpus_back.log"; then
        got=$(grep -E '^corpus_enumerated: ' "$TMPD/corpus_back.log" | head -n 1)
        c5_reasons="$c5_reasons probe-removal-not-restored(got:$got)"
    fi
fi
if [ -z "$c5_reasons" ]; then
    pass 5 "corpus-enumeration"
else
    fail 5 "corpus-enumeration:$c5_reasons"
fi

# --- check 6: header conventions and // needs: prelude ---
c6_reasons=""
total_hado=$(ls tests/accept/*/main.hd tests/reject/*/main.hd 2>/dev/null | wc -l | tr -d ' ')
if [ "${total_hado:-0}" -eq 0 ]; then
    c6_reasons="$c6_reasons no-hado-files"
fi
for f in tests/accept/*/main.hd tests/reject/*/main.hd; do
    [ -e "$f" ] || continue
    first=$(grep -vE '^[[:space:]]*$' "$f" | head -n 1)
    if ! printf '%s' "$first" | grep -qE '^// spec: §[0-9]+$'; then
        c6_reasons="$c6_reasons $f:first-line-not-spec"
    else
        num=$(printf '%s' "$first" | grep -oE '§[0-9]+' | tr -d '§')
        hc=$(grep -cE "^# ${num}\\." "$SPEC" || true)
        if [ "$hc" -ne 1 ]; then
            c6_reasons="$c6_reasons $f:spec-heading-#$num-count=$hc"
        fi
    fi
    case "$f" in
        tests/accept/*)
            nexpect=$(grep -cE '^// expect:( .*)?$' "$f" || true)
            nexpect_err=$(grep -cE '^// expect-error: .+$' "$f" || true)
            nerror=$(grep -cE '^// error:' "$f" || true)
            [ $((nexpect + nexpect_err)) -eq 1 ] || c6_reasons="$c6_reasons $f:accept-expect-lines=$((nexpect + nexpect_err))"
            [ "$nerror" -eq 0 ] || c6_reasons="$c6_reasons $f:accept-has-error"
            for m in 'List.map' 'List.filter' 'List.fold' 'List.length' 'List.reverse' 'List.append' 'List.sort' 'Option.map' 'Option.unwrap_or' 'Option.is_some' 'Option.ok_or' 'Result.map' 'Result.map_error' 'Result.is_ok' 'String.length' 'String.concat' 'Int.to_string' 'Float.to_string'; do
                if grep -qF -- "$m" "$f"; then
                    grep -qE '^// needs: prelude$' "$f" || c6_reasons="$c6_reasons $f:uses-$m-without-needs-prelude"
                fi
            done
            ;;
        tests/reject/*)
            nexpect=$(grep -cE '^// expect(-error)?:' "$f" || true)
            [ "$nexpect" -eq 0 ] || c6_reasons="$c6_reasons $f:reject-has-expect"
            ;;
    esac
done
if [ -z "$c6_reasons" ]; then
    pass 6 "headers"
else
    fail 6 "headers:$c6_reasons"
fi

# --- check 7: existing tests green, compiler body untouched ---
c7_reasons=""
cargo test --test walk >"$TMPD/walk.log" 2>&1
[ "$?" -eq 0 ] || c7_reasons="$c7_reasons walk-exit!=0"
cargo test --test tail >"$TMPD/tail.log" 2>&1
[ "$?" -eq 0 ] || c7_reasons="$c7_reasons tail-exit!=0"
bash scripts/verify-m0.sh >"$TMPD/m0.log" 2>&1
m0_status=$?
m0_last=$(tail -n 1 "$TMPD/m0.log")
if [ "$m0_status" -ne 0 ] || [ "$m0_last" != "PASSED: 7/7" ]; then
    c7_reasons="$c7_reasons verify-m0(status=$m0_status,last=$m0_last)"
fi
if ! sha256sum --status -c "$TMPD/protected.sha"; then
    c7_reasons="$c7_reasons protected-files-changed-during-verification"
fi
if [ -z "$c7_reasons" ]; then
    pass 7 "no-regressions"
else
    fail 7 "no-regressions:$c7_reasons"
fi

echo "PASSED: $ok/$total"
[ "$ok" -eq "$total" ]
