#!/usr/bin/env bash
# highlight-treesitter: Hado の tree-sitter 文法が受理コーパスと stdlib を
# ERROR / MISSING なしで parse し、宣言形と第7節の優先順位の golden に一致し、
# Neovim 0.12.5 が約束した字句に capture の highlight group を付けることを測る。
#
# tree-sitter CLI と Neovim は scripts/treesitter-setup.sh が固定した版と
# checksum で取ってきた絶対パスを使う。PATH 上の同名ツールは見ない。
#
# tier: core
# round: highlight-treesitter
set -eu
cd "$(dirname "$0")/.."

tools=$(bash scripts/treesitter-setup.sh)
ts=$(printf '%s\n' "$tools" | sed -n 's/^tree-sitter //p')
nvim=$(printf '%s\n' "$tools" | sed -n 's/^nvim //p')
if [ ! -x "$ts" ]; then
    echo 'verify-treesitter: setup returned no tree-sitter binary' >&2
    exit 1
fi
if [ ! -x "$nvim" ]; then
    echo 'verify-treesitter: setup returned no nvim binary' >&2
    exit 1
fi

exec python3 scripts/treesitter-check.py "$ts" "$nvim"
