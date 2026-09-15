#!/usr/bin/env bash
# R41 旧仕様の記載を外す（docs/plan.md 37 節）。対応表と tag v0.3 との照合を
# 落とした後の、現行の仕様の自己整合だけを測る。リポジトリルートから実行する。
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

# --- 41-a(1): 節は §1〜§59 が `## N.` として 1 回ずつ読み順に並び、部の下にある ---
spec_structure() {
    python3 - <<'PY'
import re
from pathlib import Path
spec = Path('docs/spec.md').read_text()
nums = [int(n) for n in re.findall(r'^## (\d+)\.', spec, flags=re.M)]
assert nums == list(range(1, 60)), nums
seen_part = False
for line in spec.splitlines():
    assert not re.match(r'^# [0-9]+\.', line), line
    if re.match(r'^## [0-9]+\.', line):
        assert seen_part, 'section before any part heading'
    if line.startswith('# '):
        seen_part = True
parts = re.findall(r'^# (.+)$', spec, flags=re.M)
assert parts[-1] == '付録', parts[-1]
assert re.search(r'^## 59\. 文法の概形$', spec, flags=re.M), '文法の概形 is not §59'
print(f'sections: {len(nums)}, part headings: {len(parts)}, last: {parts[-1]}')
PY
}

# --- 41-a(2): 最新の snapshot だけを持つ（対応表が無く、前書きに旧版の記載が無い） ---
spec_current_only() {
    python3 - <<'PY'
import re
from pathlib import Path
assert not Path('docs/spec-map.md').exists(), 'docs/spec-map.md still exists'
front = Path('docs/spec.md').read_text().split('## 規範性', 1)[0]
assert 'spec-map' not in front, 'the front matter mentions spec-map'
versions = set(re.findall(r'v[0-9]+(?:\.[0-9]+)+', front))
assert versions == {'v0.3.1'}, versions
bad = []
for rel in ['docs/spec.md', 'docs/spec-coverage.md', 'docs/spec-trace.md', 'docs/corpus.md', 'README.md']:
    if 'spec-map' in Path(rel).read_text():
        bad.append(rel)
for path in sorted(Path('scripts').glob('verify-*.sh')):
    if path.name == 'verify-r41.sh':
        continue  # この検査自身が検査の文言に spec-map を含む
    if 'spec-map' in path.read_text():
        bad.append(str(path))
assert not bad, bad
print(f'current only: version {sorted(versions)}, no spec-map in the live documents or gates')
PY
}

# --- 41-a(3): コーパスのタグが §56 の表と一致し、全て実在の節を指す ---
spec_tags() {
    python3 - <<'PY'
import re
from pathlib import Path

spec = Path('docs/spec.md').read_text()
heads = {int(n) for n in re.findall(r'^## (\d+)\.', spec, flags=re.M)}
table_text = spec.split('## 56. エラーコード', 1)[1]
table = dict(re.findall(r'^\| (E[1-7]\d\d) \|[^|]*\| §(\d+) \|', table_text, flags=re.M))
assert len(table) == 69, len(table)
bad = []
for path in sorted(Path('tests').rglob('main.hd')):
    text = path.read_text()
    tag = re.search(r'^// spec: §(\d+)$', text, flags=re.M)
    if not tag:
        bad.append(f'{path}:no-spec-tag')
        continue
    if int(tag.group(1)) not in heads:
        bad.append(f'{path}:missing-heading-{tag.group(1)}')
    error = re.search(r'^// error: (E[1-7]\d\d)$', text, flags=re.M)
    if error and table.get(error.group(1)) != tag.group(1):
        bad.append(f'{path}:{error.group(1)}-table-{table.get(error.group(1))}-tag-{tag.group(1)}')
for path in sorted(Path('tests/library').rglob('*')):
    if path.suffix in ('.hd', '.hdi'):
        tag = re.search(r'^// spec: §(\d+)$', path.read_text(), flags=re.M)
        if tag and int(tag.group(1)) not in heads:
            bad.append(f'{path}:missing-heading-{tag.group(1)}')
assert not bad, bad[:10]
print(f'codes: {len(table)}, corpus tags resolve and match')
PY
}

# --- 41-a(4): 現行の文書の §参照が全て実在の節を指す ---
spec_refs() {
    python3 - <<'PY'
import re
from pathlib import Path

heads = {int(n) for n in re.findall(r'^## (\d+)\.', Path('docs/spec.md').read_text(), flags=re.M)}
bad = []
for rel in ['docs/spec-coverage.md', 'docs/spec-trace.md', 'docs/corpus.md', 'README.md', 'docs/bug.md', 'docs/wants.md']:
    missing = sorted({int(n) for n in re.findall(r'§(\d+)', Path(rel).read_text())} - heads)
    if missing:
        bad.append(f'{rel}:{missing}')
assert not bad, bad
print('live document references resolve')
PY
}

# --- 41-a(5): 版が v0.3.1 で、spec と spec-coverage が同じ版を名乗る ---
spec_version() {
    python3 - <<'PY'
import re
from pathlib import Path
spec = re.search(r'v[0-9]+(?:\.[0-9]+)+', Path('docs/spec.md').read_text().splitlines()[0])
coverage = re.search(r'v[0-9]+(?:\.[0-9]+)+', Path('docs/spec-coverage.md').read_text().splitlines()[0])
assert spec and coverage and spec.group(0) == coverage.group(0) == 'v0.3.1', (spec, coverage)
print('version: v0.3.1')
PY
}

check 'spec sections §1..§59 are in reading order under parts, 文法の概形 is §59' spec_structure
check 'the current spec is the only snapshot (no spec-map, no older version)' spec_current_only
check 'corpus tags match the §56 table and resolve' spec_tags
check 'section references in the live documents resolve' spec_refs
check 'spec and spec-coverage name v0.3.1' spec_version

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
