# check --fix の印字し直しに門番の期待値を合わせる（check-fix-reprint-expectations）

## 手順書

### 背景

F5（PR #76、`350a575`、`docs/rounds/fmt-layout.md`）が `docs/spec.md` §57 に次を入れた。

> 1 つでも書き換えを当てたら、そのあと `fmt` と同じ印字し直し（§58.1）まで行う。書き換えを当てなかったソースには触らない。したがって、書き換えを当てたソースについては `check --fix`、`fmt`、`build --fix` が同じバイト列を残す。

これで `check --fix` の出力が「書き換えだけを当てた綴り」から「印字し直した正準形」へ変わった。門番 `scripts/verify-r46.sh`・`scripts/verify-r47.sh`・`scripts/verify-r63.sh` は印字し直しを経ない綴りを期待していたので赤になった。本ラウンドは期待値を §57・§58.1 から導いて直す。`crates/hado/src/**` は変えない。

F5 以前（`bf1310c`、`.worktrees/pre-f5`）では `verify-r63.sh` が 3/4 で、赤は 63-c だけである（本ラウンドで実測）。63-c は F5 と無関係な既知の欠陥で、当時から赤い。`verify-r46.sh` と `verify-r47.sh` は 5/5 と 4/4 だった（親エージェントの実測）。

### 決定

- **決定 1: 期待値の根拠は §58.1 の「印字し直した形が正準形である」に置く。** 診断の `canonical form:` は `crates/hado/src/parser.rs` の `canonical` が作る別の印字器の綴りである（`canonical` は `Neg` を無条件に `(-…)` にする）。門番は `canonical form:` の綴りを正準形の根拠にしない。二つの印字器が割れることは `docs/issues/canonical-form-hint-uses-a-second-printer.md` に切り出されている。
- **決定 2（46-b）: List の要素の負の Int は括弧で包まない。** `def main = [-2, 7, -5, 9, 8]` が正準形である。`-2` は 1 つの Int リテラルで、`,` が要素を分けるので括弧は要らない。診断の `canonical form:` は `[(-2), 7, …]` と包むが、これは `canonical_list` が `canonical` の `(-2)` をそのまま並べるためである。
- **決定 3（47-b）: 適用の引数の負の Int だけ括弧で包む。** `g 9` と `g (-9)` が正準形である。`g -9` は `g - 9` と同じに読めて E305 になるので、負の引数は括弧が要る。0 以上はそのまま並べる。診断の `canonical form:` は正の引数にも `g (9)` と包むが、これも別の印字器の綴りである。
- **決定 4（63-d の `let-fix`）: 定義の本体が束縛のときは `def f … =>` と束縛の頭を同じ行に置く。** §58.1 は「改行は式の形だけで決める。腕を持つ分岐、束縛、`fn … /fn`、`module`・`implement`・`effect`・`handler` が改行を持つ」とし、腕の並びを改行するのは「その腕の並びが定義の本体ちょうどか `fn` の本体ちょうどのときだけ」とする。この定義の本体は束縛 `if(b => 1 | 2) as v -> v + 1` であって腕の並びではないから、残る改行は束縛自身のもの（`as v ->` の後ろ）だけである。`as p ->` を最後の行の末尾に置く規則にも合う。旧期待の `=>` 直後の改行は §58.1 の不動点でなく、`fmt --check` が 1 rewrite を報告する。
- **決定 5: 63-c と 63-d の `argument-fix` は期待値の問題ではない。** どちらも実装の欠陥なので触らない（「本物の欠陥」に実測を置く）。
- **決定 6: 検査は減らさない。** 直すのは期待値の計算式とその根拠のコメントだけである。検査の本数も対象も変えない。

### 期待値の根拠

| 門番 | 入力 | 旧期待 | 新期待 | 根拠 |
|---|---|---|---|---|
| 46-b | `Cons(-2, Cons(7, Cons(-5, Cons(9, Cons(8, Nil)))))` | `[(-2), 7, (-5), 9, 8]` | `[-2, 7, -5, 9, 8]` | 決定 1・2、§58.1 |
| 47-b | `g (9) as x -> x` | `g (9)` | `g 9` | 決定 1・3、§58.1 |
| 47-b | `g (-9) as x -> x` | `g (-9)` | `g (-9)`（変わらない） | 決定 3、§58.1 |
| 63-d `let-fix` | `if(b \| true => 1 \| false => 2) as v -> v + 1` | `def f … =>` の直後で改行 | `def f … => if(b => 1 \| 2) as v ->` と 1 行 | 決定 1・4、§58.1 |

46-b と 47-b の期待値は値から計算する（負の値の扱いだけを規則から決める）。63-d は入力と期待を対で持つ（もとからの門番の形）。

### 本物の欠陥（このラウンドでは直さない）

- **63-c: `type T = | A | B` を宣言する根が E402 で拒否される。** 門番の fixture がこの形を持つため、`check` の最初の診断が E520 でなく E402 になり 63-c が赤い。F5 以前の `bf1310c` でも同じ E402 が出る（`verify-r63.sh` は 3/4 で 63-c が赤）。原因は `docs/issues/implement-local-adt-identity-is-name-only.md` のとおりで、根の variant `T` が prelude の `implement Map as Map` の内部型 `T` と同じ `Ty::Adt("T")` に束なり、網羅性が混ざる。`type U = | A | B` に変えると E520 が最初に出る。期待値の書き換えでは直らない。
- **63-d の `argument-fix`: 印字し直しが動くプログラムを E100 にする。** `id (if b => 1 | 2)` を印字し直すと `id if(b => 1 | 2)` になり、`check` が `error[E100]: if(...) with nothing after it` を出す。並置の最後の引数を括弧で囲まない印字器の欠陥である。PR #81（`cursor/critical-bug-management-13c6`、未着地）が直す。門番の期待値 `id (if b => 1 | 2)` は正しいので変えない。

### 成果物

- `scripts/verify-r46.sh`・`scripts/verify-r47.sh`・`scripts/verify-r63.sh`（期待値と根拠のコメント）
- `docs/rounds/check-fix-reprint-expectations.md`（本ファイル）

### 工程

1. worktree を切る。
2. 3 本の門番の赤を測り、`canonical form:` と印字し直しの出力を比べる。
3. 期待値を §57・§58.1 から導いて直す。
4. r46・r47・r63 を回し、残った赤を実装の欠陥として切り分ける。
5. 本ファイルを書く。
6. fmt-layout と quick 層と `cargo test` を最後に 1 回回す。

### 判定

- `verify-r46.sh` が 5/5。
- `verify-r47.sh` が 4/4。
- `verify-r63.sh` は 63-a・63-b が緑、63-c と 63-d が赤（本物の欠陥 2 件）。63-d の `let-fix` の期待値は本ラウンドで直す。
- `verify-fmt-layout.sh` が 6/6。
- `bash scripts/run-gates.sh --tier quick target/.gates-cfre` が 41/41。
- `cargo test --offline` が 0 failed。

### 許容される赤

- `verify-r63.sh` の 63-c（`implement-local-adt-identity-is-name-only`）と 63-d の `argument-fix`（PR #81）。どちらも F5 の期待値の問題ではない。
- `verify-r62.sh`・`verify-r68.sh` の既知の赤（同じ issue）。

### スコープ外

- `crates/hado/src/**` の実装。門番とラウンドファイルだけを触る。
- `docs/spec.md`。F5 の決定は §57・§58.1 に書かれている。
- `tests/accept`・`tests/reject`・`stdlib/`、`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`。
- 63-c と 63-d の `argument-fix`（別のレーンが受け持つ）。

### 中断の条件

- 期待値を §57・§58.1 の文から説明できない食い違いが残ったら、直さずに報告して止める。
- 実装を変えないと直らないと分かったら、止めて報告する。

### 仕様への疑義

- **診断の `canonical form:` が正準形と違う。** `parser.rs` の `canonical` は `Neg` を無条件に `(-…)` にし、適用の引数にも括弧を付ける。`fmt` の印字し直し（`postfix_print.rs` の `program`）は文脈で括弧を決める。§57 は診断に `canonical form: <正準形>` を足すと定めるので、`canonical form:` が正準形でないのは仕様違反に見える。ただし §58.1 が正準形を印字し直しの側に定めているので、直すべきは案内の側である。`docs/issues/canonical-form-hint-uses-a-second-printer.md` に切り出されている。
- **E402 の位置が実在しない行を指す。** 63-c の E402 は `main.hd:255:1` を指すが、入力は 4 行である。prelude の型と混ざった結果の位置で、`implement-local-adt-identity-is-name-only` の一部である。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §57 | 書き換えを当てたら `fmt` と同じ印字し直しまで行う | 冗長 | — | `scripts/verify-r46.sh`・`scripts/verify-r47.sh` |
| §58.1 | 印字し直した形が正準形である | 冗長 | — | `scripts/verify-r46.sh`・`scripts/verify-r47.sh`・`scripts/verify-r63.sh` |
| §58.1 | 書き換えが尽きたソースは `fmt` の不動点である | 冗長 | — | `scripts/verify-r63.sh` の 63-d `let-fix` |
| §58.1 | 腕の並びは定義の本体ちょうどのときだけ改行する | 冗長 | — | `scripts/verify-r63.sh` の 63-d `let-fix` |
| §58.1 | `as p ->` は複数行のとき最後の行の末尾に置く | 冗長 | — | `scripts/verify-r63.sh` の 63-d `let-fix` |

## 実測

main は `4baca98`。作業は `.worktrees/check-fix-reprint-expectations` の `check-fix-reprint-expectations` で行った。探針の出力は `target/cfre/evidence/` に置いた（追跡しない）。

### 46-b: 診断の綴りと印字し直しの綴りが割れる

```
$ cat target/cfre/evidence/46b/main.hd
def main = Cons(-2, Cons(7, Cons(-5, Cons(9, Cons(8, Nil)))))
$ ./target/debug/hado check target/cfre/evidence/46b/main.hd
main.hd:1:12: error[E510]: use List literal syntax instead of Nil or Cons; canonical form: [(-2), 7, (-5), 9, 8]
$ ./target/debug/hado check --fix target/cfre/evidence/46b/main.hd
fmt: main.hd (1 rewrites)
$ cat target/cfre/evidence/46b/main.hd
def main = [-2, 7, -5, 9, 8]
$ ./target/debug/hado check target/cfre/evidence/46b/main.hd; echo $?
0
$ ./target/debug/hado fmt --check target/cfre/evidence/46b/main.hd; echo $?
0
```

`canonical form:` は `[(-2), …]` と括弧を付けるが、`check --fix` が残すバイト列は `[-2, …]` で、`check` が受理し `fmt --check` が 0 を返す。§58.1 の「印字し直した形が正準形」に従い、門番の期待値を括弧なしへ直した。

### 47-b: 適用の引数の負の Int

```
$ ./target/debug/hado check target/cfre/evidence/47b_pos/main.hd
main.hd:3:3: error[E501]: binding x is returned unchanged at 3:17; canonical form: g (9)
$ ./target/debug/hado check --fix target/cfre/evidence/47b_pos/main.hd
fmt: main.hd (1 rewrites)
$ cat target/cfre/evidence/47b_pos/main.hd
def g n => n + 5
def main = Int.to_string (g 9)
$ ./target/debug/hado check target/cfre/evidence/47b_pos/main.hd; echo $?
0
$ ./target/debug/hado fmt --check target/cfre/evidence/47b_pos/main.hd; echo $?
0
```

```
$ ./target/debug/hado check --fix target/cfre/evidence/47b_neg/main.hd
fmt: main.hd (1 rewrites)
$ cat target/cfre/evidence/47b_neg/main.hd
def g n => n + 5
def main = Int.to_string (g (-9))
$ ./target/debug/hado check target/cfre/evidence/47b_neg/main.hd; echo $?
0
$ ./target/debug/hado fmt --check target/cfre/evidence/47b_neg/main.hd; echo $?
0
```

正の引数は括弧が落ち、負の引数は残る。括弧が要るのは、外すと引き算に読めるためである。

```
$ cat target/cfre/evidence/47b_noparen/main.hd
def g n => n + 5
def main = Int.to_string (g -9)
$ ./target/debug/hado check target/cfre/evidence/47b_noparen/main.hd; echo $?
main.hd:2:27: error[E305]: cannot unify Fn(Int => Int {A}) with Int
1
```

### 63-c: E402 は F5 と無関係な既知の欠陥

fixture の `type T = | A | B` は prelude の `implement Map as Map` の内部型 `T` と混ざる。`check` の最初の診断が E520 でなく E402 になり、門番の `check_code(..., 'E520')` が落ちる。

```
$ cat target/cfre/evidence/63c_T/main.hd
type T = | A | B
def f (b: Bool): Int => if b | true => 2 | false => 3
def g (t: T): Int => if t | A => 1 | _ => 0
def main = f true + g A
$ ./target/debug/hado check target/cfre/evidence/63c_T/main.hd; echo $?
main.hd:255:1: error[E402]: not all constructor values are covered
1
$ ./target/debug/hado check --fix target/cfre/evidence/63c_T/main.hd; echo $?
main.hd:255:1: error[E402]: not all constructor values are covered
1
$ ./target/debug/hado run target/cfre/evidence/63c_T/main.hd; echo $?
main.hd:255:1: error[E402]: not all constructor values are covered
1
```

`--fix` は E402 に書き換えを持たないのでファイルを変えず、`run` も E402 で落ちる。`type` の名前を変えると E520 が最初に出る（門番が意図した診断へ届く）。

```
$ ./target/debug/hado check target/cfre/evidence/63c_U/main.hd; echo $?
main.hd:2:25: error[E520]: use a Boolean branch instead of Boolean pattern arms or a single condition arm; canonical form: if b => 2 | 3
1
```

F5 以前の `bf1310c` でも同じ E402 が出る。門番は当時から 3/4 で、63-c だけが赤だった。

```
$ cd .worktrees/pre-f5 && ./target/debug/hado check .../main.hd
main.hd:255:1: error[E402]: not all constructor values are covered
$ bash scripts/verify-r63.sh 2>&1 | tail -3
NG: 63-c E503 and E520 settle together
OK: 63-d the exclusions and the argument position
PASSED: 3/4
```

### 63-d: 印字し直しが動くプログラムを E100 にする

```
$ cat target/cfre/evidence/63d_arg/main.hd
def id (n: Int): Int => n
def f (b: Bool): Int => id (if b | true => 1 | false => 2)
def main = f true
$ ./target/debug/hado check --fix target/cfre/evidence/63d_arg/main.hd
fmt: main.hd (1 rewrites)
$ cat target/cfre/evidence/63d_arg/main.hd
def id (n: Int): Int => n
def f (b: Bool): Int => id if(b => 1 | 2)
def main = f true
$ ./target/debug/hado check target/cfre/evidence/63d_arg/main.hd; echo $?
main.hd:2:28: error[E100]: `if(...)` with nothing after it; write it without parentheses; canonical form: if b => 1 | 2
1
```

### 63-d: 束縛の本体は同じ行に並ぶ（本ラウンドで直した期待値）

```
$ ./target/debug/hado check --fix target/cfre/evidence/63d_let/main.hd
fmt: main.hd (1 rewrites)
$ cat target/cfre/evidence/63d_let/main.hd
def f (b: Bool): Int => if(b => 1 | 2) as v ->
  v + 1
def main = f true
$ ./target/debug/hado check target/cfre/evidence/63d_let/main.hd; echo $?
0
$ ./target/debug/hado fmt --check target/cfre/evidence/63d_let/main.hd; echo $?
0
```

旧期待は `fmt` の不動点でない。

```
$ ./target/debug/hado check target/cfre/evidence/63d_let_old/main.hd; echo $?
0
$ ./target/debug/hado fmt --check target/cfre/evidence/63d_let_old/main.hd; echo $?
fmt: main.hd is not canonical (1 rewrites)
1
```

### 門番の走行

```
$ bash scripts/verify-r46.sh 2>&1 | tail -1
PASSED: 5/5
$ bash scripts/verify-r47.sh 2>&1 | tail -1
PASSED: 4/4
$ bash scripts/verify-r63.sh 2>&1 | tail -1
PASSED: 2/4
$ bash scripts/verify-fmt-layout.sh 2>&1 | tail -1
PASSED: 6/6
```

`verify-r63.sh` の残る 2 本は 63-c と 63-d で、どちらも本物の欠陥である（上の実測）。

### 層別と全走

```
$ bash scripts/run-gates.sh --tier quick target/.gates-cfre 2>&1 | tail -1
TALLY: 41/41 passed; red: none
```

```
$ cargo test --offline 2>&1 | tail -25
     Running tests/type_engine.rs (target/debug/deps/type_engine-7b51220bf28a1951)

running 5 tests
test failed_unification_does_not_leave_partial_substitutions ... ok
test constrained_scheme_instantiates_predicate_with_function_type ... ok
test generalize_preserves_environment_variables_and_freshens_others ... ok
test occurs_rejects_indirect_infinite_types ... ok
test unify_substitutes_nested_types ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/walk.rs (target/debug/deps/walk-9c7cc362abb217e9)

running 1 test
test walk ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.53s

   Doc-tests hado

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test` は失敗したテスト対象で止まる（`--no-fail-fast` を付けない既定）ので、最後の `Doc-tests hado` まで届いたことと、そこで `0 failed` であることが、全スイート 0 failed の根拠である。

