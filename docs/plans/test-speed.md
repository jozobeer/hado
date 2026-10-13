# テストと門番の所要を開発ループに収める（test-speed）

## 目標

2026-10-13 にユーザーが決めた。両方を満たすまで下の手当てを順に回す。

- 手元の `cargo test --offline` が 5 分以内。
- PR の CI（core 層、`.github/workflows/ci.yml` の `gates` の job）が 15 分以内。

## 現状（2026-10-13）

CI の門番の所要（走行ログの `RUN:` 行の時刻差）。

| 区間 | PR 38305183510（core） | main 38290048902（full） |
|---|---|---|
| verify-m0（`cargo test` の全走） | 47 分 | 39 分 |
| verify-m1-corpus / m2 / m3（直列） | 11 / 11 / 4 分 | 9 / 11 / 4 分 |
| ソースを書き換える門番（直列） | 3 分 | 9.5 分 |
| fan-out | 5 分 | 50 分 |
| 計 | 81 分 | 122 分 |

runner は 1 台で、走行は待ち行列に並ぶ（PR 38305419671 は門番の開始まで 80 分待った）。

verify-m0 の 40 分はほぼ全部がテストの実行である。コンパイルは 15 秒、テスト二進の
`finished in` の和は 2347 秒。上位 6 本（`corpus` 253 秒・`r9_order` 203 秒・
`check_cli` 193 秒・`r9_names` 179 秒・`named_record_constructors` 172 秒・
`r25_effects` 153 秒）で半分を占める。

## 原因

1. **1 回の起動の固定費。** 空のプログラムの `hado check` が debug で 0.94 秒、release で
   0.23 秒かかる。release の内訳は名前解決 49ms・型推論 141ms・検査 44ms で、どれも
   prelude（`stdlib/*.hd` の 306 行）の解析である。prelude はユーザーのソースの行数だけ
   改行を前置して連結する（`crates/hado/src/lib.rs` の `with_prelude`）ので、解析結果を
   入力をまたいで使い回せない。
2. **最適化なしの dev プロファイル。** 1 の固定費を約 4 倍にする。
3. **直列の反復。** 重いテストは `for padding in 0..20` × 入力 × `check`/`build`/`dump-mono`
   のように 1 本のテストの中で反復し、反復ごとに 1 の固定費を払う。`cargo test` は
   テスト二進も 1 本ずつ回すので、所要は二進ごとの最長のテストの和になる。
4. **コーパスの重複走行。** `tests/corpus.rs`・`check_cli.rs`・verify-m1-corpus・m2・m6・m7
   がそれぞれコーパスを全件回す。

## 手当て（この順に 1 ラウンドずつ）

| 順 | ラウンド | 手当て | 効く原因 |
|---|---|---|---|
| 1 | `dev-opt-level` | `[profile.dev] opt-level = 1` | 2 |
| 2 | 未定 | prelude の解析の固定費を削る（計算量の見直し） | 1 |
| 3 | 未定 | テストと門番の直列の反復を並列にする | 3 |
| 4 | 未定 | 1〜3 の後の実測で要否を決める（重複の削減、CI の層） | 4 |

1 は 1 行で効き、判定を変えない。2 は処理系の利用者（LSP・REPL・`build`）にも効く根本の
手当てで、どこが重いかの特定に perf が要る（この機械は `kernel.perf_event_paranoid=4` で
使えない）。3 は 1 と 2 の倍率に掛かる。各ラウンドの後に CI の所要を測り直し、目標との差を
この表に書き足す。

## 経過

| ラウンド | 手元の `cargo test --offline` | PR の CI（core） |
|---|---|---|
| 着手前 | 39〜47 分（CI の verify-m0。runner は同じ機械の docker） | 81 分 |
| `dev-opt-level` | 9 分 19 秒 | 未測 |
