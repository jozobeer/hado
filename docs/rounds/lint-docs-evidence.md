# 検査 9 を実測の約束だけにする（lint-docs-evidence）

開始 2026-09-18。

## 手順書

### 背景

PR #5 の検査 9（issue の現象が実測を持つこと）は、シェルの文法（継続行・heredoc・置換・複文・PS2 行）を正規表現で真似る形になり、`scripts/lint-docs.sh` の 296 行と 113 個の探針に育った。四十次〜四十二次のレビューで見つかった綴りの穴はこの近似に対するもので、形は無限に作れる（`docs/rounds/issues.md` の三十五次〜四十二次）。門番は 1 件も実ファイルの欠陥を捕まえておらず、赤はすべて探針用の一時ファイルだった。

### 決定

- **決定 1: 門番は実測の約束だけを見る。** フェンスの中に `$ ` で始まる行（コマンド）と、`> ` で始まらず `#` で始まらずプレースホルダでもない空でない行（出力）が 1 行ずつ以上あることだけを見る。シェルの文法は解析しない。約束は `docs/issues.md` の「実測のフェンスの約束（2026-09-18）」に書く。
- **決定 2: 対応と真偽はレビューで見る。** コマンドと出力の対応、heredoc の本文、`> ` の行の混入、実測の妥当性は人とレビューが見る（`AGENTS.md` の「レビューと判断の振り分け」）。
- **決定 3: 残件は切り出し先へ移す。** 字下げしたフェンス・引用の中の表・URI と en ダッシュのアンカー・空の節は `docs/issues/lint-docs-hardening.md` に書き換え、そちらで生成物・安定 ID・スキーマとして扱う。

### 成果物

- `scripts/lint-docs.sh`: `doc_issue_evidence` のシェル解析（`Shell` 相当の状態機械・heredoc の区切り・PS2 の分岐・113 個の探針）を削除し、`transcript`（コマンドと出力の 1 行ずつ）に置き換える。786 → 667 行。
- `docs/issues.md`: 実測のフェンスの約束と、フェンスは行頭・表は引用の外・範囲は `-`・原因に URI を書かない規約を追記。
- `docs/issues/lint-docs-hardening.md`: 残件（生成物・安定 ID・スキーマ・空の節）に書き換え。

### 判定

- `bash scripts/lint-docs.sh` が `PASSED: 10/10`（実ファイルは 10 件の issue すべてで通る）。
- 縮めた検査の探針: コマンドだけのフェンス・プレースホルダだけの出力・PS2 行だけの出力・出力だけのフェンスが偽、コマンドと出力の組が真。
- 削ったことによる穴を 1 つ記録する。heredoc の本文だけのフェンスは真になる（`$ cat <<EOF` と本文と `EOF`）。約束がシェルを解析しない以上、本文と出力は区別できない。これはレビューで見る（決定 2）。
- `cargo test --offline` が 0 failed、`bash scripts/run-gates.sh --tier full --jobs 4` が red なし。

### スコープ外

- 字下げしたフェンス・引用の中の表・URI・en ダッシュの受け入れ（`docs/issues/lint-docs-hardening.md`）。
- 実測の生成（`scripts/record-round.sh`）と参照の安定 ID 化（同上）。
- 検査 8 の `file:line` の綴り（同上）。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （文書運用） | 現象は実測（コマンドと出力）をフェンスか表で持つ | 文書 | 成功 | `lint-docs.sh` 検査 9 |
| （文書運用） | 実測のフェンスは `$ ` がコマンド、`> ` が PS2、それ以外の空でない行が出力。シェルの文法は解析しない | 文書 | 成功 | `docs/issues.md`（2026-09-18） |

## 実測

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 10/10

# 行数（前 → 後）
git show HEAD:scripts/lint-docs.sh | wc -l
786
wc -l scripts/lint-docs.sh
667

# 縮めた検査の探針（doc_issue_evidence の python ブロックを exec して呼ぶ）
command only        : False
command + output    : True
placeholder output  : False
PS2 line as output  : False
heredoc body        : True     # シェルを解析しない約束の穴。レビューで見る（決定 2）
indented fence      : False

# 最終の木での全走
cargo test --offline -> cargo-test-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3751/3751 passed; red: none
# PR #5 の枝での 3743/3743 より 8 本多い。main にある verify-ci.sh（8 検査）が
# その枝の分岐点より後に入った分で、この枝は main（31f3378）から切っている
```
