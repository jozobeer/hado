# 入れ子の適用の効果付き閉包が続きを落とす（nested-application-continuation）

## 手順書

### 背景

#115 の現象。`(get 0) 0 as n -> n + 10` は、`get 0` が返した効果付き閉包をその場で
呼んでいる。`resume` はその値 1 だけを返し、`+ 10` が走らない。main `ca66db0` では
次の入力の終了コードが 1 になり、stdout と stderr は空になる。

```hado
effect Choice
again pick: Int
handler Choice as C
again pick => resume 1
return value => value
def get x => fn y =>
  pick as n ->
  n + x * 0 + y * 0
/fn
def main =
  with C ->
  (get 0) 0 as n ->
  n + 10
```

先に閉包を束ねた `get 0 as f -> f 0 as n -> n + 10` は 11 になる。差は呼び手の形だけ
である。`crates/hado/src/mono.rs` の `callee_is_outcome_closure` は名前・field・分岐
しか効果付き閉包として見ておらず、呼び手が適用だと `call_carries_effects` が偽になる。
`as` が `propagate_outcome` に落ち、`$Yield` は続きを張らずにそのまま外へ流れる。

### 意図

callee の推論した型が、いまの handler の下で結果を outcome にする閉包なら、field と
同じく呼び出しを効果付きとして扱う。`let_expression` が `bind_again_outcome` を選び、
`as` の続きが `make_continuation` の継続に入る。判定はノードの種類ではなく型で行う
ので、適用・field・分岐のどれでも同じ結果になる。

### 仮説

- `callee_is_outcome_closure` の型の照合を `NodeKind::Field` から任意の callee に
  広げれば、`(get 0) 0` の呼び出しが効果付きとして見える。
- 適用の span の型は `composite_types` にあり、field は `value_types` にある。両方を
  見れば、既存の field の判定は変わらない。
- 効果を運ばない閉包の入れ子は outcome に直らないので、素の呼び出しのまま残る。

### 判定器

`scripts/verify-nested-application-continuation.sh`（新設、`# tier: quick`、
`# round: nested-application-continuation`）:

- a. 入れ子の適用 `(get 0) 0 as n -> n + 10` は 11 になる。
- b. 束ねた形 `get 0 as f -> f 0 as n -> n + 10` も 11 になる（対照）。
- c. 同じ入れ子の呼び出しで `8 / 0` が起きれば Error の腕が 0 を返す（対照）。
- d. 効果を運ばない閉包の入れ子は、handler の下でも素の呼び出しのまま 14 になる（対照）。

結合テストは `crates/hado/tests/r26_effects.rs` の
`nested_application_of_an_effectful_closure_keeps_the_continuation` が持つ。

### スコープ

触る: `crates/hado/src/mono.rs`、`crates/hado/tests/r26_effects.rs`、
`scripts/verify-nested-application-continuation.sh`（新設）、
`docs/rounds/nested-application-continuation.md`。

触らない: `docs/spec.md`（§11・§37・§38 の解釈は変えない）、
`tests/accept`・`tests/reject`・`stdlib/`（この修正はコーパスを変えない）。

### 工程

1. main で入れ子の形が 1、束ねた形が 11 になることを再現する。
2. 再現テストを先に置き、main の木で赤を測る。
3. callee の型の照合を任意のノードに広げる。
4. 専用の門番を置き、main の実装で赤、修正後に緑になることを測る。
5. main を取り込み、corpus lock の差分を読んでから照合する。
6. 門番・quick 層・full テストを回して PR にする。
7. 着地後に main で門番と層別を実測してこのファイルに足し、#115 を閉じる。

### 許容される赤

- なし。

### スコープ外

- 呼び手がさらに深く入れ子になった形（`((get 0) 0) 0` など）。`get` の結果が
  関数を返さないので、この形は型が通らない。
- `|>` の target が適用の形。`pipe` は target を `Apply` として callee を取り出すので、
  同じ経路で直る。専用の検体は置かない。
- 効果行の推論そのもの。型が効果を運ぶかは checker の結果を読むだけで、判定を増やさない。

### 中断の条件

- 型を広げたことで、効果を運ばない閉包の呼び出しが outcome になる（門番 d が赤）。
- 既存の field の判定が変わり、`r26_effects` の他のテストが赤になる。
- 判定が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §11 | `as` の本体は `->` の右側の継続である | lower | `callee_is_outcome_closure`・`bind_again_outcome` | 門番 a・b |
| §36 | 効果による失敗は何も書かずに伝わる | lower | `propagate_outcome` の `$Done` の腕 | 門番 c |
| §37 | 効果の操作の種別で `resume` の有無が決まる | lower | `make_continuation` | 門番 a |
| §38 | `with H -> rest` は `rest` を `H` の下で評価する | lower | `inferred_outcome_closure` の handler の答え | 門番 a・d |

## 実測

main は `ca66db0`。作業は `.worktrees/nested-application-continuation` の
`nested-application-continuation` で行った。

### 現象（main）

```
$ ./target/debug/hado run .repro115/nested/main.hd >out 2>err; echo "exit=$?"; echo "stdout: $(cat out)"; echo "stderr: $(cat err)"
exit=1
stdout: 
stderr: 
```

束ねた形は 11 になる（対照）。

```
$ ./target/debug/hado run .repro115/bound/main.hd >out 2>err; echo "exit=$?"
exit=11
```

### テスト先行（`d152fa5`）

修正前の木で新しい結合テストは赤。

```
$ cargo test --offline --test command r26_effects::nested_application_of_an_effectful_closure_keeps_the_continuation
thread 'r26_effects::nested_application_of_an_effectful_closure_keeps_the_continuation' panicked at crates/hado/tests/r26_effects.rs:1753:5:
assertion `left == right` failed: ...
  left: [49, 10]
 right: [49, 49, 10]
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 399 filtered out
```

stdout は `1` で、期待する `11` にならない。

### 修正後（`15d2d8d`）

```
$ cargo test --offline --test command r26_effects::nested_application_of_an_effectful_closure_keeps_the_continuation
test r26_effects::nested_application_of_an_effectful_closure_keeps_the_continuation ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 399 filtered out
```

```
$ ./target/debug/hado run .repro115/nested/main.hd >out 2>err; echo "exit=$?"
exit=11
$ ./target/debug/hado run .repro115/bound/main.hd >out 2>err; echo "exit=$?"
exit=11
```

### 門番（`e539890`）

main の実装（`crates/hado/src/mono.rs` を `ca66db0` に戻す）では検査 a だけが赤。

```
$ bash scripts/verify-nested-application-continuation.sh
NG: a: a nested application keeps the continuation
OK: b: the bound form still keeps the continuation
OK: c: an abort arm works through the nested call
OK: d: a pure closure stays a plain call
PASSED: 3/4
```

修正後は 4/4。

```
$ bash scripts/verify-nested-application-continuation.sh
OK: a: a nested application keeps the continuation
OK: b: the bound form still keeps the continuation
OK: c: an abort arm works through the nested call
OK: d: a pure closure stays a plain call
PASSED: 4/4
```

### テスト

```
$ cargo test --offline --test command r26_effects::
test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 304 filtered out
```
