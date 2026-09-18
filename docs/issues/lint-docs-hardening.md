# 検査 8〜10 の残りを、生成物と安定 ID とスキーマで減らす（lint-docs-hardening）
優先度: 中。

## 現象

PR #5 で検査 9 を実測の約束（`$ ` がコマンド、`> ` が PS2、それ以外が出力）だけに縮め、シェル文法の解析を消した（`docs/rounds/lint-docs-evidence.md`）。残るのは、正しい綴りが赤になる側と、実測でないフェンスが通る側の綴りである。`doc_issue_causes` と `doc_issue_evidence` の python ブロックを `exec` して呼んだ値が次の表である。

| 綴り | 現行の検査の値 |
| --- | --- |
| 3 スペース字下げしたフェンス（Markdown は開きとして認める） | `complete_fence=False` |
| 引用の中の表（`> \| a \| b \|`） | `complete_table=False` |
| 原因の `file:///work/app/x.hdi:1` | `causes=[('work/app/x.hdi', '1', '')]`（実在しない相対パスとして赤になる） |
| 原因の `crates/hado/src/mono.rs:4407–4532`（en ダッシュの範囲） | `causes=[('crates/hado/src/mono.rs', '4407', '')]`（範囲として読まない） |
| CRLF の終端フェンス | `complete_fence=True`（問題なし） |
| heredoc の本文だけのフェンス（`$ cat <<EOF` と本文と `EOF`） | `complete_fence=True`（本文を出力と数える。約束どおりで、対応はレビューで見る） |
| 見出しだけで本文が空の `## 案` `## 完了条件` `## 由来` | `issue_problem=None`（通る。`docs/issues.md` は案・完了条件・由来を要求する） |

いちばん大きいのは行番号の参照である。`file:line` は安定でなく、PR #5 の間に `crates/hado/src/mono.rs` の `3845` と `3962`、`4407` と `4532`、`crates/hado/src/lib.rs` の `231` と `227-229` の 4 回追随した。

## 原因

- `scripts/lint-docs.sh:420` の `fence_open` が行頭のバッククォートだけを開きとして認めるので、Markdown が許す 3 スペースまでの字下げを開きとして読まない。
- `scripts/lint-docs.sh:429` の `transcript` は規約（`$ ` の行・`> ` の行・プレースホルダ）だけを見る。heredoc の本文は出力に数える（約束の範囲で、シェルは解析しない）。
- `scripts/lint-docs.sh:327` の `anchor` が、URI の `scheme://host/path:line` を行頭からの相対パスとして拾い、範囲の区切りを `-` だけに限る。行番号そのものが安定な識別子でない。
- `scripts/lint-docs.sh:560` の `outside_fences` と `:472` の `complete_table` が、行頭の表だけを実測として読む（引用の中の表を読まない）。

## 案

- 案 A（推奨）: 実測を生成物にする。`scripts/record-round.sh <slug>` が走らせたコマンドと出力を実測ブロックに書き、門番は `--check` でバイト一致を見る（`scripts/corpus-lock.sh` と同じ形）。手書きの実測が無くなり、プレースホルダや綴りの穴がまとめて消える。
- 案 B: 参照を安定 ID にする。`ファイル:行` をやめ、節番号（`§47`）・エラーコード（`E602`）・シンボル名を書き、門番が実在を解決する。行番号の追随が要らなくなる。
- 案 C: 前方領域をスキーマにする。`# round:` `優先度:` `# tier:` を TOML にして、検査 10 の正規表現をスキーマ検証に置き換える。
- 案 D: 字下げしたフェンス・引用の中の表・URI・en ダッシュだけを規約で決める（`docs/issues.md` に「フェンスは行頭、表は引用の外、範囲は `-`、原因に URI を書かない」と明記する）。門番は触らない。

推奨は A、次に B、次に C。D は A〜C のどれかを採るまでの書き手向けの規約である。

## 完了条件

- 案 A を採る場合: 実測ブロックが `scripts/record-round.sh` の生成物になり、`--check` がバイト一致を見る。手で書き換えた実測ブロック・出力の無いフェンス・プレースホルダだけの出力の 3 種が赤になる探針で測る。
- 案 B を採る場合: `docs/issues/*.md` の原因が `file:line` を持たず、安定 ID だけを指す。存在しない節番号・エラーコード・シンボルが赤になり、`crates/hado/src/mono.rs:4407` のような行番号も赤になる探針で測る。
- 案 C を採る場合: issue とラウンドの前方領域が TOML になり、必須項目の欠落と未知の優先度が赤になる探針で測る。検査 10 の正規表現は消える。
- 検査 10 が、必須の節それぞれに空白でない本文があることを見る（見出しだけで通る形の探針で測る）。
- `bash scripts/lint-docs.sh` が `PASSED: 10/10`、`git diff --check` が 0、`cargo test --offline` が 0 failed、`bash scripts/run-gates.sh --tier full --jobs 4` が red なし。

## 由来

`docs/wants.md` の番号は無い。PR #5 の四十次〜四十二次のレビュー指摘と、`docs/rounds/issues.md` の三十五〜四十二次の実測から。門番基盤（`docs/plan.md` 35 節）。検査 9 のシェル文法の解析は `lint-docs-evidence` で消した（`docs/rounds/lint-docs-evidence.md`）。
