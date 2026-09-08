# AGENTS.md

## 読む順

1. `docs/plan.md` — 開発計画、決定記録、現在のラウンドの手順書。触ってよい場所と判定条件はここが決める
2. `docs/corpus.md` — コーパスと門番の規約
3. `docs/spec.md` — 言語仕様。全ての判断の根拠
4. `docs/ledger.md` — ラウンドごとの実行記録

`docs/` の文書は常に現行版で、過去の版は git tag（`v0.1` など）が持つ。文書が食い違ったら仕様本文を正とし、食い違いを報告に書く。計画の中での優先順位は `docs/plan.md` が示す。

## 環境

- Rust、`cc`、`libgc-dev`。Cranelift の版は `Cargo.lock` に固定
- `CARGO_TARGET_DIR` が `~/.cargo/target` に向いている。バイナリのパスを探さず `cargo run --offline -q -- <args>` で呼ぶ
- Cranelift の API は docs.rs の latest と食い違う。`docs/cranelift-<版>.md` を先に読む

## コマンド

```
cargo test --offline                 # 単体テストとコーパス判定
bash scripts/verify-*.sh             # リポジトリルートから。最後に PASSED: n/N
cargo run --offline -q -- check [--print-type] <file>
cargo run --offline -q -- dump-mono <file>
cargo run --offline -q -- build <file>        # 入力の隣に実行ファイルを置く
```

## 守ること

- **コーパスは既定でロック。** `tests/accept` `tests/reject` `stdlib/` は、そのラウンドの成果物に含まれるときだけ変える。`tests/corpus.sha256` を実装の工程で更新しない
- **拒否コーパスは 1 ファイル 1 欠陥。** ヘッダの `error:` タグのコードが、仕様の検査段階順で最初に当たる唯一の欠陥でなければならない
- **件数を直書きしない。** 受理・拒否の本数は仕様のエラーコード表の行数とディスクの実数から導く
- **判定器に暗記を持ち込まない。** コーパスのファイル名や行を判定器へ埋め込まない。期待値は判定器側で計算する
- **門番を緩めて green にしない。** 赤の門番は理由とともに報告する
- **仕様に疑義が出たら仕様を変えない。** 解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める

## コミットと報告

- 工程ごとにコミットする（振る舞いの変更と構造の変更は必ず分ける）。形式は `feat:` `test:` `docs:` に日本語の命令形（`git log` に倣う）。コミットログには理由や経緯などコンテキストを書く。
- 報告は完了時と、依存する作業で止まったときに返す。内容は (1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点
