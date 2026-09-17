# 構文と分岐の拒否に正準の綴りを出す（canonical-in-syntax-errors）
優先度: 高。

## 現象

20 行の CLI を書く途中で、2 つの診断が「どう書けばよいか」を示さず、綴りを仕様で探した（2026-09-17）。

```
$ hado check main.hd    # def rec bump (entries: Entry List) (word: String): Entry List =>
main.hd:3:36: error[E100]: expected =>, found (
$ hado check main.hd    # 腕の中に if c => a | b
main.hd:6:24: error[E101]: branches cannot be nested inside another branch
```

正しい綴りはそれぞれ `def f(a: T, b: U): R =>`（§8）と、腕の条件列 `=> if | c => a | b`（§26.1）である。

## 原因

E100 は parser の期待トークンをそのまま出す。E101 は禁止だけを述べ、§26.1 が定める正準形（腕の条件列）を示さない。`docs/wants.md` 1 は冗長の診断（E5xx）に正準形を足す項目で、E100 と E101 は対象に入っていない。E100 と E101 は `crates/hado/src/project.rs:313-339` の解析段階で `LoadError::Diagnostic` になり、`crates/hado/src/main.rs:119-125` が `report_json` を通さず既定の 1 行を出す。`hado check --format json` でも JSON は出ず、`canonical` の有無を機械可読に確かめられない（2026-09-17、`tests/reject/E101/main.hd` で実測。E505 は JSON 1 行になる対照）。

## 案

- E101 のうち、Boolean branch を pattern の腕に直接置いた形だけを検出し、メッセージに `use arm conditions: | p => if | c => a | b` の形を足す。E101 は他にも `fn(| ... )` の腕形式（`tests/reject/E101/main.hd`）、腕の中の pattern branch、条件列の腕の中の分岐（§30）を含み、この書き換えはそれらには当たらないので、`canonical` は `null` のままにするか部分形ごとの提案にする。`canonical` は書き換えではなく形の提示なので、`hado fmt` の対象にはしない。
- E100 のうち、型注釈付き引数の列を括弧ごとに分けた形（`(a: T) (b: U)`）だけを検出し、`def f(a: T, b: U): R =>` を示す。付録C の `parameters` は引数全体を 1 つの括弧に入れる形を許すので、`def f (a: T): R =>` と `def f x : T =>` は正当であり、対象にしない。他の E100 は触らない。
- E100 と E101 は解析段階で止まるので、`--format json` でも他の診断と同じ 1 行の JSON になるよう、`crates/hado/src/main.rs` の読み込み失敗の経路を診断の印字経路へ通す。

## 完了条件

- 上の 2 つの入力で、既定の診断に正準の綴りが含まれる。`hado check --format json` も `code` を E100 / E101 とする JSON を 1 行出し、`canonical` が正準の綴りを含む（`null` でない）。解析段階の診断が既定の 1 行のままにならないよう、`main.rs` の読み込み失敗の経路も `--format json` に従わせる。拒否コーパスの E100 / E101 のケースは 1 ファイル 1 欠陥のまま。
- `hado fmt` はこの提示だけの正準形を適用しない（書き換えの `canonical` と区別する）。
- E101 の他の部分形（`tests/reject/E101/main.hd` の `fn(| ... )`、腕の中の pattern branch、条件列の腕の中の分岐）では `canonical` を `null` のままにし、当たらない書き換えを出さない。
- `--format json` の `canonical` の意味（書き換え vs 形の提示）を §58.4 で決める。

## 由来

`docs/wants.md` 1 の拡張。§8、§26.1、§56、§58.4。
