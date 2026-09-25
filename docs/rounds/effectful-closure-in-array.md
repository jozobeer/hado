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

一次レビュー（Codex）の指摘 2 件。1 件は再現した退行で、1 件は再現できなかった。

指摘 1（`specialize_function` の結果型の復元）は再現した。引数の論理型が同じで
実際の型が違うと、結果型の出現を片方の引数の実際の型へ戻していた。

```
$ cat sel/main.hd
handler Error as Catch
abort raise _ => 0
return value => value

def select(f: Fn(Int => Int {Error}), g: Fn(Int => Int)): Fn(Int => Int) => do f 1 -> g

def main =
  with Catch ->
  let h = select fn(x => 8 / x) fn(x => x + 1) ->
  h 1
$ cargo run --offline -q -- run sel/main.hd
lowered return type differs from inferred signature for file0$select
# この復元の適用だけを外すと通る（この PR が入れた退行）
$ cargo run --offline -q -- run sel/main.hd
2
```

`ambiguous_logical_types` で、同じ論理型を持つ引数の実際の型が食い違うときは
その論理型へ戻さないようにした。戻さない結果は復元を入れる前と同じ挙動である。
`Array.of_list` の形（引数が 1 つ）は衝突しないので目的は変わらない。

```
$ cargo test --offline -q --test r26_effects
test result: ok. 54 passed; 0 failed
$ cargo test --offline -q --test r26_effects an_effectful_parameter_does_not_restore
# ガードを外すと赤
assertion `left == right` failed: lowered return type differs from inferred signature for file0$select
$ bash scripts/verify-m4.sh 2>&1 | tail -1
PASSED: 140/140
$ cargo test --offline -q 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 755 failed 0
```

指摘 2（`restored_success_type`）は再現できなかった。構造照合が働くこと自体は
確かめた。`Array.get#Catch` の本体で、abort の成功型 `Closure([Int], Int)` に対し
`Closure([Int], $Outcome(Int, Int, Void))` を返している。ただし、これが有効な
プログラムを壊す入力を組めなかった。`if b => fn(x => (x + 1)) | raise { message =
"no" }` を評価してから効果付きの閉包を返す形は、この修正の前後で同じ
`lowered return type differs from inferred signature for file0$make` で落ちる
（`restored_success_type` を外しても同じ出力）。abort の成功型が本当に純粋な
閉包で、関数の結果型が効果付きの閉包になる入力が要る。返信で最小の入力を仰いだ。
