# `Char` module と `Float.parse` `Int.to_float`（char-and-float-conversions）
優先度: 中。

## 現象

`String.get` で Char は取れるが、Char から先へ進む member が無い（2026-09-17）。

```
$ cargo run --offline -q -- check main.hd    # def main = Char.to_int 'a'
main.hd:1:12: error[E204]: unbound name Char
$ cargo run --offline -q -- check main.hd    # def main = Float.parse "1.5"
main.hd:1:18: error[E204]: module member is not exported
$ cargo run --offline -q -- check main.hd    # def main = Int.to_float 3
main.hd:1:16: error[E204]: module member is not exported
```

Char のリテラル比較（`c == 'a'`）と `Float.to_string`、`Int.parse` は通る。

## 原因

§55 の関数集合に `Char` module が無く、`Float` は `to_string` だけである。`docs/spec.md:1904` の効果を持たない member の一覧に `Char` は無く、`stdlib/float.hd:2-4` も `to_string` だけである。

## 案

- `Char`: `to_int`、`of_int`、`to_string`。`of_int` は Unicode scalar value の `0x0000..=0xD7FF` と `0xE000..=0x10FFFF` だけを受理し、それ以外は `message` を `"InvalidScalar"` とする `{Error}` にする。
- `Float`: `parse`（受理文法は `"-"? digit+ "." digit+`。文字列全体がこれに一致する。先頭の `-` は任意、`+` は無し、整数部と小数部はそれぞれ 1 桁以上、指数は無し、先行 0 と小数部の末尾 0 は許す — `Int.parse` の `runtime/hado_runtime.c:481` が先行 0 を読み飛ばすのと揃える。不正入力は `message` を `"InvalidFloat"` とする `{Error}`。受理した表記は IEEE 754 の round-to-nearest, ties-to-even で f64 に丸め、丸めた結果が無限大になる表記と、0 でないのに最小の非正規数未満で 0 に丸まる表記も `"InvalidFloat"` にする）、`of_int`。`Int.to_float` か `Float.of_int` かは prelude の命名（`Bytes.of_string` `Array.of_list`）に合わせて `of_int`。`of_int` は i64 を IEEE 754 の round-to-nearest, ties-to-even で f64 にする。
- `Float.to_string` はいま §54 の正準形（往復可能な最短の 10 進桁、指数を使わない位取り、小数点の両側に最低 1 桁）を満たしていない。`runtime/hado_runtime.c:514` の `%.17g` は `0.0` を `0`、10 の 300 乗を `1e+300` と出し、`main` の印字だけが `CanonicalFloat` を通る（`crates/hado/src/mono.rs:4409` が `Type::Float` に `RuntimeFunction::CanonicalFloat` を選び、その結果を `PrintString` に渡す）。`runtime/hado_runtime.c:462` の `hado_print_float` は `%.17g` のままだが、コンパイラからの呼び出しが無い（`crates/hado/src/runtime_abi.rs` の宣言だけ）。`main` の印字は `parse` の往復の回帰として測り、直す対象は `Float.to_string` の経路に限る。`parse` の往復の期待に使う前に §54 へ直す。
- 入れない: 大文字化などの Unicode の性質。必要になってから。

## 完了条件

- 各 member の型と、`Char.of_int`・`Float.parse` の失敗の `message` を §55 の失敗の表に書き、受理ケースと、実行時の `{Error}` を `// expect-error:` で測る受理ケース（`docs/corpus.md`）を足す。両方の失敗は実行時の `{Error}` で §56 に静的コードが無いので、拒否コーパス（`// error:`）には置かない。
- `Char.of_int` は `0`、`55295`（`0xD7FF`）、`57344`（`0xE000`）、`1114111`（`0x10FFFF`）を受理し、`-1`、`55296`（`0xD800`）、`57343`（`0xDFFF`）、`1114112`（`0x110000`）は `message` が `"InvalidScalar"` の `{Error}` になる。Int は 10 進のリテラルだけなので（`docs/spec.md:214`）、入力は 10 進で書く（`0xD7FF` は字句解析が `0` で止まる）。受理した各値で `Char.to_int (Char.of_int n) == n` を照合し、`Char.to_string` の出力がそのスカラ値の UTF-8 バイト列になり abort しないことも測る。代表として BMP の `12354`（`0x3042`）と BMP 外の `128512`（`0x1F600`）でも同じ往復を測り、定数を返す `of_int` や `to_int` では通らないようにする。
- `Float.to_string` が §54 の正準形になり、`0.0` は `0.0`、`-0.0` は `-0.0`、`0.1` は `0.1`、10 の 300 乗は `1` の後に `0` が 300 個と `.0` になり、どの出力にも `e` も `E` も含まれない。往復の期待はこの形から測る。`main` の Float の印字（`crates/hado/src/mono.rs:4409` が `CanonicalFloat` を選ぶ経路）も同じ正準形で測る。`def main = 0.0` は `0.0`、`def main = -0.0` は `-0.0`、10 の 300 乗は `1` の後に `0` が 300 個と `.0` になり、どの出力にも `e` も `E` も含まれない。整形器の共有は実装の手段であり、`Float.to_string` の照合や共有の宣言だけでは `main` の印字を省略できない。NaN と無限大も §54（`docs/spec.md:1852`）のとおり `nan` `inf` `-inf` と出す。`Float.to_string (0.0 / 0.0)` は `"nan"`、`Float.to_string (1.0 / 0.0)` は `"inf"`、`Float.to_string (-1.0 / 0.0)` は `"-inf"` になり、`main` の印字でも `def main = 0.0 / 0.0` は `nan`、`def main = 1.0 / 0.0` は `inf`、`def main = -1.0 / 0.0` は `-inf` になる。`0.0 / 0.0` の符号は実装依存なので `-nan` を期待しない。これらの出力は `e` も `E` も含まず、`Float.parse` の受理文法（`"-"? digit+ "." digit+`）の外である。`nan` `inf` `-inf` を印字せず abort する、platform の綴り（`NaN`、`Inf`、`Infinity`、`1.#INF`）を出す、`Float.to_string` だけを測って `main` の印字を省略する実装では通らない。位取りだけの照合（末尾の `.0` が無い形）では通らない。
- `Float.of_int` は `9007199254740992`（2^53）、`9007199254740993`（2^53+1）、`-9007199254740993`、`9223372036854775807`、`(-9223372036854775807 - 1)`（i64 の最小値はリテラルに書けない。`docs/issues/exit-code-from-main.md:35` と同じ）で IEEE 754 の round-to-nearest, ties-to-even に一致する。期待値は判定器側で独立に計算する。
- `Float.parse` の受理文法 `"-"? digit+ "." digit+` と往復（`to_string` → `parse`）を仕様に書き、門番で測る。受理する綴りは `Float.to_string` で測る。`"0.0"` は `"0.0"`、`"-0.0"` は `"-0.0"`、`"0.1"` は `"0.1"`、`"1.0"` は `"1.0"`、先行 0 の `"007.5"` は `"7.5"`、`"-012.0"` は `"-12.0"`、末尾 0 の `"1.00"` は `"1.0"`。成功したかどうかだけを見る照合や、IEEE の `==` だけの照合では通らない。文法の外の入力は、整数だけの形と、文法に合う綴りへ空白・指数・16 進を足した形を含む（空文字列、`"-"`、`"1"`、`"1."`、`"."`、`".5"`、`"+1.0"`（`+` は受理しない）、`"1e3"`、`"1.0e3"`、`" 1.0"`、`"1.0 "`、`"0x1"`、`"0x1.0"`、`"1.2.3"`）。`" 1"` のような、小数点の無い形だけの空白探針では通らない。文法の外の入力は `message` が `"InvalidFloat"` の `{Error}` になる。受理した表記は IEEE 754 の round-to-nearest, ties-to-even で f64 に丸め、丸めた結果が無限大になる表記（f64 の最大有限値を超える）と、0 でないのに最小の非正規数未満で 0 に丸まる表記も `"InvalidFloat"` にして、アンダーフローを実装依存にしない。有限値の往復は `parse (to_string x) == x` になる。`-0.0` は IEEE の `==` では `+0.0` と区別できないので、`Float.to_string (Float.parse "-0.0") == "-0.0"` で測る（§54）。
- `"9007199254740993.0"`（2^53+1 のちょうど中間）が round-to-nearest, ties-to-even で `9007199254740992.0` になる。範囲境界は指数を使わない受理表記で測る。`2` の後に `0` が 308 個と `.0`（2×10^308、最大有限値を超える）と、その先頭に `-` を付けたものと、`0.` の後に `0` が 323 個と `1`（10^-324、最小の非正規数の半分を下回り 0 に丸まる）は `"InvalidFloat"` になる。有限側の境界も測る。`17976931348623157` の後に `0` が 292 個（1.7976931348623157×10^308、最大有限値に丸まる）と、その先頭に `-` を付けたものは有限値として受理し、`Float.to_string` が `17976931348623157` の後に `0` が 292 個と `.0` を返す（§54 の正準形は指数を使わず、いまの runtime も最大有限値をこの形で印字する）。不足側の境界も両側で測る。`0.` の後に `0` が 323 個と `2`（2×10^-324、最小の非正規数の半分 2.4703…×10^-324 を下回り 0 に丸まる）は `"InvalidFloat"`、`3`（3×10^-324、同じ境界を上回り最小の非正規数に丸まる）と `5`（5×10^-324、最小の非正規数）は有限値として受理する。負の側もそれぞれ符号を付けて同じことを測る。有限側の境界を受理しない実装（1×10^308 のような独自の上限で切る実装）、不足側の境界の上を 0 にする実装、負の側の桁あふれを `-inf` として受理する実装では通らない。期待値は判定器側で IEEE の規則から独立に計算する。

## 由来

`docs/wants.md` に無い。§19、§55。
