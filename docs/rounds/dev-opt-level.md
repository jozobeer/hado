# dev プロファイルを opt-level 1 にしてテストの固定費を下げる（dev-opt-level）

## 手順書

### 背景

`docs/plans/test-speed.md` の手当ての 1 本目。テストと門番は反復のたびに debug ビルドの
`hado` を起動し、そのたびに prelude を解析する。その固定費が opt-level 0 では 1 回約 1 秒
になり、verify-m0（`cargo test` の全走）だけで CI の 40 分を占めている。

### 決定

- ワークスペースの `Cargo.toml` に `[profile.dev] opt-level = 1` を置く。`cargo test` の
  `test` プロファイルは `dev` を継ぐので、テスト二進と `CARGO_BIN_EXE_hado` の両方に効く。
- `debug-assertions` と `overflow-checks` は dev の既定（有効）のまま触らない。判定に関わる
  検査は変わらない。
- 却下: 依存だけを最適化する（`[profile.dev.package."*"] opt-level = 3`）。固定費は
  hado 自身の名前解決・型推論・検査にあり、実測で縮まない（下の実測）。
- 却下: opt-level 2 以上。release（opt-level 3）でも固定費は 0.23 秒で、opt-level 1 の
  0.31 秒からの伸びしろは小さい。残りは計算量の問題として別ラウンドで扱う。

### 期待値の根拠

- 固定費（空のプログラムの `hado check`）: opt-level 0 で 1.0 秒、1 で 0.31 秒。
- `cargo test --test r9_names`: opt-level 0 で 152.7 秒、1 で 44.2 秒（3.45 倍）。
- 1 行の変更の後のテスト二進の再ビルド: opt-level 0 で 6.4 秒、1 で 5.7 秒。開発ループの
  ビルド待ちは増えない。

### 成果物

- `Cargo.toml` — `[profile.dev]`。
- `docs/plans/test-speed.md` — 目標・原因・手当ての順。
- `docs/rounds/dev-opt-level.md` — 本ファイル。

### 工程

1. opt-level 0 / 1 / 依存だけ 3 で、固定費・テスト二進 1 本・再ビルドを測る。
2. `Cargo.toml` に `[profile.dev]` を置く。
3. `cargo test --offline --no-fail-fast` の所要を測る。
4. verify-m0 と quick 層を回す。
5. PR の CI（core 層）の所要を測り、`docs/plans/test-speed.md` の表に書き足す。

### 判定

- `cargo test --offline --no-fail-fast` が exit 0。
- `bash scripts/verify-m0.sh` が `PASSED: 8/8`。
- `bash scripts/run-gates.sh --tier quick` が `red: none`。
- 手元の `cargo test --offline --no-fail-fast` の所要が、CI の verify-m0 の 39〜47 分より
  短い（runner は同じ機械の docker で、CPU は 12）。

### 許容される赤

なし。

### スコープ外

- prelude の解析の計算量、テストの並列化、コーパスの重複（`docs/plans/test-speed.md` の 2〜4）。

### 中断の条件

opt-level 1 で判定が変わる門番やテストがあれば、原因（未定義動作・時間の閾値）を報告して止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （開発基盤） | テストと門番の hado は opt-level 1 でビルドする | 道具 | 成功 | `Cargo.toml` |

## 実測

固定費（空のプログラムの `hado check`、2026-10-13、12 CPU）。`depsopt` は依存だけ opt-level 3。

```
dev check .997523495
o1 check .306596359
depsopt check 1.074386652
release check .227380027
```

テスト二進 1 本と、`crates/hado/src/types.rs` に 1 行足した後の再ビルド。

```
test result: ok. 16 passed; ... finished in 152.70s   # dev  --test r9_names
test result: ok. 16 passed; ... finished in 44.22s    # o1   --test r9_names
dev test-rebuild 6.435851580
o1 test-rebuild 5.663047042
```

全走（opt-level 1）。CI の verify-m0 は PR 38305183510 で 47 分、main 38290048902 で 39 分。

```
$ cargo test --offline --no-fail-fast
EXIT=0 WALL=559
sum of finished-in 557.79s, passed 850

$ bash scripts/verify-m0.sh | tail -1
PASSED: 8/8
M0 EXIT=0 WALL=561

$ bash scripts/run-gates.sh --jobs 12 --tier quick target/q | tail -1
TALLY: 51/51 passed; red: none
```
