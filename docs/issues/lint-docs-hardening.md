# 検査 8〜10 の綴りの残りを規約として詰める（lint-docs-hardening）
優先度: 中。

## 現象

`scripts/lint-docs.sh` の検査 8（原因の `file:line`）と検査 9（現象の実測）は、シェルと Markdown の綴りを小さな規約で近似している。規約の外の綴りでは、正しい綴りが赤になる側と、実測でないフェンスが通る側の両方が残る。PR #5 の四十次〜四十二次のレビューで挙がった形を、`doc_issue_causes` と `doc_issue_evidence` の python ブロックを `exec` して呼んだ値が次の表である（`docs/rounds/issues.md` の四十次〜四十二次の実測と同じ手順）。

| 綴り | 現行の検査の値 |
| --- | --- |
| 3 スペース字下げしたフェンス（Markdown は開きとして認める） | `complete_fence=False` |
| 引用の中の表（`> \| a \| b \|`） | `complete_table=False` |
| 原因の `file:///work/app/x.hdi:1` | `causes=[('work/app/x.hdi', '1', '')]`（実在しない相対パスとして赤になる） |
| 原因の `crates/hado/src/mono.rs:4407–4532`（en ダッシュの範囲） | `causes=[('crates/hado/src/mono.rs', '4407', '')]`（範囲として読まない） |
| CRLF の終端フェンス | `complete_fence=True` |

`> ` で始まる行を PS2 の入力として無条件に読み飛ばす規約（`scripts/lint-docs.sh:457`）は、`\` による継続を見ていないので、コマンドの終わりと heredoc の本文の始まりを区別しない。この区別を求める綴りは無限に作れるので、PR #5 では既知の形（PS2 行に並ぶ複数の heredoc、情報文字列のバッククォート、`<<-`、here-string）だけを探針で固定し、残りはここで扱う。

## 原因

- `scripts/lint-docs.sh:420` の `fence_open` が行頭のバッククォートだけを開きとして認めるので、Markdown が許す 3 スペースまでの字下げを開きとして読まない。
- `scripts/lint-docs.sh:457` の PS2 の分岐が、`commands` が立っていれば `> ` の行を無条件に飛ばすので、コマンドの継続（直前の行が `\` で終わるか）と heredoc の本文の始まりを区別しない。
- `scripts/lint-docs.sh:327` の `anchor` が、URI の `scheme://host/path:line` を行頭からの相対パスとして拾い、範囲の区切りを `-` だけに限る。
- `scripts/lint-docs.sh:679` の `outside_fences` と `:499` の `complete_table` が、行頭の表だけを実測として読む（引用の中の表を読まない）。

## 案

- 案 A（推奨）: 4 つの綴りを規約の側で受け入れる。開きのフェンスは 3 スペースまでの字下げを許し、PS2 は直前の行の `\` で継続を判定し、原因のアンカーは URI を除外して en ダッシュを範囲として読み、表は引用の中も読む。探針を `scripts/lint-docs.sh` の自己検査に足す。
- 案 B: 規約の側を変えず、`docs/issues.md` に「フェンスは行頭、表は引用の外、範囲は `-`、原因には URI を書かない」と明記して、赤くなったら書き手が直す。門番は今のまま。
- 案 C: 検査 8〜10 をやめ、実測の有無の判定をレビューに戻す。門番が減るので取らない。

案 A を推す。いまの門番は実ファイルで `PASSED: 10/10` を保っており、残るのは綴りの受け入れ幅だけで、探針を足す場所は 3 か所に閉じている。案 B は書き手の負担を増やす割に、綴りの穴は残る。

## 完了条件

- `fence_open` が 0〜3 スペースの字下げを開きとして認め、終端も同じ幅まで認める。字下げしたフェンスと、4 スペース字下げした行（開きにしない）の探針で測る。
- PS2 の分岐が直前の行の `\` で継続を判定する。`$ cat \`・`> <<A \`・`> <<B` の順（本文が 2 つ）と、コマンドの後に置いた本文が `> ` で始まる形の探針で測る。
- `causes` が `scheme://` を含む綴りを除外し、`–`（en ダッシュ）の範囲を `-` と同じに読む。`file:///work/app/x.hdi:1` が `[]`、`crates/hado/src/mono.rs:4407–4532` が `[('crates/hado/src/mono.rs', '4407', '4532')]` になる探針で測る。
- 引用の中の表を実測として読む（または `docs/issues.md` に引用の外に限ると明記し、検査 9 のメッセージにも書く）。どちらを選んだかをラウンドファイルに書く。
- `bash scripts/lint-docs.sh` が `PASSED: 10/10`、`git diff --check` が 0、`cargo test --offline` が 0 failed、`bash scripts/run-gates.sh --tier full --jobs 4` が red なし。

## 由来

`docs/wants.md` の番号は無い。PR #5 の四十次〜四十二次のレビュー指摘と、`docs/rounds/issues.md` の三十五〜四十二次の実測から。門番基盤（`docs/plan.md` 35 節）。PR #5 はここで打ち切り、この issue を次の候補にする。
