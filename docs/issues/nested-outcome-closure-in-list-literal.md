# 効果の違う入れ子の閉包を並べたリストリテラルで単相化が落ちる（nested-outcome-closure-in-list-literal）
優先度: 中。

## 現象

効果を持つ閉包を返す閉包と、持たない閉包を返す閉包を 1 つのリストリテラルに並べると、
型検査は通る（`hado check` が終了コード 0）のに、単相化が `list element type mismatch`
で落ちる（2026-09-27、`effectful-closure-in-list-literal` の着地後の木）。

```text
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  let fs = [fn(x => fn(y => x * 0 + 8 / y)), fn(x => fn(y => x * 0 + y + 1))] ->
  if fs | [_, g] => (g 0) 4 | 0
```

```
$ cargo run --offline -q -- check target/scratch/listlit-nested/main.hd; echo "check exit=$?"
check exit=0
$ cargo run --offline -q -- run target/scratch/listlit-nested/main.hd; echo "run exit=$?"
list element type mismatch
run exit=1
```

## 原因

`crates/hado/src/mono.rs` の `list_elements` は、宣言側の要素型が outcome の閉包のとき
`as_outcome_closure` で要素を直すが、直すのは要素の結果が直接 `$Outcome` のときだけである。
要素が閉包を返し、その閉包の結果が `$Outcome` の場合（入れ子の outcome）、
`as_outcome_closure` は対象の結果型が outcome でないので何も直さず、`cons_list` の
要素型の照合が `list element type mismatch` で止める。§36 の型では受理される形である。

## 案

- 推奨: `as_outcome_closure` を入れ子へ広げる。要素が返す閉包の結果が outcome のとき、
  返り値を呼び出し側で `done` で包む閉包へ直してから並べる。
- 退ける: 効果の違う入れ子の閉包を並べたリストを拒否する。§36 の型では受理される形で、
  仕様変更になる。

## 完了条件

- 上のプログラムが `5` を印字して終了コード 0 になる。
- 同じ入力が `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。

## 由来

`docs/wants.md` に無い。`effectful-closure-in-list-literal` の実測
（`docs/rounds/effectful-closure-in-list-literal.md` の「入れ子の要素」、2026-09-27）。§36。
