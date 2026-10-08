# 多相 record の field に載せた curried な効果付き閉包が実行時に落ちる（curried-outcome-closure-in-polymorphic-record）
優先度: 中。

## 現象

型引数を持つ record の field に、結果が閉包になる効果付きの閉包を載せて呼び出すと、
型検査は通るのに実行時に `hado: exit code out of range` になる（2026-10-04、
`effectful-closure-in-pure-typed-field` の着地後。main でも同じ）。

```text
handler Error as Catch
abort raise _ => 0
return value => value
type (A, B) Box = { f: A; g: B }
def main =
  with Catch ->
  { f = fn(x => 8 / x); g = fn(x => fn(y => x / y)) } as b ->
  b.f 2 + (b.g 40) 2
```

```
$ cargo run --offline -q -- run target/probe/b2/main.hd; echo "exit=$?"
hado: exit code out of range
exit=1
```

`b.f 2` は `0`（`8 / 2` の abort 腕）、`(b.g 40) 2` は `20` になるはずで、和は `20`。
`g` の内側の閉包を素の `Int` として読んでいる。

## 原因

`crates/hado/src/mono.rs` の `actual_type` は `Ty::Fn` の結果を再帰して `answer_for_row`
で `$Outcome` を載せるが、field の宣言が型引数 `B` で、その `B` が入れ子の `Ty::Fn` に
なっている場合、内側の行が空のまま `Closure([Int], Int)` になる。`record_get` の射影は
その型をそのまま返すので、`CallClosure` が `$Outcome` をほどかない。

`dump-mono` では `g` の値が `Closure([Int], Closure([Int], $Outcome))` なのに、record の
型引数と射影は `Closure([Int], Closure([Int], Int))` になる。

## 案

- 推奨: 射影の結果型を、record の値に実際に入っている閉包の型に合わせる。入れ子の
  結果についても `$Outcome` を保つ（`outcome_form` が単相の型で行っているのと同じ）。
- 退ける: 入れ子の閉包を field に載せることを拒否する。§36 は許している。

## 完了条件

- 上の入力が `20` を印字して終了コード 0 になる。
- 同じ入力が `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed。

## 由来

`effectful-closure-in-pure-typed-field` のラウンドで見つけた残件（2026-10-04）。
`docs/rounds/effectful-closure-in-pure-typed-field.md` の「多相 record」。§36。
