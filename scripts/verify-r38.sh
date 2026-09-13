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
workdir=$(mktemp -d)
trap 'rm -rf "$workdir"' EXIT

partial_run() {
    local started=$SECONDS name
    bash scripts/run-gates.sh "$workdir/partial" verify-r11 verify-r12 || return 1
    echo "partial run: $((SECONDS - started))s"
    python3 - "$workdir/partial" <<'PY'
from pathlib import Path
import sys
logs = list(Path(sys.argv[1]).iterdir())
assert sorted(p.name for p in logs) == ['verify-r11.log', 'verify-r12.log']
assert all(p.is_file() for p in logs)
PY
    [ "$?" -eq 0 ] || return 1
    for name in verify-r11 verify-r12; do
        grep -qx 'PASSED: 1/1' "$workdir/partial/$name.log" || return 1
        echo "$name.log: $(tail -1 "$workdir/partial/$name.log")"
    done
}
check '38-a selected real gates and flat logs' partial_run

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
