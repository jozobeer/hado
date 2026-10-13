# テストのテストを常設の門番から外す（prune-meta-gates）

開始 2026-10-14。着手時の main は `5ac0932`（PR #102 着地後）。

## 手順書

### 背景

main の full の走行 38418467127 は 24 分、PR の core の走行 38421444290 は 14 分かかった。
`RUN:` 行の時刻差で見ると、前段の直列の門番が full で 935 秒、core で 702 秒を占める。
そのうち `crates/hado/src` を書き換えて `cargo build` し直す 6 本が full で 595 秒、core で
362 秒である（`docs/rounds/ci-serial-mutators.md` が直列へ移した門番）。

| 区間 | full | core |
|---|---|---|
| verify-gate-cache | 11 s | 11 s |
| verify-m0 | 231 s | 222 s |
| verify-m1-corpus / m2 / m3 | 66 / 28 / 9 s | 69 / 28 / 10 s |
| ソースを書き換える 6 本 | 595 s | 362 s |
| fan-out（4 並列） | 約 7.5 分 | 約 2 分 |

`docs/rounds/prune-gates.md` は「同じ走行で同じ内容を測っている」検査だけを削った（決定 1）。
本ラウンドでは基準を一段強める。

### 決定

- **決定 1: 常設の門番には、製品（`crates/`・`runtime/`・`stdlib/`・`tools/`・コーパス）か、
  AGENTS.md の守ることの契約（コーパスの lock・仕様の表・凍結した文書・文書の整合・lint・整形）の
  変更で赤になり得る検査だけを置く。** 次の 6 種はこの基準を満たさないので外す。
  - **変異と探針。** 製品のソース・コーパス・道具を一時的に壊して、テストや判定器が赤になる
    ことを見る検査。見ているのはテストの検出力で、赤くなるのはテストを弱めたときだけである。
    ラウンドの時点の 1 回の証拠として各ラウンドの `## 実測` に残っている。
  - **ラッパー。** `cargo test --test <名前>` を呼び直す検査。m0 の全体テストが同じ対象を走らせる。
  - **入れ子と読み返し。** 門番の中で別の門番や `scripts/lint-docs.sh` を呼び直す検査。
    同じ走行で本体が走る。
  - **木の保護。** 門番自身が保護対象を書き換えていないかを見る検査。変異と探針を外すと、
    門番は木を書かない。コーパスの照合は m3 の検査 9 と `scripts/corpus-lock.sh --check` が持つ。
  - **判定器の較正とテスト対象の実在。** 判定器の期待値の計算や、テスト二進の名前の実在を見る検査。
  - **門番の仕組みの自己検査と一度きりの判断。** run-gates・キャッシュ・層・CI の配線・nextest の
    入口・ラウンド運用の道具を合成木で測る門番と、既定値を一度ベンチで決めた門番。
- **決定 2: 門番のキャッシュと直列の一覧を消す。** キャッシュの利用者は入れ子の呼び出し・ラッパー・
  探針の再走だけで、決定 1 でいなくなる。木を書く門番もいなくなるので、前段は m0（ビルドと全体
  テスト）だけにし、残りを fan-out する。
- **決定 3: 製品の振る舞いを測る門番は消さない。** r29〜r81 と名前つきの門番の振る舞いの検査は、
  処理系を起動して出力を測る。`cargo test` へ移す余地はあるが、所要の問題ではないので本ラウンドの
  範囲外にする。
- **決定 4: 消した門番が名指ししていた入力や文書は一緒に消すか、付け替える。**

### 判定表

門番ごと消すもの:

| 門番 | 種 | 中身 |
|---|---|---|
| verify-localize-message-positions | ラッパー・変異・読み返し | J1 は `cargo test --test localize_message_positions`、J2 は変異、J3 は lint-docs |
| verify-postfix-binding | ラッパー・変異・読み返し | J1・J2 は `cargo test`、J3 は印字器の変異、J4 は lint-docs |
| verify-r11・verify-r12 | ラッパー | `cargo test --test r11_comments` など |
| verify-gate-cache | 仕組み | キャッシュの判定器。キャッシュごと消す |
| verify-r38 | 仕組み | run-gates の選択・直列・ログと集計の道具を合成木で測る |
| verify-ci | 仕組み | CI の層の選択と配線を合成木で測る |
| verify-nextest | 仕組み | `scripts/nextest-full.sh` を stub で測る |
| verify-parallel-rounds | 仕組み | 層の宣言・計画の分割・lock の道具を合成木で測る |
| verify-cranelift-opt-level | 一度きり | `opt_level` の既定をベンチで決めた。既定の値の錠は m0 の検査 6 が持つ |

門番の中の検査を消すもの:

| 門番 | 消す検査 | 種 |
|---|---|---|
| verify-m0 | 3 walk・4 tail・8 test-targets | テスト対象の実在（全体テストの合否は検査 2） |
| verify-m1-corpus | 5 corpus-enumeration・7 protected-files | 探針・木の保護 |
| verify-m2 | 1〜3・4 の探針・5・7・8 | `corpus_counts` の assert と重複・探針・木の保護。残るのは E1xx の位置と glue |
| verify-m3 | 4 swap・5 perturb・8 の unit-hm と全体テスト・11 extra-def | 探針（拒否コーパスを書き換える）・ラッパー・テストの印字の読み返し。残りは 1〜8 に詰める |
| verify-m7 | 2 全体テスト | ラッパー |
| verify-fmt-layout | e | 変異 |
| verify-restore-option-question | b | 変異 |
| verify-pure-closure-in-type-variable-argument-fails-specialization | 6 | 変異 |
| verify-recursive-effectful-closure-specialization-reads-outcome-as-int | 9 | 変異 |
| verify-dogfood-text-tool | g | 変異 |
| verify-r35 | output controls | 較正 |
| verify-r36 | output controls・manifest の対照・36-g の lock と変異の対照・36-g pairs・36-f m7 | 較正・木の保護・探針・入れ子 |
| verify-r37 | output controls・manifest の対照・37-i の lock と変異の対照・37-i の m1-corpus・37-j | 較正・木の保護・探針・入れ子・門番の自己検査 |
| verify-r51 | 集計の対照・51-c | 較正・仕組み（run-gates の合成木） |
| verify-char-and-float-conversions | i | 較正 |
| verify-prelude-tail-recursion | g | 較正 |
| verify-string-split-linear | j | 較正 |
| verify-build-json-diagnostics | K2 | 読み返し（lint-docs） |
| verify-spec-model | d | 仕組み（lint-docs の -O） |
| verify-spec-contracts | b | 仕組み（層の lock）。`scripts/gate-tiers.lock` と `scripts/gate-tiers-lock.sh` も消す |

### 成果物

- 上の表の門番の削除と検査の削除。
- `scripts/lib/gate-cache.sh`・`scripts/gate-cache-run.sh` の削除と、`scripts/run-gates.sh` からの
  キャッシュ・指紋・flock・直列の一覧の除去。`scripts/lib/gate-tiers.sh` の `hado_tier_serial_gates` の削除。
- `tests/perf/opt-level/` の削除（verify-cranelift-opt-level だけが読む）。
- `scripts/nextest-full.sh` の `HADO_NEXTEST_BIN`（verify-nextest の stub の入口）の削除。
- CI の `python3-yaml` の削除（verify-ci だけが使う）。
- AGENTS.md: 「門番の走行中は木を触らない」を外す（根拠の m1-corpus の検査 7 が無くなる）。
  「常設の門番にテストのテストを置かない」を足す。
- `docs/issues/gate-cache-libtest-evidence.md` の削除（対象の仕組みが無くなる）。
- AGENTS.md・CONTRIBUTING.md・`docs/` の参照の付け替え。
- 本ファイル。

### 工程

1. 門番の中の検査の削除（変異・探針・ラッパー・読み返し・較正）。
2. 門番ごとの削除と、キャッシュ・直列の一覧の除去。
3. 文書の参照の付け替え。
4. `bash scripts/lint-docs.sh`、触った門番、`scripts/run-gates.sh --tier full --jobs 4`。
5. 本ファイルの実測を書き、PR にする。着地後の main の full の所要を追記する。

### 判定

- `bash scripts/run-gates.sh --jobs 4 --tier full <dir>` が `red: none`。
- `bash scripts/lint-docs.sh` が緑。
- `bash scripts/corpus-lock.sh --check` が緑（コーパスは変えない）。
- `crates/`・`runtime/`・`stdlib/`・`tests/accept`・`tests/reject` を変えない。
- PR の CI（core）と着地後の main の CI（full）の所要が着手前より短い。

### 許容される赤

なし。消すのは検査だけで、製品は変えない。

### スコープ外

- 振る舞いの門番の `cargo test` への移設（決定 3）。
- `tools/gates` の変更。門番の走行で毎回集計に使われる。
- 過去のラウンドのファイルの書き換え。消した門番を名指ししていても当時の記録として残す。

### 中断の条件

- 消そうとした検査が、ほかのどの常設の検査も持たない製品の性質を持っていると分かったとき。
  その検査は残し、表から外して理由を書く。

## 系譜

| 節 | 規則 | 固定するもの | ラウンド | 備考 |
|---|---|---|---|---|
| — | — | — | prune-meta-gates | 仕様は変えない |

## 実測

門番は 84 本から 74 本になった。触った門番 21 本（2026-10-14、手元、`--jobs 4`）:

```
$ HADO_GATE_TIMES=touched.times bash scripts/run-gates.sh --jobs 4 touched m0 m1-corpus m2 m3 m7 fmt-layout ... string-split-linear m1-spec
TALLY: 1427/1427 passed; red: none
real	3m18.628s
```

full 層（2026-10-14、手元、`--jobs 4`。文書層を含む）:

```
$ HADO_GATE_TIMES=full.times bash scripts/run-gates.sh --jobs 4 --tier full full
TALLY: 3662/3662 passed; red: none
real	5m15.911s
```

文書と lock:

```
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/corpus-lock.sh --check && bash scripts/frozen-docs-lock.sh --check
（差分なし）
```

PR #103 の CI（core、走行 38427398926）。着手前の PR の core（38421444290）は門番の区間が 14 分だった:

```
ci-run: event=pull_request ref=refs/pull/103/merge tier=core jobs=4 out=target/ci-gates
TALLY: 565/565 passed; red: none
PASSED: 16/16
gates	pass	4m23s
m0（前段）174 s、fan-out 53 s、門番の計 227 s
```
