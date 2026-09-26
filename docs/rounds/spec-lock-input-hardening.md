# spec/ の入力の検証を詰める（spec-lock-input-hardening）

着手時の main は `f71986e`。計画は `docs/plans/issue-backlog.md` の G レーン 2 番目で、
issue の推奨 (a)（3 件とも入力の検証で弾く）を採る。`docs/rounds/spec-model.md` の 6 回目の
審査で打ち切った 3 件を、このラウンドで回収する。

## 手順書

### 一周目の計画

| 要素 | 内容 |
|---|---|
| 意図 | 仕様の正本を保守する人が `spec/` を編集するとき、表やフェンスを壊す値を入力の時点で知らせてほしい。壊れた `docs/spec.md` が GFM として読めなくなると、後続のラウンドが仕様を読めない（推定） |
| 仮説 | 3 件は独立な入力経路の穴である。字下げフェンスは `grammar_groups`、表のセルの `|` は `error_table` の `field`、使われない族は `model_case` の族の集合の比較で、それぞれの持ち主の関数が弾ける。`|` は「逃がす」より「拒む」ほうが完了条件（変異が赤）を満たす（推定） |
| 判定器 | issue の完了条件をそのまま使う。issue の 3 つの変異（字下げフェンス・段階の `|`・未使用の族 `E8`）のいずれでも `bash scripts/spec-lock.sh --check` か `bash scripts/verify-spec-model.sh` が赤（現在値は 3 件とも緑、目標値は赤）。正しい入力では `docs/spec.md` がバイト一致のまま |
| スコープ | 触る: `scripts/spec-lock.sh`・`scripts/verify-spec-model.sh`・本ファイル・`docs/issues/spec-lock-input-hardening.md` の削除。触らない: `docs/spec.md`・`spec/`・`scripts/lint-docs.sh`・`scripts/lib/gate-tiers.sh`・`scripts/run-gates.sh`・`scripts/gate-tiers.lock`・コーパス・`stdlib/` |
| 未確認 | `|` を逃がすだけにしたとき変異 2 が緑のままか（仮説の決め打ち）。`fence_like` の 3 文字の境界が正しいか。族の集合の比較で `spec/stages.toml` の重複・欠落の既存検査と二重にならないか |

### 背景

`spec/` の正本を特定の形に壊すと、`scripts/spec-lock.sh` が壊れた `docs/spec.md` を作った
うえで一致を報告する。`docs/spec.md` の表が壊れると GFM として読めなくなる。

- 字下げしたフェンス行は、Markdown が閉じフェンスとして読むのに、門番は通す。
- `|` は `anchor` だけが逃がされているので、段階に `|` を入れると表の列が増える。
- 段階の対応表は、使われない族を持っていても通る。

### 決定

issue の推奨 (a) を採る。3 件とも入力の検証で弾く。

- 決定 1: `grammar_groups` は本文の行を、先頭の空白を 3 文字まで無視してフェンス行と
  見なし、弾く（GFM の閉じフェンスは 3 文字まで下げられる）。
- 決定 2: `error_table` は `field` に `|` の扱いを集める。`anchor` は従来どおり `\|` に
  逃がし、`code`・`name`・`section`・`stage` は `|` を拒む。逃がすだけでは変異 2 が赤に
  ならない（下の Green で実測）。
- 決定 3: `model_case` は `spec/stages.toml` の族の集合が、`spec/errors.toml` のコードが
  使う族の集合と等しいことを見る。
- 決定 4: `scripts/verify-spec-model.sh` の `# round: spec-model` は変えない。既存の門番で
  `# round:` を後から書き換えた例が無く（`git log` で全門番を走査して 0 件）、`# round:` は
  その門番を新設したラウンドを指す。本ラウンドの実測は本ファイルに置く。`docs/rounds/spec-model.md`
  を書き換えないので、同ファイルの `# round: spec-model` の記述とも矛盾しない。
- 決定 5: 門番は増やさない。`scripts/gate-tiers.lock` は触らない。

### 期待値の根拠

- 変異は issue の「現象」のコマンドをそのまま再現する。当てて赤を測り、`git checkout` で戻す
  （変異は木に残さない。探針は門番の中に置く）。
- 正しい入力の出力は変えない。`bash scripts/spec-lock.sh --check` が一致を出し、`spec/` から
  作り直しても `docs/spec.md` が変わらないことを `git status --short` が空であることで見る。

### 成果物

- `scripts/spec-lock.sh`（`fence_like` と `field` の `|` の扱い）。
- `scripts/verify-spec-model.sh`（`model_case` の族の網羅）。
- `docs/rounds/spec-lock-input-hardening.md`（本ファイル）。
- `docs/issues/spec-lock-input-hardening.md` の削除。

### 工程

1. Red: 現行の木で 3 つの変異を回し、緑のまま通ることを記録する。
2. Green: `scripts/spec-lock.sh` に決定 1・2 を入れる。
3. `scripts/verify-spec-model.sh` に決定 3 を入れる。
4. 3 つの変異を再度回し、赤になることを記録する。
5. `docs/issues/spec-lock-input-hardening.md` を消し、PR にする。

### 判定

issue の完了条件をそのまま使う。

- 3 つの変異のいずれでも `bash scripts/spec-lock.sh --check` か
  `bash scripts/verify-spec-model.sh` が赤になる。
- `bash scripts/spec-lock.sh --check` が緑のまま、`spec/` から作り直しても `docs/spec.md` が
  変わらない。
- `bash scripts/run-gates.sh --tier quick` が red なし、`bash scripts/lint-docs.sh` と
  `bash scripts/verify-spec-contracts.sh` が赤なし、`cargo test --offline` が 0 failed。

### 許容される赤

なし。

### スコープ外

- `docs/spec.md`・`spec/` の変更（仕様の疑義が出たら止めて報告する）。
- `scripts/lint-docs.sh`・`scripts/lib/gate-tiers.sh`・`scripts/run-gates.sh` の変更。
- 門番の増設と `scripts/gate-tiers.lock` の作り直し。
- `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` の変更（`AGENTS.md`）。

### 中断の条件

- 正しい入力で `docs/spec.md` の生成結果が変わったとき。
- `|` を逃がすだけでは変異 2 が赤にならないと分かり、拒む以外の選択が要るとき。
- `scripts/lint-docs.sh` を触る必要が出たとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | `spec/` の正本は表・フェンスを壊す値を弾き、段階の対応表は誤りの族を過不足なく覆う | 道具 | 成功 | `scripts/spec-lock.sh`・`scripts/verify-spec-model.sh` |

## 実測

### Red（3 つの変異は緑のまま通る）

字下げしたフェンス行（3 文字）を字句群の本文に足すと、`spec-lock` は 0 で終わり、門番も
`PASSED: 2/2` のままである。

````
$ python3 -c 'from pathlib import Path; p=Path("spec/grammar.ebnf"); l=p.read_text().split("\n"); l.insert(l.index("### 字句:")+1, "   "+chr(96)*3); p.write_text("\n".join(l))'
$ sed -n '/### 字句:/,+2p' spec/grammar.ebnf
### 字句:
   ```
int             := digit+                              10 進。i64 の範囲（E112）
$ bash scripts/spec-lock.sh >/dev/null 2>&1; echo exit=$?
exit=0
$ bash scripts/verify-spec-model.sh 2>&1 | tail -1
PASSED: 2/2
````

段階を `名前|解決` にすると、`spec-lock --check` は一致を報告し、生成物の表の列が増える。
門番も緑のままである。

````
$ sed -i 's/stage   = "名前解決"/stage   = "名前|解決"/' spec/errors.toml
$ sed -i 's/stages = \["名前解決"\]/stages = ["名前|解決"]/' spec/stages.toml
$ bash scripts/spec-lock.sh >/dev/null 2>&1; bash scripts/spec-lock.sh --check; echo exit=$?
spec-lock --check: 一致（70 エラーコード、10 文法群）
exit=0
$ grep -n '名前|解決' docs/spec.md | head -1
2037:| E201 | implicit-recursion | §9 | 名前|解決 | 自分を参照する名前 |
$ bash scripts/verify-spec-model.sh 2>&1 | tail -1
PASSED: 2/2
````

使われない族 `E8` を `spec/stages.toml` に足しても、門番は緑のままである。

````
$ printf '\n[[stage]]\nfamily = "E8"\nstages = ["型"]\n' >> spec/stages.toml
$ bash scripts/verify-spec-model.sh 2>&1 | tail -2
OK: the model is well formed and its sections exist
PASSED: 2/2
````

```
$ git checkout -- spec/grammar.ebnf spec/errors.toml spec/stages.toml docs/spec.md
$ git status --short
（空）
```

### Green（正しい入力の出力は変えない）

`spec-lock --check` は一致し、`spec/` の正本から作り直しても `docs/spec.md` は変わらない。
門番は `PASSED: 2/2`。

```
$ bash scripts/spec-lock.sh --check
spec-lock --check: 一致（70 エラーコード、10 文法群）
$ bash scripts/spec-lock.sh; git status --short docs/spec.md
（空）
$ bash scripts/verify-spec-model.sh
spec-lock --check: 一致（70 エラーコード、10 文法群）
OK: docs/spec.md matches spec/ (spec-lock --check)
70 エラーコード、10 文法群、7 段階の族、見出し 69
OK: the model is well formed and its sections exist
PASSED: 2/2
```

### 変異（修正後は赤になる）

字下げしたフェンス行は `spec-lock` が弾き、`--check` も門番も赤になる。

````
$ python3 -c 'from pathlib import Path; p=Path("spec/grammar.ebnf"); l=p.read_text().split("\n"); l.insert(l.index("### 字句:")+1, "   "+chr(96)*3); p.write_text("\n".join(l))'
$ bash scripts/spec-lock.sh >/dev/null 2>&1; echo exit=$?
exit=1
$ bash scripts/spec-lock.sh --check 2>&1 | tail -1
spec-lock: spec/grammar.ebnf の 字句: の本文にフェンス行 '   ```'
$ bash scripts/verify-spec-model.sh 2>&1 | tail -1
PASSED: 1/2
````

段階の `|` は `spec-lock` が弾く（逃がさない）。

```
$ sed -i 's/stage   = "名前解決"/stage   = "名前|解決"/' spec/errors.toml
$ sed -i 's/stages = \["名前解決"\]/stages = ["名前|解決"]/' spec/stages.toml
$ bash scripts/spec-lock.sh 2>&1 | tail -1; echo "write exit=${PIPESTATUS[0]}"
spec-lock: E201 の stage に `|`（表の列が増える）
write exit=1
$ bash scripts/spec-lock.sh --check 2>&1 | tail -1; echo "check exit=${PIPESTATUS[0]}"
spec-lock: E201 の stage に `|`（表の列が増える）
check exit=1
$ bash scripts/verify-spec-model.sh 2>&1 | tail -1
PASSED: 1/2
```

使われない族 `E8` は門番が弾く。

```
$ printf '\n[[stage]]\nfamily = "E8"\nstages = ["型"]\n' >> spec/stages.toml
$ bash scripts/verify-spec-model.sh 2>&1 | tail -3
AssertionError: ["family-coverage=table['E1', 'E2', 'E3', 'E4', 'E5', 'E6', 'E7', 'E8'] vs codes['E1', 'E2', 'E3', 'E4', 'E5', 'E6', 'E7']"]
NG: the model is well formed and its sections exist
PASSED: 1/2
```

「逃がす」だけでは変異 2 は緑のままである（拒む側を採った根拠）。`field` を全セル
`escape_pipe=True` に一時的に変えて測ると、`--check` は一致し、門番も緑である。

```
$ sed -i "s/cells = \[field(e, k) for k in ('code', 'name', 'section', 'stage')\]/cells = [field(e, k, escape_pipe=True) for k in ('code', 'name', 'section', 'stage')]/" scripts/spec-lock.sh
$ sed -i 's/stage   = "名前解決"/stage   = "名前|解決"/' spec/errors.toml
$ sed -i 's/stages = \["名前解決"\]/stages = ["名前|解決"]/' spec/stages.toml
$ bash scripts/spec-lock.sh >/dev/null 2>&1; bash scripts/spec-lock.sh --check; echo exit=$?
spec-lock --check: 一致（70 エラーコード、10 文法群）
exit=0
$ grep -n '| E201 |' docs/spec.md
2037:| E201 | implicit-recursion | §9 | 名前\|解決 | 自分を参照する名前 |
$ bash scripts/verify-spec-model.sh 2>&1 | tail -1
PASSED: 2/2
```

```
$ git checkout -- scripts/spec-lock.sh spec/errors.toml spec/stages.toml docs/spec.md
$ git status --short
（空）
```

### 門番と層（ラウンドの最後）

`cargo test` の末尾は最後のテストバイナリの要約である（全体は exit 0）。full 層は着地後に
main を checkout した側が回す。

```
$ cargo test --offline 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
exit=0
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
TALLY: 40/40 passed; red: none
exit=0
$ bash scripts/lint-docs.sh 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
PASSED: 16/16
exit=0
$ bash scripts/verify-spec-contracts.sh 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
PASSED: 2/2
exit=0
```

### 実装から分かったこと

仮説「3 件は独立な入力経路の穴で、それぞれの持ち主の関数が弾ける」は当たった。
`grammar_groups` の本文の行、`error_table` の `field`、`model_case` の族の集合で、3 件とも
入力の時点で止まる。正しい入力の生成物はバイト一致のままである。

外れたのは `|` の扱いの決め打ちで、こちらは実測で「拒む」に定まった。逃がすだけの形
（全セル `escape_pipe=True`）では変異 2 が `名前\|解決` という正しい表になり、`--check` は
一致・門番も `PASSED: 2/2` で緑のままである。issue の (a) は「逃がし（または拒み）」と
両論を並べるが、完了条件を判定器にする以上、変異を赤にできる「拒む」が要る。表のセルごとに
`|` の扱いが違う（`anchor` だけが逃がす）ことを `field` の引数で表した。

字下げの境界は 3 文字に置いた。GFM の閉じフェンスは 3 文字まで下げられる。4 文字以上は
本文（インデントコード）として閉じないので弾かない。タブ下げは測っていないが、Markdown は
タブを 4 文字として扱うので安全側である。`spec/grammar.ebnf` にタブ下げのフェンスを
持ち込む動機が無いので、これ以上は詰めない。

族の集合の比較は、既存の「各コードの族が対応表にある」（`family-absent`）の逆方向を埋める。
両方で集合が等しくなり、重複は既存の `families` の重複検査が持つ。検査の二重化はしていない。

門番は増やしていない。3 件の探針は門番の中（`lock_case` が呼ぶ `spec-lock.sh` と
`model_case`）に置き、変異は一時的に当てて `git checkout` で戻した。`# round:` は既存の
門番で後から書き換えた例が無い（`git log` で全門番を走査して 0 件）ので `spec-model` の
ままにし、本ラウンドの実測は本ファイルに置いた。


着地後（2026-09-27）。PR #48 は `ddcbb6a` で入った。着地後の全走は手元で回さず、main の CI（full 層）を読む。

```
$ git log --oneline -1
ddcbb6a Merge pull request #48 from jozobeer/spec-lock-input-hardening
$ gh run list --branch main --limit 3
completed	success	Merge pull request #48 from jozobeer/spec-lock-input-hardening	ci	main	push	36267746758	14m31s	2026-09-26T19:55:58Z
```
