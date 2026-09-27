# `|>` を末尾呼び出しの引数のリストに書くと単相化が落ちる（pipe-in-tail-argument）

## 手順書

### 背景

- 効果を持つ呼び出しを `|>` で書いた要素を末尾呼び出しの引数のリストに置くと、
  型検査は通るのに単相化が `list element type mismatch` で落ちる
  （`docs/issues/pipe-in-tail-argument.md`）。直接適用に書き換えると同じプログラムが通る。
- `crates/hado/src/mono.rs` は効果を運ぶ呼び出しを ANF で継続（`$anfN`）に載せ、
  `bind` で outcome をほどく。載せるかどうかの判定は `contains_handled_call` が
  `call_carries_effects` に聞く。この判定は `Apply`/`Call` の**構文**だけを見て、
  callee が module 修飾（`Bytes.to_string`）だと `NodeKind::Field` になるので
  `call_carries_effects` の callee 判定（`Name` と `Field` だけを見る）に届く。
- `|>` の target が `Bytes.to_string` の形だと、`lift_again_arguments` は `|>` を
  `Apply` に書き換えるが、その書き換えは node を差し替えるだけで、**親**（`[...]`
  のリスト）の ANF が先に走る。親から見た子は `Pipe` のままなので、効果の判定が
  届かず、要素が `$Outcome` の閉包のまま `Cons` に並ぶ。
- callee が裸の名前（`__io_dir_get |> Bytes.to_string` ではなく
  `__argv_get index |> Bytes.to_string`）でも同じ。`Field` の target は
  `call_carries_effects` が扱えるが、呼び出し元が `Apply`/`Call` しか渡さない。

### 決定

issue の「案」の推奨に従う。退ける案（`element_type_mismatch` の検査を緩める）は採らない。

1. `call_parts` を足し、`|>` を「左辺を先頭に足した `Apply`」と同じ callee と
   引数の並びに解く。`lift_again_arguments` の書き換えと同じ規則にする。
2. `contains_handled_call` は `Apply`/`Call` だけでなく `call_parts` の経路で
   callee を見る。`|>` の target が `Field`（module 修飾）でも効果の判定に届く。
3. `call_carries_effects` の引数を `&[crate::parser::Node]` から
   `&[&crate::parser::Node]` にする。`call_parts` が借用を返すため。
4. `element_type_mismatch` の検査は変えない。食い違った `Cons` を組むと
   実行時に壊れるので、止める検査自体は要る。

### code-complete の一周目

- 意図: 効果を持つ呼び出しを `|>` で書いた要素を末尾呼び出しの引数のリストに
  置いたプログラムを、型検査のとおりに実行させる。`|>` と直接適用で意味が
  変わらないようにする。
- 仮説: `|>` の target も `Apply` と同じ callee として効果の判定に載せれば、
  親の ANF が要素を継続に載せ、要素が outcome をほどいた成功型になる。
  `lift_again_arguments` の書き換えと同じ規則で callee と引数を解けば足りる。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、
  `stdlib/env.hd`・`stdlib/io.hd`（回避策の復帰）、本ファイル。
- 未確認: `|>` の左辺が効果を運ぶ場合も同じ経路で直るか。
  `callee_is_outcome_closure` の `Apply` だけを見る形（`|>` の target が
  閉包の変数の場合）も直すべきか。module 修飾の呼び出しで `Field` の効果判定が
  右辺まで届くか。

### 期待値の根拠

issue の完了条件のとおり。`handler Error as H` の下で `Bytes.to_string` は
失敗しうる（`Bytes.to_string` は UTF-8 でないバイト列で `raise` する）ので
outcome を運ぶ。`go ["x"] []` は `"x"` を UTF-8 に直せるので abort は走らず、
`H` の return 腕が `"ok"` を返す。直す前は `Cons` を組む時点で落ちる。

### 成果物

`crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、
`stdlib/env.hd`、`stdlib/io.hd`、`tests/corpus.sha256`、本ファイル。

### 工程

再現（main で）→ 赤のテスト（issue の入力そのまま）→ `call_parts` の追加と
`contains_handled_call` の経路の追加 → 周辺の形の実測 → 回避策の復帰 →
着地の直前に `git merge main` と corpus lock の作り直し。

### 判定

- 上の `main.hd` が `ok` を印字して終了コード 0。
- 同じ入力が `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-r37.sh` が赤なし。

### 許容される赤

無し。

### スコープ外

- `callee_is_outcome_closure` の `Apply` だけを見る形（`|>` の target が
  閉包の変数の場合）。本ラウンドの完了条件の入力では差が出ず、実測でも
  `|>` と直接適用が一致した（下の「周辺の形」）。
- 構文の変更（後置構文への書き換え）。F レーンが着地したら写す。
- `cargo test --offline` と層別（quick）はラウンドの最後に 1 回だけ回す。

### 中断の条件

判定が 2 回続けて赤になったら止める。仕様の解釈に疑義が出たら仕様を変えず、
依存しない作業だけ進める。

### 仕様への疑義

無し。§5 は `|>` を「左辺を最初の引数にした適用」と定め、§7 の継続の規則は
書いた形に依存しない。今回の入力は型検査（`hado check`）が受理する。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §5, §7, §55 | `|>` は左辺を最初の引数にした適用と同じで、効果を持つ呼び出しの失敗は handler へ伝わる | 単相化 | 成功 | `crates/hado/tests/r26_effects.rs` |

## 実測

issue の入力を `target/scratch/pipetail/main.hd` に、直接適用に書き換えたものを
`target/scratch/pipetail-direct/main.hd` に置いて走らせる。直す前（`8c2ec1e` の親）。

```
$ cargo run --offline -q -- run target/scratch/pipetail/main.hd; echo "pipe exit=$?"
list element type mismatch
pipe exit=1
$ cargo run --offline -q -- run target/scratch/pipetail-direct/main.hd; echo "direct exit=$?"
ok
direct exit=0
```

直す前のテスト。

```
$ cargo test --offline --test r26_effects pipe_in_a_tail_call_argument_keeps_the_callees_outcome
list element type mismatch

  left: Some(1)
 right: Some(0)
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 62 filtered out; finished in 0.12s
```

直した後。

```
$ cargo run --offline -q -- run target/scratch/pipetail/main.hd; echo "exit=$?"
ok
exit=0
$ cargo test --offline --test r26_effects pipe_in_a_tail_call_argument_keeps_the_callees_outcome
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 62 filtered out; finished in 0.17s
$ cargo test --offline --test r26_effects
test result: ok. 63 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s
```

触ったところに効く門番（`mono.rs` は M レーンの成果物を判定する `verify-m4.sh` が見る）。

```
$ bash scripts/verify-m4.sh | tail -4
reject: 27/27
accept: 124/124
PASSED: 153/153
```

### 原因の切り分け

`HADO_DEBUG_PIPE` を一時的に足して `dump-mono` を走らせ、`list_elements` が見る
要素の値の型を比べた。`|>` の側だけ `$Outcome` の閉包のまま残る。

```
$ HADO_DEBUG_PIPE=1 cargo run --offline -q -- dump-mono target/scratch/pipetail/main.hd
DBG list_elements span=(6, 32) declared=None resolved=Data("List", [String]) derived=None valtys=[Data("$Outcome", [String, String, Void])]
main.hd:7:12: error: list element type mismatch
$ HADO_DEBUG_PIPE=1 cargo run --offline -q -- dump-mono target/scratch/pipetail-direct/main.hd
DBG list_elements span=(6, 32) declared=None resolved=Data("List", [String]) derived=Some(String) valtys=[String]
```

module 修飾の target は `Application` の callee が `Field` になり、
`call_carries_effects` の名前判定（`Name` と `Field`）には届く。届かないのは
入口で、`contains_handled_call` が `Apply`/`Call` の形しか `call_carries_effects`
に渡していなかった。

### 周辺の形

`|>` と直接適用で意味が変わらないことを、直した後に 3 つの形で確かめる。
どれも両側が同じ値を印字する。

```
$ cargo run --offline -q -- run target/scratch/pipeleft/main.hd; echo "pipe exit=$?"
9
pipe exit=0
$ cargo run --offline -q -- run target/scratch/pipeleft-direct/main.hd; echo "direct exit=$?"
9
direct exit=0
$ cargo run --offline -q -- run target/scratch/pipecallee/main.hd; echo "pipe exit=$?"
9
pipe exit=0
$ cargo run --offline -q -- run target/scratch/pipecallee-direct/main.hd; echo "direct exit=$?"
9
direct exit=0
$ cargo run --offline -q -- run target/scratch/harmcallee/main.hd; echo "exit=$?"
9
exit=0
```

入力は、左辺が効果を運ぶ `8 / 1 |> id` と `id (8 / 1)`、`|>` の左辺が効果を持つ
閉包で target が効果を持つ定義（`g |> apply` と `apply g`）、handler の腕が返す
閉包を callee にした `(if true => h | g) 1`。

### 回避策の復帰

`stdlib/env.hd` の `argv_elements` と `stdlib/io.hd` の `directory_elements` は
この欠陥を避けるために `|>` をやめて直接適用にしてあった。欠陥が消えたので
`|>` の形に戻す。深さを上げる門番が緑。

```
$ git diff --stat stdlib
 stdlib/env.hd | 4 +---
 stdlib/io.hd  | 4 +---
$ bash scripts/verify-prelude-tail-recursion.sh 2>&1 | tail -3
OK: h docs: spec and corpus
PASSED: 8/8
```

`|>` を戻した直後の `verify-r37.sh` は `37-i full corpus lock` だけが赤だった。
`stdlib/` を変えたので `tests/corpus.sha256` が古い。lock を作り直した後は緑。

```
$ bash scripts/verify-r37.sh > target/scratch/r37.log 2>&1; echo "exit=$?"; grep -n "NG:" target/scratch/r37.log
exit=1
512:NG: 37-i full corpus lock
$ tail -1 target/scratch/r37.log
PASSED: 259/260
```

lock を作り直す前の `--check` の差分。変わる行は `stdlib/env.hd` と
`stdlib/io.hd` の 2 行だけで、どちらもこのラウンドの成果物である。

```
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
--- tests/corpus.sha256	2026-09-27 10:10:47.417738111 +0900
+++ /tmp/tmp.e2CWSKEWtS	2026-09-27 10:24:55.313422686 +0900
@@ -1,11 +1,11 @@
 8cab964491d747f6b2280f9ee51810fdf73c49dcfd37cfe9b47425b963b518c3  stdlib/array.hd
 8f40e22e2dc1dd4536a7f5ce4f476f6328b73062294cf73119eec642ce3c1849  stdlib/bytes.hd
 3fca3c48816294c7a1766dcb546d05ca0eff8740e7bf758c1694edee96f035a7  stdlib/char.hd
-67bb1080d67a68d2a48baf1dabdbeb2704d96c5b2eb5ef5d9e294e9d2fdd8f71  stdlib/env.hd
+e04481671a5640f9b5ec246bbc2c8cf384827fc834f622f9ba58c68b108b13b6  stdlib/env.hd
 3b2a0f6605ca571b70546f3a572b4974647b43690840e0b0646ffe03e819cd93  stdlib/error.hd
 1d9bf10266423fb75fedcdd6fa08d444c1ef836519e49d216493b8e8d1e1f294  stdlib/float.hd
 22b252e3ad1fa7f9555fec52bb310e4dc2f7e3e377d46c2358e4a549005e9ada  stdlib/int.hd
-c9ff7427ae7d49e891013cc32d05b4f51b35e715ad705e631a77681f52dfdd58  stdlib/io.hd
+9e44ce3ef35a90cffadc1a68b7b21e5cbc25368decf2aa60652aff67c7447cdd  stdlib/io.hd
 c78a1bd83d5a5cd0ee6d3d2a85970b1663d4ea5c4509fbe8634b8d0860f51878  stdlib/list.hd
 021092574d9835fd4ea865a8ad3e9a2b3a7e80a061e0464d77ac400762762976  stdlib/option.hd
 a951b6e909818c3d2ccfe93f64f8b858824c0838480acf1551c888bb1ed971c8  stdlib/string.hd
exit=1
$ grep -cE '^[+-][0-9a-f]' target/scratch/lock-check.log
4
```

### ラウンド最後の全走

`git merge main`（`main` は `origin/main` と同じ `e9e2352` で、取り込む変更は
無かった）と `bash scripts/corpus-lock.sh` の後。

```
$ cargo test --offline > target/final-cargo-test.log 2>&1; echo "exit=$?"
exit=0
$ grep -c '^test result: ok' target/final-cargo-test.log
69
$ grep -cE '^test result: FAILED' target/final-cargo-test.log
0
$ bash scripts/run-gates.sh --tier quick target/gates-quick | tail -1
TALLY: 41/41 passed; red: none
$ bash scripts/verify-r37.sh | tail -1
PASSED: 260/260
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh | tail -1
PASSED: 2/2
```

`verify-rustfmt` は最初の全走で 1 件赤になった（`call_parts` の腕の折り返し）。
`cargo fmt --all` を当てて緑。整形だけなので `feat:` のコミットに畳んだ。

着地後（2026-09-27）。PR #56 は `a19f6cd` で入った。着地後の全走は手元で回さず、main の CI（full 層）を読む。

```
$ gh run list --branch main --limit 12
completed	success	Merge pull request #56 from jozobeer/pipe-in-tail-argument	ci	main	push	36291224783	20m08s	2026-09-27T01:48:14Z..2026-09-27T02:08:22Z
```
