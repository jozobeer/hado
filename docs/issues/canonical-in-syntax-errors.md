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

E100 は parser の期待トークンをそのまま出す。E101 は禁止だけを述べ、§26.1 が定める正準形（腕の条件列）を示さない。`docs/wants.md` 1 は冗長の診断（E5xx）に正準形を足す項目で、E100 と E101 は対象に入っていない。`--format json` の `canonical` は E100 / E101 で `null` である。

## 案

- E101: メッセージに `use arm conditions: | p => if | c => a | b` の形を足す。`canonical` は書き換えではなく形の提示なので、`hado fmt` の対象にはしない。
- E100 のうち、型注釈付き引数を括弧なしで書いた形（`(a: T) (b: U)`）と、`def f x : T =>` の形だけを検出し、`def f(a: T, b: U): R =>` を示す。他の E100 は触らない。

## 完了条件

- 上の 2 つの入力で、診断に正準の綴りが含まれる。拒否コーパスの E100 / E101 のケースは 1 ファイル 1 欠陥のまま。
- `--format json` の `canonical` の意味（書き換え vs 形の提示）を §58.4 で決める。

## 由来

`docs/wants.md` 1 の拡張。§8、§26.1、§56、§58.4。
