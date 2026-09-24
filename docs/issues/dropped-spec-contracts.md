# 削った門番が持っていた仕様本文の契約を戻す（dropped-spec-contracts）
優先度: 高。

## 現象

門番を削るラウンド（`docs/rounds/prune-gates.md`）で、各門番の `NN-x spec, trace, tier, wants` 系の検査を落とした。落ちた 4 つの対象のうち、**仕様本文の綴りと門番ごとの層の所属に引き受け手が無い**。仕様本文から規範文を消しても、層の所属を書き換えても、緑のままになる。

```
$ sed -i 's/hado: stack overflow/hado: stack overflowX/' docs/spec.md
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ bash scripts/verify-r50.sh 2>&1 | tail -1
PASSED: 4/4
```

層の所属は、層を**後ろへ**動かしたときだけ検出力が落ちる（`hado_tier_gates` は累積なので、`full` は `quick` と `core` を含む。`full` へ動かすと PR を回す `core` から消える）。`--print-tier` は `verify-` と `.sh` を落とした名前を出すので、門番のファイル名ではなくその名前で照合する。

```
$ bash scripts/run-gates.sh --print-tier core | tr ' ' '\n' | grep -c '^m1-spec$'
1
$ sed -i 's/^# tier: quick$/# tier: full/' scripts/verify-m1-spec.sh
$ bash scripts/run-gates.sh --print-tier core | tr ' ' '\n' | grep -c '^m1-spec$'
0
$ bash scripts/verify-gate-cache.sh 2>&1 | tail -1
PASSED: 26/26
```

落ちた検査のうち仕様本文の針を持っていた門番は 25 本である。針の例は §53.1 の `hado: stack overflow`、§58 の `hado fmt [--check] <file.hd>`、§57 の `局所的な書き換えを持たない正準形違反（E507 / E508）` が仕様に無いこと、§56 の `first declared` / `declared in the contract`、§24・§29 の `書き換え`、§58.4 の `json`、§53.1 の終了コードの二値などである（`git diff f2a3c81..HEAD` の削除行から）。

## 原因

- 門番を削るときに、`NN-x spec, trace, tier, wants` の 4 つをまとめて「既存の門番が引き受ける」と判断した。実際に引き受けるのは trace だけで、仕様本文と層の所属は引き受け手が無い。
- `scripts/verify-r50.sh` の `r50_overflow` のような実行時の検査は仕様本文を読まない。規範文を消しても振る舞いは変わらないので緑のままである。
- `scripts/lint-docs.sh` の `doc_type_spelling` と `doc_effect_keywords` は §5・§19・§54 の綴りだけを名指ししており、他の節の規範文を対象にしていない。
- `scripts/verify-gate-cache.sh` の `tier_list_case` は `# tier:` の字種と一意性を見るが、門番ごとの層の所属は見ない。`scripts/lib/gate-tiers.sh` が所属を導くので、門番の層が黙って変わっても赤にならない。

## 検証した対象（引き受け手があるもの、契約でないもの）

| 落ちた対象 | 判定 | 根拠 |
| --- | --- | --- |
| `docs/spec-trace.md` の門番名と § 行 | 引き受け手がある | `scripts/lint-docs.sh` の `doc_gate_relationships`。§58.1 の行を消すと `PASSED: 15/16` |
| `docs/wants.md` の参照 | 契約ではない | `docs/wants.md` は `# やりたいことメモ` で「以下は未決定」。`docs/issues.md` が「根拠と完了条件が揃った項目を `docs/issues/` へ移し、`docs/wants.md` からは消す」と定めるので、`'R52' in wants` は昇格のたびに赤になる。落としたままでよい |
| 門番ごとの層の所属 | 引き受け手が無い | 上の実測。`verify-gate-cache.sh` は字種と一意性だけを見る |

## 案

- 案 A（推奨）: 仕様本文の針を `scripts/lint-docs.sh` の 1 つの検査にまとめて移す。節の見出しの実在と、その節にある規範文の針を表で持ち、`§N` の一覧は `doc_spec_refs` と同じ形で照合する。層の所属は門番ごとに 1 行の表にして、`scripts/lib/gate-tiers.sh` の導出結果と照合する検査を同じ門番に置く。
- 案 B: 門番ごとの `NN-x spec` 検査を元に戻す。25 本の検査が戻り、門番を削る前の状態に近づく。削りの効果は減る。
- 案 C: 仕様本文の針だけを 1 本の門番（`verify-spec-contracts.sh`）に集める。`scripts/lint-docs.sh` は現行の文書の整合に留める。

推奨は A。文書の整合は `scripts/lint-docs.sh` が持つ規約（`AGENTS.md`）に合い、門番の本数を増やさない。C は門番が 1 本増えるが、仕様本文の契約を文書層から分けたい場合に選ぶ。B は削りの成果を戻すので、削りをやり直す場合に限る。

## 完了条件

- 案 A を採る場合: 新しい検査が、落ちた 25 本の仕様本文の針を覆う。§53.1 の `hado: stack overflow` を消す探針、§58 の `hado fmt` の行を消す探針、門番の `# tier:` を書き換える探針の 3 種が赤になる。
- 案 B を採る場合: 落ちた 25 本の検査が戻り、それぞれの元の探針が赤になる。
- 案 C を採る場合: 新しい門番 1 本が仕様本文の針を覆い、`# tier: full` を宣言して `scripts/lib/gate-tiers.sh` の層に載る。
- `bash scripts/lint-docs.sh` が赤なし、`bash scripts/run-gates.sh --tier full` が red なし。

## 由来

`docs/wants.md` の番号は無い。PR #31 の六次審査の指摘（`docs/rounds/prune-gates.md` の「審査の指摘と対応」）から。門番基盤（`docs/plan.md` 35 節）。
