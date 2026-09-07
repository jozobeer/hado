# R8〜R9 実装計画（v0.1 の完了に向けて）

R6（2026-09-07）の決定（`docs/plan.0.1.md` 7 節）を実行する。順序は 仕様 → コーパス → 実装。R7（仕様改訂）は commander が実施済みで、`docs/spec.0.1.md` が正本になった（経緯は `docs/ledger.0.1.md` の R6）。

| ラウンド | 成果物 | 書き手 | 審査 |
|---|---|---|---|
| R7 仕様改訂（済） | `docs/spec.0.1.md` `docs/plan.0.1.md` 6〜7 節、`docs/spec-trace.0.1.md`、`docs/corpus.0.1.md` | commander | plan-reviewer（未実施） |
| R8 コーパス | `tests/accept` `tests/reject` `stdlib/` `tests/corpus.sha256` | builder | reviewer（目視）+ 構造検査 |
| R9 実装 | `crates/shin/**` `scripts/` `crates/shin/tests/**` | builder | reviewer + 全門番 |

**R7 終了時点で `verify-m1-corpus.sh` チェック 1（§50 と拒否コーパスの 1 対 1）と、それに依存する `verify-m2.sh` チェック 5、`verify-m3.sh` チェック 9 は既に赤である。R8 の終了時点でさらに増える。** 新形式（`def display | ...`）を parse できず、新しいコードを拒否できず、`-- expect:` 空を通せないため。R8 の合否は門番で測らず、下記「R8 の判定」に限る。門番を緩めて green にすることは禁止。R9 の完了条件が全門番 green である。

---

## R7 の残作業

- plan-reviewer に `docs/spec.0.1.md` の差分を出す。観点: §41 の EBNF から §39 の例と受理コーパスの全構文が導出できるか、§50 の 56 行に段階とアンカーが全て書かれているか、`docs/spec-trace.0.1.md` の各行が実在する節を指すか、7 節の決定が本文に漏れなく反映されているか。
- 指摘は commander が直す。`verify-m1-spec.sh` が `PASSED: 6/6` を保つこと。

---

## R8 コーパス

R7 の仕様を根拠に書き直す。件数: 受理 23 → 27、拒否 46 → 56（E1xx 7 → 14、非 E1xx 39 → 42）。

### 番号の付け直し（§50。ファイル名とヘッダを追随させる）

| 旧 | 新 | 短い名前 |
|---|---|---|
| E203 | E202 | dunder-in-user |
| E204 | E203 | dunder-as-value |
| E205 | E204 | unbound-name |
| E514 | E207 | rec-cycle-value |
| E516 | E208 | missing-rec |
| E515 | E514 | unused-struct-def |
| E601 | E113 | missing-signature |
| E602 | E601 | opaque-type |
| E701 | E307 | poly-recursion |
| E702〜E705 | E701〜E704 | main-polymorphic, main-contains-fn, eq-on-fn, compare-on-fn |

E303 は番号そのまま、短い名前を rail-type に。

### 受理の書き直し

| ファイル | 変更 | 根拠 |
|---|---|---|
| `user_store.shin` | `type Error = \| NotFound` を足し、`struct` 実装を持たせて `main` から呼ぶ | §34, §27（未使用の `module type` は E513） |
| `recursive_type.shin` | 先頭 `\|` を付け、`List2` を `main` で使う | §11, §27 |
| `create_user.shin` | `Cons(head, tail)` → `Cons(_, _)` ×3 | §27 |
| `delete_user.shin` `unit.shin` | `-- expect:` を空に | §51 |
| `display.shin` `name.shin` `record.shin` `literal_pattern.shin` | 第52節の形に | §52 |
| `short_and.shin` | `-- item: 末尾呼び出し` を `tail_self.shin` へ移す | `docs/corpus.0.1.md` |

### 受理の新設（`verify-m6.sh` の生成プローブを移す）

| ファイル | 内容 | ヘッダ |
|---|---|---|
| `division_error.shin` | `def main = 7 / 0` | `-- expect-error: DivisionByZero` |
| `tail_self.shin` | 自己再帰 1 億 | `-- expect: 42`、`-- item: 末尾呼び出し` |
| `tail_mutual.shin` | 引数数の異なる相互再帰 1 億 | `-- expect: 42`、`-- item: 相互再帰` |
| `tail_closure.shin` | 捕捉環境を持つ関数値経由 1 億 | `-- expect: 42` |

### 拒否の書き直し

| ファイル（新番号） | 変更 | 根拠 |
|---|---|---|
| `E508.shin` | `def find_user (id: Int): Result<Int, String> => Ok(id)` を足し、成功枝を `Ok(user + 1)` に。失敗枝はそのまま | 二重欠陥の解消 |
| `E511.shin` | `one` `two` `other` → `"one"` `"two"` `"other"` | 同上 |
| `E509.shin` `E510.shin` | 引数を使う形に（`def f x => if x > 0 => Ok(()) \| Error("neg")`、`def f x => Cons(x, Nil)`） | §27 の引数、§50 の位置順で E505 が先に当たる |
| `E503.shin` `E507.shin` `E512.shin` | 第52節の形に。同一段階の E515 が位置順で先に当たるため | §52 |
| `E302.shin` | 先頭 `\|` を付ける | §11 |
| `E106.shin` | `def main = inc(1)` → `def main = Some 1`。空白は意味を持たず `inc(1)` は正当な並置になった | §8, §41 |
| `E307.shin`（旧 E701） | 内容そのまま。ヘッダの番号だけ | §4 |

E402 E403 E405 は網羅性段階で E515 より先に当たるので変えない。E303 と E207 の引数未使用は、型段階・名前解決段階が冗長段階より先なので変えない。

### 拒否の新設（10 本）

| ファイル | 内容 |
|---|---|
| `E100.shin` | `def main = 1 < 2 < 3`（比較の連鎖） |
| `E108.shin` | `type Size = Large \| Small` |
| `E109.shin` | `def f => 1` |
| `E110.shin` | `def main = "a\q"` |
| `E111.shin` | `def main = ''` |
| `E112.shin` | `def main = 99999999999999999999` |
| `E205.shin` | `def f x => x + 1` を 2 回 |
| `E206.shin` | `main` が無い（`def f x => x + 1` のみ）。引数を持つ `main` は判定器側のプローブ |
| `E515.shin` | `def display user => if user \| Guest => "guest" \| Member(m) => m.name`（`type` 定義付き） |
| `E602.shin` | sig の member を struct が欠く |

### prelude（`stdlib/`）

| 定義 | 変更 | 根拠 |
|---|---|---|
| `List.odds` `evens` `sort` | 未使用の pattern 束縛を `_` に | §27 |
| `List.length` | `fold` + lambda をやめ、第52節の再帰に: `def rec length \| Nil => 0 \| Cons(_, tail) => 1 + length tail` | §27（引数）, §52 |
| `Option.is_some` `Result.is_ok` | 未使用束縛を `_` にし、第52節の形に | §27, §52 |
| `Result.map_error` | `Ok(x) => Ok(x)` は片腕だけの再包装で E507 の対象外（E507 は全腕が再包装のとき）。変更なし。builder は `check_rewrap` の条件で確認する | §33 |

### lock

`tests/corpus.sha256` を新設（`sha256sum tests/accept/*.shin tests/reject/*.shin crates/shin/tests/corpus.rs`）。更新は commander だけが行う。

### R8 の判定（門番は使わない）

- 8-a. 拒否 56 本が §50 の 56 行と 1 対 1（`verify-m1-corpus.sh` チェック 1 の `46` を §50 の行数から導く形に直したうえで実行）。
- 8-b. 全件の `-- spec: §N` が実在する見出しを指し、`-- item:` が `docs/spec-coverage.0.1.md` と文字列一致。
- 8-c. reviewer が全 83 本を読み、「1 ファイル 1 欠陥」（§50 の段階順とアンカー位置順で判定）と「タグを寄せていない」を確認する。
- 8-d. R8 終了時に赤になる門番を台帳に列挙する（見込み: `verify-m1-corpus` `verify-m2` `parse_corpus.rs` の件数、`verify-m3` の accept-20、`verify-m4`〜`m7` の新形式と新コード）。

---

## R9 実装

順序に依存あり。各項目の判定器は「入力を実行時に生成し、期待値を判定器側で計算する」（`docs/corpus.0.1.md`）。`docs/spec-trace.0.1.md` の「固定するもの」列が、各項目の判定器の名前を参照する。

1. **字句と構文**（`lexer.rs` `parser.rs`）: E100（規則に対応しない失敗）、E108、E109、E110〜E112、Char のエスケープ 4 つ、負のリテラルパターン、`def` と `fn` の腕形式、sig の型は本体なし、型の別名と無名 record を E100。E106 を constructor を先頭に置く並置とパイプの右辺の constructor に変え、識別子と `(` の隣接判定（`parser.rs:1112`）を消す。診断コードの付け直し。判定器: §41 の EBNF から乱数でプログラムを生成し、parse → 正規形印字 → 再 parse の往復（`parse_corpus.rs` の式木往復を全文法に広げる）。E106 は乱数の constructor 名と引数で `C x` と `x |> C` を生成し、全件 E106。
2. **検査順**（`lib.rs` `check_source`）: §50 の段階順。同一段階内はアンカー位置順。判定器: 型エラーと冗長欠陥を同居させた `def` を 20 本生成し、全件 E305 か E204。位置順は、同一段階の欠陥 2 つを位置を入れ替えて生成し、先の方が報告されること。`%` を Int 専用に型付けし、Float への適用が E305 になること。
3. **名前解決**（`names.rs` `infer.rs`）: 実装されない `module type` 内の型名も E204。E205（同名宣言。record の field と sig の型変数を含む）。E206（`main` が無い・引数を持つ）。E513 を `type` `module type` `module` に広げ、prelude の免除を E513 に閉じる。E602（sig の不一致 4 種類）。判定器: 乱数の型名で sig を生成、乱数の名前で重複を生成。
4. **網羅性と冗長**（`coverage.rs` `check.rs`）: `let` `let?` のパターンに網羅性（E402）。E505 を pattern 束縛と引数に（束縛を考慮した参照計数）。E515（自由な参照の計数は `names::references` 系を使う）。E304 の 0 件。E303 を `do?` に。判定器: E515 の 4 条件を 1 つずつ外した 4 プローブ、refutable な `let` の生成。
5. **印字**（`mono.rs` の `print_T`、`emit.rs`、`runtime/`）: §51。List、入れ子の String と Char、最上位 Unit、Float。判定器: 型ごとの値を乱数で作り、期待文字列を判定器側で組み立てて実行結果と照合する。
6. **コーパス走行**（`corpus.rs`）: `-- expect:` 空、`-- expect-error:`。
7. **門番**: 件数の直書き（`verify-m1-corpus` 46、`verify-m2` 23/39、`parse_corpus.rs` 7/39、`verify-m3` 20）を §50 の行数とディスクの実数から導く形に。`verify-m3.sh` チェック 10 を `tests/corpus.sha256` との照合に。`verify-m4.sh` `verify-m7.sh` の末尾を `PASSED: n/N` に。`verify-m6.sh` の生成プローブ 4 本を消す。上記 1〜5 の判定器を `scripts/verify-r9.sh` に置く。

R9 の受け入れ条件:

- 9-a. 全門番（m0〜m7、r9）が終了コード 0、`cargo test` 全件通過。
- 9-b. 受理 27 本が期待 stdout / stderr / 終了コードを返す（`verify-m7.sh`）。
- 9-c. 拒否 56 本が指定コードと指定アンカーで落ちる。位置の摂動プローブ（`-- error:` 行の直後に空行）を全コードに当てる。
- 9-d. `tests/corpus.sha256` が一致する（R9 でコーパスを触っていない）。
- 9-e. `docs/spec-trace.0.1.md` の「固定するもの」に書かれた判定器が全て実在する。

---

## スコープ外

`docs/plan.0.1.md` 6 節「v0.2 へ保留」の項目。warning の導入（緩和は「規則から外す」で行う）。

## 分からなかったこと

- E307 の判定（`infer.rs:405-415`）が注釈付きの多相再帰をどこまで捕まえるかは未検証。`def` に型変数を書く構文が無く、プローブが書けなかった。§4 の定義はこの判定を写している。
- prelude が全検査を通ることは、全プログラムへの連結経由で間接的にしか確かめられていない。R9-3 で免除を E513 に閉じたあと、reviewer が免除の範囲を見る。
