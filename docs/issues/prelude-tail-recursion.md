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

`stdlib/list.hd:15-46` の `map`（15）`filter`（17）`append`（21）`length`（19）`sort` の `merge`（24）は素直な再帰で、末尾位置に無い。§53 は末尾呼び出しを保証するが、prelude 自身がその外で書かれている。`def rec` は stdlib 全体で 25 本（`grep -c 'def rec' stdlib/*.hd`）あり、`stdlib/string.hd:26-38` の `split_from` `join`、`stdlib/array.hd` `stdlib/bytes.hd` `stdlib/env.hd` `stdlib/io.hd` の `to_list` 系も同じ性質である。`docs/plan.md` 6 節は v0.1 で「素直な再帰で書く」と決めていた。

## 案

- 推奨: 累積 + `reverse` に書き直す（`reverse` は `fold` 経由で既に末尾）。添字が取れる列（Array・Bytes・argv・read_dir）は末尾から先頭へ累積して `reverse` を省く。言語意味は足さない。
- `stdlib/` の 25 本の `def rec` を末尾・非末尾に分類し、非末尾の helper へ到達する公開 member と効果操作を対応付ける。この表をラウンドの手順書に置き、対応の無い非末尾再帰を残さない。
- §55 に「`map` と `filter` は先頭から順に適用する」を 1 文足す。`filter` の順序が変わるので、ラウンドで仕様変更として記録する。
- 退ける: スタック上限の引き上げ（上限を動かすだけ）。TRMC（§53 の保証範囲を広げる規範変更で、コンパイラ側が大きい）。

## 完了条件

- 100 万要素で `List.map` `filter` `append` `sort` `length`、`List.take`（100 万件取る）、`Array.set`（末尾の index）`to_list`、`Bytes.to_list` が通る。`Env.args` と `Io.read_dir` は変更前に stack overflow する件数の実入力に加え、門番用 runtime stub が 100 万件を返す public path で運転する。門番は終了コード 0 と結果を判定する。
- `String.split`（100 万部分）と `String.join`（100 万要素）は、`__string_length` `__string_find` `__string_slice` `__string_concat` を定数時間にする門番用 runtime stub で、再帰の深さだけを運転する。いまの runtime は `string_byte_offset` で毎回先頭から走査し、`join` の右結合 `concat` はコピー量が総長の二乗になるので、stub 無しの 100 万部分は実時間で数時間かかる。実時間の線形性は string-split-linear の門番が持つ。
- 25 本の棚卸しで非末尾とした全関数が、上の公開経路のいずれかから、変更前に stack overflow する深さで運転される。公開経路を持たない helper は、その helper を所有する member の門番で同じ深さまで到達させる。
- `filter` の述語が先頭から呼ばれる受理ケースを 1 本足す。

## 由来

`docs/wants.md` 34。§53、§55。
