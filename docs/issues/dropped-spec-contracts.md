# 削った門番が持っていた仕様本文の契約を戻す（dropped-spec-contracts）
優先度: 高。

## 現象

門番を削るラウンド（`docs/rounds/prune-gates.md`）で、各門番の `NN-x spec, trace, tier, wants` 系の検査を落とした。tier は `scripts/verify-gate-cache.sh` の `tier_list_case` が、trace の門番名とラウンドの記録は `scripts/lint-docs.sh` の `doc_gate_relationships` が引き受けるが、**仕様本文の綴りと `docs/wants.md` の参照は引き受け手が無い**。落ちた検査は 30 本である。

```
$ sed -i 's/hado: stack overflow/hado: stack overflowX/' docs/spec.md
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ bash scripts/verify-r50.sh 2>&1 | tail -1
PASSED: 4/4
```

`scripts/verify-r50.sh` の `r50_overflow` は実行時の振る舞いだけを見るので、仕様本文から規範文を消しても緑のままである。落ちた検査の一覧は次のとおり（`git diff f2a3c81..HEAD` の削除行から）。

| 門番 | 落ちた検査 | 落ちた対象 |
| --- | --- | --- |
| r45 | 45-e | §30 の文、付録C の規則、E125 の行、§26.1 |
| r46 | 46-f | §57、wants の参照、full 層 |
| r47 | 47-e | §57、wants、full 層 |
| r48 | 48-e | §58、wants、full 層 |
| r49 | 49-b、49-d | `docs/bug.md` の節、full 層 |
| r50 | 50-d | §53.1、trace、full 層 |
| r51 | 51-d | full 層、配線、wants |
| r52 | 52-f | §58・§58.1、trace、full 層、wants |
| r53 | 53-e | §50・§51 の除外、trace、full 層、wants |
| r54〜r58 | 54-f〜58-i | 仕様の節、trace、full 層、wants |
| r59 | 59-i | 仕様の節、trace、full 層、`docs/bug.md` |
| r60〜r65 | 60-i〜65-f | 仕様の節、trace、full 層 |
| r68〜r76 | 68-e〜76-e | 仕様の節（E512 と「書き換え」など）、trace、full 層、corpus lock |

`scripts/lint-docs.sh` の `doc_type_spelling` と `doc_effect_keywords` は、このうち §5・§19・§54 の型名と効果操作の綴りだけを戻したもので、残りは引き受け手が無い。

## 原因

- 門番を削るときに「trace・tier・wants は既存の門番が引き受ける」と判断し、仕様本文の綴りを同じ扱いにした。tier と trace には引き受け手があるが、仕様本文には無い。
- `scripts/verify-r50.sh` の `r50_overflow` のような実行時の検査は、仕様本文を読まない。規範文を消しても振る舞いは変わらないので緑のままである。
- `scripts/lint-docs.sh` の `doc_type_spelling` は §5・§19・§54 の綴りだけを名指ししており、他の節の規範文を対象にしていない。
- `docs/wants.md` の参照を検査する門番が無い。R46〜R58 の wants の参照は、落ちた検査だけが持っていた。

## 案

- 案 A（推奨）: 落ちた 30 本のうち仕様本文と wants の部分を、`scripts/lint-docs.sh` の 1 つの検査にまとめて移す。門番ごとに「節の見出しの実在」と「その節にある規範文の針」を表で持ち、`§N` の一覧は `doc_spec_refs` と同じ形で照合する。tier と trace は既存の検査に任せ、重複させない。
- 案 B: 門番ごとの `NN-x spec` 検査を元に戻す。30 本の検査が戻り、門番を削る前の状態に近づく。削りの効果（検査数 −68）は減る。
- 案 C: 落ちた検査のうち、規範文の針だけを 1 本の門番（`verify-spec-contracts.sh`）に集める。`lint-docs.sh` は現行の文書の整合に留める。

推奨は A。文書の整合は `scripts/lint-docs.sh` が持つ規約（`AGENTS.md`）に合い、門番の本数を増やさない。C は門番が 1 本増えるが、仕様本文の契約を文書層から分けたい場合に選ぶ。B は削りの成果を戻すので、削りをやり直す場合に限る。

## 完了条件

- 案 A を採る場合: `scripts/lint-docs.sh` の新しい検査が、落ちた 30 本の仕様本文と wants の針を覆う。仕様から `hado: stack overflow` を消す探針、`docs/wants.md` から `R52` を消す探針、§58 の `hado fmt` の行を消す探針の 3 種が赤になる。
- 案 B を採る場合: 落ちた 30 本の検査が戻り、それぞれの元の探針が赤になる。
- 案 C を採る場合: 新しい門番 1 本が仕様本文の針を覆い、`# tier: full` を宣言して `scripts/lib/gate-tiers.sh` の層に載る。
- `bash scripts/lint-docs.sh` が赤なし、`bash scripts/run-gates.sh --tier full` が red なし。

## 由来

`docs/wants.md` の番号は無い。PR #31 の六次審査の指摘（`docs/rounds/prune-gates.md` の「審査の指摘と対応」）から。門番基盤（`docs/plan.md` 35 節）。
