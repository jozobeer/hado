# prelude の非末尾再帰を末尾形にする（prelude-tail-recursion）

`docs/plan.md` 35 節（v0.5、P1）。着手前の根拠は `docs/issues/prelude-tail-recursion.md`。

## 手順書

### 背景

100 万要素の `Int List` で `List.fold` だけが通り、`map` `filter` `append` `length` `sort` は `hado: stack overflow` になる。`stdlib/` の `def rec` は 25 本あり、§53 の末尾呼び出しの保証の外で書かれている。`map` は先頭から順に適用し、`filter` は末尾側から適用する（効果多相なので観測できる差）。§55 は適用順を書いていない。

### 決定

1. 累積 + `reverse` の末尾形にする。新しい言語構成は足さない（plan 35 節 P1）。`reverse` は `fold` 経由で既に末尾である。
2. 添字が取れる列（Array・Bytes・argv・`read_dir`）は末尾から先頭へ積んで `reverse` を省く（同じ末尾形）。
3. S1: `map` は関数を、`filter` は述語を、先頭の要素から順に適用する（§55 に 1 文足す仕様変更。`filter` の適用順が変わる）。`sort` の分割と比較の順は書かない。
4. S2: `sort` の比較子の型を `Pure` にする（`Fn(A, A => Int Pure)`）。§36 により効果を持つ比較子は分岐の条件に置けず、`Pure` にしても受理されるプログラムは変わらない（効果付きの比較子は E305 で拒否される。実測）。plan 35 節 P1 の「`odds` を `evens` より先に整列し、`merge` は左の先頭を比べる順に書く」はこれで置き換える。比較子の呼び出し列の golden は足さない。
5. S3: スタック深さの保証は §55 に足さない。§53 の末尾呼び出しの保証で足り、門番が実際にスタックを尽くす深さで測る。
6. 深さの門番はスタック上限を 512 KB に下げて 3 万件を回す。既定の 8 MB では非末尾の形が 15 万件まで粘り、`String.split` の 100 万件は O(n^2) で分単位になる。runtime stub は足さない。`Env.args` は argv の上限（RLIMIT_STACK/4）が深さを決めるので 4 MB と 8 万件、`read_dir` は 512 KB と 3 万件で回す。

### 成果物

- `stdlib/list.hd` `stdlib/array.hd` `stdlib/bytes.hd` `stdlib/env.hd` `stdlib/io.hd` `stdlib/string.hd`
- `docs/spec.md` §55（適用順の 1 文と、`sort` の比較子の `Pure`）
- `tests/accept/` に先頭順の `map` と `filter`（効果付き）を 1 本ずつ
- `scripts/verify-prelude-tail-recursion.sh`（`# tier: core`、`# round: prelude-tail-recursion`）
- 26 本の棚卸し表（このファイルの実測）

### 工程

1. 25 本の `def rec` を末尾・非末尾に分類し、公開経路との対応表を実測に置く。
2. `stdlib/list.hd` を書き直す（`map` `filter` `length` `append` `odds`/`evens`/`merge`/`sort`、`take`）。
3. `docs/spec.md` §55 に適用順と比較子の `Pure` を足す。
4. `stdlib/array.hd` `bytes.hd` `env.hd` `io.hd` `string.hd` を書き直す。
5. 深さの門番を足す。直す前の実装で門番が赤になることを確かめる。
6. 先頭順の受理 2 本を足す。

### 判定

- 100 万件で `List.map` `filter` `append` `sort` `length` `take`、`Array.to_list` `Array.set`、`Bytes.to_list` が終了コード 0 と正しい結果を返す。
- 深さ 3 万件で `String.split` `join` と `Io.read_dir`、4 MB と 8 万件で `Env.args` が終了コード 0 と正しい結果を返す。
- 直す前の実装で同じ入力が `hado: stack overflow`（終了コード 1）になる。
- 効果を持つ比較子が E305 で拒否される。
- `cargo test --offline` が 0 failed、`bash scripts/corpus-lock.sh --check` が差分なし。
- quick 層と core 層が red なし。

### 許容される赤

無し。§55 の適用順の追記は既存の受理コーパスの golden を変えない（`filter` の適用順を固定する golden は無かった）。

### スコープ外

- TRMC（§53 の保証範囲を広げる規範変更）。
- スタック上限の引き上げ。
- `sort` 以外の比較子の順序の仕様化。

### 中断の条件

- `filter` の適用順を先頭にすると既存の受理コーパスの golden が変わり、その変更が §55 の 1 文で説明できないとき。
- 深さの門番が実用の時間で回らないとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §53 | `again` を経由しない末尾呼び出しはスタックを消費しない | prelude | 成功 | `scripts/verify-prelude-tail-recursion.sh` |
| §55 | `map` は関数を、`filter` は述語を、先頭の要素から順に適用する | prelude | 成功 | `tests/accept/list_map_head_order` `tests/accept/list_filter_head_order` |
| §55 | `sort` の比較子の型は `Pure` | prelude | 成功 | `scripts/verify-prelude-tail-recursion.sh` の docs 検査 |

## 実測

着手前の `def rec` の本数と分類。

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

25 本の分類（着手前。末尾位置に自己呼び出しがあるか）。

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

非末尾は 16 本、末尾は 9 本。非末尾の 16 本すべてに公開経路がある。

着手後の `def rec` は 26 本で、すべて末尾位置に自己（または相互）呼び出しを持つ。

| ファイル | 関数 | 形 |
|---|---|---|
| list.hd | `map_go` `filter_go` `filter_keep` `fold` `length_go` `prepend_reversed` `odds_go` `evens_go` `merge` `merge_sort` `nth_from` `nth_more` `take_go` `take_more` `drop_from` `drop_more` | すべて末尾 |
| array.hd | `build` `elements` `replace_go` `replace_keep` | すべて末尾 |
| bytes.hd | `build` `elements` | すべて末尾 |
| env.hd | `argv_elements` | 末尾 |
| io.hd | `directory_elements` | 末尾 |
| string.hd | `split_go` `join_go` | すべて末尾 |

`take` は `take_go`/`take_more` に分かれ、`Array.replace` は `replace_go`/`replace_keep` に分かれたので、本数は 25 から 26 に増えた。

着手前の深さの実測（既定の 8 MB スタック）。

```
$ hado run <非末尾の map を書いた probe>   # n=100000
100000
$ hado run <同じ probe>                    # n=300000
hado: stack overflow                       # 終了コード 1
$ hado run <commas.txt を String.split する probe>   # 100001 件
100001
$ hado run <同じ probe>                    # 160001 件
hado: stack overflow                       # 終了コード 1
```

`ulimit -s` を下げたときの、直す前の実装の限界。

```
512 KB:  非末尾の map        n=10000 通過 / n=15000 stack overflow
512 KB:  直す前の read_dir   n=9000 通過 / n=12000 stack overflow
4 MB:    直す前の args       n=60000 通過 / n=80000 stack overflow
```

`Env.args` は 512 KB では argv の上限（RLIMIT_STACK/4）が先に来て、直す前の実装の限界に届かない（n=9000 で exec が E2BIG）。4 MB と 8 万件なら届く。

着手後の深さの門番。

```
$ bash scripts/verify-prelude-tail-recursion.sh
OK: a depth: non-tail probes overflow a 512 KB stack
OK: b list: 1000000 elements
OK: c sort: 1000000 elements
OK: d array and bytes: 1000000 elements
OK: e string: split and join at the depth
OK: f env and io: args and read_dir at the depth
OK: g control: wrong expectations are rejected
OK: h docs: spec and corpus
PASSED: 8/8
```

同じ門番を、`stdlib/list.hd` を着手前に戻して回すと赤になる（判定「直す前の実装で同じ入力が stack overflow」の実測）。

```
$ git checkout -- stdlib/list.hd && bash scripts/verify-prelude-tail-recursion.sh
OK: a depth: non-tail probes overflow a 512 KB stack
NG: b list: 1000000 elements
NG: c sort: 1000000 elements
NG: d array and bytes: 1000000 elements
NG: e string: split and join at the depth
OK: f env and io: args and read_dir at the depth
OK: g control: wrong expectations are rejected
NG: h docs: spec and corpus
PASSED: 3/8
```

赤の中身は `hado: stack overflow`（`b/map`、`c/sort`、`d/array`、`e/join` の 4 本）。`f` が緑なのは、このとき `stdlib/env.hd` と `stdlib/io.hd` が直った後だったためである（上の 2 行が着手前の実測）。

着手後の 100 万件の実時間（512 KB スタック、`time` の差）。

```
map 133ms / filter 130ms / take 122ms / append 96ms / length 46ms
sort 3114ms / Array.to_list 75ms / Array.set 230ms / Bytes.to_list 113ms
```

`String.split` は O(n^2) なので門番では 3 万件にする（512 KB スタック）。

```
split 30000 件: 2700ms 程度   join 30000 件: 400ms 程度
参考: split 50000 件 7530ms / split 100000 件 30602ms
参考: join  50000 件  966ms / join  100000 件  3987ms
```

効果を持つ比較子は型で拒否される（S2 の実測）。

```
$ hado check <効果付きの cmp を List.sort に渡す probe>
main.hd:9:61: error[E305]: expected Fn(Int, Int => Int), found Fn(Int, Int => Int {Int Yield})
```

`cargo test --offline --no-fail-fast` は 66 本すべて `test result: ok`、rc=0（5 分 44 秒）。`stdlib/string.hd` が `List.reverse` を使うようになったので、`crates/hado/tests/prelude.rs` の自己完結の対象に `stdlib/list.hd` を足した（型宣言は言語が名前で解決する `Exception` と `List` を除く）。

書き直しの途中で E511 に当たった。`Array.replace` の内側の `if | index == 0 => ... | ...` は腕の条件列として読まれ、「全条件が同一変数 `==` リテラル」で拒否される。`replace_go` と `replace_keep` に分け、`index == 0` を関数本体の Boolean branch に置いた。
