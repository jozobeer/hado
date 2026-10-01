# 人が認めた `?` の綴りと E508 を戻す（restore-option-question）

## 手順書

### 背景

前のラウンド `do-option-absent`（M7、#69）は決定 D2「不在を効果 `Absent` で表す」を
正しく入れたが、同時に人が認めていない `?` の構文を消した。人は「`as p? ->` は残す」
と言っていた。このラウンドは消した分だけを戻す。

### 決定

1. **人の差分 `git diff 8472497 eeb1704` の 1〜9 を戻す。** D2 の成果（`Absent`・
   `Maybe`・`abort absent`・`e ->` は効果専用）は残す。両者は同居する。`as p? ->` は
   データの `Option` の伝播、`Absent` は効果としての不在を表す。
2. **E508 の本文を直す（審査の指摘）。** 旧い本文は `e ->` を勧めていたが、`e ->` は
   効果専用で `Option` を置くと E303 になる。束縛を使わない形（`Some(_)`）は
   さらに E505 になるので、`as _? ->` を名指しする。
3. **記録を 1 ラウンド 1 ファイルに直す（審査の指摘）。** このラウンドの記録は
   `docs/rounds/do-option-absent.md` の末尾の節ではなく、このファイルに置く。
4. **門番を足す。** E508 の本文が勧める綴りに従った入力が受理されることを測り、
   本文を古い `->` の案内に戻す変異で赤になることを測る。

### 期待値の根拠

- `as p? ->` と E508 の正準形は §34・§36・§57 が定める。正準形は実装の
  `check --fix` の出力から読む（判定器に暗記を持ち込まない）。
- E508 の本文が勧めてよい綴りは、`Option` の伝播を表す `as p? ->` と、束縛を
  使わない形の `as _? ->` だけである（§57）。`e ->` は効果を持つ式の値の破棄で、
  `Option` を置くと E303（§35、§36）。
- 受理・拒否の件数は仕様のエラーコード表の行数とディスクの実数から導く。

### 成果物

`crates/hado/src/{parser,check,infer,mono,fixes}.rs`、`spec/grammar.ebnf`、
`spec/errors.toml`、`docs/spec.md`、`stdlib/option.hd`、
`tests/accept/option_propagation`、`tests/reject/E508`、
`editors/tree-sitter-hado`、`scripts/verify-r57.sh`、`scripts/verify-r71.sh`、
`scripts/verify-do-option-absent.sh`、`scripts/verify-restore-option-question.sh`。

### 工程

1. 赤: `?` が E100、E508 が出ないことを実測する。
2. 緑: 1〜9 と必要だった範囲を戻す。`cargo test --offline` を通す。
3. 直す: E508 の本文と、門番と、記録の形。
4. 全走: `cargo test --offline`、quick / core / full、`lint-docs`、
   `verify-spec-contracts`、locks。

### 判定

`cargo test --offline` が緑、quick / core が緑、full は main と同じ 4 本だけが赤、
`lint-docs`・`verify-spec-contracts`・`verify-restore-option-question` が緑。

### 許容される赤

`verify-r62`・`verify-r63`・`verify-r68`（E402 の期待ずれ）と
`verify-string-split-linear`（検査 i の性能判定）。いずれも main で赤である。

### スコープ外

- E504 と `tests/reject/E504`。`as _? ->` は受理され、書き換えを持たない。
  `e ->` が `Option` を取らないので、E504 の正準形が無い。
- `let p? =`（E100 のまま）。
- `tests/accept/rail_forms`・`tests/accept/user_store` の `?`。
- E508 の本文が `Some(_)` の正準形を出すこと。この書き換えは束縛を使う形だけを
  対象にする（§57）。`_` の形は本文が `as _? ->` を名指しする。

### 中断の条件

`as p? ->` が `Absent` と両立しない、または戻すために `Absent` 側を作り直す必要が
出たら止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §34 | `as p? ->` はデータの `Option` を伝播し、`e ->` は効果だけを実行する | 構文 / 型 | E303 | `tests/accept/option_propagation`・`tests/reject/E303` |
| §36 | `\| Some(p) => body` と `\| None => None` の 2 腕は `as p? ->` へ書き換わる | 冗長 | E508 | `tests/reject/E508`・`scripts/verify-r57.sh`・`scripts/verify-restore-option-question.sh` |
| §34 | `as _? ->` の破棄は受理され、書き換えを持たない（E504 は戻さない） | 冗長 | — | `scripts/verify-r71.sh` |
| §34 | 不在は効果 `Absent` で表し、`Maybe` が `Option` へ戻す | 実行 | — | `tests/accept/absent_effect`・`scripts/verify-do-option-absent.sh` |
| §55 | prelude の効果は `Absent` `Error` `Env` `Io` | 名前解決 | — | `scripts/verify-do-option-absent.sh` |

## 実測

Red（M7 の木。`?` は E100、E508 は出ない）。

```
$ cargo run --offline -q -- check tests/accept/option_propagation/main.hd
main.hd:4:42: error[E100]: `as p? ->` is removed; absence is an effect (bind with `as p ->`)
$ cargo run --offline -q -- check tests/reject/E508/main.hd
（診断を出さない。終了コード 0）
```

Green（このラウンドの木）。

```
$ cargo run --offline -q -- check tests/accept/option_propagation/main.hd
（終了コード 0）
$ cargo run --offline -q -- run tests/accept/option_propagation/main.hd
{ none = None; some = Some(42) }
$ cargo run --offline -q -- check tests/reject/E508/main.hd
main.hd:4:60: error[E508]: absence bound at 4:62 is propagated unchanged; use `as p? ->`, or `as _? ->` when the arm binds nothing; canonical form: find_user id as user? -> Some(user + 1)
```

E508 の本文（審査の指摘で直した）。2 腕の形は束縛を使う形と `Some(_)` の形の 2 つで
測る。本文の案内に従った入力はどちらも受理される。

```
$ bash scripts/verify-restore-option-question.sh
OK: a the E508 message names `as p? ->` / `as _? ->` and both are accepted
OK: b the mutation that makes the message recommend `->` is caught
PASSED: 2/2
```

本文の実測（同じ探針を手で回した出力）。

```
束縛を使う形:    error[E508]: ... use `as p? ->`, or `as _? ->` when the arm binds nothing; canonical form: find id as user? -> Some(user + 1)
`Some(_)` の形:  error[E508]: ... use `as p? ->`, or `as _? ->` when the arm binds nothing
正準形（find id as user? -> Some(user + 1)）: 終了コード 0
`Some(_)` に勧める綴り（find id as _? -> Some(1)）: 終了コード 0
旧い本文が勧めていた綴り（find id -> Some(1)）: error[E303]: `e ->` requires an effect
```

`?` と D2 の同居（`as p? ->` は `Option` の伝播、`e ->` は効果専用、`let p? =` は E100）。

```
$ bash scripts/verify-do-option-absent.sh
PASSED: 4/4
$ bash scripts/verify-r71.sh
PASSED: 4/4
```

文法の変異。`verify-treesitter` の検査 2 と同じ手順（`tree-sitter generate` / `build` /
`parse`）を、文法だけ M7 の `git show eeb1704:editors/tree-sitter-hado/grammar.js` に
戻して回す。`tests/accept/option_propagation/main.hd` の `?` が ERROR になる。

```
$ python3 target/roundlogs/probe-treesitter-mutation.py "$ts" target/roundlogs/ts-probe
restored: exit=0 ERROR/MISSING=0
without-rail_binding: exit=1 ERROR/MISSING=1
```

層別の全走（実装の直後、main 取り込み前）。

```
$ cargo test --offline
test result 73 本が ok、FAILED は 0
$ bash scripts/run-gates.sh --tier quick target/gates/quick
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates/core
TALLY: 725/725 passed; red: none
$ bash scripts/run-gates.sh --tier full target/gates/full
TALLY: 3846/3854 passed; red: verify-r62.log verify-r63.log verify-r68.log verify-string-split-linear.log
```

full の赤 4 本は main でも赤の既知の赤。赤の顔ぶれは M7 と同じで、減った門番は無い。
通過数は 5 増えた。内訳は `verify-m4` が +2（拒否 `tests/reject/E508` と受理
`tests/accept/option_propagation` を 1 件ずつ数える）、`verify-m5` が +2（同じ 2 本を
1 件ずつ数える）、`verify-m6` が +1（受理の 1 本を数える）。`verify-r57` は 6 本のまま、
`verify-postfix-binding` は変異の本数によらず J3 が 1 件である。

```
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
$ bash scripts/verify-treesitter.sh
PASSED: 5/5
```

### 審査の指摘に当てた後（本文・門番・記録）

本文と門番だけの変更なので、審査の指示により full は回していない。core は
28 本すべてのログが `PASSED: n/N` で終わり、集計は緑である。core の 734 は、
直前の 725 に main の dogfood-text-tool（core、7 件）と足した門番（core、2 件）を
足した数である。

```
$ cargo test --offline
test result 73 本が ok、FAILED は 0
$ bash scripts/run-gates.sh --tier quick target/gates/quick-review
TALLY: 41/41 passed; red: none
$ target/debug/hado run tools/gates/main.hd -- target/gates/core-review
734/734 passed; red: none
$ bash scripts/verify-restore-option-question.sh
PASSED: 2/2
$ bash scripts/verify-do-option-absent.sh
PASSED: 4/4
$ bash scripts/verify-r57.sh
PASSED: 6/6
$ bash scripts/verify-r71.sh
PASSED: 4/4
$ bash scripts/verify-postfix-binding.sh
PASSED: 4/4
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
$ bash scripts/verify-dogfood-text-tool.sh
PASSED: 7/7
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/corpus-lock.sh --check && bash scripts/gate-tiers-lock.sh --check && bash scripts/spec-lock.sh --check
（3 つとも差分なし、終了コード 0）
```

core 走行の `TALLY` 行は `| tail -1` の出力に出なかった（門番の契約では TALLY の
欠落は印なので記録する）。同じ出力先を集計の道具へ直接渡すと上のとおり緑である。

### 着地前の再走（main `621002c` 取り込み後）

`main` はこのラウンドの間に dogfood-text-tool（PR #70）の分だけ進んでいた。取り込んだ
（衝突なし）。main は locked コーパス（`tests/accept`・`tests/reject`・`stdlib`）を
変えていないので、`corpus-lock.sh --check` は差分なしで通り、作り直しは要らなかった。
main の新しい `tools/dogfood-text-tool/main.hd` は効果を持つ prelude を `as p ->` で
束縛しており、戻した仕様でも `check` は 0 で通る。

```
$ bash scripts/corpus-lock.sh --check
（差分なし、終了コード 0）
$ cargo run --offline -q -- check tools/dogfood-text-tool/main.hd
（終了コード 0）
$ bash scripts/run-gates.sh --tier quick target/gates/quick-postmerge
TALLY: 41/41 passed; red: none
```
