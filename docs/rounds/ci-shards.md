# CI を複数の台に分けて同時に回す（ci-shards）

開始 2026-10-14。着手時の main は `f4a89fd`（PR #104 着地後）。

## 手順書

### 背景

2026-10-14 にユーザーが目標を決めた。PR の CI を 3 分以内、main の CI を 5 分以内にする
（`docs/plans/test-speed.md`）。

着手時の CI は 1 台の standard runner（4 vCPU）で、m0（ビルドと全体テスト）を前段に直列で
回し、残りの門番を 4 並列で回していた。

| 走行 | 層 | 所要 | 内訳 |
|---|---|---|---|
| 38430986813（PR #96） | core | 4 分 49 秒 | 準備 22 秒、m0 193 秒、残りの門番 74 秒 |
| 38432131158（main） | full | 11 分 41 秒 | 準備 22 秒、m0 230 秒、残りの門番 約 7.5 分 |

公開リポジトリの standard runner は無料で、同時に複数台取れる。m0 の後の門番を前段に
待たせているのは、1 つの作業ツリーと 1 台の CPU を共有していることだけが理由である。

### 決定

1. **ワークフローは matrix で 8 台を同時に回す。** `tests:1/3`〜`tests:3/3` は m0 だけを回し、
   全体テストの I 番目の切れ（nextest `--partition count:I/3`）を走らせる。`gates:1/5`〜
   `gates:5/5` は m0 以外の層の門番を一覧の順に 1 本ずつ 5 台へ配る。8 台の和集合が層の門番と
   全体テストをちょうど 1 回ずつ覆う。台の一覧はワークフローの `matrix.shard` だけに置き、
   取り分は `scripts/run-gates.sh --shard` が選ぶ（門番の一覧はワークフローに書かない。
   `docs/rounds/ci.md` の決定 2）。
2. **gates の台は門番の前に `cargo build` を 1 回走らせる。** m0 がビルドしないため。quick 層と
   同じ段を使う。
3. **文書層と full-only の一覧は `gates:1` だけが出す。** 文書層は門番と並べて走らせ、出力は
   門番の後に出す。
4. **重い門番のケースの判定を並列に走らせる。** r35・r34・embed-runtime・r36 は、生成した
   ケースを 1 件ずつ build して走らせる直列のループだった。`scripts/lib/prerun.sh` がケースの
   判定を先に並列で走らせて出力と終了コードを残し、`check` は元の順で読む。検査の数・順・
   ラベル・判定は変えない。r35 の timeout（2 秒の期限）は並列にしない。並列の `cargo run` は
   cargo の錠を取り合うので、r35・r36 は r34 と同じくビルドした処理系を直接呼ぶ。
5. **string-split-linear の検査 b を CPU 時間で測る。** 壁時計は同じ台の他の門番に取られた
   時間を含み、走行 38511523268 で `large <= small * 2.5` が落ちた。同じ門番の検査 e〜i と
   r34・map-by-builtin-compare は `getrusage` の CPU 時間で測っている。閾値は変えない。
6. **結合テストを 2 本の二進に束ねる。** `tests/` の 72 ファイルは、それぞれが cranelift ごと
   リンクされる別々の二進だった。ファイルは今の場所に置き、`tests/command.rs`（`hado` の
   コマンドを子プロセスで起動するファイル、`CARGO_BIN_EXE_hado` を使うもの）と
   `tests/compiler.rs`（ライブラリを直接呼ぶファイル）が `#[path]` で module として組む。
   `autotests = false` なので、どちらの束にも書かれていないファイルは走らない。
   `every_test_file_is_in_exactly_one_bundle` がそれを赤にする。
7. **依存のビルド成果物をキャッシュに置く。** hado 自身の成果物と門番のログを消した `target/`
   （507 MB）を `actions/cache` に置く。鍵は `Cargo.lock`・`rust-toolchain.toml`・
   `Cargo.toml`（profile）。鍵が外れた走行では `tests:1` の台が保存する。`docs/rounds/ci.md`
   の決定 5 が退けたのは `target/` 全体（4 GB 級）の保存で、保存するものと大きさが違う。
8. **`apt-get update` は、image の索引で `libgc-dev` が入らないときだけにする。**
9. **門番のログは緑の走行でも 14 日残す。** 台ごとの内訳は門番のログにしか出ない。

却下した案:

- m0 をどの台でも走らせ、全体テストの切れと門番を同じ台に載せる（最初の形）。門番が m0 の
  コンパイルと切れを待つので、core は 3 分 26 秒だった（走行 38511527031）。
- 門番の重さを宣言して台へ配る。重さの数字は保守されずに腐る。重い門番を速くすれば一覧の
  順の配り方で足りる。
- 時間を測る門番を台の中で単独で走らせる。検査 b 以外は CPU 時間で測っており、検査 b の
  時計を揃えれば足りる。
- ビルドを 1 台で済ませて成果物を各台へ配る。ビルドの台を待つ直列の段が増え、各台で
  ビルドするより遅い。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| `--shard tests:I/N` | m0 だけ、`HADO_TEST_PARTITION=count:I/N` | 決定 1 |
| `--shard gates:I/N` | m0 を除いた一覧の k 番目（0 始まり）を `k mod N + 1` の台へ | 決定 1 |
| `--shard` の形が違う・`I > N`・先頭 0 | 門番を回さずに落ちる | 決定 1 |
| 文書層 | `gates:1` だけ | 決定 3 |
| 束に書かれていない `tests/*.rs` | 全体テストが赤 | 決定 6 |

### 成果物

- `.github/workflows/ci.yml` — matrix、依存のキャッシュ、apt、ログ。
- `scripts/ci-run.sh`・`scripts/run-gates.sh`・`scripts/nextest-full.sh`・`scripts/verify-m0.sh` — シャード。
- `scripts/lib/prerun.sh` と `scripts/verify-r34.sh`・`verify-r35.sh`・`verify-r36.sh`・`verify-embed-runtime.sh` — 並列化。
- `scripts/verify-string-split-linear.sh` — 検査 b の時計。
- `crates/hado/Cargo.toml`・`crates/hado/tests/command.rs`・`crates/hado/tests/compiler.rs`・
  `crates/hado/tests/corpus.rs`、`scripts/verify-m2.sh`・`scripts/verify-string-split-linear.sh` — 束ねる。
- `docs/rounds/ci-shards.md` — 本ファイル。`docs/plans/test-speed.md` — 目標と経過。

### 工程

1. シャードの配線を入れ、ブランチの `workflow_dispatch` で core と full を測る。
2. 重い門番を並列化し、手元の 4 コアで単独の所要と検査数を測る。
3. 全体テストの台と門番の台を分ける。
4. 結合テストを束ね、全体テスト・clippy・rustfmt・m2・m3 を回す。
5. 依存のキャッシュを入れ、鍵が外れる走行と当たる走行を測る。

### 判定

- 鍵が当たる `workflow_dispatch` の走行で、core が 3 分以内、full が 5 分以内、どちらも緑。
  所要は走行の作成から最後の job の完了まで。
- 束ねた全体テストの件数が、束ねる前の件数 + 1（束の検査）と一致する。

### 許容される赤

なし。

### スコープ外

- 層ごとに台の数を変えること。
- hado 自身のビルドをキャッシュすること（チェックアウトのたびにビルドし直される）。
- テストのファイルを中身の種類で並べ直すこと。

### 中断の条件

GitHub の同時実行の上限（Free の org で 20 job）に当たって job が待たされるなら、台の数を
報告して止める。PR と main が重なると 16 job になる。

## 系譜

| 節 | 規則 | 固定するもの | ラウンド | 備考 |
|---|---|---|---|---|
| — | — | — | ci-shards | 仕様は変えない |

## 実測
CI の走行（ブランチ `ci-shards` の `workflow_dispatch`、`ubuntu-latest` 4 vCPU）。所要は走行の
作成から最後の job の完了まで。

| 走行 | 層 | 形 | 所要 | 結果 |
|---|---|---|---|---|
| 38511527031 | core | 4 台、m0 はどの台でも | 3 分 26 秒 | 緑 |
| 38511523268 | full | 同上 | 7 分 57 秒 | 赤（string-split-linear の検査 b） |
| 38513052872 | core | tests 3 台 + gates 5 台、重い門番を並列化 | 3 分 08 秒 | 緑 |
| 38513050122 | full | 同上 | 3 分 54 秒 | 緑 |
| 38513765363 | core | テスト二進を束ねた | 2 分 57 秒 | 緑 |
| 38513768547 | full | 同上 | 4 分 02 秒 | 緑 |
| 38514578011 | core | 依存のキャッシュ（鍵が外れて保存） | 3 分 09 秒 | 緑 |
| 38514887696 | core | 同上（鍵が当たる） | 2 分 26 秒 | 緑 |
| 38514890060 | full | 同上（鍵が当たる） | 3 分 22 秒 | 緑 |

鍵が当たる core の全体テストの台（走行 38514887696）。依存のキャッシュは 117 MB で 2 秒で戻る。

```
tests:1/3  restore 05:07:02 -> 05:07:04  RUN: verify-m0 05:07:05  TALLY 05:08:05
tests:2/3  restore 05:07:07 -> 05:07:10  RUN: verify-m0 05:07:11  TALLY 05:08:45
tests:3/3  restore 05:07:29 -> 05:07:32  RUN: verify-m0 05:07:32  TALLY 05:09:04
```

走行 38511523268 の赤（gates のログ `verify-string-split-linear.log`）。同じ台で r35 と
embed-runtime が並んで走っていた。

```
length_probe: /tmp/tmp.o5ow8EEBdf/length_probe.c:84: hado_entry: Assertion `large <= small * 2.5 + 0.0002' failed.
NG: b length: String.length scans no bytes and does not slow with length
PASSED: 9/10
```

時計を CPU 時間にした後、r35 と r34 を同じ 4 コアで並べて走らせた（手元）。

```
$ taskset -c 4-7 bash scripts/verify-r35.sh & taskset -c 4-7 bash scripts/verify-r34.sh &
$ taskset -c 4-7 bash scripts/verify-string-split-linear.sh | grep -E '^NG|^OK: b|PASSED'
OK: b length: String.length scans no bytes and does not slow with length
PASSED: 10/10
```

重い門番の単独の所要（手元、`taskset -c 4-7`、4 コア）。検査の数は変わらない。

| 門番 | 前 | 後 | 検査 |
|---|---|---|---|
| verify-r35 | 142.7 秒 | 46.6 秒 | 713/713 |
| verify-r34 | 114.5 秒 | 51.3 秒 | 1360/1360 |
| verify-embed-runtime | 92.8 秒 | 78.1 秒 | 4/4 |
| verify-r36 | 55.5 秒 | 13.9 秒 | 379/379 |

r35 の 1 回の build（手元、20 回の平均）。`cargo run` は錠と鮮度の確認に 1 回 61 ms を足す。

```
cargo run --offline -q -- build main.hd   2.712s / 20
target/debug/hado build main.hd           1.487s / 20
```

テスト二進を束ねる前と後（手元、4 コア、ビルド済みの処理系の後の `cargo nextest run --no-run`）。

```
72 本  real 0m42.550s   856 tests run: 856 passed
2 本   real 0m10.783s   857 tests run: 857 passed
$ cargo nextest list --offline | awk '{print $1}' | sort | uniq -c
     45 hado
    389 hado::command
    423 hado::compiler
```

クリーンからのビルドの内訳（手元、4 コア、sccache なし）。

```
cargo build（クリーン）                 real 1m2.623s
cargo clean -p hado 後の cargo build    real 0m25.442s
```
