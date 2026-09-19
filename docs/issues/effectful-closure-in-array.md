# 効果を持つ閉包を Array に置くと outcome が落ちる（effectful-closure-in-array）
優先度: 高。

## 現象

型検査は通るのに、`Array.of_list` に入れた効果を持つ閉包を `Array.get` で取り出して呼ぶと、単相化が `array intrinsic argument type mismatch` で落ちる（2026-09-18）。

```
$ cat main.hd
handler Error as Catch
abort raise _ => 0
return value => value
def main =
  with Catch ->
  let xs = Array.of_list [fn(x => 8 / x)] ->
  let f = Array.get xs 0 ->
  f 0
$ cargo run --offline -q -- run main.hd
array intrinsic argument type mismatch
```

除算の 0 が Error handler に届く前にコンパイルが止まる。`List` と ADT では同じ穴が既に塞がっている。

## 原因

`crates/hado/src/mono.rs` の `strip_outcome`、`logical_argument` が、`Array.of_list` の単相化で実引数の閉包から入れ子の outcome を落とす。`__array_alloc` は outcome 無しの閉包の配列を作るのに、`Array.get` の `raise` の腕は検査の成功型を保つので `done` が取り出した閉包を包めない（§19、§36）。

## 案

- 推奨: 実引数と論理引数の差から入れ子の outcome を特殊化した結果型へ戻し（`restore_nested_outcomes`）、`__array_alloc` には戻した配列型を、特殊化した callee の abort/`raise` には戻した成功型を使う。PR #7 の差分で、`crates/hado/tests/r26_effects.rs` の `an_effectful_closure_in_an_array_slot_keeps_its_outcome` が赤から緑になる。
- 退ける: `Array` の member を効果多相にする。§19 の型を変える仕様変更で、効果の位置が広がる。

## 完了条件

- 上のプログラムが `0` を印字して終了コード 0 になる。
- `crates/hado/tests/r26_effects.rs` に効果を持つ閉包を Array に置いて handler で受けるテストがあり、直す前は赤になる。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` の Array の検査が red なし。

## 由来

`docs/wants.md` に無い。PR #7（`cursor/critical-bug-management-7ad0`）。§19、§36。
