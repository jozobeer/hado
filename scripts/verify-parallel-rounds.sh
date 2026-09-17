#!/usr/bin/env bash
# tier: full
# round: parallel-rounds
# 並走できるラウンド運用（層の宣言、計画の分割、round 宣言、lock の道具）。
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
cleanup() {
    rm -rf "$workdir"
}
trap cleanup EXIT

tier_scan_case() {
    local tree copy probe_full out status
    tree="$workdir/tree-a"
    rm -rf -- "$tree"
    mkdir -p "$tree"
    cp -a scripts "$tree/scripts"
    copy="$tree/scripts"
    printf '%s\n' '#!/usr/bin/env bash' '# tier: full' 'exit 0' >"$copy/verify-probe.sh"
    probe_full=$(
        # shellcheck source=scripts/lib/gate-tiers.sh
        . "$copy/lib/gate-tiers.sh"
        hado_tier_minimal_gates full
    ) || return 1
    printf '%s\n' "$probe_full" | grep -qx probe || return 1
    grep -v '^# tier: ' "$copy/verify-probe.sh" >"$copy/verify-probe.sh.new"
    mv "$copy/verify-probe.sh.new" "$copy/verify-probe.sh"
    if (
        . "$copy/lib/gate-tiers.sh"
        hado_tier_all_gates
    ) >/dev/null 2>&1; then
        return 1
    fi
    out="$workdir/out-a"
    rm -rf -- "$out"
    mkdir -p "$out"
    status=0
    bash "$copy/run-gates.sh" --tier full "$out" >/dev/null 2>&1 || status=$?
    [ "$status" -ne 0 ] || return 1
    [ -z "$(find "$out" -name 'verify-*.log' -print)" ]
}

plan_split_case() {
    python3 - <<'PY'
from pathlib import Path
import re

heading = re.compile(r'^## [0-9]+\. ')
rnum = re.compile(r'R[0-9]')
bad = [
    line
    for line in Path('docs/plan.md').read_text().splitlines()
    if heading.match(line) and rnum.search(line)
]
assert not bad, bad[:3]
rounds = sorted(Path('docs/rounds').glob('*.md'))
assert rounds
for path in rounds:
    lines = path.read_text().splitlines()
    assert lines and lines[0].startswith('# '), path.name
    assert any(line == '## 手順書' for line in lines), path.name
PY
}

round_header_case() {
    python3 - <<'PY'
from pathlib import Path
import re

slug_ok = re.compile(r'^[a-z0-9][a-z0-9-]*$')
reserved = re.compile(r'^r[0-9]+(-r[0-9]+)?$')
header = re.compile(r'^# round: (.*)$', re.M)
found = 0
for path in sorted(Path('scripts').glob('verify-*.sh')):
    slugs = header.findall(path.read_text())
    if not slugs:
        continue
    found += 1
    assert len(slugs) == 1, path.name
    slug = slugs[0]
    assert slug_ok.fullmatch(slug), path.name
    assert not reserved.fullmatch(slug), path.name
    assert (Path('docs/rounds') / f'{slug}.md').is_file(), path.name
assert found >= 1
PY
}

lock_tool_case() {
    local copy lines before after case_dir
    copy="$workdir/lock-d"
    rm -rf -- "$copy"
    mkdir -p "$copy/scripts"
    cp -a tests stdlib "$copy/"
    cp scripts/corpus-lock.sh "$copy/scripts/"
    bash "$copy/scripts/corpus-lock.sh" --check || return 1
    lines=$(bash "$copy/scripts/corpus-lock.sh")
    cmp -s "$copy/tests/corpus.sha256" tests/corpus.sha256 || return 1
    before=$(grep -c '^' tests/corpus.sha256)
    [ "$lines" = "$before" ] || return 1
    case_dir=$(mktemp -d "$copy/tests/accept/probeXXXXXX")
    printf 'def main = ()\n' >"$case_dir/main.hd"
    after=$(bash "$copy/scripts/corpus-lock.sh")
    [ "$after" = "$((before + 1))" ] || return 1
    bash "$copy/scripts/corpus-lock.sh" --check || return 1
}

check 'a probe with # tier: full appears; a missing header aborts run-gates with no logs' tier_scan_case
check 'b plan headings have no R numbers; every round file has # title and ## 手順書' plan_split_case
check 'c each # round: slug is unique, well-formed, and has docs/rounds/<slug>.md' round_header_case
check 'd corpus-lock --check matches; a generated accept case adds one lock line' lock_tool_case

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
