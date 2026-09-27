# 診断本文の位置をファイル座標にする（localize-message-positions）

## 手順書

### 一周目の計画

| 要素 | 内容 |
|---|---|
| 意図 | hado の入口以外のファイルを読む人（作者・CI・エディタ）が、そのファイルに出た診断の本文が指す位置を、そのファイルの中で数えた座標として読めるようにする（推定）。いまは連結後の行番号が出るので、入口の行数によって同じ欠陥の位置が変わり、エディタで開いた行と本文の行が食い違う |
| 仮説 | `Diagnostic` に本文が指す位置の並び（`mentions: Vec<(usize, usize)>`）を持たせ、本文は `{0}` `{1}` の穴を持つ形にする。`project::source_diagnostic` がアンカーと同じ `localize` で `mentions` の行をファイルの座標へ直してから穴を埋めれば、既定 1 行・`--format json` の `message`・LSP の `message` の綴り（`at 3:3`）を変えずに値だけ直る。単独ファイルの経路（`check_source`・`check_ast`）は生の座標のまま穴を埋めれば今の出力を保つ（推定） |
| 判定器 | issue の完了条件をそのまま使う。現在値: 3 ファイルの根で入口 1 行なら `at 6:3`、8 行なら `at 13:3`、単独なら `at 3:3`（main `54bf927` で実測）。目標値: 入口の行数によらず `at 3:3`。E502（2 位置）・E507・E508・E406・E403・E405 も同様。門番は入口の行数を変えて測り、ケース名と行番号を埋め込まず、単独ファイルの出力を基準に期待値を計算する |
| スコープ | 触る: `crates/hado/src/check.rs`・`crates/hado/src/project.rs`・`crates/hado/src/parser.rs`（`Diagnostic` の欄と穴埋め）・`crates/hado/src/lsp.rs`（要れば）・`crates/hado/tests/localize_message_positions.rs`（新設）・`scripts/verify-localize-message-positions.sh`（新設）・`scripts/lsp-check.py`・本ファイル・`docs/issues/localize-message-positions.md` の削除。触らない: `docs/spec.md`・`README.md`・`docs/getting-started.md`・`tests/`（コーパス）・`stdlib/`・`crates/hado/src/{mono,emit}.rs`・`runtime/`・`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` |
| 未確認 | 穴を埋める境目を `check_source`・`check_ast`・`source_diagnostic` の 3 箇所に置く形で、穴が残る診断の出し方が無いか。E502 の 2 位置が並びで足り、`related` を増やさずに済むか。LSP の code action の判定が変わらないか |

### 背景

§56 は「行と列はその位置を含むファイルの中で数える」と定める。§50 の読む順では複数のファイルを 1 本のトークン列に連結してから検査するので、`check.rs` が組む診断本文は連結後の座標を持つ。`project::source_diagnostic` はアンカー（`path`・`line`）と `related` をファイルの座標へ直すが、本文の中の数字は直さない。入口以外のファイルの診断本文の位置が、入口の行数だけずれる。

### 決定

- 決定 1: 本文が指す位置は `Diagnostic.mentions: Vec<(usize, usize)>` に持つ（issue の推奨）。`related`（§56 の関連位置）へは移さない。E502 は 2 位置を持つが `related` は 1 つで、§56 のラベルをコードごとに 1 つ定める規則も変えることになるため。
- 決定 2: 本文は `{0}` `{1}` … の穴を持つ。`source_diagnostic` が `mentions` をファイルの座標へ直してから `fill_mentions()` で穴を埋める。既定 1 行・JSON・LSP の綴りは変えない。
- 決定 3: 単独ファイルの経路（`check_source`・`check_ast`）は、生の座標（= ファイルの座標）で穴を埋める。`check_parsed` では埋めない（`Project` が `source_diagnostic` を通すため）。
- 決定 4: `end_line` は今のままにする。CLI の JSON と LSP は `Project::diagnostic_end` がファイルの本文から終端を数え直すので、本文の数字だけが対象である。

### 期待値の根拠

issue の現象を後置の綴りで再現した実測（下の「Red」）を基準にする。入口 1 行と 8 行の出力を比べ、さらに同じファイルを単独で check した出力を基準に、本文の位置が一致することを測る。

### 成果物

- `crates/hado/src/parser.rs`: `Diagnostic.mentions` と `fill_mentions`。
- `crates/hado/src/check.rs`: E501・E502・E507・E508・E406・E403・E405 の本文を穴付きにし、`mentions` を積む。`check_ast` は穴を埋める。
- `crates/hado/src/lib.rs`: `check_source` は穴を埋める。
- `crates/hado/src/project.rs`: `source_diagnostic` が `mentions` を localize してから穴を埋める。
- `crates/hado/tests/localize_message_positions.rs`: 入口の行数を変えて測る判定器。
- `scripts/verify-localize-message-positions.sh`: 門番（`# tier: core`・`# round: localize-message-positions`）。
- `scripts/lsp-check.py`: 入口以外のファイルの E501 の `message` を見る検査。
- `docs/rounds/localize-message-positions.md`: 本ファイル。

### 工程

1. Red: 入口の行数を 1 行と 8 行にした 3 ファイルの根で、本文の位置が単独ファイルの出力と一致することを測る失敗テストを書く。
2. Green: `Diagnostic` に `mentions` を足し、6 つの本文を穴付きにし、`source_diagnostic` で localize して埋める。
3. 門番を新設し、LSP の判定器に検査を足す。
4. ラウンドの最後に 1 回だけ `cargo test --offline`・`run-gates.sh --tier quick`・`--tier core`・`lint-docs.sh`・`verify-spec-contracts.sh` を回す。
5. `docs/issues/localize-message-positions.md` を消し、push して PR にする（マージしない）。

### 判定

issue の完了条件をそのまま使う。単独ファイルの出力は変えない。

### 許容される赤

なし。

### スコープ外

- `related` の追加と §56 のラベルの変更（issue の退ける案）。
- 本文から位置を消す案、正規表現で書き換える案（issue の退ける案）。
- `end_line` の localize（決定 4）。
- `docs/plans/issue-backlog.md` の後続（`build-json-diagnostics` 以降）。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- 穴が残る診断の出し方があると分かった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §56 | 診断本文が指す位置も、その位置を含むファイルの中で数える | 冗長・網羅性 | E501・E502・E507・E508・E406・E403・E405 | `tests/reject/` の各コードを入口以外のファイルへ置いた判定器 |
| §58.4 | JSON の `message` は本文で、位置の綴りは `at L:C` のまま | 冗長・網羅性 | 同上 | 門番の `--format json` |
| §58.5 | LSP の診断の `message` も同じ本文 | 冗長 | E501 | `scripts/lsp-check.py` |

## 実測

（後で追記）
