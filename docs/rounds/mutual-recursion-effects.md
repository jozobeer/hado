# 相互再帰の相手の効果を行に残す（mutual-recursion-effects）

## 手順書

### 背景

#116 の現象。相互再帰する群で、効果を出すのが片方の定義だけだと、もう片方の
効果の行にその効果が届かない。main `3f63b32` では次の入力の `check` が終了 0 で
通り、`run` が `lowered return type differs from inferred signature for file0$h` で
終了 1 になる。

```hado
handler Error as Catch
abort raise _ => 42
return value => value
def rec h n => if n == 0 => g n | h (n - 1)
def rec g n => if n == 0 => 1 / n | h (n - 1)
def main = with Catch -> h 1
```

`h` は `Fn(Int => Int {A})`、`g` は `Fn(Int => Int {Error | A})` と推論され、`h` の
行から `Error` が落ちる。ユーザー効果でも同じ穴が空く。`g` だけが `ask` を出す
相互再帰を `main` から呼ぶと `check` が通り、実行時に `unhandled operation ask`
になる。

### 意図

群の推論が終わってから、群の内側の再帰呼び出しをたどり、呼ばれる定義の行にある
具体的な効果のラベルだけを呼び手の行に足す。行の末尾（効果変数）は共有しないので、
`R = {Error | R}` の出現検査は戻らない。書いた閉じた行に入らないラベルは E305。
入れ子の `fn` の中の呼び出しはその閉包の行に残し、定義自身の行には足さない。

### 仮説

- 群の終わりに呼び出しの辺をたどってラベルを固定点まで伝播すれば、`h` の行に `g` の
  `Error` と `Ask` が届く。
- 行の末尾は各定義のままにするので、自己再帰の割り算と書いた `{Error}` の行は
  今までどおり動く。
- `Pure` と具体行は閉じているので、伝播したラベルが入らないときは E305 で止まる。

### 判定器

`scripts/verify-mutual-recursion-effects.sh`（新設、`# tier: quick`、
`# round: mutual-recursion-effects`）:

- a. 相互再帰の割り算の `Error` が相手の行に届き、handler が捕まえて 42 になる。
- b. 相手だけが出すユーザー効果は main まで届き、handler なしでは E706。
- c. 同じ呼び出しを handler の下で行うと `Ask` は main に残らず、腕の 3 が返る。
- d. 書いた `Pure` の行は広がらず、E305 のまま（対照）。
- e. 自己再帰の割り算は今までどおり 42 になる（対照）。
- f. 書いた `{Error}` の行は今までどおり 42 になる（対照）。

結合テストは `crates/hado/tests/r26_effects.rs` の `mutual_recursion_*` が持つ。

### スコープ

触る: `crates/hado/src/infer.rs`、`crates/hado/src/types.rs`、
`crates/hado/tests/r26_effects.rs`、`scripts/verify-mutual-recursion-effects.sh`（新設）、
`docs/rounds/mutual-recursion-effects.md`。

触らない: `docs/spec.md`（§20・§15・§10 の解釈は変えない）、
`tests/accept`・`tests/reject`・`stdlib/`（この修正はコーパスを変えない）。

### 工程

1. main で割り算が `lowered return type differs`、ユーザー効果が
   `unhandled operation ask` になることを再現する。
2. 再現テストを先に置き、main の実装で赤を測る（`da3db15`）。
3. 群の終わりに具体的な効果だけを伝播する（`85c5859`）。
4. 専用の門番を置き、main の実装で赤、修正後に緑になることを測る。
5. main を取り込み、corpus lock の差分を読んでから照合する。
6. 門番・quick 層・full テストを回して PR にする。
7. 着地後に main で門番と層別を実測してこのファイルに足し、#116 を閉じる。

### 許容される赤

- なし。

### スコープ外

- 行の末尾（効果変数）の同一化。伝播するのは具体ラベルのみで、効果多相の行は
  各定義のまま残す。
- 群をまたぐ伝播。呼び出し側の通常の単一化が既に相手の行を運ぶ。
- 推論された行の最小化。省略した行が開いたままなのは r61 の既存の振る舞いである。

### 中断の条件

- 自己再帰・書いた `{Error}` の行が赤になる（門番 e・f）。
- `r26_effects` の他のテストが赤になる。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §20 | 書いた行はその通りで、省略した行は推論して全称量化する | 型 | `propagate_mutual_effects`・`install_effect_labels` | 門番 a・d・f |
| §15 | 型が食い違えば E305 | 型 | `closed_row_rejects` | 門番 d |
| §10 | `main` に残ってよいのは prelude の効果だけ | 単相化 | 推論された行が main まで届く経路 | 門番 b・c |

## 実測

main は `3f63b32`。作業は `.worktrees/mutual-recursion-effects` の
`mutual-recursion-effects` で行った。

### 現象（main）

```
$ ./target/debug/hado check .repro116/div/main.hd; echo "check exit=$?"
check exit=0
$ ./target/debug/hado run .repro116/div/main.hd; echo "run exit=$?"
lowered return type differs from inferred signature for file0$h
run exit=1
```

```
$ ./target/debug/hado check .repro116/ask/main.hd; echo "check exit=$?"
check exit=0
$ ./target/debug/hado run .repro116/ask/main.hd; echo "run exit=$?"
unhandled operation ask
run exit=1
```

### テスト先行（`da3db15`）

main の実装で新しい結合テストは 4 本とも赤。

```
$ cargo test --offline --test command r26_effects::mutual_recursion
test result: FAILED. 2 passed; 4 failed; 0 ignored; 0 measured; 398 filtered out
---- r26_effects::mutual_recursion_keeps_the_other_functions_effect stdout ----
assertion `left == right` failed: lowered return type differs from inferred signature for file0$h
  left: Some(1)
 right: Some(0)
---- r26_effects::mutual_recursion_of_a_user_effect_reaches_main stdout ----
  left: Some(0)
 right: Some(1)
---- r26_effects::mutual_recursion_of_a_user_effect_is_handled stdout ----
assertion `left == right` failed: main.hd:8:1: error[E706]: main cannot have a user-defined effect
  left: Some(1)
 right: Some(0)
```

### 修正後（`85c5859`）

```
$ cargo test --offline --test command r26_effects::mutual_recursion
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 398 filtered out
```

```
$ ./target/debug/hado run .repro116/div/main.hd; echo "run exit=$?"
run exit=42
$ ./target/debug/hado check .repro116/ask/main.hd; echo "check exit=$?"
main.hd:5:1: error[E706]: main cannot have a user-defined effect
check exit=1
$ ./target/debug/hado run .repro116/handled/main.hd; echo "run exit=$?"
run exit=3
$ ./target/debug/hado check .repro116/pure/main.hd; echo "check exit=$?"
main.hd:1:1: error[E305]: effect row mismatch
check exit=1
$ ./target/debug/hado run .repro116/selfrec/main.hd; echo "run exit=$?"
run exit=42
$ ./target/debug/hado run .repro116/annot/main.hd; echo "run exit=$?"
run exit=42
```

### 門番

main の実装（`crates/hado/src/infer.rs` と `types.rs` を `main` に戻す）では
検査 a〜d が赤。

```
$ bash scripts/verify-mutual-recursion-effects.sh
NG: a: the other function keeps the division error
NG: b: the other function carries a user effect to main
NG: c: a handled user effect from the other function returns its arm
NG: d: a written Pure row is not widened
OK: e: self recursion still catches the division error
OK: f: a written Error row still catches the division error
PASSED: 2/6
```

修正後は 6/6。

```
$ bash scripts/verify-mutual-recursion-effects.sh
OK: a: the other function keeps the division error
OK: b: the other function carries a user effect to main
OK: c: a handled user effect from the other function returns its arm
OK: d: a written Pure row is not widened
OK: e: self recursion still catches the division error
OK: f: a written Error row still catches the division error
PASSED: 6/6
```

### テスト

```
$ cargo test --offline --test command r26_effects::
test result: ok. 100 passed; 0 failed; 0 ignored; 0 measured; 304 filtered out
$ cargo test --offline --test compiler check_recursion::
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 422 filtered out
```

`scripts/verify-r61.sh` の 61-a（省略した行の再帰生成器が 5 を印字する）は、この
修正の後も緑。

```
$ bash scripts/verify-r61.sh
OK: 61-a the reproduction from bug.md
OK: 61-b recursion, and a mutual group
OK: 61-c a written row still constrains
OK: 61-d an omitted row still reaches the caller
OK: 61-e E307 stays
PASSED: 5/5
```

### 層別と lint

```
$ bash scripts/run-gates.sh --tier quick /tmp/gates-quick-mutual
TALLY: 52/52 passed; red: none
$ cargo fmt --all --check; echo "fmt exit=$?"
fmt exit=0
$ cargo clippy --offline --all-targets -- -D warnings; echo "clippy exit=$?"
clippy exit=0
$ cargo test --offline
...
test result: ok. 426 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
