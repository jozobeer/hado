# 効果を持つ閉包を多相な prelude 関数に通すと単相化が落ちる（effectful-closure-through-prelude）
優先度: 高。

## 現象

型検査は通る（`hado check` が終了コード 0）のに、効果を持つ閉包のリストを `List.reverse`・`List.map` に渡すと単相化が落ちる。`Array.set`・`Array.to_list` も同じ穴で落ちる（2026-09-25、effectful-closure-in-array の修正後の木）。各入力の先頭は `handler Error as Catch` / `abort raise _ => 0` / `return value => value` の 3 行。

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
```

`reverse`・`map` は effectful-closure-in-array の前（df4f2ac）でも同じ出力。`to_list`・`set` は前は `array intrinsic argument type mismatch` で、いまは List 側で落ちる。4 つとも 2026-09-26 の二次レビュー対応（結果型を実際の型の単一化から取る）の後も同じ出力。

返す閉包の引数の位置に型変数が現れる形も同じ穴で落ちる。契約の member が `Fn(A => Int)` を返し、その閉包の引数の型が実装の引数から決まらないと、単相化は結果型を解けない（三次レビュー、2026-09-26）。

```
$ cat maker/main.hd | tail -n +4
def main =
  with Catch ->
  let x = Some(fn(y => 8 / y)) ->
  let f = Maker.make x ->
  f x
module Maker
def A make(x: A): Fn(A => Int) Pure
implement Maker as Maker
def make _ => fn(_ => 1)
$ cargo run --offline -q -- run maker/main.hd
unresolved return type: Variable(3)
```

この入力は `e42dd6f` の親（`4ab9a98`）でも `9469380` でも同じ出力で落ちる。実装 `def make _ => fn(_ => 1)` の scheme は `Fn(A => Fn(B => Int {C}) {D})` で、閉包の引数の型 `B` はどの引数からも決まらない。`specialize_result` はこの `B` を解決できず、結果型を組む前に止まる。

## 原因

`crates/hado/src/mono.rs` の `specialize_function` は scheme を `logical_argument` の論理型で単一化するので、多相な callee の本体では型変数が outcome の無い閉包に解ける。本体で作るリスト（`[head, ...acc]`）や閉包呼び出しの型は論理型、引数から来た値は実際の型で、両者が食い違う。

2026-09-26 の二次レビュー対応で、結果型は「実際の型での単一化」から取るようになった（scheme が引数の型に関数型を書いている引数だけ論理型で束縛する）。`restore_nested_outcomes` は消えた。残っているのは本体の側で、`List.map` の `Fn(A => B)` のように scheme が関数型を書いた引数は論理型で束縛するので、本体が作るリストの要素型は outcome の無い閉包のままになる。呼び出し元は同じリストの要素を効果付きの閉包として扱うので、`call arguments do not match the closure signature` で食い違う。`restored_array_allocation`・`restored_success_type` は、結果型と本体の型が食い違う箇所を個別に埋めている。

返す閉包の引数の型が引数から決まる形でも、本体は `logical` の型で下ろす（`lower_specialization` が `specialization.unify(scheme.body(), &concrete)` に `logical` を渡す）。結果型だけ実際の型にすると、本体が作る閉包の引数の型と食い違う。本体の側を `mixed` にすると scheme の単一化そのものが `Mismatch` になる（2026-09-26、試して戻した）。

## 案

- 推奨: 本体も実際の型で下ろす。scheme が関数型を書いた引数にも実際の型を束縛し、本体の呼び出しと構築を効果付きの型で解く。`restore_*` は要らなくなる見込み（未確認）。`mixed` を `concrete` にそのまま渡す形は scheme の単一化が `Mismatch` になるので、本体が読む型の解決だけを実際の型へ寄せる必要がある（2026-09-26、試して戻した）。
- 退ける: 落ちる箇所ごとに `restored_*` を足す。stdlib の関数の書き方ごとに穴が残る。

## 完了条件

- 上の 5 つの入力が、それぞれ期待値（`reverse` は `0`、`map` は `2`、`to_list` は `2`、`set` は `3`、`maker` は `1`）を印字して終了コード 0 になる。
- 同じ 5 つが `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。

## 由来

`docs/wants.md` に無い。effectful-closure-in-array の周辺の実測（`docs/rounds/effectful-closure-in-array.md`）。§19、§36。残る穴の切り出しは 2026-09-26（同ラウンドの二次レビュー対応）。返す閉包の引数の位置の形は同ラウンドの三次レビュー（PR #43）で足した。
