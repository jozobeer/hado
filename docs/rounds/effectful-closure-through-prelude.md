# 効果を持つ閉包を多相な prelude 関数に通すと単相化が落ちる（effectful-closure-through-prelude）

## 手順書

### 背景

- 効果を持つ閉包のリストを `List.reverse`・`List.map`・`Array.to_list`・`Array.set` に
  通すと、型検査は通るのに単相化が落ちる（`docs/issues/effectful-closure-through-prelude.md`）。
  契約の member が `Fn(A => Int)` を返す `maker` の形も同じ issue が挙げている。
- `effectful-closure-in-array`（PR #43）の二次レビュー対応で、結果型は「実際の型での
  単一化」から取るようになった。本体は今も `logical`（閉包の結果から `$Outcome` を
  外した型）の引数と `inferred.result` で下ろすので、引数から来た値（実際の型）と
  本体が組み立てる値（論理型）が食い違う。`restored_array_allocation`・
  `restored_success_type` はその食い違いを箇所ごとに埋めている。
- `maker` は `Maker.make` の呼び出しが module 修飾なので、`specialize_function` に
  期待する結果型が渡らず、返す閉包の引数の型が解けない（`unresolved return type`）。

### 決定

issue の「案」の推奨に従う。落ちる箇所ごとに `restored_*` を足す方向は採らない。

1. `specialize_function` が scheme を単一化する期待型を、引数の実際の型から組む。
   引数の型が関数型の位置では、本体が outcome をほどくので、その結果型だけ
   `$Outcome` を外し、引数の型（ドメイン）は実際の型のまま残す。
2. `lower_specialization` は本体をその期待型（および実際の結果型）で下ろす。
   本体の呼び出しと構築が、引数の実際の型と一貫する。
3. `Maker.make` のような module 修飾の呼び出しでも、呼び出し位置の期待結果型を
   `specialize_function` に渡す。

### code-complete の一周目

- 意図: 効果を持つ閉包を prelude の多相関数や契約越しに渡すプログラムを、
  型検査のとおりに実行させる。
- 仮説: 本体を実際の型で下ろせば、引数から来た値と本体の構築が一致する。
  scheme が関数型を書いた引数は、ドメインを実際の型・結果を `$Outcome` を外した
  成功型にすれば単一化が通り、`restore_*` は要らなくなる。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。
- 未確認: 本体を実際の型で下ろすと、効果付きの要素を持つリストを `List.map` が
  たどるとき、callback の呼び出しがどの handler の答えを運ぶか。`restore_*` が
  本当に不要になるか。M2（list literal）・M3（pipe）の穴が同時に消えるかは
  本ラウンドの判定器の外。

### 期待値の根拠

issue の完了条件のとおり。`handler Error as Catch` の下で `8 / 0` の `raise` が
腕に届き、`reverse` は `g 0` の失敗で `0`、`map`・`to_list` は割り切れて
`List.length` が `2`、`set` は `f 5 = 5 * 3 / 5 = 3`、`maker` は `f x = 1`。

### 成果物

`crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。

### 工程

再現 → 赤のテスト（5 入力、maker は期待結果型の経路）→ 期待型の組み直しと本体の
実際の型化 → 欠陥の注入 → 周辺の形の実測。

### 判定

- 5 つの入力が期待値（`reverse` 0、`map` 2、`to_list` 2、`set` 3、`maker` 1）を
  印字して終了コード 0。
- 同じ 5 つが `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。

### 許容される赤

無し。

### スコープ外

- 効果の違う閉包を並べたリストリテラル（`effectful-closure-in-list-literal`）。
- 末尾引数の pipe（`pipe-in-tail-argument`）。
- `cargo test --offline` と層別（quick）はラウンドの最後に 1 回だけ回す。

### 中断の条件

判定が 2 回続けて赤になったら止める。仕様の解釈に疑義が出たら仕様を変えず、
依存しない作業だけ進める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §19, §36 | 効果を持つ閉包を多相な prelude 関数や契約越しに渡しても、失敗が handler へ伝わる | 単相化 | 成功 | `crates/hado/tests/r26_effects.rs` |

## 実測

issue の 5 入力（`handler Error as Catch` の下で `8 / 0` を腕に届かせる）を
`target/scratch/<名>/main.hd` に置いて走らせる。同じ 5 つが
`crates/hado/tests/r26_effects.rs` のテストになっている。

```
$ for d in reverse map to_list set maker; do
>   printf '%s: ' "$d"
>   cargo run --offline -q -- run target/scratch/$d/main.hd
> done
reverse: 0
map: 2
to_list: 2
set: 3
maker: 1
```

直す前の `mono.rs`・`types.rs`（`2e32d3a`）に同じテストを当てると、5 つが赤。

```
$ git checkout 2e32d3a -- crates/hado/src/mono.rs crates/hado/src/types.rs
$ cargo test --offline --test r26_effects
test result: FAILED. 55 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.55s
test array_set_of_effectful_closures_keeps_their_outcome ... FAILED
test array_to_list_of_effectful_closures_keeps_their_outcome ... FAILED
test contract_member_returning_a_closure_over_an_effectful_argument ... FAILED
test map_over_effectful_closures_keeps_their_outcome ... FAILED
test reverse_over_effectful_closures_keeps_their_outcome ... FAILED
```

直した後。

```
$ cargo test --offline --test r26_effects
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.98s
```

全走と、触ったところに効く門番。

```
$ cargo test --offline > /tmp/t.log 2>&1; echo "exit=$?"
exit=0
$ grep -c '^test result: ok' /tmp/t.log
69
$ grep -cE '^test result: FAILED' /tmp/t.log
0
$ bash scripts/verify-m4.sh
reject: 27/27
accept: 111/111
PASSED: 140/140
```

### 入れ子 handler の内側優先（審査 1 回目）

同じ効果を捕まえる handler が入れ子のとき、`answer_for_row`・`callback_answer`・
`specialization_answer` が外側の答えを選んでいた。`handler_for` と同じく内側を
選ぶよう `.rev()` を付ける。`crates/hado/tests/r26_effects.rs` の
`nested_handlers_of_the_same_effect_pick_the_inner_answer`（審査の入力そのまま）
で固定する。

直す前（`.` を `rev` なしに戻す）は赤。

```
$ cargo test --offline --test r26_effects nested_handlers_of_the_same_effect_pick_the_inner_answer
lowered return type differs from inferred signature for file0$apply
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

直した後は緑。内側 `Catch` の答え `Int` が閉包の `$Outcome` に載ることを
`dump-mono` で確かめる。

```
$ cargo test --offline --test r26_effects nested_handlers_of_the_same_effect_pick_the_inner_answer
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
$ cargo run --offline -q -- dump-mono target/scratch/nested/main.hd | grep -o 'Closure(\[Int\], Data("\$Outcome", \[Int, Int, Void\]))' | head -1
Closure([Int], Data("$Outcome", [Int, Int, Void]))
```

入力は `with Outer -> with Catch -> apply fn(x => 8 / x)`、`apply` は `f 1` を
呼ぶ。`8 / 1` は割り切れるので `Catch` の abort は走らず、内側 `Catch` の
return 腕が `8` を返し、外側 `Outer` の return 腕が `"outer"` にして返す。
審査の「`0` を印字」は入力の `x = 1` と `Outer` の return 腕（値を `"outer"` に
する）のため成り立たず、`outer` が正しい出力である。内側優先は、直す前の
`String` を選ぶ型不一致が消えて実行できることと、上の `$Outcome` の答えが
`Int` であることで確かめる。

再測（入れ子優先の後）。

```
$ cargo test --offline --test r26_effects
test result: ok. 61 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.98s
$ for d in reverse map to_list set maker; do cargo run --offline -q -- run target/scratch/$d/main.hd; done
0
2
2
3
1
$ cargo test --offline > /tmp/t.log 2>&1; echo "exit=$?"; grep -c '^test result: ok' /tmp/t.log; grep -cE '^test result: FAILED' /tmp/t.log; grep -oE 'ok\. [0-9]+ passed' /tmp/t.log | grep -oE '[0-9]+' | paste -sd+ | bc
exit=0
69
0
762
$ bash scripts/verify-m4.sh
PASSED: 140/140
```

