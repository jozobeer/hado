# hado の開発に参加する

コンパイラ・ランタイム・仕様を変更する開発者向けの案内。言語の利用方法は [README.md](README.md) を参照。

守るべき規則（コーパスのロック、節番号、ラウンドと worktree、着地手順）や文書を読む順、開発環境は [AGENTS.md](AGENTS.md) を参照。

## 検証

```
cargo test --offline                     # 単体テストとコーパス判定
cargo nextest run --offline              # 同じテストを nextest で並列に（門番の全体テストはこちら）
bash scripts/run-gates.sh --tier quick 出力先 # 保存ごと。ビルドと軽い門番（≦ 2 分）
bash scripts/run-gates.sh --tier core 出力先  # コミット前。m0〜m7 と core の門番
bash scripts/run-gates.sh --tier full 出力先  # 節目。門番すべてと文書層（--jobs 4 で約 5 分）
bash scripts/run-gates.sh 出力先              # 引数なしは full と同じ（文書層まで回す）
bash scripts/run-gates.sh --jobs 8 --tier full 出力先 # 同じ判定を 8 並列で（既定 1=直列）
bash scripts/verify-m0.sh                # 門番は単独でも走る
cargo fmt --all --check                  # 整形。verify-rustfmt（quick）と同じ
cargo clippy --offline --all-targets -- -D warnings # lint。verify-clippy（core）と同じ
```

### 門番の実行

全体テストは固定した版の nextest で `scripts/nextest-full.sh` が走らせ、m0 がその結果を `nextest-case` 行で読む（`docs/rounds/nextest.md`）。常設の門番は、製品か AGENTS.md の守ることの契約の変更で赤になり得る検査だけを持つ。変異・探針・判定器の較正のような「テストのテスト」はラウンドの時点で 1 回測り、その `docs/rounds/<slug>.md` の実測に残す（`docs/rounds/prune-meta-gates.md`）。

- 層に個別の門番を追加する場合は `--tier core 出力先 verify-r33` のように指定する。
- 文書の整合性は `scripts/lint-docs.sh` が検証し、full 層の最後に実行される。
- Rust の整形と lint は `scripts/verify-rustfmt.sh`（quick）と `scripts/verify-clippy.sh`（core）が検証する（`docs/rounds/rust-lint.md`）。

### 並列実行（`--jobs N`）

`--jobs N`（既定 1）は門番を N 並列で実行する。合否判定は直列実行と同じである。m0（ビルドと全体テスト）を先に完走させ、残りをファンアウトする。門番は作業ツリーを書かないので、どの組でも並べられる。実行中断時は、親プロセスが全子孫の門番を停止する。`HADO_GATE_TIMES` にファイルを渡すと、門番ごとの所要と終了コードを追記する。

## CI

CI は GitHub Actions で実行する（`.github/workflows/ci.yml`、`docs/rounds/ci.md`）。

- **実行層**: PR は core 層と `scripts/lint-docs.sh`、main への push は full 層を実行する。手動実行（workflow_dispatch）では層を選択できる（既定: full）。
- **層の管理**: 層の選択は `scripts/ci-run.sh` が持つ。
- **依存と制約**: 依存関係は初回に `cargo fetch --locked` で取得し、以降は `--offline` で回す。runner は GitHub の standard runner（`ubuntu-latest`）で、public リポジトリのため無料（`docs/rounds/ci-public.md`）。

## 構成

```
crates/hado/      コンパイラ本体。lexer → parser → 名前解決・型 → 検査 → 単相化 → Cranelift
runtime/          C のランタイム。build.rs が静的ライブラリにする
stdlib/           prelude。hado 自身で書き、全プログラムに暗黙に連結する
editors/          tree-sitter 文法と Neovim のハイライト
tests/accept/     受理コーパス。expect: タグに実行結果を持つ
tests/reject/     拒否コーパス。エラーコードごとに 1 ケース
tests/library/    ライブラリのコーパス（.hd と .hdi）
scripts/          門番スクリプト
docs/             仕様・計画・台帳。常に現行版
AGENTS.md         守る規則と読む順
```

## 文書

`docs/` の文書は常に現行版。過去の版は git tag（`v0.1` など）で読む。

- `docs/getting-started.md` — 最初のプログラム
- `docs/spec.md` — 言語仕様。§56 にエラーコード表と検査の段階順
- `docs/plan.md` — 凍結した開発計画（版ごとの決定記録）。以後の計画は `docs/plans/`
- `docs/plans/` — 版をまたぐ計画と版ごとの決定
- `docs/rounds.md` `docs/rounds/` — ラウンドの識別子・ファイルの形・門番の宣言行。1 ラウンド 1 ファイル
- `docs/corpus.md` — コーパスの規約と門番の設計
- `docs/ledger.md` — R81 までのラウンドの実行記録（凍結）
- `docs/spec-trace.md` `docs/spec-coverage.md` — 仕様の規範文と計画の項目の対応表
- `docs/bug.md` — 修正していない不具合
- `docs/issues.md` `docs/issues/` — 次にやる候補。1 件 1 ファイル。`docs/wants.md` は根拠が揃う前のメモ
- `docs/cranelift-<版>.md` — Cranelift の API の実際の形
