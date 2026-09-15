#!/usr/bin/env bash
# R39 仕様書の再構成（docs/plan.md 35 節）。節番号の写像と構造、参照の追随、
# および tag v0.3 の各節本文が新しい節に保存されていることを測る。
# リポジトリルートから実行する。
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

# --- 39-a(1): 節は §1〜§59 が `## N.` として 1 回ずつ読み順に並び、部の下にある ---
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
parts = len(re.findall(r'^# ', spec, flags=re.M))
print(f'sections: {len(nums)}, part headings: {parts}')
PY
}

# --- 39-a(2): docs/spec-map.md の写像が旧 1..59 と新 1..59 の全単射 ---
spec_map() {
    python3 - <<'PY'
import re
from pathlib import Path
rows = re.findall(r'^\| §(\d+) \| §(\d+) \|', Path('docs/spec-map.md').read_text(), flags=re.M)
old = [int(a) for a, _ in rows]
new = [int(b) for _, b in rows]
assert len(rows) == 59, len(rows)
assert sorted(old) == list(range(1, 60)), sorted(old)
assert sorted(new) == list(range(1, 60)), sorted(new)
print(f'map rows: {len(rows)} (bijection)')
PY
}

# --- 39-a(3): tag v0.3 の各節の本文が、写像後の対応する節に現れる ---
spec_preservation() {
    python3 - <<'PY'
import re
import subprocess
from pathlib import Path

rows = re.findall(r'^\| §(\d+) \| §(\d+) \|', Path('docs/spec-map.md').read_text(), flags=re.M)
mapping = {int(a): int(b) for a, b in rows}
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

# --- 39-a(4): コーパスのタグが §57 の表と一致し、全て実在の節を指す ---
spec_tags() {
    python3 - <<'PY'
import re
from pathlib import Path

spec = Path('docs/spec.md').read_text()
heads = {int(n) for n in re.findall(r'^## (\d+)\.', spec, flags=re.M)}
table_text = spec.split('## 57. エラーコード', 1)[1]
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

# --- 39-a(4): 現行の文書の §参照が全て実在の節を指す ---
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

check 'spec sections §1..§59 are in reading order under parts' spec_structure
check 'spec-map is a bijection between the old and new numbering' spec_map
check 'every v0.3 section body survives in its new section' spec_preservation
check 'corpus tags match the §57 table and resolve' spec_tags
check 'section references in the live documents resolve' spec_refs

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
