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
- 呼び手の括弧を落とす変異（`matches!(f.kind, NodeKind::Apply(..))` を `false` にする）は、検体の `fmt` の前後の `run` を必ず変える。変異は常設の門番に置かず（`docs/rounds/prune-meta-gates.md`）、ラウンドの時点で 1 回当てて実測に残す。

### 判定器

issue の完了条件をそのまま使う。門番 `scripts/verify-nested-application-round-trip.sh`（`# tier: core`、`# round: nested-application-round-trip`）:

- 検体ごとに、`fmt` の前後で `run` の出力（終了コードと stdout/stderr）が同じ。
- 検体ごとに、`print-postfix` の出力を読み直した `run` が元のソースの `run` と同じ。
- 検体ごとに、`fmt` の直後の `fmt --check` が終了コード 0（冪等）。

検体は `tests/accept/` の `(f a) b` の形をディスクから列挙して数える。件数は直書きしない。検体の `run` が `// expect:` と一致することは `tests/corpus.rs` が見る。呼び手の括弧を落とす変異で門番が赤になることは、ラウンドの時点で 1 回測る。

### スコープ

触る: `scripts/verify-nested-application-round-trip.sh`（新設）、`tests/accept/nested_application_call*`（3 本の検体）、`tests/corpus.sha256`（作り直す）、`docs/rounds/nested-application-round-trip.md`、`docs/issues/` の 2 本の削除。

触らない: `crates/hado/src/postfix_print.rs`（他のレーンが同時に触っている）、`crates/hado/src/repr.rs`・`named_patterns.rs`・`float_format.rs`・`infer.rs`（同上）。実装は 1 行も変えない。`docs/spec.md`・`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`・`docs/plans/issue-backlog.md`。

### 工程

1. 検体を足さずに門番を書き、赤（検体 0 本）を見る。
2. 検体を足して門番を緑にする。
3. 変異を手で当て、検体のある木で門番が赤になることを見る。戻して緑を確かめる。
4. `tests/corpus.sha256` を、`--check` の差分を読んでから作り直す。
5. ラウンドファイルを書き、issue 2 本を消す。
6. 判定と層別を回す。

### 許容される赤

- なし。`verify-r62`・`verify-r63`・`verify-r68` などの既知の赤は main でも赤く、このラウンドの成果ではない。

### スコープ外

- `postfix_print.rs` の実装。現象は既に直っており、このラウンドの門番は実装を変えずに緑になる。
- `stdlib`・`tools` への検体追加。issue は受理コーパスへと定めており、`tools/gates` は門番の道具で印字の対象ではない。
- トップレベルの curry した `def` を直接入れ子適用する形（`def f a => fn(b => a + b)` の `((f 1) 41)`）。検体は `fn(...)` の直接適用と `as mk ->` の束縛を使う。

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

main は `b9f4d61`。作業は `.worktrees/nested-application-round-trip` の `nested-application-round-trip` で行った。

### 検体が無いこと（main）

`(f a) b` の形は main の受理コーパス・`stdlib`・`tools` に無い。

```
$ git grep -nE '\(\([a-zA-Z_]\w* [^)]*\) [^)]*\)' main -- tests/accept stdlib tools; echo "exit=$?"
exit=1
```

検体を除いた木では、門番は検体 0 本で赤い。

```
$ bash scripts/verify-nested-application-round-trip.sh; echo "exit=$?"
NG: 検体: tests/accept に (f a) b の形がある (0 本)
PASSED: 0/1
exit=1
```

### 変異

`apply_text` の `matches!(f.kind, NodeKind::Apply(..))` を `false` にしてビルドし直すと、検査 a・b が 3 本とも赤になる。変異を戻すと `git status --short` に差分は残らない。

```
$ bash scripts/verify-nested-application-round-trip.sh; echo "exit=$?"
NG: a: nested_application_call: fmt の前後で run の出力が同じ (fmt が意味を変えた)
NG: b: nested_application_call: print-postfix の出力を読み直した run が同じ (往復で意味が変わった)
NG: a: nested_application_call_binding: fmt の前後で run の出力が同じ (fmt が意味を変えた)
NG: b: nested_application_call_binding: print-postfix の出力を読み直した run が同じ (往復で意味が変わった)
NG: a: nested_application_call_twice: fmt の前後で run の出力が同じ (fmt が意味を変えた)
NG: b: nested_application_call_twice: print-postfix の出力を読み直した run が同じ (往復で意味が変わった)
PASSED: 3/9
exit=1
```

### 門番

検体は `(f a) b`・`((f a) b) c`・`adder as mk -> ((mk 1) 41)` の 3 本で、3 検査 × 3 本を測る。

```
$ bash scripts/verify-nested-application-round-trip.sh
OK: a: nested_application_call: fmt の前後で run の出力が同じ
OK: c: nested_application_call: fmt の直後の fmt --check が 0（冪等）
OK: b: nested_application_call: print-postfix の出力を読み直した run が同じ
OK: a: nested_application_call_binding: fmt の前後で run の出力が同じ
OK: c: nested_application_call_binding: fmt の直後の fmt --check が 0（冪等）
OK: b: nested_application_call_binding: print-postfix の出力を読み直した run が同じ
OK: a: nested_application_call_twice: fmt の前後で run の出力が同じ
OK: c: nested_application_call_twice: fmt の直後の fmt --check が 0（冪等）
OK: b: nested_application_call_twice: print-postfix の出力を読み直した run が同じ
PASSED: 9/9
```

### corpus lock

main との差は足した 3 本の行だけで、`--check` は差分なしで通る。

```
$ git diff main -- tests/corpus.sha256 | grep '^[-+][^-+]'
+6492996b862e6568a3efe59aa4bc7cd765fcbca71534dbb869fef59ec3c8e19d  tests/accept/nested_application_call/main.hd
+7780aad66248f6891dece80857c1e523aebd438da4a8164f48196fca4257150f  tests/accept/nested_application_call_binding/main.hd
+6b89bd7df26a0c4d28e431462e742d7bce499a2ebcbcf75ebe5c18fff798a0cd  tests/accept/nested_application_call_twice/main.hd
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
```

### 検体の正準形

3 本は `fmt` で変わらない。

```
$ ./target/debug/hado fmt --check tests/accept/nested_application_call/main.hd; echo $?
0
$ ./target/debug/hado fmt --check tests/accept/nested_application_call_twice/main.hd; echo $?
0
$ ./target/debug/hado fmt --check tests/accept/nested_application_call_binding/main.hd; echo $?
0
```

### 判定

すべて緑。

```
$ bash scripts/verify-fmt-layout.sh | tail -1
PASSED: 5/5
$ bash scripts/verify-r52.sh | tail -1; bash scripts/verify-r53.sh | tail -1; bash scripts/verify-r57.sh | tail -1; bash scripts/verify-r64.sh | tail -1
PASSED: 5/5
PASSED: 5/5
PASSED: 6/6
PASSED: 4/4
$ cargo test --offline >target/final/cargo-test.log 2>&1; echo "exit=$?"
exit=0
$ grep -c '^test result: FAILED' target/final/cargo-test.log
0
$ bash scripts/run-gates.sh --jobs 4 --tier quick target/.gates-nart | tail -1
TALLY: 51/51 passed; red: none
```

### 仕様への疑義

- **issue の「由来」は適用を §36 とするが、§36 は「伝播と分岐」である。** 適用と呼び出しの規範文は §13（「関数の呼び出しは並置」「定義の引数数と呼び出しの引数数は一致しなければならない」）にある。仕様本文を正として、検体の `// spec:` は §13 を指す。issue はこのラウンドで消すので、仕様は動かさない。
