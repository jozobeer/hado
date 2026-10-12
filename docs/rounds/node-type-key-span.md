# 節点の型表を span の範囲で鍵にする（node-type-key-span）

## 手順書

### 背景

外部エージェント（`app/cursor`）の PR #90 を記録する。`docs/issues.md:7` が求める
ラウンドファイルが切られていなかったので、着地後に PR 本文（`gh pr view 90`）と
審査 `.worktrees/pr90/target/reviews/pr90-review.md` から書く。着地は `349efa7`
（`Merge pull request #90`）、変更は `c8cfc2e`（実装）と `d9a7ad9`（テスト）。

`check` は通るのに `run` / `dump-mono` がエラーコードの無い `Mismatch` で終了 1 に
なる。入れ子の適用（カリーした関数を返して呼ぶ `(get 1) 40`）と、最上位の record 値の
field 参照（`r.a`）という常出の形で起きる。局所の `as` 束縛なら通るので気づきにくい。

### 原因

`crates/hado/src/lib.rs` の型表（`value_types`・`composite_types`・`with_types`）の鍵が
`(span.line, span.col)` だけだった。`crates/hado/src/parser.rs` の `parse_group` は
括弧を落とすので、入れ子の `Apply` は内側の callee と開始位置を共有する。`Field` も
base の `Name` と開始位置を共有する。後から書いた節点の型が先の節点を上書きし、単相化が
違う型で特殊化して `Mismatch` になる。`Span` は終端（`end_line`・`end_col`）を既に
持っている。

### 決定

- 鍵を開始位置から `Span` 全体へ移す。`crates/hado/src/lib.rs` に
  `pub type NodeTypeKey = parser::Span` を置き、`value_types`・`composite_types`・
  `with_types` の鍵にする。元の鍵より細かくなるだけなので、新しく衝突は増えない。
- functor の実体化で行をずらす箇所（`crates/hado/src/infer/functors.rs`）は、合成 AST と
  同じく `line` だけをずらす。型表の写しと揃える。
- 鍵の型を変える射程は `crates/hado/src/infer.rs`・`crates/hado/src/infer/named.rs`・
  `crates/hado/src/dependencies.rs`・`crates/hado/src/lexer.rs`・`crates/hado/src/mono.rs`
  に及ぶ。

### 成果物

- `crates/hado/src/lib.rs`・`crates/hado/src/infer.rs`・`crates/hado/src/infer/functors.rs`・
  `crates/hado/src/infer/named.rs`・`crates/hado/src/dependencies.rs`・
  `crates/hado/src/lexer.rs`・`crates/hado/src/mono.rs`。
- `crates/hado/tests/mono.rs` の
  `nested_apply_and_toplevel_field_keep_distinct_inferred_types`、
  `crates/hado/tests/run_cli.rs` の `run_nested_application_of_a_returned_closure`・
  `run_projects_a_toplevel_record_field`。
- 本ファイル。

### 工程

1. 赤のテスト（`d9a7ad9`）: `mono.rs` と `run_cli.rs` に 3 本を足す。
2. 鍵を `Span` にする（`c8cfc2e`）。
3. main を取り込み、`cargo test --offline`・clippy・fmt を回す。

### 判定

- `(get 1) 40` が `41`、`r.a` が終了コード 41、`(g inc) 1` が `2` になる。
- `cargo test --offline` が 0 failed。
- `cargo fmt --all --check` と `cargo clippy --offline --all-targets -- -D warnings` が 0 件。

### スコープ外

- `40 |> (get 1)` が base でも枝でも E301 になること。原因は `parse_group` が括弧を
  落とすことで、§7 が右辺を括弧で囲んだ場合を定めていない。仕様は変えていない
  （審査 §6）。
- `crates/hado/src/mono.rs` の `again_result_type` のコメントが「pipe と左辺は span を
  共有する」と書くが、`Pipe` の span は `|>` から始まる。この PR が触った箇所ではない
  （審査 §7）。

### 中断の条件

なし（着地済み）。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §7 | 適用は書かれた順に評価し、`(get 1) 40` は内側の適用の結果を呼ぶ | 単相化 | — | `crates/hado/tests/mono.rs`・`crates/hado/tests/run_cli.rs` |

## 実測

### 直ったこと（main `b6f7126` の worktree で 2026-10-13 に再測）

審査の記録では、base（`1b8a92a`）は `(get 1) 40` も `r.a` も `Mismatch` で終了 1
だった（`check` はどちらも終了 0）。着地後の木では次のとおりである。

```
$ ./target/debug/hado check target/pr90/a/main.hd; echo "check=$?"
check=0
$ ./target/debug/hado run target/pr90/a/main.hd; echo "run=$?"
41
run=0
$ ./target/debug/hado run target/pr90/b/main.hd; echo "run=$?"
run=41
$ ./target/debug/hado run target/pr90/c/main.hd; echo "run=$?"
2
run=0
```

`target/pr90/a/main.hd` は `def get x => fn(y => y + x)` と
`def main = Int.to_string ((get 1) 40)`、`b/main.hd` は `type R = { a: Int }` と
`def r = { a = 41 }` と `def main = r.a`、`c/main.hd` は `def g x => x` と
`def inc x => x + 1` と `def main = Int.to_string ((g inc) 1)`。

### validation（PR 本文）

- 上の 2 つの入力と `(g inc) 1` が `dump-mono` / `run` で期待どおりの値になる。
- `cargo test --offline`（`r26_effects` の RSS 計測は環境に `/usr/bin/time` が無いと
  赤だが、入れたあとは緑）。
- `cargo clippy --offline --all-targets -- -D warnings`・`cargo fmt --all --check`。

### 審査で残った点（欠陥ではない）

- テストのコミット `d9a7ad9` は単独ではコンパイルできない。この時点では鍵がまだ
  `HashMap<(usize, usize), _>` で `Span` に `Hash` が無く、
  `checked.value_types[&callee.span]` と添字を付けているためである。コミット本文の
  「直る前は赤」は振る舞いの赤ではなくコンパイルエラーの赤になる（審査 §4 指摘 1）。
- `nested_apply_and_toplevel_field_keep_distinct_inferred_types` は `unwrap()` が通るか
  しか見ておらず、型や値を確かめていない。値は `run_cli` の 2 本が見ている
  （審査 §4 指摘 2）。
- 受理コーパス 146 本の `dump-mono` はすべて終了 0 で、出力は 142 本で base と一致した。
  差の 4 本（`again_resume_*`）は base 自身も 6 回走らせると 2 通りの出力を出す元からの
  非決定性である（審査 §3）。
- 保護ファイル 5 本の差分は 0 行で、`git merge-tree --write-tree origin/main HEAD` の
  終了コードは 0（衝突なし）だった（審査 §5）。
