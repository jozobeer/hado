# 次版の issue 候補を決める（issues）

開始 2026-09-17。

## 手順書

### 背景

20 行の CLI を試した結果、既存の wants だけでは拾えていなかった標準ライブラリ、診断、エディタ統合の不足が見つかった。候補を実装可能な単位へ分け、次版で先に閉じる範囲を決める。

### 決定

- 候補は `docs/issues/<slug>.md` に現象、原因、案、完了条件、由来を置く。着手後は同じ slug のラウンドファイルが手順と実測を引き継ぐ。
- v0.5 は `prelude-tail-recursion`、`highlight-treesitter`、`lsp-minimal` の順に進める。
- 完了条件は実装経路を直接判定し、依存方向、位置情報、文字境界、道具の再現性を固定する。

### 期待値の根拠

| 対象 | 期待 | 根拠 |
|---|---|---|
| issue 候補 | 現象と原因をソースまたは実測で特定し、独立して着地できる完了条件を持つ | `docs/issues.md` |
| v0.5 | prelude、highlight、LSP の順で着手する | `docs/plan.md` 35 節 |
| 文書 | 現行の節参照とラウンド規約に適合する | `scripts/lint-docs.sh` |

### 成果物

`docs/issues.md`、`docs/issues/*.md`、`docs/plan.md` 35 節、本ファイル。

### 工程

- dogfood の実測から候補を切り出す。
- 各候補の現象、原因、最小案、完了条件を既存コードと仕様に照らす。
- 次版の範囲と順序を `docs/plan.md` に固定する。
- 完了条件と依存関係を再検証する。

### 判定

- `bash scripts/lint-docs.sh` が全件通る。
- `git diff --check` が通る。
- v0.5 の各 slug に候補ファイルがあり、着手後の記録先が同じ slug の `docs/rounds/<slug>.md` と明記されている。

### 許容される赤

終了時に許容される赤は無い。

### スコープ外

候補の実装、仕様・runtime・stdlib・corpus の変更。

### 中断の条件

候補の根拠をソースまたは実測で確認できない場合と、候補の判断が現行仕様と衝突する場合は止めて報告する。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （文書運用） | issue 候補は slug で識別し、着手後は同じ slug のラウンドが手順と実測を持つ | 文書 | 成功 | `lint-docs.sh` |
| （文書運用） | issue の原因は `file:line` を指す | 文書 | 成功 | `lint-docs.sh` 検査 8 |

## 実測

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -2
OK: R38 spec sentences are pinned verbatim once each
PASSED: 7/7
```

### レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 7/7

nvim --version | head -1
NVIM v0.12.5
# カスタム filetype "hado" に lua parser を割り当て、-u NONE（nvim-treesitter 無し）で開く
# filetype=hado / parser=true / active_before_start=false / active_after_start=true

# query は runtimepath の queries/<言語>/<名前>.scm として解決される
RT=$(nvim --headless -u NONE -i NONE -c 'lua io.write(vim.env.VIMRUNTIME)' -c 'qa!' 2>/dev/null)
grep -n "queries/%s/%s.scm" "$RT/lua/vim/treesitter/query.lua"
159:  local query_path = string.format('queries/%s/%s.scm', lang, query_name)

cargo run --offline -q -- check --format json tests/reject/E101/main.hd
main.hd:6:40: error[E101]: branches cannot be nested inside another branch
cargo run --offline -q -- check --format json tests/reject/E505/main.hd
{"path": "main.hd", "line": 4, "col": 7, "code": "E505", "message": "binding unused is unused", "canonical": null, "related": null}
```

### 追加レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -2
OK: each issue cause cites a file:line anchor
PASSED: 8/8

# 検査 8 が空でないこと。原因に file:line の無い一時 issue を docs/issues/ に置くと赤になる
NG: each issue cause cites a file:line anchor
PASSED: 7/8

# def の形（付録C の parameters）。1 つの括弧と引数だけの注釈は正当で、括弧の連続だけ E100
cargo run --offline -q -- check target/pr5/t1/main.hd   # def f x : Int => x
exit=0
cargo run --offline -q -- check target/pr5/t2/main.hd   # def f (a: Int): Int => a
exit=0
cargo run --offline -q -- check target/pr5/t3/main.hd   # def f (a: Int) (b: Int): Int => a
main.hd:1:16: error[E100]: expected =>, found (
```

### 四次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 8/8

grep -n '%.17g' runtime/hado_runtime.c
462:  check_stdout(printf("%.17g\n", value) >= 0);
514:  int length = snprintf(buffer, sizeof(buffer), "%.17g", value);

sed -n '9p;31p' stdlib/list.hd
def A sort(values: A List, comparator: Fn(A, A => Int)): A List
def rec sort values comparator => if values | [] => [] | [_] => values | [_, _, ..._] => odds values |> sort comparator |> merge (evens values |> sort comparator) comparator

grep -n 'def main = 1 + 2' crates/hado/tests/walk.rs
15:        ("def main = 1 + 2", "3"),
grep -n 'def main = \$int_value' scripts/verify-r29.sh
108:if ! build_kind int "def main = $int_value" \
grep -n 'def main = Int.abs' scripts/verify-r43.sh
245:    source = f'def main = Int.abs {expression(value)}\n'
```
