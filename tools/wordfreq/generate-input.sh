#!/usr/bin/env bash
# 単語頻度の入力を再生成する手順（docs/rounds/map-by-builtin-compare.md）。
#
# 空白区切り 200000 語、相異なる語 500、各語が 1 回以上。語は `w000`..`w499`。
# 種を固定した擬似乱数で回数を配り、その多重集合を並べ替える。同じ手順は
# 常に同じバイト列を返す（tests/ の corpus-lock と同じく、真偽が要る実測は
# 走らせた側に生成させる）。末尾に改行は置かない（語の区切りは空白だけ）。
set -euo pipefail

if [ "$#" -ne 1 ]; then
    printf 'usage: %s <出力先>\n' "$0" >&2
    exit 2
fi

python3 - "$1" <<'PY'
import random
import sys
from pathlib import Path

WORDS = 500
TOTAL = 200000
SEED = 20260927

rng = random.Random(SEED)
counts = [1] * WORDS
for _ in range(TOTAL - WORDS):
    counts[rng.randrange(WORDS)] += 1
sequence = []
for index, count in enumerate(counts):
    sequence.extend([f'w{index:03d}'] * count)
rng.shuffle(sequence)
assert len(sequence) == TOTAL
assert len(set(sequence)) == WORDS
Path(sys.argv[1]).write_text(' '.join(sequence))
PY
