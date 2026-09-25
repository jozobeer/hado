# 構文と分岐の拒否に正準の綴りを出す（canonical-in-syntax-errors）

## 手順書

### 背景

- 引数を括弧ごとに分けた `def` は `expected =>, found (`（E100）、Boolean branch を
  pattern の腕に置いた形は `branches cannot be nested inside another branch`（E101）で、
  どちらも正しい綴りを示さない（`docs/issues/canonical-in-syntax-errors.md`）。
- この 2 つは読み込みの段階で止まり、`Project` を作らずに印字されるので、
  `--format json` を付けても既定の 1 行で出る。

### 決定

1. `Diagnostic` に `suggestion`（形の提示）を足す。値は 1 バイトの列挙 `Suggestion`
   （`&'static str` にすると `Diagnostic` が 128 バイトに達し、clippy の
   `result_large_err` に当たる）。
2. parser が形を知っている箇所で埋める。`parse_params` は閉じ括弧の直後の `(` に、
   `parse_pattern_arm_body` は腕の `if` が Boolean branch のときに添える。Boolean branch
   かどうかは、括弧の外で `=>` と `|` のどちらが先に来るかで決める
   （`boolean_branch_ahead`）。メッセージは変えない。
3. 既定の 1 行は末尾に `; suggestion: ...`、JSON は `suggestion` フィールド。
   `fmt` と `--fix` は適用しない。
4. 読み込みの失敗は `load_error_report` で描き、check・build・`check --fix`・run・test の
   どれでも `--format json` に従わせる。書き換えは引けないので `canonical` は `null`、
   終端はアンカーと同じ位置。
5. §58.4 のフィールドを 10 にし、`suggestion` の意味を書く。

### code-complete の一周目

- 意図: 仕様を引かずに、診断だけで正しい綴りへ直せるようにする。
- 仮説: 形は parser が知っているので、組み立てたときに `Diagnostic` へ載せれば、
  既定の 1 行と JSON の両方に 1 つの値から出せる。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/parser.rs`・`project.rs`・`main.rs`（と `Diagnostic` を
  組み立てる各所）、§58.4、門番、本ファイル。
- 未確認: `Diagnostic` を組み立てる箇所の数と、フィールドを足したときの大きさ。

### 期待値の根拠

綴りは §8（`def f(a: T, b: U): R =>`）と §26.1（`| p => if | c => a | b`）の本文。
他の E101 の形（`fn(| ... )`、腕の中の pattern branch、条件列の腕の中の分岐）と
他の E100 には、当たらない提示を出さない。

### 成果物

`scripts/verify-canonical-in-syntax-errors.sh`、上のソース、§58.4、本ファイル。

### 工程

門番（赤）→ `report_json` から `diagnostic_json` を分ける → 提示と読み込みの経路 →
欠陥の注入 → §58.4。

### 判定

- `bash scripts/verify-canonical-in-syntax-errors.sh` が、修正前は 1/9（通るのは
  「提示を適用しない」の対照）、修正後は 9/9。
- 欠陥を注入すると赤。`boolean_branch_ahead` を常に真にすると腕の中の pattern branch に
  提示が出て赤、`execute` の読み込みの失敗を既定の 1 行に戻すと run と test が赤。
- `cargo test --offline` が 0 failed、clippy が赤なし、`bash scripts/verify-r65.sh` が
  赤なし。

### 許容される赤

無し。

### スコープ外

- 読み込みの後の検査で止まる診断が、`build`・`run`・`test` で `--format json` を
  無視する（`docs/issues/build-json-diagnostics.md`）。
- LSP の診断のメッセージ。LSP は `message` だけを使い、提示は載せない。

### 中断の条件

`Diagnostic` にフィールドを足すと clippy か LSP の門番が赤になり、1 バイトの値で
収まらないときは、提示を `Diagnostic` の外（ソースから後で計算する `canonical_hint` の形）
に移す。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §58.4 | E100・E101 の 2 つの形に正準の綴りを `suggestion` として添え、`fmt` と `--fix` は適用しない | 構文 | E100, E101 | `scripts/verify-canonical-in-syntax-errors.sh` |

## 実測

修正前（df4f2ac）。

```
$ cargo run --offline -q -- check --format json e100/main.hd
main.hd:1:36: error[E100]: expected =>, found (
$ cargo run --offline -q -- check --format json e101/main.hd
main.hd:4:12: error[E101]: branches cannot be nested inside another branch
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 1/9
```

修正後。

```
$ cargo run --offline -q -- check e100/main.hd
main.hd:1:36: error[E100]: expected =>, found (; suggestion: def f(a: T, b: U): R =>
$ cargo run --offline -q -- check e101/main.hd
main.hd:4:12: error[E101]: branches cannot be nested inside another branch; suggestion: | p => if | c => a | b
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 9/9
$ cargo test --offline -q 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 750 failed 0
$ bash scripts/verify-r65.sh | tail -1
PASSED: 5/5
```

欠陥の注入。

| 注入 | 門番 |
|---|---|
| `boolean_branch_ahead` を常に真にする | `PASSED: 8/9`（other E101 shapes が NG） |
| `execute` の読み込みの失敗を `error.to_string()` に戻す | `PASSED: 7/9`（run と test が NG） |

一次レビュー（Codex）の指摘 1 件を修正と門番に取り込んだ（補修）。`test` は根の
列挙の失敗を、`build --fix` は整形の前段の失敗を、既定の 1 行で出していた。どちらも
`LoadError` なので `load_error_report` に通した。指摘は `test` の 1 か所だが、
`source_paths` の失敗が同じ形で残っていたので揃えた。

```
# 直す前（親ディレクトリが無い入力を `test --format json` に与える）
$ target/debug/hado test --format json /tmp/nope/main.hd
/tmp/nope:1:1: error: No such file or directory (os error 2)
# 直した後は 1 行の JSON（`check` と同じ形）
$ target/debug/hado test --format json /tmp/nope/main.hd
{"path": "/tmp/nope", "line": 1, "col": 1, "end_line": 1, "end_col": 1, "code": null, ...}
$ target/debug/hado build --fix --format json /tmp/nope/main.hd
{"path": "/tmp/nope", "line": 1, "col": 1, "end_line": 1, "end_col": 1, "code": null, ...}
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 10/10
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
# 探針（`run_tests` と `format_sources` を既定の 1 行に戻す）
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 9/10（a load error is JSON for every command that takes --format が NG）
```
