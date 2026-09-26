# 削った門番が持っていた仕様本文の契約を戻す（dropped-spec-contracts）

着手時の main は `7e5a3dd`。決定 D3（2026-09-27）は「案 C を改めた形」である
（`docs/plans/issue-backlog.md`）。

## 手順書

### 背景

門番を削るラウンド（`docs/rounds/prune-gates.md`）は、各門番の `NN-x spec, trace, tier,
wants` の 4 つの対象をまとめて「既存の門番が引き受ける」と判断した。実測で振り分け直すと、
引き受け手があるのは trace（`scripts/lint-docs.sh` 検査 14）だけで、**仕様本文の針と門番
ごとの層の所属には引き受け手が無い**。仕様本文から規範文を消しても、門番の層を後ろへ
動かしても、緑のままである（下の「Red」）。

### 決定

- 決定 1: 仕様本文の針は `scripts/verify-spec-contracts.sh` 1 本に隔離する。
  `scripts/lint-docs.sh` には入れない（決定 D3。`docs/plan.md` 36 節の第 3 段「散文の針を
  隔離する」と同じ向き）。
- 決定 2: 層の所属は手で書く表にせず、`scripts/gate-tiers.lock` を
  `scripts/lib/gate-tiers.sh` の導出から作る生成物にし、`--check` がバイト一致を見る。
  正本は各門番の `# tier:` 行のままで、正本を 2 つにしない（issue の「案 C を改めた形」）。
- 決定 3: `scripts/verify-spec-contracts.sh` は `# tier: core`。理由: 文書と `scripts/` を
  短時間で読むだけで、PR を回す層に要る検出力だから（`scripts/ci-run.sh` は
  `pull_request` を `core` で回す）。`# round: dropped-spec-contracts` を宣言する。

### 期待値の根拠

- 針は「ある節に、ある規範文（の一部）が実在すること」を §N の一覧で表す。節の本文は
  `## N. ` の見出しから次の整数節の手前まで（小数節は同じ節に含める）、`§N.M` は次の
  `## ` の手前まで、`付録X` は次の `## ` の手前までとする。意味は解釈せず、文字列の実在
  だけを見る。
- 針の出所は `git diff f2a3c81..HEAD` の削除行で、仕様本文を読んでいた 25 本の門番
  （r45〜r60 のうち r49・r51・r61 を除く 14 本、r62〜r65、r68・r69、r72〜r76）が持って
  いたものである。r81（`Unit`/`Void`）は `scripts/lint-docs.sh` 検査 15、r44
  （`once`/`again`）は検査 16、r42（節の構造）は検査 1・2・5・11〜13 が既に引き受けて
  いるので含めない。
- 層の所属の生成物は `<門番名>\t<層>` の行を門番名で `LC_ALL=C sort` したもの。門番名は
  `--print-tier` と同じく `verify-` と `.sh` を落とした名にする。

### 成果物

- `scripts/verify-spec-contracts.sh`（新、core）。仕様本文の針と層の所属の照合。
- `scripts/gate-tiers-lock.sh`（新）と `scripts/gate-tiers.lock`（新、生成物）。
- `docs/rounds/dropped-spec-contracts.md`（本ファイル）。
- `docs/issues/dropped-spec-contracts.md` の削除。

### 工程

1. Red: 現行の木で 2 つの探針を回し、緑のまま通ることを記録する。
2. Green: 門番と生成物を足す。
3. 探針を再度回し、赤になることを記録する。
4. `docs/issues/dropped-spec-contracts.md` を消し、PR にする。

### 判定

issue の「完了条件」をそのまま使う。

- `scripts/verify-spec-contracts.sh` が落ちた 25 本の仕様本文の針を覆い、`# tier:` と
  `# round:` を宣言する。§53.1 の `hado: stack overflow` を消す探針と、§58 の
  `hado fmt` の行を消す探針が赤になる。
- 層の所属の生成物が記録され、`--check` がバイト一致を見る。`scripts/verify-m1-spec.sh`
  の `# tier: quick` を `full` に書き換える探針が赤になる。
- `bash scripts/lint-docs.sh` が赤なし、`bash scripts/run-gates.sh --tier full` が red
  なし。

### 許容される赤

なし。

### スコープ外

- `scripts/lint-docs.sh`・`scripts/lib/gate-tiers.sh`・`scripts/run-gates.sh` の変更
  （決定 D3。門番の一覧は gate-tiers.sh の走査のまま）。
- `docs/spec.md` の変更（仕様の疑義が出たら止めて報告する）。
- 25 本の門番の復活（案 B は削りの成果を戻すので採らない）。
- `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` の変更（`AGENTS.md`）。

### 中断の条件

- 針のうち実在しないものが出た場合は、その針を落とさずに、仕様が変わったのか節の読みが
  違うのかを報告する。
- `scripts/lint-docs.sh` を触る必要が出た場合は止めて報告する。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §15.1 | 型の食い違いの印字（`expected` `found` `cannot unify`、型変数は現れる順） | 型 | 失敗 | `scripts/verify-spec-contracts.sh` |
| §24・§29 | 正準形への書き換え（`書き換え`・`に書き換える`・`canonical form`） | 工程 | 成功 | `scripts/verify-spec-contracts.sh` |
| §30 | 腕の本体に置ける分岐は条件列だけ | 構文 | 失敗 | `scripts/verify-spec-contracts.sh` |
| §37 | 効果の操作を参照するか | 型 | 失敗 | `scripts/verify-spec-contracts.sh` |
| §50・§51 | `.` で始まる名前とディレクトリの除外 | 構文 | 失敗 | `scripts/verify-spec-contracts.sh` |
| §53.1 | スタックの枯渇の診断文 `hado: stack overflow` | 実行 | 失敗 | `scripts/verify-spec-contracts.sh` |
| §54 | 終了コードは 0 と 1 の二値 | 実行 | 成功 | `scripts/verify-spec-contracts.sh` |
| §56 | エラー表のアンカー（`first declared`・`declared in the contract`・`handler`・`型が食い違った式の先頭`） | 型 | 失敗 | `scripts/verify-spec-contracts.sh` |
| §57 | 正準形の修正（`--fix`・`E501`・`E507`・`E508`・`E510`・`E515`〜`E517`） | 工程 | 成功 | `scripts/verify-spec-contracts.sh` |
| §58・§58.1〜§58.4 | コマンド一覧と `fmt`・`test`・`--print-type`・`json` の各定義 | 工程 | 成功 | `scripts/verify-spec-contracts.sh` |
| 付録C | パターン分岐の文法規則 | 構文 | 成功 | `scripts/verify-spec-contracts.sh` |
| （門番運用） | 層の所属は各門番の `# tier:` が唯一の正で、導出を `scripts/gate-tiers.lock` が写す | 工程 | 成功 | `scripts/gate-tiers-lock.sh --check`・`scripts/verify-spec-contracts.sh` |

## 実測

### Red（門番を足す前。現行の木は探針を緑のまま通す）

§53.1 の `hado: stack overflow` を消しても `lint-docs` は緑のままである。

```
$ sed -i 's/hado: stack overflow/hado: stack overflowX/' docs/spec.md
$ grep -n 'stack overflow' docs/spec.md | head -1
1831:実行中のプロセスのスタックが尽きたときは、`hado: stack overflowX` の 1 行を stderr に出力し、終了コード 1 で終了する。...
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
```

`scripts/verify-m1-spec.sh` の層を `quick` から `full` へ動かしても、`core` から消える
だけで `verify-gate-cache.sh` は緑のままである。`spec-contracts` の門番はまだ無い。

```
$ bash scripts/run-gates.sh --print-tier core | tr ' ' '\n' | grep -c '^m1-spec$'
1
$ bash scripts/run-gates.sh --print-tier full | tr ' ' '\n' | grep -c '^spec-contracts$'
0
$ sed -i 's/^# tier: quick$/# tier: full/' scripts/verify-m1-spec.sh
$ head -2 scripts/verify-m1-spec.sh | tail -1
# tier: full
$ bash scripts/run-gates.sh --print-tier core | tr ' ' '\n' | grep -c '^m1-spec$'
0
$ bash scripts/run-gates.sh --print-tier full | tr ' ' '\n' | grep -c '^m1-spec$'
1
$ bash scripts/verify-gate-cache.sh 2>&1 | tail -1
PASSED: 26/26
```

```
$ git checkout -- docs/spec.md scripts/verify-m1-spec.sh
$ git status --short
（空）
```

### Green（門番と生成物を足した後）

```
$ bash scripts/verify-spec-contracts.sh
spec needles: 69 in §15.1, §24, §26.1, §29, §37, §50, §51, §53.1, §56, §57, §58, §58.1, §58.2, §58.3, §58.4, 付録C and the whole spec, 5 table rows, 1 once, 1 absent
OK: the spec body keeps the needles of the dropped gates
OK: the gate tier lock matches the membership derived from # tier:
PASSED: 2/2
$ bash scripts/gate-tiers-lock.sh
67
```

### 探針（門番を足した後は赤になる）

`hado: stack overflow` の後ろに `X` を足すと、メッセージが変わるので赤になる（針の右端を
語の続きでないことまで見る）。

```
$ sed -i 's/hado: stack overflow/hado: stack overflowX/' docs/spec.md
$ bash scripts/verify-spec-contracts.sh 2>&1 | tail -3
AssertionError: ['r50 §53.1: hado: stack overflow']
NG: the spec body keeps the needles of the dropped gates
OK: the gate tier lock matches the membership derived from # tier:
PASSED: 1/2
$ git checkout -- docs/spec.md
```

§58 の `hado fmt` の行（§58 の一覧と §58.1 の本文）を消すと赤になる。

```
$ sed -i 's/hado fmt \[--check\] <file.hd>/hado fmtX/g' docs/spec.md
$ bash scripts/verify-spec-contracts.sh 2>&1 | tail -3
AssertionError: ['r52 §58: hado fmt [--check] <file.hd>', 'r52 §58.1: hado fmt']
NG: the spec body keeps the needles of the dropped gates
OK: the gate tier lock matches the membership derived from # tier:
PASSED: 1/2
$ git checkout -- docs/spec.md
```

`scripts/verify-m1-spec.sh` の層を `quick` から `full` へ動かすと、生成物と導出が食い違う。

```
$ sed -i 's/^# tier: quick$/# tier: full/' scripts/verify-m1-spec.sh
$ bash scripts/gate-tiers-lock.sh --check 2>&1 | tail -5
 m1-corpus	core
-m1-spec	quick
+m1-spec	full
 m2	core
 m3	core
lock exit=1
$ bash scripts/verify-spec-contracts.sh 2>&1 | tail -2
NG: the gate tier lock matches the membership derived from # tier:
PASSED: 1/2
$ git checkout -- scripts/verify-m1-spec.sh
```

生成物は門番を黙って落とさない。`# tier:` を欠く門番を置くと、作り直しも照合も落ちる。

```
$ printf '#!/usr/bin/env bash\nexit 0\n' > scripts/verify-probe.sh
$ bash scripts/gate-tiers-lock.sh --check; echo exit=$?
invalid tier header: verify-probe.sh
exit=1
$ bash scripts/gate-tiers-lock.sh; echo rebuild-exit=$?
invalid tier header: verify-probe.sh
rebuild-exit=1
$ rm scripts/verify-probe.sh
```

### 門番と層

```
$ bash scripts/verify-gate-cache.sh 2>&1 | tail -1
PASSED: 26/26
$ bash scripts/verify-parallel-rounds.sh 2>&1 | tail -1
PASSED: 5/5
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 40/40 passed; red: none
$ bash scripts/corpus-lock.sh --check; echo exit=$?
exit=0
$ cargo test --offline 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit=0
```

`--tier full` は着地前には回さない（`AGENTS.md`。着地後に main を checkout した側が回し、
本節に追記する）。新設した門番は `core` なので、単独の `PASSED: 2/2` と quick 層の
`TALLY: 40/40` が着地の条件である。

### 実装から分かったこと

仮説は「針は文字列が節に実在することだけを見れば足りる」だったが、外れた。issue の
「現象」の探針は `hado: stack overflow` の後ろに `X` を足すもので、部分文字列の実在だけを
見ると緑のままである（`hado: stack overflow` は `hado: stack overflowX` の部分文字列）。
針の右端が語の続きでないことまで見ることにして、はじめて探針が赤になった。左端は見ない
（針は規範文の一部で、`E501` のような綴りの前には他の字が続きうる）。

節の読みは期待通りだった。整数節は小数節を飲み込むので、r48・r52・r54・r55 の §58 の針は
§58.1〜§58.4 のどこにあっても取れる。付録C は `## 付録C ` から次の `## ` までで取れる。

生成物は期待通り「`# tier:` の導出の写し」になったが、作り方に穴があった。プロセス置換
（`< <(hado_tier_minimal_gates ...)`）では層の走査の失敗がループに伝わらず、`# tier:` を
欠く門番を生成物から黙って落として作り直してしまう。代入に変えて、走査が落ちたら作り直し
も照合も落ちるようにした（fail closed）。`verify-gate-cache.sh` の層分割検査も同じ欠落を
別に見ているが、生成物の側でも落ちるようにした。

25 本の数え方は `git diff f2a3c81..HEAD` の削除行と一致した（r45〜r60 のうち仕様本文を
読まない r49・r51 を除く 14 本、r62〜r65、r68・r69、r72〜r76 で合計 25 本）。



着地後（2026-09-27）。PR #46 は `f71986e` で入った。着地後の全走は手元で回さず、main の CI（full 層）を読む。

```
$ git log --oneline -1
f71986e Merge pull request #46 from jozobeer/dropped-spec-contracts
$ gh run list --branch main --limit 3
completed	success	Merge pull request #46 from jozobeer/dropped-spec-contracts	ci	main	push	36262364465	20m59s	2026-09-26T18:23:25Z
```
