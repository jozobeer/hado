# `implement Map` の中の型名が根の名前空間へ漏れ、利用者の `type T` と E205 で衝突する（map-internal-types-leak-into-root）
優先度: 高。

`map-by-builtin-compare`（PR #57、`bb44419`）の着地で入った退行。prelude を使う利用者のプログラムが、`implement` の中だけで使う型名と同じ名前の型を宣言できなくなった。

## 現象

`Map` の実装（`stdlib/map.hd` の `implement Map as Map`）が内部型として `type (K, V) T` を宣言する。この名前 `T` が根の名前空間へ出るため、利用者が `type T` を宣言すると E205 になる。

```
$ cat main.hd
type T = { a: Int }
def main = 0
$ cargo run --offline -q -- check main.hd
main.hd:235:13: error[E205]: local data type conflicts with an existing type
```

`Map` の内部型は 3 つあり、どれも同じように漏れる。

```
$ for n in T MapColor MapStep; do printf "type $n = { a: Int }\ndef main = 0\n" > main.hd; cargo run --offline -q -- check main.hd; done
T:        main.hd:235:13: error[E205]: local data type conflicts with an existing type
MapColor: main.hd:234:6:  error[E205]: local data type conflicts with an existing type
MapStep:  main.hd:239:13: error[E205]: local data type conflicts with an existing type
```

既存の門番 `scripts/verify-r60.sh` がこれで赤になった（60-d・60-g）。fixture は `type T = { a: Int; b: String }` を使っている。

```
$ bash scripts/verify-r60.sh
AssertionError: ('E305', 'main.hd:245:13: error[E205]: local data type conflicts with an existing type')
NG: 60-d the field of a record literal
AssertionError: ('E305', 'main.hd:236:13: error[E205]: local data type conflicts with an existing type')
NG: 60-g one report per run
PASSED: 6/8
```

`map-by-builtin-compare` の着地の前後で変わる（`bb44419^` = `38ea671` では `PASSED: 8/8`、`bb44419` では `6/8`）。

## 原因

`stdlib/map.hd` の `implement Map as Map` が `type (K, V) T = | MapLeaf | MapNode(...)` を宣言し、これが根の名前空間へ出る。`module Map` が公開する抽象型 `(K, V) T` と同じ名前を、実装の側でも `type (K, V) T` として定義しているため、実装の宣言が根へ漏れる。`docs/rounds/map-by-builtin-compare.md` の「実装から分かったこと」が記録する `type Color` の E205 と同じ族で、そのときは `Map` 前置き（`MapColor`・`MapStep`）を付けて避けたが、**型 `T` だけは前置きを付けておらず、しかも `Map` の公開名と同じなので残った**。`MapColor`・`MapStep` も根へ漏れる（前置きは衝突を減らすが、漏れ自体は塞いでいない）。

## 案

- 推奨: `implement` の中の `type` を根へ出さない。`implement` の本文は §47 の契約の実装なので、そこだけの名前空間に入れる（`module` の公開名と衝突しない形にする）。あわせて `stdlib/map.hd` の内部型名を、公開名と重ならない名前にする（`T` を `MapTree` などに）。
- 次点: 内部型名を全て `Map` 前置きにし、利用者の名前空間と衝突しない名前にする（`MapColor` → `MapNodeColor`、`MapStep` → `MapDeleteStep`、`T` → `MapTree`）。漏れ自体は残るので、利用者が同じ名前を宣言すると依然として E205 になる。
- 退ける: E205 の検査を緩める。名前の衝突を黙って通すと、利用者の型と実装の型が混ざる。

## 完了条件

- `type T = { a: Int }`・`type MapColor = ...`・`type MapStep = ...` を宣言する利用者のプログラムが、E205 にならずに `check` を通る（prelude を使う根で）。
- `bash scripts/verify-r60.sh` が 8/8。
- `bash scripts/verify-map-by-builtin-compare.sh` が 9/9 のまま（`Map` の公開 member と E204 の検査を弱めない）。
- `cargo test --offline` が 0 failed、`bash scripts/run-gates.sh --tier full` が red なし。

## 由来

`docs/wants.md` 33（`Map`）。`map-by-builtin-compare` の着地（`bb44419`）で `verify-r60.sh` が赤くなったことから。§47、E205。
