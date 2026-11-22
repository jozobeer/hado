# 入れ子の適用の正準形（canonical-application-parentheses）

## 手順書

### 背景

#110 の現象。`hado check` の `canonical form:` と `hado fmt` が、適用の呼び手が
適用である式 `(add 1) 2` を `add 1 2` と書く。前者は 1 引数関数 `add` への適用の
あと、返った `fn(y => ...)` へのもう 1 回の適用である。後者は `add` への 2 引数
呼び出しで、§13 の「定義の引数数と呼び出しの引数数は一致しなければならない」に
反し E301 になる。

E510（`Cons` → リスト）と E520（真偽の腕 → `if`）は、この正準形をソースへ書く
（§57）。書き換えは意味を変えてはならないので、綴りが壊れていると `--fix` と
`fmt` が壊れたソースを残す。印字し直し（`postfix_print::apply_text`）は同じ形を
括弧で守っているが、診断と書き換えが使う `parser::canonical_apply` が守っていない。

### 意図

正準形を、書き戻しても同じ意味になる綴りにする。呼び手が適用のときは括弧で囲み、
`fmt`・`check --fix`・`canonical form:` の 3 つが同じ綴りを出すようにする。

### 仮説

- 括弧を落としているのは `parser::canonical_apply` の呼び手の綴りだけである。
- 呼び手が `Apply` のとき括弧で囲めば、E510 と E520 の書き換え、`fmt`、`check --fix`
  が同じ正準形を出す。
- 平坦化した `[add 1 2]` は E301 なので、括弧の有無は意味の差として測れる。
- 呼び手が `fn(...)` の形は、括弧が無くても読めて同じ値になる（実測）。今回は触らない。

### 判定器

`scripts/verify-canonical-application-parentheses.sh`（`# tier: quick`、
`# round: canonical-application-parentheses`）:

- 検査 a: E510 の `canonical form:` を `main` の本体に書いたソースが `[3]` を出す。
- 検査 b: `fmt` の出力がその `canonical form:` と同じ綴りで、`run` が `[3]` を出す。
- 検査 c: E520 の `canonical form:` をリストに置いたソースが `[3]` を出す。
- 検査 d: 平坦化した `[add 1 2]` が E301 で拒否される（対照）。
- 検査 e: `check --fix` と `fmt` が同じバイト列を残す（§57）。

期待値は診断から読む。平坦化した綴りだけを対照として門番に書く。単体テストは
`crates/hado/tests/r46_fixes.rs` の `nested_application_inside_cons_keeps_call_boundaries`
と `nested_application_in_a_boolean_arm_keeps_call_boundaries` が持つ。

### スコープ

触る: `crates/hado/src/parser.rs`、`crates/hado/tests/r46_fixes.rs`、
`scripts/verify-canonical-application-parentheses.sh`（新設）、
`docs/rounds/canonical-application-parentheses.md`。

触らない: `docs/spec.md`（§13・§57・§58.1 の解釈は変えない）、`tests/accept`・
`tests/reject`・`stdlib/`（この修正はコーパスを変えない）、
`scripts/verify-nested-application-round-trip.sh`（印字し直しの往復は既存の門番が持つ）。

### 工程

1. main で現象を再現し、`canonical form: [add 1 2]` と、その綴りを書いた `fmt` の
   出力が E301 になることを見る。
2. 再現テストと専用の門番を書き、main で赤、修正後に緑になることを確かめる。
3. `parser::canonical_apply` で呼び手の適用を括弧で囲む。
4. 単体テスト・専用の門番・quick 層・`lint-docs` を回す。
5. main を取り込み、`corpus lock` の差分が無いことを確かめる。
6. PR を着地し、main の full 層を実測してこのファイルに足す。

### 許容される赤

- なし。

### スコープ外

- 呼び手が `fn(...)` の形の正準形。`fn(x => x) 1` は括弧なしで読めて同じ値になるので、
  この現象ではない。
- 部分適用の意味論。§13 は部分適用を許さず、`(add 1) 2` は 1 引数関数の適用の連鎖である。
- 印字し直しの往復。`postfix_print::apply_text` は既に括弧を守っており、
  `scripts/verify-nested-application-round-trip.sh` が測っている。

### 中断の条件

- 括弧を付けても E510・E520 の書き換えが同じ意味を残さないと分かった。
- 正準形の変更が、受理コーパスか `fmt` の既存の期待値を割った。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §13 | 呼び出しは並置 `f x y` で、定義の引数数と呼び出しの引数数は一致する（部分適用は E301） | 型 | E301 | `crates/hado/tests/r46_fixes.rs` の `nested_application_inside_cons_keeps_call_boundaries`、門番検査 d |
| §57 | 局所的な書き換えは意味を変えず、`canonical form:` にその綴りを添える | 構文 | E510・E520 | 門番検査 a・c・e、`crates/hado/tests/r46_fixes.rs` の 2 本 |
| §58.1 | 書き換えを尽くして印字し直した形が正準形で、同じ意味のソースは同じバイト列になる | 構文 | E301（平坦化した形） | 門番検査 b・e |

## 実測

main は `de014f1`。作業は `.worktrees/canonical-application-parentheses` の
`canonical-application-parentheses` で行った。

### 現象（main）

`canonical form:` が括弧を落とし、`fmt` がその綴りを書く。書き換えたソースは E301。

```
$ ./target/debug/hado check target/repro110/a/main.hd; echo "exit=$?"
main.hd:2:12: error[E510]: use List literal syntax instead of Nil or Cons; canonical form: [add 1 2]
exit=1
$ ./target/debug/hado fmt target/repro110/b/main.hd && cat target/repro110/b/main.hd
fmt: main.hd (1 rewrites)
def add x => fn(y => x + y)
def main = [add 1 2]
$ ./target/debug/hado run target/repro110/b/main.hd; echo "exit=$?"
 main.hd:2:13: error[E301]: cannot unify Fn(Int => Fn(Int => Int {A}) {B}) with Fn(Int, Int => C {D})
exit=1
```

### 修正後

同じソースの `canonical form:` と `fmt` が括弧を残し、`run` が `[3]` を出す。

```
$ ./target/debug/hado check target/repro110/a/main.hd; echo "exit=$?"
main.hd:2:12: error[E510]: use List literal syntax instead of Nil or Cons; canonical form: [(add 1) 2]
exit=1
$ ./target/debug/hado fmt target/repro110/b/main.hd && cat target/repro110/b/main.hd
fmt: main.hd (1 rewrites)
def add x => fn(y => x + y)
def main = [(add 1) 2]
$ ./target/debug/hado run target/repro110/b/main.hd; echo "exit=$?"
[3]
exit=0
```

平坦化した綴りは受理されない（対照）。

```
$ ./target/debug/hado run target/probe110/flat/main.hd; echo "exit=$?"
 main.hd:2:13: error[E301]: cannot unify Fn(Int => Fn(Int => Int {A}) {B}) with Fn(Int, Int => C {D})
exit=1
```

`check --fix` は `fmt` と同じバイト列を残す（§57）。

```
$ ./target/debug/hado check --fix target/probe-fix2/main.hd; echo "exit=$?"; cat target/probe-fix2/main.hd; ./target/debug/hado run target/probe-fix2/main.hd
exit=0
def add x => fn(y => x + y)
def main = [(add 1) 2]
[3]
```

### 単体テスト

```
$ cargo test --offline --test compiler r46_fixes:: 2>&1 | tail -3
running 6 tests
...
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 419 filtered out; finished in 0.00s
```

### 門番

main の `parser.rs` に戻すと検査 a・b・c・e が赤で、対照の d だけが緑（1/5）。

```
$ bash scripts/verify-canonical-application-parentheses.sh
NG: a: E510 の canonical form を本体に書いたソースが [3] を出す (exit=1 hint=[add 1 2])
NG: b: fmt の出力が同じ canonical form で [3] を出す (printed=[add 1 2] exit=1)
NG: c: E520 の canonical form をリストに置いたソースが [3] を出す (exit=1 hint=if true => add 1 2 | 0)
OK: d: 平坦化した [add 1 2] は E301 で拒否される
NG: e: check --fix と fmt が同じバイト列を残す (main.hd:2:13: error[E301]: cannot unify Fn(Int => Fn(Int => Int {A}) {B}) with Fn(Int, Int => C {D}))
PASSED: 1/5
```

修正後は 5/5。

```
$ bash scripts/verify-canonical-application-parentheses.sh
OK: a: E510 の canonical form を本体に書いたソースが [3] を出す
OK: b: fmt の出力が同じ canonical form で [3] を出す
OK: c: E520 の canonical form をリストに置いたソースが [3] を出す
OK: d: 平坦化した [add 1 2] は E301 で拒否される
OK: e: check --fix と fmt が同じバイト列を残す
PASSED: 5/5
```

### corpus lock

このラウンドはコーパスを変えない。main を取り込んだ後の照合は差分なし。

```
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
```

### 触った範囲のテスト

```
$ cargo test --offline --test compiler 2>&1 | tail -1
test result: ok. 425 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 21.67s
$ cargo test --offline --test command 2>&1 | tail -1
test result: ok. 392 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 22.14s
```

### 層別（quick）

```
$ bash scripts/run-gates.sh --jobs 2 --tier quick target/.gates-round2 | tail -1
TALLY: 25/25 passed; red: none
```

### 文書層

```
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
```

