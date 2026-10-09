# 入れ子の適用の往復を門番と検体で固定する（nested-application-round-trip）

## 手順書

### 背景

`fmt-layout`（F5、`350a575`）が `crates/hado/src/postfix_print.rs` の `apply_text` で、適用の呼び手が適用のときは括弧で囲むようにした。`(f a) b` を `f a b` と印字し直して 2 回の適用を 1 回の 2 引数呼び出しへ変える欠陥（`fmt-reprints-nested-application-as-one-call`）はこれで直っている。`fmt` の前後で `run` の出力は同じで、`print-postfix` の往復も意味を保つ。

しかし受理コーパス（`tests/accept/`）・`stdlib/`・`tools/` に `(f a) b` の形の検体が 1 つも無い。`verify-fmt-layout.sh` の検査 b は `print-postfix` の出力が元のファイルと 1 バイトも変わらないことを全ファイルで見るが、この形がコーパスに無ければこの欠陥には触れない。現象は消えているが、何も固定していないので、いつ壊れても気づけない。

`whole-program-reprint-breaks-fmt-gates-and-idempotence` の完了条件のうち、r52・r53・r57・r64 が緑・冪等・空ファイル不変は main で満たされている（`r52 5/5`・`r53 5/5`・`r57 6/6`・`r64 4/4`、`verify-fmt-layout.sh` 6/6）。足りないのは検体と、変異で赤になることの確認だけである。

このラウンドは実装を 1 行も変えない。門番と検体と記録だけで 2 つの issue の完了条件を満たす。

### 意図

（推定）hado を書く人と読む人が `fmt` と `print-postfix` を安全に使えるようにする。整形と印字の往復が `(f a) b` の意味を変えないことを、機械が測り続ける。

### 仮説

- 現象は `apply_text` の呼び手の括弧で既に直っている。だから門番は、この形の検体さえあれば、実装を変えずに緑になる。
- `(f a) b` の形は受理コーパスのソース上で `((f 1) 41)` のような括弧の並びとして現れる。受理コーパスは正準形（`fmt_layout.rs` の `corpus_stdlib_and_tools_are_canonical_and_idempotent` が固定）なので、印字の形をテキストで探せる。
- 呼び手の括弧を落とす変異（`matches!(f.kind, NodeKind::Apply(..))` を `false` にする）は、検体の `fmt` の前後の `run` を必ず変える。門番が変異を一時的に当てて自分で赤を確かめれば、検体が実効であることを門番自身が測れる。

### 判定器

issue の完了条件をそのまま使う。門番 `scripts/verify-nested-application-round-trip.sh`（`# tier: core`、`# round: nested-application-round-trip`）:

- 検体ごとに、`fmt` の前後で `run` の出力（終了コードと stdout/stderr）が同じ。
- 検体ごとに、`print-postfix` の出力を読み直した `run` が元のソースの `run` と同じ。
- 検体ごとに、`fmt` の直後の `fmt --check` が終了コード 0（冪等）。
- 検体ごとに、`run` の出力が検体の `// expect:` と一致する（検体自身の妥当性）。
- 呼び手の括弧を落とす変異で `fmt` の前後の `run` が変わること（門番が一時コピーへ当てて戻す）。

検体は `tests/accept/` の `(f a) b` の形をディスクから列挙して数える。件数は直書きしない。

### スコープ

触る: `scripts/verify-nested-application-round-trip.sh`（新設）、`tests/accept/nested_application_call*`（3 本の検体）、`tests/corpus.sha256`（作り直す）、`scripts/lib/gate-tiers.sh`（直列の一覧へ足す）、`scripts/gate-tiers.lock`（作り直す）、`docs/rounds/nested-application-round-trip.md`、`docs/issues/` の 2 本の削除。

触らない: `crates/hado/src/postfix_print.rs`（他のレーンが同時に触っている）、`crates/hado/src/repr.rs`・`named_patterns.rs`・`float_format.rs`・`infer.rs`（同上）。実装は 1 行も変えない。`docs/spec.md`・`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`・`docs/plans/issue-backlog.md`。

### 工程

1. 検体を足さずに門番を書き、赤（検体 0 本）を見る。
2. 検体を足して門番を緑にする。
3. 変異を手で当て、検体のある木で門番が赤になることを先に見る。戻して緑を確かめる。
4. `tests/corpus.sha256` を、`--check` の差分を読んでから作り直す。
5. ラウンドファイルを書き、issue 2 本を消す。
6. 判定と層別を回す。

### 許容される赤

- なし。`verify-r62`・`verify-r63`・`verify-r68` などの既知の赤は main でも赤く、このラウンドの成果ではない。

### スコープ外

- `postfix_print.rs` の実装。現象は既に直っており、このラウンドの門番は実装を変えずに緑になる。
- `stdlib`・`tools` への検体追加。issue は受理コーパスへと定めており、`tools/gates` は門番の道具で印字の対象ではない。
- トップレベルの curry した `def` を直接入れ子適用する形（`def f a => fn(b => a + b)` の `((f 1) 41)`）は main で `run` が `Mismatch` で落ちる。印字の欠陥ではなく単相化の欠陥で、このラウンドの検体はこの形を避ける。報告に書く。

### 中断の条件

- 実装を変えないと門番が緑にならないと分かった。
- 変異を当てても門番が赤にならない（検体が実効でない）と分かった。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §13 | 関数の呼び出しは並置。定義の引数数と呼び出しの引数数は一致（部分適用は許可しない） | 型 | E301 | `tests/accept/nested_application_call*`・`scripts/verify-nested-application-round-trip.sh` |
| §57 | 局所的な書き換えは意味を変えない | 冗長 | — | `scripts/verify-nested-application-round-trip.sh` の検査 a |
| §58.1 | 印字し直しは意味を変えない。書き換えが尽きたソースはもう変わらない | 冗長 | — | `scripts/verify-nested-application-round-trip.sh` の検査 a・c |

## 実測

main は `5b15cc5`。作業は `.worktrees/nested-application-round-trip` の `nested-application-round-trip` で行った。

### 検体が無いこと（main）

`(f a) b` の形は受理コーパス・`stdlib`・`tools` に無い。門番の検出器（テキストの形）も 0 本を返す。

```
$ grep -rnE '\(\([a-zA-Z_]\w* [^)]*\) [^)]*\)' tests/accept stdlib tools; echo "exit=$?"
exit=1
```

### 赤を先に見た記録

検体を足す前の門番は、検体 0 本で赤い。変異の検査は測る対象が無いので緑のままになる。

```
$ bash scripts/verify-nested-application-round-trip.sh
NG: 検体: tests/accept に (f a) b の形がある (0 本)
NG: e: 呼び手の括弧を落とす変異で a が赤になる (変異でも緑のまま)
PASSED: 0/2
gate exit=1
```

検体を足した後、変異（`apply_text` の `matches!(f.kind, NodeKind::Apply(..))` を `false` にする）を手で当ててビルドし直すと、検査 a・b が 3 本とも赤になる。6/13。

```
=== MUTATED gate run ===
OK: d: nested_application_call: run の出力が // expect: と一致する
NG: a: nested_application_call: fmt の前後で run の出力が同じ (fmt が意味を変えた)
--- /tmp/tmp.OiVarQ2Bdm/run/original	2026-10-10 00:22:36.354833536 +0900
+++ /tmp/tmp.OiVarQ2Bdm/run/formatted	2026-10-10 00:22:40.674950883 +0900
@@ -1,2 +1,2 @@
-0
-42
+1
...
NG: b: nested_application_call_twice: print-postfix の出力を読み直した run が同じ (往復で意味が変わった)
--- /tmp/tmp.OiVarQ2Bdm/run/original	2026-10-10 00:23:03.601574455 +0900
+++ /tmp/tmp.OiVarQ2Bdm/run/printed	2026-10-10 00:23:12.671821525 +0900
@@ -1,2 +1,2 @@
-0
-6
+1
MUTATION-NOT-APPLIED
NG: e: 呼び手の括弧を落とす変異で a が赤になる (変異を当てられない)
PASSED: 6/13
mutated gate exit=1
```

変異を戻すと `apply_text` は元に戻り、`git status --short` に差分は残らない。

### 検体を足した後の門番

検体は `(f a) b`・`((f a) b) c`・`adder as mk -> ((mk 1) 41)` の 3 本。門番は検体をディスクから列挙し、4 検査 × 3 本 + 変異 1 = 13 を測る。

```
$ bash scripts/verify-nested-application-round-trip.sh
OK: d: nested_application_call: run の出力が // expect: と一致する
OK: a: nested_application_call: fmt の前後で run の出力が同じ
OK: c: nested_application_call: fmt の直後の fmt --check が 0（冪等）
OK: b: nested_application_call: print-postfix の出力を読み直した run が同じ
OK: d: nested_application_call_binding: run の出力が // expect: と一致する
OK: a: nested_application_call_binding: fmt の前後で run の出力が同じ
OK: c: nested_application_call_binding: fmt の直後の fmt --check が 0（冪等）
OK: b: nested_application_call_binding: print-postfix の出力を読み直した run が同じ
OK: d: nested_application_call_twice: run の出力が // expect: と一致する
OK: a: nested_application_call_twice: fmt の前後で run の出力が同じ
OK: c: nested_application_call_twice: fmt の直後の fmt --check が 0（冪等）
OK: b: nested_application_call_twice: print-postfix の出力を読み直した run が同じ
OK: e: 呼び手の括弧を落とす変異で a が赤になる（検体 3 本）
PASSED: 13/13
```

### corpus lock

`--check` の差分は足した 3 本の行だけである。

```
$ bash scripts/corpus-lock.sh --check
--- tests/corpus.sha256	2026-10-10 00:11:43.038008921 +0900
+++ /tmp/tmp.BOEdQk7xG1	2026-10-10 00:23:45.342713143 +0900
@@ -140,6 +140,9 @@
+6492996b862e6568a3efe59aa4bc7cd765fcbca71534dbb869fef59ec3c8e19d  tests/accept/nested_application_call/main.hd
+7780aad66248f6891dece80857c1e523aebd438da4a8164f48196fca4257150f  tests/accept/nested_application_call_binding/main.hd
+6b89bd7df26a0c4d28e431462e742d7bce499a2ebcbcf75ebe5c18fff798a0cd  tests/accept/nested_application_call_twice/main.hd
```

`check` のあと `bash scripts/corpus-lock.sh` で作り直す（261 行）。

### 検体の正準形

3 本は `fmt` でも `print-postfix` でも変わらない。

```
$ ./target/debug/hado fmt --check tests/accept/nested_application_call/main.hd; echo $?
0
$ ./target/debug/hado fmt --check tests/accept/nested_application_call_twice/main.hd; echo $?
0
$ ./target/debug/hado fmt --check tests/accept/nested_application_call_binding/main.hd; echo $?
0
```

### 判定

すべて緑。`cargo test --offline` は 74 スイートが緑で失敗は無く、quick 層は 41/41。quick 層の門番の集合は変わっていないので、内訳は main と同じである。

```
$ bash scripts/verify-nested-application-round-trip.sh | tail -1
PASSED: 13/13
$ bash scripts/verify-fmt-layout.sh | tail -1
PASSED: 6/6
$ bash scripts/verify-r52.sh | tail -1; bash scripts/verify-r53.sh | tail -1; bash scripts/verify-r57.sh | tail -1; bash scripts/verify-r64.sh | tail -1
PASSED: 5/5
PASSED: 5/5
PASSED: 6/6
PASSED: 4/4
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
$ cargo test --offline >target/final/cargo-test.log 2>&1; echo "exit=$?"
exit=0
$ grep -c '^test result: ok' target/final/cargo-test.log
74
$ grep -c '^test result: FAILED' target/final/cargo-test.log
0
$ bash scripts/run-gates.sh --tier quick target/.gates-nart | tail -1
TALLY: 41/41 passed; red: none
```

### 仕様への疑義

- **issue の「由来」は適用を §36 とするが、§36 は「伝播と分岐」である。** 適用と呼び出しの規範文は §13（「関数の呼び出しは並置」「定義の引数数と呼び出しの引数数は一致しなければならない」）にある。仕様本文を正として、検体の `// spec:` は §13 を指す。issue はこのラウンドで消すので、仕様は動かさない。
- **トップレベルの curry した `def` を直接入れ子適用すると `run` が落ちる。** `def f a => fn(b => a + b)` と `def main = Int.to_string ((f 1) 41)` は `check` が緑で、`build` と `run` が `Mismatch`（単相化のメッセージ）で終了コード 1 になる。`f 1 as g -> g 41` と一度束縛すると 42 になる。印字ではなく単相化の欠陥で、このラウンドのスコープ外である。検体はこの形を避け、`fn(...)` の直接適用と `as mk ->` の束縛を使う。
