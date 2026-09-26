# 効果の違う閉包を並べたリストリテラルで単相化が落ちる（effectful-closure-in-list-literal）

## 手順書

### 背景

- 効果を持つ閉包と持たない閉包を 1 つのリストリテラルに並べると、型検査は通るのに
  単相化が `list element type mismatch` で落ちる（`docs/issues/effectful-closure-in-list-literal.md`）。
- `crates/hado/src/mono.rs` の `list_elements` は、要素型が効果を運ぶ関数なら
  `outcome_closure_of` で要素型を outcome の閉包にする。効果を持たない要素の閉包は
  outcome の無い閉包のまま lower されるので、`Cons` を組む直前の型照合で食い違う。
- 呼び出し側はリストの要素型（outcome の閉包）で callee を単一化するので、値の側でも
  outcome を返さないと実行時に handler が解釈できない。

### 決定

issue の「案」の推奨に従う。

1. `list_elements` は宣言側の要素型が outcome の閉包のとき、outcome を運ばない
   要素値を「呼んで結果を `done` で包む閉包」へ直してから並べる。
2. 直せない不一致（引数の型が違う、成功型が違う）はそのまま `cons_list` に渡し、
   従来どおり `list element type mismatch` で止める。
3. 「要素ごとに効果が違うリストを拒否する」案は採らない。§36 の型では受理される。

### code-complete の一周目

- 意図: 効果の違う閉包を 1 つのリストリテラルに並べたプログラムを、型検査のとおり
  実行させる。§36 は効果の伝播を何も書かずに許す。
- 仮説: 宣言要素型（outcome の閉包）に合わせて、outcome を持たない要素の閉包を
  「呼んで結果を `done` で包む閉包」に直せば、`Cons` の要素型が揃い、呼び出し側の
  outcome の解釈とも一致する。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。
- 未確認: 要素の値の型が宣言要素型の logical 形と一致するか。閉包を返す閉包
  （入れ子の outcome）でも同じずれが起きるか。

### 期待値の根拠

issue の完了条件のとおり。`with Catch` の下で `8 / x` は効果を運ぶ閉包になり、
`fn(x => x + 1)` は運ばない。`fs` の要素型は両者を単一化した outcome の閉包なので、
`g 4` は `4 + 1 = 5`。直す前は組み立ての時点で落ちる。

### 成果物

`crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。

### 工程

再現 → 赤のテスト（issue の入力そのまま）→ `as_outcome_closure` の追加 →
周辺の形の実測。

### 判定

- 上のプログラムが `5` を印字して終了コード 0。
- 同じ入力が `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。

### 許容される赤

無し。

### スコープ外

- 閉包を返す閉包（入れ子の outcome の閉包）を並べたリスト（下の「入れ子の要素」）。
  `hado check` は受理するが、同じ `list element type mismatch` で落ちる。
- `cargo test --offline` と層別（quick）はラウンドの最後に 1 回だけ回す。

### 中断の条件

判定が 2 回続けて赤になったら止める。仕様の解釈に疑義が出たら仕様を変えず、
依存しない作業だけ進める。

### 仕様への疑義

無し。§36 は効果の伝播を禁じず、§19 が `/` を効果として挙げる。今回の入力は
型検査（`hado check`）が受理する。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §19, §36 | 効果の違う閉包を並べたリストリテラルも受理され、失敗が handler へ伝わる | 単相化 | 成功 | `crates/hado/tests/r26_effects.rs` |

## 実測

issue の入力を `target/scratch/listlit/main.hd` に置いて走らせる。直す前。

```
$ cargo run --offline -q -- run target/scratch/listlit/main.hd
list element type mismatch
run exit=1
```

直す前のテスト。

```
$ cargo test --offline --test r26_effects list_literal_of_closures_with_different_effects_keeps_their_outcome
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 61 filtered out; finished in 0.29s
```

直した後。

```
$ cargo run --offline -q -- run target/scratch/listlit/main.hd
5
issue exit=0
$ cargo test --offline --test r26_effects
test result: ok. 62 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.56s
$ bash scripts/verify-m4.sh
reject: 27/27
accept: 111/111
PASSED: 140/140
```

対照（効果の無い要素を先に置き、効果を持つ要素を後ろに置く）。

```
$ cargo run --offline -q -- run target/scratch/listlit-reversed/main.hd
5
$ cargo run --offline -q -- run target/scratch/listlit-three/main.hd
0
```

### 入れ子の要素

閉包を返す閉包を並べると、型検査は通るが同じずれが残る。本ラウンドの完了条件の外。

```
$ cat target/scratch/listlit-nested/main.hd
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  let fs = [fn(x => fn(y => x * 0 + 8 / y)), fn(x => fn(y => x * 0 + y + 1))] ->
  if fs | [_, g] => (g 0) 4 | 0
$ cargo run --offline -q -- check target/scratch/listlit-nested/main.hd; echo "check exit=$?"
check exit=0
$ cargo run --offline -q -- run target/scratch/listlit-nested/main.hd; echo "run exit=$?"
list element type mismatch
run exit=1
```

### ラウンド最後の全走

`git merge main`（14be875）と `bash scripts/corpus-lock.sh --check`（差分なし、exit 0）の後。

```
$ cargo test --offline > target/final-cargo-test.log 2>&1; echo "exit=$?"
exit=0
$ grep -c '^test result: ok' target/final-cargo-test.log
69
$ grep -cE '^test result: FAILED' target/final-cargo-test.log
0
$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 41/41 passed; red: none
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
```
