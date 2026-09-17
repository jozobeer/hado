# CI（ci）

## 手順書

### 背景

- リモートは GitHub（`jozobeer/hado`、private、org は Free プラン）。Actions は有効だが
  ワークフローは 0 本。Free の private では branch protection が使えず、CI を必須チェックには
  できない。赤は人が見る。
- 門番は層別済み（`docs/plan.md` 33・34 節、`README.md` の検証）。quick は保存ごと、core は
  コミット前、full は節目。層の中身の唯一の正は `scripts/lib/gate-tiers.sh` で、門番の一覧を
  ワークフローに書く場所は無い。
- ローカルのコマンドは `--offline` が前提（`AGENTS.md`）。CI では依存の取得の段だけ network を
  開ける必要がある。
- CI に要るものは Rust（Cranelift 0.135 が 1.95 以上を要求）、`cc`、`libgc-dev`、`flock`
  （`--jobs` > 1）、`python3` と `python3-yaml`（門番がワークフローを YAML として読む）。

### 決定

1. PR は core 層 + 文書層（`scripts/lint-docs.sh`）、main への push は full 層、
   workflow_dispatch は層を選べる（空なら full）。
2. 層の選択は `scripts/ci-run.sh <event> <ref> [requested-tier]` が持つ。ワークフローは層も
   門番の一覧も書かず、入口を呼ぶだけにする（唯一の正は `scripts/lib/gate-tiers.sh`）。
3. 文書層は run-gates の層に含まれない（run-gates が回すのは full の最後だけ）ので、
   `ci-run.sh` がどの層でも必ず回す。full では 2 回になるが判定は同じで所要は数秒。
4. 依存は `cargo fetch --locked` で取得する。以後は `CARGO_NET_OFFLINE=true` を既定にして、
   ローカルと同じ `--offline` の条件で門番を回す。
5. キャッシュは `~/.cargo/registry` と `~/.cargo/git` だけ。`target/` はローカルで 3.8 GB
   （`target/debug/deps` が 2.9 GB）なので保存しない。
6. action は一次の `actions/` だけ（`checkout@v7`・`cache@v6`・`upload-artifact@v7`）。
   第三者の action を使わない。
7. 同じ ref の古い走行は PR のときだけ打ち切る。main の走行は打ち切らない。
8. 門番 `scripts/verify-ci.sh`（quick、network を使わない）が決定 1〜7 を機械で固定する。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| pull_request の `--tier` | core 以上 | 決定 1。core は m0（ビルドと全走テスト）とコーパスを含む |
| push `refs/heads/main` の `--tier` | full | 決定 1 |
| workflow_dispatch の requested | その層 | 決定 1 |
| 知らない event・ref・層 | 門番を 1 本も実走せず exit 1 | 決定 2 |
| ワークフローの `run:` | `--tier` も `verify-` も無い | 決定 2 |
| `cargo fetch --locked` | 入口より前。この段だけ `CARGO_NET_OFFLINE=false` | 決定 4 |

### 成果物

`.github/workflows/ci.yml`（新設）、`scripts/ci-run.sh`（新設）、`scripts/verify-ci.sh`（新設）、
`docs/rounds/ci.md`（本ファイル）、`README.md`、`AGENTS.md`（環境に `python3-yaml` を足す）。

### 工程

- 工程 1: 入口（`scripts/ci-run.sh`）。
- 工程 2: ワークフロー（`.github/workflows/ci.yml`）。
- 工程 3: 門番（`scripts/verify-ci.sh`）と本ファイル。否定的対照で赤を測る。
- 工程 4: quick 層・`lint-docs.sh`・`corpus-lock.sh --check`。PR にして CI の実走を測る。

### 判定

- ci-a. `bash scripts/verify-ci.sh` が `PASSED: 8/8`、`NG:` が 0。
- ci-b. quick 層が緑（`bash scripts/run-gates.sh --jobs 8 --tier quick target/ci-quick`）。
- ci-c. `bash scripts/lint-docs.sh` が `PASSED: 7/7`。
- ci-d. `bash scripts/corpus-lock.sh --check` が差分なし。
- ci-e. PR の CI が success で終わり、ログの `ci-run: ... tier=core` と `PASSED: n/N` が
  ローカルの core の結果と一致する。main への着地後に full 層が success で終わることも見る。
- ci-f. 否定的対照（PR を quick に弱める、文書層を落とす、ワークフローが層を名指しする、
  取得を入口の後ろへ動かす、第三者の action を使う、main も打ち切る）で門番が赤になる。

### 許容される赤

工程 3 で本ファイルの `## 実測` を書くまで `lint-docs.sh` 検査 6 は赤でよい。
**終了時に許容される赤は無い。**

### スコープ外

言語機能、既存の門番の判定条件、`crates/`・`runtime/`・`stdlib/`・`tests/` の変更、
`target/` のキャッシュ（実測を見て別ラウンド）、branch protection（Free の private では
使えない）、release と配布。

### 中断の条件

`--offline` の門番が CI の `cargo fetch` だけでは緑にならないなら、`--offline` を外す方向で
CI を緩めずに止めて報告せよ。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | PR は core 以上、main は full を回し、層と門番の一覧は門番側の 1 か所が持つ | 道具 | 成功 | `scripts/verify-ci.sh`（a〜c） |

## 実測

```
$ bash scripts/verify-ci.sh
OK: a pull_request runs at least the core tier
OK: a push to main runs the full tier
OK: workflow_dispatch defaults to the full tier
OK: workflow_dispatch runs the requested tier
OK: every entry runs the document layer
OK: an unknown event, ref, or tier is refused before any gate
OK: a red gate or a red document layer is not hidden
OK: the workflow parses and is wired for the decisions
PASSED: 8/8
```

```
# 否定的対照（worktree の scripts/ と .github/ を一時ディレクトリに写して 1 箇所ずつ壊す）
=== weaken-pr-tier ===      NG: a pull_request runs at least the core tier   PASSED: 7/8
=== drop-doc-layer ===      NG: every entry runs the document layer
                            NG: a red gate or a red document layer is not hidden   PASSED: 6/8
=== workflow-names-tier === NG: the workflow parses and is wired for the decisions PASSED: 7/8
=== fetch-after-entry ===   NG: the workflow parses and is wired for the decisions PASSED: 7/8
=== third-party-action ===  NG: the workflow parses and is wired for the decisions PASSED: 7/8
=== cancel-main ===         NG: the workflow parses and is wired for the decisions PASSED: 7/8
```

```
# runner の CPU 数で --jobs を決め、層は scripts/ci-run.sh が選ぶ（合成木で実走）
pull_request   refs/pull/1/merge   -> tier=core  run-gates --jobs 1 --tier core;  lint-docs
push           refs/heads/main     -> tier=full  run-gates --jobs 1 --tier full;  lint-docs
workflow_dispatch refs/heads/x     -> tier=full  run-gates --jobs 1 --tier full;  lint-docs
workflow_dispatch refs/heads/x quick -> tier=quick run-gates --jobs 1 --tier quick; lint-docs
schedule / push refs/heads/other / requested=nope -> exit 1、run-gates の実走 0
```

```
# キャッシュの判断（ローカルの実測）
$ du -sh target target/debug/deps target/debug/incremental
3.8G	target
2.9G	target/debug/deps
901M	target/debug/incremental
$ python3 -c 'import yaml; print("pyyaml", yaml.__version__)'
pyyaml 6.0.3
```

```
# 層と文書層（worktree のルートから）
$ bash scripts/corpus-lock.sh --check
（差分なし。exit 0）
$ bash scripts/lint-docs.sh
OK: spec sections §1..§56 and 付録A.. exist once in reading order
...
OK: each gate with a round header has a round file with recorded results
OK: R38 spec sentences are pinned verbatim once each
PASSED: 7/7
$ time bash scripts/run-gates.sh --jobs 8 --tier quick target/ci-quick
RUN: build -> target/ci-quick/build.log
RUN: verify-gate-cache -> target/ci-quick/verify-gate-cache.log
RUN: verify-ci -> target/ci-quick/verify-ci.log
RUN: verify-m1-spec -> target/ci-quick/verify-m1-spec.log
TALLY: 36/36 passed; red: none
real	0m37.533s
（build 1/1、verify-gate-cache 26/26、verify-ci 8/8、verify-m1-spec 1/1）
```
