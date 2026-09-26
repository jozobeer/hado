# 文書層の検査を -O で消さない（gate-optimize-flag）

着手時の main は `d5ff67c`。計画は `docs/plans/issue-backlog.md` の G レーン 3 番目で、
issue の推奨 (a)（`scripts/lint-docs.sh` の python の先頭に門を置く）を採る。
`scripts/verify-spec-model.sh` の `model_case` が持つ門と同じ穴を、文書層にも塞ぐ。

## 手順書

### 一周目の計画

| 要素 | 内容 |
|---|---|
| 意図 | `scripts/lint-docs.sh` を回す人（作者・CI・並走する builder）が、環境に `PYTHONOPTIMIZE` や `-O` が入ったまま回したとき、検査が消えたことに気づかず緑と読むのを防ぐ。文書層が黙って通ると、壊れた仕様が着地し、以降のラウンドがその仕様を読む（推定） |
| 仮説 | `assert` で書いた python の検査は `-O` で消えるので、検査ごとの書き換えではなく、`scripts/lint-docs.sh` の先頭で `__debug__` を見て理由を出して落とす 1 つの門が 12 個の python 断片をまとめて覆う。門は `scripts/verify-spec-model.sh` に足せる（同じ穴の先例を持ち、quick 層なので core でも回る）（推定） |
| 判定器 | issue の完了条件をそのまま使う。`PYTHONOPTIMIZE=1 bash scripts/lint-docs.sh` が非 0 で終わり、理由を出す（現在値: 非 0 だが理由は R38 の NG で、`doc_type_spelling` が OK に変わり消えたことが読めない。目標値: 理由が出る）。門番 `bash scripts/verify-spec-model.sh` が `PYTHONOPTIMIZE=1` つきの呼び出しで赤と理由の両方を見る（現在値: その検査は無い。足した直後は赤、修正後は緑）。正しい環境では `bash scripts/lint-docs.sh` が `PASSED: 16/16` のまま |
| スコープ | 触る: `scripts/lint-docs.sh`・`scripts/verify-spec-model.sh`・本ファイル・`docs/issues/gate-optimize-flag.md` の削除。触らない: `scripts/lib/gate-tiers.sh`・`scripts/run-gates.sh`・`scripts/gate-tiers.lock`・`docs/spec.md`・`spec/`・コーパス・`stdlib/`・他の `scripts/verify-*.sh` |
| 未確認 | 門を各 python 断片でなく script の先頭に 1 つ置く形で「検査を消さずに」を満たすか（完了条件の読み方の決め打ち）。`scripts/verify-spec-model.sh` に文書層の検査を足すことがこの門番の責務として妥当か。門番の走行時間への影響（`-O` の呼び出しは門が先頭で落とすので短い、という見込み） |

### 背景

`python3` は `-O`（`PYTHONOPTIMIZE`）で `assert` を消す。`scripts/lint-docs.sh` の検査は
`assert` で書かれているので、この環境では条件が偽でも何も言わずに通る。赤の数は 1 のまま
なので、消えた検査が出力から読めない（issue の現象）。

`scripts/verify-spec-model.sh` の `model_case` と `scripts/verify-spec-contracts.sh` の
`spec_needles` は先頭で `if not __debug__` を見て理由を出して落ちるが、`scripts/lint-docs.sh`
にはその門が無い。文書層は full の最後にしか回らないので、退行を PR の層（core）で測る
検査も無い。

### 決定

- 決定 1: 門は `scripts/lint-docs.sh` の先頭に 1 つ置く。python を走らせる前に `__debug__`
  を見て理由を出して `exit 1` する。12 個の python 断片ごとに置くと門だけで 24 行になり、
  issue の「3 行程度」に反する。先頭の 1 つは、後から足した python 断片も覆う。
- 決定 2: 門番は `scripts/verify-spec-model.sh` に足す。同じ穴の先例（`model_case`）を持ち、
  tier quick なので core の走行にも入る。`scripts/lint-docs.sh` は full の最後にしか
  回らないので、文書層自身に `-O` の退行を測る場所が無い。`scripts/verify-spec-contracts.sh`
  は core だが、「`spec/` の針」と「層の鎖」を持ち、lint-docs を走らせない。
- 決定 3: `scripts/verify-spec-model.sh` の `# round: spec-model` は変えない。G2
  （`docs/rounds/spec-lock-input-hardening.md` の決定 4）と同じで、`# round:` はその門番を
  新設したラウンドを指す。本ラウンドの実測は本ファイルに置く。
- 決定 4: 門番を増やさない。`scripts/gate-tiers.lock` は触らない。

### 期待値の根拠

- 現象は issue のコマンドをそのまま再現する。`docs/spec.md` を一時的に壊して測り、
  `git checkout` で戻す（変異は木に残さない）。正しい環境の NG が `doc_type_spelling` から
  R38 に変わり、赤の数が 1 のままで消えたことが読めないことを見る。
- 修正後は `-O` の理由が門のものになり、正しい環境の出力（`PASSED: 16/16`）は変わらない。

### 成果物

- `scripts/lint-docs.sh`（先頭の門）。
- `scripts/verify-spec-model.sh`（`doc_lint_optimize` の検査）。
- `docs/rounds/gate-optimize-flag.md`（本ファイル）。
- `docs/issues/gate-optimize-flag.md` の削除。

### 工程

1. Red: `scripts/verify-spec-model.sh` に `doc_lint_optimize` を足し、`bash scripts/verify-spec-model.sh`
   が赤になることを測る。
2. Green: `scripts/lint-docs.sh` の先頭に門を置く。
3. `bash scripts/verify-spec-model.sh` が緑に戻り、`PYTHONOPTIMIZE=1 bash scripts/lint-docs.sh`
   が理由を出すことを測る。
4. ラウンドの最後に 1 回だけ `cargo test --offline`・`bash scripts/run-gates.sh --tier quick`・
   `bash scripts/lint-docs.sh`・`bash scripts/verify-spec-contracts.sh` を回す。
5. `docs/issues/gate-optimize-flag.md` を消し、push して PR にする（マージしない）。

### 判定

issue の完了条件をそのまま使う。

- `PYTHONOPTIMIZE=1 bash scripts/lint-docs.sh` が、検査を消さずに理由を出して非 0 で終わる。
- 門番 `bash scripts/verify-spec-model.sh` が `-O` の呼び出しで赤と理由の両方を見る。
- 正しい環境では `bash scripts/lint-docs.sh` が `PASSED: 16/16`、`bash scripts/verify-spec-model.sh`
  が `PASSED: 3/3`。
- `bash scripts/run-gates.sh --tier quick` が red なし、`cargo test --offline` が 0 failed、
  `bash scripts/verify-spec-contracts.sh` が赤なし。

### 許容される赤

なし。

### スコープ外

- `docs/spec.md`・`spec/`・コーパス・`stdlib/` の変更。
- `scripts/lib/gate-tiers.sh`・`scripts/run-gates.sh`・`scripts/gate-tiers.lock` の変更。
- 検査を `assert` から `if ...: raise` へ書き換える案 (b)、`AGENTS.md` に追記する案 (c)。
- `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` の変更（`AGENTS.md`）。

### 中断の条件

- 正しい環境で `scripts/lint-docs.sh` の出力が変わったとき。
- 門番の走行時間が quick 層の予算を明らかに超えたとき。
- 仕様への疑義（仕様本文を触らないので出ない見込み）。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 文書層の検査は `-O`（`PYTHONOPTIMIZE`）でも消えず、理由を出して非 0 で終わる | 道具 | 成功 | `scripts/lint-docs.sh`・`scripts/verify-spec-model.sh` |

## 実測

### Red（issue の現象。`-O` で検査が消える）

`docs/spec.md` の `Void` を `Unit` に壊すと、正しい環境では `doc_type_spelling` が赤になる。
`PYTHONOPTIMIZE=1` ではその検査が消えて OK に変わり、別の検査（R38）だけが赤になる。赤の
数は 1 のままで、消えたことが出力から読めない。

```
$ sed -i 's/Void は /Unit は /' docs/spec.md
$ bash scripts/lint-docs.sh 2>&1 | grep '^NG:'
NG: the spec keeps the Void type name and its spelling
$ PYTHONOPTIMIZE=1 bash scripts/lint-docs.sh 2>&1 | grep '^NG:'
NG: R38 spec sentences are pinned verbatim once each
$ git checkout -- docs/spec.md
$ git status --short
（空）
```

正しい入力でも `-O` は 15/16 で、`doc_type_spelling` の OK が残る（検査が消えている）。

```
$ bash scripts/lint-docs.sh 2>&1 | tail -1; echo "exit=${PIPESTATUS[0]}"
PASSED: 16/16
exit=0
$ PYTHONOPTIMIZE=1 bash scripts/lint-docs.sh 2>&1 | tail -1; echo "exit=${PIPESTATUS[0]}"
PASSED: 15/16
exit=1
```
