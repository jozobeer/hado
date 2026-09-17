# `Char` module と `Float.parse` `Int.to_float`（char-and-float-conversions）
優先度: 中。

## 現象

`String.get` で Char は取れるが、Char から先へ進む member が無い（2026-09-17）。

```
$ hado check main.hd    # def main = Char.to_int 'a'
main.hd:1:12: error[E204]: unbound name Char
$ hado check main.hd    # def main = Float.parse "1.5"
main.hd:1:18: error[E204]: module member is not exported
$ hado check main.hd    # def main = Int.to_float 3
main.hd:1:16: error[E204]: module member is not exported
```

Char のリテラル比較（`c == 'a'`）と `Float.to_string`、`Int.parse` は通る。

## 原因

§55 の関数集合に `Char` module が無く、`Float` は `to_string` だけである。`docs/spec.md:1904` の効果を持たない member の一覧に `Char` は無く、`stdlib/float.hd:2-4` も `to_string` だけである。

## 案

- `Char`: `to_int`、`of_int`、`to_string`。`of_int` は Unicode scalar value の `0x0000..=0xD7FF` と `0xE000..=0x10FFFF` だけを受理し、それ以外は `message` を `"InvalidScalar"` とする `{Error}` にする。
- `Float`: `parse`（受理文法は `Int.parse` と同じく仕様で決め、不正入力は `message` を `"InvalidFloat"` とする `{Error}`。受理した表記は IEEE 754 の round-to-nearest, ties-to-even で f64 に丸め、丸めた結果が無限大になる表記と、0 でないのに最小の非正規数未満で 0 に丸まる表記も `"InvalidFloat"` にする）、`of_int`。`Int.to_float` か `Float.of_int` かは prelude の命名（`Bytes.of_string` `Array.of_list`）に合わせて `of_int`。`of_int` は i64 を IEEE 754 の round-to-nearest, ties-to-even で f64 にする。
- `Float.to_string` はいま §54 の正準形（往復可能な最短の 10 進桁、指数を使わない位取り、小数点の両側に最低 1 桁）を満たしていない。`runtime/hado_runtime.c:514` の `%.17g` は `0.0` を `0`、10 の 300 乗を `1e+300` と出し、`main` の印字も `runtime/hado_runtime.c:462` で同じ `%.17g` を使う。`parse` の往復の期待に使う前に §54 へ直す。
- 入れない: 大文字化などの Unicode の性質。必要になってから。

## 完了条件

- 各 member の型と、`Char.of_int`・`Float.parse` の失敗の `message` を §55 の失敗の表に書き、受理ケースと、実行時の `{Error}` を `// expect-error:` で測る受理ケース（`docs/corpus.md`）を足す。両方の失敗は実行時の `{Error}` で §56 に静的コードが無いので、拒否コーパス（`// error:`）には置かない。
- `Char.of_int` は `0`、`0xD7FF`、`0xE000`、`0x10FFFF` を受理し、`-1`、`0xD800`、`0xDFFF`、`0x110000` は `message` が `"InvalidScalar"` の `{Error}` になる。受理した各値で `Char.to_int (Char.of_int n) == n` を照合し、`Char.to_string` の出力がそのスカラ値の UTF-8 バイト列になり abort しないことも測る。代表として BMP の `0x3042` と BMP 外の `0x1F600` でも同じ往復を測り、定数を返す `of_int` や `to_int` では通らないようにする。
- `Float.to_string` が §54 の正準形になり、`0.0` は `0.0`、`-0.0` は `-0.0`、`0.1` は `0.1`、10 の 300 乗は `1` の後に `0` が 300 個と `.0` になり、どの出力にも `e` も `E` も含まれない。往復の期待はこの形から測る。`main` の Float の印字（`runtime/hado_runtime.c:462`）も同じ正準形にし、`def main = 0.0` が `0.0`、10 の 300 乗が位取りで出ることを測る。両方の経路が同じ整形器を共有するか、両方を測る。
- `Float.of_int` は `9007199254740992`（2^53）、`9007199254740993`（2^53+1）、`-9007199254740993`、`9223372036854775807`、`(-9223372036854775807 - 1)`（i64 の最小値はリテラルに書けない。`docs/issues/exit-code-from-main.md:35` と同じ）で IEEE 754 の round-to-nearest, ties-to-even に一致する。期待値は判定器側で独立に計算する。
- `Float.parse` の受理文法と往復（`to_string` → `parse`）を仕様に書き、門番で測る。文法の外の入力（空文字列、`"1."`、`"."`、`"1e3"`、`" 1"`、`"0x1"`）は `message` が `"InvalidFloat"` の `{Error}` になる。受理した表記は IEEE 754 の round-to-nearest, ties-to-even で f64 に丸め、丸めた結果が無限大になる表記（f64 の最大有限値を超える）と、0 でないのに最小の非正規数未満で 0 に丸まる表記も `"InvalidFloat"` にして、アンダーフローを実装依存にしない。有限値の往復は `parse (to_string x) == x` になる。`-0.0` は IEEE の `==` では `+0.0` と区別できないので、`Float.to_string (Float.parse "-0.0") == "-0.0"` で測る（§54）。
- `"9007199254740993.0"`（2^53+1 のちょうど中間）が round-to-nearest, ties-to-even で `9007199254740992.0` になる。範囲境界は指数を使わない受理表記で測る。`2` の後に `0` が 308 個と `.0`（2×10^308、最大有限値を超える）と、`0.` の後に `0` が 323 個と `1`（10^-324、最小の非正規数未満）は `"InvalidFloat"` になる。`0.` の後に `0` が 323 個と `5`（5×10^-324、最小の非正規数）は有限値として受理する。期待値は判定器側で IEEE の規則から独立に計算する。

## 由来

`docs/wants.md` に無い。§19、§55。
