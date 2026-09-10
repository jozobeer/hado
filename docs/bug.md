# 既知の不具合

修正していない不具合を 1 件 1 節で、検出順に記録する。直したら節を消し、経緯は git 履歴に残す。仕様の側に穴があるものは「仕様の状態」に書く。

---

## B1. 印字の失敗を捨てている（2026-09-09 検出、未修正）

**症状。** `main` の値の書き込みが失敗しても、終了コード 0 を返す。

**再現。**

```
$ hado build hello.hd
$ ./hello
hello
$ ./hello >&-
$ echo $?
0
```

**原因。** `runtime/hado_runtime.c` の出力経路が、どれも戻り値を見ていない。

| 行 | 関数 | 見ていない呼び出し |
|---|---|---|
| 16-17 | `hado_print_int` | `printf` `fflush` |
| 56-58 | `hado_print_string` | `fwrite` `fputc` `fflush` |
| 90-92 | `hado_print_char` | `fwrite` `fputc` `fflush` |
| 96-97 | `hado_print_float` | `printf` `fflush` |
| 133-135 | `hado_print_error` | `fwrite` `fputc` `fflush`（stderr） |

**仕様の状態。** §51 は `main` の値を「stdout に印字して終了コード 0」と定めるが、印字が失敗した場合を定めていない。実装はこの規範に違反しておらず、仕様の穴である。

**影響。** fd が閉じている場合は上のとおり実測で成功を報告する。呼び出し側がこの実行ファイルを使って判断すると、出ていない出力を出たものとして扱う。

リダイレクト先が満杯の場合とパイプの受け手が先に終わる場合は未確認である。`./hello | head -c 0` は終了コード 0 だったが、これは書き込みがパイプのバッファに入って成功したためで、失敗経路を通っていない。

**関連する決定。** `docs/plan.md` 16 節 Q1 で「プログラムの外の環境に依存する操作は Option か Result を返す」と決め、17 節が Result を廃して効果に移した。印字は runtime の側にあって言語の関数ではないが、外の環境に依存する点は同じである。B1 を直すときは §51 に失敗時の規範を足す必要がある。

**未確認。** 失敗時に何を返すべきかは決めていない。終了コードだけを変える案と、stderr に理由を出す案がある。後者は stderr も閉じている場合の扱いが要る。

---

## B4. 入れ子の handler が正しく下がらない（2026-09-10 検出、未修正）

**症状。** `with` を入れ子にすると 3 通りの誤りが出る。1 つ目は合法なプログラムに誤った値を返す。

**再現 1: 別の効果の handler が外側の `abort` を捕まえる。**

```ocaml
effect A
abort a
effect B
abort b
handler A as Outer
abort a => 7
return x => x + 10
handler B as Inner
abort b => 2
return x => x + 100
def f (n: Int): Int {A} => if n > 0 => n | a
def main = with Outer -> with Inner -> f 0
```

出力は `12`。`Inner` は `B` の handler なので `a` は素通りして `Outer` の `abort a => 7` に当たるべきで、`7` が正しい。`Inner` の `abort b => 2` が走り、その結果に `Outer` の `return x => x + 10` が当たっている。`mono.rs` の `$Fail` の tag が対象の効果を区別していない。

**再現 2: handler の腕の中の `with` の `return` が飛ばされる。**

```ocaml
effect A
abort a
effect B
reply b: Int
handler A as Outer
abort a => with Inner -> b
return x => x * 10
handler B as Inner
reply b => 3
return x => x + 2
def f (n: Int): Int {A} => if n > 0 => n | a
def main = with Outer -> f 0
```

出力は `3`。`Inner` の `return x => x + 2` が当たって `5` になるべきである。腕の評価中は `return` の書き換えを抑制する仕組みが、腕の中の別の `with` にまで及んでいる。

**再現 3: `abort` の腕から効果付き関数を呼ぶとビルドできない。**

```ocaml
effect A
abort a
handler A as G
abort a => 99
return x => x + 100
handler A as H
abort a => f 1
return x => x + 10
def f (n: Int): Int {A} => if n > 0 => n | a
def main = with G -> with H -> f 0
```

`check` は `Int` を返して通るが、`build` が `handler arms disagree on the answer type` で失敗する。腕が受け取った tagged 値を処理せずそのまま腕の結果にしている。

**仕様の状態。** §58 はこれらの形を禁じていない。17.5「やらないこと」は「継続を別の handler の下へ運ぶ使い方」を範囲外とするが、これは `capture` 固有で、再現 1 と 3 は継続を伴わない素の `abort` である。**`abort` の入れ子について計画も仕様も沈黙している。**

**範囲。** 19 節の R26 の箇条書きに無い。判定 26-a〜26-f は入れ子を要求せず、全て満たされている。

**直すには。** `$Fail` の tag が効果の同一性を運ぶ必要がある。tag の表現と evidence の受け渡しに関わる設計変更で、葉の修正では届かない。R26 の審査 3 巡で `mono.rs` の handler 下げに毎回新しい欠陥が出たため、指揮者が打ち切りを判断した（`docs/ledger.md` の R26 の節）。
