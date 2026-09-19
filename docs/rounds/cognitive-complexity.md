# clippy の認知複雑度を 7 に絞る（cognitive-complexity）

## 手順書

### 背景

- Rust の lint は `scripts/verify-clippy.sh` が守るが（`docs/rounds/rust-lint.md`）、
  `cargo clippy --offline --all-targets -- -D warnings` は既定の lint 集合を見るだけで、
  認知複雑度（`clippy::cognitive_complexity`）は allow のままである。rust-lint は
  `clippy.toml` の新設を明示的にスコープ外としていた。
- 実測（origin/main `6a797ad` の worktree、`clippy.toml` に
  `cognitive-complexity-threshold = 7` を置いた状態）: `cargo clippy --offline --lib --
  -W clippy::cognitive_complexity` は **64 件**の警告を出す。テストコード
  （`#[cfg(test)]` の内側）を除いた本番の関数だけを数えている。
  `--all-targets` で見るとさらに 38 件（`tests/` の 14 と `src` 内のテストの 3、
  および `tests/unit/parser.rs` の 1、残りは重複）が加わる。
- 最上位は `fixes.rs` の `walk_node`（35）、`infer.rs` の 171 行の式（28）、
  `names.rs` の `declaration_types`（27）、`namespaces.rs` の `item`（25）、
  `infer.rs` の `ordinary_module`（23）、`emit.rs` の `build_mono_expr`（22）。
- 閾値を 9 にすると本番の対象は 46 件、7 では 64 件である。

### 決定

1. `clippy.toml` を新設し `cognitive-complexity-threshold = 7` を置く。
2. 対象は**本番のコード（`src/` のうち `#[cfg(test)]` の外側）に限る**。テストは
   対象外とする。`tests/` と `src` 内のテストモジュールは現状の複雑度のままでよい。
   したがって門番は `--lib` に認知複雑度を掛け、`--all-targets` は現行どおり
   既定の lint 集合に掛ける（テスト側の複雑度は判定しない）。
3. リファクタは**振る舞いを変えない**。`cargo test` の判定が前後で同じであることを測る。
   テストとプロダクションは同時に書き換えない。
4. 7 で green にできない関数が出たら、閾値を 9 に落とすのではなく、まず分割で解く。
   どうしても解けない場合だけ報告して相談する（閾値を黙って緩めない）。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| `cargo clippy --offline --lib -- -D clippy::cognitive_complexity` | 警告 0 で exit 0 | 決定 1・2 |
| `cargo clippy --offline --all-targets -- -D warnings` | 警告 0 で exit 0 | 既存の core 門番を緩めない |
| リファクタ前後の `cargo test` | 同じ判定 | 決定 3 |
| `cargo fmt --all --check` | 差分なし | 既存の quick 門番 |

### 成果物

`clippy.toml`（新設）、`scripts/verify-cognitive-complexity.sh`（新設）、
`docs/rounds/cognitive-complexity.md`（本ファイル）、`crates/hado/src/` の 64 関数の分割。

### 工程

- 工程 1: `clippy.toml` を置き、作業リストを固定する（済）。
- 工程 2: 複雑度 16 以上の関数を分割する。
- 工程 3: 複雑度 10〜15 の関数を分割する。
- 工程 4: 複雑度 8〜9 の関数を分割する。
- 工程 5: 門番を足し、`cargo test`・quick・core を回して実測を書く。

### 判定

- cc-a. `cargo clippy --offline --lib -- -D clippy::cognitive_complexity` が警告 0 で終わる。
- cc-b. `cargo clippy --offline --all-targets -- -D warnings` が警告 0 で終わる。
- cc-c. `bash scripts/verify-cognitive-complexity.sh` が `PASSED: n/n`。
- cc-d. `cargo test --offline` が 0 failed（リファクタ前と同じ）。
- cc-e. `cargo fmt --all --check` が差分なし。
- cc-f. quick 層と core 層が緑。
- cc-g. 否定的対照: 複雑度 8 以上の関数を 1 つ戻すと cc-c が赤になる。

### 許容される赤

工程 5 で本ファイルの `## 実測` を書くまで `scripts/lint-docs.sh` 検査 6 は赤でよい。
**終了時に許容される赤は無い。**

### スコープ外

言語機能、`tests/`・`stdlib/`・`runtime/` の変更、テストコードの複雑度、
`--all-targets` の既定 lint 集合、`docs/plan.md` と `docs/ledger.md` の改訂。

### 中断の条件

分割が `cargo test` の判定を変えるなら、allow や `#[allow]` で隠さずに止めて報告せよ。
同じ関数を 2 度分割しても 7 を割らないなら、閾値の判断を人に上げて止まる。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 本番のコードは認知複雑度 7 以下に保つ | 道具 | 成功 | `clippy.toml`・`scripts/verify-cognitive-complexity.sh` |

## 実測

先行分（`walk_node`・`lexer`・`repr`・`dependencies`・`emit`・`infer`・`mono`
ほか）を差分でレビューし、振る舞いが変わりうる 4 件を見つけて直した。
`source_print` の末尾 `Nil` 判定、`infer` の `current_definition` の戻し忘れ、
`fixes` の畳み込み失敗時のフォールバック、`dependencies` の `M.field` の
子走査と handler 値引数の型走査。いずれも原典の制御フローに戻した。

```
# 変更前: origin/main 6a797ad に clippy.toml を置いた一時 worktree。
# `--lib` には `#[cfg(test)]` の内側は現れない。
$ cargo clippy --offline --lib --message-format short -- -W clippy::cognitive_complexity 2>&1 \
    | grep -c 'cognitive complexity of'
64

$ cargo clippy --offline --lib --message-format short -- -W clippy::cognitive_complexity 2>&1 \
    | grep -oE 'cognitive complexity of \([0-9]+' | grep -oE '[0-9]+' | sort -n | uniq -c
      7 8
     14 9
      4 10
      7 11
      6 12
      5 13
      4 14
      3 15
      2 16
      1 17
      1 18
      1 19
      2 20
      1 21
      1 22
      1 23
      1 25
      1 27
      1 28
      1 35
```

```
$ cargo clippy --offline --lib --message-format short -- -W clippy::cognitive_complexity 2>&1 \
    | grep -c 'cognitive complexity of'
0

$ bash scripts/verify-cognitive-complexity.sh
OK: cargo clippy --offline --lib -- -D clippy::cognitive_complexity is clean
OK: no #[allow(clippy::cognitive_complexity)] under crates/hado/src
PASSED: 2/2
```

```
$ cargo test --offline > /tmp/t.log 2>&1; echo exit=$?
exit=0
$ grep -c 'test result: ok' /tmp/t.log
66
$ grep -c 'test result: FAILED' /tmp/t.log
0
$ awk '/^test result/ {p+=$4; f+=$6} END {print "passed="p" failed="f}' /tmp/t.log
passed=818 failed=0
$ tail -3 /tmp/t.log
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```
$ bash scripts/verify-clippy.sh
OK: cargo clippy --offline --all-targets -- -D warnings is clean
OK: clippy-driver --crate-type staticlib -D warnings on crates/hado/float_format.rs is clean
PASSED: 2/2

$ bash scripts/verify-rustfmt.sh
OK: cargo fmt --all --check reports no diff
OK: rustfmt --check crates/hado/float_format.rs reports no diff
PASSED: 2/2
```

```
$ bash scripts/run-gates.sh --tier quick /tmp/cc-quick
RUN: build -> /tmp/cc-quick/build.log
RUN: verify-ci -> /tmp/cc-quick/verify-ci.log
RUN: verify-gate-cache -> /tmp/cc-quick/verify-gate-cache.log
RUN: verify-m1-spec -> /tmp/cc-quick/verify-m1-spec.log
RUN: verify-rustfmt -> /tmp/cc-quick/verify-rustfmt.log
TALLY: 38/38 passed; red: none
# exit 0

$ bash scripts/run-gates.sh --tier core /tmp/cc-core
RUN: verify-ci -> /tmp/cc-core/verify-ci.log
RUN: verify-gate-cache -> /tmp/cc-core/verify-gate-cache.log
RUN: verify-m1-spec -> /tmp/cc-core/verify-m1-spec.log
RUN: verify-rustfmt -> /tmp/cc-core/verify-rustfmt.log
RUN: verify-clippy -> /tmp/cc-core/verify-clippy.log
RUN: verify-cognitive-complexity -> /tmp/cc-core/verify-cognitive-complexity.log
RUN: verify-m0 -> /tmp/cc-core/verify-m0.log
RUN: verify-m1-corpus -> /tmp/cc-core/verify-m1-corpus.log
RUN: verify-m2 -> /tmp/cc-core/verify-m2.log
RUN: verify-m3 -> /tmp/cc-core/verify-m3.log
RUN: verify-m4 -> /tmp/cc-core/verify-m4.log
RUN: verify-m5 -> /tmp/cc-core/verify-m5.log
RUN: verify-m6 -> /tmp/cc-core/verify-m6.log
RUN: verify-m7 -> /tmp/cc-core/verify-m7.log
RUN: verify-r38 -> /tmp/cc-core/verify-r38.log
TALLY: 575/575 passed; red: none
# exit 0
```

```
# 否定的対照 1: ambiguous_declarations.rs を origin/main の collect（複雑度 10）に戻す
$ bash scripts/verify-cognitive-complexity.sh
error: the function has a cognitive complexity of (10/7)
NG: cargo clippy --offline --lib -- -D clippy::cognitive_complexity is clean
OK: no #[allow(clippy::cognitive_complexity)] under crates/hado/src
PASSED: 1/2
# exit 1

# 否定的対照 2: contract_types.rs に #[allow(clippy::cognitive_complexity)] を注入する
$ bash scripts/verify-cognitive-complexity.sh
OK: cargo clippy --offline --lib -- -D clippy::cognitive_complexity is clean
NG: no #[allow(clippy::cognitive_complexity)] under crates/hado/src
PASSED: 1/2
# exit 1
```
