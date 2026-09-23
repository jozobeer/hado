# 凍結した台帳の書き換えを門番で止める（frozen-ledger-guard）
優先度: 中。

## 現象

`docs/ledger.md` は R81 までの実行記録で、凍結されている（`AGENTS.md`）。書き換えても文書層は緑のままである。台帳の `Unit` を `Void` に置き換えて測った（R81 は型名を `Unit` から `Void` に改名したラウンドなので、台帳の `Unit` は当時の綴りとして残っている）。

```
$ grep -c '\bUnit\b' docs/ledger.md
14
$ sed -i 's/Unit/Void/g' docs/ledger.md
$ grep -c '\bUnit\b' docs/ledger.md
0
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
```

同じ書き換えを `docs/spec.md` に対して行うと検査 15 が赤になる。台帳だけが対象外である。

## 原因

- `scripts/lint-docs.sh` の `doc_type_spelling` は `docs/spec.md` だけを読み、`docs/ledger.md` を読まない。検査 15 は仕様本文の綴りを固定するが、凍結された記録の綴りは見ない。
- `scripts/lint-docs.sh` の `doc_effect_keywords` も同じく仕様本文だけを読む。文書層は「現行の文書」の整合を持ち、凍結された文書の不変性を持つ検査が無い。
- `scripts/verify-m2.sh` の `protected.sha` は走行中の変更を見るだけなので、コミットされた書き換えは見ない。凍結は `AGENTS.md` の規約で、門番が無い。

## 案

- 案 A（推奨）: 凍結した文書のハッシュを記録し、`--check` でバイト一致を見る。`scripts/corpus-lock.sh` と同じ形で、対象は `docs/ledger.md` と `docs/plan.md` の凍結された節にする。書き換えの意図（綴りの一括置換・節の追加）を機械で止められる。
- 案 B: `docs/ledger.md` に限り、`Unit` の実在を行番号なしで固定する（検査 15 の対象を台帳へ広げる）。小さいが、`Unit` 以外の書き換えは止まらない。
- 案 C: `AGENTS.md` の規約のままにする。凍結はレビューで守る。

推奨は A。B は A を採るまでの間の小さな補修である。C は、台帳が長く参照され続けるあいだは書き換えの動機（綴りの統一・節番号の整理）が繰り返し生じるので採らない。

## 完了条件

- 案 A を採る場合: 凍結した文書のハッシュが記録され、`--check` がバイト一致を見る。台帳の綴りを一括置換した探針と、凍結された節へ行を足した探針の 2 種が赤になる。
- 案 B を採る場合: 台帳の `Unit` を `Void` に置き換える探針が赤になる。
- `bash scripts/lint-docs.sh` が赤なし、`bash scripts/run-gates.sh --tier full` が red なし。

## 由来

`docs/wants.md` の番号は無い。PR #31 の五次審査の指摘（`docs/rounds/prune-gates.md` の「審査の指摘と対応」）から。門番基盤（`docs/plan.md` 35 節）。
