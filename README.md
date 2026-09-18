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
```

拒否の例:

```
$ cargo run --offline -q -- check tests/reject/E508/main.hd
main.hd:7:3: error[E508]: absence bound at 7:5 is propagated unchanged; use let p? = or do; canonical form: let user? = find_user id -> Some(user + 1)
```

## 検証

```
cargo test --offline                     # 単体テストとコーパス判定
scripts/run-gates.sh --tier quick 出力先 # 保存ごと。ビルドと軽い門番（≦ 2 分）
scripts/run-gates.sh --tier core 出力先  # コミット前。m0〜m7・r38（≦ 7 分）
scripts/run-gates.sh --tier full 出力先  # 節目・夜間。38 本すべて（≦ 18 分）
scripts/run-gates.sh 出力先              # 引数なしは full と同じ（文書層まで回す）
scripts/run-gates.sh --jobs 8 --tier full 出力先 # 同じ判定を 8 並列で（既定 1=直列）
bash scripts/verify-m0.sh                # 門番は単独でも走る
```

`run-gates.sh` は 1 回の全走のあいだ、同じ内容の門番と `cargo test` の実走を 1 度に
畳む（`docs/plan.md` 32 節）。畳み込みは全走のログに「対象の実行」と「ok」の証拠が
あるときだけ効く（33 節）。層に門番を足すときは `--tier core 出力先 verify-r33` の
ように書く。門番を 1 本ずつ別のシェルで走らせると、畳み込みは各門番の入れ子の
範囲でしか効かない。文書の整合は `scripts/lint-docs.sh` が持ち、full の最後に回る。

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
