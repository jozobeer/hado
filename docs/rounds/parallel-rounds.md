# 並走できるラウンド運用（parallel-rounds）

開始 2026-09-17。

## 手順書

### 背景

R78 の番号を 2 本の枝が同時に取り、着地時に `verify-r78.sh → verify-r81.sh` の改名、手順書の節番号の繰り下げ（73 → 76）、層の 1 行、台帳の見出し、lock の再生成が要った（`db1ddfb`）。番号起因の変更が意味的な変更より多かった。

2 本が並走すると必ず衝突する場所は、末尾に連番で追記する構造そのものにある。

| 場所 | 衝突の形 |
|---|---|
| `docs/plan.md` | `## N.` の整数連番を末尾に追記 |
| `docs/ledger.md` | `## Rxx` を末尾に追記 |
| `docs/spec-trace.md` | 系譜の行を末尾に追記 |
| `scripts/lib/gate-tiers.sh` | full 層の門番名が 1 行に並ぶ |
| R 番号 | 開始時に次の番号を取る |

触る場所が重なったときだけ衝突するのは `tests/corpus.sha256`（2 番目に着地する側は必ず作り直す）と `docs/spec.md` §56 の表・`docs/spec-coverage.md`（意味的な衝突。道具では消えない）。

### 決定

- **決定 1: ラウンドの識別子は slug にする。** `[a-z0-9][a-z0-9-]*`。`r<数字>` と `r<数字>-r<数字>` の形は決定 3 の旧節のファイル名に予約し、新しい slug には使わない。R 番号は R81 で終わる。ブランチ名・worktree のディレクトリ名・ラウンドファイル名・門番の `# round:` 行に同じ slug を使う。
- **決定 2: 1 ラウンド 1 ファイル。** `docs/rounds/<slug>.md` に手順書・系譜・実測を同居させる。見出しは `# <題>（<slug>）`、`## 手順書`、`## 系譜`、`## 実測`。手順書の内側の型（背景・決定・成果物・工程・判定・許容される赤・スコープ外・中断の条件）は従来の節と同じ。系譜は `docs/spec-trace.md` と同じ 5 列の表。
- **決定 3: 既存のラウンド節を `docs/rounds/` へ移し、`docs/plan.md` から見出しごと消す。** 対象は題に R 番号を含む `## N.` の節すべてで、7・8・10・12・14・15・19〜23・25〜31・35〜76 の 60 節（7 節「R6 で確定した事項」と 15 節「R21追加決定」の決定記録も含む）。ファイル名は R 番号（`r6.md`、`r8-r9.md`、`r10-r13.md`、`r14-r17.md`、`r18-r22.md`、`r21-addendum.md`、`r23-r27.md`、`r28.md`〜`r81.md`）。変換は機械的に行う: `## N. 題` → `# 題（rN）`、次の行に「旧 `docs/plan.md` N 節。実測は `docs/ledger.md`」、その次に `## 手順書`、以下の `###` 以下の階層とコードフェンスは動かさない。残る節は 1〜6・9・11・13・16〜18・24・32〜34 の 16 節で、番号は動かさず欠番のままにする。旧綴りを探す門番（R10・R44・R81）は `docs/` を走査していない（実測済み）ので、移しても赤にならない。`docs/rounds/r8-r9.md`・`docs/rounds/r10-r13.md` の表など、節の内側で他のラウンド節を指す参照と、`docs/wants.md`・`docs/corpus.md`・`docs/spec-trace.md`・`scripts/` の参照はファイル名へ書き換える。
- **決定 4: `docs/ledger.md` は歴史として凍結する。** R81 で追記を止め、本文も参照も書き換えない（R81 決定 5 と同じ扱い）。以後の実測は決定 2 のファイルに書く。
- **決定 5: 層は門番自身が宣言する。** 各 `scripts/verify-*.sh` の冒頭のコメント塊に `# tier: quick|core|full` の行をちょうど 1 つ置く（位置ではなく `^# tier: ` の一意性で判定する）。`scripts/lib/gate-tiers.sh` は一覧を持たず、`${BASH_SOURCE[0]}` から `../verify-*.sh` を走査して層ごとの集合を導く。順序は `LC_ALL=C sort` の名前順になる（現行はリスト順。直列前段があるので走行の結果は変わらない）。`verify-r51.sh` 51-c と `verify-gate-cache.sh` の合成木は自前の `gate-tiers.sh` を関数の界面（`hado_tier_gates` 等）だけで使っている（実測済み）ので、走査式にしても前提は変わらない。直列の一覧（`hado_tier_serial_gates`）は木を書く門番の性質なので `gate-tiers.sh` に残す。`verify-gate-cache.sh` の照合は「各門番が有効な `# tier:` 行をちょうど 1 つ持つ」「run-gates が同じ集合を解決する」に変える。層一覧を文字列で読む 32 本（`tiers.split('full)')` と `grep r70 gate-tiers.sh`）は `bash scripts/run-gates.sh --print-tier full` の出力照合に書き換える。
- **決定 6: 門番はラウンドを宣言する。** 新方式の門番は冒頭のコメント塊に `# round: <slug>` の行をちょうど 1 つ置く。`scripts/lint-docs.sh` 検査 6「最新ラウンドの台帳の節がある」は「`# round:` を持つ各門番について `docs/rounds/<slug>.md` が存在し、`## 実測` の下にコードフェンス（実測の出力）がある」に変える。既存の `verify-r*.sh` は `# round:` を持たない。
- **決定 7: `docs/spec-trace.md` の追記を止める。** R81 の行を最後にし、以後の系譜の行は決定 2 の `## 系譜` に書く。`spec-trace.md` の冒頭にその旨を 1 文足す。
- **決定 8: lock は道具で作り直す。** `scripts/corpus-lock.sh` を新設し、`docs/corpus.md` の定義（`tests/accept` `tests/reject` `tests/library` の `*.hd` `*.hdi` と `stdlib/*.hd`、`LC_ALL=C` のパス順）で `tests/corpus.sha256` を書く。main を取り込んだ後は必ず作り直し、`sha256sum --strict --status -c` で照合する。分割はしない。
- **決定 9: 並走の規約を `AGENTS.md` に書く。** 1 ラウンド 1 worktree（`git worktree add .worktrees/<slug> -b <slug>`）。`CARGO_TARGET_DIR` は設定しない（未設定なので target は worktree ごとに分かれる。`~/.cargo/target` に向いているという現行の記述は事実と違うので直す）。`.worktrees/` を `.gitignore` に足す（今は `.git/info/exclude` にだけあり、他の clone に伝わらない）。着地は PR で main へ。着地の直前に main を取り込み、決定 8 の手順で lock を作り直し、そのラウンドの門番と quick 層を回す。着地後の全走は着地した側が回し、`## 実測` に記録する。`docs/plan.md` に残る版の決定節（`v0.x の範囲と決定`）は今後も 1 本ずつ書き、並走しない。
- **決定 10: 規約と対応表は `docs/rounds.md` に置く。** `docs/corpus.md` が `tests/` を説明するのと同じ置き方で、決定 1・2・5・6 の書き方と、旧節番号 → ファイル名の対応表を書く。対応表は凍結し、新ラウンドは足さない。`docs/rounds/` にはラウンドファイル以外を置かない。`AGENTS.md` の「読む順」に足す。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| 2 つの worktree が各自 `docs/rounds/<slug>.md` と `verify-<slug>.sh` を足して main へ着地 | 衝突なし | 決定 2・5・6・7。共有ファイルを触らない |
| `verify-probe.sh` を `# tier: full` で一時木に置く | `--print-tier full` に `probe` が現れる | 決定 5 |
| `# tier:` 行の無い門番 | `verify-gate-cache.sh` が赤 | 決定 5 |
| `docs/plan.md` に `## N. R` で始まる見出し | 無い | 決定 3 |
| `scripts/corpus-lock.sh` の出力 | 現行の `tests/corpus.sha256` とバイト一致。一時ケースを 1 本足すと 1 行増える | 決定 8 |

### 成果物

`docs/rounds/`（本ファイル、移した 60 節）、`docs/rounds.md`（新設）、`docs/plan.md`、`docs/wants.md`、`docs/corpus.md`、`docs/spec-trace.md`、`AGENTS.md`、`.gitignore`、`scripts/lib/gate-tiers.sh`、`scripts/verify-gate-cache.sh`、`scripts/lint-docs.sh`、`scripts/verify-*.sh`（`# tier:` 行と参照の書き換え）、`scripts/corpus-lock.sh`（新設）、`scripts/verify-parallel-rounds.sh`（新設）。

### 工程

- 工程 0: 計画（本ファイル）。
- 工程 1: 層の宣言。全門番に `# tier:` を足し、`gate-tiers.sh` を走査式にし、`verify-gate-cache.sh` の照合を変える。32 本の文字列照合を `--print-tier full` へ。`verify-gate-cache.sh` と書き換えた門番の該当検査が緑であることを測る。
- 工程 2: 文書の分割。60 節を `docs/rounds/` へ移し、参照を書き換える。`docs/rounds.md` を書く。`lint-docs.sh` 検査 6 を変える。`spec-trace.md` の追記を止める。
- 工程 3: 道具と規約。`corpus-lock.sh`、`AGENTS.md`、`verify-parallel-rounds.sh`。
- 工程 4: 稽古。2 つの worktree で空のラウンドを 1 本ずつ足し、一時ブランチに両方をマージして衝突しないことを測る。全走。

### 判定

`scripts/verify-parallel-rounds.sh` を新設する。入力は生成し、期待値は判定器側で計算する。

- 判定 a: 一時木に `# tier: full` の `verify-probe.sh` を置くと `hado_tier_minimal_gates full` の出力に `probe` が現れ、`# tier:` を欠く門番を置くと `hado_tier_all_gates` が非 0 を返す。
- 判定 b: `docs/plan.md` に題に R 番号を含む `## N.` の見出しが無く、`docs/rounds/*.md` の全件が `# ` の題と `## 手順書` を持つ。
- 判定 c: `# round:` を持つ各門番の slug が決定 1 の字種に合い、`docs/rounds/<slug>.md` が実在する。
- 判定 d: `scripts/corpus-lock.sh` を一時木で走らせた出力が `tests/corpus.sha256` と一致し、一時ケースを足すと行数が 1 増える。
- 判定 e（稽古、手で測る）: 工程 4 のマージが衝突しない。

### 許容される赤

工程 1 の途中で `verify-gate-cache.sh` の層照合と、書き換え前の 32 本の層照合は赤でよい。工程 2 で検査 6 を新形にした後、本ファイルの `## 実測` に出力を書くまで `lint-docs.sh` 検査 6 は赤でよい。**終了時に許容される赤は無い。**

### スコープ外

- `docs/spec.md` §56 の表と `docs/spec-coverage.md` の衝突（意味的な衝突）
- `tests/corpus.sha256` の分割
- `docs/ledger.md` の分割・参照の書き換え
- 既存の門番の判定内容の変更（参照先と層照合の形だけを変える）
- 委譲先（codex 等）が `CARGO_TARGET_DIR` を明示するときの扱い

### 中断の条件

**既存の門番の判定内容（参照先と層照合以外）を変えないと緑にならないなら、止めて報告せよ。**

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （文書運用） | ラウンドは slug で識別し、1 ラウンド 1 ファイルに手順書・系譜・実測を置く。層は門番が宣言する | 道具 | 成功 | `verify-parallel-rounds.sh`（a〜d） |

## 実測

```
# 稽古（判定 e）
git merge --no-edit rehearsal-a; echo merge-a=$?
merge-a=0
git merge --no-edit rehearsal-b; echo merge-b=$?
merge-b=0
git status --short | head
(empty)
bash scripts/run-gates.sh --print-tier full | grep -c rehearsal
2
bash scripts/lint-docs.sh 2>&1 | tail -2
PASSED: 6/7
bash scripts/verify-parallel-rounds.sh 2>&1 | tail -2
PASSED: 4/4

# 対照（main からの旧方式）
git merge --no-edit old-b; echo merge-b=$?
merge-b=1
git diff --name-only --diff-filter=U
docs/plan.md
scripts/lib/gate-tiers.sh

# 全走
TALLY: 3738/3739 passed; red: lint-docs.log
gates-exit=1
cargo test --offline --no-fail-fast → passed=775 failed=0
bash scripts/corpus-lock.sh --check; echo lock=$?
lock=0

# 赤と対応
lint-docs 検査 6（## 実測 にコードフェンスが無かった）。本節で閉じる。閉じたあと赤なし。
```

### 文言を整えた後の全走

`826de74`（spec-trace の参照、決定 6 の位置指定、rounds.md の削り、33 節の層一覧の正）の後に層別を 1 回回した。

```
bash scripts/run-gates.sh --jobs 4 --tier full /tmp/hado-parallel-rounds-final.ZqgxUJ
TALLY: 3739/3739 passed; red: none
```
