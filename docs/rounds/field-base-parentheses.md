# 適用を field の base にするとき括弧で囲む（field-base-parentheses）

## 手順書

### 背景

外部エージェント（`app/cursor`）の PR #92 を記録する。`docs/issues.md:7` が求める
ラウンドファイルが切られていなかったので、着地後に PR 本文（`gh pr view 92`）と
審査 `.worktrees/pr92/target/reviews/pr92-review.md` から書く。着地は `b6f7126`
（`Merge pull request #92`）、変更は `daf8ce0`（テスト）・`2c004d6`（実装）・
`c335898`（記録と issue の削除）。

`fmt` / `check --fix` / `build --fix` が `(f 1).a` を `f 1.a` に印字し直し、動いていた
プログラムを壊す。`.a` が整数リテラルに付くと E103（浮動小数の字句）、
`(f (g 1)).a` なら field が最後の引数に付いて意味が変わる。`run` は `1` だが、
`fmt` の後は `def v = f 1.a` になり
`error[E103]: float must have digits after the '.'` になる。

### 原因

§7 で `.` は適用より強い。`crates/hado/src/postfix_print.rs` の `field_text` は base を
適用と同じ優先度で印字し、base が `Apply` のとき括弧が付かない。PR #76（F5）が `fmt`
に全体の印字し直しを入れてから、この欠陥が利用者のソースを壊す。

### 決定

- base が `Apply` / `Call` / 整数のとき、base 全体を括弧で囲む。括弧の中は続きの無い
  位置として印字する（右端の分岐が `(if …)` になる）。`1.a` は浮動小数の字句にも
  読まれる。
- 変えるのは `crates/hado/src/postfix_print.rs` の `field_text` だけである。

### 成果物

- `crates/hado/src/postfix_print.rs` の `field_text`。
- `crates/hado/tests/fmt_layout.rs` の `field_after_application_keeps_the_base_grouped`。
- `docs/rounds/tail-operand-branch-parentheses.md` の追記（`Field` の base の欠陥を
  「切り出して後から直した」と記録）と、`docs/issues/parenthesize-field-base-after-application.md`
  の削除。
- 本ファイル。

### 工程

1. 赤のテスト（`daf8ce0`）: `(f 1).a`・`(f (g 1)).a`・`(f (if …)).a` が `fmt` の前後で
   同じ値を印字することを測る。
2. `field_text` で base を括弧で囲む（`2c004d6`）。
3. 直った issue を消し、切り出し元のラウンドに経緯を残す（`c335898`）。
4. main を取り込み、`fmt_layout`・`verify-fmt-layout`・`lint-docs` を回す。

### 判定

- `fmt` の前後で `run` の値が同じ（`(f 1).a` は `1`）。
- `cargo test --offline --test fmt_layout` が 11 passed。
- `bash scripts/verify-fmt-layout.sh` が 6/6。
- `bash scripts/lint-docs.sh` が 16/16。

### スコープ外

- `Call`（`B(x)` のような構築子の括弧呼び出し）を括弧の対象に含めたこと。構文解析では
  `B(x).a` はもともと `Field(Call)` として読まれ、括弧は要らない。`crates/hado/src/parser.rs`
  の `canonical_field` は `Apply` と整数だけを囲むので、2 つの印字器で判定が食い違う。
  余分な括弧が付き、診断の列もずれる（審査の欠陥 1）。
  `docs/issues/field-base-call-over-parenthesized.md` に切り出した。
- §7 が、構築子の括弧呼び出し `B(x)` と閉じの付いた `if(...)` を「適用」に含めるかを
  定めていないこと。仕様は変えていない（審査「仕様への疑義」）。
- `docs/rounds/tail-operand-branch-parentheses.md` の「切り出して後から直した」という
  追記が `docs/issues.md:45`（経緯は git 履歴が持つ）と食い違うこと。審査の指摘で、
  記録の書き方の規約なので人が決める。本ラウンドでは文書を変えていない。

### 中断の条件

なし（着地済み）。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §7 | `.` は適用より強い。base が適用のときは括弧で囲む | 構文 | E103 | `crates/hado/tests/fmt_layout.rs`・`scripts/verify-fmt-layout.sh` |
| §58.1 | 印字し直した形が正準形で、意味を変えない | 冗長 | — | 同上 |

## 実測

### `fmt` が `(f 1).a` を壊さない（main `b6f7126` の worktree で 2026-10-13 に再測）

```
$ ./target/debug/hado run target/pr92/a/main.hd; echo "run=$?"
1
run=0
$ ./target/debug/hado fmt target/pr92/a/main.hd; echo "fmt=$?"
fmt=0
$ cat target/pr92/a/main.hd
type R = { a: Int }
def f (x: Int): R => { a = x }
def v = (f 1).a
def main = Int.to_string v
$ ./target/debug/hado run target/pr92/a/main.hd; echo "run=$?"
1
run=0
$ ./target/debug/hado check target/pr92/a/main.hd; echo "check=$?"
check=0
```

`fmt` は 1 バイトも変えない（`fmt=0`、出力なし）。審査の表では、base（`349efa7`）は
`fmt` の後が `def v = f 1.a` になり E103、枝は `(f 1).a` のまま `1` だった。
`(f (g 1)).a`・`(f 1).a + 1`・`(f 1).b.a`・`(f 1).a |> g`・`((f 1)).a`・`(f 1 2).a`
も枝では括弧が残り、2 回目の `fmt` は全ケースで不動点だった（審査）。

### validation（PR 本文）

- `fmt_layout::field_after_application_keeps_the_base_grouped` が `(f 1).a`・
  `(f (g 1)).a`・`(f (if …)).a` の `fmt` 前後で `1` を印字する。
- `cargo test --offline --test fmt_layout` が 11 passed。
- `bash scripts/verify-fmt-layout.sh` が 6/6（コーパス 188 ファイルは 1 バイトも
  変わらない）。
- `cargo fmt --all --check`・`cargo clippy --offline --all-targets -- -D warnings`・
  `bash scripts/lint-docs.sh` が緑。

### 審査で残った欠陥（軽微、マージは妨げない）

`crates/hado/src/postfix_print.rs` の `field_text` が `NodeKind::Call(..)` も括弧で囲む。
構文解析では `B(x).a` はもともと `Field(Call)` として読まれ、括弧は要らない。
`crates/hado/src/parser.rs` の `canonical_field` は `Apply` と整数だけを囲むので、2 つの
印字器で判定が食い違う。影響は型が通らないプログラムだけで、受理コーパスには出てこない。
審査は `NodeKind::Call(..)` を対象から外すことを推奨している。

```
$ printf 'type R = { a: Int }\ntype B = | B(R)\ndef v = B({ a = 1 }).a\ndef main = "x"\n' > target/pr92/call/main.hd
$ ./target/debug/hado fmt target/pr92/call/main.hd
fmt: main.hd (1 rewrites)
$ cat target/pr92/call/main.hd
type R = { a: Int }
type B = | B(R)
def v = (B({ a = 1 })).a
def main = "x"
$ ./target/debug/hado check target/pr92/call/main.hd; echo "check=$?"
main.hd:3:10: error[E305]: field access requires a single record constructor
check=1
```

`fmt` の前の診断は `main.hd:3:9` で、余分な括弧の分だけ列が 1 つずれる。

`Call` を足した変更を測るテストは無い。テストがあれば、この余分な括弧で赤くなっていた
（審査「テストの評価」）。印字の不動点は保たれている。`print-postfix` を 2 回かけた
出力は base・枝とも差分 0、元のソースと 1 回目の出力も accept・stdlib で差分 0、base と
枝の 1 回目の出力どうしも差分 0 である（審査）。
