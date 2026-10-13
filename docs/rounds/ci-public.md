# CI を public の standard runner へ戻す（ci-public）

## 手順書

### 背景

2026-10-13 に `jozobeer/hado` を public にした。`docs/rounds/ci.md` の決定 9・12 は、
private のあいだ CI を自前の Linux x86_64 runner（12 コア、Docker のコンテナ）で回し、
public にしたら `ubuntu-latest` に戻すと決めている。

- 料金: public リポジトリでは standard の GitHub ホスト runner は無料で、分数の枠が無い
  （GitHub Docs「GitHub Actions billing」。larger runner は public でも有料）。
- 信頼境界: public では誰でも PR を出せる。PR のコードは `scripts/ci-run.sh` を書き換えて
  runner の中で sudo を取れる（決定 12）。fork からの PR の承認は既定の
  `first_time_contributors` で、2 回目以降の PR は承認なしで走る。

### 決定

1. `jobs.gates.runs-on` を `ubuntu-latest` にする。層・門番・`timeout-minutes`（360）は
   変えない。
2. `scripts/verify-ci.sh` は `runs-on` が `ubuntu-latest` であることだけを受ける。自前の
   runner の形（`[self-hosted, linux, x64]`）は赤にする。
3. 自前の runner は着地後に登録を解除して止める（`~/runner/hado-ci-runner.sh stop`、
   リポジトリの外の運用）。
4. `cargo on PATH` の段（`docs/rounds/ci.md` の決定 10）は残す。standard runner では
   cargo が PATH にあるので何もしない。

### 期待値の根拠

- standard runner の image には `rustfmt`・`clippy`・GNU `time` が入っている
  （`docs/rounds/ci.md` の決定 11）。toolchain は `rust-toolchain.toml` が 1.99.0 と
  `rustfmt`・`clippy` を指定し、rustup が初回の `cargo` で取ってくる。
- public の standard runner は 4 vCPU で、自前の runner（`nproc` 12、走行 38406705678）より
  少ない。full 層は自前の runner で m0 だけに 40〜47 分かかった
  （`docs/rounds/ci-full-timeout.md`）。360 分に収まるかは実走で測る。

### 成果物

- `.github/workflows/ci.yml` — `runs-on`。
- `scripts/verify-ci.sh` — runner の判定。
- `CONTRIBUTING.md` — CI の制約の記述。
- `docs/rounds/ci.md` — 決定 9・12 から本ファイルへの参照。
- `docs/rounds/ci-public.md` — 本ファイル。

### 工程

1. `runs-on` を直し、`bash scripts/verify-ci.sh` が緑のままであることを見る。
2. `scripts/verify-ci.sh` を締め、`runs-on` を 1 箇所ずつ壊した否定的対照で赤を見る。
3. PR の core 層が `ubuntu-latest` で緑になることを見る。
4. 着地後、main の full 層の所要を測り、自前の runner を止める。

### 判定

- `bash scripts/verify-ci.sh` が `PASSED: 8/8`。自前の runner・arm・macOS の形で
  `PASSED: 7/8`。
- PR の走行が `ubuntu-latest` で `success`。
- main の push 走行が `success` で、`TALLY: n/n passed; red: none` が出る。

### 許容される赤

なし。

### スコープ外

- branch protection で CI を必須チェックにすること（public の Free で使えるようになった。
  リポジトリの設定なので人が決める）。
- full 層の所要の短縮。

### 中断の条件

main の full 層が 360 分で打ち切られるなら、所要の内訳を報告して止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （CI 基盤） | public では PR を standard runner で回し、自前の runner で走らせない | 道具 | 成功 | `scripts/verify-ci.sh` |

## 実測

着地前の検査（2026-10-13）。

```
$ bash scripts/verify-ci.sh | tail -1
PASSED: 8/8

# 否定的対照（runs-on を 1 箇所ずつ壊す）
=== [self-hosted, linux, x64] === runs-on: ['self-hosted', 'linux', 'x64'] (a public repository runs on ubuntu-latest) PASSED: 7/8
=== ubuntu-24.04-arm ===          runs-on: 'ubuntu-24.04-arm' (a public repository runs on ubuntu-latest) PASSED: 7/8
=== macos-latest ===              runs-on: 'macos-latest' (a public repository runs on ubuntu-latest) PASSED: 7/8
```

PR の core 層（走行 38411482495、2026-10-13、`ubuntu-latest`）。14:13:16 に作成、14:27:20 に
`success`。比較として、直前の自前 runner での PR の core 層（走行 38406705678）は 20 分 21 秒。

```
$ gh api repos/jozobeer/hado/actions/runs/38411482495/jobs -q '.jobs[0]|{runner_name,labels}'
{"labels":["ubuntu-latest"],"runner_name":"GitHub Actions 1000003591"}

$ gh run view 38411482495 --log | grep -aE 'toolchain|ci-run:|TALLY|PASSED:'
rustc 1.99.0 (b940084d7 2026-09-28)
cargo 1.99.0 (5f94df478 2026-08-27)
4
ci-run: event=pull_request ref=refs/pull/99/merge tier=core jobs=4 out=target/ci-gates
TALLY: 771/771 passed; red: none
PASSED: 16/16
```
