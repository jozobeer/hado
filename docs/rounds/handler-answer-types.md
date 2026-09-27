# 同じ handler を答え型の違う複数の `with` に使うと単相化が落ちる（handler-answer-types）

## 手順書

### 背景

- 1 つの関数の本体で、同じ handler を答え型の違う 2 つの `with` に使うと、
  `check` は通るのに `build` と `run` が内部エラーで止まる
  （`docs/issues/handler-answer-types.md`）。prelude の `Error` だけで再現する。
- 止まる場所は `crates/hado/src/mono.rs` の `interpret_outcome`
  （`handler arms disagree on the answer type`）と `done`
  （`done payload mismatch`）である。issue の「原因」は「単相化が handler の腕を
  答え型ごとに具体化していないとみられるが、未確認」としていた。
- 原因は単相化ではなく checker の表にあった。checker は handler の腕と `with` の
  本体を `with` ごとに推論し（`infer::with_handler` → `handler_answer` →
  `infer_handler_arms`）、節点の型を span 単位の `value_types` /
  `composite_types` に書く。同じ節点が答え型の違う複数の `with` の下に現れると、
  表は最後に推論した `with` のもので上書きされる。mono は腕を lower するとき
  `None` のような答え型だけに依存する節点をその表から引くので、別の `with` の
  答え型になり、上の 2 つで止まる。
- `Some(value)` は引数の型から結果型を組み直す（`constructed_type`）ので助かる。
  助からないのは引数を持たない `None` と、`with` を `fn` やリストの要素に置いた形。

### 決定

issue の「案」の推奨（単相化で腕を `with` ごとの答え型で具体化する）を、
checker が既に持っている `with` ごとの推論結果を捨てない形で実装する。退ける案
（型検査で 2 つ目の答え型を拒否する）は §38 が答え型を制限しないので採らない。

1. checker の `with_handler` は、本体と腕を推論している間に出た節点の型を
   `crate::WithTypes` として `with` の位置ごとに控える（`note_type`）。
   span 単位の表は従来どおり書く（他の利用者がある）。module の実装の本体で
   推論した分は、span 単位の表と同じ規則で局所型名を修飾する。
2. mono の `with_handler` は、その `with` を lower する間だけ控えた型を
   span 単位の表へ差し替え、lower し終えたら元に戻す（`install_with_types` /
   `restore_with_types`）。多相関数を呼び出しごとの型で具体化するのと同じ扱いに
   なる。
3. `answer_for_row`・`callback_answer`・`specialization_answer` の
   `.enumerate().rev()`（M1 の内側優先）は触らない。`handler_answers` の積み方も
   変えない。

### code-complete の一周目

- 意図: hado で効果の handler を書く人が、1 つの本体で同じ handler を答え型の
  違う複数の `with` に使えるようにする。§38 は答え型を制限しておらず、
  `do-option-absent`（M7）が前提にする「`None` へ戻す handler を多くの型に
  使う」形が今は書けない。
- 仮説: 停止は、checker が `with` ごとに推論した腕の型を span 単位の 1 つの表へ
  上書きし、mono が最後の `with` の型で腕を lower するため。checker が `with`
  ごとの型を控え、mono がその `with` の間だけ差し替えれば、腕が `with` ごとの
  答え型で具体化される。
- 判定器: 下の「判定」。issue の完了条件をそのまま使う。
- スコープ: `crates/hado/src/mono.rs`・`crates/hado/src/infer.rs`・
  `crates/hado/src/lib.rs`・`crates/hado/tests/r26_effects.rs`・本ファイル。
- 未確認: 控える単位は `with` の本体まで要るか腕だけで足りるか。module 実装の
  中の `with` で型の修飾が要るか。`again` handler・handler の値引数で差し替えが
  副作用を持たないか。stale の一般形が `None` 以外にもあるか。

### 期待値の根拠

issue の完了条件のとおり。

- `answer-two` は同じ `ToOption`（`Error` を `Option` にする）を `Int` と
  `String` の 2 つの答え型で使う。`Int.parse "1"` は `Some(1)`、`String.slice
  "ab" 0 1` は `Some("a")`。
- 入れ子は `Absent` の `Maybe` と `Error` の `ToOption` を `risky` の本体
  （`n == 0` で `absent`、そうでなければ `raise`）に重ねる。4 つの形を単独で
  動かすと `None`・`Some(None)`・`Some(None)`・`None` になる。これを判定器の
  独立の表として持ち、両順序を並べた本体と 4 つを 1 つの record にまとめた本体が
  同じ値になることを見る。

### 成果物

- `crates/hado/src/lib.rs`: `WithTypes` と `Checked::with_types`、`for_codegen`
  の `reveal`。
- `crates/hado/src/infer.rs`: `with` ごとの節点の型の記録（`note_type`）、
  `with_handler` の記録の範囲、`Checked` への受け渡し、module の型の修飾。
- `crates/hado/src/mono.rs`: `with_handler` での差し替えと復帰。
- `crates/hado/tests/r26_effects.rs`: 完了条件 (1)(2) の判定器。
- 本ファイル。

### 工程

main で再現（`answer-two` と probe_b 系）→ 赤のテスト（4 本、単独実行は判定器の
表）→ checker の記録と mono の差し替え → 周辺の形（`fn` の中・リストの要素・
`again` handler）の実測 → 複雑度の赤を補助関数へ出す → 着地の直前に
`git merge main` と corpus lock の作り直し。

### 判定

- `answer-two` が `{ a = Some(1); b = Some("a") }` を印字して終了コード 0。
- 入れ子の両順序（`probe_b6`）と 4 つを 1 つの record にまとめた形（`probe_b`）が、
  それぞれ単独で動かした値（`None`・`Some(None)`・`Some(None)`・`None`）のまま。
- `cargo test --offline` が 0 failed。
- 新しい 4 本のテストが直す前は赤、直した後は緑。`bash scripts/verify-m4.sh`・
  `bash scripts/run-gates.sh --tier quick`・`--tier core`・`bash scripts/lint-docs.sh`・
  `bash scripts/verify-spec-contracts.sh` が赤なし。

### 許容される赤

無し。

### スコープ外

- `again` handler を外側に、abort する本体を内側に置いた形（`do-option-absent` の
  probe_c3・probe_c5 が `with type is not concrete` で落ちる）。本ラウンドの
  完了条件の入力ではない。`again` handler を 2 つの答え型で使う形は直る
  （下の「周辺の形」）。
- handler の値引数（`handler Absent as OrValue(fallback: Int)`）が腕で束縛されない
  件（`handler-value-args`）。
- `do-option-absent`（M7）本体。
- `cargo test --offline` と層別（quick / core）はラウンドの最後に 1 回だけ回す。

### 中断の条件

判定が 2 回続けて赤になったら止める。仕様の解釈に疑義が出たら仕様を変えず、
依存しない作業だけ進める。

### 仕様への疑義

無し。§38 は handler の答え型を制限せず、`with` の入れ子と重ね掛けを許す。
今回の入力は型検査（`hado check`）が受理する。実装の当て所が issue の推測
（`mono.rs` の `handler_answers` の周辺）ではなく checker の span 単位の表だった
ことは、下の「実装から分かったこと」に書く。仕様は変えない。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §38 | handler は答え型の制限を持たず、同じ handler を答え型の違う複数の `with` に使える | 単相化 | 成功 | `crates/hado/tests/r26_effects.rs` |

## 実測

issue の `answer-two` と、`do-option-absent` の実験（`absent-effect-draft`）の
probe_b 系を `target/issue/<名>/main.hd` に置いて走らせる。probe_b 系は後置の
綴りを含まないのでそのまま使える。

再現（main、直す前）。

```
$ cargo run --offline -q -- check target/issue/answer-two/main.hd; echo "check exit=$?"
check exit=0
$ cargo run --offline -q -- run target/issue/answer-two/main.hd; echo "run exit=$?"
handler arms disagree on the answer type
run exit=1
```

入れ子の 4 つの形を単独で動かした値（判定器の独立の表）と、まとめた形の直す前。

```
$ for f in probe_b1 probe_b2 probe_b3 probe_b4; do printf '%s: ' $f; cargo run --offline -q -- run target/issue/$f/main.hd 2>&1 | tr '\n' ' '; echo; done
probe_b1: None
probe_b2: Some(None)
probe_b3: Some(None)
probe_b4: None
$ for f in probe_b probe_b5 probe_b6; do printf '%s: ' $f; cargo run --offline -q -- run target/issue/$f/main.hd 2>&1 | tr '\n' ' '; echo; done
probe_b: done payload mismatch
probe_b5: handler arms disagree on the answer type
probe_b6: done payload mismatch
```

直す前の新しいテスト。

```
$ cargo test --offline --test r26_effects -- one_body_uses_a_handler_at_two_answer_types nested_handler_orders all_nested_handler_orders
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 63 filtered out; finished in 1.52s
```

直した後。

```
$ cargo run --offline -q -- run target/issue/answer-two/main.hd; echo "exit=$?"
{ a = Some(1); b = Some("a") }
exit=0
$ cargo run --offline -q -- run target/issue/probe_b/main.hd
{ mo_abs = None; mo_err = Some(None); eo_abs = Some(None); eo_err = None }
$ cargo run --offline -q -- run target/issue/probe_b6/main.hd
{ a = None; b = Some(None) }
$ cargo test --offline --test r26_effects -- one_body_uses_a_handler_at_two_answer_types nested_handler_orders all_nested_handler_orders
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 63 filtered out; finished in 0.71s
```

### 原因の切り分け

`mono.rs` に一時の `eprintln!` を足して `dump-mono` と `run` を走らせた
（探針は残していない）。

```
$ HADO_MONO_DEBUG=1 cargo run --offline -q -- dump-mono target/issue/answer-two/main.hd
DBG ctor Some node (4, 17) declared Data("Option", [String]) args [Int]
main.hd:6:12: error: handler arms disagree on the answer type

$ HADO_MONO_DEBUG=1 cargo run --offline -q -- run target/issue/probe_b5/main.hd
DBG arms disagree outgoing=Data("Option", [Int]) arms=[("$Done", Data("Option", [Int])), ("$Fail#1", Data("Option", [Data("Option", [Int])]))]

$ HADO_MONO_DEBUG=1 cargo run --offline -q -- run target/issue/probe_b/main.hd
DBG done mismatch value=Data("Option", [Data("Option", [Int])]) success=Data("Option", [Int]) outcome=Data("$Outcome", [Data("Option", [Int]), Data("Option", [Int]), Void])
done payload mismatch
```

`Some(value)` は引数の型で結果型を組み直すので `Option[Int]` に直る。`None` は
引数を持たず `value_types` の `(4, 17)` ではなく自分の span の型をそのまま使うので、
最後の `with`（`String`、外側の答え `Option[Int]`）の型が残る。表の stale が原因で、
腕の lower 自体は毎回やり直されている。

### 周辺の形

直した後に、同じ handler を 2 つの答え型で使う形を 3 つ足して確かめる。`fn` の中と
リストの要素は直す前は赤だった（`answer-again` は直す前も緑で、`again` の腕が
答え型を引数から組むため）。

```
$ cargo run --offline -q -- run target/issue/answer-poly/main.hd
{ a = Some(1); b = Some("a") }
$ cargo run --offline -q -- run target/issue/answer-lambda/main.hd
{ a = Some(1); b = Some("a") }
$ cargo run --offline -q -- run target/issue/answer-list/main.hd
{ a = [Some(1), Some(2)]; b = [Some("a")] }
$ cargo run --offline -q -- run target/issue/answer-again/main.hd
{ a = Some(0); b = Some(()) }
```

直す前（`a286e96` の親の `crates/hado/src`）に同じ 3 つを当てた結果。

```
before-fix answer-again: { a = Some(0); b = Some(()) }
before-fix answer-lambda: handler arms disagree on the answer type
before-fix answer-list: handler arms disagree on the answer type
```

### ラウンド最後の全走

`git merge main` の前の木（`7d625ed`）で回した。

```
$ cargo test --offline > target/final-cargo-test.log 2>&1; echo "exit=$?"
exit=0
$ grep -c '^test result: ok' target/final-cargo-test.log
70
$ grep -cE '^test result: FAILED' target/final-cargo-test.log
0
$ bash scripts/run-gates.sh --tier quick target/gates-quick | tail -1
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates-core | tail -1
TALLY: 697/697 passed; red: none
$ bash scripts/verify-m4.sh | tail -2
accept: 130/130
PASSED: 159/159
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh | tail -1
PASSED: 2/2
```

`verify-cognitive-complexity.sh` は最初の core 走で赤になった。
`qualify_module_value_types` が 8/7。`with` ごとの型を修飾する腕を補助関数
（`qualify_module_with_types`）へ出して 3/3 に戻した。往復と `cargo test` は
緑のまま。
