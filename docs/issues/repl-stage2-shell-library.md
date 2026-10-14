# シェル用ライブラリを読み込む REPL（repl-stage2-shell-library）
優先度: 低。

## 現象

段階 1 の `repl` は式と `e as x ->` だけを受け、prelude の効果に外部コマンドの起動が無い。REPL の延長でシェルとして使うには、外部コマンドの起動と、ファイルの一覧や検索のための操作が要る。

```
$ grep -A6 "^effect Io" docs/spec.md
effect Io
once read_file(path: String): Bytes {Error}
once write_file(path: String, contents: Bytes): Void {Error}
once log(contents: String): Void {Error}
once read_dir(path: String): String List {Error}
once read_stdin: Bytes {Error}
$ grep -c "Process" docs/spec.md
0
```

## 原因

prelude の効果は `docs/spec.md` の `Io` の 5 つだけで、外部コマンドの起動が無い。段階 1 の `hado repl` は `crates/hado/src/repl.rs` の `run_with` が式と `e as x ->` の 1 歩だけを受け、トップレベル宣言は断る。シェル用のライブラリを置く場所（§52 の `<dir>`）と、その効果を prelude に足す設計が未決定である。

## 案

- 推奨: シェル向けの操作（外部コマンドの起動、ファイルの一覧や検索など）をシェル用のライブラリにまとめ、`hado repl <シェル用ライブラリ>` で起動してシェルとして使う。外部コマンドの起動は prelude に新しい効果（例: `Process`）を足すことになり、失敗は `Error` として raise する（§55 の既定実装と同じ）。
- 退ける: 既存の `Io` に外部コマンドの起動を足す。`Io` は §10 の `main` に残ってよい効果で、既存のプログラムの効果の集合が変わる。

## 完了条件

- `Process`（またはシェル用の効果）の仕様が `docs/spec.md` にあり、その効果の操作を呼ぶシェル用のライブラリが `hado repl <dir>` で読み込める。
- 外部コマンドの起動の失敗が `Error` として raise され、セッションが続く（段階 1 の raise の扱いと同じ）。
- `cargo test --offline` が 0 failed、`bash scripts/lint-docs.sh` が PASSED。

## 由来

`docs/issues/repl.md` の段階 2（`repl` の着地で切り出した）。§10、§52、§55。関連する issue は `repl`（段階 1）。
