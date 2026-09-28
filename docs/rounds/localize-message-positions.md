# 診断本文の位置をファイル座標にする（localize-message-positions）

## 手順書

### 一周目の計画

| 要素 | 内容 |
|---|---|
| 意図 | hado の入口以外のファイルを読む人（作者・CI・エディタ）が、そのファイルに出た診断の本文が指す位置を、そのファイルの中で数えた座標として読めるようにする（推定）。いまは連結後の行番号が出るので、入口の行数によって同じ欠陥の位置が変わり、エディタで開いた行と本文の行が食い違う |
| 仮説 | `Diagnostic` に本文が指す位置の並び（`mentions`）を持たせ、本文は `{0}` `{1}` の穴を持つ形にする。`project::source_diagnostic` がアンカーと同じ `localize` で `mentions` の行をファイルの座標へ直してから穴を埋めれば、既定 1 行・`--format json` の `message`・LSP の `message` の綴り（`at 3:3`）を変えずに値だけ直る。単独ファイルの経路（`check_source`・`check_ast`）は生の座標のまま穴を埋めれば今の出力を保つ（推定） |
| 判定器 | issue の完了条件をそのまま使う。現在値: 3 ファイルの根で入口 1 行なら `at 6:3`、8 行なら `at 13:3`、単独なら `at 3:3`（main `54bf927` で実測）。目標値: 入口の行数によらず `at 3:3`。E502（2 位置）・E507・E508・E406・E403・E405 も同様。門番は入口の行数を変えて測り、ケース名と行番号を埋め込まず、単独ファイルの出力を基準に期待値を計算する |
| スコープ | 触る: `crates/hado/src/check.rs`・`crates/hado/src/project.rs`・`crates/hado/src/parser.rs`（`Diagnostic` の欄と穴埋め）・`crates/hado/src/lsp.rs`（要れば）・`crates/hado/tests/localize_message_positions.rs`（新設）・`scripts/verify-localize-message-positions.sh`（新設）・`scripts/lsp-check.py`・本ファイル・`docs/issues/localize-message-positions.md` の削除。触らない: `docs/spec.md`・`README.md`・`docs/getting-started.md`・`tests/`（コーパス）・`stdlib/`・`crates/hado/src/{mono,emit}.rs`・`runtime/`・`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` |
| 未確認 | 穴を埋める境目を `check_source`・`check_ast`・`source_diagnostic` の 3 箇所に置く形で、穴が残る診断の出し方が無いか。E502 の 2 位置が並びで足り、`related` を増やさずに済むか。LSP の code action の判定が変わらないか |

### 背景

§56 は「行と列はその位置を含むファイルの中で数える」と定める。§50 の読む順では複数のファイルを 1 本のトークン列に連結してから検査するので、`check.rs` が組む診断本文は連結後の座標を持つ。`project::source_diagnostic` はアンカー（`path`・`line`）と `related` をファイルの座標へ直すが、本文の中の数字は直さない。入口以外のファイルの診断本文の位置が、入口の行数だけずれる。

### 決定

- 決定 1: 本文が指す位置は `Diagnostic.mentions` に持つ（issue の推奨）。`related`（§56 の関連位置）へは移さない。E502 は 2 位置を持つが `related` は 1 つで、§56 のラベルをコードごとに 1 つ定める規則も変えることになるため。
- 決定 2: 本文は `{0}` `{1}` … の穴を持つ。`source_diagnostic` が `mentions` をファイルの座標へ直してから `fill_mentions()` で穴を埋める。既定 1 行・JSON・LSP の綴りは変えない。
- 決定 3: 単独ファイルの経路（`check_source`・`dump_mono`・`check_ast`）は、生の座標（= ファイルの座標）で穴を埋める。`Project` は `source_diagnostic` を通すので、`Project` が呼ぶ関数は `*_concatenated` と名付けて穴を残す（決定 6）。
- 決定 4: `end_line` は今のままにする。CLI の JSON と LSP は `Project::diagnostic_end` がファイルの本文から終端を数え直すので、本文の数字だけが対象である。
- 決定 5: `mentions` は `Option<Box<Mentions>>`（`Mentions` は位置の並び）にする。並びをそのまま欄にすると `Diagnostic` が 144 バイトになり、clippy の `result_large_err`（128 バイト）が 239 か所で赤になる。あわせて `code` を `Option<Box<str>>` にする（E コードは固定の字句）。`Diagnostic` は 120 バイトに戻る。
- 決定 6: 穴埋めを呼び出し側の 3 箇所に置く形は、経路が増えると漏れる（審査で `hado::dump_mono` の漏れが出た）。`check_parsed` と `dump_parsed` の plain 名を**単独ファイル用（埋める）**にし、`Project` が使う生の形を `check_parsed_concatenated`・`check_parsed_for_print_concatenated`・`check_parsed_all_concatenated`・`dump_parsed_concatenated` と名付ける。新しい単独ファイルの経路が plain 名を呼べば埋まり、`Project` 側は名前に `_concatenated` が見えて localize の必要に気づく。

### 期待値の根拠

issue の現象を後置の綴りで再現した実測（下の「Red」）を基準にする。入口 1 行と 8 行の出力を比べ、さらに同じファイルを単独で check した出力を基準に、本文の位置が一致することを測る。

### 成果物

- `crates/hado/src/parser.rs`: `Diagnostic.mentions`・`Mentions`・`fill_mentions`。
- `crates/hado/src/check.rs`: E501・E502・E507・E508・E406・E403・E405 の本文を穴付きにし、`mentions` を積む。`check_ast` は穴を埋める。
- `crates/hado/src/lib.rs`: `check_parsed`・`dump_parsed`（単独ファイル用。`fill_mentions` で穴を埋める）と、`Project` 用の `check_parsed_concatenated`・`check_parsed_for_print_concatenated`・`check_parsed_all_concatenated`・`dump_parsed_concatenated`（決定 6）。`check_source`・`dump_mono` は前者を通る。
- `crates/hado/src/project.rs`: `source_diagnostic` が `mentions` を localize してから穴を埋める。
- `crates/hado/src/*.rs`: 診断の組立に `mentions: None` を足し、`code` を `Box<str>` にする（決定 5）。
- `crates/hado/tests/localize_message_positions.rs`: 入口の行数を変えて測る判定器。
- `scripts/verify-localize-message-positions.sh`: 門番（`# tier: core`・`# round: localize-message-positions`）。
- `scripts/lsp-check.py`: 入口以外のファイルの E501 の `message` を見る検査。
- `scripts/gate-tiers.lock`: 門番を `core` に足した導出。
- `docs/rounds/localize-message-positions.md`: 本ファイル。

### 工程

1. Red: 入口の行数を 1 行と 8 行にした 3 ファイルの根で、本文の位置が単独ファイルの出力と一致することを測る失敗テストを書く。
2. Green: `Diagnostic` に `mentions` を足し、6 つの本文を穴付きにし、`source_diagnostic` で localize して埋める。
3. その並びが `Diagnostic` を大きくして clippy の `result_large_err` を赤にするので、`mentions` の箱化と `code` の `Box<str>` で大きさを戻す（決定 5）。
4. 門番を新設し、LSP の判定器に検査を足す。
5. 審査の指摘（`hado::dump_mono` の経路で穴が残る）を受け、単独ファイル用の plain 名と `Project` 用の `*_concatenated` 名に分け、単独ファイルの公開経路を洗う検査を足す（決定 6）。
6. ラウンドの最後に 1 回だけ `cargo test --offline`・`run-gates.sh --tier quick`・`--tier core`・`lint-docs.sh`・`verify-spec-contracts.sh` を回す。
7. `docs/issues/localize-message-positions.md` を消し、push して PR にする（マージしない）。

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

### Red（main の現象。入口の行数で本文の位置がずれる）

着手時の main は `54bf927`。issue の再現コードは旧構文なので後置へ写して測った。
3 ファイルの根（`hello.hd`・`bad.hd`・`extra.hd`）で、入口の行数を 1 行と 8 行に
変えると本文の位置が `6:3` と `13:3` でずれ、単独では `3:3` である。

```
$ cargo run --offline -q -- check target/lmp/three/hello.hd    # 入口 1 行
bad.hd:2:3: error[E501]: binding x is returned unchanged at 6:3; canonical form: "hello"
$ cargo run --offline -q -- check target/lmp/three/hello.hd    # 入口 8 行
bad.hd:2:3: error[E501]: binding x is returned unchanged at 13:3; canonical form: "hello"
$ cargo run --offline -q -- check target/lmp/repro/bad.hd      # 単独
bad.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: "hello"
```

失敗するテスト（`test:` のコミット）は、同じ欠陥を単独ファイルと入口以外の
ファイルで測り、期待値を単独ファイルの出力そのものから計算する。入口 1 行と
8 行の両方で赤になった。

```
$ cargo test --offline --test localize_message_positions
test result: FAILED. 1 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
```

### Green（本文の位置をファイルの座標へ直す）

```
$ cargo test --offline --test localize_message_positions
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

完了条件 (1): 3 ファイルの根で、入口 1 行と 8 行のどちらも同じ 1 行を出し、
JSON の `message` も `at 3:3` になる。単独ファイルの出力は変えない。

```
$ cargo run --offline -q -- check target/lmp/three/hello.hd
bad.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: "hello"
$ cargo run --offline -q -- check --format json target/lmp/three/hello.hd
{"path": "bad.hd", "line": 2, "col": 3, "end_line": 3, "end_col": 4, "code": "E501", "message": "binding x is returned unchanged at 3:3", "canonical": "\"hello\"", "suggestion": null, "related": null}
$ cargo run --offline -q -- check target/lmp/repro/bad.hd
bad.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: "hello"
```

完了条件 (2): 7 つのコードを、コーパスの拒否ケース（E403 だけは `pattern is
covered by the arm at` を出すために包む腕を `Some(_)` にした）から写して測った。
左が単独、右が入口以外のファイルで、位置が一致する。入口の行数を変えても同じ
（上のテストが両方を測る）。

```
E501  binding x is returned unchanged at 2:3
E502  identical branch results at 1:24 and 1:28
E507  branches reconstruct the value at 1:5
E508  absence bound at 2:62 is propagated unchanged; use `as p? ->` or `->`
E406  condition is redundant after 1:20
E403  pattern is covered by the arm at 1:19
E405  pattern is covered by the arm at 1:19
```

完了条件 (3): LSP の判定器に、入口以外のファイルの E501 の `message` を見る検査を
足した。両方の文書を開くと、それぞれの入口の解析が同じファイルへ診断を出す。
本文の位置が直っていれば重複が畳まれて 1 件になり、直っていなければ
`at 3:3` と `at 6:3` の 2 件が並ぶ。期待値は入力から数える（行番号を埋めない）。

```
$ python3 scripts/lsp-check.py <nvim> <repo> target/lmp/lsp-work
OK: diagnostics-localized-message
PASSED: 12/12
```

変異（`source_diagnostic` の localize を外す）では同じ検査が赤になる。

```
NG: diagnostics-localized-message localized-1: bad.hd published 2 E501 diagnostics, want 1: ['binding x is returned unchanged at 3:3', 'binding x is returned unchanged at 6:3']
```

### 大きさ（決定 5）

`mentions` をそのまま欄にすると `Diagnostic` は 144 バイトになり、clippy の
`result_large_err` が 239 か所で赤になる。箱化と `code` の `Box<str>` で
120 バイトに戻り、緑になる。

```
$ cargo clippy --offline --all-targets -- -D warnings   # 並びをそのまま欄にしたとき
error: the `Err`-variant returned from this function is very large
    = help: try reducing the size of `parser::Diagnostic`, ...
error: could not compile `hado` (lib) due to 239 previous errors
$ cargo clippy --offline --all-targets -- -D warnings   # 箱化の後
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s
```

### 門番

```
$ bash scripts/verify-localize-message-positions.sh
== J1 ==
J1:診断位置(ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.50s): PASS
== J2 ==
J2 skip-mention-localize: caught
J2 skip-mention-fill: caught
J2 skip-single-source-fill: caught
J2:変異(cases=3): PASS
== J3 ==
J3:lint-docs(PASSED: 16/16): PASS
PASSED: 3/3
```

### 審査対応: `hado::dump_mono` の経路に穴が残っていた

審査で、公開 API `hado::dump_mono` が `check_parsed` を通るのに `fill_mentions` を呼ばず、本文に `{0}` `{1}` が残ることが出た（このラウンドが入れた退行）。穴埋めを呼び出し側の 3 箇所に置いた形が漏れた。単独ファイル用の plain 名（`check_parsed`・`dump_parsed`）が埋め、`Project` 用は `*_concatenated` と名付けて穴を残す形に直した（決定 6）。

直す前の探針（main `54bf927` から変わらない `hado::dump_mono` の出力）:

```
$ cargo test --offline --test zz_probe_dump -- --nocapture
PROBE e501 code=Some("E501") message="binding x is returned unchanged at {0}"
PROBE e502 code=Some("E502") message="identical branch results at {0} and {1}"
```

直した後（同じ探針）:

```
$ cargo test --offline --test zz_probe_dump -- --nocapture
PROBE e501 code=Some("E501") message="binding x is returned unchanged at 3:3"
PROBE e502 code=Some("E502") message="identical branch results at 2:14 and 2:18"
```

足した検査 `single_source_entries_report_filled_messages` は、7 コードを `dump_mono`・`check_source`・`check::check_ast`・`infer::check_ast` の 4 経路に通し、本文に `{` が残らず `L:C` を持つことを見る。直す前は赤、直した後は緑。

```
$ cargo test --offline --test localize_message_positions -- --exact single_source_entries_report_filled_messages   # 直す前
E501: dump_mono left a message hole: "binding x is returned unchanged at {0}"
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 5 filtered out
$ cargo test --offline --test localize_message_positions -- --exact single_source_entries_report_filled_messages   # 直した後
test single_source_entries_report_filled_messages ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out
```

学び: 穴を埋める呼び出しを経路ごとに足す形は、経路が 1 本増えるだけで漏れる。**埋める場所は「単独ファイルの出口」に 1 つだけ置き、生の座標を渡す形は名前（`_concatenated`）で区別する**と、新しい経路は plain 名を呼ぶだけで正しくなり、`Project` 側は名前で localize の必要性に気づく。判定器も「代表 1 経路」ではなく、その出口を通る公開経路を数えて全部測る。

### ラウンドの最後の実測

```
$ cargo test --offline
（`test result: ok` が 71 行、`FAILED` は 0 行。rc=0）
$ cargo clippy --offline --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.74s
$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates-core
TALLY: 701/701 passed; red: none
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
$ bash scripts/corpus-lock.sh --check
（差分なし。rc=0）
```


