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

## B5. `capture` を挟む 3 段の入れ子がビルドできない（2026-09-10 検出、未修正）

**症状。** `capture` の handler を別の handler で挟むと、`check` は通るが `build` が
`continuation result type mismatch` で失敗する。誤った値は返さない。

**再現。**

```ocaml
effect A
reply a: Int
effect Y
capture y(value: Int): Unit
effect B
reply b: Int
handler A as Outer
reply a => 1
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
handler B as Inner
reply b => 2
def main = with Outer -> with Collect -> with Inner -> do y (a + b) -> ()
```

```
$ hado build main.hd
continuation result type mismatch
```

**対照（入れ子を除くと通る）。**

```ocaml
effect Y
capture y(value: Int): Unit
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
def main = with Collect -> do y 3 -> ()
```

出力は `[3]`。

**原因。** 継続の構築が、対応する handler ではなく現在の内側 handler に依存している
（`crates/hado/src/mono.rs:1574` 付近）。handler スタックを `Collect` まで巻き戻すと型は揃うが、
`Collect` の上が `Outer` なので `interpret_outcome` が forwarding になり、`Collect` の List を
「success が `yield` の Unit である Outcome」の `$Done` に包んで done payload mismatch になる。

**直すには。** 継続を `Collect` の下で閉じつつ、残りは `Inner` を見たまま、かつ forwarding が
handler の答えをその Outcome へ載せない形が要る。これはヒープのフレーム列と short-cut の畳み方、
および残効果の Outcome 形に関わる。**`docs/plan.md` の 17.6 の未確認 2（`under` フレームの復元）
そのものである。** 計画は「実装方式の問題で、R26 で必要になれば扱う」と保留していた。

**仕様の状態。** §58 はこの形を禁じていない。17.5「やらないこと」の「継続を別の handler の下へ
運ぶ使い方」に当たるかは解釈が割れる。この例は継続を運んでいないが、`capture` の継続が
別の handler の下で構築される点では近い。

**範囲。** 20 節の R28 の箇条書きは `abort` の 3 件（B4）を対象としており、`capture` の入れ子は
含まない。判定 28-a〜28-c は全て満たされている。

**B4 との違い。** B4 の 1 件目は合法なプログラムに**誤った値**を返したが、B5 は**ビルドが失敗する**。
誤った答えを黙って返すことはない。
