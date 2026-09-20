#!/usr/bin/env python3
"""tree-sitter parse の s 式を node 種別だけの木に正規化する。

標準入力の `tree-sitter parse` 出力（名前付き node と `ERROR` / `MISSING`。
範囲と field 名を含む）から、範囲と field 名を落とした node 列を作る。
匿名トークン（`+` や `def` など）は出ない。演算子の高さは node 種別
（`additive` / `multiplicative` / `unary` / `pipe` …）が表す。

使い方: tree-sitter parse ... <file> | python3 scripts/treesitter_dump.py
"""
import re
import sys

FIELD = re.compile(r'[A-Za-z_][A-Za-z0-9_]*:\s*')
TYPE = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')


def parse(text):
    """s 式を node 種別の入れ子リストにする。"""
    root = []
    stack = [root]
    i = 0
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n and text[i] != '"':
                if text[i] == '\\':
                    i += 1
                i += 1
            i += 1
        elif c == '(':
            j = i + 1
            match = FIELD.match(text, j)
            if match:
                j = match.end()
            match = TYPE.match(text, j)
            node = [match.group(0) if match else '?']
            stack[-1].append(node)
            stack.append(node)
            i = j
        elif c == ')':
            if len(stack) > 1:
                stack.pop()
            i += 1
        else:
            i += 1
    return root


def render(node, indent, lines):
    pad = '  ' * indent
    if len(node) == 1:
        lines.append('%s(%s)' % (pad, node[0]))
        return
    lines.append('%s(%s' % (pad, node[0]))
    for child in node[1:]:
        render(child, indent + 1, lines)
    lines.append('%s)' % pad)


def main():
    text = sys.stdin.read()
    # 末尾の集計行（`<path>\tParse: ...`）は木ではない。ERROR を含むことがあり、
    # そのまま読むと余計な node が増える。
    text = '\n'.join(line for line in text.splitlines() if '\tParse:' not in line)
    lines = []
    for node in parse(text):
        render(node, 0, lines)
    sys.stdout.write('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
