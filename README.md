# hado

ML 系の型システムと ADT を核に、より小さく、より平坦に、より一意に書ける言語。Rust 製コンパイラが検査と実行ファイル生成を行う。

仕様 `docs/spec.md` §1 の 2 原則:

> **Control flow should be flat.**

> **If code can be removed without changing meaning, it should not compile.**

失敗しうる処理の記述例（[仕様 付録B](docs/spec.md#付録b-典型コード)）。`let` は値を残し、`do` は値を捨てて次へ進む。失敗は型の `{Error}` に現れ、呼び出し位置には印を書かない。

```ocaml
def create_user input =>
  let parsed = parse input ->
  do validate parsed ->
  do authorize parsed ->
  save parsed
```

`let x = v -> x` のような冗長な記述は E501 で拒否し、診断に正準形 `v` を添える（[仕様 §2](docs/spec.md#2-正準形の原則)）。

## 必要なもの

- Rust（Cranelift 0.135 が 1.95 以上を要求。1.98 で実測済み）
- `cc`（リンクに使う）
- `libgc-dev`（Boehm GC）

## はじめる

依存クレートは初回のみ `cargo fetch --locked` で取得し、以後は `--offline` で回す。

```
$ cargo fetch --locked
$ cargo build --offline
$ cargo run --offline -q -- run tests/accept/create_user/main.hd
ok
```

自分のプログラムを書く手順は [docs/getting-started.md](docs/getting-started.md)。

## コマンド

`cargo run --offline -q -- <命令>` で呼び出し、バイナリの出力先には依存しない（[仕様 §58](docs/spec.md#58-実行と出力先)）。

```
check [--print-type[=<名前>]] [--show-all] [--fix] <file>   検査。診断は stderr へ出し、あれば終了コード 1、無ければ 0
dump-mono <file>                                            単相化後のプログラムを印字
build [-o <path>] [--fix] <file.hd>                         実行ファイルを入力の隣に置く
run <file.hd> [-- <arg>...]                                 一時ディレクトリで作って実行し、隣に何も残さない
fmt [--check] <file.hd>                                     正準形への一括整形
test <file.hd>                                              入力根の直下の *_test.hd を走らせる
lsp                                                         LSP サーバ（stdio）。エディタから使う
```

- `check`: 既定では先頭段階の診断を 1 件だけ出力する。
  - `--show-all`: 同じ段階の診断を全件アンカー順に出力（字句・構文・型の致命的エラーは 1 件のみ）。
  - `--print-type[=<名前>]`: `main`（または指定した定義）の型を印字。網羅性・冗長性・契約の検査で止まっていても型を返すが、字句・構文・名前解決・型の致命的エラーでは返らない。
  - `--fix`: 診断が示した正準形への書き換えを 1 つずつ適用する（[仕様 §57](docs/spec.md#57-正準形の修正)）。
  - `--format json`: `check` `build` `run` `test` の診断を 1 行 1 つの JSON で出力する（[仕様 §58.4](docs/spec.md#584-診断の機械可読な出力)）。
- `build`: 実行ファイルを入力ファイルの隣に生成する。`--fix` を付けるとビルド前に `fmt` と同じ整形を適用する。
- `run`: 一時ディレクトリでビルドして実行し、隣にファイルを残さない。`--` 以降の引数をプログラムに渡し、作業ディレクトリは変更せず、プログラムの終了コードを返す。
- `fmt`: 入力根配下の全ソースに局所的な書き換えを適用する（検査は行わない）。`--check` は書き換えを行わず、未整形のファイルがあれば終了コード 1 を返す（[仕様 §58.1](docs/spec.md#581-正準形への一括整形)）。
- `test`: 入力根直下の `*_test.hd` を実行し、終了コード 0 を合格とする（[仕様 §58.2](docs/spec.md#582-テストの実行)）。

拒否の例:

```
$ cargo run --offline -q -- check tests/reject/E508/main.hd
main.hd:7:3: error[E508]: absence bound at 7:5 is propagated unchanged; use let p? = or do; canonical form: let user? = find_user id -> Some(user + 1)
```

## エディタ（Neovim）

`hado lsp` は LSP を stdio で提供する。Neovim 0.12 の組み込みクライアントから利用できる。

```lua
vim.filetype.add({ extension = { hd = "hado", hdi = "hado" } })
vim.lsp.config("hado", {
  cmd = { "<checkout>/scripts/hado-lsp.sh" },
  filetypes = { "hado" },
})
vim.lsp.enable("hado")
```

`<checkout>` はこのリポジトリの絶対パスに置き換える（`hado` を `PATH` に置く必要はない）。

- 対応機能: 診断、hover（カーソル下の最上位 `def` の型表示）、正準形へ置換する code action（`check --fix` と同じ書き換え）。
- `.hdi`（契約ファイル）: 同様に `hado` として開き、診断と hover を提供する（書き換え対象の式がないため code action は空）。
- 未対応: 補完、rename、go to definition。

構文ハイライトは [editors/tree-sitter-hado/README.md](editors/tree-sitter-hado/README.md) の tree-sitter 文法で設定する。

## 仕様

言語仕様は [docs/spec.md](docs/spec.md) を参照。§4 入れないもの、§56 エラーコード表と検査の段階順、付録B 典型コード。

## 開発に参加する

コンパイラ・ランタイム・仕様を変更する開発者向け案内は [CONTRIBUTING.md](CONTRIBUTING.md)、開発ルールは [AGENTS.md](AGENTS.md) を参照。
