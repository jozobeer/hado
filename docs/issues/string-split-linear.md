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

- A（推奨、仕様不変）: `split` と `join` を C の単一走査の組み込みにし、extern 表（`crates/hado/src/runtime_abi.rs`）に足す。C からは `String Array` を返し、prelude が `Array.to_list` で List にする。`HadoString` にスカラ数を足して `length` を O(1) にする。String と Bytes は同じ配置を使うため、Bytes の確保時はスカラ数を未知値にし、全ての String 生成経路と UTF-8 検査済みの `Bytes.to_string` が正しい値を入れる。`Bytes.of_string` は同じポインタと既存の値を保つ。
- B（仕様不変、退ける）: ASCII だけ索引を O(1) にする高速経路。日本語で二乗に戻る。
- C（§19/§55 を変える）: 索引をバイト単位にし、境界外を `Error` にする。利用者のコードに索引ループが現れてから検討する。

A は `Array.to_list` が非末尾再帰（`stdlib/array.hd`）なので、prelude-tail-recursion の後に置く。

## 完了条件

- `split` と `join` の時間が行数に比例する。小さい側の中央値が 200 ms 以上になる入力と、その 2 倍の入力を生成する。各入力を 1 回 warm-up してから 5 回測り、中央値の比が 2.5 以下であることを判定する。大きい側には比とは別に 10 秒の timeout を置く。同じ判定を ASCII の入力と多バイトの入力（日本語の本文と、`split` には多バイトの区切り）の両方で行い、ASCII だけを O(1) にする案 B の経路では多バイト側が二乗のまま残ることを測る。
- `String.length` の計算量を §55 に書く。小さい文字列と 100 万文字の文字列で `String.length` の 1 回あたりの時間が文字列長に依らないことも門番で測る。同じ値への多数回呼びを平均すると、走査してスカラ数をキャッシュする実装は `(O(n) + (k-1)O(1)) / k` になって `k` を大きくすれば通るので、計測する各回は新しく作った文字列に対する 1 回だけの呼び出しにし、構築だけを行う対照との差を取る。同じ値を warm-up してから 2 回目以降だけを集計すると初回の走査が隠れるので、確保した直後の最初の呼び出しも含める。同じ判定を ASCII の入力と多バイトの入力（日本語）の両方で行い、ASCII だけを O(1) にする経路では多バイト側が走査のまま残ることを測る（`hado_string_length` が UTF-8 を走査したまま `split` と `join` だけを組み込みにしても通らない）。時間の差では走査を隠せるので、runtime のバイト走査を数える計器で `String.length` が走査しないことを判定する。計器は構築のあと・`String.length` の直前に 0 に戻し、length 実装が読んだバイトごとに増やす（`string_byte_offset` を通らない走査も含む。いまの `hado_string_length` は `:327` で自前にバイトを辿る）。新しく作った文字列への 1 回の `String.length` のあと、小さい側も 100 万文字側も、ASCII も日本語も、増分は 0 である。`string_byte_offset` だけを数える計器や、数を置いただけの対照では通らない。
- `Bytes.of_list [227, 129, 130] |> Bytes.to_string |> String.length` が 1 になる。String・Bytes の全確保経路がスカラ数を初期化し、未知値を String の `length` が読まないことを runtime の単体テストで判定する。
- `HadoString` にスカラ数を足すと `bytes[]` の位置が動くので、`{ size_t length; unsigned char bytes[N]; }` を組んで `HadoString *` に cast する既存の runtime 門番を、次の箇所の全部で移行する。`crates/hado/tests/runtime_string.rs:14`（埋め込み NUL と `あ` / `😀` の concat）、`crates/hado/tests/r34_bytes.rs:217`（生成する `Utf8Probe`）、`crates/hado/tests/r34_bytes.rs:262`（`あ` / ASCII / 継続バイトを残した途中切れ）、`crates/hado/tests/r34_bytes.rs:349`（`c_string_value`。NUL・`あ`・`😀` の slice/find）、`crates/hado/tests/r36_int.rs:47`（長さの外の尾 `"42"`）、`crates/hado/tests/r36_int.rs:50`（埋め込み NUL の `Int.parse`）、`crates/hado/tests/r37_world.rs:131`（環境名 `R37_KEYX` の長さ区切り）、`crates/hado/tests/r37_world.rs:185`（I/O の path `entriesXX` / `fileXX` の長さ区切り）。同じ関数内の `{ size_t length; }` 空値も同じ配置にする。`hado_alloc(sizeof(HadoString) + n)` で組む箇所（`crates/hado/tests/r36_int.rs:16`）は配置に従うので対象外。コンパイル済みの文字列リテラルも runtime の確保ではないので、`crates/hado/src/repr.rs:219-238` の `string_literal_data`（長さヘッダの直後に payload）と、それを埋め込む `crates/hado/src/emit.rs:559` を同じ配置に移行し、`crates/hado/tests/repr.rs:146-158` のレイアウト検査を更新する。移行後は ASCII・BMP（`あ`）・BMP 外（`😀`）のリテラルを `String.length` で測り、payload の先頭語を新しいフィールドとして読まないことを確かめる。共有の initializer を用意するか各所を同じ配置に直す。移行後も同じ門番が、埋め込み NUL、`Utf8Probe` の受理と拒否、途中切れ継続バイト、長さの外の尾、環境名の長さ 7 と 8、I/O の path の長さ 7 と 4 を測る。3 箇所だけ直す、`Utf8Probe` や `c_string_value` を古い配置のまま残す、リテラルの直列化を古い配置のまま残す、埋め込み NUL・途中切れ・長さの外の尾を落とす実装では通らない。古い配置のまま読むと payload を新しいフィールドとして読み、`bytes` の範囲外に触れる。
- 受理コーパスの String 系ケースが変わらず通る。

## 由来

`docs/wants.md` に無い。§19、§55。`docs/wants.md` 53 の実践（stdin の単語頻度を数える 20 行の CLI）で出た。
