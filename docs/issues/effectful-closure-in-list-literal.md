# 効果の違う閉包を並べたリストリテラルで単相化が落ちる（effectful-closure-in-list-literal）
優先度: 中。

## 現象

型検査は通る（`hado check` が終了コード 0）のに、効果を持つ閉包と持たない閉包を 1 つのリストリテラルに並べると、単相化が落ちる（2026-09-25、df4f2ac と effectful-closure-in-array の修正後の木で同じ出力）。

```
$ cat main.hd
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  let fs = [fn(x => 8 / x), fn(x => x + 1)] ->
  if fs | [_, g] => g 4 | 0
$ cargo run --offline -q -- run main.hd
list element type mismatch
```

要素の効果が揃っている `[fn(x => 8 / x), fn(x => 9 / x)]` は通る。

## 原因

`crates/hado/src/mono.rs` の `list_elements` は、要素型が効果を運ぶ関数なら `outcome_closure_of` で要素型を outcome の閉包にするが、効果を持たない要素の閉包は outcome の無い閉包のまま lower されるので、要素型の照合で食い違う。

## 案

- 推奨: 要素型が outcome の閉包のとき、outcome の無い閉包の要素を、結果を `done` で包む閉包へ直してから並べる。
- 退ける: 要素ごとに効果が違うリストを拒否する。§36 の型では受理される形で、仕様変更になる。

## 完了条件

- 上のプログラムが `5` を印字して終了コード 0 になる。
- 同じ入力が `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed。

## 由来

`docs/wants.md` に無い。effectful-closure-in-array の周辺の実測（`docs/rounds/effectful-closure-in-array.md`）。§36。
