# REPL の入力の完結を閉じのキーワードで決める（repl-if-input-terminator）

## 手順書

### 背景

#111 の現象。`hado repl` は 1 行読むごとに入力が完結したかを `input_complete` で判定し、
未完なら次の行を待つ。`block_balance` が閉じのキーワード（`/fn` の `fn` など）を開きに
も数え、`/if` をブロックの閉じに数えていた。

- 関数の中の `/if` で入力が割れる。`/fn` より後ろが別の式になり、束縛した関数は腕の
  中の効果を走らせない。main では `fn` が閉じていないという E100 と、`f` の E204 が
  出た。
- `effect` と `handler` は対応する `/effect` `/handler` を持たないのに開きに数えて
  いた。見出しのあとの式が EOF まで同じ未完の入力に吸い込まれ、評価されない。
- `/module` `/implement` の直後のキーワードも開きに数えていたので、閉じたブロックが
  釣り合わなかった。

### 意図

§12.1 の閉じの規則どおりに開きと閉じを数える。閉じは `/fn` `/module` `/implement`
だけである。`/if` は `if` の閉じであって `fn` の閉じではない。`effect` と `handler` は
次の見出しかファイル末尾で終わるので計数しない。閉じの直後のキーワードは開きに数えない。

### 仮説

- `/if` を閉じから外し、閉じの直後の `fn`・`module`・`implement` を開きに数えなければ、
  `fn` ブロックは `/fn` まで 1 つの入力になる。
- `effect` と `handler` を計数から外すと見出しの行で入力が完結する。段階 1 は定義の
  入力を受けないので 1 行の断りが出たあと、次の式が評価される。
- 変更は入力の区切りだけに効き、式の評価と型検査は変えない。

### 判定器

`scripts/verify-repl-if-input-terminator.sh`（`# tier: quick`、
`# round: repl-if-input-terminator`、6 検査）:

- a. 関数の中の `/if` は `fn` の閉じではない。`f 1` と `f 0` が `1` / `z` / `0` を出す。
- b. `/fn` は開いた `fn` を閉じる。効果付きの関数を `as` で束縛し、2 回呼べる。
- c. `handler` 見出しは閉じを待たない。次の `1` が評価される。
- d. `effect` 見出しも閉じを待たない。次の `1` が評価される。
- e. `module` は `/module` で閉じる。閉じたあとの `1` が評価される。
- f. `implement` は `/implement` で閉じる。閉じたあとの `1` が評価される。

単体テストは `crates/hado/src/repl.rs` の `if_close_does_not_finish_an_open_function`・
`effect_and_handler_headings_do_not_wait_for_a_close`・
`module_and_implement_blocks_complete_at_their_close` が持つ。結合テストは
`crates/hado/tests/repl.rs` の 3 本が持つ。

### スコープ

触る: `crates/hado/src/repl.rs`、`crates/hado/tests/repl.rs`、
`scripts/verify-repl-if-input-terminator.sh`（新設）、`docs/rounds/repl.md`（決定 8）、
`docs/rounds/repl-if-input-terminator.md`。

触らない: `docs/spec.md`、`tests/accept`・`tests/reject`・`stdlib/`（この修正はコーパスを
変えない）。

### 工程

1. main で現象を再現し、E100 と E204・`1` が評価されないことを見る。
2. テストと門番を先に置き、main の木で赤を測る。
3. `block_balance` を閉じの規則どおりに直す。
4. main を取り込み、corpus lock の差分を読んでから作り直す。
5. 門番・quick 層を回して PR にする。
6. 着地後に main で full 層を実測し、このファイルに足す。

### 許容される赤

- なし。

### スコープ外

- 最上位で `/if` を必要とする `if`（`if true => log "a" -> 1 | 2` の次行に `/if`）の
  完結判定。`if` は腕に `->` を含むときだけ `/if` を要するので、`if` を開きに数えるだけ
  では通常の `if` を未完にしてしまう。issue #111 の Fix 節は閉じの計数のみを定めており、
  この形は変えない。fix 後も 1 行目で E100 になり、`/if` は別の入力として扱われる。
- 定義の入力（`def`・`module`・`effect` など）を受け付けること。段階 2 の
  `docs/issues/repl-stage2-shell-library.md` が持つ。

### 中断の条件

- 閉じの計数を直すと式の評価が変わると分かった。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §11 | 1 入力が 1 継続の歩である | 入力の区切り | `input_complete` | `crates/hado/tests/repl.rs` の `repl_keeps_a_function_open_across_an_if_close` ほか |
| §12.1 | `fn` は本体に `->` があれば `/fn`、`module` は `/module`、`implement` は `/implement` で閉じる。`/if` は `if` の閉じ | 字句・構文 | `block_balance` | `scripts/verify-repl-if-input-terminator.sh` 検査 a〜f、単体テスト 3 本 |

## 実測

main は `7ee9772`。作業は `.worktrees/repl-if-input-terminator` の
`repl-if-input-terminator` で行った。

### 現象（main）

issue の入力は `/if` で割れ、関数を束縛できない。

```
$ printf 'fn x =>\n  if x == 0 => log "z" -> 0 | x\n/if\n/fn as f ->\nf 1\nf 0\n' | ./target/debug/hado repl 2>&1; echo "exit=$?"
main.hd:1:12: error[E100]: `fn` is not closed; close it with `/fn`; canonical form: fn x =>     if x == 0 => log "z" ->     0 | x     /if   /fn
main.hd:1:12: error[E100]: expected an expression, found /
main.hd:1:12: error[E204]: unbound name f
main.hd:1:12: error[E204]: unbound name f
exit=0
```

`handler` と `effect` の見出しは、あとの式を EOF まで吸い込む。

```
$ printf 'handler Error as Catch\n1\n' | ./target/debug/hado repl 2>&1; echo "exit=$?"
repl: 定義の入力はまだ使えない（式と `e as x ->` だけを受ける）
exit=0
$ printf 'effect E\n1\n' | ./target/debug/hado repl 2>&1; echo "exit=$?"
repl: 定義の入力はまだ使えない（式と `e as x ->` だけを受ける）
exit=0
```

### 修正後

issue の入力が `1` / `z` / `0` を出す。

```
$ printf 'fn x =>\n  if x == 0 => log "z" -> 0 | x\n/if\n/fn as f ->\nf 1\nf 0\n' | ./target/debug/hado repl; echo "exit=$?"
1
z
0
exit=0
```

見出しの次の式が評価される（見出しは定義の入力として 1 行断る）。

```
$ printf 'handler Error as Catch\n1\n' | ./target/debug/hado repl 2>&1; echo "exit=$?"
repl: 定義の入力はまだ使えない（式と `e as x ->` だけを受ける）
1
exit=0
$ printf 'effect E\n1\n' | ./target/debug/hado repl 2>&1; echo "exit=$?"
repl: 定義の入力はまだ使えない（式と `e as x ->` だけを受ける）
1
exit=0
```

### テスト

main の木では、新しい単体テスト 3 本と結合テスト 3 本が赤だった。

```
$ cargo test --offline --lib repl::
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 45 filtered out
$ cargo test --offline --test command 'repl::repl_keeps_a_function_open_across_an_if_close'
left: ""
right: "1\nz\n0\n"
```

修正後は単体と結合が緑である。

```
$ cargo test --offline --lib repl:: 2>&1 | tail -1
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 45 filtered out
$ cargo test --offline --test command repl:: 2>&1 | tail -1
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 374 filtered out
```

### 門番

main の木では 0/6、修正後は 6/6。main の測定は、門番を main の `scripts/` に一時的に
置いて走らせ、終わったら消した（`git status --short` は空）。

```
$ bash scripts/verify-repl-if-input-terminator.sh
OK: a: /if does not cut an open fn
OK: b: /fn closes an open fn
OK: c: a handler heading does not swallow the next input
OK: d: an effect heading does not swallow the next input
OK: e: /module closes a module block
OK: f: /implement closes an implement block
PASSED: 6/6
```

既存の REPL の門番も緑のままである。

```
$ bash scripts/verify-repl.sh | tail -1
PASSED: 11/11
```

### lint

認知複雑度 7 の門番に収めるため、判定を `closes_block`・`is_block_close`・
`opens_block` へ切り出した。

```
$ cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity 2>&1 | tail -1
    Finished `dev` profile [optimized + debuginfo] target(s) in 0.06s
$ cargo fmt --all --check; echo "exit=$?"
exit=0
```

### テストの全走

```
$ cargo test --offline 2>&1 | grep 'test result:'
test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 395 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 425 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### corpus lock

このラウンドはコーパスを変えない。main `7ee9772` を取り込んだ後の照合は差分なし。

```
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
```

### 層別（quick）

```
$ bash scripts/run-gates.sh --jobs 2 --tier quick target/.gates-round3 | tail -1
TALLY: 31/31 passed; red: none
```

`lint-docs.sh` は quick 層の中で緑である。

```
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
```