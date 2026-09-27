# Neovim の extmark を数える検査が、まれに marks=[] で赤になる（flaky-treesitter-extmark-probe）
優先度: 中。

## 現象

`scripts/verify-treesitter.sh` の検査 5（`Neovim marks each promised token with its capture group`）が、まれに **extmark を 1 つも観測しないまま**赤になる。main の CI で起きた（2026-09-27、`e0a06a7` の全走）。

```
$ gh run view --job=108575720470 --log | grep -a TALLY
TALLY: 3806/3807 passed; red: verify-treesitter.log

$ tail -12 target/gate-logs-f1/verify-treesitter.log
treesitter-setup: fetching tree-sitter-linux-x64.gz
treesitter-setup: unpacking tree-sitter 0.27.0
treesitter-setup: unpacking Neovim 0.12.5
OK: parser builds
OK: accept corpus and stdlib parse without ERROR or MISSING
OK: escape literals parse without ERROR or MISSING
OK: declaration forms and section 7 precedence goldens match
NG: Neovim marks each promised token with its capture group (no @comment mark at 0:0-16 ('// comment 12345'); marks=[])
PASSED: 4/5
corpus files: 163
```

同じ木は手元で 4 回続けて `PASSED: 5/5`（`extmarks: 39`）。直後の main（`a19f6cd`・`e9e2352`）の CI でもこの検査は緑で、赤は 1 回だけ。

## 原因

`marks=[]` は「highlighter が 1 つも extmark を置かなかった」ことを意味する。探針（`scripts/treesitter-hl.lua`）は `nvim_buf_set_extmark` を包んで記録し、`redraw!` の**後**に記録を書き出す。ところが記録は `redraw!` の完了を待たずに読まれる。`redraw!` は非同期に描画を進めるので、描画が高lighter の extmark を置く前に `marks` を読むと 0 件になる。`capture` の行は出ているのに `mark` の行が 1 つも出ないのがこの形である（query の読み込みと capture の登録は `redraw!` の前なので残る）。負荷の高い計算機で窓が開くと踏む。

`scripts/treesitter-check.py` の `parse_probe_output` は `mark` を数えるだけなので、0 件でも「探針が走った」ことは検出できる。いまは `assert hit` が先に落ちて 0 件であることが分からない。

## 案

- 推奨: 探針が `redraw!` の**完了を待って**から記録する（`vim.api.nvim__redraw({ flush = true })` を `pcall` で呼び、無ければ `vim.wait` で描画が進むのを待つ）。待っても 0 件なら本物の欠陥として赤にする。
- 次点: 記録が 0 件のときに `probe 0 marks` の行を出し、門番が「探針が走ったが 1 つも置かれなかった」と「探針が走らなかった」を区別して赤の理由を残す（切り分けが速くなる）。
- 退ける: 検査を 1 回の試行で判定することをやめ、数回試して 1 つでも marks があれば緑にする。本物の highlighter の退行を隠す。

## 完了条件

- 探針が `redraw!` の完了を待ってから記録する（`marks=[]` が「探針が走ったが置かれなかった」を意味するようになる）。
- 赤の理由に marks の件数が出る。
- `bash scripts/verify-treesitter.sh` が、CPU を飽和させた状態でも緑になる（手元で再現して実測を残す）。
- `bash scripts/run-gates.sh --tier full` が red なし。

## 由来

`docs/wants.md` になし。F1（`postfix-binding`）着地後の main の CI（run 36303573802）の赤の切り分けから。`docs/rounds/postfix-binding.md` の `## 実測` と、`docs/issues/flaky-string-split-timing-gate.md`（同じ「負荷で偽赤」の族）。
