# fixes.rs の引数を文脈型に畳む（fixes-arg-context）

## 手順書

### 背景

- PR #11（`docs/rounds/cognitive-complexity.md`）が `crates/hado/src/fixes.rs` を
  認知複雑度 7 以下へ分割した。分割は複雑度を下げたが、`fixes.rs` には
  `#[allow(clippy::too_many_arguments)]` が 13 箇所残っている。
- origin の 10（`boolean_branch_fix` / `catch_all_variable_fix` / `identical_bool_fix` /
  `literal_pattern_fix` / `walk_item` / `rewrap_if_fix` / `rewrap_arm_fix` /
  `repropagate_fix` / `push_edit` / `push_cons_edit`）に、分割が増やした 3
  （`substitute_binding` / `catch_all_folded_fix` / `repropagate_range`）を足して 13。
  分割で消えた 1 は `walk_node`（allow 無しで 7 以下になった）。
- どの関数も同じ 5 つ組（`source` / `lines` / `tokens` / `index` / `branches`）を
  引き回している。既存の `Walk<'a>` はちょうどこの 5 つを束ねた型で、
  `Found<'a>` / `Tail` も既にある。

### 決定

1. 5 つ組を既存の `Walk<'a>` に集約し、`&Walk` を 1 引数として渡す。`Found` / `Tail`
   も既存の型を再利用し、新しい文脈型は作らない。
2. `#[allow(clippy::too_many_arguments)]` を 1 関数ずつ外し、その都度
   `cargo clippy --offline --all-targets -- -D warnings` が green であることを確かめる。
   全部を一度に外さない。
3. 引数の畳み込みだけを行い、振る舞いを変えない。編集・範囲・早期 return・評価順は
   動かさない。
4. 門番の閾値は緩めない。`clippy.toml` の認知複雑度 7 と clippy の引数上限 7 の
   どちらも現行のまま保つ。

### code-complete の一周目

- 意図: `fixes.rs` に残る引数超過の allow を 0 にし、本番コードの引数を clippy の
  上限 7 以下へ戻す。読み手は将来の保守者で、allow の理由をコードから消す。
- 仮説: 13 関数はすべて同じ 5 つ組を引数に並べているだけなので、`&Walk` へ畳めば
  どの関数も 7 以下（多くは 4 以下）になる。`walk_item` は `&Walk` と `&mut Vec<Fix>`
  の 3 引数になり、`rewrap_if_fix` は 2 引数になる。
- 判定器:
  - `grep -rn 'too_many_arguments' crates/hado/src` が 0 件。
  - `bash scripts/verify-clippy.sh` が `PASSED: 2/2`。
  - `bash scripts/verify-cognitive-complexity.sh` が `PASSED: 3/3`（検査 3 本）。
  - `cargo test --offline` が exit 0・`passed=818 failed=0`。
  - `bash scripts/lint-docs.sh` が `PASSED: 10/10`。
- スコープ: `crates/hado/src/fixes.rs` と本ファイル。`tests/`・`stdlib/`・`runtime/`・
  テストコード・`tests/corpus.sha256`・`scripts/`・`clippy.toml` は触らない。
- 未確認: 畳み込みが借用の寿命・`*walk` のコピー・短絡評価を変えないか。clippy は
  引数を 7 で数えるか（`walk_item` の 7 が残るか）。いずれも実装後に clippy と
  `cargo test` で確かめる。
- Red: skip。振る舞いを変えない構造の変更なので、新しい失敗テストは置かない。
  現状の振る舞いは既存の `cargo test --offline`（818 passed / 0 failed）が固定する。
  代わりに clippy の allow を外した状態が赤（引数超過）で、畳むと緑になることを
  1 関数ずつ確かめる。この赤は `clippy::too_many_arguments` の警告そのもので、
  テストではなく lint が判定器になる。
- Green: 下表の畳み込み。1 関数ずつ allow を外して clippy を回す。
- Refactor: 畳み込みの前後で `walk_item_in` などの呼び出し側を `&Walk` に揃える。
  点検で見つけた内部構造の問題は「学習」に書く。

### 対象の関数と引数

| 関数 | 変更前 | 変更後 | 主な引数（5 つ組以外） |
|---|---|---|---|
| `boolean_branch_fix` | 8 | 4 | node / arms / fallback |
| `catch_all_variable_fix` | 8 | 4 | node / arms / fallback |
| `substitute_binding` | 8 | 5 | body / end / binding / value |
| `catch_all_folded_fix` | 8 | 4 | node / pattern / scrutinee_text |
| `identical_bool_fix` | 8 | 4 | node / then / els |
| `literal_pattern_fix` | 8 | 4 | node / arms / fallback |
| `walk_item` | 7 | 3 | item / found |
| `rewrap_if_fix` | 6 | 2 | node |
| `rewrap_arm_fix` | 8 | 4 | node / replacement / tail |
| `repropagate_fix` | 8 | 4 | node / tail / def |
| `repropagate_range` | 8 | 4 | node / absence / tail |
| `push_edit` | 7 | 6 | found / code / anchor / replacement / length |
| `push_cons_edit` | 8 | 5 | found / code / anchor / replacement |

5 つ組を引き回す補助関数（`catch_all_range` / `catch_all_binding_value` /
`arms_reconstruct` / `arm_reconstructs` / `branch_range` / `catch_all_fallback_fix` /
`sole_parameter_fix` / `let_identity_fix` / `condition_arm_fix`）は引数が 7 以下なので
据え置く。畳む関数の本体は先頭の `let Walk { .. } = *walk;` で分解し、補助関数への
呼び出しはそのまま残す（本体と補助関数の引数を変えないため）。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| `grep -rn 'too_many_arguments' crates/hado/src` | 0 件 | 決定 1・2 |
| `cargo clippy --offline --all-targets -- -D warnings` | 警告 0 で exit 0 | 既存の core 門番を緩めない |
| `cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity` | 警告 0 で exit 0 | 既存の core 門番を緩めない |
| 畳み込み前後の `cargo test --offline` | 同じ判定（818 passed / 0 failed） | 決定 3 |
| `bash scripts/lint-docs.sh` | `PASSED: 10/10` | 文書層 |

### 成果物

`crates/hado/src/fixes.rs` の 13 関数とその呼び出し側の畳み込み、
`docs/rounds/fixes-arg-context.md`（本ファイル）。

### 工程

- 工程 1: `Walk` を `Copy` にし、引数超過の 13 関数を 1 つずつ `&Walk` へ畳む。
- 工程 2: 各関数の allow を外すたびに clippy を回し、緑を保つ。
- 工程 3: `walk_item` の `Walk` 構築を呼び出し元（`fixes`）へ移す。
- 工程 4: `grep` で allow 0 件を確かめ、4 つの門番と `cargo test` を回して実測を書く。

### 判定

- fa-a. `grep -rn 'too_many_arguments' crates/hado/src` が 0 件。
- fa-b. `bash scripts/verify-clippy.sh` が `PASSED: 2/2`。
- fa-c. `bash scripts/verify-cognitive-complexity.sh` が `PASSED: 3/3`（PR #11 が
  閾値固定の検査を足したため、タスク指定の 2/2 ではなく 3 本）。
- fa-d. `cargo test --offline` が exit 0・`passed=818 failed=0`。
- fa-e. `bash scripts/lint-docs.sh` が `PASSED: 10/10`。
- fa-f. 否定的対照: 畳む前の 1 関数（例 `boolean_branch_fix`）の allow を外すと
  clippy が `too_many_arguments` で赤になる。

### 許容される赤

無し。本ファイルの `## 実測` は工程 4 で書く。

### スコープ外

言語機能、`tests/`・`stdlib/`・`runtime/` の変更、テストコード、`tests/corpus.sha256`、
`scripts/` の門番、`clippy.toml` の閾値、`docs/plan.md`・`docs/ledger.md`・
`docs/spec-trace.md` の改訂。

### 中断の条件

畳み込みが `cargo test` の判定を変えるなら、allow を戻さずに止めて報告する。
`&Walk` へ寄せても 7 を割らない関数が出たら、`#[allow]` を残したまま理由を報告して
人に上げる。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 本番の関数は clippy の引数上限 7 以下に保つ | 道具 | 成功 | `scripts/verify-clippy.sh` |

## 実測

```
$ grep -rn 'too_many_arguments' crates/hado/src | wc -l
0
```

否定的対照（fa-f）。`boolean_branch_fix` の allow だけを外し、引数を畳む前の
8 引数のまま clippy をかけると赤になる。

```
$ cargo clippy --offline --all-targets --message-format short -- -D warnings 2>&1 \
    | grep too_many_arguments
crates/hado/src/fixes.rs:221:1: error: this function has too many arguments (8/7)
```

```
$ bash scripts/verify-clippy.sh
OK: cargo clippy --offline --all-targets -- -D warnings is clean
OK: clippy-driver --crate-type staticlib -D warnings on crates/hado/float_format.rs is clean
PASSED: 2/2
```

```
$ bash scripts/verify-cognitive-complexity.sh
OK: clippy.toml pins cognitive-complexity-threshold = 7
OK: cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity is clean
OK: no #[allow] / #![allow] for cognitive_complexity under crates/hado/src
PASSED: 3/3
```

```
$ cargo test --offline > /tmp/fixes-test.log 2>&1; echo exit=$?
exit=0
$ awk '/^test result/ {p+=$4; f+=$6} END {print "passed="p" failed="f}' /tmp/fixes-test.log
passed=818 failed=0
$ grep -c 'test result: FAILED' /tmp/fixes-test.log
0
$ tail -2 /tmp/fixes-test.log
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```
$ cargo fmt --all --check; echo exit=$?
exit=0
```

```
$ bash scripts/lint-docs.sh
OK: spec sections §1..§56 and 付録A.. exist once in reading order
OK: spec sections sit under part headings
OK: spec-coverage matches the plan tables 2.2/2.3/9.2/11.2/13.2/18.4/24.2
OK: condition column prose does not require a newline
OK: spec and spec-coverage name the same version
OK: each gate with a round header has a round file with recorded results
coverage: §21=1 §49=1 §55=4
OK: R38 spec sentences are pinned verbatim once each
OK: each issue cause points at a path and an identifier
OK: each issue 現象 carries measured evidence in a fence or table
OK: each issue document has the required name, priority and sections
PASSED: 10/10
```

### 学習

- 仮説は当たった。13 関数はすべて 5 つ組（`source` / `lines` / `tokens` / `index` /
  `branches`）かその部分集合を引数に並べていた。`&Walk` へ畳み、関数の先頭で
  `let Walk { .. } = *walk;` と分解する形にしたので、関数の本体は 1 行も変えずに
  引数だけを減らせた。`cargo test` は前後で 818 passed / 0 failed。
- 実測で分かったこと: 13 の allow のうち、実際に閾値 7 を超えていたのは 8 引数の
  10 関数だけだった。`rewrap_if_fix`（6 引数）・`walk_item`（7）・`push_edit`（7）は
  PR #11 の分割で既に 7 以下になっており、allow だけが残っていた。`walk_node` は
  分割時に allow を外したが、同じく 7 以下になったこの 3 つは外し忘れていた。
  allow を 1 つずつ外して clippy を回す手順がこの残りを露出した。
- `Walk` は全フィールドが共有参照なので `#[derive(Clone, Copy)]` を足せた。これで
  `*walk` の分解が移動にならず、借用を増やさずに `&Walk` のまま渡し回せる。
- 付随する `Refactor` は呼び出し側の書き換えのみ。`walk_item` が組んでいた `Walk` を
  呼び出し元の `fixes` へ移し、`fixes` の 2 つの走査（`condition_arm_fix` と
  `walk_item`）が同じ `Walk` を見るようにした。`condition_arm_fix` 自体の引数は
  6 のまま据え置いた（allow が無く、5 つ組の一部だけを使うため）。
- 認知複雑度への影響は無い。分解は分岐を足さないので `-F clippy::cognitive_complexity`
  は green のまま。allow の再導入もしていない。

