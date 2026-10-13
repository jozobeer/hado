# prelude の解析の固定費を計算量から削る（prelude-fixed-cost）

## 手順書

### 背景

`docs/plans/test-speed.md` の手当ての 2 本目。`dev-opt-level` の後も、空のプログラムの
`hado check` に 0.31 秒かかる。入力は空なので、全部が prelude（`stdlib/*.hd`）の解析である。
テストと門番は反復のたびにこの固定費を払う。

`pprof`（SIGPROF のサンプリング。この機械は `kernel.perf_event_paranoid=4` で perf が使えない）
を一時的に組み込んだ release ビルドで、空のプログラムの `check` を 20 回測った。

| 関数 | 包含の割合 |
|---|---|
| `names::definition_groups` → `names::reachable` | 56% |
| `types::Inference::unify` の中の `HashMap<u32, Ty>` の複製 | 21% |

`definition_groups` は、残りの節点から「自分の強連結成分が他の成分に依存しない」最初の名前を
探すのに、節点の組ごとに `reachable` の DFS を回していた。定義の数に対して 4 乗に近い。
`unify` と `unify_row` は失敗に備えて、呼ばれるたびに置換表と行の表を丸ごと複製していた。

### 決定

- `definition_groups` を Tarjan の強連結成分分解と、取り出せる成分を最小の名前の順に出す
  待ち行列に置き換える。群の中身と順は変わらない。旧い定義をテストの中に正本として残し、
  乱数のグラフ 2000 個で一致を照合する（`names::tests`）。
- `unused_definitions` も根ごとの DFS を、根の全体からの 1 回の走査にする。
- `unify` と `unify_row` は、書いた束縛の以前の値を記録し、失敗したときだけ逆順に戻す。
  入れ子の `unify` は自分の印より後ろだけを戻し、いちばん外側を抜けたら記録を捨てる。
  外側に `unify` が無いときの書き込み（`close_rows` など）は記録しない。
- `scripts/run-gates.sh` の core 層の full-only の一覧の `comm` を `LC_ALL=C` で呼ぶ。
  手元（`en_US.UTF-8`）で core 層を回すと、`comm` が順序の不一致で 1 を返し、
  `set -euo pipefail` で `TALLY` の前に抜けて赤になっていた。CI の runner は `LANG` が空で
  起きていなかった。
- 却下: prelude の解析結果をプロセスをまたいで使い回す。prelude はユーザーのソースの行数だけ
  改行を前置して連結する（`crates/hado/src/lib.rs` の `with_prelude`）ので位置が入力ごとに
  変わり、変更が広い。計算量を直した後の固定費は 0.035 秒で、要らない。

### 期待値の根拠

- 強連結成分分解と待ち行列は辺の数に対して線形（待ち行列の対数を除く）である。
- 旧い `definition_groups` の出力は「残りの成分のうち、他の成分に依存しないもので、最小の
  名前を含むもの」を順に取り出した列である。待ち行列の鍵を成分の最小の名前にすれば同じ列になる。
- 巻き戻しの記録は、その `unify` の中で書いた束縛の数に比例する。

### 成果物

- `crates/hado/src/names.rs` — `graph_groups`・`strongly_connected`・`unreached` と `names::tests`。
- `crates/hado/src/types.rs` — `Inference` の `trail`・`depth`・`transaction` と `types::tests`。
- `scripts/run-gates.sh` — `LC_ALL=C comm`。
- `docs/plans/test-speed.md` — 経過。
- `docs/rounds/prelude-fixed-cost.md` — 本ファイル。

### 工程

1. 計測版の release ビルドで、空のプログラムの `check` をプロファイルする。
2. 新旧の群分けを照合するテストを先に書き、赤（未定義）を確かめる。
3. 強連結成分分解に置き換え、照合を緑にする。待ち行列の鍵を壊して赤になることを確かめる。
4. `unify` の巻き戻しのテストを先に書き（複製の実装で緑）、変更履歴に置き換える。巻き戻しを
   止めて赤になることを確かめる。
5. 固定費、`cargo test` の全走、CI と同じ入口の core 層を測る。

### 判定

- `cargo test --offline --no-fail-fast` が exit 0。
- `bash scripts/ci-run.sh pull_request <ref>`（core 層）が `TALLY: n/n passed; red: none` と
  文書層の `PASSED:`。
- 手元の `cargo test --offline --no-fail-fast` が 5 分以内。
- PR の CI の `gates` の job が 15 分以内。

### 許容される赤

なし。

### スコープ外

- テストと門番の直列の反復の並列化、コーパスの重複（`docs/plans/test-speed.md` の 3〜4）。
  目標に届いたので行わない。

### 中断の条件

群の順か型推論の結果が変わってテストや門番の判定が動くなら、止めて報告する。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （開発基盤） | 定義の依存群と単一化の巻き戻しは、入力の大きさに対して線形に近い | 道具 | 成功 | `crates/hado/src/names.rs`・`crates/hado/src/types.rs` |

## 実測

空のプログラムの `hado check` のプロファイル（変更前、release、20 回、4000 Hz）。

```
total 4581
 56.7% hado::names::definition_groups
 51.9% hado::names::reachable
 31.5% <hado::types::Inference>::unify
 21.0% <hashbrown::raw::RawTable<(u32, hado::types::Ty)> as core::clone::Clone>::clone
```

照合のテストが欠陥を捕まえること。

```
# 待ち行列の鍵を成分の最小の名前から成分の大きさに変える
thread 'names::tests::groups_match_pairwise_reachability' panicked at crates/hado/src/names.rs:1162:13:
test result: FAILED. 1 passed; 1 failed; ...
# 巻き戻しのループを止める
test result: FAILED. 1 passed; 1 failed; ...
```

固定費（空のプログラムの `hado check`、opt-level 1）。

```
o1 check .035775215
o1 check .034780759
o1 check .034470028
```

全走と core 層（同じ機械、12 CPU）。

```
$ cargo test --offline --no-fail-fast
EXIT=0 WALL=90
sum of finished-in 90.09s, passed 854

$ bash scripts/ci-run.sh pull_request refs/pull/0/merge
EXIT=0 WALL=339
TALLY: 765/765 passed; red: none
PASSED: 16/16
```

PR #95 の CI（走行 38388275944、core 層、job `gates` 11:09:31〜11:16:55 の 7 分 24 秒）。

```
11:09:55 ci-run: event=pull_request ref=refs/pull/95/merge tier=core jobs=12 out=target/ci-gates
11:10:05 RUN: verify-m0
11:12:42 RUN: verify-m1-corpus
11:13:37 RUN: verify-m2
11:14:00 RUN: verify-m3
11:15:43 RUN: verify-ci
11:16:36 TALLY: 765/765 passed; red: none
conclusion: success
```
