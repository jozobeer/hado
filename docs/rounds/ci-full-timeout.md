# main の full 層が job timeout で打ち切られるのを直す（ci-full-timeout）

## 手順書

### 背景

main の CI（push、full 層）は 2026-09-27 の走行 `36313894264` を最後に、以後の走行が
すべて `cancelled` で終わっている。注記はどれも `The job has exceeded the maximum
execution time of 1h0m0s` である。`.github/workflows/ci.yml` の
`jobs.gates.timeout-minutes` が 60 のまま、full 層の所要がそれを超えたためである。
門番そのものは赤ではなく、走行が判定に到達する前に打ち切られている。

直近 2 走を `gh run view <id> --log` で読むと、門番は次のところまで進んでいた。

| 走行 | verify-gate-cache | verify-m0 | verify-m1-corpus | verify-m2 | 打ち切り |
|---|---|---|---|---|---|
| 38257234801 | 10s | 40m01s | 8m37s | 11m26s 走って未完了 | 60m |
| 38263977047 | 12s | 47m13s | 10m44s | 開始直後 | 60m |

`verify-m2`・`verify-m3` と後段の並列門番は 1 度も完走していない。job の実行時間の上限
だけが原因で、門番の判定は 1 つも赤になっていない。

### 決定

- `jobs.gates.timeout-minutes` を 60 から 360 へ上げる。360 は GitHub Actions の job
  timeout の上限である。層・門番・判定条件は変えない。上げても赤は隠れない（門番は
  従来どおり最後まで走り、赤なら `TALLY` に出る）。
- 却下: `target/` のキャッシュ。`actions/checkout` の既定の `clean` が `target/` を
  消すため、self-hosted runner でも毎回 cold build になる。しかし cold build は m0 の
  所要の一部で、`cargo test` と m1〜m3 の所要が支配的である。まず打ち切りを解き、
  所要の内訳は別のラウンドで測る。
- 却下: main への push の層を core へ下げる（`docs/rounds/ci.md` の決定 1 を弱める）。
- 却下: full 層を複数の job へ割る。runner は 1 台で、共有ツリーを書き換える門番
  （`scripts/lib/gate-tiers.sh` の `hado_tier_serial_gates`）は直列が要る。

### 期待値の根拠

- job timeout は job の実行時間の上限であり、門番の判定には関与しない。360 分は
  実測（m0 だけで 40 分、m1-corpus に 8 分 37 秒、m2 に 11 分超）に対して十分な余裕がある。
- 層と門番の一覧は `scripts/lib/gate-tiers.sh` が唯一の正のままで、ワークフローの
  `run:` は入口を呼ぶだけである（`docs/rounds/ci.md` の決定 2）。この変更は
  `timeout-minutes` だけを触る。
- `scripts/verify-ci.sh` は `timeout-minutes` が整数であることだけを要求する。

### 成果物

- `.github/workflows/ci.yml` — `timeout-minutes`。
- `docs/rounds/ci-full-timeout.md` — 本ファイル。

### 工程

1. main の直近の走行を `gh run view` で読み、`cancelled` と注記を確かめる。
2. 走行ログから門番ごとの開始時刻を抜き、m0 と m1-corpus の所要を測る。
3. `timeout-minutes` を 360 へ直す。
4. `bash scripts/verify-ci.sh` と `bash scripts/lint-docs.sh` を回す。
5. main へ着地し、full 層の走行が `cancelled` でなく `success` で終わることを見る。

### 判定

- main の push 走行が `cancelled` でなく `success` で終わり、ログに
  `TALLY: n/n passed; red: none` と文書層の `PASSED:` が出る。
- `bash scripts/verify-ci.sh` が `PASSED: 8/8`。
- `bash scripts/lint-docs.sh` が赤なし。

### 許容される赤

なし。

### スコープ外

- full 層の所要そのものの短縮（`target/` のキャッシュ、門番の並列化、runner の増設）。
  所要の実測を見て別ラウンドにする。
- 門番の判定条件。

### 中断の条件

360 分でも打ち切られるなら、所要そのものを下げる手当てが要る。所要の内訳を報告して止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （CI 基盤） | main への push は full 層を回し、走行は打ち切られずに判定まで届く | 道具 | 成功 | `.github/workflows/ci.yml` |

## 実測

診断（2026-10-12）。

```
$ gh run list --branch main --limit 5
in_progress          Merge pull request #92 ... ci main push 38263977047
completed  cancelled Merge pull request #90 ... ci main push 38257234801
completed  cancelled Merge pull request #91 ... ci main push 38078341621
completed  cancelled Merge pull request #89 ... ci main push 38044788958
completed  cancelled Merge pull request #88 ... ci main push 38031311574

$ gh run view 38263977047 --json conclusion,createdAt,updatedAt,event,headBranch
{"conclusion":"cancelled","createdAt":"2026-10-12T14:35:01Z","event":"push","headBranch":"main","updatedAt":"2026-10-12T15:40:49Z"}

$ gh run view 38263977047 --log | grep -a "exceeded"
... The job has exceeded the maximum execution time of 1h0m0s
```

走行 38257234801 の門番の開始時刻（ログの `RUN:` 行）。

```
13:39:57.20 ci-run: event=push ref=refs/heads/main tier=full jobs=12 out=target/ci-gates
13:39:57.88 RUN: verify-gate-cache
13:40:07.89 RUN: verify-m0
14:20:09.70 RUN: verify-m1-corpus
14:28:46.39 RUN: verify-m2
（14:40:12 に cancelled）
```

着地前の検査。

```
$ bash scripts/verify-ci.sh | tail -3
OK: the workflow parses and is wired for the decisions
PASSED: 8/8

$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
```
