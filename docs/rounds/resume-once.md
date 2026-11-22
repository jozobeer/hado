# `resume` の 1 回性（resume-once）

## 手順書

### 背景

#114 の現象。§38 は `resume` が捕まえた継続を 1 回だけ再開すると定めるが、
`resume` の実体は `continuation_calling_convert` が作る普通の閉包だった。同じ
`again` の腕で 2 回目を呼ぶと、`with` の続きがもう一度走る。main `e716721` では
次の入力が `after` を 2 回印字し、`main` の Int 3 が終了コード 3 になる。

```hado
effect Choice
again pick: Int
handler Choice as C
again pick =>
  resume 1 as a ->
  resume 2 as b ->
  a + b
return value => value
def main =
  with C ->
  pick as n ->
  log "after" ->
  n
```

2 回目の `resume 2` が同じ継続を再実行するので、`pick` の後ろの `log` が 2 回
起き、2 つの resumed 値 1 と 2 が足される。

### 意図

継続を包む関数に、ビルド時に 1 語のセルを割り当てる。入口で印を見て、未使用なら印を
付け、使用済みなら継続を呼ばず `resume was already used` を stderr に出して終了コード 1
で止める。セルは `again` のたびに作るので、2 回目以降の `again` の `resume` は
それぞれ 1 回は再開できる。`if` の片側だけにある `resume` も同じ 1 回性を持つ。

§38 の「1 回だけ再開する」を、呼び出し側の慣習ではなく継続の入口で強制する。2 回目を
止める手段は仕様が名指ししていないので、§37 の `abort` が handler 無しで `main` に
達したときと同じ形（stderr の 1 行と終了コード 1、§54）に合わせた。REPL では同じ経路が
段を捨てて入力列へ戻る（`docs/rounds/repl.md`）。

### 仮説

- 継続の入口にセルを 1 つ置けば、同じ腕の 2 回目だけを止められる。
- セルを `again` ごとに割り当てれば、続く `again` の `resume` は止まらない。
- `if` の片側だけにある `resume` は 1 回しか通らないので、セルの導入で変わらない。
- セルは runtime が返す語なので、既存の ABI の `Bytes` で運べる。

### 判定器

`scripts/verify-resume-once.sh`（新設、`# tier: quick`、`# round: resume-once`）:

- a. 同じ腕の 2 回目は継続を再実行しない。`after` は 1 回、stderr は
  `resume was already used`、終了コード 1。
- b. 1 回だけの `resume` は値 7 を返す（対照）。
- c. `if` の片側だけの `resume` は値 7 を返す（対照）。
- d. `again` が 2 回起きれば、それぞれの継続が 1 回走って 3 になる（対照。セルを
  共有すると 2 回目が止まる）。

結合テストは `crates/hado/tests/r26_effects.rs` の
`second_resume_does_not_rerun_the_continuation`・`one_resume_runs_the_continuation_once`・
`resume_in_only_one_arm_still_runs_once` が持つ。

### スコープ

触る: `crates/hado/src/mono.rs`、`crates/hado/src/runtime_abi.rs`、
`runtime/hado_runtime.c`、`crates/hado/tests/r26_effects.rs`、
`scripts/verify-resume-once.sh`（新設）、`docs/rounds/resume-once.md`。

触らない: `docs/spec.md`（§38 の解釈は変えない）、
`tests/accept`・`tests/reject`・`stdlib/`（この修正はコーパスを変えない）。

### 工程

1. main で 2 回目の `resume` が継続を再実行することを再現する。
2. 再現テストを先に置き、main の木で赤を測る。
3. 継続を包む関数の入口にセルを置き、2 回目を止める。
4. 専用の門番を置き、main の木で赤、修正後に緑になることを測る。
5. main を取り込み、corpus lock の差分を読んでから照合する。
6. 門番・quick 層・full テストを回して PR にする。
7. 着地後に main で full 層を実測してこのファイルに足し、#114 を superseded で閉じる。

### 許容される赤

- なし。

### スコープ外

- `resume` を値として保存・返す形。§38 が構文的な入れ子だけで場所を決める。
- 3 回目以降の区別。2 回目で止まるので観測できない。
- メッセージ本文の仕様化。§38 は 1 回性だけを定め、文面は名指ししない。
- 継続を 1 回も再開しないパス（`abort` の腕など）。セルは使われないまま捨てられる。

### 中断の条件

- セルの導入で、正当な 1 回目の `resume` が止まる。
- 2 回目の `again` の `resume` が止まる（セルの寿命が短すぎる）。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §38 | `resume` は捕まえた継続を 1 回だけ再開する。腕の中の任意の位置に置ける | lower | `continuation_calling_convert` の `OneshotEnter` | 門番 a・b・c、`r26_effects` の 3 本 |
| §37 | `again` は節の中のどこでも 1 回。第53節の保証の外 | 実行 | `hado_oneshot_new`・`hado_oneshot_enter` | 門番 d |
| §54 | raise・abort が `main` に達すれば stderr と終了コード 1 | 実行 | `hado_abort_error` 経由の停止 | 門番 a の stderr と終了コード |

## 実測

main は `e716721`。作業は `.worktrees/resume-once` の `resume-once` で行った。

### 現象（main）

```
$ ./target/debug/hado run .repro114/main.hd >out 2>err; echo "exit=$?"; echo "stdout: $(cat out)"; echo "stderr: $(cat err)"
exit=3
stdout: after
after
stderr: 
```

### テスト先行（`8b0171a`）

修正前の木で新しい結合テストは赤。

```
$ cargo test --offline --test command r26_effects::second_resume_does_not_rerun_the_continuation
thread 'r26_effects::second_resume_does_not_rerun_the_continuation' panicked at crates/hado/tests/r26_effects.rs:2314:5:
assertion `left == right` failed: Output { status: ExitStatus(unix_wait_status(0)), stdout: "after\nafter\n3\n", stderr: "" }
  left: Some(0)
 right: Some(1)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 398 filtered out
```

### 修正後

```
$ ./target/debug/hado run .repro114/main.hd >out 2>err; echo "exit=$?"; echo "stdout: $(cat out)"; echo "stderr: $(cat err)"
exit=1
stdout: after
stderr: resume was already used
```

### 門番（`5db2d41`）

main の木（3 つの実装ファイルを `git checkout main` で戻す）では検査 a だけが赤。

```
$ bash scripts/verify-resume-once.sh
NG: a: a second resume does not rerun the continuation
  status=3
OK: b: one resume runs the continuation once
OK: c: a resume in one if arm still runs once
OK: d: each again gets its own one-shot cell
PASSED: 3/4
```

修正後は 4/4。

```
$ bash scripts/verify-resume-once.sh
OK: a: a second resume does not rerun the continuation
OK: b: one resume runs the continuation once
OK: c: a resume in one if arm still runs once
OK: d: each again gets its own one-shot cell
PASSED: 4/4
```

### テスト

```
$ cargo test --offline --test command r26_effects::
test result: ok. 95 passed; 0 failed; 0 ignored; 0 measured; 304 filtered out
```

```
$ cargo test --offline
test result: ok. 53 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 399 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 426 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### lint

```
$ cargo fmt --all --check; echo "exit=$?"
exit=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [optimized + debuginfo] target(s) in 18.81s
$ cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity 2>&1 | tail -1
    Finished `dev` profile [optimized + debuginfo] target(s) in 3.57s
```

### corpus lock

このラウンドはコーパスを変えない。main `e716721` を取り込んだ後の照合は差分なし。

```
$ git merge main
Already up to date.
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
```

### 層別（quick）

```
$ bash scripts/run-gates.sh --jobs 2 --tier quick target/.gates-round6 2>&1 | tail -1
TALLY: 42/42 passed; red: none
```
