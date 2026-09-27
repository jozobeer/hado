# 組み込み `compare` の上に連想配列 `Map` を置く（map-by-builtin-compare）

## 手順書

### 背景

単語頻度の CLI で連想リストを自分で書いた（`Entry List` と `bump`）。20 万語 × 語彙 500 で線形探索になり、語彙 500 の最悪探索が 1.77 s（issue の表）。`docs/wants.md` 33 は association list を想定しているが、typeclass も演算子過負荷も無い（§4）ので、比較を誰が持つかが決まっていなかった。組み込みの `compare` は単相化時にキー型ごとに派生され（`crates/hado/src/mono.rs` の `derive_comparison`）、モジュールの型変数の上で呼べる（issue の実測）。prelude の契約 `Map` をその上に置く。

### 意図

- 使う人: キーで値を引くデータ構造を自分で書いている hado の書き手（推定）。
- 場面: 語の頻度、名前から値への対応など、キーが多数あり探索が何度も起きるとき。
- 理由: 連想リストの線形探索を、キー型ごとの派生 `compare` を使う平衡木に置き換え、利用者が比較関数を渡さずに済むようにする。

### 仮説（一周目のモデル化）

- 境界: `Map` は prelude の契約（`module Map` + `implement Map as Map`）。実体の ADT と補助 `def` は `implement Map as Map` の中だけに置き、constructor を外へ出さない。公開する member は `empty` `insert` `find` `remove` `fold` `to_list` の 6 つ。`to_list` の要素型は外で宣言した `(K, V) MapEntry`。
- 型: キー型 `K` と値型 `V` は契約の抽象型 `(K, V) T` の型変数。比較は `compare` を内部で呼ぶ。`find` は `V Option`、`remove` は同型を返す。`fold` は `Fn(A, K, V => A)` を取る。
- データフロー: 実体は左傾赤黒木（LLRB）。木の高さ `h` は `h <= 2 * floor(log2(n + 1))` で、1 段につき `compare` を 1 回だけ呼ぶ（`<` と `==` の 2 回に分けない）。よって 1 回の `insert` `find` `remove` の primitive `hado_string_compare` 呼び出しは `2h <= 4 * floor(log2(n + 1))` に収まる。
- `fold` と `to_list` は木を帰りがけ順（左 → 節 → 右）に辿り、キーの昇順で累算器を呼ぶ。
- 未確認（着手時に決めていなかった点）:
  - `fold` の効果行。§55 の「`Fn` を引数に取る member は効果多相なので効果を書かない」に合わせて効果を書かなかったが、署名が抽象型 `(K, V) T` を含む member は効果行が一般化せず、効果を持つ累算器を渡すと `E706` になる（`## 実測` の「効果行」を参照）。判定器は効果の代わりに、非可換の値の累算器を順序の証拠にする。
  - 計測する最大の `n`。コンパイラは 4095 件のリスト literal でスタックを溢れさせる（`## 実測`）。上限を超える大きさは 1023 で足りるのでそこで測る。

### 期待値の根拠（判定器）

- issue の完了条件をそのまま検査にする。件数・期待値は門番側で計算し、コーパスのファイル名や行を埋め込まない。
- `fold` の昇順は、区切り `<` を挟む非可換の String 累算器で訪問順を出し、その列を `IoDefault.log` で効果として出して、判定器側が `compare` で独立に並べた鍵の列（String の `hado_string_compare` と同じ規則）と照合する。区切りのある文字列は戻り値を `reverse` しても昇順に戻らないので、降順に呼んで結果を反転する実装は落ちる。挿入は昇順でない 3 件以上。`to_list` だけの照合では通らない。
- `compare` の呼び出し回数はテストビルドの計測用 stub（`hado_string_compare` を数える）で測る。上限は判定器が各 `n` について `4 * floor(log2(n + 1))` を独立に計算する。鍵は String の `compare` 順で単調な昇順鍵（同じ桁に 0 埋め）とし、素の二分探索木の最後の `insert` の primitive 回数が上限を超える大きさを含める。最後の `insert` と、同じ `n` の `find`/`remove` で増分 0 なら落とす。
- 単語頻度は、判定器が入力を空白区切りで独立に数えた語 → 回数と、両プログラムの出力を同じ対応に正規化して照合する。速さは頻度の更新を除く対照プログラムの中央値を引いた時間で比べる。

### 成果物

- `stdlib/map.hd`（新設）と `crates/hado/src/lib.rs` の prelude 連結。
- `docs/spec.md` §55 の `Map` の member の型・失敗の表・昇順の規定。
- `tests/accept/map_*` — 受理と `// expect:`（`// needs: prelude`）。
- `scripts/verify-map-by-builtin-compare.sh`（`# tier: full`、`# round: map-by-builtin-compare`）、`scripts/gate-tiers.lock`。
- `tools/wordfreq/`（連想リスト版・`Map` 版・対照・入力生成の手順）。
- `docs/rounds/map-by-builtin-compare.md`。

### 工程

1. 空の写像と不在の `find` / `remove` を測る門番を書き、変更前の実装で赤を測る。
2. `stdlib/map.hd` の LLRB を実装し、`compare` 回数と `fold` の順を測る。
3. §55 と受理コーパスを足す。
4. 単語頻度の CLI 2 本と対照、入力生成をリポジトリに入れ、独立照合と速さを測る。
5. main を取り込み、`corpus-lock.sh` を作り直し、ラウンドの最後の全走を回す。

### 判定

issue の完了条件ごとの検査（門番のどの検査が測るか）。

1. §55 の member の型と昇順 → 検査 g。
2. 空・不在で `find` が `None`、`remove` が no-op → 検査 b。
3. `fold` の呼び出し順 → 検査 c。
4. constructor の E204 と `compare` 回数の上限 → 検査 a・d・e。
5. 単語頻度 2 本と対照、独立照合、速さ → 検査 f・h。
6. 重複鍵 `-0.0`/NaN の畳み → 検査 i。

着地の判定は次とする。

- `bash scripts/verify-map-by-builtin-compare.sh` が `PASSED: n/n`。
- `cargo test --offline` が赤なし。
- `bash scripts/run-gates.sh --tier quick target/gates-quick` が赤なし。
- `bash scripts/lint-docs.sh` と `bash scripts/verify-spec-contracts.sh` が赤なし。

### 許容される赤

- 検査 h（速さ）は対照を引いた差の中央値で `map_extra < bump_extra`（比が 1 より大きい）を判定する。赤になるのは、`Map` 版が `bump` 版より速くないとき、または片方の差が 0 以下で測れていないとき。負荷で偽赤を出さない形は保つ（warm-up 1 回 + 5 組、対照減算、組ごとの差の中央値）が、判定そのものは外さない（`docs/issues/flaky-string-split-timing-gate.md` の「中央値で判定し、閾値は据え置く。退行を測る力は落とさない」に従う）。

### スコープ外

- 独自の順序が要る functor `MakeMap(Key: Ord)`（§48）。いまは作らない。
- record の field 更新、参照カウント。
- `Map` 以外の新しい prelude member。
- 抽象型を含む契約 member の効果行の一般化（コンパイラ側）。`docs/issues/contract-member-effect-generalization.md` に切り出した。

### 中断の条件

- `compare` が `implement` の中の型変数 `K` の上で呼べず、単相化が失敗したとき。
- LLRB の削除の比較回数が `4 * floor(log2(n + 1))` に構造的に収まらないとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §55 | `Map.find` は `V Option`、`fold` と `to_list` はキーの昇順 | 型 | — | `scripts/verify-map-by-builtin-compare.sh` 検査 g |
| §55 | `Map` の member の型（`(K, V) T` と関数の型） | 型 | — | 検査 g、`tests/accept/map_*` |
| §21 | 等価な鍵（`compare` が 0）は 1 つに畳む。`-0.0`/`+0.0`、NaN 同士は同順位 | 実行 | — | 検査 i |
| §47 | 実装の内部 ADT と constructor は外から E204 | 名前解決 | E204 | 検査 a |

## 実測

### 赤（`Map` を prelude に繋ぐ前）

`crates/hado/src/lib.rs` の `stdlib/map.hd` の 2 行を外して `cargo build --offline -q --bin hado` し、門番を回した。`Map` が名前解決できないので 9 件のうち 8 件が赤になる（検査 a は `stdlib/map.hd` のソースを直接読むため緑のまま）。

```
$ bash scripts/verify-map-by-builtin-compare.sh
NG: b empty: find is None and remove is a no-op on the empty map and a missing key
Traceback (most recent call last):
  ...
AssertionError: ('order', 'main.hd:1:18: error[E204]: unbound name Map\n')
...
PASSED: 1/9
```

### 緑（このラウンドの成果物）

```
$ bash scripts/verify-map-by-builtin-compare.sh
OK: a surface: internal constructors and defs are unreachable (E204)
OK: b empty: find is None and remove is a no-op on the empty map and a missing key
OK: c fold: the accumulator visits keys in compare order
ascending n=255: last insert 14, find 16, remove 16 (limit 32; plain BST last insert 508)
ascending n=1023: last insert 18, find 20, remove 20 (limit 40; plain BST last insert 2044)
descending n=1023: last insert 18, find 11, remove 11 (limit 40; plain BST last insert 1022)
random n=1023: last insert 16, find 16, remove 20 (limit 40; plain BST last insert 17)
OK: d counts: derived compare stays under 4 * floor(log2(n + 1)) primitives
OK: e order: insert and remove through the public members keep compare order
OK: f frequencies: both CLIs match an independent word count
OK: g spec: §55 types the Map members and fold/to_list are ascending
OK: h timing: the control-subtracted time of the Map CLI
OK: i duplicates: equivalent keys fold to one entry with the later value
PASSED: 9/9
```

検査 d の数（`hado_string_compare` の呼び出し回数）:

| 挿入の順 | `n` | 最後の `insert` | `find` | `remove` | 上限 `4 * floor(log2(n + 1))` | 素の二分探索木の最後の `insert` |
|---|---|---|---|---|---|---|
| 昇順 | 255 | 14 | 16 | 16 | 32 | 508 |
| 昇順 | 1023 | 18 | 20 | 20 | 40 | 2044 |
| 降順 | 1023 | 18 | 11 | 11 | 40 | 1022 |
| ランダム | 1023 | 16 | 16 | 20 | 40 | 17 |

素の二分探索木の値は判定器が同じ primitive の数え方（`<` は各節で 1、`<` が偽のとき `==` が 1）で独立に計算する。昇順と降順の最後の `insert` が上限を大きく超えるので、平衡木でなければこの大きさで赤になる。

検査 h（対照を引いた差の中央値。`map_extra < bump_extra` を門番が assert する）:

```
bump: total 2.036s control 0.034s subtracted 2.002s
map:  total 0.247s control 0.034s subtracted 0.215s
ratio bump/map of the subtracted time: 9.31
```

赤の証拠: `bump` 版と `Map` 版の実体を入れ替える一時探針（木には残していない）で回すと、判定が落ちる。

```
bump: total 0.270s control 0.036s subtracted 0.234s
map:  total 2.154s control 0.036s subtracted 2.118s
AssertionError: the Map CLI is not faster than the association list: bump 0.234s, map 2.118s, ratio 0.11
```

### 効果行（`fold` の効果多相が効かないこと）

署名が抽象型 `(K, V) T` を含む契約 member は、効果行が一般化しない。`stdlib/map.hd` の `fold` に効果を持つ累算器を渡すと `main` で `E706` になる。`check --print-type` だけはこの欠陥を見逃す（`build` と `check` は赤、`--print-type` は緑）ので、再現は `build` で取る。

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
$ cargo run --offline -q -- check --print-type main.hd
String List {Yield}
```

同じ形の `fold` でも、契約に抽象型を置かず型を top level で宣言すると効果行は一般化する。`List.fold` が効果多相なのはこの形だからで、抽象型を隠す `Map` では成り立たない。

```
$ cargo run --offline -q -- build -o prog list_fold.hd && ./prog
["1", "2", "3"]
```

判定器の検査 c はこの制限を避けて、非可換の String 累算器を順序の証拠にしている。§55 の「`Fn` を引数に取る member は効果多相」という規則との差なので、仕様を変えずに `docs/issues/contract-member-effect-generalization.md` に切り出した。

### 単語頻度の入力

`tools/wordfreq/generate-input.sh` は 200000 語・相異なる語 500（各 1 回以上）を決定的に生成する。門番は 2 回生成してバイト一致を確かめ、入れた digest と照合する。手元の値は `2cc095b8607dbec3f54e15a99105febd1a645ac706944d2b764d78ad747ecd95`。

```
$ bash tools/wordfreq/generate-input.sh words.txt && sha256sum words.txt
2cc095b8607dbec3f54e15a99105febd1a645ac706944d2b764d78ad747ecd95  words.txt
```

### 内部型の名前が root の名前空間に出る

`implement Map as Map` の中の `type` は、root の名前空間にそのまま出る。`type Color = | Red | Black` のような普通の名前を置くと、利用者が同じ名前の型を宣言したときに E205 になる。`cargo test --offline` の `check_adts::nullary_constructor_patterns_constrain_the_scrutinee_type` が `type Color = | Red | Blue` を使うので、この衝突が実際に赤になった。

```
$ cargo test --offline
---- nullary_constructor_patterns_constrain_the_scrutinee_type stdout ----
thread 'nullary_constructor_patterns_constrain_the_scrutinee_type' panicked at crates/hado/tests/check_adts.rs:96:5:
assertion `left == right` failed
  left: Some("E205")
 right: Some("E305")
```

内部型と constructor に `Map` の前置きを付けて直した（`MapColor` `MapRed` `MapBlack` `MapLeaf` `MapNode` `MapStep` `MapUnchanged` `MapDeleted`）。門番の検査 a が、実装の中の内部型の名前が `Map` で始まることを見るようにして、この種類の後戻りを止める。赤の証拠は、前置きを外して検査 a を回すと `internal type name Black is not prefixed with Map` で落ちること。

`MapEntry` は外に出す型なので前置きをせず、root の名前空間で一意な名前にしてある。

### コンパイラの限界（未確認）

計測する最大の `n` を 4095 にすると `hado build` が「thread 'main' has overflowed its stack」で落ちる（4095 件のリスト literal）。上限を超える大きさは 1023 で足りる（素の二分探索木で 2044 > 40）ので、門番は 1023 までを測る。より大きい `n` を測るには、literal ではなく再帰で鍵の列を組む門番側の工夫か、コンパイラのスタック使用の見直しが要る。

### F1（postfix-binding）の取り込み後

main（`e0a06a7`）を取り込み、旧構文が E100 になったので成果物を後置へ写した。`docs/spec.md` は自分の §55 の `Map` の節と main の `Char` の閉じが同じ場所で衝突したので、両方を残して後置の綴り（`def 名前 (引数)` と `/module`）で書いた。`tests/corpus.sha256` は main 側を採り、写しの後に作り直した。

写したのは 10 本（`stdlib/map.hd`、受理コーパス 6 本、`tools/wordfreq/` の `bump.hd` `control.hd` `map.hd`）と、門番に埋め込んだ入力と §55 を読む正規表現。正準形は `hado fmt` で作った（手で整えた行は `postfix_close::corpus_is_canonical_and_round_trips` に赤くされた）。

```
$ for d in tests/accept/map_*; do cargo run --offline -q -- run $d/main.hd; done
none:2
1,pos,pos,-0.0,none,0
1,second,0
none:0
[a][b][c]
0001,0002,0003
```

6 本とも `// expect:` と一致する。wordfreq の 3 本は 200000 語の入力で頻度表が一致する（`bump==map True bump==ind True map==ind True entries 500 sum 200000`）。

```
$ cargo test --offline
（全 suite とも 0 failed。passed=776 failed=0）
$ bash scripts/verify-map-by-builtin-compare.sh
PASSED: 9/9
$ bash scripts/run-gates.sh --tier quick target/gates-quick2
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates-core4
TALLY: 709/709 passed; red: none
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
$ bash scripts/verify-m1-corpus.sh
PASSED: 7/7
```

`bash scripts/corpus-lock.sh --check` の差分は「main 側の変更」と「このラウンドの成果物」だけで説明できたので作り直した（248 行）。`bash scripts/gate-tiers-lock.sh --check` は差分なし。

#### 正準形は `print-postfix` が作る（`fmt` ではない）

`postfix_close::corpus_is_canonical_and_round_trips` は `parser::canonical_program` の印字と入力が一致することを求める。`fmt` は `fixes::format` を通る別の経路で、この一致を作らない。手で整えた行は赤くなり、`print-postfix` の出力（`def main =` の行に最初の束縛を続け、パターンの腕を 1 行に畳む）が正準形だった。

```
$ cargo test --offline --test postfix_close
test corpus_is_canonical_and_round_trips ... FAILED
assertion `left == right` failed: tests/accept/map_absent_find_remove/main.hd: 正準形ではない
  left: "def main =\n  Map.insert ... as m ->\n  ..."
 right: "def main = Map.insert ... as m ->\n  ..."
$ cargo run --offline -q -- print-postfix <根> <出力先>   # この出力を採る
$ cargo test --offline --test postfix_close
test result: ok. 6 passed; 0 failed
```

`print-postfix` はコメントを落とす（`stdlib/map.hd` で 18 個）。写した後に、コメント塊を元の錨（次に現れる定義）の位置へ戻した。18 個とも残っていることを確かめてある。
