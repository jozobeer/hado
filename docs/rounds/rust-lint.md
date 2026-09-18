# Rust の整形と lint（rust-lint）

## 手順書

### 背景

- 門番は層別済みで（`docs/plan.md` 33・34 節）、文書の整合は `scripts/lint-docs.sh` が持ち、
  CI が PR でも回す（`docs/rounds/ci.md`）。**Rust の整形と lint を守る門番は無い。**
- 実測: `cargo fmt --all --check` は `crates/` の Rust 96 ファイル中 86 ファイルで差分を出す。
  `cargo clippy --offline --all-targets` は lib だけで 113 警告、全 target で `result_large_err`
  204 件（lib と lib test の重複を含む）・`needless_borrow` 9 件はじめ計 230 件の警告を出し、
  `mistyped_literal_suffixes`（correctness、既定で deny）が `crates/hado/tests/r26_effects.rs:184`
  でコンパイルを落とす。
- `parser::Diagnostic` は 128 バイトで `result_large_err` の閾値ちょうどである。`Related` は
  40 バイトなので、`related: Option<Related>` を `Option<Box<Related>>` にすると 96 バイトに
  なり、204 件が 1 箇所の構造の変更で消える。
- `AGENTS.md` の環境は Rust・`cc`・`libgc-dev` を挙げるが、rustfmt と clippy を挙げていない。

### 決定

1. 整形は rustfmt の既定に従う（`rustfmt.toml` を置かない）。木全体に 1 回通し、以後は門番が守る。
2. `result_large_err` は allow せず、`Diagnostic.related` を `Option<Box<Related>>` にして消す。
   閾値は型の大きさで決まるので、1 箇所の箱化で 204 件が消える。
3. 残る lint はコードを直す。`too_many_arguments`（3 件）だけは `fixes.rs` の既存 8 箇所と
   同じ項目の allow を足す。`type_complexity`（2 件）は型別名で消す。
4. 門番を 2 本足す。`scripts/verify-rustfmt.sh`（quick）が `cargo fmt --all --check` を、
   `scripts/verify-clippy.sh`（core）が `cargo clippy --offline --all-targets -- -D warnings` を
   回す。整形は保存ごと、lint はコミット前である。
5. 整形と lint の修正は振る舞いを変えない。直す前後で `cargo test` の判定が同じであることを測る。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| `cargo fmt --all --check` | 差分なしで exit 0 | 決定 1 |
| `cargo clippy --offline --all-targets -- -D warnings` | 警告 0 で exit 0 | 決定 4 |
| `Diagnostic` の大きさ | 128 → 96 バイト | `Related` 40・`Option<Box<_>>` 8。決定 2 |
| 整形・lint 修正の前後の `cargo test` | 同じ判定 | 決定 5 |
| 門番の層 | rustfmt は quick、clippy は core | 決定 4 |

### 成果物

`scripts/verify-rustfmt.sh`（新設）、`scripts/verify-clippy.sh`（新設）、`docs/rounds/rust-lint.md`
（本ファイル）、木全体の整形、`parser.rs`・`ambiguous_declarations.rs`・`names.rs` の
`Diagnostic.related` の箱化、`fixes.rs`・`project.rs`・`mono.rs` ほかの lint 修正、`README.md`、
`AGENTS.md`。

### 工程

- 工程 1: `cargo fmt --all` を木全体に通し、振る舞いを変えないコミットにする。
- 工程 2: `Diagnostic` を箱化し、残る lint を直す。
- 工程 3: 門番 2 本と `# tier:`・`# round:` の宣言行、README と AGENTS。
- 工程 4: quick 層・core 層・`scripts/lint-docs.sh`・`scripts/corpus-lock.sh --check`。
  否定的対照で赤を測る。

### 判定

- rl-a. `cargo fmt --all --check` が差分なしで終わる。
- rl-b. `cargo clippy --offline --all-targets -- -D warnings` が警告 0 で終わる。
- rl-c. `bash scripts/verify-rustfmt.sh` が `PASSED: 1/1`。
- rl-d. `bash scripts/verify-clippy.sh` が `PASSED: 1/1`。
- rl-e. quick 層と core 層が緑（`TALLY: n/n passed; red: none`）。
- rl-f. `bash scripts/lint-docs.sh` が `PASSED: n/n`。
- rl-g. `bash scripts/corpus-lock.sh --check` が差分なし。
- rl-h. 否定的対照（整形を 1 箇所崩す、lint を 1 つ戻す）で対応する門番が赤になる。

### 許容される赤

工程 4 で本ファイルの `## 実測` を書くまで `scripts/lint-docs.sh` 検査 6 は赤でよい。
**終了時に許容される赤は無い。**

### スコープ外

言語機能、既存の門番の判定条件、`tests/`（コーパス）・`stdlib/`・`runtime/` の変更、
rustfmt・clippy の設定ファイル（`rustfmt.toml`・`clippy.toml`・`Cargo.toml` の `[lints]`）の
新設、CI の層の割り当て（`docs/rounds/ci.md` の決定）。

### 中断の条件

整形か lint の修正が `cargo test` の判定を変えるなら、allow で隠さずに止めて報告せよ。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | Rust の整形は保存ごと、lint はコミット前に回し、木全体を守る | 道具 | 成功 | `scripts/verify-rustfmt.sh`・`scripts/verify-clippy.sh` |

## 実測

```
# 直す前（origin/main 53f61ce の worktree）
$ cargo fmt --all --check | grep '^Diff in' | sed -E 's/:[0-9]+:$//' | sort -u | wc -l
86        # crates/ の Rust は 96 ファイル
$ cargo clippy --offline --all-targets -j 2
（exit 101。lint 別の件数: result_large_err 204、needless_borrow 9、too_many_arguments 3、
  useless_format 2、unnecessary_map_or 2、type_complexity 2、unusual_byte_groupings 1、
  replace_box 1、question_mark 1、needless_lifetimes 1、mistyped_literal_suffixes 1、
  manual_is_multiple_of 1、iter_overeager_cloned 1、cloned_ref_to_slice_refs 1）
error: mistyped literal suffix
   --> crates/hado/tests/r26_effects.rs:184:21
    |
184 |     let mut state = 0x26_32;
    |                     ^^^^^^^ help: did you mean to write: `0x0026_i32`
```
