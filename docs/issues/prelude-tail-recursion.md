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

`stdlib/list.hd` の `map` `filter` `append` `length` `sort` の `merge` は素直な再帰で、末尾位置に無い。§53 は末尾呼び出しを保証するが、prelude 自身がその外で書かれている。`def rec` は stdlib 全体で 25 本（`grep -c 'def rec' stdlib/*.hd`）あり、`stdlib/string.hd` の `split_from` `join`、`stdlib/array.hd` と `stdlib/bytes.hd` の `to_list` 系、`stdlib/env.hd` の `argv_elements`、`stdlib/io.hd` の `directory_elements` も同じ性質である。`docs/plan.md` 6 節は v0.1 で「素直な再帰で書く」と決めていた。

## 案

- 推奨: 累積 + `reverse` に書き直す（`reverse` は `fold` 経由で既に末尾）。添字が取れる列（Array・Bytes・argv・read_dir）は末尾から先頭へ累積して `reverse` を省く。新しい言語構成は足さない。`filter` の適用順は変わりうるので、`map` と `filter` の適用順を §55 に書くのは仕様変更として記録する。
- `stdlib/` の 25 本の `def rec` を末尾・非末尾に分類し、非末尾の helper へ到達する公開 member と効果操作を対応付ける。この表をラウンドの手順書に置き、対応の無い非末尾再帰を残さない。
- §55 に「`map` と `filter` は先頭から順に適用し、`sort` は奇数位置（1 番目、3 番目、…）の部分列を偶数位置（2 番目、4 番目、…）の部分列より先に再帰的に整列してから `merge` し、`merge` は左の先頭（奇数位置の側）と右の先頭（偶数位置の側）を比べる順に比較子を呼ぶ」を 1 文足す。`stdlib/list.hd:22-23` の `odds` / `evens` の分割と `:31` の適用の順をそのまま仕様にする（連続した前半と後半ではない）。比較子の呼び出し順は、分割・再帰の順と `merge` の比較の順の両方で決まる。効果を持つ比較子の痕跡がこれで一意になり、連続した前半と後半に分ける実装や、`evens` を先に整列する実装は同じ入力でも別の痕跡を出すので、固定入力の golden が落ちる。`filter` と `sort` の比較子の順序が変わりうるので、ラウンドで仕様変更として記録する。
- 退ける: スタック上限の引き上げ（上限を動かすだけ）。TRMC（§53 の保証範囲を広げる規範変更で、コンパイラ側が大きい）。

## 完了条件

- 100 万要素で `List.map` `filter` `append` `sort` `length`、`List.take`（100 万件取る）、`Array.set`（末尾の index）`to_list`、`Bytes.to_list` が通る。`Env.args` と `Io.read_dir` は変更前に stack overflow する件数の実入力に加え、門番用 runtime stub が 100 万件を返す public path で運転する。門番は終了コード 0 と結果を判定する。
- `String.split`（100 万部分）と `String.join`（100 万要素）は、`__string_length` `__string_find` `__string_slice` `__string_concat` を定数時間にする門番用 runtime stub で、再帰の深さだけを運転する。いまの runtime は `string_byte_offset` で毎回先頭から走査し、`join` の右結合 `concat` はコピー量が総長の二乗になるので、stub 無しの 100 万部分は実時間で数時間かかる。実時間の線形性は string-split-linear の門番が持つ。
- 25 本の棚卸しで非末尾とした全関数が、上の公開経路のいずれかから、変更前に stack overflow する深さで運転される。公開経路を持たない helper は、その helper を所有する member の門番で同じ深さまで到達させる。
- `map` の関数と `filter` の述語が先頭から呼ばれる受理ケースをそれぞれ 1 本足す。末尾から呼んで結果を reverse する実装では通らない。
- `sort` の比較子が呼ばれる順を、変更前の実装が同じ入力で出す引数列としてラウンドファイルに記録し、書き直した実装が同じ列を出すことを門番にする。入力は 5 要素以上で `odds` と `evens` の両方に要素が残り、`merge` が左右どちらを選ぶ比較を両方含む順列（例 `[3, 1, 4, 1, 5]`）に固定し、空列や 1 要素では測らない。`sort` の比較子は `Pure` でない（`stdlib/list.hd:9`）ので、列を保存しない書き直しをするなら比較子を `Pure` に限る仕様変更を別に決めてからにする。

## 由来

`docs/wants.md` 34。§53、§55。
