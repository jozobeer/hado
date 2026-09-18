#!/usr/bin/env bash
# 文書層の検査（docs/plan.md 33 節）。docs/spec.md を「表として読む（処理系との
# 契約）」検査は門番に残し、「文章・見出し・対応表を逐語や個数で固定する（文書管理）」
# 検査だけをここへ移した。処理系を守らないので scripts/verify-*.sh には数えず、
# full 層の最後にだけ回す（scripts/run-gates.sh）。
#
# 移した元:
#   verify-m1-spec.sh の検査 1（見出し）・2（新設節）・4（対応表）・5（条件列の逐語）
#   verify-r38.sh の 38-c の逐語 regex・38-e の版文字列と ledger 節
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

SPEC="docs/spec.md"
PLAN="docs/plan.md"
COV="docs/spec-coverage.md"

TMPD="$(mktemp -d)"
trap 'rm -rf "$TMPD"' EXIT

# 引数は `§N`（N は整数か小数）または `付録X`。
heading_exists() {
    case "$1" in
        付録*) grep -qE "^## ${1} " "$SPEC" 2>/dev/null ;;
        §*) local number=${1#§}; grep -qE "^## ${number//./\\.}\\. " "$SPEC" 2>/dev/null ;;
        *) return 1 ;;
    esac
}

# --- 検査 1: 節番号（本文 §1〜§56 が `## N.` として 1 回ずつ読み順に、付録は A から順に） ---
doc_headings() {
    python3 - "$SPEC" <<'PY'
import re
import sys
from pathlib import Path

spec = Path(sys.argv[1]).read_text()
body, appendix, in_appendix = [], [], False
for line in spec.splitlines():
    if re.match(r'^## \d', line):
        assert not in_appendix, f'body section after the appendix: {line}'
        match = re.match(r'^## (\d+(?:\.\d+)?)\. ', line)
        assert match, line
        body.append(tuple(int(part) for part in match.group(1).split('.')))
    elif re.match(r'^## 付録', line):
        in_appendix = True
        match = re.match(r'^## (付録[A-Z]) ', line)
        assert match, line
        appendix.append(match.group(1))
assert body == sorted(body), 'body sections are not in reading order'
assert len(set(body)) == len(body), 'duplicate body section number'
integers = [number for number in body if len(number) == 1]
assert [number[0] for number in integers] == list(range(1, len(integers) + 1)), integers
assert appendix == [f'付録{chr(ord("A") + i)}' for i in range(len(appendix))], appendix
assert appendix, 'no appendix section'
PY
}

# --- 検査 2: 節は部の下にあり、部の見出しに節番号は無い ---
doc_parts() {
    local reasons="" seen=0 line
    while IFS= read -r line; do
        if printf '%s' "$line" | grep -qE '^# [0-9]+\.'; then
            reasons="$reasons numbered-part"
        fi
        if printf '%s' "$line" | grep -qE '^## ([0-9]+(\.[0-9]+)?\.|付録[A-Z] )'; then
            [ "$seen" -eq 1 ] || reasons="$reasons section-before-part"
        fi
        if printf '%s' "$line" | grep -qE '^# '; then
            seen=1
        fi
    done <"$SPEC"
    [ -z "$reasons" ]
}

# --- 検査 3: 対応表（docs/spec-coverage.md）が計画の表 2.2/2.3/9.2/11.2/13.2/18.4/24.2 と一致 ---
doc_coverage() {
    local cov_reasons="" nrows nplan item citem found name secs ref num s
    if [ ! -f "$COV" ]; then
        return 1
    fi
    awk -F '|' '
        {
            t=$2; name=$3; secs=$4
            gsub(/^ +| +$/, "", t)
            gsub(/^ +| +$/, "", name)
            gsub(/^ +| +$/, "", secs)
            if (t=="計画の表" || t=="" || t ~ /^-+$/) next
            if (name=="" || name=="項目名") next
            print name "\t" secs
        }
    ' "$COV" >"$TMPD/cov.tsv"

    awk -F '|' '
        /^### 2\.2/ { p=1; next }
        /^### 2\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan22.txt"
    awk -F '|' '
        /^### 2\.3/ { p=1; next }
        /^## 3\./ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan23.txt"
    awk -F '|' '
        /^### 9\.2/ { p=1; next }
        /^### 9\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan92.txt"
    awk -F '|' '
        /^### 11\.2/ { p=1; next }
        /^### 11\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan112.txt"
    awk -F '|' '
        /^### 13\.2/ { p=1; next }
        /^### 13\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan132.txt"
    # Why not /^### 18\.4/: 19節に「18.4 の項目の割り当て」があり、同じ項目名が二重に入る。
    awk -F '|' '
        /^### 18\.4 仕様の項目/ { p=1; next }
        /^### 18\.5/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan184.txt"
    awk -F '|' '
        /^### 24\.2 仕様の項目/ { p=1; next }
        /^### 24\.3/ { p=0 }
        p {
            name=$2
            gsub(/^ +| +$/, "", name)
            if (name=="" || name=="項目" || name ~ /^-+$/) next
            if ($0 ~ /^\|/) print name
        }
    ' "$PLAN" >"$TMPD/plan242.txt"
    cat "$TMPD/plan22.txt" "$TMPD/plan23.txt" "$TMPD/plan92.txt" "$TMPD/plan112.txt" "$TMPD/plan132.txt" "$TMPD/plan184.txt" "$TMPD/plan242.txt" >"$TMPD/plan_items.txt"
    nrows=$(wc -l <"$TMPD/cov.tsv" | tr -d ' ')
    nplan=$(wc -l <"$TMPD/plan_items.txt" | tr -d ' ')
    if [ "$nrows" -ne "$nplan" ]; then
        cov_reasons="$cov_reasons rows=$nrows(want-$nplan)"
    fi
    cut -f1 "$TMPD/cov.tsv" >"$TMPD/cov_items.txt"

    while IFS= read -r item || [ -n "$item" ]; do
        found=0
        while IFS= read -r citem || [ -n "$citem" ]; do
            if [ "$item" = "$citem" ]; then
                found=1
                break
            fi
        done <"$TMPD/cov_items.txt"
        if [ "$found" -eq 0 ]; then
            cov_reasons="$cov_reasons missing-item"
            break
        fi
    done <"$TMPD/plan_items.txt"

    while IFS= read -r citem || [ -n "$citem" ]; do
        found=0
        while IFS= read -r item || [ -n "$item" ]; do
            if [ "$citem" = "$item" ]; then
                found=1
                break
            fi
        done <"$TMPD/plan_items.txt"
        if [ "$found" -eq 0 ]; then
            cov_reasons="$cov_reasons extra-item"
            break
        fi
    done <"$TMPD/cov_items.txt"

    while IFS=$'\t' read -r name secs; do
        printf '%s\n' "$secs" | grep -oE '§[0-9]+(\.[0-9]+)?|付録[A-Z]' | while read -r ref; do
            if ! heading_exists "$ref"; then
                echo " missing-sec-$ref"
            fi
        done >"$TMPD/covsec.$$"
        if [ -s "$TMPD/covsec.$$" ]; then
            cov_reasons="$cov_reasons$(cat "$TMPD/covsec.$$")"
        fi
        rm -f "$TMPD/covsec.$$"
    done <"$TMPD/cov.tsv"

    [ -z "$cov_reasons" ]
}

# --- 検査 4: 条件列の逐語（新しい改行要求が入っていない） ---
doc_condition_column() {
    local a b
    a=$(awk 'index($0, "条件列") && index($0, "改行") { n++ } END { print n+0 }' "$SPEC")
    b=$(awk 'index($0, "パーサは改行を見ない") { n++ } END { print n+0 }' "$SPEC")
    [ "$a" -eq 0 ] && [ "$b" -ge 1 ]
}

# --- 検査 5: 版（spec と spec-coverage が同じ版を名乗る） ---
doc_version() {
    local sv cv
    sv=$(head -n 1 "$SPEC" | grep -oE 'v[0-9]+(\.[0-9]+)+' | head -n 1)
    cv=$(head -n 1 "$COV" | grep -oE 'v[0-9]+(\.[0-9]+)+' | head -n 1)
    [ -n "$sv" ] && [ "$sv" = "$cv" ]
}

# --- 検査 6: # round: を持つ門番にラウンドファイルと実測がある ---
doc_round_files() {
    python3 - <<'PY'
import re
from pathlib import Path

slug_ok = re.compile(r'^[a-z0-9][a-z0-9-]*$')
reserved = re.compile(r'^r[0-9]+(-r[0-9]+)?$')
header = re.compile(r'^# round: (.*)$', re.M)
fence = re.compile(r'^```')
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
    round_path = Path('docs/rounds') / f'{slug}.md'
    assert round_path.is_file(), path.name
    text = round_path.read_text().splitlines()
    try:
        idx = next(i for i, line in enumerate(text) if line == '## 実測')
    except StopIteration:
        raise AssertionError(path.name)
    end = next(
        (
            i
            for i, line in enumerate(text[idx + 1 :], idx + 1)
            if line.startswith('## ') and not line.startswith('###')
        ),
        len(text),
    )
    assert any(fence.match(line) for line in text[idx + 1 : end]), path.name
PY
}

# --- 検査 7: R38 が足した規則の文が spec に逐語で 1 回ずつある ---
# r38 の構造検査（probe の key と §番号の対応・見出しの実在）は verify-r38.sh に残る。
doc_r38_sentences() {
    python3 - <<'PY'
import re
from pathlib import Path
rules = {
    '38-c(1)': (21, r'^Array の `compare` は要素を先頭から比較して最初に異なる要素の順序を返し、共通部分がすべて等しいときだけ短い方を先とする。$'),
    '38-c(2)': (55, r'^`Int\.parse` の受理文法は、往復から導いた下限として `Int\.to_string` が返す全ての Int の10進表記を含み、'
                   r'本節が決めた上限として ASCII の `-\?\[0-9\]\+` に一致して値が -9223372036854775808 以上 9223372036854775807 以下である文字列だけを受理し、'
                   r'先行ゼロの個数は制限せず、`\+`・空白・`_`・小数点・全角数字は拒否する。$'),
    '38-c(3)': (55, r'^`read_dir` は指定したディレクトリの各項目の名前だけを返し、`\.` と `\.\.` を除いて UTF-8 のバイト順に整列する。$'),
    '38-c(4)': (55, r'^`read_stdin` は標準入力を EOF まで読み、同じプロセスでの2回目の呼び出しは空の `Bytes` を返す。$'),
    '38-c(5)': (55, r'^`env` の値が非 UTF-8 のときも、`message` が `"InvalidUtf8"` の `Error` を raise する。$'),
    '38-c(6)': (49, r'^既定実装の契約名 `EnvDefault` と `IoDefault` は契約の名前空間で予約し、利用者の同名の契約宣言は E205 とする。$'),
}
def sections(text):
    return dict(re.findall(r'^## (\d+)\.[^\n]*\n(.*?)(?=^## \d+\.|\Z)', text, re.M | re.S))
def present(text):
    found = sections(text)
    for key, (section, pattern) in rules.items():
        assert str(section) in found, f'{key}: missing section {section}'
        assert len(re.findall(pattern, found[str(section)], re.M)) == 1, key
spec = Path('docs/spec.md').read_text()
present(spec)
for key, (_, pattern) in rules.items():
    try:
        present(re.sub(pattern, '', spec, flags=re.M))
    except AssertionError:
        pass
    else:
        raise AssertionError(f'{key}: missing sentence accepted')
print('coverage: ' + ' '.join(f'§{section}={sum(1 for s, _ in rules.values() if s == section)}' for section in (21, 49, 55)))
PY
}

# --- 検査 8: issue の原因がパスと識別子で場所を指す（docs/issues.md のファイルの形） ---
doc_issue_causes() {
    python3 - <<'PY'
import re
from pathlib import Path

line_anchor = re.compile(r'((?:\.\./)*[A-Za-z0-9_.][A-Za-z0-9_./-]*):([0-9]+)(?:-([0-9]+))?(?!:?[0-9])')
path_token = re.compile(r'`((?:\.\./)*[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+)`')
location = re.compile(r'`((?:\.\./)*[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+)`[ \t]*の[ \t]*`([^`]+)`')
section = re.compile(r'(§[0-9]+(?:\.[0-9]+)?|付録[A-Z])(?![0-9A-Za-z_.])')
error_code = re.compile(r'(?<![A-Za-z0-9_])E[0-9]{3}(?![0-9A-Za-z_])')

bare_line = re.compile(r'(?<![0-9A-Za-z_.:]):([0-9]+)(?![0-9:])')

def lines(text):
    """`file:line` と、パスを省いた `:line` の綴り。規約により原因は行番号を書かない。"""
    spelled = [m for m in line_anchor.findall(text) if not re.fullmatch(r'[0-9.]+', m[0])]
    return spelled + [('', m.group(1), '') for m in bare_line.finditer(text)]

def paths(text):
    """バッククォートで囲んだリポジトリのパス。`List.length` のような member は含まない。"""
    return path_token.findall(text)

def pairs(text):
    """`パス` の `識別子`。同じパスの識別子は `・` か `、` か空白で続けて書ける。"""
    found = []
    for match in location.finditer(text):
        target = match.group(1)
        found.append((target, match.group(2)))
        rest = text[match.end():]
        while True:
            following = re.match(r'[ \t]*[・、]?[ \t]*`([^`]+)`', rest)
            if not following or is_path(following.group(1)):
                break
            found.append((target, following.group(1)))
            rest = rest[following.end():]
    return found

def is_path(token):
    """バッククォートの中身がパスか（`/` を含むものだけをパスと読む）。"""
    return re.fullmatch(r'(?:\.\./)*[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)+', token) is not None

def mentions(source, ident):
    """識別子は語の全体で照合する（`make_is` は `make_isa` に一致しない）。"""
    pattern = r'(?<![A-Za-z0-9_])' + re.escape(ident) + r'(?![A-Za-z0-9_])'
    return re.search(pattern, source) is not None

assert lines('crates/hado/src/mono.rs:4407') == [('crates/hado/src/mono.rs', '4407', '')], \
    'a file:line spelling is detected so that it can be rejected'
assert lines('crates/hado/src/mono.rs:4407-4532') == [('crates/hado/src/mono.rs', '4407', '4532')], \
    'a ranged file:line spelling is detected too'
assert lines('main.hd:1:36') == [], 'a diagnostic span is not a line anchor'
assert lines('§47 と `E602`') == [], 'a section number or error code is not a line anchor'
assert lines('と `:391` の `hado_string_find`') == [('', '391', '')], \
    'a line number without its path is still a line number'
assert lines('12:30 と main.hd:1:36') == [], \
    'a time and a diagnostic span are not line anchors'
assert paths('`crates/hado/src/mono.rs` の `derive_comparison`') == ['crates/hado/src/mono.rs']
assert paths('`docs/wants.md` 53 と `stdlib/float.hd`') == ['docs/wants.md', 'stdlib/float.hd'], \
    'a document path and a stdlib path are both found'
assert paths('`List.length` は member であってパスではない') == [], \
    'a dotted member name is not a path'
assert paths('`E602` と `§47`') == [], 'an error code and a section are not paths'
assert pairs('`crates/hado/src/mono.rs` の `derive_comparison`') == \
    [('crates/hado/src/mono.rs', 'derive_comparison')]
assert pairs('`crates/hado/src/lib.rs` の `check_ast` `diagnostic_order`。次') == \
    [('crates/hado/src/lib.rs', 'check_ast'), ('crates/hado/src/lib.rs', 'diagnostic_order')], \
    'identifiers written after the path belong to the same file'
assert pairs('`crates/hado/src/lib.rs` の `check_ast` が') == \
    [('crates/hado/src/lib.rs', 'check_ast')], 'prose after the identifier ends the pair'
assert section.findall('§26.1 と §55 と 付録C') == ['§26.1', '§55', '付録C']
assert section.findall('§55x と 付録Cfoo と §55.1') == ['§55.1'], \
    'a section that continues with a word character is not a section'
assert error_code.findall('E602 と `E100` と E3xx と E1000') == ['E602', 'E100'], \
    'an error code is read with or without backticks, and E3xx and E1000 are not codes'
assert error_code.findall('E100foo と E602') == ['E602'], \
    'an error code that continues with a word character is not a code'
assert mentions('fn make_isa() -> Isa {', 'make_isa'), 'a whole word matches'
assert not mentions('fn make_isa() -> Isa {', 'make_is'), \
    'a prefix of an identifier does not match (make_is is not make_isa)'
assert not mentions('fn make_isa_slow()', 'make_isa'), \
    'an identifier that extends the name does not match'
assert mentions('def rec to_list values =>', 'to_list'), 'a Hado def matches'
assert not mentions('def rec to_listx values =>', 'to_list'), 'a longer Hado name does not'
assert pairs('`crates/hado/src/lib.rs` の `check_ast`・`diagnostic_order`。') == \
    [('crates/hado/src/lib.rs', 'check_ast'), ('crates/hado/src/lib.rs', 'diagnostic_order')], \
    'identifiers separated by a middle dot belong to the same file'
assert pairs('`crates/hado/src/lib.rs` の `check_ast`、`diagnostic_order`') == \
    [('crates/hado/src/lib.rs', 'check_ast'), ('crates/hado/src/lib.rs', 'diagnostic_order')], \
    'identifiers separated by a comma belong to the same file'
assert pairs('`crates/hado/src/lib.rs` の `check_ast`、`crates/hado/src/check.rs` の `check_ast`') == \
    [('crates/hado/src/lib.rs', 'check_ast'), ('crates/hado/src/check.rs', 'check_ast')], \
    'a second path starts its own location instead of continuing the first'
assert is_path('crates/hado/src/lib.rs'), 'a path is a path'
assert not is_path('check_ast'), 'an identifier is not a path'
root = Path.cwd().resolve()

def classify(root, target):
    fp = (root / target).resolve()
    if not fp.is_relative_to(root):
        return 'escapes'
    if not fp.is_file():
        return 'missing'
    return 'ok'

assert classify(root, 'AGENTS.md') == 'ok'
assert classify(root, 'crates/../AGENTS.md') == 'ok'
assert classify(root, 'crates/../../AGENTS.md') == 'escapes', \
    'path that leaves the repository must not pass'
assert classify(root, '../local-notes.rs') == 'escapes', \
    'parent-relative path must not pass'
assert classify(root, 'missing.rs') == 'missing'
assert path_token.findall('`../local-notes.rs` と `crates/hado/src/emit.rs`') == [
    '../local-notes.rs', 'crates/hado/src/emit.rs',
], 'a leading ../ stays in the path'
assert classify(root, 'crates/../../AGENTS.md') == 'escapes'

import tempfile
with tempfile.TemporaryDirectory() as td:
    td = Path(td).resolve()
    repo = td / 'repo'
    repo.mkdir()
    outside = td / 'local-notes.rs'
    outside.write_text('x\n')
    (repo / 'inside.rs').write_text('x\n')
    (repo / 'link.rs').symlink_to(outside)
    assert classify(repo, 'inside.rs') == 'ok'
    assert classify(repo, '../local-notes.rs') == 'escapes', \
        'parent-relative existing file must not pass'
    assert classify(repo, 'link.rs') == 'escapes', \
        'symlink target outside the repository must not pass'
    assert classify(repo, 'crates/../../local-notes.rs') == 'escapes', \
        'resolved path outside the repository must not pass even if that file exists'

spec_text = (root / 'docs/spec.md').read_text()
spec_headings = re.findall(r'^## .*$', spec_text, re.M)
spec_codes = set(re.findall(r'^\| (E[0-9]{3}) \|', spec_text, re.M))

def known_section(ref):
    if ref.startswith('付録'):
        return any(heading.startswith(f'## {ref} ') for heading in spec_headings)
    return f'## {ref[1:]}. ' in spec_text

assert known_section('§55'), 'the spec has §55'
assert known_section('§26.1'), 'the spec has §26.1'
assert known_section('付録C'), 'the spec has 付録C'
assert not known_section('§999'), 'an unknown section is rejected'
assert not known_section('付録Z'), 'an unknown appendix is rejected'
assert spec_codes, 'the spec has an error code table'

for path in sorted(Path('docs/issues').glob('*.md')):
    text = path.read_text()
    match = re.search(r'^## 原因$\n(.*?)(?=^## |\Z)', text, re.M | re.S)
    assert match, f'{path.name}: no 原因 section'
    body = match.group(1)
    spelled = lines(body)
    assert not spelled, \
        f'{path.name}: 原因 writes a line number: {spelled[0][0]}:{spelled[0][1]}（`パス` の `識別子` で指す）'
    for target in paths(body):
        status = classify(root, target)
        assert status != 'escapes', f'{path.name}: 原因 path escapes the repository: {target}'
        assert status != 'missing', f'{path.name}: 原因 path does not exist: {target}'
    for target, ident in pairs(body):
        assert classify(root, target) == 'ok', \
            f'{path.name}: 原因 location file does not exist: {target} の {ident}'
        source = (root / target).read_text(errors='replace')
        assert mentions(source, ident), f'{path.name}: 原因 identifier is not in {target}: {ident}'
    for ref in section.findall(body):
        assert known_section(ref), f'{path.name}: 原因 section does not exist in the spec: {ref}'
    for code in error_code.findall(body):
        assert code in spec_codes, f'{path.name}: 原因 error code is not in the spec table: {code}'
    located = pairs(body) or section.findall(body) or error_code.findall(body)
    assert located, \
        f'{path.name}: 原因 points at nothing（`パス` の `識別子`、§N、Exxx のどれかを書く）'
PY
}

# --- 検査 9: issue の現象が実測をコードフェンスか表で持つ（docs/issues.md のファイルの形） ---
doc_issue_evidence() {
    python3 - <<'PY'
import re
from pathlib import Path

fence_open = re.compile(r'^(`{3,})([^`]*)$')
fence_close = re.compile(r'^(`{3,})\s*$')
placeholders = {'', '?', '-', '--', '---', '...', '…', 'tbd', 'todo', 'xxx', 'n/a', 'na', '未定', '未計測'}

def closes_fence(line, delimiter):
    """A closing fence is only backticks (at least as many as the opener) and spaces."""
    match = fence_close.match(line)
    return bool(match) and len(match.group(1)) >= len(delimiter)

def transcript(block):
    """実測の約束（docs/issues.md）: `$ ` で始まる行がコマンド、`> ` で始まる行は
    PS2 の入力、`#` で始まらないプレースホルダでない空でない行が出力である。
    シェルの文法（継続・heredoc・置換）は解析しない。コマンドと出力の対応は
    レビューで見る。"""
    commands = sum(1 for line in block if line.startswith('$ '))
    outputs = sum(1 for line in block
                  if not line.startswith('$ ') and not line.startswith('> ')
                  and not line.lstrip().startswith('#')
                  and line.strip() and line.strip().lower() not in placeholders)
    return commands >= 1 and outputs >= 1

def complete_fence(lines):
    i = 0
    while i < len(lines):
        opening = fence_open.match(lines[i])
        if opening:
            delimiter = opening.group(1)
            j = i + 1
            while j < len(lines) and not closes_fence(lines[j], delimiter):
                j += 1
            if j < len(lines) and transcript(lines[i + 1:j]):
                return True
            i = j + 1
        else:
            i += 1
    return False

def cells(line):
    parts = re.split(r'(?<!\\)\|', line.strip())
    if parts and not parts[0].strip():
        parts = parts[1:]
    if parts and not parts[-1].strip():
        parts = parts[:-1]
    return [part.replace('\\|', '|').strip() for part in parts]

alignment = re.compile(r':?-{3,}:?')

def measured(row):
    # `:---:` in a data cell is an alignment marker, not a measurement.
    return all(cell and cell.lower() not in placeholders
               and not alignment.fullmatch(cell) for cell in row)

def complete_table(lines):
    for i in range(len(lines) - 2):
        if not (lines[i].startswith('|') and lines[i + 1].startswith('|') and lines[i + 2].startswith('|')):
            continue
        header = cells(lines[i])
        separator = cells(lines[i + 1])
        if not header or any(not cell for cell in header):
            continue
        if len(separator) != len(header) or any(not re.fullmatch(r':?-{3,}:?', cell) for cell in separator):
            continue
        j = i + 2
        ok = True
        while j < len(lines) and lines[j].startswith('|'):
            row = cells(lines[j])
            if len(row) != len(header) or not measured(row):
                ok = False
                break
            j += 1
        if ok:
            return True
    return False

def fence(*inner):
    return ['```', *inner, '```']

# フェンスの形（Markdown の規則）。中身の綴りは解析しない。
assert not complete_fence(['```lang`bad', '$ cargo test', 'ok', '```']), \
    'an opening fence whose info string contains a backtick does not open a block'
assert not complete_fence(['```', '$ cargo test', '```not-a-closing-fence']), \
    'a fence line with trailing text does not close the block'
assert complete_fence(['```', '$ cargo test', 'PASSED: 9/9', '```   ']), \
    'a closing fence with trailing spaces closes the block'
assert complete_fence(['````', '$ cargo test', 'PASSED: 9/9', '````']), \
    'a four-backtick fence closes with four backticks'
assert not complete_fence(['````', '$ cargo test', 'PASSED: 9/9', '```']), \
    'a shorter fence does not close a longer one'
assert not complete_fence(['   ```', '$ cargo test', 'ok', '   ```']), \
    'an indented fence is not read (fences start at the line head)'

# 実測の約束: コマンドと出力が 1 行ずつ以上あることだけを見る。
assert complete_fence(fence('$ cargo test', 'PASSED: 9/9')), \
    'a command and its output are evidence'
assert not complete_fence(fence('$ cargo test')), \
    'a command-only fence is not evidence'
assert not complete_fence(fence('PASSED: 9/9')), \
    'output without a command is not evidence'
assert not complete_fence(fence('$ cargo test', 'TBD')), \
    'a placeholder output is not evidence'
assert not complete_fence(fence('$ cargo test', '  ...')), \
    'an indented placeholder output is not evidence'
assert not complete_fence(fence('$ cargo test', '> TBD')), \
    'a PS2 line is not output'
assert not complete_fence(fence('$ cargo test', '# output TBD')), \
    'a comment line is not output'
assert complete_fence(fence('$ printf x \\', '> tee out', 'x')), \
    'a continued command with an output line is evidence'

assert not complete_table(['| metric |', '| : |', '| TBD |']), \
    'colon-only separator is not a table'
assert not complete_table(['| metric |', '| :-: |', '| TBD |']), \
    'one-hyphen aligned separator is not a table'
assert not complete_table(['| metric |', '| -- |', '| TBD |']), \
    'two-hyphen separator is not a table'
assert complete_table(['| metric |', '| --- |', '| 1.2 |']), \
    'three-hyphen separator is a table'
assert complete_table(['| left | mid | right |', '| :--- | :---: | ---: |', '| a | b | c |']), \
    'aligned three-hyphen separators are a table'
assert not complete_table(['| metric |', '| --- |', '| TBD |']), \
    'placeholder-only data row is not evidence'
assert not complete_table(['| metric |', '| --- |', '| - |']), \
    'dash-only data row is not evidence'
assert not complete_table(['| metric |', '| --- |', '| :---: |']), \
    'an aligned separator as a data row is not evidence'
assert not complete_table(['| metric |', '| --- |', '| :--- |']), \
    'a left-aligned separator as a data row is not evidence'
assert not complete_table(['| metric |', '| --- |', '| ---: |']), \
    'a right-aligned separator as a data row is not evidence'
assert complete_table(['| metric |', '| --- |', '| 1.2 s |']), \
    'a real data row with an alignment separator header is evidence'
assert not complete_table(['| case | result |', '| --- | --- |', '| case-a | TBD |']), \
    'a placeholder in a result cell is not evidence'
assert not complete_table(['| case | result | time |', '| --- | --- | --- |', '| case-a | ok | TBD |']), \
    'a placeholder in any cell is not evidence'
assert not complete_table(['| metric |', '| --- |', '| real |', '| TBD |']), \
    'a placeholder data row invalidates the table'
assert complete_table(['| 対象 | 入力 | 時間 |', '| --- | --- | --- |', '| 同 | 20,000 行 | 0.33 s |']), \
    'measured data row counts'

def outside_fences(lines):
    """Lines that are not inside a fenced block, so an example table is not evidence."""
    result = []
    delimiter = None
    for line in lines:
        if delimiter is None:
            opening = fence_open.match(line)
            if opening:
                delimiter = opening.group(1)
                continue
            result.append(line)
        elif closes_fence(line, delimiter):
            delimiter = None
    return result

assert not complete_table(outside_fences(['```', '| expected |', '| --- |', '| 1 |', '```'])), \
    'a table inside a fence is not an evidence table'
assert complete_table(outside_fences(['| expected |', '| --- |', '| 1 |'])), \
    'a table outside fences is evidence'
assert complete_table(outside_fences(['```', 'x', '```', '| expected |', '| --- |', '| 1 |'])), \
    'a table after a closed fence is evidence'

missing = []
for path in sorted(Path('docs/issues').glob('*.md')):
    text = path.read_text()
    match = re.search(r'^## 現象$\n(.*?)(?=^## |\Z)', text, re.M | re.S)
    assert match, f'{path.name}: no 現象 section'
    lines = match.group(1).split('\n')
    if not (complete_fence(lines) or complete_table(outside_fences(lines))):
        missing.append(path.name)
assert not missing, '現象 has no closed nonempty fence or complete table: ' + ', '.join(missing)
PY
}

# --- 検査 10: issue のファイル名と文書の形（docs/issues.md の識別子とファイルの形） ---
doc_issue_names() {
    python3 - <<'PYEOF'
import re
from pathlib import Path

slug = re.compile(r'[a-z0-9][a-z0-9-]*')
reserved = re.compile(r'r[0-9]+(-r[0-9]+)?')
sections = ['## 現象', '## 原因', '## 案', '## 完了条件', '## 由来']

def usable(name):
    return bool(slug.fullmatch(name)) and not reserved.fullmatch(name)

def issue_problem(name, text):
    lines = text.split('\n')
    head = lines[0] if lines else ''
    match = re.fullmatch(r'# (.+)（([^（）]+)）', head)
    if not match:
        return 'heading is not `# <題>（<slug>）`'
    if match.group(2) != name:
        return f'heading slug {match.group(2)} does not match the file name'
    priority = lines[1] if len(lines) > 1 else ''
    if not re.fullmatch(r'優先度: (高|中|低)。', priority):
        return 'line 2 is not `優先度: 高|中|低。`'
    heads = [line for line in lines if line in sections]
    if heads != sections:
        return f'sections must be {sections} in order, found {heads}'
    return None

assert usable('dogfood-text-tool')
assert not usable('bad_name'), 'an underscore is not in the slug grammar'
assert not usable('Bad-Name'), 'an uppercase letter is not in the slug grammar'
assert not usable('-lead'), 'a leading hyphen is not in the slug grammar'
assert not usable('r82'), 'r<digits> is reserved for the old round files'
assert not usable('r50-r51'), 'r<digits>-r<digits> is reserved'

full = ('# 題（good-slug）\n優先度: 高。\n## 現象\na\n## 原因\nb\n'
        '## 案\nc\n## 完了条件\nd\n## 由来\ne\n')
assert issue_problem('good-slug', full) is None
assert 'heading' in issue_problem('good-slug', full.replace('# 題（good-slug）', '# 題'))
assert 'does not match' in issue_problem('other-slug', full)
assert '優先度' in issue_problem('good-slug', full.replace('優先度: 高。', '優先度: 最高。'))
assert '## 案' in issue_problem('good-slug', full.replace('## 案\n', ''))
assert '## 完了条件' in issue_problem(
    'good-slug', full.replace('## 完了条件\nd\n', '## 完了条件\na\n## 由来\nb\n'))

root = Path('docs/issues')
assert root.is_dir(), 'docs/issues is not a directory'
for entry in sorted(root.iterdir()):
    assert not entry.is_symlink() and entry.is_file(), \
        f'{entry}: docs/issues holds only regular files (no nested paths)'
    assert entry.suffix == '.md', f'{entry.name}: issue files are .md'

for path in sorted(root.glob('*.md')):
    name = path.stem
    assert usable(name), f'{path.name}: file name is not a usable slug'
    problem = issue_problem(name, path.read_text())
    assert problem is None, f'{path.name}: {problem}'
PYEOF
}

check 'spec sections §1..§56 and 付録A.. exist once in reading order' doc_headings
check 'spec sections sit under part headings' doc_parts
check 'spec-coverage matches the plan tables 2.2/2.3/9.2/11.2/13.2/18.4/24.2' doc_coverage
check 'condition column prose does not require a newline' doc_condition_column
check 'spec and spec-coverage name the same version' doc_version
check 'each gate with a round header has a round file with recorded results' doc_round_files
check 'R38 spec sentences are pinned verbatim once each' doc_r38_sentences
check 'each issue cause points at a path and an identifier' doc_issue_causes
check 'each issue 現象 carries measured evidence in a fence or table' doc_issue_evidence
check 'each issue document has the required name, priority and sections' doc_issue_names

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
