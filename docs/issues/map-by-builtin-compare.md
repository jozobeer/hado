# 連想配列を組み込み `compare` の上に置く（map-by-builtin-compare）
優先度: 中。

## 現象

単語頻度の CLI で連想リストを自分で書いた（`Entry List` と `bump`）。20 万語 × 語彙 500 で線形探索になる。prelude に `Map` は無い。

## 原因

`docs/wants.md` 33 は association list を想定している。typeclass も演算子過負荷も無い（§4）ので、比較を誰が持つかが決まっていなかった。

組み込みの `compare` は単相化時に型ごとに派生され（`crates/hado/src/mono.rs:3294` の `derive_comparison`）、契約経由の多相関数から型変数の上で呼べることを実測した（2026-09-17）。

```
$ hado run main.hd   # module Ord / def A less(a: A, b: A): Bool / implement: compare a b < 0
[true, false, true]  # Ord.less 1 2, Ord.less "b" "a", Ord.less [1] [1, 2]
```

## 案

- prelude の契約 `Map` に抽象型 `(K, V) T` と `empty` `insert` `find` `remove` `fold` `to_list` を置く。`implement Map as Map` の中だけに平衡木の ADT を実体化し、constructor を外へ出さない。公開 member の型は `(K, V) Map.T` を使い、内部で `compare` を呼ぶ。比較関数は渡さない。Fn を含むキーは E704 で単相化時に弾かれ、wants 33 の「キーは Fn を含まない」と一致する。Float キーの NaN の順は §21 が既に決めている。
- 独自の順序が要る日は functor（§48）で `MakeMap(Key: Ord)`。いまは作らない。
- `compare` が `0` を返す等価な鍵は 1 つに畳む。`insert` は既存の値だけを置き換えて先に入っている鍵を保ち、`find` は等価な鍵で現在の値を返し、`remove` は等価な鍵を 1 つ消す。Float の `-0.0` と `+0.0`、NaN 同士は §21 により同順位なので同じ鍵に畳まれる。
- `-0.0` の鍵を `Float.to_string` で測るので、char-and-float-conversions（§54 の正準形）の後に置く。

## 完了条件

- `Map` の member の型を §55 に書く。`fold` と `to_list` はキーの昇順。
- `fold` を非可換の累算器で直接呼び、累算器の呼び出し順を効果の列（訪問した鍵を log するなど）で測る。挿入順が昇順でない 3 件以上の相異なる鍵（例: `3` のあと `1` のあと `2`）を入れ、呼び出しがキーの昇順（判定器側で `compare` により独立に並べた列。例: `1`, `2`, `3`）になることを照合する。降順に呼んで結果を reverse する実装や、`to_list` との一致だけでは通らない。空や 1 件では測らない。
- 外から内部 constructor を構築・pattern 分解すると E204 になる。`empty` から公開 member だけで作った値について、挿入・削除後も順序と平衡条件を満たすことを門番で判定する。公開 member の結果だけでは木の形が見えないので、`implement Map as Map` の中に門番専用の内部 member（高さなど）を置くか、`compare` の呼び出し回数を数える計測用の runtime stub を用意する。実体は赤黒木（根は黒、赤が連続しない、根から葉までの黒高さが等しい）とし、判定器は各 `n` について高さ上限 `2 * floor(log2(n + 1))` と、1 回の `insert` `find` `remove` の `compare` 上限 `4 * floor(log2(n + 1))` を独立に計算して照合する。比較回数の上限は観測した高さの倍率にはしない。測定する `n` には、昇順挿入の素の二分探索木の高さと最後の `insert` の `compare` 回数がその上限を超える大きさを含め、昇順・降順・ランダムな挿入と削除で測る。
- 単語頻度の CLI を `Map` で書き直し、20 万語で `bump` 版より速いことを門番で測る。
- 重複する鍵、`-0.0` の後に `+0.0`、NaN 同士を `insert` し、`to_list` の長さが 1、値が後から入れたもの、`find` がどちらの鍵でもその値を返すこと、`remove` がどちらの鍵でもその 1 件を消すことを測る。`-0.0` の後に `+0.0` を入れたときの鍵は IEEE の `==` では区別できないので、`to_list` の鍵の `Float.to_string` が `"-0.0"` であることを測る（§54）。NaN 同士は `==` が false なので、長さ 1 と `find`/`remove` で畳みを測り、鍵の `==` では照合しない。

## 由来

`docs/wants.md` 33。§4、§21、§48、§55。
