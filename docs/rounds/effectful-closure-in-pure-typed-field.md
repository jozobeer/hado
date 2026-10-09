# 純粋な型の field に載せた効果付き閉包が単相化の型を割る（effectful-closure-in-pure-typed-field）

## 手順書

### 背景

- 効果を運ぶ閉包を record や ADT の field に載せると、射影（`b.f`）や取り出し
  （`if b | Box(f) => …`）が宣言側の `Fn(Int => Int)` を使い、実際の閉包
  （結果が `$Outcome`）を素の `Int` として読む。実行すると
  `hado: exit code out of range` になる（`docs/issues/effectful-closure-in-pure-typed-field.md`）。
- 型引数のある型（`type A Box = { f: A }`、`Some(fn(x => 8 / x))`）は、`actual_type`
  が効果行を見て型引数に `$Outcome` の閉包を載せるので通る。型引数の無い単相の型は
  `Data("Box", [])` となり、実際の field 型が消える。
- 実際の field 型を `Data` の引数に載せると直るが、載せ方を値の中身に任せると
  同じソースの型が 2 つの mono の型に割れる。効果を運ぶ値は `$Outcome` の閉包、
  運ばない値は素の閉包になるためである。`Fun(fn(x => 8 / x))` と `Num(7)` が
  同じ `Data("Sh", …)` に合流できず、`invalid branch types` でビルドが落ちる。
  この形は main では `107` / `3` と正しく走るので、着地すると退行になる。

### 決定

1. handler の下では、単相の型の閉包の field をすべて `$Outcome` の閉包の形に揃える。
   効果を運ばない値は `done` で包んで同じ型にする（`outcome_form` と
   `as_outcome_closure`）。`list_elements` が宣言側の要素型に合わせて行っているのと
   同じ扱いである。
2. 揃えるのは単相の型だけに限る。多相の record / ADT は `actual_type` が既に効果行を
   見ており、値だけ揃えると型と食い違う。
3. 答えは `context_answer` ではなく `callback_answer` から取る。field の閉包は
   handler の下で呼ばれるので、handler の答えを返す形が正しい。
4. 単相の ADT で `Data` の引数に field 型を載せるのは、構築子が 1 つのときだけに
   限る。2 つ以上あると構築子ごとに field の並びが違うので載せられない。
5. `constructor_field_types` の早期 return に構築子の条件を足す。引数の個数だけでは
   `Num(n)` の `n` に `Fun` の field 型を当ててしまう。

### code-complete の一周目

- 意図: 型検査が受理する「純粋な型の field に載せた効果付き閉包」を、型検査のとおり
  実行させる。同時に、同じソースの型が mono で 1 つの表現になるようにする。
- 仮説: handler の下で field の閉包を outcome の形に揃えれば、射影が `$Outcome` を
  保ち、どの構築でも同じ `Data` の引数になって合流点でも割れない。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。
- 未確認: 効果を運ぶ閉包を field に持つ多相 record（`type (A, B) Box = { f: A; g: B }`）
  で、curried な閉包を載せた形。main でも値が違う（下の「多相 record」）。

### 期待値の根拠

- record: `with Catch` の下で `8 / 0` は `raise` する。handler の `abort` の腕が `0` を
  返すので `b.f 0` は `0`。
- 単相の ADT（構築子 1 つ）: 同じ理由で `f 0` は `0`。
- 合流点: `if 1 == 2 => … | …` は片方しか走らない。`Num(7)` と `fn(x => x + 1)` の側が
  走るので、それぞれ main と同じ `107` と `3` でなければならない。

### 成果物

`crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。

### 工程

再現（main と PR #79 の枝で突き合わせ）→ 赤のテスト → `outcome_form` の追加 →
`refine_record_type` / `constructed_type` の揃え → 周辺の形の実測 → 全走。

### 判定

- `target/probe/issue/main.hd` が `0` で終了する（main は `hado: exit code out of range`）。
- `target/probe/b3/main.hd`（構築子 1 つの ADT）が `0` で終了する（main は同じ誤り）。
- `target/probe/a3/main.hd` が `107`、`target/probe/a4/main.hd` が `3`（main と同じ）。
- `target/probe/b2/main.hd` が main と同じ `hado: exit code out of range`（退行しない）。
- `cargo test --offline --test r26_effects` が 0 failed。
- `cargo test --offline` が 0 failed、`cargo fmt --all --check` と clippy が赤なし。

### 審査の結果（着地不可）

別モデル系（`claude -p --model claude-opus-5-5 --effort high`）の審査が「欠陥あり」。
`6747eb1` は main では正しく走る 4 系統を壊す。どれも #79 の枝では main と同じ値なので、
本ラウンドの修正が持ち込んだ退行である。

| 探針 | main | #79 の枝 | 直した枝 |
|---|---|---|---|
| `def mk = { f = fn(x => x + 1) }` / `with Catch -> mk as b -> b.f 2` | 3 | 3 | BUILD 失敗 `lowered return type differs from inferred signature for mk` |
| `def call(b: Sh) => b.f 2` / `with Catch -> call({ f = fn(x => x + 1) })` | 3 | 3 | BUILD 失敗（同上、`file0$call`） |
| `Map.insert Map.empty "b" { f = fn(x => x + 1) }` を `Map.find` で取り出す | 3 | 3 | BUILD 失敗 `Arity` |
| `type Sh = { f: Fn(Int => Fn(Int => Int)) }` に `fn(x => fn(y => x + y))` | 42 | 42 | exit=132（SIGILL、core dump） |

4 件目は `outcome_form` が入れ子の閉包まで直すのに、`wrap_pure_closure` が包めずに値を
そのまま返すためである（`mono.rs` の `wrap_pure_closure` が `value_result != success` で
何もせず返す）。record の型と値が食い違ったまま codegen に進む。

**筋が破綻する理由。** 「実際の field 型を `Data` の引数に載せる」にせよ「handler の下で
`$Outcome` の形に揃える」にせよ、同じソースの型が効果の文脈によって 2 つの mono の型に
割れる。単相の型は型引数を持たないので、`$Outcome` が要る答え型を載せる場所が無い。
record を値で作った場所だけ揃えても、関数の境界・型を書いた引数・`Map` の型引数は
宣言側の型のままになる。`a3` と `a4` が #79 の枝で `invalid branch types` になったのも
同じ理由である。

このラウンドは着地させない（PR #80 は下書きに戻した）。表現の決め方を人が決めるまで
止める。`docs/issues/effectful-closure-in-pure-typed-field.md` を戻し、分かったことを
書いた。

### 許容される赤

無し。

### スコープ外

- 効果を運ぶ閉包を field に持つ多相 record（下の「多相 record」）。
  `docs/issues/curried-outcome-closure-in-polymorphic-record.md` に切った。
- 構築子を 2 つ以上持つ単相の ADT の field（下の「多構築子の ADT」）。
  `docs/issues/multi-constructor-adt-field-outcome.md` に切った。
- `cargo test --offline` と層別はラウンドの最後に 1 回だけ回す。

### 中断の条件

判定が 2 回続けて赤になったら止める。仕様の解釈に疑義が出たら仕様を変えず、
依存しない作業だけ進める。

### 仕様への疑義

無し。§36 は record の field に効果付きの関数値を載せることを許し、§19 が `/` を
効果として挙げる。本ラウンドの入力はすべて `hado check` が受理する。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §19, §36 | 純粋な型の field に載せた効果付き閉包も、射影と取り出しで outcome を保つ | 単相化 | 成功 | `crates/hado/tests/r26_effects.rs` |

## 実測

PR #79 の枝（`cc922a9`）と main（`350a575`）を突き合わせた。`a3` は
`type Sh = | Fun(Fn(Int => Int)) | Num(Int)`、`a4` はその record 版で、どちらも
`(if 1 == 2 => <効果付き> | <効果なし>)` の合流点を持つ。

```
$ cd .worktrees/pr79 && cargo run --offline -q -- run target/probe/a3/main.hd; echo "exit=$?"
invalid branch types
exit=1
$ cd .worktrees/pr79 && cargo run --offline -q -- run target/probe/a4/main.hd; echo "exit=$?"
invalid branch types
exit=1
$ cd /home/zyun/git/local/hado && cargo run --offline -q -- run target/probe/a3/main.hd; echo "exit=$?"
exit=107
$ cd /home/zyun/git/local/hado && cargo run --offline -q -- run target/probe/a4/main.hd; echo "exit=$?"
exit=3
```

issue の入力（`type Box = { f: Fn(Int => Int) }` に `fn(x => 8 / x)` を載せ、
`b.f 0` を呼ぶ）と、構築子 1 つの ADT 版。

```
$ cd /home/zyun/git/local/hado && cargo run --offline -q -- run target/probe/issue/main.hd; echo "exit=$?"
hado: exit code out of range
exit=1
$ cd /home/zyun/git/local/hado && cargo run --offline -q -- run target/probe/b3/main.hd; echo "exit=$?"
hado: exit code out of range
exit=1
```

直した枝（`6747eb1`）。

```
$ cargo run --offline -q -- run target/probe/issue/main.hd; echo "exit=$?"
exit=0
$ cargo run --offline -q -- run target/probe/b3/main.hd; echo "exit=$?"
exit=0
$ for p in a3 a4 b2; do out=$(cargo run --offline -q -- run target/probe/$p/main.hd 2>&1); echo "$p exit=$? out=$out"; done
a3 exit=107 out=
a4 exit=3 out=
b2 exit=1 out=hado: exit code out of range
```

```
$ cargo test --offline --test r26_effects
test result: ok. 98 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 88.40s
$ cargo fmt --all --check && echo FMT_OK
FMT_OK
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | grep -cE '^(error|warning)'
0
```

### ラウンド最後の全走

`cargo test --offline` は 1 つでも落ちれば非 0 を返す。末尾の suite は `walk` が 1 passed、
`Doc-tests hado` が 0 passed。

```
$ cargo test --offline; echo "exit=$?"
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.87s

   Doc-tests hado

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit=0
$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 41/41 passed; red: none
$ bash scripts/verify-m4.sh
reject: 26/26
accept: 146/146
PASSED: 174/174
$ bash scripts/lint-docs.sh
PASSED: 16/16
```

### 多相 record

`type (A, B) Box = { f: A; g: B }` の `g` に curried な閉包（結果が閉包）を載せた形は、
main でも値が違う。揃える対象を単相の型に限ったので、直した枝でも同じである。

```
$ cd /home/zyun/git/local/hado && cargo run --offline -q -- run target/probe/b2/main.hd; echo "exit=$?"
hado: exit code out of range
exit=1
```

### 多構築子の ADT

構築子を 2 つ持つ単相の ADT の field に効果付き閉包を載せ、その field を直接取り出す形は、
main でも直した枝でも同じ誤りになる。`Data` の引数に field 型を載せる形は、構築子ごとに
field の並びが違うので選べない。

```
$ cargo run --offline -q -- run target/probe/b4/main.hd; echo "exit=$?"
hado: exit code out of range
exit=1
```
