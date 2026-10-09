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
- 再審査 `target/reviews/pr81-rereview.md` が、さらに 6 つの位置を示した。演算子を
  `(…)` で包んで、その後ろに続きが来る位置である。`wrap` が足した `)` が §12.1 の
  閉じを兼ねるので、包まれた中の右端も「続きの無い位置」になる。

### 意図

`fmt` と `--fix` が、動いていたプログラムを E100 に壊さないようにする。続きの
無い位置の分岐は `(if …)` と括弧で囲み、印字した綴りをそのまま `check` が受理し、
`run` の値が変わらないようにする。

### 仮説

1. `tail` は既に「その位置に続きが無いこと」を運んでいる。`Neg`・`Not`・`Bin`・
   `Pipe`・`Apply` が右端の被演算子へ `tail` を渡していなかったので、続きの無い
   位置でも `if(...)` を印字していた。右端へ `tail` を渡し、続きの無い分岐を
   `(if …)` にすれば通る。
2. `wrap` が `(…)` を足す位置（`prec < min`）では、中身の右端も続きが無い。
   右端へ渡す `tail` に `prec < min` を足せば、包まれた中も `(if …)` になる。
   `tail` を引き継ぐのは演算子だけで、束縛や `with` は裸で被演算子に置けないので
   引き継がない。

### code-complete の一周目（右端の被演算子）

- 意図: 並置の最後の引数だけでなく、右端の被演算子すべてで `fmt` が壊さない。
- 仮説: 1。
- 判定器: 下の「判定」。
- 赤: `rightmost_operand_branches_survive_the_reprint` が `5 passed; 2 failed`
  （`sub` と `neg-argument` が E100）。
- 直し: 5 つの演算子が右端へ `tail` を渡し、続きの無い分岐を `(if …)` にする
  `operand_text` を足す。
- 判明: 並置の最後の引数だけでなく、二項の右辺・単項 `-`・`not`・`|>` の右辺・
  入れ子の適用の引数が直る。`with` の引数も `single_argument` の既定で直る。
- 次の一手: 包まれて後ろに続きがある位置を測る。

### code-complete の二周目（包まれた中）

- 意図: `wrap` の括弧の中も「続きの無い位置」として囲む。
- 仮説: 2。再審査の推奨（`tail || prec < min` を 4 か所へ）を自分で測り直す。
- 判定器: 下の「判定」。
- 赤: `wrapped_operand_branches_survive_the_reprint` の 6 件が E100。探索では
  `w1`・`w2`・`w3`・`w5`・`w6` を探針で、`w4` を直したテストで確かめた。
- 直し: `enclosed_by_wrap(tail, prec, min) = tail || prec < min` を足し、
  `Not`（`7 < min`）・`neg_text`（`7 < min`）・`Pipe`（`1 < min`）・`bin_text`
  （右辺、`prec < min`）の右端へ渡す。
- 判明: 左辺が包まれる場合（`(1 + (if …)) * 2`）も、後ろに引数が続く場合
  （`f (1 + (if …)) 2`）も、内側の `min` が包みを表すので同じ経路で直る。
- 次の一手: 全走と層別。

### 期待値の根拠

- §12.1: `if` の本体に `->` が無いとき、`if` の全体に続き（`->`、`as`、`|>`、
  二項演算子、後続の引数）があれば `if(...)` と書き、続きが無ければ何も
  書かない。`if(...)` の全体に続きが無ければ E100。`(` の閉じの `)` は `if` の
  閉じを兼ねるので、`wrap` が足した `)` の直前も続きが無い。
- §59: `unary := "-" unary | "not" unary | application`、
  `application := postfix postfix*`、`primary := … | "(" expression ")"`。
  `if` は `prefix_form` なので、裸の `if …` は被演算子と引数に置けない
  （`expected an expression, found if`）。`(if …)` は `primary` の
  `"(" expression ")"` として置ける。
- この 2 つから、続きの無い被演算子に置けるのは `(if …)` だけである。

### 成果物

`crates/hado/src/postfix_print.rs`、`crates/hado/tests/fmt_layout.rs`、本ファイル、
`docs/issues/parenthesize-field-base-after-application.md`（実測で見つけた別件）。

### 工程

6 つの位置を探針で測る（赤の全部書き出し）→ 赤のテスト → `operand_text` と
`tail` の受け渡し → 再審査の 6 位置を探針で測る → 赤のテスト →
`enclosed_by_wrap` → 周辺の位置の実測 → 門番 → 全走。

### 判定

- 最初の 6 つの位置と包まれた 6 つの位置のすべてで、`fmt` の後も `check` が通り、
  `run` の値が変わらない。
- `fmt` を 2 回当てて同じバイト列（冪等）。
- `cargo test --offline --test fmt_layout`。
- `bash scripts/verify-fmt-layout.sh` が 6/6。
- `verify-r52.sh`・`verify-r53.sh`・`verify-r57.sh`・`verify-r64.sh` が緑。
- `verify-r63.sh` が悪化しない（土台と同じ `2/4`）。
- `cargo fmt --all --check` と
  `cargo clippy --offline --all-targets -- -D warnings` が 0 件。
- 最後に 1 回だけ `cargo test --offline` と
  `bash scripts/run-gates.sh --tier quick target/.gates-tailoperand`。

### 許容される赤

`verify-r63.sh` の `2/4` の 2 件は、この枝の土台（PR #81 の印字器）でも同じ赤で、
本ラウンドの起因ではない。F5 の印字し直しで古くなった期待値と、別レーンの欠陥で
ある。

- `63-c` の `c/mixed`: `check` が E520 を期待される入力で E402
  （`not all constructor values are covered`）を返す。ADT の実装と名前の同一視の
  欠陥（別レーン）で、審査も main（`4baca98`）で同じ赤と報告している。
- `63-d` の `d/let-fix`: `check --fix` の印字し直しが本体を `=>` の次の行から
  同じ行へ移す。門番の期待値 `LET_VALUE_FIXED` は改行したままである。同じ
  `63-d` の `d/argument-fix` は土台でも本ラウンドの後でも緑である。

どちらも門番の期待値を書き換えずに直せない。門番を緩めない規約に従い、赤のまま
報告する。

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
（「括弧なしの `if true => …`」）はこの位置では通らない。

さらに §59 の文法では `branch` は `prefix_form`（`expression` の段）にしか
現れないので、被演算子の位置の `if(...)`（`if(…) + 1`・`if(…) |> f`）も文法には
現れない。§12.1 の文章だけが被演算子の `if(...)` を認めている。文法と文章が
揃っていない。仕様は変えていない。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §12.1 | 続きの無い `if(...)` は E100。`(` の閉じの `)` が `if` の閉じを兼ねる | 構文 | E100 | `crates/hado/tests/fmt_layout.rs` |
| §59 | `unary`・`application` は `prefix_form` を取らないので、裸の `if …` は被演算子と引数に置けない | 構文 | E100 | `crates/hado/tests/fmt_layout.rs` |
| §58.1 | `fmt` の印字し直した綴りが、動くプログラムの綴りでなければならない | 冗長 | — | `crates/hado/tests/fmt_layout.rs`・`scripts/verify-fmt-layout.sh` |

## 実測

### 最初の 6 つの位置（探針 `target/probe/positions.sh`）

各位置を独立した根に置く（`hado` は入力の親を根として配下の `.hd` を読むので、
1 根 1 ファイルにする）。直す前は 6 つとも、`run` は通り、`fmt` の後が E100 だった。

| 位置 | 入力 | `run` | `fmt` の後（直す前 → 直した後） | `check` |
|---|---|---|---|---|
| 二項 `-` の右辺 | `Int.to_string (100 - (if true => 59 \| 0))` | 41 | `100 - if(…)` → `100 - (if …)` | E100 → ok |
| 単項 `-` | `Int.to_string (- (if true => 41 \| 0))` | -41 | `-if(…)` → `-(if …)` | E100 → ok |
| 引数の中の単項 `-` | `f (- (if true => 42 \| 0))` | -42 | `f (-if(…))` → `f (-(if …))` | E100 → ok |
| `not` | `not (if true => false \| true)` | 1 | `not if(…)` → `not (if …)` | E100 → ok |
| `&&` の右辺 | `true && (if true => true \| false)` | 1 | 変化なし | E100 → ok |
| `<` の右辺 | `1 < (if true => 2 \| 0)` | 1 | 変化なし | E100 → ok |

直した後は 6 つとも `check` の exit 0、`run` の値は直す前と同じ、2 回目の `fmt` は
同じバイト列である（探針の出力の末尾）。

```
$ bash target/probe/positions.sh | tail -7
入力: def b = 1 < (if true => 2 | 0)~def main = if b => Int.to_string 1 | Int.to_string 0~
run(before): exit=0 out=1
fmt 後: def b = 1 < (if true => 2 | 0)~def main = if b => Int.to_string 1 | Int.to_string 0~
check: exit=0 out=
run(after): exit=0 out=1
fmt 2 回目後: def b = 1 < (if true => 2 | 0)~def main = if b => Int.to_string 1 | Int.to_string 0~
```

### 包まれた 6 つの位置（探針 `target/probe/wrapped.sh`）

`wrap` が `(…)` を足す位置。足した `)` の直前も「続きの無い位置」である。
直す前は 6 つとも `fmt` の後が E100 だった。

| 位置 | 入力 | `run` | `fmt` の後（直す前 → 直した後） | `check` |
|---|---|---|---|---|
| 包まれた引数の中の `-` | `f (- (if true => 1 \| 0)) 2` | 1 | `f (-if(…)) 2` → `f (-(if …)) 2` | E100 → ok |
| 包まれた左辺の二項 | `(1 + (if true => 1 \| 0)) * 2` | 4 | `(1 + if(…)) * 2` → 変化なし | E100 → ok |
| 包まれた引数の二項 | `f (1 + (if true => 1 \| 0)) 2` | 4 | `f (1 + if(…)) 2` → 変化なし | E100 → ok |
| 包まれた引数の `not` | `f (not (if true => false \| true)) 3` | 3 | `f (not if(…)) 3` → 変化なし | E100 → ok |
| 包まれた左辺の `\|>` | `(1 \|> (if true => inc \| dec)) + 1` | 3 | `(1 \|> if(…)) + 1` → 変化なし | E100 → ok |
| 包まれた入れ子の二項 | `(10 - (2 - (if true => 1 \| 0))) * 2` | 18 | `(10 - (2 - if(…))) * 2` → 変化なし | E100 → ok |

`f` は `def f (a: Int, b: Int): Int => a + b`、`not` の位置は
`def f (a: Bool, b: Int): Int => if a => b | 0`、`|>` の位置は
`def inc x => x + 1` / `def dec x => x - 1`。

```
$ bash target/probe/wrapped.sh | tail -7
=== w6 ===
入力: def main = Int.to_string ((10 - (2 - (if true => 1 | 0))) * 2)~
run(before): exit=0 out=18
fmt 後: def main = Int.to_string ((10 - (2 - (if true => 1 | 0))) * 2)~
check: exit=0 out=
run(after): exit=0 out=18
fmt 2 回目: 同じ
```

### 周辺の位置（探針 `target/probe/positions_all.sh`）

右端の被演算子は並置の引数だけではない。同じ規則が効く位置を先に赤の一覧として
書き出し、直した後に全部緑になることを確かめる。包まれて後ろに続きがある位置
（`wrapped` の 6 件）も含めて、`fmt` の後を示す。

```
$ bash target/probe/positions_all.sh | grep -v 入力が不正
def-body-bin             OK   def v = 1 + (if true => 1 | 0)~def main = Int.to_string v~
arg-bin                  OK   def main = Int.to_string (1 + (if true => 1 | 0))~
arg-bin-sub              OK   def main = Int.to_string (100 - (if true => 1 | 0))~
arg-bin-and              OK   def b = true && (if true => true | false)~def main = Int.to_string (if b => 1 | 0)~
arg-bin-lt               OK   def b = 1 < (if true => 1 | 0)~def main = Int.to_string (if b => 1 | 0)~
arg-neg                  OK   def main = Int.to_string (-(if true => 1 | 0))~
arg-not                  OK   def b = not (if true => true | false)~def main = Int.to_string (if b => 1 | 0)~
nested-bin-bin           OK   def main = Int.to_string (1 + (2 - (if true => 1 | 0)))~
nested-bin-neg           OK   def main = Int.to_string (1 + -(if true => 1 | 0))~
list-elem-bin            OK   def v = [1 + (if true => 1 | 0)]~def main = Int.to_string (List.length v)~
list-elem-neg            OK   def v = [-(if true => 1 | 0)]~def main = Int.to_string (List.length v)~
binding-body-bin         OK   def main = 1 as y ->~  Int.to_string (y + (if true => 1 | 0))~
binding-value-bin        OK   def main = 1 + if(true => 1 | 0) as y ->~  Int.to_string y~
pipe-right               OK   def inc x => x + 1~def dec x => x - 1~def main = Int.to_string (1 |> (if true => inc | dec))~
callee-if                OK   def inc x => x + 1~def dec x => x - 1~def main = Int.to_string (if(true => inc | dec) 1)~
record-field-bin         OK   type R = { a: Int }~def r = { a = 1 + (if true => 1 | 0) }~def main = Int.to_string r.a~
call-arg-bin             OK   def main = Int.to_string (Option.unwrap_or Some(1 + (if true => 1 | 0)) 0)~
fn-body                  OK   def main = Int.to_string (fn(x => x + (if true => 1 | 0)) 0)~
fn-last-arg              OK   def main = Int.to_string (List.length (List.map [1] fn(x => x + (if true => 1 | 0))))~
field-base               OK   type R = { a: Int }~def main = Int.to_string if(true => { a = 1 } | { a = 2 }).a~
```

括弧を外さない位置（`if(...)` のままで受理される）も変わらない。`binding-value-bin`
（`as` が続く）・`callee-if`（呼び手の位置、探針の古いラベル `pipe-left` を直した）・
`field-base`（`.a` が続く）の 3 つである。包まれて後ろに続きがある位置は、上の
「包まれた 6 つの位置」で `run` の値を含めて測った。

### 赤のテスト

`crates/hado/tests/fmt_layout.rs` に 2 つのテストを足した。直す前の
`rightmost_operand_branches_survive_the_reprint` は `5 passed; 2 failed`。
`wrapped_operand_branches_survive_the_reprint` は包みを直す前の印字器
（`c337f37` の `crates/hado/src/postfix_print.rs`）に対して `0 passed; 1 failed`。

```
$ cargo test --offline --test fmt_layout wrapped_operand_branches_survive_the_reprint
wrapped-neg-argument: t:2:31: error[E100]: `if(...)` with nothing after it; write it without parentheses; canonical form: if true => 1 | 0:
def main = Int.to_string (f (-if(true => 1 | 0)) 2)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 7 filtered out
```

直した後。

```
$ cargo test --offline --test fmt_layout
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
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

### `verify-r63.sh` の赤（土台でも同じ、`2/4`）

```
$ bash scripts/verify-r63.sh
OK: 63-a two Boolean arms become a Boolean branch
OK: 63-b an arm and a fallback become a Boolean branch
AssertionError: ('c/mixed', 'E520', 'main.hd:255:1: error[E402]: not all constructor values are covered')
NG: 63-c E503 and E520 settle together
AssertionError: ('d/let-fix', "'def f (b: Bool): Int => if(b => 1 | 2) as v ->\\n  v + 1\\n…'",
                              "'def f (b: Bool): Int =>\\n  if(b => 1 | 2) as v ->\\n  v + 1\\n…'")
NG: 63-d the exclusions and the argument position
PASSED: 2/4
```

`63-c` の `c/mixed` は ADT の実装と名前の同一視の欠陥（別レーン）、`63-d` の
`d/let-fix` は F5 の印字し直しで古くなった期待値である。土台（`d852e77` の
印字器）でも同じ 2 件が赤になる。`d/argument-fix` は土台でも本ラウンドの後でも
緑である。

```
$ git checkout d852e77 -- crates/hado/src/postfix_print.rs
$ bash scripts/verify-r63.sh | tail -2
NG: 63-d the exclusions and the argument position
PASSED: 2/4
```

### ラウンド最後の全走

（下に追記する。）
