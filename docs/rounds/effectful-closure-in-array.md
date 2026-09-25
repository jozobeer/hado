# 効果を持つ閉包を Array に置くと outcome が落ちる（effectful-closure-in-array）

## 手順書

### 背景

- `Array.of_list` に効果を持つ閉包を入れ、`Array.get` で取り出して呼ぶプログラムは型検査を
  通るのに、単相化が `array intrinsic argument type mismatch` で落ちる
  （`docs/issues/effectful-closure-in-array.md`、PR #7）。
- 特殊化は scheme を論理型（閉包の結果から outcome を外した型）で単一化するので、
  `of_list` の本体では型変数が outcome の無い閉包に解ける。`__array_alloc` はその型で
  確保し、`__array_fill_unchecked` に渡る実際の閉包と `call_array` の照合で食い違う。

### 決定

1. `specialize_function` の結果型を、引数の論理型と実際の型の差から実際の型へ戻す
   （`restore_nested_outcomes`）。
2. 本体の `__array_alloc` の型が戻す前の結果型と一致するなら、戻した結果型を使う
   （`restored_array_allocation`）。
3. handler へ届く abort の成功型も戻した型に揃える（`restored_success_type`）。
   handler の無い abort（`abort_raise`）は修正前から通るので触らない。

### code-complete の一周目

- 意図: 効果を持つ閉包を Array に置くプログラムを、型検査のとおりに実行させる。
- 仮説: PR #7 の 3 か所（結果型・確保型・abort の成功型）を現行の `mono.rs` に移せば直る。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。
- 未確認: PR #7 の後に `mono.rs` が変わっても、同じ継ぎ目が残っているか。

### 期待値の根拠

issue の完了条件のとおり。`8 / 0` の `raise` が `Catch` に届き、腕の `0` が印字される。

### 成果物

`crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs` の
`an_effectful_closure_in_an_array_slot_keeps_its_outcome`、本ファイル。

### 工程

再現 → 赤のテスト → 3 か所の修正 → 欠陥の注入 → 周辺の形の実測。

### 判定

- issue のプログラムが `0` を印字して終了コード 0。
- 追加したテストが修正前は赤、修正後は緑。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。
- 3 か所のどれを外しても issue のプログラムが落ちる。

### 許容される赤

無し。

### スコープ外

- 効果を持つ閉包を多相な prelude 関数に通すと落ちる、より広い不具合
  （`docs/issues/effectful-closure-through-prelude.md`）。`Array.set`・`Array.to_list` は
  この穴で落ちる。
- 効果の違う閉包を並べたリストリテラル（`docs/issues/effectful-closure-in-list-literal.md`）。

### 中断の条件

欠陥の注入で、戻しても出力が変わらない箇所は入れない。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §19, §36 | 効果を持つ閉包を Array に置いて取り出して呼ぶと、失敗が handler へ伝わる | 単相化 | 成功 | `crates/hado/tests/r26_effects.rs` |

## 実測

修正前（`origin/main` の df4f2ac）。

```
$ cargo run --offline -q -- run main.hd
array intrinsic argument type mismatch
$ cargo test --offline -q --test r26_effects an_effectful_closure_in_an_array_slot
assertion `left == right` failed: array intrinsic argument type mismatch
```

修正後。

```
$ cargo run --offline -q -- run main.hd; echo "exit=$?"
0
exit=0
$ cargo test --offline -q 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 751 failed 0
$ bash scripts/verify-m4.sh 2>&1 | tail -1
PASSED: 140/140
```

欠陥の注入。各行は、その箇所だけを修正前に戻したときの issue のプログラムの出力。

| 戻した箇所 | 出力 |
|---|---|
| `restored_array_allocation` | `array intrinsic argument type mismatch` |
| `abort_outcome` の `restored_success_type` | `done payload mismatch` |
| `specialize_function` の結果型の復元 | `array intrinsic argument type mismatch` |

handler の無い abort（`abort_raise`）の成功型は、戻しても戻さなくても出力が同じ。

```
$ cat noh/main.hd
def main =
  let xs = Array.of_list [fn(x => 8 / x)] ->
  let f = Array.get xs 0 ->
  f 0
$ cargo run --offline -q -- run noh/main.hd
DivisionByZero
```

`Array.get xs 5` にした入力は `IndexOutOfRange`。どちらも df4f2ac で同じ出力。
