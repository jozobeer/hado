# 仕様の機械可読部分を spec/ に出す計画（spec-model）

`docs/plan.md` 36 節（2026-09-23）の計画を移した。`docs/plan.md` は凍結した記録で、
以後の版ごとの決定と計画は `docs/plans/<名前>.md` が持つ。本ファイルは段ごとの状態と、
次に決めることを持つ。

仕様の規範内容のうち、機械が読む部分を `docs/spec.md` の Markdown から出す。正本は `spec/` に置き、`docs/spec.md` の表とフェンスは生成物にする（`scripts/spec-lock.sh`。`scripts/corpus-lock.sh` と同じ形）。

動機は 2 つある。同じ事実が 4 か所（`docs/spec.md` §56 の表・`crates/hado/src/*.rs` の `"Exxx"`・corpus の `// spec:`・`docs/spec-trace.md` の行）に写されており、正本が無いので「仕様は X と言っている」を文字列の実在（針）でしか表せない。`docs/plan.md` 33 節が 2026-09-14 に棚卸しを決めた文章固定検査がこれである。針は指摘の形が無限に作れるので、`scripts/lint-docs.sh` に検査が積み上がる（1000 行・16 検査・検査ごとに専用の正規表現）。

| 段 | 内容 | 状態 |
|---|---|---|
| 1 | エラーコード表と文法。`spec/errors.toml`・`spec/grammar.ebnf`・`spec/stages.toml` を正本にし、§56 の表と付録C のフェンスを生成する | 済み（`docs/rounds/spec-model.md`） |
| 2 | 追跡表。`docs/spec-trace.md`・`docs/spec-coverage.md` をデータにし、Markdown を生成する | 未着手 |
| 3 | `scripts/lint-docs.sh` を 1 つのパーサと宣言的な規則に割る。散文の針を隔離する | 未着手 |
| 4 | 実測を生成物にする（`docs/issues/lint-docs-hardening.md` の案 A） | 未着手 |

第 1 段で、`docs/spec.md` §56 の表の 5 行が壊れていたことが分かった（セルの `` `|` `` を逃がしていない。GFM はコードスパンの中でも `|` の escape を求める）。正本が無いので誰も気づいていなかった。H1（tree-sitter 文法）の「付録C からの生成」は、付録C が `spec/grammar.ebnf` の生成物になったので読み先が定まった。

第 2 段は、追跡表の消費者が人間だけでなく `scripts/lint-docs.sh`（検査 3・12・14）と `scripts/verify-m1-corpus.sh` であることを確かめてから決める。人間のための記録なら Markdown のまま残し、機械が読む部分だけをデータにする。
