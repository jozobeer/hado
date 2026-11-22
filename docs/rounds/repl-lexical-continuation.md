# REPL の継続を字句の `->` で切る（repl-lexical-continuation）

## 手順書

### 背景

#113 の現象。`split_step` は入力の文字列の接尾辞 `->` を剥がしていた。行コメントは
字句に残らないので、`1 // as x ->` のコメントの中の `->` が継続になり、直前の `as x`
を束縛として拾って先の値を上書きする。逆向きの `1 as x -> // keep` では末尾が `keep`
なので剥がさず、`->` が続きとして送られないまま E100 と E204 になる。

### 意図

入力の最後の字句が `->` のときだけ継続として剥がす。束縛はその直前が `as` と名前の
字句のときだけ。本体はその字句より前のソースで、切り目は字句と同じスカラ単位にする。
字句が読めない入力は今までどおり剥がさず、本検査の E100 に任せる。

§5 はコメントを `//` の行コメントだけとし、字句に残さない。§11 は `->` を後置の
束縛の継続記号とし、`e as x ->` の `x` には名前だけを置けるとする。継続の判定を
文字列の接尾辞ではなく字句列にするのは、この 2 節の区別をそのまま実装に写すことである。

### 仮説

- 最後の字句が `->` のときだけ剥がせば、コメントの中の `->` は継続にも束縛にもならない。
- `->` の後ろのコメントは字句に残らないので、束縛の判定は直前の `as` と名前の字句で決まる。
- 剥がす位置を字句のスパンからバイト位置へ直せば、多バイト文字を含む入力でも切れ目が
  文字境界に当たる。
- 字句が読めない入力の扱い（剥がさず E100）は変えないので、既存の E100 の診断は動かない。

### 判定器

`scripts/verify-repl.sh` 検査 12（既存の門番に足した。`# tier: quick`、`# round: repl`）:

- a. `0 as x ->` のあと `1 // as x ->` の `x` は `0` のまま。`1` と `0` を印字する。
- b. `1 as x -> // keep` のあとの `x` は `1` を印字する。
- c. `1 -> // gone` は値を破棄し、次の `2` を印字する。診断は出さない。

単体テストは `crates/hado/src/repl.rs` の 5 本、結合テストは
`crates/hado/tests/repl.rs` の `repl_does_not_take_a_binding_from_a_comment` が持つ。

### スコープ

触る: `crates/hado/src/repl.rs`、`crates/hado/tests/repl.rs`、`scripts/verify-repl.sh`、
`docs/rounds/repl.md`（決定 16）、`docs/rounds/repl-lexical-continuation.md`。

触らない: `docs/spec.md`、`tests/accept`・`tests/reject`・`stdlib/`（この修正はコーパスを
変えない）。

### 工程

1. main で 3 つの入力を再現する（束縛の上書き・束縛の失敗・E100）。
2. テストと門番を先に置き、main の木で赤を測る。
3. `split_step` を字句列で判定するように直す。
4. main を取り込み、corpus lock の差分を読んでから作り直す。
5. 門番・quick 層を回して PR にする。
6. 着地後に main で full 層を実測し、このファイルに足す。

### 許容される赤

- なし。

### スコープ外

- `input_complete` の未完判定（決定 8）は変えない。字句が読めない入力を未完として
  続きを待つ動きはそのままで、`split_step` は EOF 後の最終判定にだけ当たる。
- 字句にならない空白（NBSP など）を含む `1 as\u{a0}x ->` は、字句が読めない入力として
  剥がさず E100 に任せる。旧 `trailing_binding` はこれを束縛にしていたが、仕様の字句に
  無い区切りであり、issue の Fix 節の「直前が `as` と値の名前のときだけ」に従う。

### 中断の条件

- 字句列での判定が、仕様が受理する継続の形（`e as x ->`・`e ->`）まで剥がさなくなる。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §5 | コメントは `//` の行コメントだけ。字句には残らない | 字句 | `split_step`（`lex_full` の字句列） | `scripts/verify-repl.sh` 検査 12、`crates/hado/tests/repl.rs` の `repl_does_not_take_a_binding_from_a_comment` |
| §11 | `->` は後置の束縛の継続記号。`e as x ->` と `e ->`。`as` の右は名前だけ | 入力の区切り | `split_step`・`binding_name`・`source_before` | 単体テスト `a_comment_ending_in_an_arrow_is_not_a_binding` ほか 4 本、検査 12 |

## 実測

main は `b67f323`。作業は `.worktrees/repl-lexical-continuation` の
`repl-lexical-continuation` で行った。

### 現象（main）

コメントの中の `->` が束縛を作り、先の値を上書きする。`0` が印字されない。

```
$ printf '0 as x ->\n1 // as x ->\nx\n' | cargo run --offline -q -- repl; echo "exit=$?"
1
exit=0
```

`->` の後ろのコメントが束縛を隠す。

```
$ printf '1 as x -> // keep\nx\n' | cargo run --offline -q -- repl >out 2>err; echo "exit=$?"; echo "stdout: $(cat out)"; echo "stderr: $(cat err)"
exit=0
stdout: 
stderr: main.hd:3:1: error[E100]: expected an expression, found end of input
main.hd:1:12: error[E204]: unbound name x
```

`1 -> // gone` も `->` が継続として送られず、E100 になる。

```
$ printf '1 -> // gone\n2\n' | cargo run --offline -q -- repl >out 2>err; echo "exit=$?"; echo "stdout: $(cat out)"; echo "stderr: $(cat err)"
exit=0
stdout: 2
stderr: main.hd:3:1: error[E100]: expected an expression, found end of input
```

### テスト先行（`1d9fe50`）

`split_step` を直す前の木で単体テストは赤。

```
$ cargo test --offline --lib repl::tests::
---- repl::tests::a_comment_ending_in_an_arrow_is_not_a_binding stdout ----
thread 'repl::tests::a_comment_ending_in_an_arrow_is_not_a_binding' panicked at crates/hado/src/repl.rs:914:9:
assertion failed: step.display
---- repl::tests::a_comment_after_the_arrow_still_binds stdout ----
thread 'repl::tests::a_comment_after_the_arrow_still_binds' panicked at crates/hado/src/repl.rs:922:9:
assertion `left == right` failed
  left: None
 right: Some("x")
test result: FAILED. 6 passed; 2 failed; 0 ignored; 0 measured; 45 filtered out
```

main の木に新しい門番を一時的に置くと、検査 12 が赤（`scripts/verify-repl.sh` は
測ったあと `git checkout` で戻し、`git status --short` は空）。

```
$ bash scripts/verify-repl.sh
OK: 11 a later input may reuse a binding name as a parameter or field
  stdout=1
NG: 12 a line comment does not create or hide a binding
PASSED: 11/12
```

### 修正後

3 つの入力が期待どおりになる。

```
$ printf '0 as x ->\n1 // as x ->\nx\n' | cargo run --offline -q -- repl; echo "exit=$?"
1
0
exit=0
$ printf '1 as x -> // keep\nx\n' | cargo run --offline -q -- repl; echo "exit=$?"
1
exit=0
$ printf '1 -> // gone\n2\n' | cargo run --offline -q -- repl; echo "exit=$?"
2
exit=0
```

### テスト

```
$ cargo test --offline --lib repl::tests::
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 45 filtered out
$ cargo test --offline --test command repl::
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 374 filtered out
```

### 門番

```
$ bash scripts/verify-repl.sh
OK: 12 a line comment does not create or hide a binding
PASSED: 12/12
```

### lint

```
$ cargo fmt --all --check; echo "exit=$?"
exit=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [optimized + debuginfo] target(s) in 18.51s
$ cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity 2>&1 | tail -1
    Finished `dev` profile [optimized + debuginfo] target(s) in 3.53s
```

### corpus lock

このラウンドはコーパスを変えない。main `b67f323` を取り込んだ後の照合は差分なし。

```
$ git merge main
Already up to date.
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
```

### 層別（quick）

```
$ bash scripts/run-gates.sh --jobs 2 --tier quick target/.gates-round5 2>&1 | tail -1
TALLY: 38/38 passed; red: none
```

### 着地後（main `37dfc2c`）

PR #126 を merge した。main の CI 44974287917 は success。

```
$ gh run view 44974287917 --json conclusion,headSha
{"conclusion":"success","headSha":"37dfc2c1fddf375003e73d24b3dc6ffa2c09a7e6"}
```

検査 12 は gates:4/5 で走り、この層の計数は 518/518 だった。

```
gates (gates:1/5) TALLY: 1473/1473 passed; red: none
gates (gates:2/5) TALLY: 958/958 passed; red: none
gates (gates:3/5) TALLY: 594/594 passed; red: none
gates (gates:4/5) RUN: verify-repl -> /home/runner/work/hado/hado/target/ci-gates/verify-repl.log
gates (gates:4/5) TALLY: 518/518 passed; red: none
gates (gates:5/5) TALLY: 156/156 passed; red: none
gates (tests:1/3) TALLY: 5/5 passed; red: none
gates (tests:2/3) TALLY: 5/5 passed; red: none
gates (tests:3/3) TALLY: 5/5 passed; red: none
```

main を checkout した側でも門番と corpus lock を測った。

```
$ bash scripts/verify-repl.sh 2>&1 | tail -1
PASSED: 12/12
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
```

PR #113 は superseded として閉じた。
