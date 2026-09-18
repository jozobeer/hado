# `String.split` と `String.join` を入力長に線形にする（string-split-linear）
優先度: 高。

## 現象

行数を 2 倍にすると時間が約 4 倍になる（2026-09-17、`cargo run --offline -q -- run <file>`、warm-up 後 5 回の中央値。起動と `run` のビルドを含む。入力は 1 行 1 語で、末尾に改行を置かない）。

```
$ cat split.hd
def main =
  let bytes = IoDefault.read_stdin ->
  let values = Bytes.to_string bytes |> String.split "\n" ->
  values |> List.length |> Int.to_string |> IoDefault.log
$ cat join.hd
def rec build n acc => if n == 0 => acc | build (n - 1) [Int.to_string n, ...acc]
def main =
  let values = build 20000 [] ->
  String.join values "," |> String.length |> Int.to_string |> IoDefault.log
$ python3 -c 'import sys; sys.stdout.write("\n".join("word%d" % i for i in range(20000)))' > in20000.txt
$ cargo run --offline -q -- run split.hd < in20000.txt
20000
```

| 対象 | 入力 | 時間 |
|---|---|---|
| `split.hd` | 5,000 行 | 0.84 s |
| 同 | 10,000 行 | 2.96 s |
| 同 | 20,000 行 | 11.8 s |
| `join.hd`（`build 20000`。`String.split` を通さず `Int.to_string` の fold でリストを作り、`join` だけを測る） | 20,000 要素 | 1.09 s |
| 同（`build 40000`） | 40,000 要素 | 4.10 s |

対照: 文字列比較 `==` 1000 万回は 0.08 s、末尾再帰 1000 万回は 0.14 s。遅いのは prelude の String 関数である。

## 原因

§55 は String の索引をスカラ値の単位と定め、内部は UTF-8 のバイト列である（§19）。`runtime/hado_runtime.c:369` の `hado_string_slice` と `:391` の `hado_string_find` は `string_byte_offset` で毎回先頭から走査し、`:327` の `hado_string_length` も O(n) である。`stdlib/string.hd` の `split_from` は片ごとに `slice`（内側で `length s` を呼ぶ）と `__string_find` を呼ぶので、片数 × 文字列長になる。`join` は右結合の `concat` で、コピー量が総長の二乗になる。`slice` と `find` から組み立てた String 関数は全部この性質を持つ。

## 案

- A（推奨、仕様不変）: `split` と `join` を C の単一走査の組み込みにし、extern 表（`crates/hado/src/runtime_abi.rs`）に足す。C からは `String Array` を返し、prelude が `Array.to_list` で List にする。`HadoString` にスカラ数を足して `length` を O(1) にする。String と Bytes は同じ配置を使うため、Bytes の確保時はスカラ数を未知値にし、全ての String 生成経路と UTF-8 検査済みの `Bytes.to_string` が正しい値を入れる。`Bytes.of_string` は同じポインタと既存の値を保つ。
- B（仕様不変、退ける）: ASCII だけ索引を O(1) にする高速経路。日本語で二乗に戻る。
- C（§19/§55 を変える）: 索引をバイト単位にし、境界外を `Error` にする。利用者のコードに索引ループが現れてから検討する。

A は `Array.to_list` が非末尾再帰（`stdlib/array.hd`）なので、prelude-tail-recursion の後に置く。

## 完了条件

- `split` と `join` の時間が行数に比例する。小さい側の中央値が 200 ms 以上になる入力と、その 2 倍の入力を生成する。各入力を 1 回 warm-up してから 5 回測り、中央値の比が 2.5 以下であることを判定する。大きい側には比とは別に 10 秒の timeout を置く。同じ判定を ASCII の入力と多バイトの入力（日本語の本文と、`split` には多バイトの区切り）の両方で行い、ASCII だけを O(1) にする案 B の経路では多バイト側が二乗のまま残ることを測る。区切りの長さを入力と一緒に増やす対照も置く。入力本文は `a` の連続 1 本とし、短い行を並べた入力にはしない。区切りは `a` の連続のあとに `b` で、長さ m は入力長 n に比例する（例: 入力 `a` × n、区切り `a` × (n/2 − 1) + `b`）。本文に区切りを短い間隔で挟まない（末尾に 1 回足すか、現れないかのどちらか）。この対照でも小さい側の中央値が 200 ms 以上になる入力と、入力と区切りの両方を 2 倍した入力を生成し、各入力を 1 回 warm-up してから 5 回測り、`split` の中央値の比が 2.5 以下であることを判定する。大きい側には比とは別に 10 秒の timeout を置く。短い固定長の区切り（`"\n"` や `"ab"`）だけを測る、本文が `a` 以外で候補位置が 1 文字で外れる、行数だけ 2 倍して区切り長を据え置く実装では通らない。区切りの候補位置ごとに `memcmp` で照合する実装は O(n·m) のまま残るので通らない。多バイトの区切りでも同じ対照を置く。本文を `あ` の連続 1 本、区切りを `あ` × (n/2 − 1) + `い` とし、入力と区切りの両方を 2 倍した 2 つで `split` の中央値の比が 2.5 以下であることを判定する（`あ` は 3 バイトなので、区切りの長さも入力長に比例して伸びる）。ASCII の区切りだけを線形にし、多バイトの区切りは候補位置ごとの照合のまま残す実装では通らない。
- `String.length` の計算量を §55 に書く。小さい文字列と 100 万文字の文字列で `String.length` の 1 回あたりの時間が文字列長に依らないことも門番で測る。同じ値への多数回呼びを平均すると、走査してスカラ数をキャッシュする実装は `(O(n) + (k-1)O(1)) / k` になって `k` を大きくすれば通るので、計測する各回は新しく作った文字列に対する 1 回だけの呼び出しにし、構築だけを行う対照との差を取る。同じ値を warm-up してから 2 回目以降だけを集計すると初回の走査が隠れるので、確保した直後の最初の呼び出しも含める。同じ判定を ASCII の入力と多バイトの入力（日本語）の両方で行い、ASCII だけを O(1) にする経路では多バイト側が走査のまま残ることを測る（`hado_string_length` が UTF-8 を走査したまま `split` と `join` だけを組み込みにしても通らない）。時間の差では走査を隠せるので、runtime のバイト走査を数える計器で `String.length` が走査しないことを判定する。計器は構築のあと・`String.length` の直前に 0 に戻し、length 実装が読んだバイトごとに増やす（`string_byte_offset` を通らない走査も含む。いまの `hado_string_length` は `:327` で自前にバイトを辿る）。新しく作った文字列への 1 回の `String.length` のあと、小さい側も 100 万文字側も、ASCII も日本語も、増分は 0 である。`string_byte_offset` だけを数える計器や、数を置いただけの対照では通らない。
- `Bytes.of_list [227, 129, 130] |> Bytes.to_string |> String.length` が 1 になる。String・Bytes の全確保経路がスカラ数を初期化し、未知値を String の `length` が読まないことを runtime の単体テストで判定する。
- `HadoString` にスカラ数を足すと `bytes[]` の位置が動くので、`{ size_t length; unsigned char bytes[N]; }` を組んで `HadoString *` に cast する既存の runtime 門番を、次の箇所の全部で移行する。`crates/hado/tests/runtime_string.rs:14`（埋め込み NUL と `あ` / `😀` の concat）、`crates/hado/tests/r34_bytes.rs:217`（生成する `Utf8Probe`）、`crates/hado/tests/r34_bytes.rs:262`（`あ` / ASCII / 継続バイトを残した途中切れ）、`crates/hado/tests/r34_bytes.rs:349`（`c_string_value`。NUL・`あ`・`😀` の slice/find）、`crates/hado/tests/r36_int.rs:47`（長さの外の尾 `"42"`）、`crates/hado/tests/r36_int.rs:50`（埋め込み NUL の `Int.parse`）、`crates/hado/tests/r37_world.rs:131`（環境名 `R37_KEYX` の長さ区切り）、`crates/hado/tests/r37_world.rs:185`（I/O の path `entriesXX` / `fileXX` の長さ区切り）。同じ関数内の `{ size_t length; }` 空値も同じ配置にする。`hado_alloc(sizeof(HadoString) + n)` で組む箇所（`crates/hado/tests/r36_int.rs:16`）は配置に従うので対象外。コンパイル済みの文字列リテラルも runtime の確保ではないので、`crates/hado/src/repr.rs:219-238` の `string_literal_data`（長さヘッダの直後に payload）と、それを埋め込む `crates/hado/src/emit.rs:559` を同じ配置に移行する。`crates/hado/tests/repr.rs:146-158` のレイアウト検査は、長さヘッダとスカラ数ヘッダの両方のあとに payload が続くことと、バイト長とスカラ数が異なる入力（いまの `"a\0あ"` は 5 バイト・3 スカラ）を残して更新する。コンパイラ側の印字も `HadoString` を組むので、`crates/hado/runtime_print.c:13` の `quote_text`（`result->length` だけを設定し、`bytes` に引用符と本文を書く）と `crates/hado/runtime_print.c:39` の `hado_canonical_float`（同じく `length` だけを設定する）を同じ配置に移行し、`hado_quote_string` / `hado_quote_char` の呼び出しも移行後の配置で読む。ここを古い配置のまま残すと、runtime ABI が未初期化のスカラ数を返し、`escape` を通る入れ子の文字列・文字の印字と `Float` の印字がその値を読む。`length` だけを設定する String の返り値は印字以外にも残っている。`runtime/hado_runtime.c:369` の `hado_string_slice`、`:406` の `hado_string_trim`、`:465` の `hado_int_to_string`、`:512` の `hado_float_to_string`、`:523` の `hado_char_to_string`、`:597` の `hado_bytes_display`、`:630` の `hado_bytes_of_string`、`:634` の `hado_bytes_to_string_unchecked` を同じ配置に移行する。個別の列挙では取りこぼすので、門番は `runtime/hado_runtime.c` と `crates/hado/runtime_print.c` から `HadoString *` を返す関数をソースから列挙する（列挙は正規表現でソースから作り、関数名を判定器に直書きしない）。列挙した関数は返り値の意味で 2 つに分ける。String を返す関数（`hado_string_slice` `hado_string_trim` `hado_int_to_string` `hado_float_to_string` `hado_char_to_string` `hado_bytes_display` `hado_bytes_of_string` `hado_bytes_to_string_unchecked` と `crates/hado/runtime_print.c` の `quote_text` `hado_canonical_float` など）は、スカラ数がバイト列のスカラ数と一致することを測る。Bytes を返す関数（`hado_bytes_alloc` `hado_bytes_fill_unchecked` `hado_io_take_bytes` `hado_argv_get` `hado_env_get` `hado_cwd` など）は任意のバイト列を持てるので、案 A の未知値を入れて、`String.length` がその未知値を読まないことを測る（不正な UTF-8 のバイト列に一致するスカラ数は無いので、String と同じ一致を要求しない）。`HadoString *` を返すというだけで String のスカラ数の一致を要求する門番は、Bytes の経路で成り立たないので通らない。移行後の判定は、ソース上のリテラルに対する `String.length "a"`、`String.length "あ"`、`String.length "😀"` がそれぞれ 1 であること（`"a"` はバイト長もスカラ数も 1 なので、スカラ数ヘッダを長さの前に足した旧直列化でも 1 のまま通る。`"あ"` は 3 バイト・1 スカラ、`😀` は 4 バイト・1 スカラで、旧配置は payload の先頭語をスカラ数として読む）。`== 1` を置かない、ASCII だけを測る、レイアウト検査を長さヘッダと payload だけの旧配置のまま残す実装では通らない。加えて、`print` が返す引用付き文字列と正準形の `Float` の印字を直接測る。`print "あ"`（`😀` も）の出力の `String.length` が引用符 2 文字と本文のスカラ数の和になり、`print 0.1` の出力の `String.length` が `Float.to_string 0.1` のスカラ数と一致し、`print` した 3 つ（引用付き文字列・文字・`Float`）を連結しても前の結果を壊さないことを、runtime の単体テストで判定する。multibyte の引用付き文字列と正準形 `Float` のスカラ数を測らない実装では通らない。移行した各経路の返り値も直接測る。`String.length (Float.to_string 0.1) == 3`（`0.1`）、`String.length (Int.to_string 12345) == 5`、`String.length (Char.to_string 'あ') == 1`、`String.length (String.slice "あいう" 0 1) == 1`、`String.length (String.trim " あ ") == 1`、`String.length (Bytes.to_string (Bytes.of_list [227, 129, 130]))` が UTF-8 の 1 スカラ（`Bytes.to_string` はバイト列をそのまま String にするので 3 バイト・1 スカラ）になる。Bytes の表示（`crates/hado/src/mono.rs:3833` が `hado_bytes_display` を呼ぶ経路）は runtime の単体テストで返り値のスカラ数を測り、multibyte のバイト列と ASCII のバイト列の両方で桁数と一致することを判定する。`Float.to_string`・`Int.to_string`・`Char.to_string`・`String.slice`・`String.trim`・`Bytes.to_string`・Bytes の表示のスカラ数を測らない実装では通らない。共有の initializer を用意するか各所を同じ配置に直す。移行後も同じ門番が、埋め込み NUL、`Utf8Probe` の受理と拒否、途中切れ継続バイト、長さの外の尾、環境名の長さ 7 と 8、I/O の path の長さ 7 と 4 を測る。3 箇所だけ直す、`Utf8Probe` や `c_string_value` を古い配置のまま残す、リテラルの直列化を古い配置のまま残す、埋め込み NUL・途中切れ・長さの外の尾を落とす実装では通らない。古い配置のまま読むと payload を新しいフィールドとして読み、`bytes` の範囲外に触れる。
- 受理コーパスの String 系ケースが変わらず通る。
- 組み込みへ移す前後で `split` と `join` の結果の意味を変えない。いまの prelude（`stdlib/string.hd:22`、`stdlib/string.hd:26`）と同じ結果になることを直接測る（2026-09-17 に実測した値）。`String.split "aaaaa" "aa"` は重ならず `["", "", "a"]` になる（一致した区切りの幅だけ進む）。`String.split "" ","` は `[""]`、`String.split "a,b," ","` は `["a", "b", ""]`、`String.split "a" ","` は `["a"]`。`String.join [] ","` は `""`、`String.join ["a"] ","` は `"a"`（§55）。区切りが空の `String.split "abc" ""` は §55 のとおり `message` が `"EmptySeparator"` の `{Error}` になる（stderr が `EmptySeparator`、終了コード 1）。いまの受理コーパスの `split` は区切りが 1 文字、`join` は 2 要素だけなので、重なる区切り・空の本文・末尾の区切り・空と 1 要素の `join` を直接測らない実装では通らない。

## 由来

`docs/wants.md` に無い。§19、§55。`docs/wants.md` 53 の実践（stdin の単語頻度を数える 20 行の CLI）で出た。
