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
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError:
        sys.exit('verify-spec-model: TOML を読むのに Python 3.11 以上か tomli が要る')

spec = Path('docs/spec.md').read_text()

# この門番は lint-docs.sh と同じく assert で書く。-O か PYTHONOPTIMIZE が付くと assert が
# 消えて検査が黙って通るので、先に落とす（審査の指摘）。ここは if で書く（assert では
# 消えてしまうので用を成さない）。
if not __debug__:
    sys.exit('verify-spec-model: assert が無効（PYTHONOPTIMIZE か -O）。付けずに回すこと')

# spec.md の `## ` 見出しから、節と付録の一覧を作る。
headings = set()
for line in spec.split('\n'):
    m = re.match(r'## (?:(\d+(?:\.\d+)?)\.|(付録[A-Z]))', line)
    if m:
        headings.add('§' + m.group(1) if m.group(1) else m.group(2))
assert headings, 'spec.md に見出しが無い'

errors = tomllib.loads(Path('spec/errors.toml').read_text())['error']
assert errors, 'spec/errors.toml が空'

# コードの族ごとに取り得る段階（spec/stages.toml）。同じ族を 2 度書くと辞書が黙って
# 後のものを残すので、畳む前に重複を弾く。
records = tomllib.loads(Path('spec/stages.toml').read_text())['stage']
assert records, 'spec/stages.toml が空'
families = [s['family'] for s in records]
assert len(families) == len(set(families)), \
    f'spec/stages.toml に重複した族: {sorted({f for f in families if families.count(f) > 1})}'
# stages は文字列の並びである。文字列にすると `in` が部分文字列の照合になり、族の
# 取り違えが素通りする（審査の指摘）。
for s in records:
    assert isinstance(s['stages'], list) and s['stages'], \
        f'spec/stages.toml の {s["family"]} の stages が空'
    assert all(isinstance(v, str) and v.strip() for v in s['stages']), \
        f'spec/stages.toml の {s["family"]} の stages に空か文字列でない値'
stages = {s['family']: s['stages'] for s in records}

problems = []
codes = [e['code'] for e in errors]
names = [e['name'] for e in errors]
# 対応表の族は、誤りが使う族を過不足なく覆う。使われない族が残ると、その族の段階の
# 割り当てが検査されないまま緑になる。
used_families = {code[:2] for code in codes}
if set(stages) != used_families:
    problems.append(
        f'family-coverage=table{sorted(set(stages))} vs codes{sorted(used_families)}')
for code, count in ((c, codes.count(c)) for c in set(codes)):
    if count > 1:
        problems.append(f'{code}:duplicate-code')
for name, count in ((n, names.count(n)) for n in set(names)):
    if count > 1:
        problems.append(f'{name}:duplicate-name')
for e in errors:
    code = e['code']
    if not re.fullmatch(r'E[1-7][0-9]{2}', code):
        problems.append(f'{code}:code-form')
        continue
    if not re.fullmatch(r'[a-z][a-z0-9-]*', e['name']):
        problems.append(f'{code}:name-form={e["name"]}')
    if e['section'] not in headings:
        problems.append(f'{code}:section-absent={e["section"]}')
    # 段階は族ごとに決まる。空だけでなく、他の段階への取り違えも見る。
    allowed = stages.get(code[:2])
    if allowed is None:
        problems.append(f'{code}:family-absent={code[:2]}')
    elif e['stage'] not in allowed:
        problems.append(f'{code}:stage={e["stage"]}（{code[:2]} は {allowed}）')
    if not e['anchor'].strip():
        problems.append(f'{code}:anchor-blank')
assert not problems, sorted(problems)

# 文法: `### <ラベル>` ごとに本文があり、ラベルが一意である。
# 本文の判定は spec-lock.sh と同じにする（空行だけの群を本文在りと数えない）。
groups = []
for line in Path('spec/grammar.ebnf').read_text().split('\n'):
    if line.startswith('### '):
        groups.append([line[4:].strip(), []])
    elif groups:
        groups[-1][1].append(line)
assert groups, 'spec/grammar.ebnf に群が無い'
assert all(any(b.strip() for b in body) for _, body in groups), \
    [label for label, body in groups if not any(b.strip() for b in body)]
labels = [label for label, _ in groups]
assert len(labels) == len(set(labels)), sorted({l for l in labels if labels.count(l) > 1})

print(f'{len(errors)} エラーコード、{len(groups)} 文法群、{len(stages)} 段階の族、見出し {len(headings)}')
PY
}

check 'docs/spec.md matches spec/ (spec-lock --check)' lock_case
check 'the model is well formed and its sections exist' model_case

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
