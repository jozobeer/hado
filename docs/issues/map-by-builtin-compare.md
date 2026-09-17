# 連想配列を組み込み `compare` の上に置く（map-by-builtin-compare）
優先度: 中。

## 現象

単語頻度の CLI で連想リストを自分で書いた（`Entry List` と `bump`）。20 万語 × 語彙 500 で線形探索になる。prelude に `Map` は無い。

## 原因

`docs/wants.md` 33 は association list を想定している。typeclass も演算子過負荷も無い（§4）ので、比較を誰が持つかが決まっていなかった。

組み込みの `compare` は単相化時に型ごとに派生され（`crates/hado/src/mono.rs` の `derive_comparison`）、契約経由の多相関数から型変数の上で呼べることを実測した（2026-09-17）。

```
$ hado run main.hd   # module Ord / def A less(a: A, b: A): Bool / implement: compare a b < 0
[true, false, true]  # Ord.less 1 2, Ord.less "b" "a", Ord.less [1] [1, 2]
```

## 案

- prelude の契約 `Map` に抽象型 `(K, V) T` と `empty` `insert` `find` `remove` `fold` `to_list` を置く。`implement Map as Map` の中だけに平衡木の ADT を実体化し、constructor を外へ出さない。公開 member の型は `(K, V) Map.T` を使い、内部で `compare` を呼ぶ。比較関数は渡さない。Fn を含むキーは E704 で単相化時に弾かれ、wants 33 の「キーは Fn を含まない」と一致する。Float キーの NaN の順は §21 が既に決めている。
- 独自の順序が要る日は functor（§48）で `MakeMap(Key: Ord)`。いまは作らない。

## 完了条件

- `Map` の member の型を §55 に書く。`fold` と `to_list` はキーの昇順。
- 外から内部 constructor を構築・pattern 分解すると E204 になる。`empty` から公開 member だけで作った値について、挿入・削除後も順序と平衡条件を満たすことを門番で判定する。
- 単語頻度の CLI を `Map` で書き直し、20 万語で `bump` 版より速いことを門番で測る。

## 由来

`docs/wants.md` 33。§4、§21、§48、§55。
