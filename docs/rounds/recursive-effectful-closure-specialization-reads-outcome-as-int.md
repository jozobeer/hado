# 再帰する関数が返す効果付き閉包を `$Outcome` のまま素の値として読まない（recursive-effectful-closure-specialization-reads-outcome-as-int）

## 手順書

### 背景

`docs/issues/recursive-effectful-closure-specialization-reads-outcome-as-int.md` の欠陥を直す。効果行を持つ閉包を返す再帰関数を `as f ->` で受けて適用すると、型検査と単相化が通り、実行時に黙って誤った値を返していた。分岐の腕を outcome に揃える `align_closure_outcomes`（PR #73 の `e75a828` が足した）が、まだ lower し終えていない再帰呼び出しの型（効果を落とした論理型）を目標にできるためである。

同じ欠陥は PR #73 のレビューで見つけた。main は同じ入力で `invalid branch types` により停止するので、この変更は「コンパイルできない」を「黙って誤った値を返す」へ変えていた。

### 決定

- 決定 1: 本体を lower している間の再帰呼び出しの結果型を控え、本体を lower し終えたあとの確定した署名と比べる。論理型が一致するのに `$Outcome` の有無だけが食い違うなら、`recursive call to <name> was lowered with ... but the specialization returns ...` で止める。誤った値を出すより、確定できないことを報告する。
- 決定 2: 判定は `carries_outcome`（`$Outcome` を入れ子も含めて持つか）で行う。`logical_argument` は効果を落とすので、落とす前と後で差があるかを見る。
- 決定 3: 相互再帰で成功型が変わる正当な形（`$Outcome[String,..]` と `$Outcome[Int,..]`）は、`$Outcome` の有無が同じなので見逃す。`logical_argument` の一致だけでは足りないので、`$Outcome` の有無を第 2 の条件にする。
- 決定 4: 再帰の結果型を先に確定させてから本体を lower する 2 段階の単相化（issue の案 B）は採らない。効果が大きく、この欠陥の停止で実害が無いためである。
- 決定 5: 早期 return でも控えを残さない（`truncate`）。残すと外側の単相化の検査に混ざる。

### 期待値の根拠

issue の完了条件をそのまま検査にする。「検体が `5` を返すか、止まる（誤った値を返さない）」。したがって門番は「終了コード 0 なら標準出力が期待値ちょうど、停止は許す」を測る。加えて、変異（`carries_outcome` を常に偽にする）で検査 2 が赤になることを測る。

### 成果物

- `crates/hado/src/mono.rs`: `Lowerer.recursive_results`、`recursive_specialization` の控え、`lower_specialization` の判定、`carries_outcome` と `strip_all_outcomes`。
- `crates/hado/tests/r26_effects.rs`: `recursive_closure_result_carrying_outcome_is_not_read_as_a_plain_value`（新設）。
- `scripts/verify-recursive-effectful-closure-specialization-reads-outcome-as-int.sh`（新設、`# tier: core`）。
- 本ファイル。`docs/issues/recursive-effectful-closure-specialization-reads-outcome-as-int.md` の削除。

### 工程

1. Red: 検体が誤った値を返すことを測る（issue の実測を再現）。
2. Green: 再帰呼び出しの結果型を控え、確定した署名と比べて食い違えば止める。
3. 回帰テストと門番を足す。変異で赤になることを確かめる。
4. ラウンドの最後に 1 回だけ `cargo test --offline`・`run-gates.sh --tier quick`・`verify-m0.sh` を回す。
5. 着地の直前に main を取り込み、lock を照合し、門番と quick 層を回す。
6. issue を消し、PR #73 の枝へ push して着地する。

### 判定

1. 検体が誤った値を返さない → 門番の検査 2・3。
2. 上位の `$Outcome` を返す再帰は今までどおり動く → 門番の検査 4。
3. 変異で検査 2 が赤になる → 門番の検査 5。
4. `cargo test --offline` が 0 failed、`run-gates.sh --tier quick` が red なし、`verify-m0.sh` が 8/8。

### 許容される赤

なし。

### スコープ外

- 再帰する効果付き閉包を**正しく動かす**こと。issue の案 B（2 段階の単相化）が要る。このラウンドは止めるまでで、動かすのは別のラウンドにする。
- `pick 1` のように再帰が実行時に届かない形を通すこと。静的な検査なので届かない再帰も止める。main も同じ形を止めるので退行ではない。
- `same_specialization_result` の判定そのもの。`cached_specialization` は確定した署名どうしの比較なので変えない。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- 相互再帰の正当な形が新たに赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §36 | 分岐の両腕は同じ型でなければならず、効果行も揃う | 単相化 | — | `crates/hado/tests/r26_effects.rs` |
| §26 | パターン分岐の腕は上から順に試される | 単相化 | — | 同上 |
| §20 | 効果行は関数の型に載り、`with` が捕まえる | 単相化 | — | 同上 |
| §38 | `with` の腕が効果操作を捕まえる | 単相化 | — | 同上 |

## 実測

### Red（main の現象）

PR #73 の変更（`e75a828`）を当てた木で、検体は誤った値を返す。値は走行ごとに変わる。

```
$ ./target/debug/hado run target/f3/main.hd
138814132035488
$ echo $?
0
```

`dump-mono` では、`file0$pick#Catch` の実際の結果型が `Closure([Int], $Outcome[Int,Int,Void])` であるのに、本体の中の再帰呼び出しの `CallDirect` には `result: Closure([Int], Int)` が付く。

```
$ ./target/debug/hado dump-mono target/f3/main.hd
FunctionId(0), arguments: [Constant(Int(3))], result: Closure([Int], Data("$Outcome", [Int, Int, Void])) }
FunctionId(0), arguments: [Binary { op: Sub, left: Variable { id: 0, ty: Int }, right: Constant(Int(2)) }], result: Closure([Int], Int) }
```

main は同じ入力で停止する。

```
$ ./target/debug/hado run target/f3/main.hd
invalid branch types
$ echo $?
1
```

### Green（このラウンドの成果物）

検体は誤った値を返さず、確定できないことを報告して止まる。

```
$ ./target/debug/hado run target/f3/main.hd
recursive call to file0$pick was lowered with Closure([Int], Int) but the specialization returns Closure([Int], Data("$Outcome", [Int, Int, Void]))
$ echo $?
1
```

上位の `$Outcome` を返す再帰は今までどおり動く。

```
$ ./target/debug/hado run target/rec/main.hd
4
```

門番は 5/5。

```
$ bash scripts/verify-recursive-effectful-closure-specialization-reads-outcome-as-int.sh
OK: 1 cargo-build
OK: 2 recursive-outcome-not-read-as-plain
OK: 3 non-recursive-outcome-not-read-as-plain
OK: 4 top-level-outcome-recursion-still-runs
OK: 5 mutation-turns-check-2-red
PASSED: 5/5
```

### 変異の実測

`carries_outcome` を常に偽にすると、検体はふたたび誤った値を返す。門番の検査 2 が赤くなることを確かめた。

```
$ bash scripts/verify-recursive-effectful-closure-specialization-reads-outcome-as-int.sh
wrong-value stdout=[126205707415456] expected=[5]
OK: 5 mutation-turns-check-2-red
PASSED: 5/5
```

### 相互再帰の切り分け

`r26_effects` の `mutual_recursion_forwards_a_late_raise_across_a_success_type_change` が、`$Outcome` の有無だけを見る前の厳密比較（`active.result != result`）で赤になった。`$Outcome[String,..]` と `$Outcome[Int,..]` は正当な形なので、`$Outcome` の有無を条件に足して通した。81 passed; 0 failed から、回帰テストを足して 82 passed; 0 failed。

```
$ cargo test --offline --test r26_effects
test result: ok. 82 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### ラウンドの最後の全走

```
$ cargo test --offline
CARGO_TEST_RC=0
$ bash scripts/run-gates.sh --tier quick target/.gates-fix73
TALLY: 41/41 passed; red: none
$ bash scripts/verify-m0.sh
PASSED: 8/8
```
