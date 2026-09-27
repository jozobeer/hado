# `Char` module と `Float.parse` `Float.of_int`（char-and-float-conversions）

## 手順書

### 背景

`String.get` は Char を返すが、Char から先へ進む member が無い。`Float` も `to_string` だけで、`Float.to_string` は `runtime/hado_runtime.c` の `%.17g` のままで、§54 の正準形になっていない。`main` の Float の印字だけが `CanonicalFloat` を通る（`crates/hado/src/mono.rs` の `display_value` と `derive_scalar_printer`）。`Int.parse` の受理文法は §55 にあるが、Float の受理文法は無い。issue の案をそのまま実装する。

直前に着地した `string-split-linear`（R1）は `HadoString` にスカラ数のヘッダを足し、`hado_char_to_string` と `hado_float_to_string` のスカラ数を入れた。R1 は `String.length (Float.to_string 0.1) == 3` と `String.length (Char.to_string 'あ') == 1` を完了条件に持っていたが、`Char` module と正準形の `Float.to_string` が無いので実装できず、依存しない部分だけを着地して「仕様への疑義」に残した。このラウンドでこの 2 つを引き取り、`scripts/verify-string-split-linear.sh` に検査を足して赤→緑を測る。

### 意図

- 使う人: hado で文字列から Char を取り出して加工する書き手と、Float の入出力をする書き手（推定）。
- 場面: `String.get` の結果をコードポイントへ、文字列を Float へ、Int を Float へ変換するとき。
- 理由: 変換の綴りと失敗の `message` を §55 に定め、`Float.to_string` を §54 の正準形にして往復を可能にする。

### 仮説（一周目のモデル化）

- 境界: 変換はすべて prelude の member（`Char.to_int` `of_int` `to_string`、`Float.parse` `of_int`）で、実体は C の組み込み。`Char` の値は常に Unicode スカラ値で、`of_int` だけがその不変条件を検査する。
- 型: `Char.of_int` と `Float.parse` は `{Error}` を返し、失敗は prelude の `raise { message = "InvalidScalar" | "InvalidFloat" }` で表す（`Int.parse` と同じ is_valid + unchecked の 2 関数）。他の 4 本は `Pure`。
- データフロー: `Float.parse` は文法検査 → `strtod` → 「無限大」と「0 でないのに 0.0」を弾く、の順に判定する。`strtod` は glibc で正しく丸める。
- 印字: `Float.to_string` は `hado_canonical_float` と同じ `hado_format_float`（Rust の `{}` と `.0` の補い）を呼ぶ。`main` の印字は既にこの経路なので、両者は同じ正準形になる。
- `Float.of_int` は C の `(double)int64` で、IEEE 754 の round-to-nearest, ties-to-even になる（プラットフォームの規則）。

### 期待値の根拠（判定器）

- issue の完了条件をそのまま検査にする。件数・期待値は門番側で計算し、コーパスのファイル名や行を埋め込まない。
- 正準形は Python の `repr`（往復可能な最短の 10 進桁）から指数を使わない位取りへ展開して作り、判定器側で計算する。
- `Float.parse` の合否は「文法一致」と「有限」と「0 でない桁があるのに 0.0 でない」から計算し、受理した値は Python の `float()` の丸めと比べる。境界（2×10^308、最大有限値、10^-324、2^-1075、3×10^-324、5×10^-324）は十進の綴りを生成して測る。
- `Float.of_int` の期待は Python の `float(n)`（round-to-nearest, ties-to-even）から計算する。
- `Char.of_int` の受理域 `0..=0xD7FF` と `0xE000..=0x10FFFF` は判定器側で計算し、受理値で `to_int ∘ of_int` の往復、`to_string` の UTF-8 バイト列、`String.length` を測る。

### 成果物

- `runtime/hado_runtime.c` / `runtime/hado_runtime.h` — `hado_float_to_string` を `hado_format_float` に、`hado_char_to_int`、`hado_char_of_int_is_valid`、`hado_char_of_int_unchecked`、`hado_float_parse_is_valid`、`hado_float_parse_unchecked`、`hado_int_to_float`。
- `crates/hado/src/runtime_abi.rs` — 6 つの組み込みの宣言（名前・機械語型・hado 側の型）。
- `stdlib/char.hd`（新設）、`stdlib/float.hd`、`crates/hado/src/lib.rs` の prelude 連結。
- `docs/spec.md` §54（正準形の参照）・§55（member の型・失敗の表・受理文法と丸め）。
- `tests/accept/char_*`・`tests/accept/float_*` — 受理と `// expect-error:`。
- `scripts/verify-char-and-float-conversions.sh`（`# tier: full`、`# round: char-and-float-conversions`）、`scripts/gate-tiers.lock`。
- `scripts/verify-string-split-linear.sh` — R1 の残り 2 検査。
- `docs/rounds/char-and-float-conversions.md`。

### 工程

1. 門番を書き、変更前の実装で赤を測る（E204 の 3 つ、`Float.to_string 0.1` が `0.10000000000000001`、`String.length` が 19）。
2. `hado_float_to_string` を `hado_format_float` に替え、`Float.to_string` の正準形と `main` の印字の一致を測る。
3. `Char` の 3 本と `Float.parse`・`Float.of_int` の組み込みを足し、prelude と extern 表を追随する。
4. 仕様 §54・§55 とコーパスを足し、門番を単独で回して緑を測る。
5. R1 の門番へ 2 検査を足し、main 取り込み後に lock を作り直す。

### 判定

issue の完了条件ごとの検査（門番のどの検査が測るか）。

1. §55 の失敗の表と member の型、受理・`expect-error` コーパス → 検査 g（§55 の関数集合と失敗の表）、検査 h（コーパスの実走）。
2. `Char.of_int` の受理 4 値と拒否 4 値、`to_int ∘ of_int` の往復、`to_string` の UTF-8、BMP と BMP 外 → 検査 a。
3. `Float.to_string` の正準形（`0.0` `-0.0` `0.1` 10^300、`e`/`E` を含まない）、`main` の印字、`nan` `inf` `-inf` → 検査 b（`Float.to_string`）と検査 c（`main`）。
4. `Float.of_int` の 7 値の丸め → 検査 f。
5. `Float.parse` の受理文法と往復、文法外の入力、丸め、オーバーフローとアンダーフロー、境界 → 検査 d（受理と往復）と検査 e（拒否と境界）。
6. `"9007199254740993.0"` の ties-to-even、2×10^308 とその負、10^-324、2^-1075、3×10^-324、5×10^-324 → 検査 e。
7. R1 の残り: `String.length (Float.to_string 0.1) == 3` と `String.length (Char.to_string 'あ') == 1` → 検査 a・b と `scripts/verify-string-split-linear.sh` 検査 k。

着地の判定は次とする。

- `bash scripts/verify-char-and-float-conversions.sh` が `PASSED: n/n`。
- `bash scripts/verify-string-split-linear.sh` が `PASSED: n/n`。
- `cargo test --offline` が赤なし。
- `bash scripts/run-gates.sh --tier quick target/gates-quick` が赤なし。
- `bash scripts/lint-docs.sh` と `bash scripts/verify-spec-contracts.sh` が赤なし。

### 許容される赤

無し。

### スコープ外

- `Char` の大文字化などの Unicode の性質、`Float` の他の member（`is_nan` など）。
- `hado_print_float` の削除（コンパイラからの呼び出しが無い宣言だけ。§54 の印字の対象は `Float.to_string` と `main` の経路）。
- `main` の印字そのものの変更（既に `CanonicalFloat` を通る。測る対象であって直す対象ではない）。
- 拒否コーパス（`// error:`）。失敗は実行時の `{Error}` で、§56 に静的コードが無い。

### 中断の条件

- `Float.parse` の丸めが glibc の `strtod` と判定器側の期待で食い違ったとき。
- `Float.to_string` と `main` の印字が同じ `hado_format_float` を通るのに食い違ったとき。

### 実装から分かったこと

仮説と突き合わせる。

- 当たり: `main` の印字は既に `CanonicalFloat`（`hado_format_float`）を通っていた。検査 c は変更前に緑で、直す対象は `Float.to_string` の経路だけだった。`hado_float_to_string` を同じ整形器に替えると、`Float.to_string` と `main` の印字が `0.0` `-0.0` `0.1` 10^300 最大有限値 `nan` `inf` `-inf` で一致した。
- 当たり: `Float.parse` は文法検査 → `strtod` → 「無限大を弾く」「0 でないのに 0.0 を弾く」で足りる。glibc の `strtod` は round-to-nearest, ties-to-even で、3×10^-324 と 5×10^-324 は ERANGE が立っても非零なので受理し、10^-324 と 2^-1075 は 0 に丸まるので拒否する。境界の期待は判定器側の `float()` と一致した。
- 当たり: `Int.parse` の is_valid + unchecked の 2 関数の形は `Char.of_int` と `Float.parse` にそのまま使えた。extern 表に足したのは機械語型がスカラの 6 本だけで、新しい ABI の形は要らなかった。
- 外れではなく設計に還元した点:
  1. `scripts/verify-m7.sh` は §55 の関数集合から `stdlib/<module 小文字>.hd` を導いて `module` / `implement` と宣言・定義の 2 つを見る。§55 に `Char` を足した時点で `stdlib/char.hd` が必須になり、仕様が単一の情報源として効いた。
  2. `scripts/verify-m1-corpus.sh` 検査 6 は §55 の関数集合を読んで、member 名を含む受理コーパスに `// needs: prelude` を要求する。`Char` と `Float` の member を足すと、それを使う受理はすべてタグが要る。
  3. `hado_float_to_string` が外部シンボル `hado_format_float` を参照するようになったので、runtime を単体で `#include` して `-lgc` だけでリンクする探針（`r37_world.rs` の io_probe、`verify-r50.sh` の fault probe、`verify-string-split-linear.sh` の length_probe）がリンクできなくなった。Float の整形を測らない探針に `%.17g` の控えを置いた。共有の費用は、探針のリンク方法を変えるより小さかった。
  4. 既存の `crates/hado/tests/emit_mono.rs` は `__float_to_string 0.0` に `0` を期待していた。§54 の `0.0` に直した。`Float.to_string` の正準形を変える以上、この期待は仕様側に合わせる。
  5. 門番の綴りで 3 検査が赤になった。hado の無名関数は `fn(...)` で、負の Int / Float のリテラルは適用の引数に裸で置くと引き算に読まれる（`Char.of_int -1`）。仕様どおりで、直したのは門番の綴りである。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §54 | Float は往復可能な最短の 10 進桁を指数を使わず位取りで並べ、小数点の両側に最低 1 桁 | 実行 | — | `scripts/verify-char-and-float-conversions.sh` 検査 b・c |
| §54 | NaN と無限大は `nan` `inf` `-inf` | 実行 | — | 検査 b・c |
| §55 | `Char.to_int` `of_int` `to_string` の型と `InvalidScalar` | 型 | — | 検査 a、`tests/accept/char_of_int` |
| §55 | `Float.parse` の受理文法 `"-"? digit+ "." digit+` と丸め、`InvalidFloat` | 型/実行 | — | 検査 d・e、`tests/accept/float_parse_accepted` |
| §55 | `Float.of_int` の i64 から f64 への丸め | 型/実行 | — | 検査 f、`tests/accept/float_of_int` |
| §55 | 失敗は実行時の `{Error}`（§56 にコードが無い） | 実行 | — | `tests/accept/float_parse_invalid` |

## 実測

変更前の実装（main `d5ff67c`）で赤を測る。`Char` は E204、`Float.parse` と `Int.to_float` は module member が非公開（`Float.of_int` を名乗る前の issue の現象）。

```
$ cargo run --offline -q -- check target/scratch/probe.hd    # def main = Char.to_int 'a'
probe.hd:1:12: error[E204]: unbound name Char
$ cargo run --offline -q -- check target/scratch/probe.hd    # def main = Float.parse "1.5"
probe.hd:1:18: error[E204]: module member is not exported
$ cargo run --offline -q -- check target/scratch/probe.hd    # def main = Int.to_float 3
probe.hd:1:16: error[E204]: module member is not exported
$ cargo run --offline -q -- run target/scratch/probe.hd      # def main = Float.to_string 0.1
0.10000000000000001
$ cargo run --offline -q -- run target/scratch/probe.hd      # def main = String.length (Float.to_string 0.1)
19
```

先に門番を書いて赤を測る。検査 c（`main` の印字）は既に緑で、`main` の経路は
`CanonicalFloat` を通るので直す対象ではないことがここで分かる。検査 i（判定器の
較正）も緑で、期待値の計算そのものは正しい。

```
$ bash scripts/verify-char-and-float-conversions.sh
NG: a char: boundary scalars, round trip, UTF-8 bytes and InvalidScalar
NG: b float_to_string: §54 canonical spellings, no exponent, nan and inf
main print: 8 canonical spellings through the CanonicalFloat path
OK: c main print: the same canonical spellings through CanonicalFloat
NG: d parse accepted: grammar, round trip and correct rounding
NG: e parse rejected: grammar, overflow and underflow are InvalidFloat
NG: f of_int: i64 to f64 round to nearest, ties to even
NG: g spec: §55 inventory, signatures, failure rows and grammar
NG: h corpus: the §55 accept cases with expect and expect-error run
judge: canonical round-trips, no exponent, and parse_expected agrees
OK: i judge: canonical and parse_expected are calibrated
PASSED: 2/9
```

実装後、main を取り込んだ木で回す。`verify-string-split-linear.sh` は 1 回目に検査 i
（join の線形性）だけが小さい側 0.198 s で測定の下限 0.200 s を割って赤になった。
このラウンドは `String.join` もその経路も触っておらず、同じ門番を単独で測り直すと
`small=0.208s` で緑になったので、他のラウンドと同じ計算機で走ったことによる揺れと
判断する（R1 の門番自身の「許容される赤」と同じ扱い）。

```
$ bash scripts/verify-char-and-float-conversions.sh
...
OK: a char: boundary scalars, round trip, UTF-8 bytes and InvalidScalar
OK: b float_to_string: §54 canonical spellings, no exponent, nan and inf
OK: c main print: the same canonical spellings through CanonicalFloat
OK: d parse accepted: grammar, round trip and correct rounding
OK: e parse rejected: grammar, overflow and underflow are InvalidFloat
OK: f of_int: i64 to f64 round to nearest, ties to even
OK: g spec: §55 inventory, signatures, failure rows and grammar
OK: h corpus: the §55 accept cases with expect and expect-error run
OK: i judge: canonical and parse_expected are calibrated
PASSED: 9/9

$ cargo test --offline
（赤なし。0 failed）

$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 41/41 passed; red: none

$ bash scripts/lint-docs.sh
PASSED: 16/16

$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2

$ python3 <verify-m7 の inventory 部分>
prelude inventory: 73/73

$ bash scripts/verify-string-split-linear.sh    # 2 回目
...
OK: i perf: join is linear in the element count
OK: k lengths: R1 leftovers Float.to_string 0.1 and Char.to_string あ
PASSED: 11/11
```

`bash scripts/corpus-lock.sh --check` の差分は `stdlib/char.hd` の追加、
`stdlib/float.hd` の変更、`tests/accept` の `char_*` と `float_*` の 10 件だけで、
すべてこのラウンドの成果物だった。main 側のコーパスと stdlib の変更は無い。

### 審査後（指摘への対応）

審査の fatal 2 件を門番と受理コーパスに足し、§55 の 1 文の曖昧さを消した。

- 検査 a は `Char.to_string` の UTF-8 バイト列を、受理した 6 値（`0`・`55295`・
  `57344`・`1114111`・`12354`・`128512`）すべてで照合する。長さだけでは、境界で
  誤った続きバイトを出す符号化器でも通ってしまう。
- 検査 d に `-0.(0×323)3` と `-0.(0×323)5`、検査 e に `-0.(0×323)2` を足した。
  実装は `strtod` を符号込みで呼ぶので、加えた探針はそのままで緑だった（この
  部分が赤だったのは実装前の門番全体 2/9 のときだけ）。較正 i にも負側の境界を
  入れ、`parse_expected` が `-0.(0×323)2` を拒否し `-0.(0×323)5` を受理することを
  判定器側で確かめる。
- 受理コーパスに `char_to_string_bytes`・`float_parse_negative_min_subnormal`・
  `float_parse_negative_underflow` の 3 件を足した。
- §55 の `Char.to_string` の 1 文を「UTF-8 バイト列を内容とする String を返し、
  引用符もエスケープも付けない（第54節の最上位の Char と同じ生の文字で、入れ子の
  `'a'` の形ではない）」に直した。節番号は動かしていない。

```
$ bash scripts/verify-char-and-float-conversions.sh
parse accepted: 14 spellings and 5 round trips
OK: d parse accepted: grammar, round trip and correct rounding
parse rejected: 14 grammar, 2 overflow, 6 underflow
OK: e parse rejected: grammar, overflow and underflow are InvalidFloat
corpus: 11 §55 cases run, errors={'InvalidScalar': 1, 'InvalidFloat': 4}
OK: h corpus: the §55 accept cases with expect and expect-error run
PASSED: 9/9

$ bash scripts/verify-string-split-linear.sh
join: small=0.200s large=0.397s subtracted 0.184->0.365 ratio 1.99
OK: i perf: join is linear in the element count
OK: k lengths: R1 leftovers Float.to_string 0.1 and Char.to_string あ
PASSED: 11/11
（1 回目は検査 i が small=0.198s で下限 0.200s を割って赤。荷重 4.56 の下で
  下限付近を揺れる。測り直すと上の緑。R1 の門番の「許容される赤」と同じ扱い）

$ cargo test --offline
（赤なし。0 failed）

$ bash scripts/lint-docs.sh
PASSED: 16/16

$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2

$ bash scripts/corpus-lock.sh --check
（差分は char_to_string_bytes・float_parse_negative_min_subnormal・
  float_parse_negative_underflow の 3 件だけ。作り直す）
```
