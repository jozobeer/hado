#!/usr/bin/env python3
"""Migrate v0.1.2 corpus spellings; rerunning leaves v0.1.3 sources unchanged."""
import re
from pathlib import Path


def split_arguments(source):
    depth = 0
    start = 0
    parts = []
    for index, char in enumerate(source):
        depth += (char in '(<') - (char in ')>')
        if char == ',' and depth == 0:
            parts.append(source[start:index].strip())
            start = index + 1
    return parts + [source[start:].strip()]


def type_application(match):
    name, arguments = match.groups()
    parts = split_arguments(arguments)
    if name == 'Fn':
        inputs = parts[0]
        if inputs.startswith('(') and inputs.endswith(')'):
            inputs = inputs[1:-1]
        return f'Fn({inputs} => {parts[1]})'
    prefix = parts[0] if len(parts) == 1 else '(' + ', '.join(parts) + ')'
    return f'{prefix} {name}'


def declaration(match):
    keyword, name, arguments = match.groups()
    parts = split_arguments(arguments)
    prefix = parts[0] if len(parts) == 1 else '(' + ', '.join(parts) + ')'
    return f'{keyword} {prefix} {name}'


def migrate(source):
    source = re.sub(r'\b(type|def) (\w+)<([^<>]+)>', declaration, source)
    while re.search(r'\b([A-Z]\w*(?:\.\w+)?)<([^<>]+)>', source):
        source = re.sub(r'\b([A-Z]\w*(?:\.\w+)?)<([^<>]+)>', type_application, source)
    source = re.sub(r'\(fn ([a-z][\w ]*?) (?==>|\|)',
                    lambda m: 'fn(' + ', '.join(m[1].split()) + ' ', source)
    source = source.replace('fn x => loop offset x ->', 'fn(x => loop offset x) ->')
    return source


def main():
    for root in ('tests/accept', 'tests/reject', 'stdlib'):
        for path in sorted(Path(root).rglob('*')):
            if path.suffix not in ('.hd', '.hdi') or path == Path('tests/reject/E100/main.hd'):
                continue
            old = path.read_text()
            new = migrate(old)
            if old != new:
                path.write_text(new)
    path = Path('README.md')
    path.write_text(path.read_text().replace('Result<String,String>', '(String, String) Result'))


if __name__ == '__main__':
    main()
