# 実測の生成と前方領域のスキーマで門番を減らす（lint-docs-hardening）
優先度: 中。

## 現象

PR #5 で検査 9 を実測の約束だけに縮め（`docs/rounds/lint-docs-evidence.md`）、`stable-anchors` で原因を行番号から安定な識別子に変えた（`docs/rounds/stable-anchors.md`）。残るのは、正しい綴りが赤になる側と、実測でないフェンスが通る側の綴りである。`doc_issue_evidence` と `doc_issue_names` の python ブロックを `exec` して呼んだ値が次の表である。

| 綴り | 現行の検査の値 |
| --- | --- |
| 3 スペース字下げしたフェンス（Markdown は開きとして認める） | `complete_fence=False` |
| 引用の中の表（`> \| a \| b \|`） | `complete_table=False` |
| 見出しだけで本文が空の `## 案` `## 完了条件` `## 由来` | `issue_problem=None`（通る。`docs/issues.md` は案・完了条件・由来を要求する） |

そのうえに手書きの実測が残る。`$ ` と出力を手で貼る限り、出力の無いフェンス、古い値を写した実測、走らせていないコマンドが通る。

## 原因

- `scripts/lint-docs.sh` の `fence_open` が行頭のバッククォートだけを開きとして認めるので、Markdown が許す 3 スペースまでの字下げを開きとして読まない。
- `scripts/lint-docs.sh` の `outside_fences` と `complete_table` が、行頭の表だけを実測として読む（引用の中の表を読まない）。
- `scripts/lint-docs.sh` の `issue_problem` が節の見出しと順だけを見るので、見出しだけで本文が空の節が通る。
- 実測は `docs/issues.md` の約束に従って手で書く。走らせた側が生成する仕組みが無いので、門番は綴りしか見られない。
- 前方領域の `優先度:` `# round:` `# tier:` は正規表現で読む。必須項目と字種の一覧が門番の中に散っている。

## 案

- 案 A（推奨）: 実測を生成物にする。`scripts/record-round.sh <slug>` が走らせたコマンドと出力を実測ブロックに書き、門番は `--check` でバイト一致を見る（`scripts/corpus-lock.sh` と同じ形）。手書きの実測が無くなり、出力の無いフェンス・古い値・プレースホルダの穴がまとめて消える。
- 案 B: 前方領域をスキーマにする。`優先度:` `# round:` `# tier:` を TOML にして、検査 10 の正規表現をスキーマ検証に置き換える。
- 案 C: 字下げしたフェンス・引用の中の表・空の節だけを直す（`docs/issues.md` の規約に行頭と引用の外を明記し、検査 10 に節の本文の検査を足す）。小さい。

推奨は A、次に B。C は A と B を採るまでの間の小さな補修である。

## 完了条件

- 案 A を採る場合: 実測ブロックが `scripts/record-round.sh` の生成物になり、`--check` がバイト一致を見る。手で書き換えた実測・出力の無いフェンス・プレースホルダだけの出力の 3 種が赤になる探針で測る。
- 案 B を採る場合: issue とラウンドの前方領域が TOML になり、必須項目の欠落と未知の優先度が赤になる探針で測る。検査 10 の正規表現は消える。
- 案 C を採る場合: 字下げしたフェンスと引用の中の表が実測として読まれ、本文の空の節が赤になる探針で測る。
- `bash scripts/lint-docs.sh` が `PASSED: 10/10`、`git diff --check` が 0、`cargo test --offline` が 0 failed、`bash scripts/run-gates.sh --tier full --jobs 4` が red なし。

## 由来

`docs/wants.md` の番号は無い。PR #5 の四十次〜四十二次のレビュー指摘と、`docs/rounds/issues.md` の三十五〜四十二次の実測から。門番基盤（`docs/plan.md` 35 節）。
