# 構築子を 2 つ持つ単相 ADT の field に載せた効果付き閉包の outcome が落ちる（multi-constructor-adt-field-outcome）
優先度: 中。

## 現象

構築子を 2 つ以上持つ単相の ADT の field を `Fn(Int => Int)`（効果行なし）と書き、
handler の下で `fn(x => 8 / x)` を入れてから取り出すと、型検査は通るのに実行時に
`hado: exit code out of range` になる（2026-10-04、`effectful-closure-in-pure-typed-field`
の着地後）。

```text
handler Error as Catch
abort raise _ => 0
return value => value
type Sh = | Fun(Fn(Int => Int)) | Num(Int)
def main =
  with Catch ->
  Fun(fn(x => 8 / x)) as t ->
  if t | Fun(f) => f 0 | Num(n) => n
```

```
$ cargo run --offline -q -- run target/probe/b4/main.hd; echo "exit=$?"
hado: exit code out of range
exit=1
```

`check` は終了 0 で通る。構築子が 1 つの ADT（`type Box = | Box(Fn(Int => Int))`）と
record は同じ入力で `0` になる。

## 原因

`crates/hado/src/mono.rs` の `constructed_type` は、単相の ADT で実際の field 型を
`Data` の引数に載せるのを構築子が 1 つのときに限る。2 つ以上あると構築子ごとに field の
並びが違うので、`Fun(fn(x => 8 / x))` と `Num(7)` が同じ `Data("Sh", …)` に合流できない。
`constructor_field_types` は宣言側の `Ty` だけを見るので、取り出した `f` の型は
`Closure([Int], Int)` のままになり、呼び出しが `$Outcome` をほどかない。

構築子ごとに field の並びが違う型で、field の表現を 1 つに決める方法が要る。

## 案

- 推奨: 単相の ADT の field の表現を型ごとに 1 つに決める。全構築子の field を
  構築子の順に平坦に並べた `Data` の引数にし、取り出しは自分の構築子の位置を読む。
  `constructor_field_types` は構築子の identity を見て位置を決める。
- 退ける: 多構築子の単相 ADT の field への効果付き閉包の代入を型検査で拒否する。
  §36 は record field に効果を許しており、受理の側と食い違う。

## 完了条件

- 上の入力が `0` を印字して終了コード 0 になる。
- `if t | Num(n) => n` の側も `n` を `Int` として読む（`Fun` の field 型を当てない）。
- 同じ入力が `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed。

## 由来

`effectful-closure-in-pure-typed-field` のラウンドで見つけた残件（2026-10-04）。
`docs/rounds/effectful-closure-in-pure-typed-field.md` の「多構築子の ADT」。§36。
