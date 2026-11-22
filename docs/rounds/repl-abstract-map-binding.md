# REPL で抽象型の写像を束縛する（repl-abstract-map-binding）

## 手順書

### 背景

#109 の現象。`hado repl` は 1 歩を `def main = <式>` か `def hado_step (...) => <式>` / `def main = hado_step` に合成する。後者は必ず E702（`main` は関数を含む値を返せない）になるので `is_repl_artifact` が落としていたが、`Map.T` は公開型が抽象型なので前者でも E705 になり、入力が捨てられて次の入力の `m` が E204 になっていた。

E705 を落としても、JIT が AOT と同じ `for_codegen` を通さないと `Map.T` は `unknown data type` になる。印字を開いた型のまま行うと `Map.MapNode` が漏れる。`hado check` の E705 は §10 の `main` の規則であり、そのまま拒否する。

### 意図

List・Option・Array・Bytes と同じように、prelude の写像も REPL で束縛して次の入力から使えるようにする。写像そのものは抽象型なので印字は断るが、セッションは続ける。

### 仮説

- 合成した `main` の E705 は E702 と同じ REPL の産物である。両方を落とせば束縛と使用が緑になる。
- JIT を `Checked::for_codegen` の経路に揃えれば、抽象型は実装の表現に開かれて機械語の型が取れる。
- 印字はソースの型に抽象型が残っているかで断れる。開いた型で印字しないので `Map.MapNode` は漏れない。
- `hado check` は合成しないので、E705 を落とす変更の影響を受けない。

### 判定器

`scripts/verify-repl-abstract-map-binding.sh`（`# tier: quick`、`# round: repl-abstract-map-binding`）:

- 検査 1: 束縛した写像を `Map.find` と `Map.to_list` で次の入力から使える。
- 検査 2: 写像そのものの印字は断り、`MapNode`・`MapLeaf` を漏らさず、次の入力 `7` を評価する。
- 検査 3: `hado check` は `def main = Map.insert Map.empty 1 "a"` を E705 のまま拒否する。

既存の `scripts/verify-repl.sh` は REPL 全体の門番として残し、この現象の検査は専用の門番へ分ける。単体テストは `crates/hado/tests/repl.rs` の `repl_binds_an_abstract_map` と `repl_continues_when_an_abstract_map_cannot_be_printed` が持つ。

### スコープ

触る: `crates/hado/src/repl.rs`、`crates/hado/src/project.rs`、`crates/hado/tests/repl.rs`、`scripts/verify-repl-abstract-map-binding.sh`（新設）、`docs/rounds/repl.md`（決定 3）、`docs/rounds/repl-abstract-map-binding.md`。

触らない: `docs/spec.md`（§10 と §47 の解釈は変えない）、`tests/accept`・`tests/reject`・`stdlib/`（この修正はコーパスを変えない）。

### 工程

1. main で現象を再現し、E705 と E204 を見る。
2. 既存 PR のテストと修正を取り込み、main を merge する。
3. 専用の門番を書き、main の再現が赤から緑になることを確かめる。
4. 単体テスト・門番・quick 層を回す。
5. `corpus lock` の差分を読み、差分が無いことを確かめる。
6. PR を着地し、main の full 層を実測してこのファイルに足す。

### 許容される赤

- なし。

### スコープ外

- `hado check` の E705 を緩めること。§10 の `main` の規則は実行ファイルの成果物の規則で、REPL の合成した `main` だけが産物である。
- `Map` の印字を実装すること。仕様は抽象型を外から不透明と定めており、実装の constructor を見せない。

### 中断の条件

- E705 を落とすと `hado check` の拒否も緩むと分かった。
- 印字を断っても `Map.MapNode` が漏れると分かった。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §10 | `main` の型は単相で、`shared` で解けない抽象型を含まない | 型 | E705 | `crates/hado/tests/repl.rs` の `repl_binds_an_abstract_map`、`scripts/verify-repl-abstract-map-binding.sh` 検査 3 |
| §47 | 抽象型 `Y.T` の実体化は外から不透明で、実装の constructor は見えない | 型 | — | `scripts/verify-repl-abstract-map-binding.sh` 検査 2 |
| §54 | `main` の値の印字規則 | 印字 | — | `scripts/verify-repl-abstract-map-binding.sh` 検査 2 |

## 実測

main は `d2040a7`。作業は `.worktrees/repl-abstract-map-binding` の `repl-abstract-map-binding` で行った。

### 現象（main）

写像を束縛した次の入力が E204 になる。

```
$ printf 'Map.insert Map.empty 1 "a" as m ->\nMap.find m 1\n' | target/debug/hado repl; echo "exit=$?"
main.hd:1:1: error[E705]: main cannot return a value containing an abstract type
main.hd:1:21: error[E204]: unbound name m
exit=0
```

### 修正後

同じ入力が `Some("a")` を返す。

```
$ printf 'Map.insert Map.empty 1 "a" as m ->\nMap.find m 1\n' | ./target/debug/hado repl; echo "exit=$?"
Some("a")
exit=0
```

写像そのものの印字は断り、`MapNode` を漏らさず、次の入力を評価する。

```
$ printf 'Map.insert Map.empty 1 "a" as m ->\nm\n7\n' | ./target/debug/hado repl 2>&1
7
この入力の値は印字できない: abstract type
```

`hado check` の E705 は残る。

```
$ printf 'def main = Map.insert Map.empty 1 "a"\n' >/tmp/repro109.hd
$ ./target/debug/hado check /tmp/repro109.hd; echo "exit=$?"
repro109.hd:1:1: error[E705]: main cannot return a value containing an abstract type
exit=1
```

### 単体テスト

```
$ cargo test --offline --test command repl:: 2>&1 | tail -3
test repl::repl_does_not_pass_a_binding_that_only_shares_a_token_spelling ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 374 filtered out; finished in 0.52s
```

### 門番

```
$ bash scripts/verify-repl-abstract-map-binding.sh
OK: 1 a bound abstract map is usable from the next input
OK: 2 printing an abstract map is refused without leaking its representation
OK: 3 hado check still rejects a main that returns an abstract map
PASSED: 3/3
```

```
$ bash scripts/verify-repl.sh | tail -2
OK: 11 a later input may reuse a binding name as a parameter or field
PASSED: 11/11
```

### corpus lock

このラウンドはコーパスを変えない。main を merge した後の照合は差分なし。

```
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
```

### 層別（quick）

```
$ bash scripts/run-gates.sh --jobs 2 --tier quick target/.gates-round1 | tail -1
TALLY: 20/20 passed; red: none
```
### 着地後（main `e7cf79a`）

CI の full 層（走行 44968781749、`e7cf79a`）は成功した。新しい門番は `gates:3/5` で走った。

```
$ gh run view 44968781749 --json status,conclusion,headSha
{"conclusion":"success","headSha":"e7cf79aaec0667d2aa1a28ec492db088de066765","status":"completed"}
```

```
$ gh run view 44968781749 --log 2>&1 | grep -E 'TALLY|RUN: verify-repl-abstract' | sed -E 's/\tUNKNOWN STEP\t[0-9T:.-]+Z / /' | sort
gates (gates:1/5) TALLY: 523/523 passed; red: none
gates (gates:2/5) TALLY: 164/164 passed; red: none
gates (gates:3/5) RUN: verify-repl-abstract-map-binding -> /home/runner/work/hado/hado/target/ci-gates/verify-repl-abstract-map-binding.log
gates (gates:3/5) TALLY: 1449/1449 passed; red: none
gates (gates:4/5) TALLY: 952/952 passed; red: none
gates (gates:5/5) TALLY: 593/593 passed; red: none
gates (tests:1/3) TALLY: 5/5 passed; red: none
gates (tests:2/3) TALLY: 5/5 passed; red: none
gates (tests:3/3) TALLY: 5/5 passed; red: none
```
