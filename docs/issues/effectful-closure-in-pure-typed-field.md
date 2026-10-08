# 単相の純粋関数型フィールドに置いた効果付き閉包の outcome が落ちる（effectful-closure-in-pure-typed-field）
優先度: 高。

## 現象

名前付き record（または単相 ADT）のフィールド型を `Fn(Int => Int)`（効果行なし）と書き、
handler の下で `fn(x => 8 / x)` を入れてから呼び出すと、型検査は通るのに実行時は
handler の abort 腕へ届かず、`$Outcome` のタグ付き値を `Int` として印字する
（2026-09-27、effect / codegen 着地後の木）。

```text
handler Error as Catch
abort raise _ => 0
return value => value
type Box = { f: Fn(Int => Int) }
def main =
  with Catch ->
  { f = fn(x => 8 / x) } as b ->
  b.f 0
```

```
$ cargo run --offline -q -- check target/scratch/pure-field/main.hd; echo "check exit=$?"
check exit=0
$ cargo run --offline -q -- run target/scratch/pure-field/main.hd; echo "run exit=$?"
140659220918240
run exit=0
```

同じ入力でフィールド型だけ `Fn(Int => Int {Error})` にすると `0` を印字して終了 0。
成功側（`b.f 2`）も、効果行なしでは `$Done` を `Int` として印字し、効果行ありでは `4`。

単相 ADT でも同じ。

```text
type Pair =
  | Pair(Fn(Int => Int), Fn(Int => Int))
def main =
  with Catch ->
  Pair(fn(x => 8 / x), fn(x => x + 1)) as Pair(f, _) ->
  f 0
```

`dump-mono` では格納する閉包の結果が `$Outcome` なのに、射影と呼び出しの結果型が `Int` のまま。

```
MakeClosure { signature: Signature { parameters: [Int], result: Data("$Outcome", [Int, Int, Void]) } }
RecordGet { field: "f", result: Closure([Int], Int) }
CallClosure { ..., result: Int }
```

## 原因

`crates/hado/src/mono.rs` の `record_get` は、record の scheme から取ったフィールド型に
`outcome_closure_of` を掛ける。宣言が `Fn(Int => Int)` だと効果行が無く、結果型は
`Closure([Int], Int)` のままになる。格納側の lambda は handler 下で `$Outcome` を返す閉包のまま
`Record` に入る。呼び出しは `CallClosure` の結果を `Int` と見て outcome をほどかず、
タグ付き値を印字する。

単相 ADT の射影は `constructor_field_types` が同じく宣言側の Ty だけを見るので、同じ穴になる。
`Some(fn(…))` や `Fn(… {Error})` を書いた形は、型引数か効果行から outcome の閉包へ直せるので通る。

## 案

- 退ける: 実際の field 型を `Data` の引数に載せる（PR #79 で試した）。同じソースの型が
  値の中身によって 2 つの mono の型に割れる。効果を運ぶ値は `$Outcome` の閉包、運ばない値は
  素の閉包になるので、`Fun(fn(x => 8 / x))` と `Num(7)` が同じ `Data("Sh", …)` に合流できない。
  main で `107` と `3` になる形が `invalid branch types` でビルドに失敗する。
- 退ける: handler の下で field の閉包を `$Outcome` の形に揃える（PR #80 の `6747eb1` で試した）。
  揃える範囲が値を作った場所だけでは足りず、関数の境界・型を書いた引数・`Map` の型引数で
  宣言側の型と食い違う。main で `3` になる形が `lowered return type differs from inferred
  signature` や `Arity` でビルドに失敗し、入れ子の閉包では型と値が食い違ったまま codegen に
  進んで SIGILL になる。
- 推奨: 単相の型の閉包の field をどう表すかを型ごとに 1 つに決める。`$Outcome` の答え型を
  載せる場所が要るので、`Box` を答え型について多相にする（`Data("Box", [答え型])` とし、
  宣言側の signature も同じ形にする）か、型に「field が outcome の閉包か」の印を付けて
  signature・引数・コンテナへ同じ形を伝える。どちらも §36 の型の読みに関わるので、人が決める。
- 記録: 試した 2 つの筋と退行の実測は `docs/rounds/effectful-closure-in-pure-typed-field.md`
  にある。枝 `effectful-closure-in-pure-typed-field`（PR #80、下書き）がその実装を持つ。
- 退ける: 型検査で `Fn(…)` フィールドへの効果付き閉包の代入を拒否する。§36 は record field に
  効果を許しており、既存の受理（`effect_position`）と食い違う。

## 完了条件

- 上の record 入力が `0` を印字して終了コード 0 になる（`b.f 2` は `4`）。
- 上の単相 ADT 入力も `0` を印字して終了コード 0 になる。
- 関数の境界を通す形（`def mk = { f = fn(x => x + 1) }` を `with Catch -> mk as b -> b.f 2` で
  呼ぶ）、型を書いた引数（`def call(b: Sh) => b.f 2`）、`Map` の値（`Map.insert` して
  `Map.find` で取り出す）、入れ子の閉包（`Fn(Int => Fn(Int => Int))`）が、いずれも
  main と同じ値のままである。
- 同じ入力が `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。

## 由来

`docs/wants.md` に無い。effect/codegen 着地後の監査（PR #43・#47・#52・#56 周辺、2026-09-27）。§36。
