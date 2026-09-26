# `String.split` と `String.join` を入力長に線形にし、`String.length` を O(1) にする（string-split-linear）

## 手順書

### 背景

`String.split` と `String.join` は prelude（`stdlib/string.hd`）で `slice` と `find` から組んであり、片ごとに先頭から走査する。行数を 2 倍にすると時間が約 4 倍になる。`String.length` も UTF-8 を毎回先頭から数え直す（`runtime/hado_runtime.c` の `hado_string_length`）。§19 は String を不変の UTF-8、§55 は索引をスカラ値の単位と定めるので、索引の意味は変えずに、長さをヘッダへ持たせ、`split` と `join` を C の組み込みにする。

### 決定

issue の案 A（仕様不変）を採る。

- `HadoString` に `scalars` を足し、String を作る全ての経路がスカラ数を入れる。Bytes を作る経路は `HADO_SCALARS_UNKNOWN` を入れる（`String.length` は未知値なら走査へ落ちる。値は常に正しい）。
- `String.split` を C の `hado_string_split`（`memmem` で 2 走査。重ならない）にし、`String.join` を `hado_string_join` にする。extern 表（`crates/hado/src/runtime_abi.rs`）に `__string_split` / `__string_join` を足す。
- C は `String Array` を返す。prelude が `Array.to_list` で `List` にする。`join` は `String Array` を取り、prelude が `Array.of_list` で渡す。List の `Nil` / `Cons` のタグを C に直書きしないためである。
- §55 に `String.length` が O(1)、`split` と `join` が定数回の走査であることを追記する（節番号は動かさない）。

`memmem`（glibc の Two-Way）を選ぶ理由: 区切りの長さ m が入力長 n に比例して伸びる対照を置くので、m の表を持つ KMP は O(m) の作業領域（m = 5000 万で 400 MB）になる。`memmem` は O(1) の作業領域で線形である。

### 期待値の根拠

- `split` と `join` の意味は prelude 版と同じにする。重なる区切り（`"aaaaa"` と `"aa"` は `["", "", "a"]`）、空の本文、末尾の区切り、空と 1 要素の `join` を直接測る。空の区切りは §55 の `EmptySeparator`（stderr、終了コード 1）。
- 時間は `build` で 1 回だけ実行ファイルを作り、その実行ファイルを測る。比は対照を引いた値で見る。`run` は起動とビルドを計測へ入れてしまい、二乗でも比が 2.5 以下に見える。
- `String.length` は時間の差では走査を隠せるので、runtime のバイト走査を数える計器（`hado_byte_scans`）で判定する。構築のあと・`length` の直前に 0 に戻し、`string_byte_offset` を通らない走査も数える。新しく作った文字列への 1 回の呼び出しで増分が 0 なら走査していない。
- `HadoString *` を返す関数は正規表現でソースから列挙する（関数名を判定器に直書きしない）。返り値の意味は 3 つに分かれ、String はスカラ数がバイト列のスカラ数と一致し、Bytes は未知値を持ち、`String.split` は String の Array を返す。
- 要素が小さい `join` の対照は要素の構築が `join` のコピーより重く、引いた差が測定の分解能に埋もれるので、要素は 800 バイトの固定文字列 1 本を共有する。対照は同じリストを作って `List.length` を取るだけである（形は線形）。二乗の `join` はここで timeout する。

### 成果物

- `runtime/hado_runtime.h` / `runtime/hado_runtime.c` — `scalars` ヘッダ、`hado_scalar_count`、`hado_byte_scans`、`hado_string_split`、`hado_string_join`。
- `crates/hado/runtime_print.c` — `quote_text` と `hado_canonical_float` のスカラ数。
- `crates/hado/src/repr.rs` — 文字列リテラルの長さヘッダの直後にスカラ数ヘッダ。
- `crates/hado/src/runtime_abi.rs` — `__string_split` / `__string_join`。
- `stdlib/string.hd` — `split` と `join` を組み込みの薄い包みに。
- `crates/hado/tests/{runtime_string,r34_bytes,r36_int,r37_world,repr,prelude}.rs` — 配置の移行。
- `scripts/verify-string-split-linear.sh` — 門番（`# tier: full`、`# round: string-split-linear`）。検査 10 本。
- `docs/spec.md` §55 — 計算量の追記。

### 工程

1. 門番を書き、変更前の実装で赤を確かめる（二乗の比と `HadoString` の列挙）。
2. `HadoString` にスカラ数のヘッダを足し、runtime・印字・リテラル・既存の単体テストを移行する。
3. `split` と `join` を C の組み込みにし、prelude と extern 表を差し替える。
4. §55 を追記し、門番を単独で回して緑を確かめる。

### 判定

- `bash scripts/verify-string-split-linear.sh` が `PASSED: 10/10`。
- `cargo test --offline` が赤なし。
- `bash scripts/run-gates.sh --tier quick target/gates-quick` が赤なし。
- `bash scripts/lint-docs.sh` が赤なし。

### 許容される赤

無し。計測は同じ計算機で走る他のラウンドの影響を受けるので、比が閾値をまたいだときは単独で測り直してから赤と判断する（判定は子プロセスの CPU 時間で、対照と対象を交互に測る）。

### スコープ外

- `String.slice`・`String.get` の索引をバイト単位にする案 C（§19/§55 を変える）。
- `Float.to_string` の綴り（`char-and-float-conversions` のラウンド）。
- `List` を C へ渡すためのタグの固定。

### 中断の条件

- `stdlib/string.hd` の差し替えで `split` と `join` の意味が変わったとき。
- `HadoString` の列挙に、分類できない関数が現れたとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §19 | String は不変の UTF-8、索引はスカラ値 | 実行 | — | `crates/hado/tests/runtime_string.rs`、`scripts/verify-string-split-linear.sh` 検査 a |
| §55 | `String.length` は Unicode スカラ値の個数 | 実行 | — | `scripts/verify-string-split-linear.sh` 検査 a・b |
| §55 | `String.length` は O(1) | 型/実行 | — | `scripts/verify-string-split-linear.sh` 検査 b |
| §55 | `String.split` と `String.join` は定数回の走査 | 実行 | — | `scripts/verify-string-split-linear.sh` 検査 e〜i |
| §55 | 空の区切りは `EmptySeparator` | 実行 | `EmptySeparator` | `tests/accept/string_empty_separator/main.hd`、検査 a |

## 実測

変更前の main（`7e5a3dd`）の prelude 実装で、`hado build` した実行ファイルの実行時間。入力は 1 行 1 語、末尾に改行を置かない。`split` は `String.split "\n"`。

```
$ cargo run --offline -q -- build target/perf-scratch/split/main.hd
$ /usr/bin/time -f '%e' target/perf-scratch/split/program < in5000.txt
0.64
$ /usr/bin/time -f '%e' target/perf-scratch/split/program < in10000.txt
2.63
$ /usr/bin/time -f '%e' target/perf-scratch/split/program < in20000.txt
11.18
```

`join` は `Int.to_string` の fold で作ったリストを `String.join ","` する。

```
$ /usr/bin/time -f '%e' target/perf-scratch/join/program < /dev/null
0.14
$ /usr/bin/time -f '%e' target/perf-scratch/join40000/program < /dev/null
0.62
```

門番（変更後）。

```
$ bash scripts/verify-string-split-linear.sh
OK: a semantics: split and join equations and EmptySeparator
OK: b length: String.length scans no bytes and does not slow with length
OK: c layout: runtime unit tests after the HadoString header migration
OK: d abi: every HadoString *-returning function classified and measured
split ascii lines: small=0.699s large=1.947s subtracted 0.327->0.777 ratio 2.38
OK: e perf: split on ASCII lines is linear
split multibyte lines: small=0.466s large=0.966s subtracted 0.141->0.325 ratio 2.31
OK: f perf: split on Japanese lines is linear
g: small=0.491s large=1.054s subtracted 0.294->0.650 ratio 2.21
OK: g perf: split with a growing ASCII separator is linear
h: small=0.686s large=1.395s subtracted 0.339->0.672 ratio 1.98
OK: h perf: split with a growing Japanese separator is linear
join: small=0.323s large=0.661s subtracted 0.293->0.604 ratio 2.06
OK: i perf: join is linear in the element count
control: the control-subtracted ratio judge rejects the issue example that the total ratio accepts
OK: j control: the subtracted ratio judge rejects the issue example
PASSED: 10/10
```

`split` と `join` の意味（検査 a が測る等式の一覧）。

```
split "aaaaa" "aa" == ["", "", "a"]
split "" "," == [""]
split "a,b," "," == ["a", "b", ""]
split "a" "," == ["a"]
split "abc" "-" == ["abc"]
split "あいう" "いう" == ["あ", ""]
split "あ😀い😀" "😀" == ["あ", "い", ""]
join [] "," == ""
join ["a"] "," == "a"
join ["a", "b", "c"] "," == "a,b,c"
join ["あ", "い"] "、" == "あ、い"
join ["a", "b"] "" == "ab"
join (split "a,b," ",") "," == "a,b,"
split "abc" "" -> stderr "EmptySeparator"、終了コード 1
```

`HadoString *` を返す関数の列挙（検査 d が測る）。ソースの正規表現から 25 本を列挙し、String 15 本（`hado_string_slice` `hado_string_trim` `hado_int_to_string` `hado_float_to_string` `hado_char_to_string` `hado_bytes_display` `hado_bytes_of_string` `hado_bytes_to_string_unchecked` `hado_string_concat` `hado_string_join` `make_piece` `quote_text` `hado_quote_string` `hado_quote_char` `hado_canonical_float`）はスカラ数が `hado_scalar_count` と一致し、Bytes 9 本（`bytes_from_c_string` `hado_argv_get` `hado_env_get` `hado_cwd` `hado_io_take_bytes` `hado_io_dir_get` `hado_bytes_alloc` `hado_bytes_fill_unchecked` `grow_io_bytes`）は未知値を持ち、`hado_string_split` は String の Array を返す。

### 仕様への疑義

issue の完了条件は `String.length (Char.to_string 'あ') == 1` と `String.length (Float.to_string 0.1) == 3` を要求する。

```
$ printf 'def main = Char.to_string '"'"'あ'"'"'\n' > main.hd && target/debug/hado run main.hd
error[E204]: unbound name Char
$ printf 'def main = String.length (Float.to_string 0.1)\n' > main.hd && target/debug/hado run main.hd
19
```

`Char` のモジュールは §55 の member に無く、`Char.to_string` は存在しない。`Float.to_string 0.1` は `%.17g` で 19 文字であり、`print 0.1`（`hado_canonical_float`）の `0.1` の 3 文字ではない。どちらも本ラウンドの成果物（長さのヘッダと `split` / `join`）の外にある。仕様は変えず、門番は C の `hado_char_to_string`（スカラ数 1）と、ASCII の `Float.to_string` でスカラ数がバイト数と一致すること（検査 d）を測る。`Float.to_string` の綴りは `char-and-float-conversions` のラウンドで扱う。
