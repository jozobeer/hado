# README を言語の利用者向けにする（readme-users）

開始 2026-09-21。

## 手順書

### 背景

README に、言語を使う人の案内（例・必要なもの・命令・エディタ）と、コンパイラを変える人の案内（検証・CI・構成・文書一覧）が混在していた。開発者向けの節を新設の `CONTRIBUTING.md` へ移し、守る規則・読む順・環境は `AGENTS.md` を唯一の置き場のままにする。

### 決定

- **決定 1: 読み手で分ける。** README は言語を使う人、`CONTRIBUTING.md` はコンパイラを変える人の案内とする。`AGENTS.md` は変えず、`CONTRIBUTING.md` からリンクだけ置く。同じ規則を 2 箇所に書かない。
- **決定 2: 最初の一手は `run`。** README の最初の例は `cargo run --offline -q -- run tests/accept/create_user/main.hd` とする（`docs/wants.md` 49）。
- **決定 3: `scripts/run-gates.sh` は `bash` 経由で呼ぶ。** ファイルは git 上 100644 で実行ビットが無く、直接呼ぶと exit 126 で止まる。`scripts/ci-run.sh` と `docs/rounds/ci.md` の呼び方に揃える。
- **決定 4: `scripts/verify-r42.sh` の参照検査に `CONTRIBUTING.md` を足す。** §・付録の参照が `docs/spec.md` の見出しに実在することを README と同じ門番で見る。

### 成果物

- `README.md`: 冒頭に付録B の `create_user` と E501 の拒否例、必要なもの、最初の一手、§58 の全命令と旗、Neovim の設定、仕様と `CONTRIBUTING.md` への導線。命令表の `check` の行は、診断があれば終了コード 1、無ければ 0 と書く。
- `CONTRIBUTING.md`: 旧 README の検証・CI・構成・文書一覧を移し、構成に `editors/` と `tests/library/` を足す。
- `scripts/verify-r42.sh`: 参照検査の対象に `CONTRIBUTING.md` を足す。
- `docs/wants.md`: 49 を着地済みにする。

### 判定

- `bash scripts/verify-r42.sh` が `PASSED: 5/5`。
- `bash scripts/lint-docs.sh` が `PASSED: 10/10`。
- `bash scripts/verify-lsp-minimal.sh` が `PASSED: 11/11`（README の `cmd = { ... }` から Neovim を起動する）。
- `bash scripts/run-gates.sh --tier quick target/gates-quick` が red なし。
- README に載せた例の実走。`run tests/accept/create_user/main.hd` が `ok`、`check` は受理で終了コード 0、拒否で診断と終了コード 1。

### スコープ外

- `hado` を `PATH` に置く手順と配布の案内。
- `scripts/run-gates.sh` に実行ビットを付けること。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （文書運用） | README は言語を使う人、`CONTRIBUTING.md` はコンパイラを変える人の案内。規則は `AGENTS.md` だけが持つ | 文書 | 成功 | `README.md` と `CONTRIBUTING.md` の相互リンク |
| （文書運用） | README の最初の例は `run tests/accept/create_user/main.hd` | 文書 | 成功 | `docs/wants.md` 49 |
| （文書運用） | README と `CONTRIBUTING.md` の §・付録の参照は `docs/spec.md` の見出しに実在する | 文書 | 成功 | `verify-r42.sh` |
| （文書運用） | README の Neovim の設定から LSP が起動する | 文書 | 成功 | `verify-lsp-minimal.sh` |
| （文書運用） | `check` の終了コードは診断があれば 1、無ければ 0 | 文書 | 成功 | 本ファイルの実測（仕様は `check` の終了コードを書いていない） |

## 実測

```
$ cargo run --offline -q -- run tests/accept/create_user/main.hd
ok
$ cargo run --offline -q -- check tests/accept/create_user/main.hd; echo exit=$?
exit=0
$ cargo run --offline -q -- check tests/reject/E508/main.hd; echo exit=$?
main.hd:7:3: error[E508]: absence bound at 7:5 is propagated unchanged; use let p? = or do; canonical form: let user? = find_user id -> Some(user + 1)
exit=1
```

```
$ bash scripts/verify-r42.sh 2>&1 | tail -3
version: v0.4
OK: spec and spec-coverage name v0.4
PASSED: 5/5
$ bash scripts/verify-lsp-minimal.sh 2>&1 | tail -3
OK: root-selection
OK: version-guard
PASSED: 11/11
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 38/38 passed; red: none
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 10/10
```
