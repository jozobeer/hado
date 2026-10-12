# 印字器と mono の認知複雑度を 7 以下に戻す（printer-cognitive-complexity）

## 手順書

### 背景

main `4baca98` で `bash scripts/verify-cognitive-complexity.sh` が赤である。
`clippy.toml` の `cognitive-complexity-threshold = 7` を `--lib --bins` に
`-F clippy::cognitive_complexity` で掛けたとき、4 関数が閾値を超える。

| 関数 | 複雑度 | 入った時期 |
|---|---|---|
| `crates/hado/src/postfix_print.rs` の `place_comments` | 16 | F5 `350a575` で新設 |
| `crates/hado/src/postfix_print.rs` の `apply_text` | 8 | F5 `350a575`（先頭の `if`/`else` が加わった） |
| `crates/hado/src/mono.rs` の `lift_again_arguments` | 9 | `0408ca2`（PR #78。callee を子に載せる分岐が入った） |
| `crates/hado/src/mono.rs` の `lambda` | 9 | `bf1310c`（F5 より前）から赤 |

`.worktrees/pre-f5`（`bf1310c`）で回すと `PASSED: 2/3`・エラー 1 件で、その
1 件は `lambda`（9/7）である。`lift_again_arguments` は当時 7 以下で、
`0408ca2` の callee の分岐で赤になった。`postfix_print.rs` の 2 件も
`bf1310c` には無い（`place_comments` は存在せず、当時の `apply_text` は先頭の
`if`/`else` が無いので 4 以下）。`bf1310c..4baca98` で
`crates/hado/src/mono.rs` を触るコミットは `0408ca2` だけである。

### 決定

1. 4 関数を分割して 7 以下にする。閾値（`clippy.toml`）も `#[allow]` も使わない。
2. **振る舞いを変えない。** `postfix_print.rs` は印字結果が 1 バイトも変わらない
   ことを測る。`mono.rs` は分岐・順序・短絡を同じに保つ。
3. `cargo fmt --all` を当てる。
4. 仕様（`docs/spec.md`）・コーパス（`tests/accept` `tests/reject` `stdlib/`）・
   凍結文書（`docs/plan.md` `docs/ledger.md` `docs/spec-trace.md`）は触らない。
   門番も門番の宣言も足さない（`scripts/verify-cognitive-complexity.sh` が
   既にこの不変を守っている）。

### 分割の境界

- `place_comments`: 空行の連なりを `blank_runs`、連続コメントの塊を
  `comment_blocks`、空行の挿入位置を `blank_insert_position`、コメントの塊の
  挿入位置を `comment_insert_position`、後ろからの差し込みを `insert_collected`
  に分けた。印字の位置と元ソースの行による安定ソート、後ろから入れる順、
  `lines.len()` で頭打ちにする点は変えていない。
- `apply_text`: 引数 1 つの印字（適用と大文字の名前参照を括弧で包む）を
  `apply_argument` に切り出した。
- `lift_again_arguments`: `result.kind` から ANF の `let` に載せる子を選ぶ
  `match` を `again_children` に切り出した。借用（`Vec<&mut Node>`）と腕の
  並び、`§38` の `With` の扱いはそのままである。
- `lambda`: 本体の型を宣言側の結果型に合わせる `else if` 連鎖を
  `lambda_result` に移した。outcome の成功型の照合を `lambda_outcome_result`、
  「宣言側の行が外側にも効果を載せているか」を `wraps_declared_result` に
  分けた。`self.done` で包むのは宣言側が outcome のときだけで、`logical_argument`
  は前段が偽のときだけ評価する（`||` の短絡を保つ）。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| `bash scripts/verify-cognitive-complexity.sh` | `PASSED: 3/3` | 決定 1（`clippy.toml` の 7 と allow 無しを含む 3 検査） |
| `bash scripts/verify-fmt-layout.sh` | `PASSED: 6/6` | F5 の門番（印字し直しの同一・冪等・コメント保持）を緩めない |
| `cargo test --offline --test fmt_layout` | 0 failed | 同上 |
| `cargo test --offline --test r26_effects` | 0 failed | 効果と outcome の判定を変えない |
| `cargo clippy --offline --all-targets -- -D warnings` | 警告 0 | 既存の core 門番 |
| `cargo fmt --all --check` | 差分なし | 既存の quick 門番 |
| `print-postfix` の出力 | 前後で同じ | 決定 2 |

### 成果物

`crates/hado/src/postfix_print.rs` と `crates/hado/src/mono.rs` の分割、
`docs/rounds/printer-cognitive-complexity.md`（本ファイル）。

### 工程

1. `4baca98` の worktree で 4 件の赤を再現する。
2. 4 関数を分割し、`cargo fmt --all` を当てる。
3. 分割の前後で `print-postfix` の出力を突き合わせる。
4. `verify-cognitive-complexity`・`verify-fmt-layout`・`fmt_layout`・
   `r26_effects` を回す。
5. `cargo clippy --all-targets`・`cargo fmt --check`・`cargo test` の全走を回す。
6. 行為（`refactor:`）と記録（`docs:`）を分けてコミットする。

### 判定

- pc-a. `bash scripts/verify-cognitive-complexity.sh` が `PASSED: 3/3`。
- pc-b. `bash scripts/verify-fmt-layout.sh` が `PASSED: 6/6`。
- pc-c. `cargo test --offline --test fmt_layout` が 0 failed。
- pc-d. `cargo test --offline --test r26_effects` が 0 failed。
- pc-e. `cargo clippy --offline --all-targets -- -D warnings` が警告 0。
- pc-f. `cargo fmt --all --check` が差分なし。
- pc-g. `print-postfix <root> <out>` の出力が分割の前後で同じ（`tests/accept`・
  `stdlib`・`tools`・`tests/library`・`tests/reject`・`tests/postfix-close`）。
- pc-h. `cargo test --offline` が 0 failed。

### 許容される赤

無い。

### スコープ外

言語機能、`docs/spec.md`、`tests/`・`stdlib/`・`runtime/`、`clippy.toml`、
`scripts/verify-cognitive-complexity.sh`、新しい門番、`docs/plan.md`・
`docs/ledger.md`・`docs/spec-trace.md` の改訂。

### 中断の条件

分割が `print-postfix` の出力を変えるなら差分を戻して報告する。分割で
`cargo test` の判定が変わるなら allow で隠さずに止めて報告する。同じ関数を
2 度分割しても 7 を割らないなら閾値の判断を人に上げて止まる。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 本番のコードは認知複雑度 7 以下に保つ | 道具 | 成功 | `clippy.toml`・`scripts/verify-cognitive-complexity.sh` |

## 実測

### 赤の再現（`4baca98` の worktree、分割前）

```
$ bash scripts/verify-cognitive-complexity.sh
OK: clippy.toml pins cognitive-complexity-threshold = 7
error: the function has a cognitive complexity of (16/7)
   --> crates/hado/src/postfix_print.rs:165:4
165 | fn place_comments(ast: &Ast, canonical: String) -> String {
error: the function has a cognitive complexity of (8/7)
   --> crates/hado/src/postfix_print.rs:774:4
774 | fn apply_text(f: &Node, args: &[Node], indent: usize) -> String {
error: the function has a cognitive complexity of (9/7)
   --> crates/hado/src/mono.rs:776:8
776 |     fn lift_again_arguments(&mut self, node: &crate::parser::Node) -> Option<crate::parser::Node> {
error: the function has a cognitive complexity of (9/7)
    --> crates/hado/src/mono.rs:5049:8
5049 |     fn lambda(
NG: cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity is clean
OK: no #[allow] / #![allow] for cognitive_complexity under crates/hado/src
PASSED: 2/3
# exit 1
```

### F5 より前の赤（`.worktrees/pre-f5`、`bf1310c`）

```
$ cd .worktrees/pre-f5 && bash scripts/verify-cognitive-complexity.sh
OK: clippy.toml pins cognitive-complexity-threshold = 7
error: the function has a cognitive complexity of (9/7)
    --> crates/hado/src/mono.rs:5038:8
5038 |     fn lambda(
error: could not compile `hado` (lib) due to 1 previous error
NG: cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity is clean
OK: no #[allow] / #![allow] for cognitive_complexity under crates/hado/src
PASSED: 2/3
# exit 1
```

### 4 関数の複雑度（分割後）

`clippy.toml` は 7 のままにしたいので、閾値 1 の対照を `target/` の中だけに
置き、`CLIPPY_CONF_DIR` で読ませて 7 以下の関数も数える。`target/` は追跡外
なのでリポジトリの `clippy.toml` は変わらない。

```
$ mkdir -p target/cc-conf && printf 'cognitive-complexity-threshold = 1\n' > target/cc-conf/clippy.toml
$ CLIPPY_CONF_DIR="$PWD/target/cc-conf" cargo clippy --offline --lib --bins \
    --message-format short -- -W clippy::cognitive_complexity 2>&1 \
    | grep -E 'postfix_print.rs:(165|805|822):|mono.rs:(795|841|5074):'
crates/hado/src/postfix_print.rs:165:4: warning: the function has a cognitive complexity of (4/1)
crates/hado/src/postfix_print.rs:805:4: warning: the function has a cognitive complexity of (4/1)
crates/hado/src/postfix_print.rs:822:4: warning: the function has a cognitive complexity of (5/1)
crates/hado/src/mono.rs:795:8: warning: the function has a cognitive complexity of (6/1)
crates/hado/src/mono.rs:841:8: warning: the function has a cognitive complexity of (4/1)
crates/hado/src/mono.rs:5074:8: warning: the function has a cognitive complexity of (6/1)
```

| 関数 | 分割前 | 分割後 |
|---|---|---|
| `postfix_print::place_comments` | 16 | 4 |
| `postfix_print::apply_text` | 8 | 4 |
| `postfix_print::apply_argument`（新設） | — | 5 |
| `mono::lift_again_arguments` | 9 | 6 |
| `mono::again_children`（新設） | — | 4 |
| `mono::lambda` | 9 | 6 |

`mono::lambda_result` と `lambda_outcome_result`・`wraps_declared_result` は
閾値 1 でも報告されない。clippy の認知複雑度は本体が必ず `return` する `if`
（早期 return の門）を数えないためで、計測対象から外れているのではない。
確かめるために `lambda_result` の先頭へ入れ子の `if` を 3 つ注入すると
`(3/1)` として報告された。分割後の実測値は 1 以下である。

### 印字のバイト同一（pc-g）

分割の前後で `print-postfix` の出力を比べる。前は `4baca98` の 2 ファイルを
stash で戻してから建てたバイナリで測る。

```
$ git stash push -m hado-anf-check -- crates/hado/src/postfix_print.rs crates/hado/src/mono.rs
$ cargo build --offline -q --bin hado
$ for r in tests/accept stdlib tools tests/library tests/reject tests/postfix-close; do
>     ./target/debug/hado print-postfix "$r" "target/anf/before/$r" >/dev/null; done
$ git stash pop
$ cargo build --offline -q --bin hado
$ for r in tests/accept stdlib tools tests/library tests/reject tests/postfix-close; do
>     ./target/debug/hado print-postfix "$r" "target/anf/after/$r" >/dev/null; done
$ echo "before=$(find target/anf/before -type f | wc -l) after=$(find target/anf/after -type f | wc -l)"
before=300 after=300
$ diff -r target/anf/before target/anf/after && echo IDENTICAL
IDENTICAL
```

### 触ったところの門番とテスト

```
$ bash scripts/verify-cognitive-complexity.sh
OK: clippy.toml pins cognitive-complexity-threshold = 7
OK: cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity is clean
OK: no #[allow] / #![allow] for cognitive_complexity under crates/hado/src
PASSED: 3/3

$ bash scripts/verify-fmt-layout.sh
OK: a: 配置の違う 2 つの綴りが同じバイト列になる
OK: b: コーパス 188 ファイルが印字し直しで変わらない
OK: c: 本体の中のコメントが定義の中に残り、本体の前に来る
OK: d: 空行が残る
OK: e: コメントを落とす変異で b が赤になる（6 ファイル）
OK: f: 空の本体を持つ implement が印字し直しで増えない
PASSED: 6/6

$ cargo test --offline --test fmt_layout
running 4 tests
test comments_stay_before_and_inside_their_definition ... ok
test formatting_is_idempotent_and_settles ... ok
test flat_and_ragged_become_the_same_bytes ... ok
test corpus_stdlib_and_tools_are_canonical_and_idempotent ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --offline --test r26_effects
test result: ok. 92 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 着地の判定（pc-e・pc-f・pc-h）

```
$ cargo clippy --offline --all-targets -- -D warnings; echo exit=$?
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.78s
exit=0

$ cargo fmt --all --check; echo exit=$?
exit=0

$ cargo test --offline > target/full-test.log 2>&1; echo exit=$?
exit=0
$ grep -c 'test result' target/full-test.log
75
$ grep -c 'test result: FAILED' target/full-test.log
0
$ awk '/^test result/ {p+=$4; f+=$6} END {print "passed="p" failed="f}' target/full-test.log
passed=824 failed=0
```

### 測れなかった対照

`mono.rs` の分割は `dump-mono` の出力でも比べようとしたが、同じバイナリで
2 回走らせても `Switch` の腕の順が入れ替わる（`$Yield#1` と `$Yield#2`）。
`cargo test --offline` が参照する順序はこの出力ではないので、`mono.rs` の
振る舞いは全走（824 passed / 0 failed）と `r26_effects`（92 passed）で見る。

### 着地後の全走（2026-10-13、main `b6f7126`）

main を checkout した側（`.worktrees/postlanding-main`）で回した。full 層（quick・
core・full の累積に文書層を足した 84 本）は `TALLY: 3894/3894 passed; red: none` で
赤は 1 本も無い。`cargo test --offline` は full 層の門番 `verify-m0` の検査 2 が
`cargo-test passed=850` で緑である（終了コード 0、`FAILED` は 0）。quick 層の門番
6 本は full 層の走行に含まれ、すべて緑である。

```
$ bash scripts/run-gates.sh --tier full target/.gates-full | tail -1
TALLY: 3894/3894 passed; red: none
$ ./target/debug/hado run tools/gates/main.hd -- target/.gates-full
3894/3894 passed; red: none
$ grep cargo-test target/.gates-full/verify-m0.log
OK: 2 cargo-test passed=850
$ for f in verify-ci verify-gate-cache verify-m1-spec verify-repl verify-rustfmt verify-spec-model; do tail -1 target/.gates-full/$f.log; done
PASSED: 8/8
PASSED: 26/26
PASSED: 1/1
PASSED: 10/10
PASSED: 2/2
PASSED: 3/3
$ tail -1 target/.gates-full/verify-cognitive-complexity.log
PASSED: 3/3
$ tail -1 target/.gates-full/verify-fmt-layout.log
PASSED: 6/6
```

`verify-cognitive-complexity` は分割後も 3/3 で、閾値 7 は変えていない。印字のバイト同一も保たれている。
