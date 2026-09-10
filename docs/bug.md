# 既知の不具合

修正していない不具合を 1 件 1 節で、検出順に記録する。直したら節を消し、経緯は git 履歴に残す。仕様の側に穴があるものは「仕様の状態」に書く。

---

## B6. `resume` の中から外側の handler の `capture` を起こせない（2026-09-10 検出、未修正）

**症状。** `capture` の腕で `resume` した継続の中から、**その handler より外側の handler が扱う
`capture`** を起こすと、単相化が拒否する。`hado check` は通る。

**再現。**

```ocaml
effect Z
capture z(value: Int): Unit
effect Y
capture y(value: Int): Unit
handler Z as Outer
capture z value => let rest = resume () -> [value, ...rest]
return _ => []
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
def main = with Outer -> with Collect -> do y 1 -> do z 2 -> ()
```

```
$ hado build main.hd
resume cannot yield a capture handled outside this handler
```

**対照（どちらも通る）。**

- `resume` を経ずに外側の `capture` を起こすだけなら通る。
  `def main = with Outer -> with Collect -> do z 4 -> ()` は `[4]`
- `resume` の中から外側の **`reply`** を起こすのは通る。`reply` は tail-resumptive で、
  単相化の時点で腕がその場に展開され `$Yield` を作らないため。実測は `[9, 1]`

**原因。** `interpret_outcome` が継続を `Closure([Unit], answer)`
（`crates/hado/src/mono.rs:1747-1753` 付近）として射影する。つまり `resume : Unit -> A_H` で、
**答えの型そのものを返す**。再開した計算が yield 地点と `H` の間の handler（内側）だけで
完結するなら連鎖で `A_H` を作れるが、`H` より外側へ yield したら `A_H` の値が存在しない。
`H` 自身への再 yield は `H` の解釈が再帰的に処理するので閉じる。

**直すには。** 継続が Outcome を返す形（`Unit -> Outcome<A_H, ...>`）への一般化が要り、
`capture` の腕の本体が Outcome を扱う形を要求する。R30 のスコープ外とした
（`docs/plan.md` の 22 節）。

**仕様の状態。** §58 はこの形を禁じていない。`hado check` が通ることがそれを示す。
拒否は単相化が返す診断であって、§50 のエラーコードは足していない。**仕様の穴ではなく
実装方式の限界である。**

**R30 以前との違い。** R30 の前は `done payload mismatch` でビルドが失敗していた。R30 の
工程 1 の機構を入れると、この形は `hado check` も `hado build` も通ったうえで**実行時に
SIGILL でクラッシュする**ようになった（終了コード 132、印字なし）。受理してクラッシュするのは
後退なので、判定 30-k を新設して単相化で拒否させた。**誤った答えを黙って返すことはない。**
