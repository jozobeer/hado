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
  （`--jobs` > 1）、`python3` と `python3-yaml`（門番がワークフローを YAML として読む）、
  `rustfmt` と `clippy`（門番 verify-rustfmt・verify-clippy・verify-cognitive-complexity）、
  GNU `time`（`crates/hado/tests/r26_effects.rs` が `/usr/bin/time -v` で最大 RSS を測る）。
  最後の 3 つは standard runner の image には入っているので、自己ホストの runner では
  前提を揃える（決定 11）。
- runner は private のあいだ自前の Linux x86_64（決定 9）。上の道具立ては runner 側にも要り、
  `runtime dependencies` の段が `sudo apt-get` を使うので利用者に passwordless sudo があること。

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
7. 古い走行は PR のときだけ打ち切る。push と workflow_dispatch は走行ごとに別の群にして、
   pending の走行も次の push に置き換わらないようにする。同じ群では `cancel-in-progress` が
   false でも pending は新しい走行に置き換わる（GitHub の仕様、下の「審査の指摘と対応」）。
   群は `${{ github.workflow }}-${{ github.event_name == 'pull_request' && github.ref || github.run_id }}`、
   `cancel-in-progress` は PR のときだけ true。
8. 門番 `scripts/verify-ci.sh`（quick、network を使わない）が決定 1〜7・9・10 を機械で固定する。
9. runner は private のあいだ自前の Linux x86_64（`runs-on: [self-hosted, linux, x64]`）にする。
   org は Free プランで、private の Actions は org 共有の 2000 分/月と spending limit に当たる。
   2026-09-22 に job が起動しなくなった（`The job was not started because recent account payments
   have failed or your spending limit needs to be increased`）ので、standard runner の無料枠に
   依存しない形にした。public にしたら standard runner が無料になるので `ubuntu-latest` に戻す。
   2026-10-13 に public にして戻した（`docs/rounds/ci-public.md`）。
   門番はどちらの形も受け、Linux x86_64 であることだけを固定する（決定 8 の一部）。
   ホストを汚さないため、runner は Docker のコンテナの中で動かす（`~/runner/hado-ci-runner.sh`。
   リポジトリの外の運用で、start / stop / status / logs を持つ）。コンテナは公式イメージ
   `ghcr.io/actions/actions-runner` を使い、起動時に登録、停止時に登録解除する。
10. ワークフローは `cargo fetch` より前に `$HOME/.cargo/bin` を `GITHUB_PATH` へ足す段を持つ。
    標準の runner イメージは cargo が PATH にあり、自前の runner は `~/.cargo` に置くため、
    どちらでも同じ段で通る（無いときだけ足す）。image を焼き直さずに済む。
11. 自前の runner は `rustfmt`・`clippy`・GNU `time` を入れてから登録する。standard runner の
    image には最初から入っており、門番（verify-rustfmt・verify-clippy・
    verify-cognitive-complexity）と `r26_effects.rs` の最大 RSS の測定がこれを使う。
    ワークフローの `runtime dependencies` は `libgc-dev` と `python3-yaml` だけを持つ
    （standard runner を前提にする決定 4 のまま。自己ホストの前提は runner 側で揃える）。
12. PR を sudo の通る自前 runner で走らせることを、private のあいだは許容する（2026-09-23 の
    判断）。PR を出せる人は `scripts/ci-run.sh` を書き換えられるので、門番が走る前に runner の
    root を取れる。`permissions: contents: read` は GitHub の token しか絞らない。
    private のあいだは PR を出せるのが共同作業者だけなので信頼境界が実質的に成立し、登録した
    runner は 1 台でコンテナの中なので host の root には届かない。public にして誰でも PR を
    出せるようになった時点で `runs-on` を `ubuntu-latest` に戻す（standard runner が無料になり、
    門番はどちらの形も受ける）。`sudo` を外す案・承認を挟む案は採らない（前者は PR のコードが
    runner の中で走り続け、後者は PR ごとに人の手が要る）。2026-10-13 に public にして戻し、
    門番が自前の runner の形を赤にする（`docs/rounds/ci-public.md`）。

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
- ci-e. PR の CI が success で終わり、ログに `ci-run: event=pull_request ... tier=core`、
  `TALLY: n/n passed; red: none`、文書層の `PASSED: 7/7` が出る。main への着地後に full 層が
  success で終わることも見る。
- ci-f. 否定的対照（PR を quick に弱める、文書層を落とす、ワークフローが層を名指しする、
  取得を入口の後ろへ動かす、第三者の action を使う、main も打ち切る）で門番が赤になる。
- ci-g. concurrency の群が PR のときは `github.ref` を共有し、push と workflow_dispatch では
  `github.run_id` で走行ごとに分かれる（同じ群では pending の走行が置き換わるため）。門番の
  検査と否定的対照（群を `github.ref` だけに戻す）で測る。

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

```
# GitHub Actions の実走（PR #6、run 35269175147、pull_request）
$ gh run view 35269175147 --json status,conclusion,createdAt,updatedAt,event
{"conclusion":"success","createdAt":"2026-09-17T20:10:07Z","event":"pull_request","status":"completed","updatedAt":"2026-09-17T20:30:00Z"}
（job 全体 19 分 53 秒。gates の段は 20:10:10Z -> 20:29:59Z）

$ gh run view 35269175147 --log | grep -E 'ci-run:|TALLY:|RUN: |PASSED:|nproc|rustc 1'
nproc
rustc 1.98.1 (48a229cea 2026-09-01)
cargo 1.98.1 (797e8a9bc 2026-08-05)
ci-run: event=pull_request ref=refs/pull/6/merge tier=core jobs=2 out=target/ci-gates
RUN: verify-gate-cache -> target/ci-gates/verify-gate-cache.log        20:10:28
RUN: verify-m0 -> target/ci-gates/verify-m0.log                        20:10:39 -> 20:25:10
RUN: verify-m1-corpus -> target/ci-gates/verify-m1-corpus.log          20:26:35
RUN: verify-m2 -> target/ci-gates/verify-m2.log                        20:27:42
RUN: verify-m3 -> target/ci-gates/verify-m3.log                        20:28:00
RUN: verify-ci / verify-m1-spec / verify-m4 / verify-m5 / verify-m6 / verify-m7 / verify-r38
    （後段。--jobs 2、20:28:00 -> 20:29:42）
TALLY: 569/569 passed; red: none
PASSED: 7/7
（文書層。core 層では run-gates が回さないので ci-run.sh の 1 回だけ）
```

準備の段（checkout・apt・cache・`cargo fetch --locked`）は 20:10:07Z -> 20:10:27Z の 20 秒。
runner は 2 vCPU（`nproc` = 2）で、この走行の 20 分のうち m0 が 14 分 31 秒（73%）を占める。
2000 分/月の無料枠では PR の core が約 100 回である。

```
# 2 本目（本ファイルの追記の push で走った走行。head 8dd6970）
$ gh run view 35271325121 --json conclusion,createdAt,updatedAt
{"conclusion":"success","createdAt":"2026-09-17T20:31:34Z","updatedAt":"2026-09-17T20:43:52Z"}
ci-run: event=pull_request ref=refs/pull/6/merge tier=core jobs=2 out=target/ci-gates
TALLY: 569/569 passed; red: none
PASSED: 7/7
（job 全体 12 分 18 秒。同じ判定で、所要は runner 次第で 12〜20 分に散る）
```

`target/` のキャッシュと `m0` の短縮、`--jobs` の値の見直しは、この実測を見て別ラウンドで
判断する。

### 審査の指摘と対応（PR #6、Codex review）

指摘は `.github/workflows/ci.yml:26` の concurrency に付いた P2「Give main pushes distinct
concurrency groups」。`cancel-in-progress` が false でも、同じ群では pending の走行が新しい
走行に置き換わるので、main の中間コミットが full 層の検証を飛ばし得る、というもの。

根拠を一次情報で確かめた。github/docs の
`data/reusables/actions/actions-group-concurrency.md` に「Any existing `pending` job or
workflow in the same concurrency group, if it exists, will be canceled and the new queued job
or workflow will take its place.」とある。**指摘は事実として正しく、決定 7 の意図（main の
走行を打ち切らない）は満たされていなかった。** Fix として、群を PR のときだけ `github.ref` で
共有し、push と workflow_dispatch は `github.run_id` で走行ごとに分けた（決定 7 を直した）。
門番の `workflow_case` に群の検査を足した。

```
# 否定的対照（群を github.ref だけの共有に戻す）
=== group-shared-for-all ===
NG: the workflow parses and is wired for the decisions PASSED: 7/8
```

### 自前 runner への切り替え（2026-09-22、決定 9）

PR #31 の CI が起動しなくなった。run 35714886302・35731399675 はどちらも 0 step・数秒で
`failure` になり、check-run の注記が原因を示している。

```
The job was not started because recent account payments have failed or your spending
limit needs to be increased. Please check the 'Billing & plans' section in your settings
```

org は Free プラン（`gh api /orgs/jozobeer --jq .plan.name` → `free`）で、private の Actions は
org 共有の 2000 分/月を使う（このリポジトリの PR の core は 1 回 12〜20 分）。standard runner の
無料枠に依存しないよう、runner を自前の Linux x86_64 に切り替えた（決定 9）。public にしたら
`ubuntu-latest` に戻す。門番は両方の形を受ける。

```
$ bash scripts/verify-ci.sh
OK: a pull_request runs at least the core tier
...
OK: the workflow parses and is wired for the decisions
PASSED: 8/8

# 否定的対照（runs-on を 1 箇所ずつ壊す）
=== self-hosted-linux-only ===  runs-on: ['self-hosted', 'linux'] (the gates need Linux x86_64)   PASSED: 7/8
=== self-hosted-windows ===     runs-on: ['self-hosted', 'windows', 'x64'] (the gates need Linux x86_64) PASSED: 7/8
=== macos-latest ===            runs-on: 'macos-latest' (the gates need Linux x86_64)             PASSED: 7/8
=== ubuntu-latest ===           PASSED: 8/8（public に戻すときの形）
```

自前 runner の実走（`ci-run:` の行・`TALLY:`・所要）は、runner を登録して最初の走行が緑に
なった時点でここへ追記する。runner の登録は repo の Settings → Actions → Runners で行い、
既定のラベル（`self-hosted`・`linux`・`x64`）を外さないこと。

#### コンテナ runner での実走（2026-09-23）

`~/runner/hado-ci-runner.sh start` で Docker のコンテナ runner（`hado-ci-docker`）を立て、
workflow_dispatch（tier=core）で測った。1 回目は runner が job を拾い、`runtime
dependencies`（コンテナ内の `sudo apt-get`）が通った。2 回目は `fetch dependencies` が
`cargo: command not found`（127）で落ち、決定 10 の PATH の段を足した。3 回目（PR の core）は
門番まで届き、赤は環境の道具不足だけだった。

```
TALLY: 629/637 passed; red: verify-clippy verify-cognitive-complexity verify-m0
      verify-m3 verify-m7 verify-rustfmt
```

| 赤 | ログ | 原因 |
|---|---|---|
| verify-clippy・verify-cognitive-complexity | `'cargo-clippy' is not installed for the toolchain 'stable'` | コンテナに clippy が無い |
| verify-rustfmt | `'cargo-fmt' is not installed for the toolchain 'stable'` | コンテナに rustfmt が無い |
| verify-m0・verify-m3・verify-m7 | `r26_effects.rs:1164` が `Os { code: 2, NotFound }` | `/usr/bin/time`（GNU time）が無い |

6 本とも環境の道具不足で、判定条件そのものの赤ではない（決定 11 で runner 側に
`clippy`・`rustfmt`・`time` を入れ、コールドビルドでも core 層が 15 分で完走することを
確かめた）。`rustfmt 1.9.0-stable`・`clippy 0.1.98`・`time (GNU Time)` がコンテナに入り、
`Listening for Jobs` まで進むことをログで確認した。

決定 11 の反映後（コンテナを作り直して再走。run 35811354380、途中の `workflow_dispatch`）。

```
$ gh workflow run ci.yml --ref prune-gates -f tier=core
$ gh api /repos/jozobeer/hado/actions/runs/35811354380/jobs --jq '.jobs[]|{runner_name,conclusion}'
{"runner_name":"hado-ci-docker","conclusion":"success"}

ci-run: event=workflow_dispatch ref=refs/heads/prune-gates tier=core jobs=12 out=target/ci-gates
TALLY: 637/637 passed; red: none
PASSED: 14/14
```

コンテナの runner で core 層が緑、文書層も緑。コールドビルドで 7 分 23 秒（02:41:29 →
02:48:52。standard runner の実測は 12〜20 分なので、こちらの方が速い）。`target/` を
キャッシュしていないので、この時間は毎回かかる。

`runs-on: [self-hosted, linux, x64]` の下で `sudo apt-get update && install` が通ること、
`cargo` が PATH に無いこと（決定 10 の段で通ること）を、走行のログで確かめた。
