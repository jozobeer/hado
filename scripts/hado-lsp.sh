#!/bin/bash
# `hado lsp` をリポジトリ根から起こす薄い包み。エディタの LSP client の `cmd` に
# このスクリプトの絶対パスを置く（README の「エディタ（Neovim）」）。
#
# `hado` を PATH に置かず、バイナリの出力先にも依存しない。自分の位置から根を
# 求めるので、checkout がどこにあってもよい。stdin/stdout はそのまま LSP の
# フレームが通る（このスクリプトは何も印字しない）。
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
exec cargo run --offline -q -- lsp
