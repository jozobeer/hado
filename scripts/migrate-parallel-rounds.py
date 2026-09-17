#!/usr/bin/env python3
"""Move gate-tier membership onto each verify-*.sh, then split plan sections."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / 'scripts'
PLAN = ROOT / 'docs' / 'plan.md'
ROUNDS = ROOT / 'docs' / 'rounds'
HEADING_RE = re.compile(r'^## (\d+)\. (.*)$', re.M)
MOVED_TITLE_RE = re.compile(r'R[0-9]')

# 移行前（bfa9e4c より前）の scripts/lib/gate-tiers.sh が持っていた層の一覧。
QUICK = frozenset({'gate-cache', 'm1-spec'})
CORE = frozenset({
    'm0', 'm1-corpus', 'm2', 'm3', 'm4', 'm5', 'm6', 'm7', 'r38',
})

TIER_ASSERT_GATES = (
    'r46', 'r47', 'r48', 'r49', 'r50', 'r51', 'r52', 'r53', 'r54', 'r55',
    'r56', 'r57', 'r58', 'r59', 'r60', 'r61', 'r62', 'r63', 'r64', 'r65',
    'r68', 'r69', 'r70', 'r71', 'r72', 'r73', 'r74', 'r75', 'r76', 'r77',
    'r81',
)

TIERS_ASSIGN = re.compile(
    r"^([ \t]*)tiers = Path\('scripts/lib/gate-tiers\.sh'\)\.read_text\(\)\n"
    r"\1full = tiers\.split\('full\)', 1\)\[1\]\.split\(';;', 1\)\[0\]\n",
    re.M,
)
FULL_SPLIT_ASSERT = re.compile(r"assert ('r\d+') in full\.split\(\),")
BASH_TIER_GREP = re.compile(
    r'^grep -q "(r\d+)" scripts/lib/gate-tiers\.sh \|\| missing="\$missing tier"$',
    re.M,
)
NEW_FULL = (
    "full = subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full'], "
    "capture_output=True, text=True, check=True).stdout.split()"
)
NEW_RUN = "subprocess.run(['bash', 'scripts/run-gates.sh', '--print-tier', 'full']"


def gate_name(path):
    name = path.name
    return name[len('verify-'):-len('.sh')]


def tier_for(name):
    if name in QUICK:
        return 'quick'
    if name in CORE:
        return 'core'
    return 'full'


def cmd_tier_headers():
    for path in sorted(SCRIPTS.glob('verify-*.sh')):
        text = path.read_text()
        if re.search(r'^# tier: ', text, re.M):
            continue
        lines = text.splitlines(keepends=True)
        if not lines or not lines[0].startswith('#!'):
            print(f'no shebang: {path.name}', file=sys.stderr)
            sys.exit(1)
        lines.insert(1, f'# tier: {tier_for(gate_name(path))}\n')
        path.write_text(''.join(lines))


def block_imports_subprocess(lines, start, end):
    block = ''.join(lines[start:end])
    return bool(re.search(r'^\s*import\s+(\S+\s*,\s*)*subprocess\b', block, re.M) or
                re.search(r'^\s*from subprocess import\b', block, re.M))


def python_block_start(lines, index):
    for j in range(index - 1, -1, -1):
        if re.search(r"<<['\"]", lines[j]):
            return j + 1
    return 0


def ensure_subprocess_import(text):
    lines = text.splitlines(keepends=True)
    i = 0
    while i < len(lines):
        if NEW_RUN not in lines[i]:
            i += 1
            continue
        start = python_block_start(lines, i)
        if not block_imports_subprocess(lines, start, i):
            indent = re.match(r'[ \t]*', lines[i]).group(0)
            lines.insert(i, f'{indent}import subprocess\n')
            i += 1
        i += 1
    return ''.join(lines)


def migrate_asserts(text):
    def replace_assign(match):
        return f'{match.group(1)}{NEW_FULL}\n'

    text, n_assign = TIERS_ASSIGN.subn(replace_assign, text)
    text, n_assert = FULL_SPLIT_ASSERT.subn(r'assert \1 in full,', text)
    text, n_bash = BASH_TIER_GREP.subn(
        r'bash scripts/run-gates.sh --print-tier full | grep -qx \1 || missing="$missing tier"',
        text,
    )
    if n_assign:
        text = ensure_subprocess_import(text)
    return text, n_assign + n_bash, n_assert


def cmd_tier_asserts():
    missed = []
    for name in TIER_ASSERT_GATES:
        path = SCRIPTS / f'verify-{name}.sh'
        old = path.read_text()
        new, n_repl, n_assert = migrate_asserts(old)
        if n_repl == 0:
            missed.append(path.name)
            continue
        if n_assert == 0 and 'in full.split()' in new:
            missed.append(path.name)
            continue
        path.write_text(new)
    if missed:
        print('unreplaced:', ' '.join(missed), file=sys.stderr)
        sys.exit(1)


def slug_for(number, title):
    if number == 15:
        return 'r21-addendum'
    ranged = re.search(r'R(\d+)〜R(\d+)', title)
    if ranged:
        return f'r{ranged.group(1)}-r{ranged.group(2)}'
    single = re.search(r'R(\d+)', title)
    if single:
        return f'r{single.group(1)}'
    print(f'cannot name section {number}: {title}', file=sys.stderr)
    sys.exit(1)


def one_trailing_newline(text):
    return text.rstrip('\n') + '\n'


def parse_plan_sections(text):
    matches = list(HEADING_RE.finditer(text))
    sections = []
    for i, match in enumerate(matches):
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        number = int(match.group(1))
        title = match.group(2)
        sections.append({
            'number': number,
            'title': title,
            'heading': match.group(0),
            'span': (match.start(), end),
            'body': text[match.end():end],
        })
    return sections


def section_body(section):
    body = section['body']
    if body.startswith('\n'):
        body = body[1:]
    return one_trailing_newline(body.lstrip('\n'))


def round_file_text(section):
    name = section['slug']
    body = section_body(section)
    return (
        f"# {section['title']}（{name}）\n"
        f"旧 `docs/plan.md` {section['number']} 節。実測は `docs/ledger.md`。\n"
        f"\n"
        f"## 手順書\n"
        f"\n"
        f"{body}"
    )


def cmd_split_plan():
    text = PLAN.read_text()
    sections = parse_plan_sections(text)
    moved = [s for s in sections if MOVED_TITLE_RE.search(s['title'])]
    if len(moved) != 60:
        print(f'moved sections: {len(moved)} (expected 60)', file=sys.stderr)
        for s in moved:
            print(f"  {s['number']}. {s['title']}", file=sys.stderr)
        sys.exit(1)
    names = {}
    for section in moved:
        slug = slug_for(section['number'], section['title'])
        if slug in names:
            print(f'duplicate slug {slug}: {names[slug]} and {section["number"]}', file=sys.stderr)
            sys.exit(1)
        names[slug] = section['number']
        section['slug'] = slug
    files = []
    for section in moved:
        written = round_file_text(section)
        marker = '## 手順書\n\n'
        idx = written.index(marker) + len(marker)
        files.append((section, written, written[idx:]))
    removed = ''.join(section_body(section) for section in moved)
    exported = ''.join(body for _, _, body in files)
    if removed != exported:
        print('split-plan conservation failed', file=sys.stderr)
        sys.exit(1)
    ROUNDS.mkdir(parents=True, exist_ok=True)
    for section, written, _ in files:
        path = ROUNDS / f"{section['slug']}.md"
        path.write_text(written)
    keep = []
    last = 0
    moved_spans = {s['span'] for s in moved}
    for section in sections:
        start, end = section['span']
        if last < start:
            keep.append(text[last:start])
        if (start, end) not in moved_spans:
            keep.append(text[start:end])
        last = end
    if last < len(text):
        keep.append(text[last:])
    PLAN.write_text(one_trailing_newline(''.join(keep)))


def load_moved_map():
    mapping = {}
    for path in sorted(ROUNDS.glob('*.md')):
        if path.name == 'parallel-rounds.md':
            continue
        text = path.read_text()
        match = re.match(
            r'^# .+（([^）]+)）\n旧 `docs/plan\.md` (\d+) 節。',
            text,
        )
        if not match:
            print(f'unreadable round file: {path.name}', file=sys.stderr)
            sys.exit(1)
        mapping[int(match.group(2))] = match.group(1)
    if len(mapping) != 60:
        print(f'round files with old section numbers: {len(mapping)} (expected 60)', file=sys.stderr)
        sys.exit(1)
    return mapping


def drop_unused_plan_assign(text):
    return re.sub(
        r"plan = Path\('docs/plan\.md'\)\.read_text\(\)\n"
        r"(?=assert Path\('docs/rounds/)",
        '',
        text,
    )


def replace_plan_lists(text, mapping):
    def repl(match):
        nums = [int(part) for part in match.group(1).split('・')]
        if not all(n in mapping for n in nums):
            return match.group(0)
        return '・'.join(f'`docs/rounds/{mapping[n]}.md`' for n in nums)

    return re.sub(r'計画 (\d+(?:・\d+)+) 節', repl, text)


def rewrite_refs(text, mapping):
    saved = []

    def stash(match):
        saved.append(match.group(0))
        return f'\0PROV{len(saved) - 1}\0'

    text = re.sub(r'^旧 `docs/plan\.md` \d+ 節。.*$', stash, text, flags=re.M)
    for number, name in mapping.items():
        text = text.replace(
            f"assert '## {number}. ' in Path('docs/plan.md').read_text(), "
            f"'the plan has no section {number}'",
            f"assert Path('docs/rounds/{name}.md').is_file(), "
            f"'docs/rounds/{name}.md is missing'",
        )
        text = text.replace(
            f"assert '## {number}. ' in plan, 'plan has no section {number}'",
            f"assert Path('docs/rounds/{name}.md').is_file(), "
            f"'docs/rounds/{name}.md is missing'",
        )
        text = text.replace(
            f'grep -q "^## {number}\\. " docs/plan.md || missing="$missing plan"',
            f'[ -f docs/rounds/{name}.md ] || missing="$missing plan"',
        )
    text = drop_unused_plan_assign(text)
    text = replace_plan_lists(text, mapping)
    for number in sorted(mapping, reverse=True):
        name = mapping[number]
        repl = f'`docs/rounds/{name}.md`'
        text = text.replace(f'`docs/plan.md` {number} 節', repl)
        text = text.replace(f'docs/plan.md {number} 節', repl)
        text = text.replace(f'plan.md` {number} 節', repl)
        text = text.replace(f'計画 {number} 節', repl)
    for i, line in enumerate(saved):
        text = text.replace(f'\0PROV{i}\0', line)
    return text


def ref_targets():
    paths = []
    paths.extend(sorted(SCRIPTS.glob('*.sh')))
    paths.extend(sorted((SCRIPTS / 'lib').glob('*.sh')))
    paths.extend([
        ROOT / 'docs' / 'plan.md',
        ROOT / 'docs' / 'wants.md',
        ROOT / 'docs' / 'corpus.md',
        ROOT / 'docs' / 'spec-trace.md',
        ROOT / 'README.md',
    ])
    paths.extend(sorted(ROUNDS.glob('*.md')))
    return paths


def leftover_ref_hits(mapping):
    hits = []
    for path in ref_targets():
        lines = path.read_text().splitlines()
        rel = path.relative_to(ROOT)
        for i, line in enumerate(lines, 1):
            if line.startswith('旧 '):
                continue
            for number in mapping:
                if re.search(rf'(計画|plan\.md)[^\n]{{0,12}}\b{number} 節', line):
                    hits.append(f'{rel}:{i}:{line}')
                    break
    return hits


def cmd_refs():
    mapping = load_moved_map()
    for path in ref_targets():
        old = path.read_text()
        new = rewrite_refs(old, mapping)
        if old != new:
            path.write_text(new)


def cmd_refs_check():
    mapping = load_moved_map()
    hits = leftover_ref_hits(mapping)
    if hits:
        print('\n'.join(hits), file=sys.stderr)
        sys.exit(1)


def usage():
    print(
        'usage: migrate-parallel-rounds.py '
        'tier-headers|tier-asserts|split-plan|refs [--check]',
        file=sys.stderr,
    )
    sys.exit(2)


def main():
    args = sys.argv[1:]
    if args == ['tier-headers']:
        cmd_tier_headers()
    elif args == ['tier-asserts']:
        cmd_tier_asserts()
    elif args == ['split-plan']:
        cmd_split_plan()
    elif args == ['refs']:
        cmd_refs()
    elif args == ['refs', '--check']:
        cmd_refs_check()
    else:
        usage()


if __name__ == '__main__':
    main()
