# hado

ML 系の型システムと ADT を核に、より小さく、より平坦に、より一意に書ける言語。Rust で書いたコンパイラが検査と実行ファイル生成を行う。

仕様 `docs/spec.md` §1 の 2 原則:

> **Control flow should be flat.**

> **If code can be removed without changing meaning, it should not compile.**

## 必要なもの

- Rust（Cranelift 0.135 が 1.95 以上を要求。1.98 で実測済み）
- `cc`（リンクに使う）
- `libgc-dev`（Boehm GC）

## 使い方

`cargo build --offline` でコンパイラを作る。最初のプログラムは [docs/getting-started.md](docs/getting-started.md)。`cargo run` 経由で呼び、バイナリの出力先に依存しない。

```
$ cargo run --offline -q -- check [--print-type] <file> # 検査。--print-type で main の型を印字
$ cargo run --offline -q -- dump-mono <file>            # 単相化後のプログラムを印字
$ cargo run --offline -q -- build <file.hd>              # 入力の隣に実行ファイルを置く
$ cargo run --offline -q -- run <file.hd>                # 実行ファイルを一時ディレクトリに作り、隣に何も残さない
$ cargo run --offline -q -- lsp                          # LSP サーバ（stdio）。エディタから使う
```

`check` は既定で先頭の段階の診断を 1 件だけ出す。`--show-all` を足すと同じ段階の
診断を全件、アンカー順に出す（字句・構文の致命と型の致命は 1 件のまま）。`check
--print-type=<名前>` は、網羅性・冗長・契約の検査で止まっていても型を返す（字句・
構文・名前解決・型の致命では型が無い）。

拒否の例:

```
$ cargo run --offline -q -- check tests/reject/E508/main.hd
main.hd:7:3: error[E508]: absence bound at 7:5 is propagated unchanged; use let p? = or do; canonical form: let user? = find_user id -> Some(user + 1)
```

## エディタ（Neovim）

`hado lsp` は LSP を stdio で話す。Neovim 0.12 の組み込み client から使う。設定は
数行で、`vim.lsp.config` に `cmd` と filetype を渡して `vim.lsp.enable` で有効にする。

```lua
vim.filetype.add({ extension = { hd = "hado", hdi = "hado" } })
vim.lsp.config("hado", {
  cmd = { "<checkout>/scripts/hado-lsp.sh" },
  filetypes = { "hado" },
})
vim.lsp.enable("hado")
```

`scripts/hado-lsp.sh` は自分の位置からリポジトリ根を求め、`cargo run --offline -q --
lsp` を実行する。`hado` を `PATH` に置く必要は無い（`<checkout>` はこのリポジトリの
絶対パスに置き換える）。

診断・hover（カーソル下の最上位 `def` の型）・「正準形へ置換」の code action の 3 つが
効く。code action は `check --fix` と同じ書き換えを返す。`.hdi` の契約も `hado` として
開き、診断と hover を返す（契約には書き換え対象の式が無いので code action は空になる）。
補完・rename・go to definition は入っていない。

## 検証

```
cargo test --offline                     # 単体テストとコーパス判定
scripts/run-gates.sh --tier quick 出力先 # 保存ごと。ビルドと軽い門番（≦ 2 分）
scripts/run-gates.sh --tier core 出力先  # コミット前。m0〜m7・r38（≦ 7 分）
scripts/run-gates.sh --tier full 出力先  # 節目・夜間。38 本すべて（≦ 18 分）
scripts/run-gates.sh 出力先              # 引数なしは full と同じ（文書層まで回す）
scripts/run-gates.sh --jobs 8 --tier full 出力先 # 同じ判定を 8 並列で（既定 1=直列）
bash scripts/verify-m0.sh                # 門番は単独でも走る
cargo fmt --all --check                  # 整形。verify-rustfmt（quick）と同じ
cargo clippy --offline --all-targets -- -D warnings # lint。verify-clippy（core）と同じ
```

`run-gates.sh` は 1 回の全走のあいだ、同じ内容の門番と `cargo test` の実走を 1 度に
畳む（`docs/plan.md` 32 節）。畳み込みは全走のログに「対象の実行」と「ok」の証拠が
あるときだけ効く（33 節）。層に門番を足すときは `--tier core 出力先 verify-r33` の
ように書く。門番を 1 本ずつ別のシェルで走らせると、畳み込みは各門番の入れ子の
範囲でしか効かない。文書の整合は `scripts/lint-docs.sh` が持ち、full の最後に回る。
Rust の整形と lint は `scripts/verify-rustfmt.sh`（quick）と `scripts/verify-clippy.sh`
（core）が守る（`docs/rounds/rust-lint.md`）。

`--jobs N`（既定 1）は門番を N 並列で走らせる（34 節）。判定は変えず、直列と
同じ `PASSED: n/N` を出す。既定が直列なのは再現性のためで、`--jobs` なしの
挙動は 33 節と変わらない。共有ツリーを書き換える門番（m0・m1-corpus・m2・m3・
gate-cache）は前段で 1 本ずつ完走させてから残りをファンアウトする。ゲート
キャッシュの鍵は作業ツリーの内容なので、これらを並列に走らせると他の門番の
結果が揺れるためである。前段の後とファンアウトの後で内容ハッシュを比べ、
変わっていれば分類漏れとして赤にする。前段に赤があれば後段は直列へ落とす
（赤い門番が probe を戻し切らずに終わったとき、鍵が揺れて入れ子が再走するのを
避ける。赤は既に確定しているので判定は緩まない）。走行自身がログを書く出力先は
内容ハッシュから外してあるので、`--jobs` の出力先は `target/` の中でもツリーの
外でもよい。`HADO_GATE_TIMES` と `HADO_GATE_RUNS` も `target/` の中かツリーの外に
置く。中断すると親は子孫ごとワーカーを止めてから走行スコープを消す。

CI は GitHub Actions が回す（`.github/workflows/ci.yml`、`docs/rounds/ci.md`）。PR は core 層と
`scripts/lint-docs.sh`、main への push は full 層。層の選択は `scripts/ci-run.sh` が持ち、
`scripts/verify-ci.sh`（quick）が固定する。手動実行（workflow_dispatch）では層を選べる
（既定 full）。依存は `cargo fetch --locked` で取得し、以後は `--offline` で回す。private
リポジトリなので branch protection は使えず、CI は必須チェックにはできない。

## 構成

```
crates/hado/      コンパイラ本体。lexer → parser → 名前解決・型 → 検査 → 単相化 → Cranelift
runtime/          C のランタイム。build.rs が静的ライブラリにする
stdlib/           prelude。hado 自身で書き、全プログラムに暗黙に連結する
tests/accept/     受理コーパス。expect: タグに実行結果を持つ
tests/reject/     拒否コーパス。エラーコードごとに 1 ケース
scripts/          門番スクリプト
docs/             仕様・計画・台帳。常に現行版
AGENTS.md         エージェント向けの規則
```

## 文書

`docs/` の文書は常に現行版。過去の版は git tag（`v0.1` など）で読む。

- `docs/getting-started.md` — 最初のプログラム
- `docs/spec.md` — 言語仕様。§56 にエラーコード表と検査の段階順
- `docs/plan.md` — 開発計画。決定記録、ラウンドの手順書、判断の解説
- `docs/corpus.md` — コーパスの規約と門番の設計
- `docs/ledger.md` — ラウンドごとの実行記録
- `docs/spec-trace.md` `docs/spec-coverage.md` — 仕様の規範文と計画の項目の対応表
- `docs/bug.md` — 修正していない不具合
- `docs/issues.md` `docs/issues/` — 次にやる候補。1 件 1 ファイル。`docs/wants.md` は根拠が揃う前のメモ
- `docs/cranelift-<版>.md` — Cranelift の API の実際の形
