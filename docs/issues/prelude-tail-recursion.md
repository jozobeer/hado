# prelude の非末尾再帰を末尾形にする（prelude-tail-recursion）
優先度: 高。

## 現象

100 万要素（`build 1000000 []` で末尾再帰で作った `Int List`）で、`fold` だけが通る（2026-09-17）。

| 式 | 結果 |
|---|---|
| `List.fold xs 0 fn(a, x => a + x)` | `500000500000` |
| `List.map xs fn(x => x + 1)` | `hado: stack overflow` |
| `List.filter xs fn(x => x > 0)` | `hado: stack overflow` |
| `List.append xs xs` | `hado: stack overflow` |
| `List.length xs` | `hado: stack overflow` |
| `List.sort xs fn(a, b => a - b)` | `hado: stack overflow` |
| `List.map xs Int.to_string \|> String.join ","` | `hado: stack overflow` |

`map` は 10 万要素なら通る。

`List.filter` は述語を末尾側から呼ぶ。`[1, 2, 3]` に log 付きの述語を渡すと `3 2 1` の順で出た。`filter` は効果多相（§55）なので観測できる差である。`map` は先頭順。§55 は適用順を書いていない。

## 原因

`stdlib/list.hd` の `map` `filter` `append` `length` `sort` の `merge` は素直な再帰で、末尾位置に無い。§53 は末尾呼び出しを保証するが、prelude 自身がその外で書かれている。`def rec` は stdlib 全体で 25 本（`grep -c 'def rec' stdlib/*.hd`）あり、`string.hd` の `split_from` `join`、`array.hd` `bytes.hd` `env.hd` `io.hd` の `to_list` 系も同じ性質である。`docs/plan.md` 6 節は v0.1 で「素直な再帰で書く」と決めていた。

## 案

- 推奨: 累積 + `reverse` に書き直す（`reverse` は `fold` 経由で既に末尾）。添字が取れる列（Array・Bytes・argv・read_dir）は末尾から先頭へ累積して `reverse` を省く。言語意味は足さない。
- §55 に「`map` と `filter` は先頭から順に適用する」を 1 文足す。`filter` の順序が変わるので、ラウンドで仕様変更として記録する。
- 退ける: スタック上限の引き上げ（上限を動かすだけ）。TRMC（§53 の保証範囲を広げる規範変更で、コンパイラ側が大きい）。

## 完了条件

- 100 万要素で `map` `filter` `append` `sort` `length` `String.join` `Array.to_list` が通る。門番は要素数を生成し、終了コード 0 と結果を判定する。
- `filter` の述語が先頭から呼ばれる受理ケースを 1 本足す。

## 由来

`docs/wants.md` 34。§53、§55。
