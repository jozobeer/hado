# prelude の非末尾再帰を末尾形にする（prelude-tail-recursion）

`docs/plan.md` 35 節（v0.5、P1）。着手前の根拠は `docs/issues/prelude-tail-recursion.md`。

## 手順書

### 背景

100 万要素の `Int List` で `List.fold` だけが通り、`map` `filter` `append` `length` `sort` は `hado: stack overflow` になる。`stdlib/` の `def rec` は 25 本あり、§53 の末尾呼び出しの保証の外で書かれている。`map` は先頭から順に適用し、`filter` は末尾側から適用する（効果多相なので観測できる差）。§55 は適用順を書いていない。

### 決定

1. 累積 + `reverse` の末尾形にする。新しい言語構成は足さない（plan 35 節 P1）。`reverse` は `fold` 経由で既に末尾である。
2. 添字が取れる列（Array・Bytes・argv・`read_dir`）は末尾から先頭へ積んで `reverse` を省く（同じ末尾形）。
3. `filter` は先頭から順に適用する（§55 に 1 文足す仕様変更。`map` と `filter` の適用順、`sort` の `odds` → `evens` の順、`merge` の左右の比べる順を書く）。
4. `sort` の比較子の呼び出し列を受理コーパスの golden で固定する。`[3, 1, 4, 1, 5]` のように `odds` と `evens` の両方に要素が残り、`merge` が左右どちらも選ぶ入力にする。
5. 深さの門番は「実際にスタックを尽くす最小の件数」で運転する。100 万件が実時間で重い経路（`String.split` `join`）は、まず実測し、要るときだけ runtime stub（`__string_length` `__string_find` `__string_slice` `__string_concat` を定数時間にする）を足す。stub は深さだけを運転する。

### 成果物

- `stdlib/list.hd` `stdlib/array.hd` `stdlib/bytes.hd` `stdlib/env.hd` `stdlib/io.hd` `stdlib/string.hd`
- `docs/spec.md` §55（適用順の 1 文）
- `tests/accept/` に先頭順の `map` と `filter`（効果付き）を 1 本ずつ
- `scripts/verify-prelude-tail-recursion.sh`（`# tier: core`、`# round: prelude-tail-recursion`）
- 25 本の棚卸し表（このファイルの実測）

### 工程

1. 25 本の `def rec` を末尾・非末尾に分類し、公開経路との対応表を実測に置く（済み）。
2. `stdlib/list.hd` を書き直す（`map` `filter` `length` `append` `odds`/`evens`/`merge`/`sort`、`take_more`）。
3. `docs/spec.md` §55 に適用順を足す。
4. `stdlib/array.hd` `bytes.hd` `env.hd` `io.hd` `string.hd` を書き直す。
5. 深さの門番を足す。直す前の実装で同じ入力が `stack overflow` になることを赤の探針で測る。
6. 先頭順の受理 2 本と、比較子の呼び出し列の golden を足す。
7. 直す前の比較子の呼び出し列を記録し、書き直した実装が同じ列を出すことを門番にする。

### 判定

- 100 万件で `List.map` `filter` `append` `sort` `length` `take`、`Array.to_list`、`Bytes.to_list`、`Env.args`、`Io.read_dir`、`String.split` `join` が終了コード 0 と正しい結果を返す。
- 直す前の実装で同じ入力が `hado: stack overflow`（終了コード 1）になる。
- 比較子の呼び出し列が、直す前の実装の列と `[3, 1, 4, 1, 5]` で一致する。
- `cargo test --offline` が 0 failed、`bash scripts/corpus-lock.sh --check` が差分なし。
- quick 層と core 層が red なし。

### 許容される赤

無し。`docs/spec.md` の適用順の追記は、既存の受理コーパスの `map` `filter` `sort` の golden を変えない範囲にする（変わるなら仕様変更として先に記録する）。

### スコープ外

- TRMC（§53 の保証範囲を広げる規範変更）。
- スタック上限の引き上げ。
- `sort` 以外の比較子の順序の仕様化。

### 中断の条件

- `filter` の適用順を先頭にすると既存の受理コーパスの golden が変わり、その変更が §55 の 1 文で説明できないとき。
- 100 万件の深さの門番が runtime stub 無しで 30 分を超え、stub の設計（`__string_*` の差し替え点）が `docs/cranelift-*.md` と emit の extern 表に収まらないとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §53 | `again` を経由しない末尾呼び出しはスタックを消費しない | prelude | 成功 | `scripts/verify-prelude-tail-recursion.sh` |
| §55 | `map` と `filter` は先頭から順に適用する | prelude | 成功 | `tests/accept/` の効果付き `map`/`filter` |
| §55 | `sort` は `odds` を `evens` より先に整列し、`merge` は左の先頭を比べる | prelude | 成功 | 比較子の呼び出し列 |

## 実測

```
$ grep -c 'def rec' stdlib/*.hd
stdlib/array.hd:4
stdlib/bytes.hd:2
stdlib/env.hd:1
stdlib/error.hd:0
stdlib/float.hd:0
stdlib/int.hd:0
stdlib/io.hd:1
stdlib/list.hd:15
stdlib/option.hd:0
stdlib/string.hd:2
```

25 本の分類（末尾位置に自己呼び出しがあるか）。

| ファイル | 関数 | 形 | 公開経路 |
|---|---|---|---|
| list.hd | map | 非末尾 | `List.map` |
| list.hd | filter | 非末尾 | `List.filter` |
| list.hd | length | 非末尾 | `List.length` |
| list.hd | append | 非末尾 | `List.append` |
| list.hd | odds | 非末尾 | `List.sort` |
| list.hd | evens | 非末尾 | `List.sort` |
| list.hd | merge | 非末尾 | `List.sort` |
| list.hd | sort | 非末尾 | `List.sort` |
| list.hd | take_more | 非末尾 | `List.take` |
| list.hd | fold | 末尾 | `List.fold` |
| list.hd | nth_from | 末尾 | `List.nth` |
| list.hd | nth_more | 末尾 | `List.nth` |
| list.hd | take_from | 末尾 | `List.take` |
| list.hd | drop_from | 末尾 | `List.drop` |
| list.hd | drop_more | 末尾 | `List.drop` |
| array.hd | elements | 非末尾 | `Array.to_list` |
| array.hd | replace_head | 非末尾 | `Array.set` / `replace` |
| array.hd | build | 末尾 | `Array.of_list` |
| array.hd | replace | 末尾 | `Array.set` / `replace` |
| bytes.hd | elements | 非末尾 | `Bytes.to_list` |
| bytes.hd | build | 末尾 | `Bytes.of_list` |
| env.hd | argv_elements | 非末尾 | `Env.args` |
| io.hd | directory_elements | 非末尾 | `Io.read_dir` |
| string.hd | split_from | 非末尾 | `String.split` |
| string.hd | join | 非末尾 | `String.join` |

非末尾は 16 本、末尾は 9 本。非末尾の 16 本すべてに公開経路がある。以降の工程の実測をこの節に足す。
