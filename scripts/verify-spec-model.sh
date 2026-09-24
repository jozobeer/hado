#!/usr/bin/env bash
# tier: quick
# round: spec-model
# 仕様の機械可読な部分（spec/errors.toml と spec/grammar.ebnf）の判定器。
#
# docs/spec.md の §56 の表と付録C のフェンスは、このモデルから scripts/spec-lock.sh が
# 作る生成物である。ここで測るのは 3 つ。
#   a. 生成物がモデルと一致する（表やフェンスを手で編集しても赤になる）。
#   b. モデル自身の整合（コードと名前の字種・一意性、節の実在）。
#   c. 文法の群の形（ラベルの一意性、本文が空でない）。
#
# 件数はモデルから数える（ここに 70 や 10 を書かない）。節の一覧は spec.md の見出しから
# 作る（節番号をここに書かない）。
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

# --- a: 生成物がモデルと一致する ---
lock_case() {
    bash scripts/spec-lock.sh --check
}

# --- b・c: モデル自身の整合と文法の形 ---
model_case() {
    python3 - <<'PY'
import re
import tomllib
from pathlib import Path

spec = Path('docs/spec.md').read_text()

# spec.md の `## ` 見出しから、節と付録の一覧を作る。
headings = set()
for line in spec.split('\n'):
    m = re.match(r'## (?:(\d+(?:\.\d+)?)\.|(付録[A-Z]))', line)
    if m:
        headings.add('§' + m.group(1) if m.group(1) else m.group(2))
assert headings, 'spec.md に見出しが無い'

errors = tomllib.loads(Path('spec/errors.toml').read_text())['error']
assert errors, 'spec/errors.toml が空'

problems = []
codes = [e['code'] for e in errors]
for code, count in ((c, codes.count(c)) for c in set(codes)):
    if count > 1:
        problems.append(f'{code}:duplicate')
for e in errors:
    code = e['code']
    if not re.fullmatch(r'E[1-7][0-9]{2}', code):
        problems.append(f'{code}:code-form')
    if not re.fullmatch(r'[a-z][a-z0-9-]*', e['name']):
        problems.append(f'{code}:name-form={e["name"]}')
    if e['section'] not in headings:
        problems.append(f'{code}:section-absent={e["section"]}')
    if not e['stage']:
        problems.append(f'{code}:stage-empty')
    if not e['anchor']:
        problems.append(f'{code}:anchor-empty')
assert not problems, sorted(problems)

# 文法: `### <ラベル>` ごとに本文があり、ラベルが一意である。
groups = []
for line in Path('spec/grammar.ebnf').read_text().split('\n'):
    if line.startswith('### '):
        groups.append([line[4:].strip(), []])
    elif groups and not line.startswith('#'):
        groups[-1][1].append(line)
assert groups, 'spec/grammar.ebnf に群が無い'
assert all(body for _, body in groups), \
    [label for label, body in groups if not body]
labels = [label for label, _ in groups]
assert len(labels) == len(set(labels)), sorted({l for l in labels if labels.count(l) > 1})

print(f'{len(errors)} エラーコード、{len(groups)} 文法群、見出し {len(headings)}')
PY
}

check 'docs/spec.md matches spec/ (spec-lock --check)' lock_case
check 'the model is well formed and its sections exist' model_case

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
