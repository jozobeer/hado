# Neovim 用の tree-sitter 文法（highlight-treesitter）
優先度: 中。

## 現象

`.hd` を開いても色が付かない。対象エディタは Neovim（0.12.5、nvim-treesitter、`vim.treesitter.language.add` で動的に足す設定）。tree-sitter CLI と node は入っている（2026-09-17）。

## 原因

文法は `docs/spec.md` 付録C に散文で置かれ、機械可読ではない。`docs/wants.md` 51 は付録C からの生成を想定するが、生成の前に付録C を文法ファイルにする工程が要る。

## 案

- 第一段は手書きの `grammar.js`。字句（付録C の `int` `float` `char` `string` `comment` `name` `upper_name`）、キーワード、見出し（`module` `implement` `effect` `handler`）、`def` の 4 形、`if` の腕、`let`/`do`/`with` の継続、`|>` を覆う。式の優先順位は §7 の表を写す。
- 置き場は `editors/tree-sitter-hado/`（`grammar.js`、`queries/hado/highlights.scm`）。Neovim への足し方は `README` に `vim.filetype.add({ extension = { hd = "hado" } })`、`vim.treesitter.language.add("hado", { path = ... })`、`vim.opt.runtimepath:prepend(".../editors/tree-sitter-hado")`、`FileType` で `vim.treesitter.start()` を呼ぶ autocmd の形で書く。`language.add` も `runtimepath:prepend` も、文法リポジトリ慣例の `queries/highlights.scm` を Neovim の `queries/hado/highlights.scm` には変えない。`-u NONE` では `query.lua` が runtimepath から `queries/%s/%s.scm` を探すので、query は prepend したディレクトリ直下の `queries/hado/highlights.scm` に置く。`filetype.add` と `language.add` だけではハイライトは始まらない。
- 門番が使う tree-sitter CLI、必要なら Node、Neovim 0.12.5 は版と checksum を固定する。標準環境から一時ディレクトリへ取得する setup script を置き、その前提となる取得・展開ツールは `AGENTS.md` の環境に足す。門番は PATH 上の偶然の導入ではなく、setup script が返した絶対パスを使う。
- ずれの検出: 受理コーパス全件と `stdlib/*.hd` を `tree-sitter parse` に通し、`ERROR` ノードが無いことを門番にする。拒否コーパスの E100 のケースが `ERROR` を持つことは求めない（tree-sitter の文法は寛容でよい）。
- 付録C からの生成（wants 51）は、手書きで形が固まってから。

## 完了条件

- 受理コーパスと stdlib の全ファイルが `tree-sitter parse` で `ERROR` 無し。門番は件数を直書きせずディスクの実数を数える。
- `nvim --headless` で `.hd` を通常どおり開き、`vim.bo.filetype == "hado"`、言語を明示しない `vim.treesitter.get_parser(0)` が返ること、`vim.treesitter.query.get("hado", "highlights")` が runtimepath の `queries/hado/highlights.scm` から引けること、そのバッファで highlighter が active で、キーワードと文字列の位置に highlight の extmark が実際に付くことを順に判定する。`highlighter.active` だけや capture の照会だけでは、ハイライトが出ていなくても通る。
- `AGENTS.md` の環境だけを入れた clean checkout で setup script を実行した後、tree-sitter CLI と Neovim の版・checksum が固定値と一致し、上の 2 本が緑。既に PATH に同名のツールがあっても使わない。

## 由来

`docs/wants.md` 14、51。付録C、§7。
