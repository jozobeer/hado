# implement の局所型と根の同名 variant が型検査で同じ型に混ざる（implement-local-adt-identity-is-name-only）
優先度: 中。

`map-internal-types-leak-into-root` で名前解決の漏れを塞いだ結果、`implement` の局所型と同名の variant を根で宣言できるようになった。ところが型検査では両者が同じ型 `Ty::Adt("T")` に束ねられるため、利用者の constructor が prelude の `implement Map as Map` の網羅性へ混ざり、E402 になる。

## 現象

`type T = { a: Int }`（record）は `check` を通る。variant にすると、利用者の constructor `A`・`B` が prelude の `type (K, V) T` の constructor と混ざり、prelude の `main.hd:247` 以降（`stdlib/map.hd` の `implement Map as Map`）が網羅でなくなる。

```
$ cat main.hd
type T = | A | B
def main =
  if A | A => 1 | _ => 2
$ cargo run --offline -q -- check main.hd
main.hd:247:1: error[E402]: not all constructor values are covered
```

`MapColor`・`MapStep` の variant でも同じで、それぞれ `main.hd:252:1`・`main.hd:281:16` を指す。`type U = | A | B` のように名前が重ならなければ `E503` の判定へ進む。既存の `scripts/verify-r62.sh`・`scripts/verify-r63.sh`・`scripts/verify-r68.sh` は fixture に `type T = | A | B` を持つため、この欠陥で赤くなる。

## 原因

`crates/hado/src/infer.rs` の `register_type` が `Ty::Adt(def.name.clone(), ...)` と名前だけで型の同一性を作る。`register_module_items` は `implement` の本文の型も同じ平らな `constructors` に登録するので、根の `type T = | A | B` と prelude の実装の `type (K, V) T = | MapLeaf | MapNode(...)` が同じ `T` に束なる。`constructor_groups` はその平らな `constructors` を `Ty::Adt` の名前でまとめて網羅性の family にするため、`MapLeaf`・`MapNode` だけを覆う腕が `A`・`B` を覆わないと判定される。`qualify_module_constructors` は修飾した写しを `module_constructors` に持つが、網羅性は平らな側を見る。§47、§49、E402。

## 案

- 推奨: `implement` の本文の型を、その実装の名前で修飾した同一性で型検査へ登録する。`qualify_module_exports` が本文の検査後に通している修飾（`Map.T`）を、本文の検査の間も一貫させる。
- 次点: `constructor_groups` を、その本文で見える型だけに閉じる。
- 退ける: variant のときだけ E402 を緩める。利用者の型と実装の型が混ざる原因を残す。

## 完了条件

- `type T = | A | B`・`type MapColor = | A | B`・`type MapStep = | A | B` を宣言する利用者のプログラムが、prelude を使う根で `check` を通る。
- `bash scripts/verify-r62.sh`・`bash scripts/verify-r63.sh`・`bash scripts/verify-r68.sh` が赤なし。
- `cargo test --offline` が 0 failed。

## 由来

`docs/wants.md` 33（`Map`）と §47・§49。`map-internal-types-leak-into-root` の名前解決の修正で E205 が消え、その奥にあったこの欠陥へ届くようになった。E402。
