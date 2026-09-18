# Neovim 用の tree-sitter 文法（highlight-treesitter）
優先度: 中。

## 現象

`.hd` を開いても色が付かない。対象エディタは Neovim（0.12.5、nvim-treesitter、`vim.treesitter.language.add` で動的に足す設定）。tree-sitter CLI と node は入っている（2026-09-17）。

```
$ nvim --version | head -1
NVIM v0.12.5
$ cat > hl.lua <<'LUA'
vim.cmd("filetype on")
vim.filetype.add({ extension = { hd = "hado", hdi = "hado" } })
vim.treesitter.language.register("lua", "hado")
vim.cmd("edit main.hd")
local buf = vim.api.nvim_get_current_buf()
print("filetype=" .. vim.bo.filetype)
print("parser=" .. tostring(pcall(vim.treesitter.get_parser, 0)))
print("active_before_start=" .. tostring(vim.treesitter.highlighter.active[buf] ~= nil))
vim.treesitter.start()
print("active_after_start=" .. tostring(vim.treesitter.highlighter.active[buf] ~= nil))
vim.cmd("qa!")
LUA
$ nvim --headless -u NONE -i NONE -l hl.lua
filetype=hado
parser=true
active_before_start=false
active_after_start=true
```

filetype と parser は取れるが、`FileType` で `vim.treesitter.start()` を呼ぶまで highlighter は動かない。`queries/hado/highlights.scm` も無いので capture も付かない。

## 原因

文法は `docs/spec.md:2325` の付録C に散文で置かれ、機械可読ではない。`docs/wants.md` 51 は付録C からの生成を想定するが、生成の前に付録C を文法ファイルにする工程が要る。

## 案

- 第一段は手書きの `grammar.js`。字句（付録C の `int` `float` `char` `string` `comment` `name` `upper_name`）、キーワード、見出し（外の `library`、`module` `implement` `effect` `handler`、functor の `implement ... (params)` とその適用）、`shared` 付きの型、`def` の 4 形、`if` の腕、`let`/`do`/`with` の継続、`|>` を覆う。式の優先順位は §7 の表を写す。
- 置き場は `editors/tree-sitter-hado/`（`grammar.js`、`queries/hado/highlights.scm`）。Neovim への足し方は `README` に `vim.filetype.add({ extension = { hd = "hado", hdi = "hado" } })`、`vim.treesitter.language.add("hado", { path = ... })`、`vim.opt.runtimepath:prepend(".../editors/tree-sitter-hado")`、`FileType` で `vim.treesitter.start()` を呼ぶ autocmd の形で書く。契約の `.hdi` も同じ filetype にする。`language.add` も `runtimepath:prepend` も、文法リポジトリ慣例の `queries/highlights.scm` を Neovim の `queries/hado/highlights.scm` には変えない。`-u NONE` では `query.lua` が runtimepath から `queries/%s/%s.scm` を探すので、query は prepend したディレクトリ直下の `queries/hado/highlights.scm` に置く。`filetype.add` と `language.add` だけではハイライトは始まらない。
- 門番が使う tree-sitter CLI、必要なら Node、Neovim 0.12.5 は版と checksum を固定する。標準環境から一時ディレクトリへ取得する setup script を置き、その前提となる取得・展開ツールは `AGENTS.md` の環境に足す。門番は PATH 上の偶然の導入ではなく、setup script が返した絶対パスを使う。
- ずれの検出: 受理コーパス全件と `stdlib/*.hd` を `tree-sitter parse` に通し、`ERROR` ノードも `MISSING` ノードも無いことを門番にする。拒否コーパスの E100 のケースが `ERROR` を持つことは求めない（tree-sitter の文法は寛容でよい）。
- 付録C からの生成（wants 51）は、手書きで形が固まってから。

## 完了条件

- 受理コーパスと stdlib の全ファイルが `tree-sitter parse` で `ERROR` も `MISSING` も無し。門番は件数を直書きせずディスクの実数を数える。
- 代表ファイル（`def` の 4 形、`if` の腕、`let`/`do`/`with` の継続、`|>` の優先順位、外の `library`、`module`、本文付きの `implement`、functor の定義、functor の適用、`shared` 付きの型、`effect`、`handler`）の parse 木の node 列を golden として門番に置き、約束した構造がその通りになることを判定する。宣言形は付録C の形ごとに 1 本の golden を置く。`library` は最初の見出しより前の outer_item。`module` は `module Name` のあと抽象型または型付き `def`。本文付きの `implement X as Y` と、`(Name: Contract)` を持つ functor 定義と、`= Functor(Arg)` で本文の無い適用は 3 本を別の node 列にする。`shared type T = ...` は `implement` 本文の等式で、`shared` の無い `type T = ...` では満たさない。`effect` は `once`/`abort`/`again` の操作、`handler` は `as` と腕。適用を本文付き `implement` や functor 定義と同じ node にする文法、`shared` を通常の type 実体化と同じ node にする文法、`implement` と `handler` だけの golden、キーワードと文字列の extmark だけ、catch-all の 1 節では通らない。
- `nvim --headless` で `.hd` と `.hdi` をそれぞれ通常どおり開き、`vim.bo.filetype == "hado"`、言語を明示しない `vim.treesitter.get_parser(0)` が返ること、`vim.treesitter.query.get("hado", "highlights")` が runtimepath の `queries/hado/highlights.scm` から引けること、そのバッファで highlighter が active で、キーワードと文字列の位置に highlight の extmark が実際に付くことを順に判定する。`highlighter.active` だけや capture の照会だけでは、ハイライトが出ていなくても通る。
- `AGENTS.md` の環境だけを入れた clean checkout で setup script を実行した後、tree-sitter CLI と Neovim の版・checksum が固定値と一致し、上の 2 本が緑。既に PATH に同名のツールがあっても使わない。

## 由来

`docs/wants.md` 14、51。付録C、§7。
