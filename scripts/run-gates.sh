#!/usr/bin/env bash
set -euo pipefail
if [ "$#" -eq 0 ]; then
    echo 'usage: run-gates.sh <output-directory> [verify-<name> ...]' >&2
    exit 1
fi
mkdir -p -- "$1"
output=$(cd -- "$1" && pwd)
shift
cd "$(dirname "$0")/.."
if [ "$#" -eq 0 ]; then
    set -- scripts/verify-*.sh
fi
gates=()
for gate in "$@"; do
    name=${gate##*/}
    name=${name%.sh}
    if [[ "$name" != verify-* ]] || [ ! -f "scripts/$name.sh" ]; then
        echo "unknown gate: $gate" >&2
        exit 1
    fi
    gates+=("$name")
done
status=0
for name in "${gates[@]}"; do
    echo "RUN: $name -> $output/$name.log"
    if ! bash "scripts/$name.sh" >"$output/$name.log" 2>&1; then
        status=1
    fi
done
exit "$status"
