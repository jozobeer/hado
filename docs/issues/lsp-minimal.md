# 最小の LSP（lsp-minimal）
優先度: 中。

## 現象

配管は既にある。`hado check --format json` は 1 行 1 診断で `canonical` を持ち（§58.4）、`--print-type=<名前>` は最上位の `def` の型を出す（§58.3）。

LSP にすると見える設計上の穴が 3 つある（2026-09-17）。

1. 診断は常に 1 件。E305・E513・E505 を 1 ファイルに置いても E305 だけが出る。
2. 位置は開始点だけ。JSON に `line` `col` はあるが終端が無く、LSP の range が作れない。
3. hover の型は、ファイルに誤りが無いときの最上位の `def` だけ。局所束縛を聞くと診断が返る。E505 で止まっている間は型が見えない。

## 原因

各段階は診断を `Vec` に集めてから `min_by_key(diagnostic_order)` で 1 件を選ぶ（`crates/hado/src/lib.rs:213-233`、`check.rs:27`）。E3xx は最初の致命的な型エラーで `Err` を返し（`infer.rs:258`）、E1xx は parser に回復が無い。型付け結果 `Checked` は E4xx・E5xx・E6xx で止まっているときも手元にあり、`lib.rs:231` が診断があれば捨てる。

対象エディタは Neovim（0.12.5、`vim.lsp.config` / `vim.lsp.enable`）。offline の cargo cache に `lsp-server` `lsp-types` `tower-lsp` は無く、`serde_json` はある。

## 決定（`docs/plan.md` 35 節 L1〜L4）

- `--show-all` を opt-in で足す。同じ段階の全件をアンカー順。E1xx と E3xx の致命は 1 件のまま。既定は変えない。
- 診断で止まっていても型を返す。ライブラリの入口を「診断の列と `Option<Checked>`」を返す形にする。E1xx・E2xx・E3xx の致命で止まったときは型が無い。
- `hado lsp` サブコマンドで JSON-RPC を `serde_json` で書く。

## 案

- `--format json` に終端の位置を足す（§58.4 のフィールドを 1 つ増やす）。アンカーの構文要素の終端を parser の `Span` から取る。
- 機能は診断・hover・「正準形へ置換」の code action の 3 つ。code action は `check --fix` と同じ関数を呼ぶ。補完は入れない。
- hover の第一段はカーソル下の最上位 `def` の型に限る。局所束縛と式の型は、infer が span から型への対応を保持しているか未確認なので、調べてから決める。
- Neovim 側は `vim.lsp.config("hado", { cmd = { "hado", "lsp" }, filetypes = { "hado" } })` と `vim.lsp.enable("hado")` の数行。

## 完了条件

- `nvim --headless` で `.hd` を開き、診断・hover・code action の結果を取り出す門番（`verify-skill` のハーネス）が緑。
- E505 で止まったファイルで hover が型を返す。

## 由来

`docs/wants.md` 2、7（1 件報告）、14、15。§56、§58.3、§58.4。highlight-treesitter の後。
