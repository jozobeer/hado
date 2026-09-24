#!/usr/bin/env bash
# docs/spec.md の機械可読な部分を spec/ から作り直す。
#
# 正本は spec/errors.toml（§56 のエラーコード表）と spec/grammar.ebnf（付録C の文法）。
# docs/spec.md の §56 の表と付録C のフェンスは生成物で、手で編集しない。散文（節の導入・
# フェンスの間の説明・見出し）は spec.md に残り、この道具は触らない。
#
#   bash scripts/spec-lock.sh           作り直す
#   bash scripts/spec-lock.sh --check   照合する（食い違えば差分を出して非 0）
set -euo pipefail
cd "$(dirname "$0")/.."

mode=write
case "${1:-}" in
    --check) mode=check ;;
    "") ;;
    *) printf 'usage: %s [--check]\n' "$0" >&2; exit 2 ;;
esac

python3 - "$mode" <<'PY'
import difflib
import sys
from pathlib import Path

# tomllib は 3.11 から。古い python3（Ubuntu 22.04 の 3.10 など）では tomli を探す。
try:
    import tomllib
except ModuleNotFoundError:
    try:
        import tomli as tomllib
    except ModuleNotFoundError:
        sys.exit('spec-lock: TOML を読むのに Python 3.11 以上か tomli が要る')

mode = sys.argv[1]
SPEC = Path('docs/spec.md')
text = SPEC.read_text()

# 表の見出しと区切り。列は spec/errors.toml の並びと同じ。
COLUMNS = ['コード', '短い名前', '節', '段階', 'アンカー']
# 付録C のフェンス。開きは言語を伴い、閉じは言語を伴わない。
FENCE_OPEN = '```text'
FENCE_CLOSE = '```'


def section_bounds(lines, prefix):
    """`## <prefix>` の見出しから、次の `## ` の手前まで（最後の節は末尾まで）。"""
    start = next(i for i, line in enumerate(lines) if line.startswith(prefix))
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith('## ')), len(lines))
    return start, end


def field(entry, key):
    """表のセルに置ける値か確かめて返す。改行は表を壊し、空は契約を消す。"""
    value = entry.get(key)
    if not isinstance(value, str) or not value.strip():
        sys.exit(f'spec-lock: {entry.get("code", "?")} の {key} が空')
    if '\n' in value or '\r' in value:
        sys.exit(f'spec-lock: {entry.get("code", "?")} の {key} に改行')
    return value


def error_table():
    """spec/errors.toml から §56 の表を組む。セルの `|` は `\\|` に逃がす。"""
    entries = tomllib.loads(Path('spec/errors.toml').read_text())['error']
    rows = ['| ' + ' | '.join(COLUMNS) + ' |', '|' + '---|' * len(COLUMNS)]
    for e in entries:
        cells = [field(e, k) for k in ('code', 'name', 'section', 'stage')]
        cells.append(field(e, 'anchor').replace('|', '\\|'))
        rows.append('| ' + ' | '.join(cells) + ' |')
    return rows


def grammar_groups():
    """spec/grammar.ebnf の `### <ラベル>` ごとの本文。

    先頭の `### ` より前だけが注記で、以後は `### ` の行までが本文である（本文の行は
    先頭が `#` でも落とさない）。
    """
    groups = []
    for line in Path('spec/grammar.ebnf').read_text().split('\n'):
        if line.startswith('### '):
            groups.append((line[4:].strip(), []))
        elif groups:
            groups[-1][1].append(line)
    # 群の区切りの空行を落とす。
    return [(label, body[:-1] if body and body[-1] == '' else body) for label, body in groups]


def fences_in(lines, start, end):
    """付録C のフェンスの (開き, 閉じ) を行番号で返す。開きと閉じを厳密に区別する。"""
    fences = []
    open_at = None
    for i in range(start, end):
        line = lines[i]
        if open_at is None:
            if line == FENCE_OPEN:
                open_at = i
            elif line.startswith('```'):
                raise SystemExit(f'spec-lock: {i + 1} 行目のフェンスの開きが {line!r}')
        elif line == FENCE_CLOSE:
            fences.append((open_at, i))
            open_at = None
        elif line.startswith('```'):
            raise SystemExit(f'spec-lock: {i + 1} 行目のフェンスの閉じが {line!r}')
    if open_at is not None:
        raise SystemExit('spec-lock: 付録C のフェンスが閉じていない')
    return fences


def rewrite(lines):
    """§56 の表と付録C のフェンスを、モデルから組んだものに置き換える。"""
    # --- §56 の表 ---
    start, end = section_bounds(lines, '## 56. ')
    run = [i for i in range(start, end) if lines[i].startswith('|')]
    if not run or run != list(range(run[0], run[-1] + 1)):
        raise SystemExit('spec-lock: §56 の表が連続していない（手で編集したか）')
    lines = lines[:run[0]] + error_table() + lines[run[-1] + 1:]

    # --- 付録C のフェンス ---
    start, end = section_bounds(lines, '## 付録C')
    fences = fences_in(lines, start, end)
    groups = grammar_groups()
    if len(fences) != len(groups):
        raise SystemExit(
            f'spec-lock: 付録C のフェンス {len(fences)} 本に対し spec/grammar.ebnf の群が {len(groups)} 本')
    # 後ろから置き換えて添字を保つ。
    for (open_at, close_at), (label, body) in reversed(list(zip(fences, groups))):
        # フェンスの直前の非空行がラベル。群のラベルと一致することを確かめる。
        label_at = next(i for i in range(open_at - 1, start - 1, -1) if lines[i].strip())
        if lines[label_at].strip() != label:
            raise SystemExit(
                f'spec-lock: 付録C のラベル {lines[label_at].strip()!r} が spec/grammar.ebnf の {label!r} と違う')
        lines = lines[:open_at + 1] + body + lines[close_at:]
    return lines


generated = '\n'.join(rewrite(text.split('\n')))

if mode == 'write':
    if generated != text:
        SPEC.write_text(generated)
    print(f'spec-lock: {len(error_table()) - 2} エラーコード、{len(grammar_groups())} 文法群')
    raise SystemExit(0)

if generated == text:
    print(f'spec-lock --check: 一致（{len(error_table()) - 2} エラーコード、{len(grammar_groups())} 文法群）')
    raise SystemExit(0)

diff = difflib.unified_diff(text.split('\n'), generated.split('\n'),
                            'docs/spec.md', 'spec/ から生成したもの', lineterm='', n=1)
sys.stderr.write('\n'.join(diff) + '\n')
sys.stderr.write('spec-lock --check: docs/spec.md が spec/ と食い違う\n')
raise SystemExit(1)
PY
