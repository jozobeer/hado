# 後置の束縛の試作パーサによる検証（postfix-binding-probe）
開始 2026-09-27。`docs/issues/postfix-binding.md` の推奨を試作し、コーパスが 1 通りの綴りで書けて
同じ構文木へ戻るかを測る。仕様は変えない。既定の経路も変えない。採否は人が決める。

## 手順書
- 表層は既存の `NodeKind` へ写す。変種は足さない。`as` は最も弱く、直前の区切りから `as` までを束縛する。
- 閉じ: `->` を持つ `if` は `/if`、続きがあるときだけ `if(...)`、無ければ閉じない。`fn` は `fn(...)`
  か `fn … /fn`。`module`・`implement` は `/`+名前。不足・余分・食い違いは `P001`（`canonical form:`
  付き）か E100。`fn` を二項演算子の被演算子に直接置くのは E100。
- `check`・`run`・`dump-mono` に `--syntax postfix` を足す。印字器は演算子の強さで括弧を省き、`->` の後と
  `/if`・`/fn` の前で改行する。`print-postfix <入力根> <出力先>` は `.hd`・`.hdi` を写し先頭コメントを残す。
- 成果物: `parser.rs`・`postfix_print.rs`・`main.rs`・`project.rs`・門番・`tests/postfix-probe/`・本ファイル。
- 中断: 新しい `NodeKind` を要するとき、既定の経路を変えずに試作モードを入れられないとき。

## 系譜
| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §11・§34・§35 | 束縛は `e as p ->`、破棄は `e ->`、`let`/`do` は E100 | 解析 | 試作 | `neg/N1・N2` |
| §12・§32 | `if` は `->` があれば `/if`、続きがあるときだけ `if(...)` | 解析 | 試作 | `pairs/P1・P2・P7` |
| §12 | `fn` は `fn(...)` か `fn … /fn`。被演算子には置けない | 解析 | 試作 | `pairs/P4・P6`・`neg/N8` |
| §10 | `module`・`implement` は `/`+名前で閉じる | 解析 | 試作 | `run`・`tools` の J1 |
| §34・§57 | 閉じの不足は P001、余分・食い違いは E100 | 解析 | 試作 | `neg/N5〜N9` |

## 実測
```
$ bash scripts/verify-postfix-binding-probe.sh 2>&1 | tail -16
J1:往復(ok=196): PASS
J2:冪等(ok=196): PASS
j3 cases=111 fail=0
j4 cases=54 mismatch=3
  j4-mismatch: E404 want=E404 got=None
  j4-mismatch: E516 want=E516 got=None
  j4-mismatch: E517 want=E517 got=None
J3:受理の実行(cases=111): PASS
J4:拒否のコード(cases=54,mismatch=3): ISSUES
J5:手書き(cases=19): PASS
J6 arrow-if-with-parens: caught
J6 drop-whole-if-parens: caught
J6 drop-option-question: caught
J6:変異(cases=3): PASS
PASSED: 5/6
$ bash scripts/verify-postfix-binding-probe.sh 2>&1 | grep print-postfix:
tests/accept: print-postfix: files=133 hd=126 hdi=7 skipped=0 dropped-comments=1
tests/reject: print-postfix: files=74 hd=73 hdi=1 skipped=16 dropped-comments=0
stdlib: print-postfix: files=10 hd=10 hdi=0 skipped=0 dropped-comments=11
tools: print-postfix: files=3 hd=3 hdi=0 skipped=0 dropped-comments=0

| .hd | /if | if( | /fn | fn( | /module | /implement | 複数行 if( | 最長行 |
| 196 | 4 | 4 | 0 | 21 | 44 | 51 | 3 | 282 |
```

J1/J2 は受理 126・旧パーサが読む拒否 57・stdlib 10・tools 3 の 196 で緑。J3 は受理 111 が一致。
J4 の 3 件は AST が綴りを持たず変換が正準形を書いて欠陥が消える（E404 は腕と fallback の順、E516 は
`Nil`/`[]`、E517 は `if` の式形）。J5 の N9（Option に裸の `->`）は E303 を期待したが E305、E303 の
文言も `as p? ->` を案内しない。記録のみで赤にしない。元の `let`・`do` は受理+stdlib で 20・7（issue
と一致）、tools までで 28・7、拒否までで 33・7。落とした本文コメントは 12 行。`/fn` が 0 なのは本体に
`->` を持つ `fn` がコーパスに無いため（issue の計数と一致）。

### 書き換えの例（旧 → 新）
```
effect_position（/if が長い本体を閉じる）
  let scrutinee = if fetch true | Yes => 1 | 0 ->    → if(fetch true | Yes => 1 | 0) as scrutinee ->
array_compare（if(…) が長くなる。元は 1 行）
  [if left == right => 1 | 0, … compare left right]  → [if(left == right => 1 | 0), … compare left right]
shared_store_int（/module）
  module Store / def find(id: Int): T Pure / implement Store as Store
  → module Store / /module / def find (id: Int): T Pure / implement Store as Store
```

### 仕様への疑義
- `if` の直後が `(` のとき、閉じの `if(...)` と条件の括弧を字句で区別できない。印字器は条件の外側の
  括弧を省いて避けた。人が `if (x) => a | b` と書くと E100 になる。`/`+`fn` は閉じとも除算とも読め、
  試作は「`/fn` で閉じる `fn` の中だけ閉じ」とした（`10 / fn(x => x)` は被演算子の規則で E100）。
- `with H ->` は `->` を持つが継続と数えない。複数引数の `fn` の `/fn` の綴りは未決。
- E303 の文言は `as p? ->` を案内しない（N9）。Option の裸の `->` は E303 でなく E305。
```
$ git log --oneline main..（docs コミット前）  $ git status --short
25a1d66 test: 後置束縛の探針と…        ?? docs/rounds/postfix-binding-probe.md
dc96f07 feat: 後置束縛の試作パーサ…    （本コミットで消える）
```
