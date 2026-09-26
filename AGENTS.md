# AGENTS.md

## 読む順

1. `docs/plan.md` — 開発計画と版ごとの決定記録。ラウンドの手順書は `docs/rounds/`
2. `docs/rounds.md` — ラウンドの識別子・ファイルの形・門番の宣言行
3. `docs/issues.md` — 次にやる候補の置き方。候補は `docs/issues/<slug>.md`、着手でラウンドの slug になる
4. `docs/corpus.md` — コーパスと門番の規約
5. `docs/spec.md` — 言語仕様。全ての判断の根拠
6. `docs/ledger.md` — R81 までのラウンドの実行記録（凍結）。以後の実測は各 `docs/rounds/<slug>.md`

`docs/` の文書は常に現行版で、過去の版は git tag（`v0.1` など）が持つ。文書が食い違ったら仕様本文を正とし、食い違いを報告に書く。計画の中での優先順位は `docs/plan.md` が示す。版をまたぐ作業の計画は 1 本 1 ファイルで `docs/plans/<名前>.md` に置き、`docs/plan.md` に節を足さない。

## 環境

- Rust（rustfmt と clippy を含む）、`cc`、`libgc-dev`。Cranelift の版は `Cargo.lock` に固定
- `python3`（`scripts/corpus-lock.sh`・門番）。`scripts/spec-lock.sh` と `scripts/verify-spec-model.sh` は TOML を読むので 3.11 以上（3.10 以前は `tomli` を入れる）。`scripts/verify-ci.sh` はワークフローを YAML として読むので `yaml` も要る（Ubuntu は `python3-yaml`）
- `CARGO_TARGET_DIR` は設定しない。target は worktree ごとに `./target` に分かれる。バイナリのパスを探さず `cargo run --offline -q -- <args>` で呼ぶ
- Cranelift の API は docs.rs の latest と食い違う。`docs/cranelift-<版>.md` を先に読む
- `curl`・`tar`・`gunzip`・`sha256sum`（coreutils）。`scripts/treesitter-setup.sh` が固定した版の tree-sitter CLI と Neovim を取ってきて展開するのに使う

## コマンド

```
cargo test --offline                 # 単体テストとコーパス判定
cargo fmt --all --check                  # 整形（門番 verify-rustfmt、quick）
cargo clippy --offline --all-targets -- -D warnings # lint（門番 verify-clippy、core）
bash scripts/verify-*.sh             # リポジトリルートから。最後に PASSED: n/N
bash scripts/corpus-lock.sh [--check]   # corpus lock を作り直す / 照合する
cargo run --offline -q -- check [--print-type] <file>
cargo run --offline -q -- dump-mono <file>
cargo run --offline -q -- build <file>        # 入力の隣に実行ファイルを置く
```

## 守ること

- **コーパスは既定でロック。** `tests/accept` `tests/reject` `stdlib/` は、そのラウンドの成果物に含まれるときだけ変える。`tests/corpus.sha256` を実装の工程で更新しない。main を取り込んだ後の作り直し（`scripts/corpus-lock.sh`）だけが例外
- **仕様の節番号は増やしても動かさない。** 本文の節は `## N.`（`N` は整数）。新しい節は直前に置きたい節の直後に小数で入れ（例 `§N.M`）、整数は本文の末尾にだけ足す。付録は `## 付録A` から順に足し、本文の番号を使わない。corpus の `// spec:`・エラー表の節欄・門番はこの表記（`§N`・`付録A`）をそのまま使う
- **拒否コーパスは 1 ファイル 1 欠陥。** ヘッダの `error:` タグのコードが、仕様の検査段階順で最初に当たる唯一の欠陥でなければならない
- **件数を直書きしない。** 受理・拒否の本数は仕様のエラーコード表の行数とディスクの実数から導く
- **判定器に暗記を持ち込まない。** コーパスのファイル名や行を判定器へ埋め込まない。期待値は判定器側で計算する
- **門番を緩めて green にしない。** 赤の門番は理由とともに報告する
- **門番は小さく回す。** 開発中は触ったところに効く 1 本（`bash scripts/verify-<名>.sh`）だけを回す。`scripts/run-gates.sh` の層別（quick / core / full）と `cargo test` の全走は、そのラウンドの最後に 1 回だけにする
- **木が変わらないなら門番を回さない。** コミットメッセージの言い直しや履歴の整理のように、追跡ファイルの中身が変わらない変更では `cargo test` も層別も回さない。
- **門番の走行中は木を触らない。** `verify-m1-corpus.sh` の検査 7 は `docs` `runtime` `scripts` を開始時に固定する。全走と文書の編集は直列にする（走行中の編集は `protected-files-changed-during-verification` の偽の赤になる）
- **仕様に疑義が出たら仕様を変えない。** 解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める
- **ラウンドは slug で識別し、1 ラウンド 1 worktree。** `git worktree add .worktrees/<slug> -b <slug>` で切り、`docs/rounds/<slug>.md` に手順書・系譜・実測を書く。`docs/plan.md` と `docs/ledger.md` と `docs/spec-trace.md` にラウンドの節や行を足さない（`docs/rounds.md`）。`docs/plan.md` に残る版ごとの決定（`v0.x の範囲と決定`）と門番基盤の節は、並走させず 1 本のラウンドが順に書く。2 本が同じ版の表を触るなら、先に着地する側を決めてから始める。
- **門番は自分の層とラウンドを宣言する。** 新しい `scripts/verify-*.sh` は冒頭に `# tier: quick|core|full` と `# round: <slug>` を置く。`scripts/lib/gate-tiers.sh` に一覧は無い
- **着地の直前に main を取り込み、lock を作り直す。** `git merge main`（または rebase）の後、まず `bash scripts/corpus-lock.sh --check` の差分を読み、変わる行が「main 側の変更」と「そのラウンドの成果物」だけで説明できることを確かめてから `bash scripts/corpus-lock.sh` で作り直す。説明できない行があれば作り直さずに止めて報告する。そのラウンドの門番と quick 層を回してから PR にする。着地後の全走は main を checkout した側が回し、結果は同じ `docs/rounds/<slug>.md` の `## 実測` に追記する `docs:` コミット 1 本を PR で足す（main に直接は書かない）

## レビューと判断の振り分け

レビューの往復は人の注意力を使う。指摘は次の 3 つに振り分け、人が見るのは 3 番目だけにする。

- **機械で決まる整合は門番に任せる。** 文書どうしの関係（節番号・版・plan の表との対応・アンカーの実在・ファイルの形）は `scripts/lint-docs.sh` と `scripts/verify-*.sh` が決める。レビューで指摘されたら判断を人に上げず、門番の検査か探針として取り込み、以後は門番が判定する。
- **規約は決めたら記録して二度聞かない。** 綴りや参照の書き方の判断（例: 実測フェンスの綴り、範囲の区切り、アンカーに行番号を使うか）は `docs/rounds.md` か `docs/issues.md` に日付つきで 1 行書く。記録済みの指摘にはその行を返して resolve する（対応しない）。その場で決めて記録しないことを繰り返さない。
- **往復に予算を置く。** 1 つの PR のレビュー対応は 3 往復まで。残った指摘は `docs/issues/<slug>.md` に切り出し、PR 本文に切り出し先を書いて打ち切る。打ち切りは人が決める（未解決スレッドが残っていてもよい）。
- **門番を賢くして収束を狙わない。** 散文や別言語の綴りを推測する検査は指摘の形が無限に作れるので足さない。真偽が要る実測は手で書かせず、走らせた側に生成させる（`scripts/corpus-lock.sh` と同じ形）。

人が見るのは、仕様の解釈・優先度・実測の妥当性・計画に無い決定だけにする。上げるときは次の 5 行に揃える。

```
主張:     何を決めるのか（1 行）
再現:     コマンドと出力（最小の入力）
現行:     いまの値
変えると: 何がどう変わるか、費用
推奨:     推奨と理由（1 行）
```

## コミットと報告

- 工程ごとにコミットする（振る舞いの変更と構造の変更は必ず分ける）。形式は `feat:` `test:` `docs:` `refactor:` に続けて、短い体言止めで書く（主語は目安 40 文字まで）。例: `feat: 効果操作のキーワードを once / again に改名`。本文には理由や経緯などコンテキストを書く。
- 報告は完了時と、依存する作業で止まったときに返す。内容は (1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点
