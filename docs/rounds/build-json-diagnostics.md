# `build`・`run`・`test` の診断を `--format json` に従わせる（build-json-diagnostics）

## 手順書

### 一周目の計画

| 要素 | 内容 |
|---|---|
| 意図 | hado の診断を機械で読む道具（CI・エディタ・エージェント）が、`build`・`run`・`test` でも `check` と同じ JSON 1 行を受け取れるようにする（推定）。いまは読み込みの後で止まる診断（名前解決から契約まで）だけが既定の 1 行で出るので、命令ごとに読み方を変えなければならない |
| 仮説 | `Project::build_with_opt_level` の失敗を、検査の診断（`Diagnostic`）とそれ以外の文字列に分ける `BuildError` にして返せば、`main.rs` の `build_command` と `execute` が `check` と同じ描画（`print_diagnostic` の経路）を通せる。`build` が `report` で既定の 1 行に潰している 2 箇所（`is_library` と `check` の失敗）が原因で、`mono`・`emit` と書き出しの失敗は診断ではないので文字列のままでよい（推定） |
| 判定器 | issue の完了条件をそのまま使う。現在値: `build`・`run`・`test --format json` が既定の 1 行（`main.hd:3:20: error[E520]: ... canonical form: if b => 1 \| 0`）。目標値: 3 つとも `check --format json` と同じ JSON 1 行で、`code` が E520、`canonical` が `if b => 1 \| 0` を含む。`test` は続けて `test: broken_test.hd failed (build)` を標準出力に出す。既定の 1 行は変わらない。門番は命令名・コード・位置を埋め込まず、同じ入力の `check` の出力を期待値にする。診断を文字列に潰す変異が赤になることも見る |
| スコープ | 触る: `crates/hado/src/project.rs`（`BuildError` と `build_with_opt_level`・`build`）・`crates/hado/src/main.rs`（診断の描画を 1 つの出口に寄せ、`build_command` と `execute` を通す）・`scripts/verify-build-json-diagnostics.sh`（新設）・`scripts/gate-tiers.lock`（導出）・本ファイル・`docs/issues/build-json-diagnostics.md` の削除。触らない: `docs/spec.md`・`crates/hado/src/{check,mono,emit,lsp,parser}.rs`・`tests/`（コーパス）・`stdlib/`・`runtime/`・`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` |
| 未確認 | 検査の後の診断で既定の 1 行に落ちる経路が `build` の 2 箇所のほかに残っていないか。`compile_to_object` は同じ検査の失敗を文字列で返す公開 API だが、CLI の診断の経路ではないので触らないでよいか。`is_library`（E206）や `--print-type` の「no definition named」のように `check` に対照の無い診断を JSON にするとき、`check --format json` と比べる形が取れるか |

### 背景

§58.4 は `--format json` が `build` / `run` / `test` でも同じと定める。既定の 1 行と印字先、終了コードも同じである。ところが `Project::build_with_opt_level` は検査の診断を `self.report(&error)` で既定の 1 行の文字列に潰してから `main.rs` へ返すので、`build_command` と `execute`（`run` と `test`）は `--format json` を選べない。読み込みで止まる診断（E1xx など）は `load_error_report` が JSON にするので、**「読み込みの診断は JSON になるが、検査の診断はならない」という非対称**が残っていた（2026-09-25、`canonical-in-syntax-errors` の後）。

### 決定

- 決定 1: `Project` に形式の旗を持たせない（issue の退ける案）。§58.4 の形式は CLI の選択で、LSP は使わない。
- 決定 2: `Project::build` の失敗を `BuildError::{Diagnostic, Message}` に分ける（issue の推奨）。`Diagnostic` は §56 の検査の診断（`is_library` の E206 と、`check` の失敗）、`Message` は `mono`・`emit` と書き出しの失敗である。
- 決定 3: 形式の選択を `main.rs` の `diagnostic_report` 1 箇所に寄せる。`print_diagnostic` はその薄い包みにし、`build_error_report` も `diagnostic_report` を通す。診断を描く経路が増えても、形式の選択を経路ごとに足さなくて済む（F2 の学び）。
- 決定 4: `--format json` を受けるのは §58.4 の 4 命令（`check`・`build`・`run`・`test`）だけで、`fmt` と `dump-mono` は受けない。診断を既定の 1 行に潰す経路は `project.rs` の `build_with_opt_level` の 2 箇所だけで、他は `load_error_report` か `print_diagnostic` を通っていた。

### 期待値の根拠

issue の現象を後置の綴りで再現した実測（下の「Red」）を基準にする。期待値は同じ入力の `check --format json` の 1 行そのもので、命令名・コード・位置は門番に埋め込まない。

### 成果物

- `crates/hado/src/project.rs`: `BuildError` と、`build_with_opt_level`・`build` の戻り値。
- `crates/hado/src/main.rs`: `diagnostic_report`・`build_error_report`・`print_diagnostic` と、`build_command`・`execute` の失敗の描画。
- `scripts/verify-build-json-diagnostics.sh`: 門番（`# tier: core`・`# round: build-json-diagnostics`）。
- `scripts/gate-tiers.lock`: 門番を `core` に足した導出。
- `docs/rounds/build-json-diagnostics.md`: 本ファイル。

### 工程

1. Red: 4 命令の JSON と既定の 1 行を測る門番を書き、`build`・`run`・`test` の JSON が赤になることを示す。
2. Green: `BuildError` を足し、`build_command` と `execute` を `diagnostic_report` の経路に通す。
3. Refactor: 診断を文字列に潰す変異が門番で赤になることを確かめ、`grep` で `--format` を受ける命令の診断の経路を洗う。
4. ラウンドの最後に 1 回だけ `cargo test --offline`・`run-gates.sh --tier quick`・`--tier core`・`lint-docs.sh`・`verify-spec-contracts.sh` を回す。
5. `docs/issues/build-json-diagnostics.md` を消し、push して PR にする（マージしない）。

### 判定

issue の完了条件をそのまま使う。既定の 1 行は変えない。

### 許容される赤

なし。

### スコープ外

- `Project` に形式の旗を持たせる案（issue の退ける案）。
- `compile_to_object` の戻り値（公開 API。CLI の診断の経路ではない）。
- §58.4・§56 の文の変更。`dump-mono` と `fmt` に `--format` を足すこと。
- `docs/plans/issue-backlog.md` の後続（`e101-suggestion-scope` 以降）。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- 検査の後の診断で既定の 1 行に落ちる経路が `build` のほかにも見つかり、issue の推奨の形で閉じないと分かった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §58.4 | `--format json` は `build` / `run` / `test` でも `check` と同じ JSON 1 行、既定は今までどおりの 1 行 | 冗長・型 | E520・E305 | `scripts/verify-build-json-diagnostics.sh`（`check` の 1 行と比較） |
| §58.4 | 診断の順と印字先は既定と同じで、終了コードも変えない | 冗長 | E520 | 同上（既定の 1 行の対照） |
| §56 | 診断は `パス:行:列: error[コード]: 説明` の 1 行、関連位置・正準形は続きに書く | 冗長 | E520 | 同上（既定の 1 行が `check` と一致） |
| §58.2 | `test` は `test: <相対パス> failed (build)` を標準出力に出す | 冗長 | E520 | 同上（`test` の続きの報告） |

## 実測

### Red（main の現象。検査の診断が JSON にならない）

issue の入力を後置の綴りで `target/bjd` に置き直して再現した。`check --format json` は
1 行の JSON を出し、`build`・`run`・`test --format json` は既定の 1 行を出す。既定の
1 行は 4 命令で同じである。

```
$ cargo run --offline -q -- check --format json target/bjd/main.hd
{"path": "main.hd", "line": 3, "col": 20, "end_line": 3, "end_col": 49, "code": "E520", "message": "use a Boolean branch instead of Boolean pattern arms or a single condition arm", "canonical": "if b => 1 | 0", "suggestion": null, "related": null}
$ cargo run --offline -q -- build --format json -o target/bjd/out target/bjd/main.hd
main.hd:3:20: error[E520]: use a Boolean branch instead of Boolean pattern arms or a single condition arm; canonical form: if b => 1 | 0
$ cargo run --offline -q -- run --format json target/bjd/main.hd
main.hd:3:20: error[E520]: use a Boolean branch instead of Boolean pattern arms or a single condition arm; canonical form: if b => 1 | 0
$ cargo run --offline -q -- test --format json target/bjd/broken_test.hd
broken_test.hd:3:20: error[E520]: use a Boolean branch instead of Boolean pattern arms or a single condition arm; canonical form: if b => 1 | 0
test: broken_test.hd failed (build)
test: 0/1 passed
```

失敗する門番（`test:` のコミット）は、同じ入力の `check --format json` の 1 行を
期待値にする。`build`・`run`・`test` の JSON と、`build` だけが持つ入口の検査
（E206）が赤になり、既定の 1 行の対照 3 つは緑のままである。

```
$ bash scripts/verify-build-json-diagnostics.sh
NG: build --format json matches check (E520 canonical): ('build', '{...JSON...}', 'main.hd:1:20: error[E520]: ...; canonical form: if b => 1 | 0')
NG: run --format json matches check (E520 canonical): ...
NG: test --format json matches check (E520 canonical): ...
NG: build --format json matches check (E305 type): ...
NG: run --format json matches check (E305 type): ...
OK: build keeps the default line (E520)
OK: run keeps the default line (E520)
OK: test keeps the default line (E520)
NG: build reports its entry diagnostic as JSON: Expecting value: line 1 column 1 (char 0)
NG: run reports its entry diagnostic as JSON: Expecting value: line 1 column 1 (char 0)
PASSED: 3/10
K1:4命令の診断: FAIL
K2:lint-docs: FAIL
PASSED: 0/2
```
