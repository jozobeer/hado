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

---

## B7. `do` と末尾以外の位置の `capture` が、残りの計算を捨てて誤答を返す（2026-09-11 検出、未修正）

**症状。** `capture` の操作を `do` の右辺と末尾以外の位置に書くと、`hado check` も `hado build` も
通り、実行すると残りの計算を捨てた値を印字する。実測したのはリストの要素で、引数・被演算子・
record の field は原因からの推定である。

**再現。**

```ocaml
effect Y
capture y(value: Int): Unit
handler Y as Collect
capture y value => let rest = resume () -> [value, ...rest]
return _ => []
def main = with Collect -> [y 1, y 2]
```

```
$ hado build main.hd && ./main
[]
```

§58 の意味論では `[1, 2]`。Koka 3.2.3 も `[1, 2]`（`docs/ledger.md` の「B6 の対応方針」）。

**対照（どちらも正しい）。** 末尾位置 `with Collect -> y 1` は `[1]`。`do y 1 -> y 2` は `[1, 2]`。

**原因。** `operation`（`crates/hado/src/mono.rs:1409` 付近）が `do` 以外の位置の `capture` を
継続なし（`rest = None`）で下ろし、継続が `$Done(())` を即返す。`dump-mono` では `$Yield#1` が
`Cons` の中に作られ、`main` の本体がそれを捨てて `Nil` を返している。末尾位置には「残り」が
無いので、同じ経路で偶然正しい。

**仕様の状態。** §24 は分岐の条件以外の位置に効果を持つ式を置くことを許し、受理コーパスの
`effect_position/` がそれを固定している。ただしそのコーパスは `reply` しか置いていないので、
この形はどの門番にも見えていない。仕様の穴ではなく実装の欠陥である。**受理して誤った答えを
黙って返す**ので、B6 より重い。

---

## B8. Unit 以外を返す `capture` がビルドできない（2026-09-11 検出、未修正）

**症状。** 戻り型が Unit でない `capture` の操作は、`hado check` は通るが `hado build` が失敗する。

**再現。**

```ocaml
effect Ask
capture ask(seed: Int): Int
handler Ask as Twice
capture ask seed => resume (seed * 2)
return x => x
def main = with Twice -> let v = ask 21 -> [v]
```

```
$ hado build main.hd
done payload mismatch
```

§58 の意味論では `[42]`。Koka 3.2.3 も `[42]`。

**原因。** 継続の閉包型が `Unit -> $Outcome` に固定されている（`mono.rs:70` の `outcome_fields`、
`:680` の `continuation_ty`、`continuation_function` の signature）。`resume` に値を渡す経路が無い。
`let` 位置の `capture` は B7 の経路でもあるので、直すには両方が要る。

**仕様の状態。** §57 は `capture` を契約の member と同じ形で書くと定め、戻り型を制限していない。
§58 の例は Unit を返す形だけで、非 Unit の例は仕様にもコーパスにも無い。

---

## B9. payload の型が異なる `capture` 効果の handler を重ねるとビルドできない（2026-09-11 検出、未修正）

**症状。** `String` を運ぶ `capture` の handler と `Int` を運ぶ `capture` の handler を重ねると、
`resume` の中から外へ yield しなくても `hado build` が失敗する。`hado check` は通る。

**再現。**

```ocaml
effect S
capture s(value: String): Unit
effect Y
capture y(value: Int): Unit
handler S as Names
capture s value => let rest = resume () -> [value, ...rest]
return x => x
handler Y as Collect
capture y value => let rest = resume () -> [String.concat "n" (Int.to_string value), ...rest]
return _ => []
def main = with Names -> with Collect -> do s "a" -> do y 1 -> ()
```

```
$ hado build main.hd
continuation result type mismatch
```

§58 の意味論では `["a", "n1"]`。Koka 3.2.3 も同じ。

**原因。** `$Outcome<成功, 答え, payload>` が payload の型を 1 つしか持たない（`mono.rs:66-72` の
`outcome_fields`、`outcome_parts`）。`s "a"` の Outcome は payload が `String`、その継続の本体
`do y 1 -> ()` が作る Outcome は `Int` で、`make_continuation` の型比較（`body.ty() != *outcome`）が
落ちる。R30 が通した合成（判定 30-e〜30-h）は payload がすべて `Int` だったので、この欄が
揃っていた。

**仕様の状態。** §58 は効果ごとに handler を重ねることを許し、payload の型を揃える要求は無い。
