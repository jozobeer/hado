# 抽象型を含む契約 member の効果行を一般化する（contract-member-effect-generalization）
優先度: 中。

## 現象

`Map.fold` に効果を持つ累算器を渡すと `E706` になる。同じ形の `List.fold` は通る。

```
$ cat main.hd
effect Yield
again yield(key: String): Void
handler Yield as Collect
again yield key =>
  let rest = resume () ->
  [key, ...rest]
return _ => []
def mark k => do yield k -> k
def main =
  let m = Map.insert (Map.insert (Map.insert Map.empty "3" 0) "1" 0) "2" 0 ->
  with Collect -> Map.fold m "" fn(acc, key, _ => String.concat acc (mark key))
$ cargo run --offline -q -- build -o prog main.hd
main.hd:9:1: error[E706]: main cannot have a user-defined effect
$ cargo run --offline -q -- check main.hd
main.hd:9:1: error[E706]: main cannot have a user-defined effect
```

`check --print-type` はこの欠陥を見逃す（`build` と `check` は赤、`--print-type` は緑）。

```
$ cargo run --offline -q -- check --print-type main.hd
String List {Yield}
```

契約に抽象型を置かず、型を top level で宣言した同じ形の `fold` は通る。

```
$ cat list_fold.hd
effect Yield
again yield(key: String): Void
handler Yield as Collect
again yield key =>
  let rest = resume () ->
  [key, ...rest]
return _ => []
def mark k => do yield k -> k
def key_mark | MapEntry(k, _) => mark k
def main =
  let m = Map.insert (Map.insert (Map.insert Map.empty "3" 0) "1" 0) "2" 0 ->
  with Collect -> List.fold (Map.to_list m) "" fn(acc, entry => String.concat acc (key_mark entry))
$ cargo run --offline -q -- build -o prog list_fold.hd && ./prog
["1", "2", "3"]
```

そのため `docs/rounds/map-by-builtin-compare.md` の門番は、`fold` の訪問順を効果の列ではなく非可換の String 累算器で測っている。

## 原因

署名が抽象型（`Map` なら `(K, V) T`）を含む契約 member は、効果行が一般化されない。`crates/hado/src/infer.rs` の `contract_exports` と `signature_type` が member の型を組み立て、`crates/hado/src/infer.rs` の `row_annotation` が効果の書かれていない member に `Inference::fresh_row` を割り当てる。`crates/hado/src/types.rs` の `generalize` は型変数だけを全称量化し、`crates/hado/src/types.rs` の `Scheme` の `row_constraints` は比較制約のためのもので効果行を運ばない。よって効果行の変数は scheme に入らず、`Map.fold` の 1 つの単相な型として固定される。

`List.fold` は型が top level の `T List` で、契約の抽象型を通らないため効果行が一般化される。§55 は「`Fn` を引数に取る `List.map` `filter` `fold` と `Option.map` は効果多相なので効果を書かない」とするが、この規則は抽象型を持たない member でだけ成り立っている。抽象型を隠す契約の member では、効果を書かないと（`row_annotation` が開いた行を作るので）効果多相に見えて実際は単相に潰れ、`main` の効果行が利用者の効果を含んで `crates/hado/src/infer.rs` の `check_main_type` が `E706` を出す。

`--print-type` が緑なのは、`crates/hado/src/lib.rs` が `Inspection` の印字で `E706` を落とすため。判定のずれ自体はこの issue の主題ではない。

## 案

契約 member の scheme に効果行の変数も入れる。`generalize` に効果行の全称量化を足し、`crates/hado/src/infer.rs` の `signature_type` が作った行を `Scheme` に載せて、`implement` 側の `def` と単相化がそれを実体化する形にする。§55 の「効果を書かない」規則が、抽象型を持つ member でもそのまま成り立つようになる。

代替として、§55 の規則を「抽象型を持つ契約 member は効果を明示する」と狭める案もある。効果を持つ `Fn` を渡せないことが仕様として意図されているならこちらを選ぶが、`List.fold` との差が利用者から見て説明しにくい。

`E706` の判定と `--print-type` の印字が食い違う件は、この issue では直さない。効果行を一般化すれば `main` の型に利用者の効果が現れなくなり、両方とも緑になるため。

## 完了条件

- 効果を持つ累算器を `Map.fold` に渡す上の `main.hd` が `cargo run --offline -q -- build` と `cargo run --offline -q -- check` を通り、実行して訪問順が効果の列（`["1", "2", "3"]`）として観測できる。
- 同じ入力で `List.fold` と `Map.fold` の結果が一致する。
- 既存の効果の門番（`scripts/verify-r37.sh` など）が赤くならない。
- `docs/spec.md` の §55 の「効果を書かない」規則を変えるなら、§55 の本文と受理コーパスを同時に直す。

## 由来

`docs/wants.md` 33（連想配列。もっとも、この番号の項目は `docs/wants.md` に見当たらない。`docs/plan.md` の 962 行と 984 行も 33 を参照している）と、`docs/rounds/map-by-builtin-compare.md` の「仕様への疑義」。
