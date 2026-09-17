#!/usr/bin/env bash
# tier: full
# R42 節番号を増やしても動かない形にする（`docs/rounds/r42.md`）。本文は整数と
# 小数、付録は文字で引き、参照が実在を指すことを測る。リポジトリルートから実行する。
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

# --- 42-a(1): 本文の節が `## N.` として 1 回ずつ読み順に、付録が A から順に ---
spec_structure() {
    python3 - <<'PY'
import re
from pathlib import Path
spec = Path('docs/spec.md').read_text()
body, appendix, in_appendix, seen_part = [], [], False, False
for line in spec.splitlines():
    assert not re.match(r'^# [0-9]+\.', line), line
    if line.startswith('# '):
        seen_part = True
    if re.match(r'^## \d', line):
        assert seen_part, 'section before any part heading'
        assert not in_appendix, f'body section after the appendix: {line}'
        match = re.match(r'^## (\d+(?:\.\d+)?)\. ', line)
        assert match, line
        body.append(tuple(int(part) for part in match.group(1).split('.')))
    elif re.match(r'^## 付録', line):
        assert seen_part, 'section before any part heading'
        in_appendix = True
        match = re.match(r'^## (付録[A-Z]) ', line)
        assert match, line
        appendix.append(match.group(1))
assert body == sorted(body), 'body sections are not in reading order'
assert len(set(body)) == len(body), 'duplicate body section number'
integers = [number for number in body if len(number) == 1]
assert [number[0] for number in integers] == list(range(1, len(integers) + 1)), integers
assert appendix == [f'付録{chr(ord("A") + i)}' for i in range(len(appendix))], appendix
assert re.search(r'^# 付録$', spec, flags=re.M), 'no appendix part'
assert re.search(r'^## 付録C 文法の概形$', spec, flags=re.M), '文法の概形 is not 付録C'
print(f'sections: {len(body)} (1..{integers[-1][0]}), appendix: {appendix}')
PY
}

# --- 42-a(2): 最新の snapshot だけを持つ（対応表が無く、前書きに旧版の記載が無い） ---
spec_current_only() {
    python3 - <<'PY'
import re
from pathlib import Path
assert not Path('docs/spec-map.md').exists(), 'docs/spec-map.md still exists'
front = Path('docs/spec.md').read_text().split('## 規範性', 1)[0]
assert 'spec-map' not in front, 'the front matter mentions spec-map'
versions = set(re.findall(r'v[0-9]+(?:\.[0-9]+)+', front))
assert versions == {'v0.4'}, versions
bad = []
for rel in ['docs/spec.md', 'docs/spec-coverage.md', 'docs/spec-trace.md', 'docs/corpus.md', 'README.md']:
    if 'spec-map' in Path(rel).read_text():
        bad.append(rel)
for path in sorted(Path('scripts').glob('verify-*.sh')):
    if path.name == 'verify-r42.sh':
        continue  # この検査自身が検査の文言に spec-map を含む
    if 'spec-map' in path.read_text():
        bad.append(str(path))
assert not bad, bad
print(f'current only: version {sorted(versions)}, no spec-map in the live documents or gates')
PY
}

# --- 42-a(3): コーパスのタグが §56 の表の節と一致し、全て実在の節を指す ---
spec_tags() {
    python3 - <<'PY'
import re
from pathlib import Path

spec = Path('docs/spec.md').read_text()
headings = set(re.findall(r'^## (\d+(?:\.\d+)?)\.', spec, flags=re.M))
appendix = set(re.findall(r'^## (付録[A-Z]) ', spec, flags=re.M))
tag_re = re.compile(r'^// spec: (§\d+(?:\.\d+)?|付録[A-Z])$', re.M)

def exists(tag):
    if tag.startswith('§'):
        return tag[1:] in headings
    return tag in appendix

table_text = spec.split('## 56. エラーコード', 1)[1]
table = dict(re.findall(r'^\| (E[1-7]\d\d) \|[^|]*\| (§\d+(?:\.\d+)?|付録[A-Z]) \|', table_text, flags=re.M))
# 行数は直書きせず、拒否コーパスのディレクトリ数（表と 1 対 1）から導く
reject_dirs = [path for path in Path('tests/reject').iterdir() if path.is_dir()]
assert len(table) == len(reject_dirs), (len(table), len(reject_dirs))
bad = []
for path in sorted(Path('tests').rglob('main.hd')):
    text = path.read_text()
    tag = tag_re.search(text)
    if not tag:
        bad.append(f'{path}:no-spec-tag')
        continue
    if not exists(tag.group(1)):
        bad.append(f'{path}:missing-heading-{tag.group(1)}')
    error = re.search(r'^// error: (E[1-7]\d\d)$', text, flags=re.M)
    if error and table.get(error.group(1)) != tag.group(1):
        bad.append(f'{path}:{error.group(1)}-table-{table.get(error.group(1))}-tag-{tag.group(1)}')
for path in sorted(Path('tests/library').rglob('*')):
    if path.suffix in ('.hd', '.hdi'):
        tag = tag_re.search(path.read_text())
        if tag and not exists(tag.group(1)):
            bad.append(f'{path}:missing-heading-{tag.group(1)}')
assert not bad, bad[:10]
print(f'codes: {len(table)}, corpus tags resolve and match')
PY
}

# --- 42-a(4): 現行の文書の §参照と付録参照が全て実在を指す ---
spec_refs() {
    python3 - <<'PY'
import re
from pathlib import Path

spec = Path('docs/spec.md').read_text()
headings = set(re.findall(r'^## (\d+(?:\.\d+)?)\.', spec, flags=re.M))
letters = {name[-1] for name in re.findall(r'^## (付録[A-Z]) ', spec, flags=re.M)}
bad = []
for rel in ['docs/spec-coverage.md', 'docs/spec-trace.md', 'docs/corpus.md', 'README.md', 'docs/bug.md', 'docs/wants.md']:
    text = Path(rel).read_text()
    missing = sorted({n for n in re.findall(r'§(\d+(?:\.\d+)?)', text)} - headings)
    missing += sorted({a for a in re.findall(r'付録([A-Z])', text)} - letters)
    if missing:
        bad.append(f'{rel}:{missing}')
assert not bad, bad
print('live document references resolve')
PY
}

# --- 42-a(5): 版が v0.4 で、spec と spec-coverage が同じ版を名乗る ---
spec_version() {
    python3 - <<'PY'
import re
from pathlib import Path
spec = re.search(r'v[0-9]+(?:\.[0-9]+)+', Path('docs/spec.md').read_text().splitlines()[0])
coverage = re.search(r'v[0-9]+(?:\.[0-9]+)+', Path('docs/spec-coverage.md').read_text().splitlines()[0])
assert spec and coverage and spec.group(0) == coverage.group(0) == 'v0.4', (spec, coverage)
print('version: v0.4')
PY
}

check 'body sections §1..§56 and 付録A.. are in reading order under parts' spec_structure
check 'the current spec is the only snapshot (no spec-map, no older version)' spec_current_only
check 'corpus tags match the §56 table and resolve' spec_tags
check 'section and appendix references in the live documents resolve' spec_refs
check 'spec and spec-coverage name v0.4' spec_version

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
