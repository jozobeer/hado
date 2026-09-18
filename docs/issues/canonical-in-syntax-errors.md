# 構文と分岐の拒否に正準の綴りを出す（canonical-in-syntax-errors）
優先度: 高。

## 現象

20 行の CLI を書く途中で、2 つの診断が「どう書けばよいか」を示さず、綴りを仕様で探した（2026-09-17）。

```
$ cat main.hd    # 引数を括弧ごとに分けた形（正しい綴りは def f(a: T, b: U): R =>、§8）
def rec bump (entries: Entry List) (word: String): Entry List =>
  entries
$ cargo run --offline -q -- check main.hd
main.hd:1:36: error[E100]: expected =>, found (
$ cat main.hd    # 腕の中に if c => a | b（正しい綴りは腕の条件列 | p => if | c => a | b、§26.1）
def label n =>
  if n
    | 0 => "zero"
    | _ => if n > 0 => "positive" | "negative"
$ cargo run --offline -q -- check main.hd
main.hd:4:12: error[E101]: branches cannot be nested inside another branch
```

元は 20 行の CLI を書く途中で当たった 2 件で、上の 2 入力は同じ 2 診断への最小化である（2026-09-18 に `cargo run --offline -q -- check` で取り直した）。

正しい綴りはそれぞれ `def f(a: T, b: U): R =>`（§8）と、腕の条件列 `=> if | c => a | b`（§26.1）である。

## 原因

E100 は parser の期待トークンをそのまま出す。E101 は禁止だけを述べ、§26.1 が定める正準形（腕の条件列）を示さない。`docs/wants.md` 1 は冗長の診断（E5xx）に正準形を足す項目で、E100 と E101 は対象に入っていない。E100 と E101 は `crates/hado/src/project.rs` の `parse_file` の解析段階で `LoadError::Diagnostic` になり、`crates/hado/src/main.rs` の `main` が `report_json` を通さず既定の 1 行を出す。`hado check --format json` でも JSON は出ず、`canonical` の有無を機械可読に確かめられない（2026-09-17、`tests/reject/E101/main.hd` で実測。E505 は JSON 1 行になる対照）。

## 案

- E101 のうち、Boolean branch を pattern の腕に直接置いた形だけを検出し、メッセージに `use arm conditions: | p => if | c => a | b` の形を足す。E101 は他にも `fn(| ... )` の腕形式（`tests/reject/E101/main.hd`）、腕の中の pattern branch、条件列の腕の中の分岐（§30）を含み、この提示はそれらには当たらないので、`suggestion` は `null` のままにするか部分形ごとの提示にする。`suggestion` は形の提示で書き換えではないので、`hado fmt` の対象にはしない。
- E100 のうち、型注釈付き引数の列を括弧ごとに分けた形（`(a: T) (b: U)`）だけを検出し、`def f(a: T, b: U): R =>` を示す。付録C の `parameters` は引数全体を 1 つの括弧に入れる形を許すので、`def f (a: T): R =>` と `def f x : T =>` は正当であり、対象にしない。他の E100 は触らない。
- §58.4 の 1 行のフィールドに `suggestion` を足す。`suggestion` は形の提示（書き換えではない）で、無ければ `null`。`canonical` は局所的な書き換え（§57）の正準形だけを表し、`suggestion` を `hado fmt` や `--fix` は適用しない。同じ診断で `canonical` と `suggestion` を同時に非 null にしない。
- E100 と E101 は解析段階で止まるので、`--format json` でも他の診断と同じ 1 行の JSON になるよう、`crates/hado/src/main.rs` の読み込み失敗の経路を診断の印字経路へ通す。

## 完了条件

- 上の 2 つの入力で、既定の診断に正準の綴りが含まれる。`hado check --format json` も `code` を E100 / E101 とする JSON を 1 行出し、`suggestion` が正準の綴りを含み（`null` でない）、`canonical` は `null` になる。解析段階の診断が既定の 1 行のままにならないよう、`main.rs` の読み込み失敗の経路も `--format json` に従わせる。`--format` は `check` だけでなく `build`・`run`・`test` も受ける（`crates/hado/src/main.rs:58`）。同じ 1 行（`code` が E100 / E101、`suggestion` が正準の綴り、`canonical` は `null`）を、`check` と `build` は上の 2 つの入力を `--format json` に渡して測る（`main.rs:119-125`）。`run` は同じ 2 つの入力を `hado run --format json` に渡し、`execute` の `Project::load`（`main.rs:266-270`）から出す。`test` は `test_paths`（`project.rs:259`）が入口ファイルを解析しないので、同じ本文を根の `*_test.hd` に置いて `hado test --format json` に渡し、`execute` がその試験ファイルを `Project::load` したときの同じ 1 行を測る。入口の 2 つの入力を `hado test` に渡すだけでは `test: no *_test.hd` になり、266-270 には届かない。`test: ... failed (build)` は診断ではない。119-125 だけを JSON にして `run` / `test` が既定の 1 行のままでは通らない。拒否コーパスの E100 / E101 のケースは 1 ファイル 1 欠陥のまま。
- 上の 2 つの入力に `hado fmt` と `hado check --fix` をかけても本文は変わらない（`suggestion` は適用しない）。
- 書き換え側の対照として、既存の局所書き換え（`tests/reject/E520/main.hd`）では `--format json` の `canonical` が §24 の Boolean branch の正準形を含み（`null` でない）、`suggestion` は `null` になり、`hado fmt` と `--fix` がその書き換えを適用する。`suggestion` だけを見て適用する実装や、両方を空にして適用を止めた実装では通らない。
- E101 の他の部分形（`tests/reject/E101/main.hd` の `fn(| ... )`、腕の中の pattern branch、条件列の腕の中の分岐）では `suggestion` を `null` のままにし、当たらない書き換えを出さない。
- §58.4 の 1 行のフィールドに `suggestion` を足す（無ければ `null`）。`end_line`・`end_col` を足す lsp-minimal とは独立に、フィールドを名前で数える（両方が着地すると 10 個になる）。`canonical` は局所的な書き換え、`suggestion` は形の提示。同じ診断で両方を非 null にしない。`--format json` でこの 2 つの表現を測る。

## 由来

`docs/wants.md` 1 の拡張。§8、§26.1、§56、§58.4。
