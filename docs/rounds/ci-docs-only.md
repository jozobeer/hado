# 文書だけの PR では門番を回さない（ci-docs-only）

## 手順書

### 背景

PR の CI は差分を見ずに core 層を回す（`docs/rounds/ci.md` の決定 1）。ラウンドの実測を
追記するだけの PR でも、4 vCPU の standard runner で約 14 分かかる
（`docs/rounds/ci-public.md` の走行 38411482495）。

ワークフローの `paths-ignore` では解けない。path の条件で走らなかったワークフローの
チェックは Pending のまま残り、そのチェックを必須にした PR はマージできない
（GitHub Docs「Workflow syntax for GitHub Actions」）。CI は毎回起動し、入口が差分を見て
回すものを選ぶ。

core 以上の門番と `cargo test` が読む文書を、`scripts/` と `crates/` のコメント以外の
参照から引いた（下の実測）。`docs/rounds/`・`docs/issues/`・`docs/plans/`・`README.md`・
`CONTRIBUTING.md` を読むのは文書層（`scripts/lint-docs.sh`）と full 層の
`verify-parallel-rounds` だけである。`docs/spec.md`・`docs/plan.md`・`docs/corpus.md`・
`docs/ledger.md`・`AGENTS.md` などは core の門番の入力である。

### 決定

1. pull_request の差分がすべて次の文書なら、門番を回さず文書層だけを回す。
   `README.md`・`CONTRIBUTING.md`・`docs/rounds/*.md`・`docs/issues/*.md`・`docs/plans/*.md`。
   一覧は `scripts/ci-run.sh` の `docs_only_path` だけに置く。
2. 差分は checkout した merge commit（`refs/pull/N/merge`）と 1 番目の親（取り込み先の
   先端）の比較で取る。PR 全体の差分であり、最後の push の差分ではない。コードを含む PR に
   文書のコミットを足しても core を回す。
3. リネームは `--no-renames` で削除と追加に分ける。コードを文書の場所へ移す PR は、元の
   パスが一覧に無いので core を回す。
4. merge commit でない・差分が取れない・差分が空なら core に倒す。
5. 文書だけの扱いは層ではない。`workflow_dispatch` に層として渡すことはできない。
6. ワークフローの checkout は `fetch-depth: 2` で、merge commit の親を取る。
7. main への push は差分によらず full 層を回す（`docs/rounds/ci.md` の決定 1 のまま）。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| PR の差分が決定 1 の文書だけ（追加・変更・削除） | 門番なし、文書層あり | 決定 1 |
| 文書だけの PR で文書層が赤 | 赤 | 赤を隠さない |
| `docs/spec.md`・`docs/plan.md`・`AGENTS.md` を含む | core 以上 | 背景の参照 |
| ラウンドの文書とコードを含む | core 以上 | 決定 1 |
| コードから `docs/rounds/` へのリネーム | core 以上 | 決定 3 |
| merge commit でない・親が無い | core 以上 | 決定 4 |
| `workflow_dispatch` に `docs-only` | 回さずに落ちる | 決定 5 |

### 成果物

- `scripts/ci-run.sh` — 差分の判定。
- `.github/workflows/ci.yml` — checkout の `fetch-depth`。
- `scripts/verify-ci.sh` — 判定 d と checkout の配線。
- `docs/rounds/ci-docs-only.md` — 本ファイル。

### 工程

1. `scripts/verify-ci.sh` に判定 d を足し、文書だけの PR の検査が赤になることを見る。
2. `scripts/ci-run.sh` に判定を入れ、緑にする。
3. `scripts/ci-run.sh` を 1 箇所ずつ壊し、検査が赤になることを見る。
4. checkout の `fetch-depth` を入れ、外した形と 1 の形で赤を見る。
5. この PR（コードを含む）で core が走り、文書だけの PR で門番が省かれることを実走で見る。

### 判定

- `bash scripts/verify-ci.sh` が `PASSED: 11/11`。否定的対照は各 `10/11`。
- この PR の走行のログに `tier=core` が出る。
- 文書だけの PR の走行のログに `docs-only` と文書層の `PASSED:` が出て、`RUN: verify-` が無い。

### 許容される赤

なし。

### スコープ外

- main への push の層を差分で変えること。
- PR の最後の push だけを見て門番を省くこと（前の走行の結果に依存する）。
- 門番ごとの入力を宣言して、影響のある門番だけを回すこと。

### 中断の条件

GitHub の checkout が `fetch-depth: 2` で merge commit の 2 つの親を取れないなら、差分の
取り方を報告して止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （CI 基盤） | 文書だけの PR は文書層だけを回し、それ以外は core 以上を回す | 道具 | 成功 | `scripts/verify-ci.sh` |

## 実測

core 以上の門番と `cargo test` が読む文書（コメント行を除く参照）。2026-10-13。

```
$ grep -nE "docs/(rounds|issues|plans)|README\.md|CONTRIBUTING\.md|AGENTS\.md" \
    scripts/verify-*.sh scripts/ci-run.sh scripts/run-gates.sh scripts/lib/*.sh crates/*/tests/*.rs \
    | grep -vE "^\S+:[0-9]+:\s*#" | grep -v lint-docs
scripts/verify-parallel-rounds.sh:72:rounds = sorted(Path('docs/rounds').glob('*.md'))
scripts/verify-parallel-rounds.sh:99:    assert (Path('docs/rounds') / f'{slug}.md').is_file(), path.name
scripts/verify-parallel-rounds.sh:132:    python3 - "$tree/docs/rounds/parallel-rounds.md" <<'PY'
scripts/verify-parallel-rounds.sh:154:check 'c each # round: slug is unique, well-formed, and has docs/rounds/<slug>.md' round_header_case
scripts/verify-spec-contracts.sh:75:    針を通さない（docs/rounds/dropped-spec-contracts.md の探針）。左端は見ない
crates/hado/tests/r26_effects.rs:2111:// `docs/issues/recursive-effectful-closure-specialization-reads-outcome-as-int.md`

$ grep -m1 '^# tier:' scripts/verify-parallel-rounds.sh
# tier: full

$ grep -nE "(\"|'| |\()docs(/?\"|/?'|/?\)| |$)" scripts/*.sh scripts/lib/*.sh | grep -vE ":[0-9]+:\s*#"
scripts/verify-m1-corpus.sh:25:find docs runtime scripts -type f -print0 | sort -z | xargs -0 sha256sum > "$TMPD/protected.sha"
scripts/verify-m2.sh:20:find docs runtime scripts -type f -print0 | sort -z | xargs -0 sha256sum > "$TMPD/protected.sha"
```

`verify-spec-contracts.sh:75` は docstring、`r26_effects.rs:2111` はコメントである。
`verify-m1-corpus`・`verify-m2` の `find docs` は走行中の書き換えを見張るだけで、内容を判定
しない。

着地前の検査。

```
$ bash scripts/verify-ci.sh | tail -4
OK: a pull_request that changes only round, issue, and plan documents runs the document layer alone
OK: a pull_request that changes anything else runs at least the core tier
OK: a pull_request whose diff is unknown runs at least the core tier
PASSED: 11/11

$ bash scripts/ci-run.sh workflow_dispatch refs/heads/topic docs-only; echo "exit=$?"
ci-run: unknown tier: docs-only
exit=1

# 否定的対照（scripts/ci-run.sh を 1 箇所ずつ壊す）
=== allowlist-docs-star: NG: a pull_request that changes anything else runs at least the core tier PASSED: 10/11
=== allowlist-agents:    NG: a pull_request that changes anything else runs at least the core tier PASSED: 10/11
=== renames-detected:    NG: a pull_request that changes anything else runs at least the core tier PASSED: 10/11
=== no-merge-check:      NG: a pull_request whose diff is unknown runs at least the core tier PASSED: 10/11
=== skip-doc-layer:      NG: a pull_request that changes only round, issue, and plan documents runs the document layer alone PASSED: 10/11

# 否定的対照（checkout の fetch-depth）
=== drop:   NG: the workflow parses and is wired for the decisions PASSED: 10/11
=== depth1: NG: the workflow parses and is wired for the decisions PASSED: 10/11
```
