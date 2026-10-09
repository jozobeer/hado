# 単相の型の field に載せた効果付き閉包の単相表現（effectful-closure-in-pure-typed-field）

`docs/issues/effectful-closure-in-pure-typed-field.md` の欠陥は、値の作られ方だけを直す 2 つの筋（PR #79 と #80 の `6747eb1`）がどちらも main で正しく走る形を壊したため、着地していない。ここに機構の読みと、決めるべきことと、次の一手を書く。

## 機構（`crates/hado/src/mono.rs` を読んだ結果）

単相化では、効果を運ぶ閉包の型は `Closure(parameters, $Outcome(result, answer, Void))` で表す。`answer` は `handler` の答えの型である。

**リストと `Option` では、この形が既に成立している。** 宣言側の要素型が outcome の閉包のとき、効果を運ばない閉包を `wrap_pure_closure`（1127 行）が結果を `done` で包む閉包に直し、`as_outcome_closure`（1067 行）が値の型を outcome の形へ揃える。`retag_unit_data`（1079 行）が `Data(name, [inner])` の値を同じ `inner` で付け替える。コメント（1062 行）が「直せない不一致はそのまま返し、`cons_list` が止める」と書いているとおり、**揃えられないときは止まる**。

成り立つ理由は、**リストの要素型が文脈から来る**ことにある。要素型を文脈が outcome の形で与えるので、値の側をそれに合わせればよい。

**record はこれと違う。** record の field の型は、record の名指しの型（`type Sh = { f: Fn(Int => Int) }`）から来る。この型は単相で、`answer` を置く場所が無い。だから文脈が outcome を期待していても、宣言側の field の型は純粋な `Closure([Int], Int)` のままである。値の側だけを outcome の形へ直すと、**値の型と宣言側の型が食い違う**。

`6747eb1` はここで `stored_data_type` に field の型を `Data` の引数として載せた。field の型は**値によって変わる**（同じ `Sh` でも、効果を運ぶ腕と運ばない腕で違う）。そのため 1 つのソースの型が 2 つの単相の型（`Closure([Int], $Outcome)` と `Closure([Int], Int)`）に割れ、`mk` の署名と本体が食い違って `lowered return type differs from inferred signature for mk` になった。審査の指摘（`target/reviews/ecptf-review.md`）の核心がこれである。

## 決めること

**`answer` をどこに持たせるか。** 値の作られ方（record の構築・射影）だけでは足りない。審査が挙げた 4 つの形は、いずれも**型が署名・引数・`Map` の型引数を通る**ので、型そのものに `answer` が要る。

- **案 A: 型に `answer` を引数として持たせる。** record の単相の型を `Data(name, [answer, ...])` とし、`answer` は**その場所を囲む handler の答えの型**から一様に決める。値ごとではなく**スコープごと**に決めるのが要点で、`6747eb1` の失敗はここを値ごとにしたことにある。
  - 効くところ: record の構築と射影、その record を取る／返す定義の署名、引数、`Map` の型引数。
  - 効かないところ（今日のまま）: handler の外。`answer` が無いので純粋な形のまま。
  - 費用: 大きい。定義の単相化が `answer` ごとに分かれる必要がある（`def mk = { … }` を handler の中で使うとき、`mk` の本体も outcome の形で組み直す必要がある。これには**引数を取らない定義の単相化**を `answer` で分ける仕組みが要る）。
- **案 B: field の型に「outcome を運ぶ」印を付ける。** `Fn(Int => Int)` と `Fn(Int => Int) !outcome(answer)` を区別し、印を署名・引数・コンテナへ伝える。案 A と同じ伝播が要るうえ、型の表記を増やすので仕様の変更になる。
- **案 C: 純粋な型の field への効果付き閉包の代入を型検査で拒否する。** 一番小さいが、いま受理しているプログラムを拒否するようになる。仕様の変更である。

## 推奨と、次の一手

**案 A を推す。** 印を増やさずに既存の `Data` の引数の仕組みへ乗るので、案 B より小さい。ただし**値ごとではなくスコープごとに決める**ことを守る。

次の一手は、決める前に**確かめる**ことである。順に:

1. **探針 1（機構の確認）。** handler の中の record の構築で、宣言側の field の型が純粋なままで、値だけが outcome の形になることを `dump-mono` で確かめる。
2. **探針 2（スコープごとの一様性）。** 同じ record の型が、同じ handler スコープの中では 1 つの単相の型になり、handler の外では今日の純粋な型になることを確かめる。
3. **探針 3（引数を取らない定義）。** handler の中で `def mk = { … }` を参照したとき、`mk` の本体を outcome の形で組み直せるか。これが案 A の成否を決める。**ここが確かめられなければ案 A は成立しない。**
4. 1〜3 が通れば、型の引数として `answer` を載せる実装に入る。通らなければ、案 C を人へ上げる。

いまの枝（PR #80）の `6747eb1` は**着地させない**。証拠として枝に残し、`docs/issues/` に試した 2 つの筋と退行の実測が入っている。
