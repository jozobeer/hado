#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."

passed=0
total=0
check() {
    local label="$1"
    shift
    total=$((total + 1))
    if "$@"; then
        echo "OK: $label"
        passed=$((passed + 1))
    else
        echo "NG: $label"
    fi
}

e509_gone_and_codes_in_spec() {
    local spec disk extra
    [ ! -d tests/reject/E509 ] || return 1
    spec=$(sed -n 's/^| \(E[1-7][0-9][0-9]\) |.*/\1/p' docs/spec.md | sort -u)
    disk=$(find tests/reject -mindepth 1 -maxdepth 1 -type d -name 'E[0-9][0-9][0-9]' | sed 's|.*/||' | sort)
    extra=$(comm -13 <(printf '%s\n' "$spec") <(printf '%s\n' "$disk"))
    [ -z "$extra" ]
}

no_result_constructors_in_accept() {
    ! grep -REn 'Ok\(|Error\(' tests/accept --include='*.hd' --include='*.hdi' | grep -q .
}

# Why not require disk == §50: E706 is in the table and R26 adds the file. 25-b
# asks for E509 gone and remaining codes inside the table; the count matches at R26.
check '25-a/c accept and reject corpus' cargo test --offline --test corpus -- --exact corpus
check '25-b E509 gone and reject codes subset of §50' e509_gone_and_codes_in_spec
check '25-c generated abort and Option-only rails' cargo test --offline --test r25_effects --test r14_rails --test emit_mono
check '25-e Ok( Error( absent from accept' no_result_constructors_in_accept

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
