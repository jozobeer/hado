# 型変数の引数に置いた純粋な閉包が単相化で落ちるのを直す（pure-closure-in-type-variable-argument-fails-specialization）

## 手順書

### 背景

`docs/issues/pure-closure-in-type-variable-argument-fails-specialization.md` の退行を直す。PR #75（`434e1fd`）は、型変数の引数に置いた**効果を運ぶ**閉包（`Map.insert` の V、`Option.unwrap_or` の A）が `specialization failed: Mismatch` で落ちる欠陥を直した。そのために `crates/hado/src/mono.rs` の `actual_type` を型引数の位置にも通した。

`actual_type` は `Ty::Fn` の効果行から `$Outcome` を作る。`with` の本体がほかで効果を出さないとき、型検査器は閉包の行にも効果を入れる。ところが `lambda` は本体を lower した結果の型を閉包の署名にするので、純粋な本体（`fn(x => x)`）は純粋なまま下りる。宣言側だけが `$Outcome` を載せ、単一化できない。

main では純粋な形が `102` を返し、効果を運ぶ形が `Mismatch` で落ちる。PR #75 はこの 2 つを入れ替えただけだった。issue の完了条件は**両方を同時に満たす**ことを求める。

### 決定

- 決定 1: `lambda` が、宣言側の行が効果を運ぶときは本体を `done` で包み、宣言側の型に揃える。`actual_type` が型引数の位置に作る `$Outcome` と単一化できるようになる。
- 決定 2: 宣言側の結果型は `self.actual_type(&resolved)` から取る。`Type::try_from` は行を落とすので、`$Outcome` の有無が分からない。
- 決定 3: `actual_type` を型引数の位置に通す PR #75 の変更は残す。止めるのは「宣言側と本体の型を揃える」側で行う。`actual_type` を戻すと、効果を運ぶ閉包の本体と型が合わなくなる。
- 決定 4: `scheme_takes_effectful_callback`（scheme が `Fn` と書いた位置だけを callback と見なす）も残す。`Map.insert` の V は呼ばれないので、callee の効果行に載せない。
- 決定 5: 本体が既に `$Outcome` へ落ちているときは何もしない。二重に包まない。

### 期待値の根拠

issue の完了条件をそのまま検査にする。4 つの検体（純粋/効果 × `Map.insert`/`Option.unwrap_or`）が期待値ちょうどを返すこと。生の `Int` の `main` は値が終了コードになる。

### 成果物

- `crates/hado/src/mono.rs`: `lambda` の `declared_result` と、本体を `done` で包む分岐。
- `scripts/verify-pure-closure-in-type-variable-argument-fails-specialization.sh`（新設、`# tier: core`）。
- 本ファイル。`docs/issues/pure-closure-in-type-variable-argument-fails-specialization.md` の削除。

### 工程

1. Red: main の 4 形を測る（純粋は通り、効果は落ちる）。PR #75 の木で 2 つが入れ替わることを測る。
2. Green: `lambda` が宣言側の outcome に本体を揃える。
3. 門番を足し、変異で赤になることを確かめる。
4. ラウンドの最後に 1 回だけ `cargo test --offline`・`run-gates.sh --tier quick`・`verify-m0.sh` を回す。
5. 着地の直前に main を取り込み、lock を照合し、門番と quick 層を回す。
6. issue を消し、PR #75 の枝へ push して着地する。

### 判定

1. 純粋な閉包を `Map.insert` の V に置く → `102`（門番の検査 2）。
2. 純粋な閉包を `Option.unwrap_or` の A に置く → `102`（検査 3）。
3. 効果を運ぶ閉包を `Map.insert` の V に置く → `4`（検査 4）。
4. 効果を運ぶ閉包を `Option.unwrap_or` の A に置く → `0`（検査 5）。
5. 変異（本体を宣言側の outcome に揃える分岐を殺す）で検査 2 が赤になる（検査 6）。
6. `cargo test --offline` が 0 failed、`run-gates.sh --tier quick` が red なし、`verify-m0.sh` が 8/8。

### 許容される赤

なし。

### スコープ外

- 型検査器の推論した行そのもの（`with` の本体がほかで効果を出さないと閉包の行に効果が入る）を直すこと。issue の案 B。影響が広いので採らない。
- `actual_type` を型引数の位置に通すことの是非。PR #75 の変更を残したうえで、本体側を揃える。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- `cargo test --offline` の全走が赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §20 | 効果行は関数の型に載り、`with` が捕まえる | 単相化 | — | `crates/hado/tests/r26_effects.rs` |
| §38 | `with` の腕が効果操作を捕まえる | 単相化 | — | 同上 |
| §41 | 型の正準形 | 単相化 | — | 同上 |
| §46 | 契約の型引数 | 単相化 | — | 同上 |
| §47 | 実装の局所型 | 単相化 | — | 同上 |

## 実測

### Red（main と PR #75 の現象）

main では純粋な形が通り、効果を運ぶ形が落ちる。

```
$ ./target/debug/hado run target/t75/pure-map/main.hd
$ echo $?
102
$ ./target/debug/hado run target/t75/eff-map/main.hd
specialization failed: Mismatch
$ echo $?
1
```

PR #75 の木では 2 つが入れ替わる。

```
$ ./target/debug/hado run target/t75/pure-map/main.hd
specialization failed: Mismatch
$ echo $?
1
$ ./target/debug/hado run target/t75/eff-map/main.hd
$ echo $?
4
```

### Green（このラウンドの成果物）

4 形とも期待値ちょうどになる。

```
$ ./target/debug/hado run target/t75/pure-map/main.hd
$ echo $?
102
$ ./target/debug/hado run target/t75/pure-opt/main.hd
$ echo $?
102
$ ./target/debug/hado run target/t75/eff-map/main.hd
$ echo $?
4
$ ./target/debug/hado run target/t75/eff-opt/main.hd
$ echo $?
0
```

門番は 6/6。

```
$ bash scripts/verify-pure-closure-in-type-variable-argument-fails-specialization.sh
OK: 1 cargo-build
OK: 2 pure-closure-in-map-value
OK: 3 pure-closure-in-unwrap-or
OK: 4 effectful-closure-in-map-value
OK: 5 effectful-closure-in-unwrap-or
OK: 6 mutation-turns-check-2-red
PASSED: 6/6
```

### 変異の実測

本体を宣言側の outcome に揃える分岐を殺すと、検査 2 が赤くなる。

```
$ bash scripts/verify-pure-closure-in-type-variable-argument-fails-specialization.sh
exit=1 expected=102 stderr=[warning: variable does not need to be mutable]
OK: 6 mutation-turns-check-2-red
PASSED: 6/6
```

### ラウンドの最後の全走

```
$ cargo test --offline
CARGO_TEST_RC=0                 # 73 suite ok, 0 failed
$ cargo test --offline --test r26_effects
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ bash scripts/run-gates.sh --tier quick target/.gates-pure-closure
TALLY: 41/41 passed; red: none
$ bash scripts/verify-m0.sh
PASSED: 8/8
```

### main から続く赤（このラウンドで増えていない）

`lint-docs.sh` は 14/16。main も 14/16 で、赤は同じ 2 つである。

```
$ bash scripts/lint-docs.sh
NG: each issue cause points at a path and an identifier
NG: each issue document has the required name, priority and sections
```

- 前者は `docs/issues/recursive-effectful-closure-specialization-reads-outcome-as-int.md` の `原因` が `same_specialization_result` を指すため。この識別子を入れる修正は #73 の枝にあり、まだ着地していない。#73 が着地すると、この issue は消える。
- 後者は PR #77 が入れた 3 件の issue の 2 行目が空行になっているため。同じく #73 の枝で直してある。

どちらもこのラウンドの成果物ではない。`main` を取り込んだあとで作り直し、16/16 になったことを確かめてから着地する。
