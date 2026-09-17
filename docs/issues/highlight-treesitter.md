# Neovim 用の tree-sitter 文法（highlight-treesitter）
優先度: 中。

## 現象

`.hd` を開いても色が付かない。対象エディタは Neovim（0.12.5、nvim-treesitter、`vim.treesitter.language.add` で動的に足す設定）。tree-sitter CLI と node は入っている（2026-09-17）。

## 原因

文法は `docs/spec.md` 付録C に散文で置かれ、機械可読ではない。`docs/wants.md` 51 は付録C からの生成を想定するが、生成の前に付録C を文法ファイルにする工程が要る。

## 案

- 第一段は手書きの `grammar.js`。字句（付録C の `int` `float` `char` `string` `comment` `name` `upper_name`）、キーワード、見出し（`module` `implement` `effect` `handler`）、`def` の 4 形、`if` の腕、`let`/`do`/`with` の継続、`|>` を覆う。式の優先順位は §7 の表を写す。
- 置き場は `editors/tree-sitter-hado/`（`grammar.js`、`queries/highlights.scm`）。Neovim への足し方は `README` に `vim.treesitter.language.add("hado", { path = ... })` の形で書く。
- ずれの検出: 受理コーパス全件と `stdlib/*.hd` を `tree-sitter parse` に通し、`ERROR` ノードが無いことを門番にする。拒否コーパスの E100 のケースが `ERROR` を持つことは求めない（tree-sitter の文法は寛容でよい）。
- 付録C からの生成（wants 51）は、手書きで形が固まってから。

## 完了条件

- 受理コーパスと stdlib の全ファイルが `tree-sitter parse` で `ERROR` 無し。門番は件数を直書きせずディスクの実数を数える。
- `nvim --headless` で `.hd` を開いて `vim.treesitter.get_parser` が返り、`highlights.scm` の capture がキーワードと文字列に付く証跡。

## 由来

`docs/wants.md` 14、51。付録C、§7。
