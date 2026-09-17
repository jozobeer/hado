#!/usr/bin/env python3
"""Move gate-tier membership onto each verify-*.sh; rerunning does not duplicate it."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / 'scripts'

# 現行 scripts/lib/gate-tiers.sh の hado_tier_minimal_gates の一覧。
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


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in ('tier-headers', 'tier-asserts'):
        print('usage: migrate-parallel-rounds.py tier-headers|tier-asserts', file=sys.stderr)
        sys.exit(2)
    if sys.argv[1] == 'tier-headers':
        cmd_tier_headers()
    else:
        cmd_tier_asserts()


if __name__ == '__main__':
    main()
