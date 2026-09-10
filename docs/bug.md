# 既知の不具合

修正していない不具合を 1 件 1 節で、検出順に記録する。直したら節を消し、経緯は git 履歴に残す。仕様の側に穴があるものは「仕様の状態」に書く。

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
運ぶ使い方」には**当たらない**と解釈を確定した（2026-09-10、`docs/ledger.md` の「バグ修正方針」）。
禁止は継続を第一級の値として持ち出す使い方を指し、この例は `resume` を腕の中で 1 回呼ぶだけである。
合法として直す。プロトタイプ実測を経て R30 で扱う。

**範囲。** 20 節の R28 の箇条書きは `abort` の 3 件（B4）を対象としており、`capture` の入れ子は
含まない。判定 28-a〜28-c は全て満たされている。

**範囲は当初の記録より広い（2026-09-10 追記）。** 上の再現は 3 段だが、実測では **2 段で既に
壊れる**。`capture` handler が内側なら `done payload mismatch`、外側なら
`continuation result type mismatch` になる。測った 15 形のうち通ったのは `capture` handler が
単独の 2 形だけだった。15 形すべて `hado check` は通るので、破れるのは単相化だけである。
詳細は `docs/plan.md` の 17.9、直し方は 22 節（R30）。

**B4 との違い。** B4 の 1 件目は合法なプログラムに**誤った値**を返したが、B5 は**ビルドが失敗する**。
誤った答えを黙って返すことはない。
