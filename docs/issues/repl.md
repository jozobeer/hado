# ライブラリを読み込める REPL（repl）
優先度: 低。

## 現象

hado には対話的に式を評価する入口が無い。コマンドは `check` `dump-mono` `build` `run` `fmt` `test` `lsp` だけで、プログラムは `def main` を持つファイルとして書いてビルドするしかない。

```
$ cargo run --offline -q -- repl stdlib; echo "exit $?"
usage: hado check [--print-type[=<name>]] [--fix] <file> | dump-mono <file> | build [-o <path>] [--fix] <file.hd> | run <file.hd> [-- <arg>...] | fmt [--check] <file.hd> | test <file.hd> | lsp
exit 2
```

REPL の延長でシェルとして使うには外部コマンドの起動が要るが、prelude の効果に無い。

```
$ grep -A6 "^effect Io" docs/spec.md
effect Io
once read_file(path: String): Bytes {Error}
once write_file(path: String, contents: Bytes): Void {Error}
once log(contents: String): Void {Error}
once read_dir(path: String): String List {Error}
once read_stdin: Bytes {Error}
```

## 原因

`crates/hado/src/main.rs` の `parse_args` が受け付けるコマンドは上の 7 つで、`run_program` は入力ファイルを AOT でビルドしてから実行する。評価の状態を入力をまたいで持つ仕組みが無い。改行は構文の意味を持たない（`crates/hado/src/lexer.rs` の `lex_full` が読み飛ばす）ので、行の終わりで入力が完結したかを構文だけでは決められない。

## 案

- 推奨: 段階 1 で `hado repl [<dir>]` を入れる。`<dir>` を渡せば、そのライブラリ（§52）を読み込んだ状態でセッションを始める。
  - セッションを `main` の本体の続きとして扱う。入力 1 つが継続の 1 歩で、`postfix-binding` の `e as x ->` で終えた入力の束縛は、以後の入力で使える（`->` の後ろ全部が継続という §11 の読みを、セッションの残りに当てる）。
  - 式の値は §54 の `main` の値と同じ規則で表示する。残ってよい効果と型の単相性は §10 の `main` と同じにする。
  - 入力は、括弧と閉じる印が釣り合い、行末が開いた字句（`=>`・`|>`・`|`・`,`）でなければ送る。行末の `->` は束縛して次の入力へ進む印とする。§14 のように行頭に `|>` を置く書き方はセッションでは使えない。`postfix-binding` の有力案の `if(...)`・`fn(...)` は、開き括弧が続きを示すのでこの判定と噛み合う。
  - 入力ごとに AOT でビルドして実行すると、前の入力の効果を毎回やり直す。状態を持ったまま評価を続ける方式（`docs/plan.md` の JIT）か、束縛した値を次の入力へ渡す方式かは未確認。
- 段階 2（延長）: シェル向けの操作（外部コマンドの起動、ファイルの一覧や検索など）をシェル用のライブラリにまとめ、`hado repl <シェル用ライブラリ>` で起動してシェルとして使う。外部コマンドの起動は prelude に新しい効果（例: `Process`）を足すことになり、失敗は `Error` として raise する（§55 の既定実装と同じ）。段階 1 が着地してから別の issue に切り出す。
- 退ける: 入力の終わりを `/def` のような定義の閉じで示す。対話で打つのは `def` だけでなく、`type` と式もある。複数の項目は区切りの印なしで 1 行に並べられる（`def inc x => x + 1 def double x => x * 2 def main = double (inc 3)` は `8` を出す）。

## 完了条件

- `hado repl <dir>` が `<dir>` のライブラリの `def` を呼べるセッションを始め、標準入力から与えた入力の列に対する出力を期待と照合するテストがある。
- `e as x ->` で終えた入力の束縛が、次の入力で使える。前の入力の効果が再実行されないことをテストが測る。
- `cargo test --offline` が 0 failed、`bash scripts/lint-docs.sh` が PASSED。

## 由来

`docs/wants.md` の 20（REPL）。2026-09-26 の `postfix-binding` の相談で、シェル用のライブラリを読み込んだ状態で起動する延長の案として挙がった。§10、§11、§14、§52、§54、§55。関連する issue は `postfix-binding`。
