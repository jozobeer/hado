#!/usr/bin/env python3
"""cranelift-opt-level の 3 本目の入力を作る。

手順は `string-split-linear` の完了条件（`scripts/verify-string-split-linear.sh`
検査 e）の `lines(count, width)` と同じである。1 行 64 文字の `a` を改行で
区切り、末尾に改行を置かない。`COUNT` は検査 e の小さい側と同じ 1000000 行で、
その中央値は 200 ms の床を大きく超える（ラウンドファイルの実測）。

    python3 tests/perf/opt-level/split_input.py <出力先>
"""
import sys
from pathlib import Path

COUNT = 1000000
WIDTH = 64


def lines(count, width):
    return (('a' * width + '\n') * (count - 1) + 'a' * width).encode()


def main():
    if len(sys.argv) != 2:
        sys.exit('usage: split_input.py <output>')
    Path(sys.argv[1]).write_bytes(lines(COUNT, WIDTH))


if __name__ == '__main__':
    main()
