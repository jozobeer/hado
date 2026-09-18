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

# --- 検査 8: issue の原因が file:line を指す（docs/issues.md のファイルの形） ---
doc_issue_causes() {
    python3 - <<'PY'
import re
import tempfile
from pathlib import Path

anchor = re.compile(r'((?:\.\./)*[A-Za-z0-9_][A-Za-z0-9_./-]*\.[A-Za-z0-9]+):([0-9]+)(?:-([0-9]+))?')
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
assert anchor.findall('../local-notes.rs:1 and crates/hado/src/emit.rs:23') == [
    ('../local-notes.rs', '1', ''),
    ('crates/hado/src/emit.rs', '23', ''),
], 'leading ../ must be captured as an anchor'
assert anchor.findall('crates/../../AGENTS.md:1') == [('crates/../../AGENTS.md', '1', '')]
assert anchor.findall('crates/hado/src/project.rs:313-339') == [('crates/hado/src/project.rs', '313', '339')], \
    'a ranged anchor must capture its end line'

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

for path in sorted(Path('docs/issues').glob('*.md')):
    text = path.read_text()
    match = re.search(r'^## 原因$\n(.*?)(?=^## |\Z)', text, re.M | re.S)
    assert match, f'{path.name}: no 原因 section'
    found = anchor.findall(match.group(1))
    assert found, f'{path.name}: 原因 has no file:line anchor'
    for target, line, end in found:
        status = classify(root, target)
        assert status != 'escapes', \
            f'{path.name}: 原因 anchor escapes the repository: {target}:{line}'
        assert status != 'missing', \
            f'{path.name}: 原因 anchor file does not exist: {target}:{line}'
        fp = (root / target).resolve()
        count = len(fp.read_text().splitlines())
        assert 1 <= int(line) <= count, \
            f'{path.name}: 原因 anchor line is out of range: {target}:{line} (file has {count} lines)'
        if end:
            assert int(line) <= int(end), \
                f'{path.name}: 原因 anchor range is reversed: {target}:{line}-{end}'
            assert int(end) <= count, \
                f'{path.name}: 原因 anchor range end is out of range: {target}:{line}-{end} (file has {count} lines)'
PY
}

# --- 検査 9: issue の現象が実測をコードフェンスか表で持つ（docs/issues.md のファイルの形） ---
doc_issue_evidence() {
    python3 - <<'PY'
import re
from pathlib import Path

def complete_fence(lines):
    i = 0
    while i < len(lines):
        if lines[i].startswith('```'):
            block = []
            j = i + 1
            while j < len(lines) and not lines[j].startswith('```'):
                block.append(lines[j])
                j += 1
            if j < len(lines):
                commands = 0
                outputs = 0
                heredoc = None
                shell = None
                here_re = re.compile(
                    r"""<<-?\s*(?:'([A-Za-z_][A-Za-z0-9_]*)'|"([A-Za-z_][A-Za-z0-9_]*)"|\\?([A-Za-z_][A-Za-z0-9_]*))"""
                )
                cont_re = re.compile(r'(\||&&|\\)\s*$')

                class Shell:
                    """State of one command, so PS2 input lines are never counted as measured output."""

                    OPEN = {'if': 'fi', 'for': 'done', 'while': 'done', 'until': 'done',
                            'case': 'esac', '{': '}'}
                    CLOSE = {'fi', 'done', 'esac', '}'}
                    COMMAND_AFTER = {';', '|', '&', '(', '{', '!',
                                     'if', 'then', 'else', 'elif', 'do', 'for', 'while', 'until', 'case'}
                    BREAK = set(';{}|&()')

                    def __init__(self):
                        self.single = False
                        self.double = False
                        self.depth = 0
                        self.expect = []
                        self.tail = False

                    def feed(self, text):
                        if text.startswith('$ ') or text.startswith('> '):
                            text = text[2:]
                        words = []
                        word = []

                        def flush():
                            if word:
                                words.append(''.join(word))
                                word.clear()

                        self.tail = False
                        i = 0
                        while i < len(text):
                            char = text[i]
                            if self.single:
                                if char == "'":
                                    self.single = False
                                else:
                                    word.append(char)
                            elif self.double:
                                if char == '\\':
                                    i += 1
                                    if i < len(text):
                                        word.append(text[i])
                                elif char == '"':
                                    self.double = False
                                else:
                                    word.append(char)
                            elif char == '\\':
                                if i + 1 >= len(text):
                                    self.tail = True
                                else:
                                    i += 1
                                    word.append(text[i])
                            elif char == "'":
                                self.single = True
                            elif char == '"':
                                self.double = True
                            elif char in self.BREAK:
                                flush()
                                words.append(char)
                                if char == '(':
                                    self.depth += 1
                                elif char == ')':
                                    self.depth = max(0, self.depth - 1)
                            elif char.isspace():
                                flush()
                            else:
                                word.append(char)
                            i += 1
                        flush()
                        position = True
                        previous = None
                        for token in words:
                            if token == '{':
                                # `f() {` and `function f {` open a group outside command position.
                                if previous == ')' or previous is None or words[0] == 'function':
                                    self.expect.append('}')
                            elif position and token in self.OPEN:
                                self.expect.append(self.OPEN[token])
                            elif token in self.CLOSE and self.expect and self.expect[-1] == token:
                                self.expect.pop()
                            position = token in self.COMMAND_AFTER
                            previous = token
                        trailing = bool(cont_re.search(text.rstrip()))
                        if words and words[-1] in ('|', '&', '('):
                            trailing = True
                        return (self.single or self.double or self.depth > 0 or self.tail
                                or bool(self.expect) or trailing)

                for line in block:
                    if heredoc is not None:
                        if line.strip() == heredoc:
                            heredoc = None
                        continue
                    if shell is not None:
                        if not shell.feed(line):
                            shell = None
                        continue
                    if line.startswith('$ '):
                        commands += 1
                        here = here_re.search(line)
                        if here:
                            heredoc = next(g for g in here.groups() if g)
                        else:
                            shell = Shell()
                            if not shell.feed(line):
                                shell = None
                        continue
                    if not line.strip() or line.lstrip().startswith('#'):
                        continue
                    outputs += 1
                if commands and outputs:
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

placeholders = {'', '?', '-', '--', '---', '...', '…', 'tbd', 'todo', 'xxx', 'n/a', 'na', '未定', '未計測'}

def measured(row):
    return all(cell and cell.lower() not in placeholders for cell in row)

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

assert not complete_fence(fence("$ cat > probe <<'EOF'", 'body', 'EOF')), \
    'single-quoted here-doc body only must not count as output'
assert not complete_fence(fence('$ cat > probe <<"EOF"', 'body', 'EOF')), \
    'double-quoted here-doc body only must not count as output'
assert not complete_fence(fence(r'$ cat > probe <<\EOF', 'body', 'EOF')), \
    'backslash-quoted here-doc body only must not count as output'
assert not complete_fence(fence('$ cargo test', '# output TBD')), \
    'comment-only output must not count'
assert not complete_fence(fence('$ cargo test', '  # output TBD')), \
    'indented comment-only output must not count'
assert complete_fence(fence("$ python3 - <<'PY'", 'print(1)', 'PY', '1')), \
    'here-doc followed by transcript output must count'
assert complete_fence(fence('$ cargo test', 'PASSED: 9/9')), \
    'command plus transcript output must count'
assert not complete_fence(fence("$ printf '%s\\n' \\", '> input.txt')), \
    'backslash continuation and PS2 line must not count as output'
assert not complete_fence(fence("$ printf '%s\\n' \\", '> foo \\', '> bar')), \
    'continued-only multiline command must not count as output'
assert complete_fence(fence("$ printf '%s\\n' \\", '> input.txt', 'input.txt')), \
    'continued command plus transcript output must count'
assert not complete_fence(fence('$ printf x |', '> tee result.txt')), \
    'pipe continuation and PS2 line must not count as output'
assert not complete_fence(fence('$ printf x &&', '> tee result.txt')), \
    'and-and continuation and PS2 line must not count as output'
assert complete_fence(fence('$ printf x |', '> tee result.txt', 'x')), \
    'continued pipeline plus output must count'
assert not complete_fence(fence('$ printf \'%s\\n\' "', '> input"')), \
    'unmatched double quote and PS2 line must not count as output'
assert not complete_fence(fence("$ printf '%s\\n' '", "> input'")), \
    'unmatched single quote and PS2 line must not count as output'
assert not complete_fence(fence('$ (cd sub', '> )')), \
    'unbalanced parenthesis and PS2 line must not count as output'
assert complete_fence(fence('$ printf \'%s\\n\' "', '> input"', 'input')), \
    'quoted continuation plus output must count'
assert complete_fence(fence("$ awk '/^## 現象/{p=1}' file.md", 'a')) , \
    'balanced quotes in a single command must count as complete'
assert complete_fence(fence('$ echo "(x)"', 'x')) , \
    'balanced quotes and parentheses must count as complete'
assert not complete_fence(fence('$ if true; then', '> echo x', '> fi')), \
    'if/then/fi compound and PS2 lines must not count as output'
assert not complete_fence(fence('$ for i in a; do', '> echo $i', '> done')), \
    'for/do/done compound and PS2 lines must not count as output'
assert not complete_fence(fence('$ { echo a;', '> }')), \
    'brace group and PS2 line must not count as output'
assert complete_fence(fence('$ if true; then', '> echo x', '> fi', 'x')), \
    'compound continuation plus output must count'
assert complete_fence(fence('$ while read x; do echo $x; done', 'x')), \
    'a compound on one line must count as complete'
assert complete_fence(fence('$ grep -w if file', 'if')), \
    'if as an argument must not open a compound'
assert not complete_fence(fence('$ f() {', '> echo x', '> }')), \
    'function definition and PS2 lines must not count as output'
assert not complete_fence(fence('$ function f {', '> echo x', '> }')), \
    'function keyword definition and PS2 lines must not count as output'
assert complete_fence(fence('$ f() {', '> echo x', '> }', 'x')), \
    'function definition plus output must count'
assert complete_fence(fence('$ echo a{b', 'a{b')), \
    'a brace inside a word must not open a group'
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
assert not complete_table(['| case | result |', '| --- | --- |', '| case-a | TBD |']), \
    'a placeholder in a result cell is not evidence'
assert not complete_table(['| case | result | time |', '| --- | --- | --- |', '| case-a | ok | TBD |']), \
    'a placeholder in any cell is not evidence'
assert not complete_table(['| metric |', '| --- |', '| real |', '| TBD |']), \
    'a placeholder data row invalidates the table'
assert complete_table(['| 対象 | 入力 | 時間 |', '| --- | --- | --- |', '| 同 | 20,000 行 | 0.33 s |']), \
    'measured data row counts'

missing = []
for path in sorted(Path('docs/issues').glob('*.md')):
    text = path.read_text()
    match = re.search(r'^## 現象$\n(.*?)(?=^## |\Z)', text, re.M | re.S)
    assert match, f'{path.name}: no 現象 section'
    lines = match.group(1).split('\n')
    if not (complete_fence(lines) or complete_table(lines)):
        missing.append(path.name)
assert not missing, '現象 has no closed nonempty fence or complete table: ' + ', '.join(missing)
PY
}

check 'spec sections §1..§56 and 付録A.. exist once in reading order' doc_headings
check 'spec sections sit under part headings' doc_parts
check 'spec-coverage matches the plan tables 2.2/2.3/9.2/11.2/13.2/18.4/24.2' doc_coverage
check 'condition column prose does not require a newline' doc_condition_column
check 'spec and spec-coverage name the same version' doc_version
check 'each gate with a round header has a round file with recorded results' doc_round_files
check 'R38 spec sentences are pinned verbatim once each' doc_r38_sentences
check 'each issue cause cites a file:line anchor' doc_issue_causes
check 'each issue 現象 carries measured evidence in a fence or table' doc_issue_evidence

echo "PASSED: $passed/$total"
[ "$passed" -eq "$total" ]
