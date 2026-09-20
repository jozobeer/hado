# `|>` を末尾呼び出しの引数のリストに書くと単相化が落ちる（pipe-in-tail-argument）
優先度: 中。

## 現象

効果を持つ呼び出しを `|>` で書いた要素を、末尾呼び出しの引数のリストに置くと、型検査は通るのに単相化が `list element type mismatch` で落ちる。直接適用に書き換えると同じプログラムが通る（2026-09-20。`docs/plan.md` 35 節 P1 の prelude の書き直しで当たった）。

```
$ cat main.hd
handler Error as H
abort raise e => e.message
return _ => "ok"
def rec go (values: String List, acc: String List): String List {Error} =>
  if values | [] => acc
  | [head, ...tail] => go tail [Bytes.of_string head |> Bytes.to_string, ...acc]
def main = with H -> go ["x"] []
$ cargo run --offline -q -- run main.hd
list element type mismatch
$ sed 's/Bytes.of_string head |> Bytes.to_string/Bytes.to_string (Bytes.of_string head)/' main.hd > direct.hd
$ cargo run --offline -q -- run direct.hd
ok
```

## 原因

`crates/hado/src/mono.rs` の `pipe_expression` が `application` と同じ型を引かないので、`|>` の結果が実引数の outcome を持つ閉包のまま残る。`crates/hado/src/mono.rs` の `element_type_mismatch` が `Cons` を組む前に止める。`stdlib/env.hd` の `argv_elements` と `stdlib/io.hd` の `directory_elements` は `|>` をやめて直接適用にしてある。

## 案

- 推奨: `pipe_expression` の結果型を `application` と同じ経路でそろえる（`crates/hado/src/mono.rs` の `lift_again_arguments` が `|>` を `Apply` に書き換えたときと同じ型を引く）。
- 退ける: `element_type_mismatch` の検査を緩める。食い違った `Cons` を組むと実行時に壊れるので、止める検査自体は要る。

## 完了条件

- 上の `main.hd` が `ok` を印字して終了コード 0 になる。
- `cargo test --offline` が 0 failed、`bash scripts/verify-r37.sh` が red なし。

## 由来

PR #7 と同じ族（outcome の落ちる単相化）。§5、§7、§55。
