# Rust を 1.99.0 に上げて版を固定する（rust-1-99）

## 手順書

### 背景

Rust の版はどこにも固定されていない。手元は 1.98.1 で、CI の自前 runner は `stable` のツールチェーンを使い、走行 38388275944 の `toolchain` の段では `rustc 1.98.1` だった。runner の `stable` が進むと、手元と CI の版が黙ってずれる。最新の stable は 1.99.0（2026-09-28）である。

### 決定

- `rust-toolchain.toml` で 1.99.0 と rustfmt・clippy を固定する。手元の rustup 1.29.1 は、固定された版が無ければ最初の `cargo` で入れた（実測）。CI のワークフローは変えない。`toolchain` の段の `rustc --version` に固定した版が出ることで追従を確かめる。
- 環境変数 `RUSTUP_TOOLCHAIN` は `rust-toolchain.toml` より優先される。AGENTS.md の環境に、設定しないことを書く。
- 却下: ワークフローに `rustup toolchain install` の段を足す。`rust-toolchain.toml` と版を 2 か所に持つことになる。

### 期待値の根拠

- 1.99.0 で rustfmt の差分と clippy の警告が無く、全体テストの件数と結果が 1.98.1 と同じなら、門番の判定は変わらない。
- `Cargo.toml` に `rust-version` は無く、Cranelift 0.135 の `rust-version` は 1.95.0（`docs/cranelift-0.135.md`）。

### 成果物

- `rust-toolchain.toml`
- `AGENTS.md` の環境・`README.md` の必要なもの
- 本ファイル

### 工程

1. `rust-toolchain.toml` を置き、rustup が 1.99.0 を自動で入れることを見る。
2. 1.99.0 で `cargo fmt --all --check`・`cargo clippy --offline --all-targets -- -D warnings`・`cargo test --offline --no-fail-fast` を回す。
3. core 層を回す。
4. PR の CI の `toolchain` の段に `rustc 1.99.0` が出ることを見る。

### 判定

- `rustc --version` が `rustc 1.99.0`。
- rustfmt と clippy が 0 で終わり、全体テストが 854 passed。
- core 層の `TALLY` に赤が無い。
- PR の CI の `toolchain` の段が `rustc 1.99.0`。

### 許容される赤

なし。

### スコープ外

- 依存の更新（`Cargo.lock`）。
- `target/` のキャッシュ。
- `edition` の更新。

### 中断の条件

CI の runner の rustup が固定した版を自動で入れなければ、ワークフローで入れる段を足すかを報告して止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （開発環境） | Rust の版は `rust-toolchain.toml` の 1 か所で決まり、手元と CI が同じ版で走る | 道具 | 成功 | `rust-toolchain.toml` |

## 実測

2026-10-13、f546cfa の上。

```
$ rustup show active-toolchain
1.99.0-x86_64-unknown-linux-gnu (overridden by '.../rust-toolchain.toml')
$ cargo --version   # 未導入の 1.99.0 を rustup が入れる（24 秒）
cargo 1.99.0 (5f94df478 2026-08-27)
$ rustc --version
rustc 1.99.0 (b940084d7 2026-09-28)
$ cargo fmt --all --check
fmt rc=0
$ cargo clippy --offline --all-targets -- -D warnings
clippy rc=0 secs=26
$ cargo test --offline --no-fail-fast
rc=0 secs=207
passed=854 failed=0 binaries=75
```

core 層（1.99.0、`--jobs 4`）。

```
$ bash scripts/run-gates.sh --jobs 4 --tier core out
TALLY: 765/765 passed; red: none
rc=0 secs=384
```
