# `String.split` と `String.join` を入力長に線形にする（string-split-linear）
優先度: 高。

## 現象

行数を 2 倍にすると時間が 4 倍になる（2026-09-17、`target/debug/hado`、1 行 1 語の入力）。

| 対象 | 入力 | 時間 |
|---|---|---|
| `read_stdin \|> Bytes.to_string \|> String.split "\n"` | 5,000 行 | 0.31 s |
| 同 | 10,000 行 | 1.25 s |
| 同 | 20,000 行 | 4.99 s |
| `String.join values ","` | 20,000 要素 | 0.33 s |
| 同 | 40,000 要素 | 1.23 s |

対照: 文字列比較 `==` 1000 万回は 0.08 s、末尾再帰 1000 万回は 0.14 s。遅いのは prelude の String 関数である。

## 原因

§55 は String の索引をスカラ値の単位と定め、内部は UTF-8 のバイト列である（§19）。`runtime/hado_runtime.c:369` の `hado_string_slice` と `:391` の `hado_string_find` は `string_byte_offset` で毎回先頭から走査し、`:327` の `hado_string_length` も O(n) である。`stdlib/string.hd` の `split_from` は片ごとに `slice`（内側で `length s` を呼ぶ）と `__string_find` を呼ぶので、片数 × 文字列長になる。`join` は右結合の `concat` で、コピー量が総長の二乗になる。`slice` と `find` から組み立てた String 関数は全部この性質を持つ。

## 案

- A（推奨、仕様不変）: `split` と `join` を C の単一走査の組み込みにし、extern 表（`crates/hado/src/runtime_abi.rs`）に足す。C からは `String Array` を返し、prelude が `Array.to_list` で List にする。あわせて `HadoString` にスカラ数を持たせ、`length` を O(1) にする。
- B（仕様不変、退ける）: ASCII だけ索引を O(1) にする高速経路。日本語で二乗に戻る。
- C（§19/§55 を変える）: 索引をバイト単位にし、境界外を `Error` にする。利用者のコードに索引ループが現れてから検討する。

A は `Array.to_list` が非末尾再帰（`stdlib/array.hd`）なので、prelude-tail-recursion の後に置く。

## 完了条件

- 上の表の入力で、`split` と `join` の時間が行数に比例する（2 倍で 2 倍以内）。門番は入力を生成し、比を判定する。
- `String.length` の計算量を §55 に書く。
- 受理コーパスの String 系ケースが変わらず通る。

## 由来

`docs/wants.md` に無い。§19、§55。`docs/wants.md` 53 の実践（stdin の単語頻度を数える 20 行の CLI）で出た。
