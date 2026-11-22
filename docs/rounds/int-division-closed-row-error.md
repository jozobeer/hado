# 整数除算の Error が閉じた効果行から落ちる（int-division-closed-row-error）

## 手順書

### 背景

#112 の現象。`def f x y: Int Pure => x / y` を `hado check` が終了 0 で通す。§19 は
`/` と `%` を `Int {Error}` とし、§20 は書いた行をその通りとする（`{Io}` と書いた
関数が他の効果も持てば E305）。閉じた行に `Error` が無いのに本体が `raise` するので、
署名と本体が食い違う。

原因は `default_division`。被演算子がまだ型変数の間は `Error` を足すのを後回しに
する（`divisions` に積み、`default_numbers` で後から処理する）。注釈の閉じた行は
その前に確定するので、`add_error_effect` が `Row::Closed` を見て何もせず戻っていた。

### 意図

整数除算として確定したあと、閉じた行に `Error` が無ければ E305 にする。開いた行には
今までどおり `Error` を足す。結果型の不一致（`Float` の被演算子を `Int` に既定して
`cannot unify A with Int` にする場合）は効果行の不一致より先に報告する。

### 仮説

- `add_error_effect` が閉じた行で E305 を返せば、`check` は `Pure`・`{Io}`・`%` を
  拒否する。
- 開いた行（行を書かない形）と `{Error}` は今までどおり通り、走る。
- 効果を足す順を結果型の統一の後ろへ移しても、`Float` の診断は変わらない。

### 判定器

`scripts/verify-int-division-closed-row-error.sh`（`# tier: quick`、
`# round: int-division-closed-row-error`、6 検査）:

- a. `Int Pure` の `/` は E305。
- b. `Int Pure` の `%` も E305。
- c. `Int {Io}` の `/` も E305（行に無い効果）。
- d. 被演算子に注釈を付けた `(x: Int, y: Int): Int Pure` も E305。
- e. `Int {Error}` は通り、`f 8 2` は 4、`f 1 0` は DivisionByZero を出す。
- f. 行を書かない形は `Error` を推論する（`--print-type` が `{Error}` を出す）。

単体テストは `crates/hado/tests/check_functions.rs` の
`integer_division_of_unresolved_operands_keeps_error_in_a_closed_row` が持つ。

### スコープ

触る: `crates/hado/src/infer.rs`、`crates/hado/tests/check_functions.rs`、
`scripts/verify-int-division-closed-row-error.sh`（新設）、
`docs/rounds/int-division-closed-row-error.md`。

触らない: `docs/spec.md`、`tests/accept`・`tests/reject`・`stdlib/`（この修正は
コーパスを変えない）。

### 工程

1. main で現象を再現する（`check` が 0、`--print-type` が `Error` を落とす）。
2. テストを先に置き、main の `infer.rs` で赤を測る。
3. 閉じた行に `Error` が無ければ E305 にする。
4. 門番を置き、main の木で赤を測る。
5. main を取り込み、corpus lock の差分を読んでから作り直す。
6. 門番・quick 層を回して PR にする。
7. 着地後に main で full 層を実測し、このファイルに足す。

### 許容される赤

- なし。

### スコープ外

- PR #112 の Validation にある `lowered return type differs from inferred signature`
  は現行 main では再現しなかった。`check` の受理（と `--print-type` の `Error` の
  欠落）が現行 main の赤である。単相化の食い違いは、署名と本体が食い違ったまま
  下ろせる形に限られる。このラウンドは検査段階の受理を直す。
- 効果行そのものの検査（E522 の重複、効果変数の実体化）は他節の担当で、変えない。

### 中断の条件

- E305 を足すと、仕様が受理する形（開いた行・`{Error}`）まで拒否される。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §19 | `/` と `%` は `Int {Error}` を返す。除数 0 は `raise` する | 型検査（効果行） | `default_division`・`add_error_effect` | `scripts/verify-int-division-closed-row-error.sh` 検査 a〜c・e、`crates/hado/tests/check_functions.rs` の 1 本 |
| §20 | 書いた行はその通り。`{Io}` と書いた関数が他の効果も持てば E305 | 型検査（効果行） | `add_error_effect`・`require_error_label` | 同 検査 c・d・f |

## 実測

main は `c6d8faa`。作業は `.worktrees/int-division-closed-row-error` の
`int-division-closed-row-error` で行った。

### 現象（main）

`Pure` と `{Io}` と `%` を `check` が受理する。`--print-type` は `Error` を落とす。

```
$ hado check pure/main.hd; echo "exit=$?"
exit=0
$ hado check --print-type=f pure/main.hd; echo "exit=$?"
Fn(Int, Int => Int)
exit=0
$ hado check io/main.hd; echo "exit=$?"
exit=0
$ hado check mod/main.hd; echo "exit=$?"
exit=0
```

被演算子に注釈を付けた形はもともと止まる（比較の対照）。

```
$ hado check annot/main.hd; echo "exit=$?"
main.hd:1:1: error[E305]: effect row mismatch
exit=1
```

### 修正後

```
$ hado check pure/main.hd; echo "exit=$?"
main.hd:1:24: error[E305]: effect row mismatch
exit=1
$ hado check io/main.hd; echo "exit=$?"
main.hd:1:24: error[E305]: effect row mismatch
exit=1
$ hado check mod/main.hd; echo "exit=$?"
main.hd:1:24: error[E305]: effect row mismatch
exit=1
```

`{Error}` と行を書かない形は通る。

```
$ hado run err/main.hd; echo "exit=$?"
exit=4
$ hado check --print-type=f open/main.hd; echo "exit=$?"
Fn(Int, Int => Int {Error})
exit=0
```

### テスト

main の `infer.rs` に戻すと新しいテストは赤になる。

```
$ cargo test --offline --test compiler check_functions::integer_division_of_unresolved_operands_keeps_error_in_a_closed_row
---- check_functions::integer_division_of_unresolved_operands_keeps_error_in_a_closed_row stdout ----
accepted:
def f x y: Int Pure => x / y
def main = f 1 1
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 425 filtered out
```

修正後は緑。

```
$ cargo test --offline --test compiler check_functions::integer_division_of_unresolved_operands_keeps_error_in_a_closed_row
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 425 filtered out
```

### 門番

main の木では 3/6（a・b・c が赤）、修正後は 6/6。main の測定は、門番を main の
`scripts/` に一時的に置いて走らせ、終わったら消した（`git status --short` は空）。

```
$ bash scripts/verify-int-division-closed-row-error.sh
NG: a: Pure division is E305
NG: b: Pure modulo is E305
NG: c: a closed row without Error is E305
OK: d: annotated operands are E305
OK: e: an {Error} row runs the division
OK: f: an open row infers Error
PASSED: 3/6
```

```
$ bash scripts/verify-int-division-closed-row-error.sh
OK: a: Pure division is E305
OK: b: Pure modulo is E305
OK: c: a closed row without Error is E305
OK: d: annotated operands are E305
OK: e: an {Error} row runs the division
OK: f: an open row infers Error
PASSED: 6/6
```

### lint

```
$ cargo fmt --all --check; echo "exit=$?"
exit=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [optimized + debuginfo] target(s) in 19.01s
$ cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity 2>&1 | tail -1
    Finished `dev` profile [optimized + debuginfo] target(s) in 3.51s
```

### テストの全走

```
$ cargo test --offline 2>&1 | grep "test result:"
test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 395 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 426 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
