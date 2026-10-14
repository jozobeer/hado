#!/usr/bin/env bash
# tier: core
# round: dropped-spec-contracts
#
# 仕様本文の針。門番を削るラウンド（docs/rounds/prune-gates.md）で落ちた 25 本の門番
# （r45〜r60 のうち r49・r51 を除く 14 本、r62〜r65、r68・r69、r72〜r76）が「§N に
# 規範文の断片が実在する」を見ていた。針は §N（付録は `付録X`）の一覧で表し、意味は
# 解釈せず文字列の実在だけを見る。r68・r69・r72〜r76 の針は元の門番が仕様全体で
# 見ていたので仕様全体で照合する。
#
# 仕様本文の綴りは scripts/lint-docs.sh に入れない（決定 D3。散文の針を隔離する）。
# tier は core: 文書を短時間で読むだけで、PR を回す層に要る検出力だから
# （scripts/ci-run.sh は pull_request を core で回す）。
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

# --- a: 仕様本文の針（§N の一覧と、仕様全体の一覧） ---
spec_needles() {
    python3 - <<'PY'
import re
import sys
from pathlib import Path

# この門番は lint-docs.sh と同じく assert で書く。-O か PYTHONOPTIMIZE が付くと assert が
# 消えて検査が黙って通るので、先に落とす。ここは if で書く（assert では消えてしまう）。
if not __debug__:
    sys.exit('verify-spec-contracts: assert が無効（PYTHONOPTIMIZE か -O）。付けずに回すこと')

spec = Path('docs/spec.md').read_text()
lines = spec.split('\n')


def body(ref):
    """節の本文。`N.M` と `付録X` は次の `## ` まで、整数 `N` は次の整数節まで
    （小数節は同じ節に含める）。無ければ None。"""
    if ref.startswith('付録'):
        start = next((i for i, l in enumerate(lines) if l.startswith(f'## {ref} ')), None)
        if start is None:
            return None
        end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith('## ')), len(lines))
    else:
        start = next((i for i, l in enumerate(lines) if l.startswith(f'## {ref}. ')), None)
        if start is None:
            return None
        if '.' in ref:
            end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith('## ')), len(lines))
        else:
            end = next((i for i in range(start + 1, len(lines))
                        if re.match(r'^## (\d+\. |付録)', lines[i])), len(lines))
    return '\n'.join(lines[start:end])


def found(needle, hay):
    """針が実在するか。右端が語の続きでないことまで見る。`hado: stack overflow` の
    後ろに `X` を足した `hado: stack overflowX` は、メッセージが変わっているので
    針を通さない（docs/rounds/dropped-spec-contracts.md の探針）。左端は見ない
    （針は規範文の一部である）。"""
    return re.search(re.escape(needle) + r'(?![A-Za-z0-9_])', hay) is not None


# (元の門番, 節, 針)。節が None なら仕様全体。
NEEDLES = [
    ('r45', '26.1', '## 26.1. パターンの腕の条件列'),
    ('r45', '付録C', 'pattern_branch  := "if" expression ("," expression)* pattern_arms'),
    ('r45', '付録C', 'pattern_arm     := "|" pattern ("," pattern)* "=>" (arm_body | arm_conditions)'),
    ('r45', '付録C', 'arm_conditions  := "if" condition_arms'),
    ('r46', '57', '--fix'),
    ('r46', '57', 'E510'),
    ('r46', '57', 'E516'),
    ('r46', '57', 'E517'),
    ('r46', '57', 'コメント'),
    ('r47', '57', 'E515'),
    ('r47', '57', 'E501'),
    ('r48', '58', 'hado run'),
    ('r48', '58', '-o <path>'),
    ('r48', '58', '一時ディレクトリ'),
    ('r48', '58', '終了コード'),
    ('r48', '58', 'canonical form:'),
    ('r50', '53.1', '## 53.1. スタックの枯渇'),
    ('r50', '53.1', 'hado: stack overflow'),
    ('r52', '58.1', '## 58.1. '),
    ('r52', '58', 'hado fmt [--check] <file.hd>'),
    ('r52', '58', 'hado build [-o <path>] [--fix] <file.hd>'),
    ('r52', '58.1', 'hado fmt'),
    ('r52', '58.1', '--check'),
    ('r53', '50', '`.` で始まる'),
    ('r53', '51', '`.` で始まる'),
    ('r54', '58.2', '## 58.2. '),
    ('r54', '58', 'hado test <file.hd>'),
    ('r54', '58.2', '*_test.hd'),
    ('r54', '58.2', 'failed (build)'),
    ('r54', '58.2', 'passed'),
    ('r55', '58.3', '## 58.3. '),
    ('r55', '58', '--print-type[=<名前>]'),
    ('r55', '58.3', '`A` `B`'),
    ('r55', '58.3', '| A'),
    ('r55', '58.3', 'Pure'),
    ('r55', '58.3', 'E100'),
    ('r55', '58.3', '契約'),
    ('r56', '15.1', '## 15.1. '),
    ('r56', '15.1', 'expected'),
    ('r56', '15.1', 'found'),
    ('r56', '15.1', 'cannot unify'),
    ('r56', '15.1', '`A` `B`'),
    ('r56', '15.1', '内部の番号'),
    ('r57', '57', 'E507'),
    ('r57', '57', 'E508'),
    ('r57', '57', 'def f value => value'),
    ('r57', '57', 'v as x -> x'),
    ('r57', '57', '本体ちょうど'),
    ('r57', '57', 'fallback'),
    ('r58', '56', '関連位置'),
    ('r58', '56', 'first declared'),
    ('r58', '56', 'declared in the contract'),
    ('r59', '37', '操作を参照するか'),
    ('r60', '15.1', '宣言された戻り型'),
    ('r62', '29', 'canonical form'),
    ('r62', '29', '書き換え'),
    ('r63', '24', '書き換え'),
    ('r63', '24', 'に書き換える'),
    ('r64', '58.1', '行末'),
    ('r65', '58.4', 'json'),
    ('r68', None, 'E512'),
    ('r68', None, '書き換え'),
    ('r69', None, 'E502'),
    ('r69', None, '書き換え'),
    ('r72', None, 'E506'),
    ('r73', None, 'E501'),
    ('r74', None, 'E511'),
    ('r75', None, 'E505'),
    ('r76', None, 'その値がそのまま終了コードである'),
]

# (元の門番, エラーコード, その行にある針)。§56 の表の行を名指しで見る。
ROWS = [
    ('r45', 'E125', 'pattern-column-count'),
    ('r58', 'E205', 'first declared'),
    ('r58', 'E602', 'declared in the contract'),
    ('r59', 'E513', 'handler'),
    ('r60', 'E305', '型が食い違った式の先頭'),
]

# (元の門番, 節, 代替の針)。どれか 1 つが実在すればよい。
ANY = [
    ('r60', None, ['期待の側が式である', 'expected here']),
]

# (元の門番, 仕様全体でちょうど 1 回の針)。
ONCE = [
    ('r45', '腕の本体に置ける分岐は条件列だけで、条件列の腕には分岐を置けない'),
]

# (元の門番, 仕様全体に在ってはならない針)。
ABSENT = [
    ('r57', '局所的な書き換えを持たない正準形違反（E507 / E508）'),
]

missing = []
for source, ref, needle in NEEDLES:
    hay = spec if ref is None else body(ref)
    if hay is None:
        missing.append(f'{source}: missing section §{ref}')
    elif not found(needle, hay):
        missing.append(f'{source} §{ref}: {needle}')

rows = {line.split('|')[1].strip(): line for line in lines if line.startswith('| E')}
for source, code, needle in ROWS:
    if code not in rows:
        missing.append(f'{source}: missing row {code}')
    elif not found(needle, rows[code]):
        missing.append(f'{source} {code}: {needle}')

for source, ref, alternatives in ANY:
    hay = spec if ref is None else body(ref)
    if hay is None or not any(found(needle, hay) for needle in alternatives):
        missing.append(f'{source} §{ref}: {alternatives}')

for source, needle in ONCE:
    count = len(re.findall(re.escape(needle) + r'(?![A-Za-z0-9_])', spec))
    if count != 1:
        missing.append(f'{source}: {needle} appears {count} times')

for source, needle in ABSENT:
    if needle in spec:
        missing.append(f'{source}: {needle} remains')

assert not missing, missing
sections = sorted({f'§{ref}' if not ref.startswith('付録') else ref
                   for _, ref, _ in NEEDLES if ref is not None})
print(f'spec needles: {len(NEEDLES)} in {", ".join(sections)} and the whole spec, '
      f'{len(ROWS)} table rows, {len(ONCE)} once, {len(ABSENT)} absent')
PY
}

check 'the spec body keeps the needles of the dropped gates' spec_needles

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
