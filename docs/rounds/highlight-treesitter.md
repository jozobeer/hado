# Hado の tree-sitter 文法と Neovim ハイライト（highlight-treesitter）

開始 2026-09-20。

## 手順書

### 背景

`docs/issues/highlight-treesitter.md` の完了条件を満たす。エディタで `.hd` / `.hdi`
を開いたとき、キーワード・型・constructor・数値・文字列・コメントが色分けされる
状態にする。文法の正は `docs/spec.md` の「付録C 文法の概形」（`docs/spec.md` の
`## 付録C`）と第7節の優先順位（`docs/spec.md` の `## 7.`）で、既存の `crates/hado`
の parser（`crates/hado/src/parser.rs`）の節の扱いと揃える。

### 決定

- **決定 1: 文法は付録C を写すが、宣言の並びは実装に合わせる。** 付録C は
  `outer_item* section*` だが、実装は節の見出しの後も宣言を続ける。`source_file :=
  library? (type_definition | definition | section)*` とする。ずれは「仕様への疑義」
  に書く。
- **決定 2: 型の `upper_name` と constructor の `upper_name` は別の capture にする。**
  同じ capture にすると門番の探針が通らない。
- **決定 3: ハイライトの query は `queries/hado/highlights.scm` に置く。** Neovim の
  highlighter が runtimepath から `queries/<lang>/<name>.scm` を探すため。
- **決定 4: golden は node 種別だけの木にする。** 範囲と field 名を落とし、匿名
  トークンは出さない。演算子の高さは node 種別（`additive` / `multiplicative` /
  `unary` / `pipe` …）が表す。
- **決定 5: 道具は固定した版と checksum で取る。** `scripts/treesitter-setup.sh` が
  tree-sitter 0.27.0 と Neovim 0.12.5 を一時ディレクトリへ取り、絶対パスを返す。
  門番は PATH の同名ツールを使わない。`generate` は `--js-runtime native` で回し、
  Node を要求しない。
- **決定 6: extmark は設定の呼び出しを観測する。** highlighter の extmark は
  `ephemeral = true` で、redraw の外から `nvim_buf_get_extmarks` では読めない。
  門番は本物の TUI（pty）で `redraw!` を回し、`nvim_buf_set_extmark` に渡された
  highlight group を記録する。group の名前は query の capture から
  `@<capture>.hado` として門番側で計算する。

### ループの記録

- **探索**: 文法の正は付録C と第7節。`crates/hado/src/parser.rs` の `parse_items` が
  節の後も宣言を読む。Neovim の highlighter は `@<capture>.<lang>` を group にし、
  extmark は ephemeral。tree-sitter は lookahead を持たず、キーワードは `word` で
  締め出す必要がある。
- **モデル化**: node 名を付録C の非終端に合わせ、優先順位は §7 の段を
  `pipe` / `or` / `and` / `compare` / `additive` / `multiplicative` / `unary` /
  `application` / `postfix` / `primary` の入れ子で表す。`compare` は非結合なので
  右再帰を持たせない。
- **Red-Green-Refactor**:
  - 仮説: 付録C と §7 を写せば受理コーパス全件と stdlib が ERROR / MISSING なしで
    parse でき、優先順位の境界が node 列に出る。
  - 失敗するテスト: `tree-sitter generate` の conflict、`def f (n: Int) => n` の
    ERROR、`a < b < c` の非結合、`some_field` の直後の宣言を飲み込む字句。
  - 通した実装: `constructor_expr` の GLR 自己 conflict、`constructor_pattern` の
    高い優先順位、`word: ($) => $.name`、record の `;` 区切り、`prec.right` の節。
  - 実装から分かったこと: tree-sitter の文脈付き字句は、その状態で無効な
    キーワードを `name` に落とす。`word` を宣言しないと宣言の頭を飲み込む。
  - Refactor: 優先順位の golden 38 本を置き、`tree-sitter generate` の
    `Unresolved conflict` を順に消した。
- **学習**: 正規表現の lookahead は使えない。`-x` の XML は演算子トークンで
  壊れることがあるので、門番は既定の s 式を正規化して使う。`nvim --version` は
  state ディレクトリ（書けなければカレント）に `nvim.log` を書くので、取得 script は
  書き込み先を cache の下へ向ける。

### 成果物

- `editors/tree-sitter-hado/grammar.js`: 付録C と §7 の文法。
- `editors/tree-sitter-hado/queries/hado/highlights.scm`: capture ごとに 1 group。
- `editors/tree-sitter-hado/tree-sitter.json`、`README.md`、`.gitignore`（`src/` と
  `*.so` は `tree-sitter generate` の生成物）。
- `editors/tree-sitter-hado/test/golden/`: 宣言形 22 本と §7 の境界 16 本の `.hd` と
  `.tree`。
- `scripts/treesitter_dump.py`: parse の s 式を node 種別だけの木にする。
- `scripts/treesitter-setup.sh`: 版と checksum を固定した取得。
- `scripts/treesitter-hl.lua`、`scripts/treesitter-check.py`、
  `scripts/verify-treesitter.sh`（tier: core）。
- `AGENTS.md`: 環境に `curl`・`tar`・`gunzip`・`sha256sum` を足す。

### 判定

- `bash scripts/verify-treesitter.sh` が `PASSED: 5/5`。受理コーパス 133 件と
  `stdlib/` 10 件の 143 件、golden 38 本、extmark 34 個。
- 門番が赤になることを 3 つの変異で確かめる。
  1. constructor の capture を `@type` に寄せる → `@constructor` が消えて 4/5。
  2. `//` を `--` にする → 117/143 が ERROR / MISSING、コメントの extmark も消えて
     3/5。
  3. `&&` の左再帰を外す → 2/143 が ERROR / MISSING、`prec-not-and` の golden が
     ずれて 3/5。

### スコープ外

- 受理コーパス・`tests/reject`・`stdlib/`・`tests/corpus.sha256` は変えない。
- `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` にラウンドの節や行を足さない。
- Neovim 以外のエディタ（Emacs・VS Code）の設定は置かない。
- `highlights.scm` の `@operator` など、門番が測らない capture の網羅は追わない。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| 付録C | 宣言の形（`def` 4 形・`type`・`module`・`implement`・functor・`shared type`・`effect`・`handler`） | 構文 | 受理 | `test/golden/` の宣言形 22 本、`scripts/verify-treesitter.sh` |
| 付録C | 比較は非結合 | 構文 | ERROR | `test/golden/prec-cmp-nonassoc.tree` |
| §7 | 優先順位（`\|>` < `\|\|` < `&&` < 比較 < `+` < `*` < 単項 < 適用 < `.`） | 構文 | 受理 | `test/golden/prec-*.tree` の 16 本 |
| 付録C | 空白とコメント（`//`）は区切りで意味を持たない | 字句 | 受理 | 受理コーパス 143 件、ハイライトのコメント探針 |
| 付録C | 文字列と文字のエスケープ（`\n` `\t` `\\` `\"` `\'`） | 字句 | 受理 | `verify-treesitter.sh` のエスケープ探針 |
| §12 | `if` の腕は真偽・条件列・パターンの 3 形 | 構文 | 受理 | `test/golden/if-*.tree` の 3 本 |
| §47 | `implement X as Y` は本文を持てる。`(Name: Contract)` は functor 定義、`= Functor(Arg)` は本文の無い適用 | 構文 | 受理 | `test/golden/implement-body.tree`・`functor-def.tree`・`functor-apply.tree` |
| §47 | `shared type T = ...` は `implement` 本文の等式 | 構文 | 受理 | `test/golden/shared-type.tree` と `type-realization.tree` の別 |
| 付録C | 型の名前と constructor の名前は別の綴り | 字句 | 受理 | ハイライトの `@type` / `@constructor` 探針 |

## 実測

```
$ date +%F
2026-09-20

# 受理コーパスと stdlib（門番はこの実数をディスクから数える）
$ find tests/accept -type f \( -name '*.hd' -o -name '*.hdi' \) | wc -l
133
$ ls stdlib/*.hd | wc -l
10
$ ls editors/tree-sitter-hado/test/golden/*.hd | wc -l
38

# 道具の取得（版と checksum は script の定数が正）
$ bash scripts/treesitter-setup.sh --cache /tmp/hado-setup-fresh
treesitter-setup: fetching tree-sitter-linux-x64.gz
treesitter-setup: fetching nvim-linux-x86_64.tar.gz
treesitter-setup: unpacking tree-sitter 0.27.0
treesitter-setup: unpacking Neovim 0.12.5
tree-sitter /tmp/hado-setup-fresh/tree-sitter-0.27.0
nvim /tmp/hado-setup-fresh/nvim-linux-x86_64/bin/nvim

# 門番（cache 無しで取得から回す）
$ bash scripts/verify-treesitter.sh
treesitter-setup: fetching tree-sitter-linux-x64.gz
treesitter-setup: fetching nvim-linux-x86_64.tar.gz
treesitter-setup: unpacking tree-sitter 0.27.0
treesitter-setup: unpacking Neovim 0.12.5
OK: parser builds
OK: accept corpus and stdlib parse without ERROR or MISSING
OK: escape literals parse without ERROR or MISSING
OK: declaration forms and section 7 precedence goldens match
OK: Neovim marks each promised token with its capture group
PASSED: 5/5
corpus files: 143
goldens: 38
extmarks: 34
real	0m4.251s

# 変異 1: constructor の capture を @type に寄せる
$ bash scripts/verify-treesitter.sh
NG: Neovim marks each promised token with its capture group (query lacks captures: ['@constructor'])
PASSED: 4/5

# 変異 2: コメントの綴りを // から -- にする
$ bash scripts/verify-treesitter.sh
NG: accept corpus and stdlib parse without ERROR or MISSING (117/143 files have ERROR or MISSING: [...])
NG: Neovim marks each promised token with its capture group (no @comment mark at 0:0-16 ('// comment 12345'); ...)
PASSED: 3/5

# 変異 3: && の左再帰を外す
$ bash scripts/verify-treesitter.sh
NG: accept corpus and stdlib parse without ERROR or MISSING (2/143 files have ERROR or MISSING: [...])
NG: declaration forms and section 7 precedence goldens match (golden prec-not-and differs: ...)
PASSED: 3/5

$ git status --short
（門番の走行後も追跡外の生成物は残らない）

$ git log --oneline -4
82617af docs: tree-sitter-hado の使い方と取得ツールを書く
d5d4165 feat: tree-sitter の門番と道具の取得を足す
bc6585a refactor: tree-sitter の dumper が parse の集計行を読まないようにする
efd1c26 test: tree-sitter の golden と node 列の正規化を足す
```
