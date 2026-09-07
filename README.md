# shin

ML 系の型システムと ADT を核に、より小さく、より平坦に、より一意に書ける言語。Rust で書いたコンパイラが検査と実行ファイル生成を行う。

仕様 `docs/spec.0.1.md` §1 の 2 原則:

> **Control flow should be flat.**

> **If code can be removed without changing meaning, it should not compile.**

## 必要なもの

- Rust（Cranelift 0.135 が 1.95 以上を要求。1.98 で実測済み）
- `cc`（リンクに使う）
- `libgc-dev`（Boehm GC）

## 使い方

`cargo build --offline` で `target/debug/shin` ができる（`CARGO_TARGET_DIR` を設定していればその下）。

```
shin check [--print-type] <file>    # 検査。--print-type で main の型を印字
shin dump-mono <file>               # 単相化後のプログラムを印字
shin build <file.shin>              # 入力の隣に実行ファイルを置く
```

`tests/accept/create_user.shin` を別の場所へコピーして試す（コーパスの隣に実行ファイルを作らないため）。

```
$ shin check --print-type hello.shin
Result<String,String>
$ shin build hello.shin && ./hello
ok
```

`main` の値が stdout に出て終了コード 0。`Error(e)` なら stderr に出て終了コード 1。

拒否の例:

```
$ shin check tests/reject/E508.shin
tests/reject/E508.shin:6:21: error[E508]: failure bound at 6:5 is propagated unchanged; use let? or do?
```

## 検証

```
cargo test --offline          # 単体テストとコーパス判定
bash scripts/verify-m<N>.sh   # M0〜M7 の門番。最後に PASSED: n/N
```

2026-09-07 時点で M0〜M7 の実装は済み、R7 で改訂した仕様にコーパスと実装を追随させる R8〜R9 が未着手。コーパスと実装が R7 の仕様にまだ追随していないため `verify-m1-corpus.sh` `verify-m2.sh` `verify-m3.sh` は赤で、`PLAN.md` はこれを前提にしている。赤の内訳は `PLAN.md` の冒頭にある。

## 構成

```
crates/shin/      コンパイラ本体。lexer → parser → 名前解決・型 → 検査 → 単相化 → Cranelift
runtime/          C のランタイム。build.rs が静的ライブラリにする
stdlib/           prelude。shin 自身で書き、全プログラムに暗黙に連結する
tests/accept/     受理コーパス。-- expect: に実行結果を持つ
tests/reject/     拒否コーパス。エラーコードごとに 1 ファイル
scripts/          門番スクリプト
docs/             仕様・計画・台帳
PLAN.md           現在のラウンドの手順書
AGENTS.md         エージェント向けの規則
```

## 文書

- `docs/spec.0.1.md` — 言語仕様。§50 にエラーコード表と検査の段階順
- `docs/plan.0.1.md` — 開発計画。決定記録、マイルストーン、判断の解説
- `docs/corpus.0.1.md` — コーパスの規約と門番の設計
- `docs/ledger.0.1.md` — マルチエージェント実行の台帳
- `docs/spec-trace.0.1.md` `docs/spec-coverage.0.1.md` — 仕様の規範文と計画の項目の対応表
- `docs/cranelift-0.135.md` — Cranelift 0.135 の API の実際の形
