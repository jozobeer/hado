# 効果を持つ閉包を多相な prelude 関数に通すと単相化が落ちる（effectful-closure-through-prelude）

## 手順書

### 背景

- 効果を持つ閉包のリストを `List.reverse`・`List.map`・`Array.to_list`・`Array.set` に
  通すと、型検査は通るのに単相化が落ちる（`docs/issues/effectful-closure-through-prelude.md`）。
  契約の member が `Fn(A => Int)` を返す `maker` の形も同じ issue が挙げている。
- `effectful-closure-in-array`（PR #43）の二次レビュー対応で、結果型は「実際の型での
  単一化」から取るようになった。本体は今も `logical`（閉包の結果から `$Outcome` を
  外した型）の引数と `inferred.result` で下ろすので、引数から来た値（実際の型）と
  本体が組み立てる値（論理型）が食い違う。`restored_array_allocation`・
  `restored_success_type` はその食い違いを箇所ごとに埋めている。
- `maker` は `Maker.make` の呼び出しが module 修飾なので、`specialize_function` に
  期待する結果型が渡らず、返す閉包の引数の型が解けない（`unresolved return type`）。

### 決定

issue の「案」の推奨に従う。落ちる箇所ごとに `restored_*` を足す方向は採らない。

1. `specialize_function` が scheme を単一化する期待型を、引数の実際の型から組む。
   引数の型が関数型の位置では、本体が outcome をほどくので、その結果型だけ
   `$Outcome` を外し、引数の型（ドメイン）は実際の型のまま残す。
2. `lower_specialization` は本体をその期待型（および実際の結果型）で下ろす。
   本体の呼び出しと構築が、引数の実際の型と一貫する。
3. `Maker.make` のような module 修飾の呼び出しでも、呼び出し位置の期待結果型を
   `specialize_function` に渡す。

### code-complete の一周目

- 意図: 効果を持つ閉包を prelude の多相関数や契約越しに渡すプログラムを、
  型検査のとおりに実行させる。
- 仮説: 本体を実際の型で下ろせば、引数から来た値と本体の構築が一致する。
  scheme が関数型を書いた引数は、ドメインを実際の型・結果を `$Outcome` を外した
  成功型にすれば単一化が通り、`restore_*` は要らなくなる。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。
- 未確認: 本体を実際の型で下ろすと、効果付きの要素を持つリストを `List.map` が
  たどるとき、callback の呼び出しがどの handler の答えを運ぶか。`restore_*` が
  本当に不要になるか。M2（list literal）・M3（pipe）の穴が同時に消えるかは
  本ラウンドの判定器の外。

### 期待値の根拠

issue の完了条件のとおり。`handler Error as Catch` の下で `8 / 0` の `raise` が
腕に届き、`reverse` は `g 0` の失敗で `0`、`map`・`to_list` は割り切れて
`List.length` が `2`、`set` は `f 5 = 5 * 3 / 5 = 3`、`maker` は `f x = 1`。

### 成果物

`crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、本ファイル。

### 工程

再現 → 赤のテスト（5 入力、maker は期待結果型の経路）→ 期待型の組み直しと本体の
実際の型化 → 欠陥の注入 → 周辺の形の実測。

### 判定

- 5 つの入力が期待値（`reverse` 0、`map` 2、`to_list` 2、`set` 3、`maker` 1）を
  印字して終了コード 0。
- 同じ 5 つが `crates/hado/tests/r26_effects.rs` のテストになり、直す前は赤。
- `cargo test --offline` が 0 failed、`bash scripts/verify-m4.sh` が赤なし。

### 許容される赤

無し。

### スコープ外

- 効果の違う閉包を並べたリストリテラル（`effectful-closure-in-list-literal`）。
- 末尾引数の pipe（`pipe-in-tail-argument`）。
- `cargo test --offline` と層別（quick）はラウンドの最後に 1 回だけ回す。

### 中断の条件

判定が 2 回続けて赤になったら止める。仕様の解釈に疑義が出たら仕様を変えず、
依存しない作業だけ進める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §19, §36 | 効果を持つ閉包を多相な prelude 関数や契約越しに渡しても、失敗が handler へ伝わる | 単相化 | 成功 | `crates/hado/tests/r26_effects.rs` |

## 実測

（この節はラウンドの最後に埋める。）
