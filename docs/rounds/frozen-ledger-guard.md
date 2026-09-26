# 凍結した台帳の書き換えを門番で止める（frozen-ledger-guard）

着手時の main は `14be875`。決定 D4（2026-09-27）は案 A である（`docs/plans/issue-backlog.md`）。

## 手順書

### 背景

`docs/ledger.md` は R81 までの実行記録で、凍結されている（`AGENTS.md`）。書き換えても文書層は緑のままである。`scripts/lint-docs.sh` の綴りの検査（検査 15・16）は `docs/spec.md` だけを読み、`scripts/verify-m2.sh` の `protected.sha` は走行中の変更しか見ない。凍結は規約だけで、コミットされた書き換えを止める門番が無い。

`docs/plan.md` も同じ扱いにする。計画は `docs/plans/<名前>.md` に置く規約（`AGENTS.md`）に合わせ、36 節の状態表は `docs/plans/spec-model.md` へ移す。

### 決定

- 決定 1: 凍結した文書のハッシュを `docs/frozen-docs.sha256` に記録し、`scripts/frozen-docs-lock.sh --check` がバイト一致を見る（`scripts/corpus-lock.sh` と同じ形）。対象は `docs/ledger.md` と `docs/plan.md` の丸ごとで、節を切り出すパーサを持たない（決定 D4）。
- 決定 2: 門番は `scripts/verify-frozen-docs.sh` の 1 本にする。`# tier: core` と `# round: frozen-ledger-guard` を宣言する。理由: 文書 2 本と lock を読むだけで、PR を回す層に要る検出力だから（`scripts/ci-run.sh` は `pull_request` を core で回す。決定 D3 が `scripts/verify-spec-contracts.sh` を core にしたのと同じ）。
- 決定 3: `docs/plan.md` 36 節の状態表は `docs/plans/spec-model.md` へ移し、36 節には現在の置き場を指す行を残す。以後 `docs/plan.md` は凍結した記録で、版ごとの決定は `docs/plans/<名前>.md` が持つ。

### 期待値の根拠

- lock は `<sha256>  <path>` の行を `LC_ALL=C sort` したもの。引数なしで作り直し（最後に行数を出す）、`--check` は `diff -u` を出して非 0（`scripts/corpus-lock.sh` と同じ）。
- 赤は探針で測る。台帳の `Unit` を `Void` に一括置換する探針と、凍結した節（35 節）に行を足す探針の 2 種。どちらも丸ごとのハッシュを変える。
- 層の所属は各門番の `# tier:` 行が唯一の正で、`scripts/gate-tiers.lock` はその生成物（決定 D3）。門番を足したら `bash scripts/gate-tiers-lock.sh` で作り直す。

### 成果物

- `scripts/frozen-docs-lock.sh`（新）と `docs/frozen-docs.sha256`（新、生成物）。
- `scripts/verify-frozen-docs.sh`（新、core、`# round: frozen-ledger-guard`）。
- `scripts/gate-tiers.lock` の作り直し。
- `docs/plans/spec-model.md`（新）と `docs/plan.md` 36 節の跡。
- `AGENTS.md`・`docs/rounds.md`・`docs/issues.md`・`CONTRIBUTING.md` の版ごとの決定の記述。
- `docs/rounds/frozen-ledger-guard.md`（本ファイル）。
- `docs/issues/frozen-ledger-guard.md` の削除。

### 工程

1. Red: 探針 2 種を現行の木で回し、`lint-docs` が緑のまま通ることを記録する。
2. 36 節の状態表を `docs/plans/spec-model.md` へ移し、`grep` で拾った `docs/plan.md` 参照を直す。
3. lock と門番を足す。
4. 層の生成物を作り直す。
5. 探針を再度回し、赤になることを記録する。
6. quick 層と `lint-docs`・`verify-spec-contracts` を回し、issue を消して PR にする。

### 判定

issue の「完了条件」をそのまま使う。

- 凍結した文書のハッシュが記録され、`--check` がバイト一致を見る。
- 台帳の綴りを一括置換した探針と、凍結した節に行を足した探針の 2 種が赤になる。
- `bash scripts/lint-docs.sh` が赤なし。`bash scripts/run-gates.sh --tier full` が red なし。

### 一周目の計画（意図・仮説・判定器・スコープ・未確認）

- 意図: hado の作者とラウンドの builder が、後の綴り統一や節整理の動機で凍結した記録を書き換える場面（推定）。差分の意図をレビューで見落としても、門番が止める。
- 仮説: `sha256sum` の列を corpus-lock と同じ形で記録し、`--check` を門番が呼べば、丸ごと 2 ファイルのバイト一致で 2 種の探針を止められる。節を切り出すパーサは要らない。
- 判定器: 上の「判定」の 3 行（issue の完了条件）。
- スコープ: 触るのは `scripts/` の新規 2 本・`scripts/gate-tiers.lock`・`docs/plan.md` 36 節・`docs/plans/spec-model.md`・`docs/rounds/frozen-ledger-guard.md`・`AGENTS.md`・`docs/rounds.md`・`docs/issues.md`・`CONTRIBUTING.md`・issue の削除。触らないのは `docs/ledger.md` の中身・`docs/spec.md`・`spec/`・`tests/`・`stdlib/`・`runtime/`・`crates/`、および 36 節以外の `docs/plan.md` の中身。
- 未確認: `docs/plan.md` を丸ごと凍結するので、36 節の移設を終えてからでないと lock を作れない（工程の順序が効く）。`docs/ledger.md` の「設計メモ」にある「採用した版ごとの決定は `docs/plan.md`」は凍結された本文で書き換えない。参照の書き換え先が `grep` で全部拾えるかは実装で確かめる。

### 許容される赤

なし。

### スコープ外

- `scripts/lint-docs.sh` への検査の追加（決定 D3 と同じく、文書の針は門番へ隔離する）。
- `docs/ledger.md` の中身（凍結。ハッシュの記録だけ）。
- `docs/spec.md`・`spec/`・`tests/`・`stdlib/`・`runtime/`・`crates/`。
- `docs/plan.md` の 36 節以外の節の中身。

### 中断の条件

- `docs/plan.md` と `docs/ledger.md` 以外に凍結が要る文書が出たとき（対象を広げずに報告する）。
- 36 節の移設で `lint-docs` のアンカー実在の検査（検査 13）が赤になったとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 凍結した文書（`docs/ledger.md`・`docs/plan.md`）は書き換えず、版ごとの決定は `docs/plans/<名前>.md` に置く | 工程 | 成功 | `scripts/verify-frozen-docs.sh`・`scripts/frozen-docs-lock.sh --check` |

## 実測

### Red（門番を足す前。現行の木は探針を緑のまま通す）

台帳の `Unit` を `Void` に一括置換しても `lint-docs` は緑である。

```
$ grep -c '\bUnit\b' docs/ledger.md
14
$ sed -i 's/Unit/Void/g' docs/ledger.md
$ grep -c '\bUnit\b' docs/ledger.md
0
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ git checkout -- docs/ledger.md
```

凍結した節（`docs/plan.md` の 35 節）に行を足しても緑である。

```
$ sed -i '/^## 35\. /a 探針で凍結した節に足した行。' docs/plan.md
$ sed -n '/^## 35\. /,+1p' docs/plan.md
## 35. v0.5 の範囲と決定（2026-09-17）
探針で凍結した節に足した行。
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ git checkout -- docs/plan.md
```

### Green（lock と門番を足した後）

`docs/frozen-docs.sha256` が `docs/ledger.md` と `docs/plan.md` のハッシュを記録し、`--check` がバイト一致を出す。

```
$ bash scripts/frozen-docs-lock.sh
2
$ cat docs/frozen-docs.sha256
1b04b12ee6bf567c3f403fcd75618fda603be54170313969d7df5a412cfa322f  docs/ledger.md
93eb3505fca705d455347d8da483655e8357c359c683af305d3c0413b15f9b2b  docs/plan.md
$ bash scripts/frozen-docs-lock.sh --check; echo "exit=$?"
exit=0
$ bash scripts/verify-frozen-docs.sh
OK: the frozen documents match the recorded hashes
PASSED: 1/1
```

### 探針（門番を足した後は赤になる）

台帳の `Unit` を `Void` に一括置換すると、台帳のハッシュが変わるので赤になる。

```
$ sed -i 's/Unit/Void/g' docs/ledger.md
$ bash scripts/verify-frozen-docs.sh 2>&1 | tail -5
@@ -1,2 +1,2 @@
-1b04b12ee6bf567c3f403fcd75618fda603be54170313969d7df5a412cfa322f  docs/ledger.md
+7e4836ade9334a8fd198c35fe1ebcf4151242addbb6ead1df0e120a3d900c6d0  docs/ledger.md
 93eb3505fca705d455347d8da483655e8357c359c683af305d3c0413b15f9b2b  docs/plan.md
NG: the frozen documents match the recorded hashes
PASSED: 0/1
$ git checkout -- docs/ledger.md
```

凍結した節（`docs/plan.md` の 35 節）に行を足すと、計画のハッシュが変わるので赤になる。

```
$ sed -i '/^## 35\. /a 探針で凍結した節に足した行。' docs/plan.md
$ bash scripts/verify-frozen-docs.sh 2>&1 | tail -5
@@ -1,2 +1,2 @@
 1b04b12ee6bf567c3f403fcd75618fda603be54170313969d7df5a412cfa322f  docs/ledger.md
-93eb3505fca705d455347d8da483655e8357c359c683af305d3c0413b15f9b2b  docs/plan.md
+0a84698ad617221ca29d5da5cb8573b9df18b6e854946e5f644b6635e77c3350  docs/plan.md
NG: the frozen documents match the recorded hashes
PASSED: 0/1
$ git checkout -- docs/plan.md
$ git status --short
```

### 層の生成物（門番を足すと赤になり、作り直すと緑になる）

```
$ bash scripts/gate-tiers-lock.sh --check 2>&1 | tail -3
 embed-runtime	full
+frozen-docs	core
 gate-cache	quick
$ echo "lock-check exit=${PIPESTATUS[0]}"
lock-check exit=1
$ bash scripts/verify-spec-contracts.sh 2>&1 | tail -1
PASSED: 1/2
$ bash scripts/gate-tiers-lock.sh
69
$ bash scripts/verify-spec-contracts.sh 2>&1 | tail -1
PASSED: 2/2
```

### 門番と層

```
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 41/41 passed; red: none
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh 2>&1 | tail -1
PASSED: 2/2
$ bash scripts/verify-parallel-rounds.sh 2>&1 | tail -1
PASSED: 5/5
```

`--tier full` の全走は着地側で回す（`AGENTS.md`）。本ラウンドは Rust を触らず、足した門番は core なので、着地の条件に要る quick 層・層の照合・文書層を手元で回した。着地後の全走は main を checkout した側が本節に追記する。

### 実装から分かったこと

仮説は「`sha256sum` の列を corpus-lock と同じ形で記録し、`--check` を門番が呼べば、丸ごと 2 ファイルのバイト一致で 2 種の探針を止められる」で、当たった。節を切り出すパーサは要らず、`diff -u` の出力がそのまま探針の理由（どのハッシュが変わったか）になった。`# tier: core` の判断も、`scripts/ci-run.sh` が `pull_request` を core で回すことと一致した。

外れたのは工程の順序の見立てである。`docs/plan.md` を丸ごと凍結するので、36 節の移設を終えた後にしか lock を作れない。先に lock を作ると移設そのものが赤になる。工程を「移設 → lock と門番 → 層の生成物」の順に組み替え、探針の赤は移設の後に測った。Red の実測は移設の前後で同じ結果（`lint-docs` が緑のまま）だった。

決められなかった点は 3 つある。

1. 凍結する対象の一覧は `scripts/frozen-docs-lock.sh` の `frozen_docs` が唯一の正で、対象を消す変更は門番では止まらない。`scripts/verify-m3.sh` の corpus lock は期待値をディスクから導いて同じ穴を塞いでいるが、どの文書が凍結かはディスクから導けない。機構の変更は差分としてレビューで見る。
2. `docs/ledger.md` 末尾の「設計メモ」は凍結で追記できなくなったので、仕様変更に至らなかった案の置き場を `docs/rounds.md` で `docs/plans/<名前>.md` へ寄せた。決定 D4 の「丸ごと」から出る帰結で、issue に明示は無い。
3. `docs/ledger.md` の「設計メモ」自身が「採用した版ごとの決定は `docs/plan.md`」と書くが、凍結された本文なので直さない。読み手には `AGENTS.md` と `docs/rounds.md` の新しい記述が優先する。
