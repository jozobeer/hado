# 右端の被演算子の `if` を括弧で囲む（tail-operand-branch-parentheses）（2026-10-10）

## 手順書

### 背景

- F5（PR #76、`350a575`）で `fmt` / `check --fix` / `build --fix` が §58.1 の
  印字し直しをするようになった。印字器（`crates/hado/src/postfix_print.rs`）が、
  続きの無い位置に `if(...)` を出すと、その出力は §12.1 の E100
  （`` `if(...)` with nothing after it``）になり、動いていたプログラムを `fmt` が
  壊す。
- PR #81（`17e644f`）は並置の最後の引数だけを `(if …)` で囲んだ。同じ欠陥が
  二項演算子の右辺、単項の `-` と `not` の被演算子、`|>` の右辺にも残っていた。
- 審査 `target/reviews/pr81-review.md` が 6 つの位置を実測で示した。どれも main
  でも壊れる（退行ではない）が、`fmt` が動くプログラムを壊すという同じ欠陥である。

### 意図

`fmt` と `--fix` が、動いていたプログラムを E100 に壊さないようにする。続きの
無い位置の分岐は `(if …)` と括弧で囲み、印字した綴りをそのまま `check` が受理し、
`run` の値が変わらないようにする。

### 仮説

`tail` は既に「その位置に続きが無いこと」を運んでいる。`Neg`・`Not`・`Bin`・
`Pipe`・`Apply` が右端の被演算子へ `tail` を渡していなかったので、続きの無い
位置でも `if(...)` を印字していた。右端へ `tail` を渡し、続きの無い分岐を
`enclosed` の外側に明示の括弧を足した `(if …)` にすれば、裸の `if …` と
`if(...)` のどちらも置けない位置で通る。`tail` を引き継ぐのは演算子だけで、
束縛や `with` は裸で被演算子に置けないので引き継がない。

### code-complete の一周目

- 意図: 上と同じ。
- 仮説: 上と同じ。決め打ちしたのは「`(if …)` が唯一の綴りであること」と
  「`tail` を引き継ぐのは演算子だけであること」。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/postfix_print.rs`、`crates/hado/tests/fmt_layout.rs`、
  本ファイル、切り出した `docs/issues/`。`docs/spec.md` は触らない。
- 未確認: `Field` の base が適用のとき `.a` が内側に付く欠陥（別件、下の
  「スコープ外」）。`with_args` の引数の位置は実測で直った。

### 期待値の根拠

- §12.1: `if` の本体に `->` が無いとき、`if` の全体に続き（`->`、`as`、`|>`、
  二項演算子、後続の引数）があれば `if(...)` と書き、続きが無ければ何も
  書かない。`if(...)` の全体に続きが無ければ E100。
- §59: `unary := "-" unary | "not" unary | application`、
  `application := postfix postfix*`、`primary := … | "(" expression ")"`。
  `if` は `prefix_form` なので、裸の `if …` は被演算子と引数に置けない
  （`expected an expression, found if`）。`(if …)` は `primary` の
  `"(" expression ")"` として置ける。
- この 2 つから、続きの無い被演算子に置けるのは `(if …)` だけである。実測でも
  `1 + if(true => …)`・`-if(true => …)`・`not if(true => …)`・`f if(true => …)` は
  すべて E100 で、`1 + (if true => …)` などは受理される。

### 成果物

`crates/hado/src/postfix_print.rs`、`crates/hado/tests/fmt_layout.rs`、本ファイル、
`docs/issues/parenthesize-field-base-after-application.md`（実測で見つけた別件）。

### 工程

6 つの位置を探針で測る（赤の全部書き出し）→ 赤のテスト →
`operand_text` と `tail` の受け渡し → 周辺の位置の実測 → 門番 → 全走。

### 判定

- 6 つの位置すべてで `fmt` の後も `check` が通り、`run` の値が変わらない。
- `fmt` を 2 回当てて同じバイト列（冪等）。
- `cargo test --offline --test fmt_layout`。
- `bash scripts/verify-fmt-layout.sh` が 6/6。
- `verify-r52.sh`・`verify-r53.sh`・`verify-r57.sh`・`verify-r64.sh` が緑。
- `verify-r63.sh` の `d/argument-fix` が緑のまま。
- `cargo fmt --all --check` と
  `cargo clippy --offline --all-targets -- -D warnings` が 0 件。
- 最後に 1 回だけ `cargo test --offline` と
  `bash scripts/run-gates.sh --tier quick target/.gates-tailoperand`。

### 許容される赤

無し。ただし `verify-r63.sh` の `d/let-fix` はこの枝の土台（`d852e77` の印字器、
PR #81）でも同じ assertion で赤で、F5 の印字し直しで期待値が古くなったもの
である（下の「`verify-r63.sh` の赤」）。本ラウンドの起因ではない。

### スコープ外

- `Field` の base が適用のとき `.a` が内側に付く欠陥。分岐に限らず、
  `(f (g 1)).a` も `f g 1.a` になり E103 で壊れる。本ラウンドの起因ではない。
  `docs/issues/parenthesize-field-base-after-application.md` に切り出した。
- 審査の「スコープ外の所見」5 件（`check --fix` が壊れた結果でも exit 0 を返す、
  書き換えが閉じの印と食い違う、コメントの置き場、条件列の空白 2 つ、E100 の
  勧める形が通らない）。直していない。報告に書く。
- `docs/spec.md` の疑義。仕様は変えず、報告に書く。

### 中断の条件

判定が 2 回続けて赤になったら止める。仕様の解釈に疑義が出たら仕様を変えず、
依存しない作業だけ進める。

### 仕様への疑義

§12.1 は「`if` の全体に続きがあれば `if(...)` と書き、続きが無ければ何も
書かない」と定める。しかし実装は、二項演算子の右辺、単項 `-`、`not`、並置の
引数の位置で、裸の `if …` を受理しない（`expected an expression, found if`）。
この位置で受理されるのは `(if …)` だけである。§12.1 の本文からは、どの位置が
「続きの無い被演算子」に当たり `(if …)` が要るかを直接は読めない。§59 の
`primary := "(" expression ")"` を突き合わせれば導けるが、E100 の勧める形
（「括弧なしの `if true => …`」）はこの位置では通らない。仕様は変えていない。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §12.1 | 続きの無い `if(...)` は E100。`(` の閉じの `)` が `if` の閉じを兼ねる | 構文 | E100 | `crates/hado/tests/fmt_layout.rs` |
| §59 | `unary`・`application` は `prefix_form` を取らないので、裸の `if …` は被演算子と引数に置けない | 構文 | E100 | `crates/hado/tests/fmt_layout.rs` |
| §58.1 | `fmt` の印字し直した綴りが、動くプログラムの綴りでなければならない | 冗長 | — | `crates/hado/tests/fmt_layout.rs`・`scripts/verify-fmt-layout.sh` |

## 実測

### 6 つの位置（探針 `target/probe/positions.sh`）

各位置を独立した根に置く（`hado` は入力の親を根として配下の `.hd` を読むので、
1 根 1 ファイルにする）。直す前は 6 つとも、`run` は通り、`fmt` の後が E100 だった。

| 位置 | 入力 | `run` | `fmt` の後 | `check` |
|---|---|---|---|---|
| 二項 `-` の右辺 | `Int.to_string (100 - (if true => 59 \| 0))` | 41 | `Int.to_string (100 - if(true => 59 \| 0))` | E100 |
| 単項 `-` | `Int.to_string (- (if true => 41 \| 0))` | -41 | `Int.to_string (-if(true => 41 \| 0))` | E100 |
| 引数の中の単項 `-` | `f (- (if true => 42 \| 0))` | -42 | `f (-if(true => 42 \| 0))` | E100 |
| `not` | `not (if true => false \| true)` | 1 | `not if(true => false \| true)` | E100 |
| `&&` の右辺 | `true && (if true => true \| false)` | 1 | `true && if(true => true \| false)` | E100 |
| `<` の右辺 | `1 < (if true => 2 \| 0)` | 1 | `1 < if(true => 2 \| 0)` | E100 |

直した後は 6 つとも緑。`check` の exit 0、`run` の値は直す前と同じ、2 回目の
`fmt` は同じバイト列。

```
=== 位置 1 ===
run(before): exit=0 out=41
fmt 後: def main = Int.to_string (100 - (if true => 59 | 0))
check: exit=0 out=
run(after): exit=0 out=41
fmt 2 回目後: def main = Int.to_string (100 - (if true => 59 | 0))
=== 位置 2 ===
run(before): exit=0 out=-41
fmt 後: def main = Int.to_string (-(if true => 41 | 0))
check: exit=0 out=
run(after): exit=0 out=-41
=== 位置 3 ===
run(before): exit=0 out=-42
fmt 後: def main = Int.to_string (f (-(if true => 42 | 0)))
check: exit=0 out=
run(after): exit=0 out=-42
=== 位置 4 ===
run(before): exit=0 out=1
fmt 後: def b = not (if true => false | true)
check: exit=0 out=
run(after): exit=0 out=1
=== 位置 5 ===
run(before): exit=0 out=1
fmt 後: def b = true && (if true => true | false)
check: exit=0 out=
run(after): exit=0 out=1
=== 位置 6 ===
run(before): exit=0 out=1
fmt 後: def b = 1 < (if true => 2 | 0)
check: exit=0 out=
run(after): exit=0 out=1
```

### 周辺の位置（探針 `target/probe/positions_all.sh`）

右端の被演算子は並置の引数だけではない。同じ規則が効く位置を先に赤の一覧として
書き出し、直した後に全部緑になることを確かめる。括弧の位置が変わったものを
`fmt` の後で示す。

```
def-body-bin     def v = 1 + (if true => 1 | 0)
arg-bin          def main = Int.to_string (1 + (if true => 1 | 0))
arg-bin-sub      def main = Int.to_string (100 - (if true => 1 | 0))
arg-bin-and      def b = true && (if true => true | false)
arg-bin-lt       def b = 1 < (if true => 1 | 0)
arg-neg          def main = Int.to_string (-(if true => 1 | 0))
arg-not          def b = not (if true => true | false)
nested-bin-bin   def main = Int.to_string (1 + (2 - (if true => 1 | 0)))
nested-bin-neg   def main = Int.to_string (1 + -(if true => 1 | 0))
list-elem-bin    def v = [1 + (if true => 1 | 0)]
list-elem-neg    def v = [-(if true => 1 | 0)]
binding-body-bin def main = 1 as y -> Int.to_string (y + (if true => 1 | 0))
record-field-bin def r = { a = 1 + (if true => 1 | 0) }
call-arg-bin     Option.unwrap_or Some(1 + (if true => 1 | 0)) 0
fn-body          fn(x => x + (if true => 1 | 0))
fn-last-arg      List.map [1] fn(x => x + (if true => 1 | 0))
pipe-right       1 |> (if true => inc | dec)
with-args        with Fixed(1 - (if true => 5 | 0)) -> outer 36
```

括弧を外さない位置（`if(...)` のままで受理される）も変わらない。

```
binding-value-bin  def main = 1 + if(true => 1 | 0) as y -> …
pipe-left          def main = Int.to_string (if(true => inc | dec) 1)
field-base         Int.to_string if(true => { a = 1 } | { a = 2 }).a
```

### 赤のテスト

`crates/hado/tests/fmt_layout.rs` に 2 つのテストを足した。直す前は 5 passed;
2 failed で、`fmt` の後に `check` が E100 になった。

```
$ cargo test --offline --test fmt_layout
test rightmost_operand_branches_survive_in_every_position ... FAILED
test rightmost_operand_branches_survive_the_reprint ... FAILED
test result: FAILED. 5 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
---- rightmost_operand_branches_survive_the_reprint stdout ----
sub: t:1:33: error[E100]: `if(...)` with nothing after it; write it without parentheses; canonical form: if true => 59 | 0:
def main = Int.to_string (100 - if(true => 59 | 0))
```

直した後。

```
$ cargo test --offline --test fmt_layout
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 判定条件ごとの実測

```
$ bash scripts/verify-fmt-layout.sh | tail -8
OK: a: 配置の違う 2 つの綴りが同じバイト列になる
OK: b: コーパス 188 ファイルが印字し直しで変わらない
OK: c: 本体の中のコメントが定義の中に残り、本体の前に来る
OK: d: 空行が残る
OK: e: コメントを落とす変異で b が赤になる（6 ファイル）
OK: f: 空の本体を持つ implement が印字し直しで増えない
PASSED: 6/6

$ bash scripts/verify-r52.sh | tail -1
PASSED: 5/5
$ bash scripts/verify-r53.sh | tail -1
PASSED: 5/5
$ bash scripts/verify-r57.sh | tail -1
PASSED: 6/6
$ bash scripts/verify-r64.sh | tail -1
PASSED: 4/4
$ cargo fmt --all --check; echo "exit=$?"
exit=0
$ cargo clippy --offline --all-targets -- -D warnings > /dev/null 2>&1; echo "exit=$?"
exit=0
```

### `verify-r63.sh` の赤

`verify-r63.sh` は `2/4`。`63-d` の `d/let-fix` が赤である。`check --fix` が
`LET_VALUE` を印字し直すと本体が `=>` の次の行から同じ行へ移るが、門番の期待値
`LET_VALUE_FIXED` は改行したままである。F5 の印字し直しで期待値が古くなったもの
で、土台（`d852e77` の印字器）でも同じ assertion で赤になる。本ラウンドの起因では
ない。`d/argument-fix` は土台でも直した後でも緑である。

```
$ bash scripts/verify-r63.sh | tail -3
AssertionError: ('d/let-fix', "'def f (b: Bool): Int => if(b => 1 | 2) as v ->\\n  v + 1\\n…'",
                              "'def f (b: Bool): Int =>\\n  if(b => 1 | 2) as v ->\\n  v + 1\\n…'")
NG: 63-d the exclusions and the argument position
PASSED: 2/4

# 土台（`git checkout d852e77 -- crates/hado/src/postfix_print.rs` でビルド）でも同じ
AssertionError: ('d/let-fix', …)
NG: 63-d the exclusions and the argument position
PASSED: 2/4
```

### ラウンド最後の全走

（下に追記する。）
