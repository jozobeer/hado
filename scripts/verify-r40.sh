#!/usr/bin/env bash
# R40 仕様書の再構成（docs/plan.md 36 節）。文法の概形を付録の最後へ移した後の
# 構造・写像・本文の保存・参照の追随・版を測る。リポジトリルートから実行する。
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

# --- 40-a(1): 節は §1〜§59 が `## N.` として 1 回ずつ読み順に並び、部の下にある ---
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

# --- 40-a(2): docs/spec-map.md の 3 列がどれも 1..59 の置換 ---
spec_map() {
    python3 - <<'PY'
import re
from pathlib import Path
rows = re.findall(r'^\| §(\d+) \| §(\d+) \| §(\d+) \|', Path('docs/spec-map.md').read_text(), flags=re.M)
assert len(rows) == 59, len(rows)
for column, index in [('v0.3', 0), ('R39', 1), ('現行', 2)]:
    values = sorted(int(row[index]) for row in rows)
    assert values == list(range(1, 60)), (column, values)
print(f'map rows: {len(rows)} (3 columns, each a permutation)')
PY
}

# --- 40-a(3): tag v0.3 の各節の本文が、写像後の対応する節に現れる ---
spec_preservation() {
    python3 - <<'PY'
import re
import subprocess
from pathlib import Path

rows = re.findall(r'^\| §(\d+) \| §(\d+) \| §(\d+) \|', Path('docs/spec-map.md').read_text(), flags=re.M)
mapping = {int(row[0]): int(row[2]) for row in rows}
old_text = subprocess.run(['git', 'show', 'v0.3:docs/spec.md'], capture_output=True, text=True, check=True).stdout
new_text = Path('docs/spec.md').read_text()

def parse(text, pattern):
    sections, current = {}, None
    for line in text.split('\n'):
        match = re.match(pattern, line)
        if match:
            current = int(match.group(1))
            sections[current] = []
        elif current is not None and not re.match(r'^#{1,2} ', line):
            sections[current].append(line)
    return {number: strip(body) for number, body in sections.items()}

def strip(lines):
    body = list(lines)
    while body and (not body[-1].strip() or body[-1].strip() == '---'):
        body.pop()
    while body and not body[0].strip():
        body.pop(0)
    return '\n'.join(body)

def remap(text):
    text = re.sub(r'第\s*(\d+)\s*節', lambda m: f'第{mapping[int(m.group(1))]}節', text)
    return re.sub(r'§\s*(\d+)', lambda m: f'§{mapping[int(m.group(1))]}', text)

old_sections = parse(old_text, r'^#{1,3} (\d+)\.')
new_sections = parse(new_text, r'^## (\d+)\.')
assert set(old_sections) == set(range(1, 60)), sorted(old_sections)
lost = []
for old, new in mapping.items():
    if remap(old_sections[old]) not in new_sections[new]:
        lost.append(f'§{old}->§{new}')
assert not lost, lost
print(f'preserved: {len(mapping)} sections from v0.3')
PY
}

# --- 40-a(4): コーパスのタグが §56 の表と一致し、全て実在の節を指す ---
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

# --- 40-a(5): 現行の文書の §参照が全て実在の節を指す ---
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

# --- 40-h: 版が v0.3.1 で、spec と spec-coverage が同じ版を名乗る ---
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
check 'spec-map is a 3-column bijection (v0.3 / R39 / 現行)' spec_map
check 'every v0.3 section body survives in its current section' spec_preservation
check 'corpus tags match the §56 table and resolve' spec_tags
check 'section references in the live documents resolve' spec_refs
check 'spec and spec-coverage name v0.3.1' spec_version

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
