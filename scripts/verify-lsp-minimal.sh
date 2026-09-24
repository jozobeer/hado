#!/usr/bin/env bash
# tier: core
# round: lsp-minimal
#
# lsp-minimal: 固定した Neovim 0.12.5 の組み込み LSP client から README の設定の
# `cmd`（`<checkout>/scripts/hado-lsp.sh`）で `hado lsp` を起こし、診断・hover・
# code action・lifecycle・位置の符号化・契約の URI・version の追い越しを測る
# （docs/rounds/lsp-minimal.md の完了条件と判定）。
#
# Neovim は scripts/treesitter-setup.sh が固定した版と checksum で取ってきた
# 絶対パスを使う。PATH 上の同名ツールは見ない。`target/` を PATH に足さず、
# `hado` という名前の実行ファイルにも依存しない（`cmd` は checkout の
# scripts/hado-lsp.sh を指し、判定器はそこから `cargo run --offline -q -- lsp` を
# 起こす）。起動器が exec した先はプロセス名 `hado` で見分け、それまでの cargo の
# build lock 待ちをサーバの応答の予算から外す（docs/rounds/lsp-probe-lock.md）。
set -eu
cd "$(dirname "$0")/.."

# 道具の展開先と判定器の作業場所は 1 つの一時ディレクトリにまとめ、終了時に消す。
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

tools=$(bash scripts/treesitter-setup.sh --cache "$work/tools")
nvim=$(printf '%s\n' "$tools" | sed -n 's/^nvim //p')
if [ ! -x "$nvim" ]; then
    echo 'verify-lsp-minimal: setup returned no nvim binary' >&2
    exit 1
fi

python3 scripts/lsp-check.py "$nvim" "$PWD" "$work/judge"
