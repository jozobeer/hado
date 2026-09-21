# hado

ML 系の型システムと ADT を核に、より小さく、より平坦に、より一意に書ける言語。Rust で書いたコンパイラが検査と実行ファイル生成を行う。

仕様 `docs/spec.md` §1 の 2 原則:

> **Control flow should be flat.**

> **If code can be removed without changing meaning, it should not compile.**

失敗しうる処理はこう書く（[仕様 付録B](docs/spec.md#付録b-典型コード)）。`let` は値を残し、`do` は値を捨てて次へ進む。失敗は型の `{Error}` に現れ、呼び出し位置には印を書かない。

```ocaml
def create_user input =>
  let parsed = parse input ->
  do validate parsed ->
  do authorize parsed ->
  save parsed
```

`let x = v -> x` は E501 で拒否し、診断は正準形 `v` を添える（[仕様 §2](docs/spec.md#2-正準形の原則)）。

## 必要なもの

- Rust（Cranelift 0.135 が 1.95 以上を要求。1.98 で実測済み）
- `cc`（リンクに使う）
- `libgc-dev`（Boehm GC）

## はじめる

依存は初回だけ `cargo fetch --locked` で取り、以後は `--offline` で回す。

```
$ cargo fetch --locked
$ cargo build --offline
$ cargo run --offline -q -- run tests/accept/create_user/main.hd
ok
```

自分のプログラムを書く手順は [docs/getting-started.md](docs/getting-started.md)。

## コマンド

`cargo run --offline -q -- <命令>` で呼び、バイナリの出力先に依存しない（[仕様 §58](docs/spec.md#58-実行と出力先)）。

```
check [--print-type[=<名前>]] [--show-all] [--fix] <file>   検査。診断は stderr、終了コード 1
dump-mono <file>                                            単相化後のプログラムを印字
build [-o <path>] [--fix] <file.hd>                         実行ファイルを入力の隣に置く
run <file.hd> [-- <arg>...]                                 一時ディレクトリで作って実行し、隣に何も残さない
fmt [--check] <file.hd>                                     正準形への一括整形
test <file.hd>                                              入力根の直下の *_test.hd を走らせる
lsp                                                         LSP サーバ（stdio）。エディタから使う
```

`check` は既定で先頭の段階の診断を 1 件だけ出す。`--show-all` は同じ段階の診断を全件、アンカー順に出す（字句・構文の致命と型の致命は 1 件のまま）。`--print-type` は `main` の型を、`--print-type=<名前>` はその定義の型を印字する。型は網羅性・冗長・契約の検査で止まっていても返る（字句・構文・名前解決・型の致命では返らない）。`--fix` は報告した診断が持つ正準形への書き換えを 1 つずつ当てる（[仕様 §57](docs/spec.md#57-正準形の修正)）。`--format json` は `check` `build` `run` `test` で診断を 1 行 1 つの JSON にする（[仕様 §58.4](docs/spec.md#584-診断の機械可読な出力)）。

`build --fix` は作る前に `fmt` と同じ整形を当てる。`run` は `--` の後の引数をプログラムに渡し、作業ディレクトリを変えず、終了コードはプログラムのものを返す。

`fmt` は入力根のソース全部に局所的な書き換えを当て、検査はしない。`--check` は書き換えずに、正準形でないファイルがあれば終了コード 1 を返す（[仕様 §58.1](docs/spec.md#581-正準形への一括整形)）。`test` はプログラムの終了コード 0 を合格とする（[仕様 §58.2](docs/spec.md#582-テストの実行)）。

拒否の例:

```
$ cargo run --offline -q -- check tests/reject/E508/main.hd
main.hd:7:3: error[E508]: absence bound at 7:5 is propagated unchanged; use let p? = or do; canonical form: let user? = find_user id -> Some(user + 1)
```

## エディタ（Neovim）

`hado lsp` は LSP を stdio で話す。Neovim 0.12 の組み込み client から使う。

```lua
vim.filetype.add({ extension = { hd = "hado", hdi = "hado" } })
vim.lsp.config("hado", {
  cmd = { "<checkout>/scripts/hado-lsp.sh" },
  filetypes = { "hado" },
})
vim.lsp.enable("hado")
```

`<checkout>` はこのリポジトリの絶対パスに置き換える。`hado` を `PATH` に置く必要は無い。

診断・hover（カーソル下の最上位 `def` の型）・「正準形へ置換」の code action の 3 つが効く。code action は `check --fix` と同じ書き換えを返す。`.hdi` の契約も `hado` として開き、診断と hover を返す（契約には書き換え対象の式が無いので code action は空になる）。補完・rename・go to definition は入っていない。

構文ハイライトは [editors/tree-sitter-hado/README.md](editors/tree-sitter-hado/README.md) の tree-sitter 文法で付ける。

## 仕様

言語仕様は [docs/spec.md](docs/spec.md)。§4 入れないもの、§56 エラーコード表と検査の段階順、付録B 典型コード。

## 開発に参加する

コンパイラ・ランタイム・仕様を変える人の案内は [CONTRIBUTING.md](CONTRIBUTING.md)。守る規則は [AGENTS.md](AGENTS.md)。
