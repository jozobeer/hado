# 効果を持つ閉包を多相な prelude 関数に通すと単相化が落ちる（effectful-closure-through-prelude）
優先度: 高。

## 現象

型検査は通る（`hado check` が終了コード 0）のに、効果を持つ閉包のリストを `List.reverse`・`List.map` に渡すと単相化が落ちる。`Array.set`・`Array.to_list` も同じ穴で落ちる（2026-09-25、effectful-closure-in-array の着地後の木）。各入力の先頭は `handler Error as Catch` / `abort raise _ => 0` / `return value => value` の 3 行。

```
$ cat reverse/main.hd | tail -n +4
def main =
  with Catch ->
  let fs = List.reverse [fn(x => 8 / x), fn(x => 9 / x)] ->
  if fs | [g, ...rest] => g 0 + List.length rest | 5
$ cargo run --offline -q -- run reverse/main.hd
call arguments do not match the closure signature
$ cat map/main.hd | tail -n +4
def main =
  with Catch ->
  List.map [fn(x => 8 / x), fn(x => 9 / x)] fn(f => f 3) |> List.length
$ cargo run --offline -q -- run map/main.hd
call arguments do not match the closure signature
$ cat to_list/main.hd | tail -n +4
def main =
  with Catch ->
  let xs = Array.of_list [fn(x => 8 / x), fn(x => 9 / x)] ->
  let fs = Array.to_list xs ->
  List.map fs fn(f => f 2) |> List.length
$ cargo run --offline -q -- run to_list/main.hd
list element type mismatch
$ cat set/main.hd | tail -n +4
def main =
  with Catch ->
  let xs = Array.of_list [fn(x => 8 / x)] ->
  let ys = Array.set xs 0 fn(x => x * 3 / x) ->
  let f = Array.get ys 0 ->
  f 5
$ cargo run --offline -q -- run set/main.hd
list element type mismatch
$ cat mixed/main.hd | tail -n +4
def main =
  with Catch ->
  let fs = [fn(x => 8 / x), fn(x => x + 1)] ->
  if fs | [_, g] => g 4 | 0
$ cargo run --offline -q -- run mixed/main.hd
list element type mismatch
```

`reverse`・`map`・`mixed` は effectful-closure-in-array の前（df4f2ac）でも同じ出力。`to_list`・`set` は前は `array intrinsic argument type mismatch` で、いまは List 側で落ちる。

## 原因

`crates/hado/src/mono.rs` の `specialize_function` は scheme を `logical_argument` の論理型で単一化するので、多相な callee の本体では型変数が outcome の無い閉包に解ける。本体で作るリスト（`[head, ...acc]`）や閉包呼び出しの型は論理型、引数から来た値は実際の型で、両者が食い違う。`restore_nested_outcomes` は callee の結果型と `__array_alloc`・abort の成功型だけを戻し、本体の中の構築と呼び出しは戻さない。`mixed` は要素ごとに効果の違う閉包で、要素型を 1 つの outcome の閉包へ直す `outcome_closure_of` が、効果を持たない要素を直さない。

## 案

- 推奨: 型変数には実際の型を、scheme が関数型を書いた位置にだけ論理型を束縛する。本体の型を解くときに実際の型が出るので、`restore_nested_outcomes` と `restored_array_allocation`・`restored_success_type` は要らなくなる見込み（未確認）。
- 退ける: 落ちる箇所ごとに `restored_*` を足す。stdlib の関数の書き方ごとに穴が残る。

## 完了条件

- 上の 5 つの入力が、それぞれ期待値（`reverse` は `0`、`map` は `2`、`to_list` は `2`、`set` は `3`、`mixed` は `5`）を印字して終了コード 0 になる。
- 同じ 5 つが `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。

## 由来

`docs/wants.md` に無い。effectful-closure-in-array の周辺の実測（`docs/rounds/effectful-closure-in-array.md`）。§19、§36。
