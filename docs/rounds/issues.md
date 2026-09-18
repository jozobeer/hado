# 次版の issue 候補を決める（issues）

開始 2026-09-17。

## 手順書

### 背景

20 行の CLI を試した結果、既存の wants だけでは拾えていなかった標準ライブラリ、診断、エディタ統合の不足が見つかった。候補を実装可能な単位へ分け、次版で先に閉じる範囲を決める。

### 決定

- 候補は `docs/issues/<slug>.md` に現象、原因、案、完了条件、由来を置く。着手後は同じ slug のラウンドファイルが手順と実測を引き継ぐ。
- v0.5 は `prelude-tail-recursion`、`highlight-treesitter`、`lsp-minimal` の順に進める。
- 完了条件は実装経路を直接判定し、依存方向、位置情報、文字境界、道具の再現性を固定する。

### 期待値の根拠

| 対象 | 期待 | 根拠 |
|---|---|---|
| issue 候補 | 現象と原因をソースまたは実測で特定し、独立して着地できる完了条件を持つ | `docs/issues.md` |
| v0.5 | prelude、highlight、LSP の順で着手する | `docs/plan.md` 35 節 |
| 文書 | 現行の節参照とラウンド規約に適合する | `scripts/lint-docs.sh` |

### 成果物

`docs/issues.md`、`docs/issues/*.md`、`docs/plan.md` 35 節、本ファイル。

### 工程

- dogfood の実測から候補を切り出す。
- 各候補の現象、原因、最小案、完了条件を既存コードと仕様に照らす。
- 次版の範囲と順序を `docs/plan.md` に固定する。
- 完了条件と依存関係を再検証する。

### 判定

- `bash scripts/lint-docs.sh` が全件通る。
- `git diff --check` が通る。
- v0.5 の各 slug に候補ファイルがあり、着手後の記録先が同じ slug の `docs/rounds/<slug>.md` と明記されている。

### 許容される赤

終了時に許容される赤は無い。

### スコープ外

候補の実装、仕様・runtime・stdlib・corpus の変更。

### 中断の条件

候補の根拠をソースまたは実測で確認できない場合と、候補の判断が現行仕様と衝突する場合は止めて報告する。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （文書運用） | issue 候補は slug で識別し、着手後は同じ slug のラウンドが手順と実測を持つ | 文書 | 成功 | `lint-docs.sh` |
| （文書運用） | issue の原因は `file:line` を指す | 文書 | 成功 | `lint-docs.sh` 検査 8 |

## 実測

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -2
OK: R38 spec sentences are pinned verbatim once each
PASSED: 7/7
```

### レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 7/7

nvim --version | head -1
NVIM v0.12.5
# カスタム filetype "hado" に lua parser を割り当て、-u NONE（nvim-treesitter 無し）で開く
# filetype=hado / parser=true / active_before_start=false / active_after_start=true

# query は runtimepath の queries/<言語>/<名前>.scm として解決される
RT=$(nvim --headless -u NONE -i NONE -c 'lua io.write(vim.env.VIMRUNTIME)' -c 'qa!' 2>/dev/null)
grep -n "queries/%s/%s.scm" "$RT/lua/vim/treesitter/query.lua"
159:  local query_path = string.format('queries/%s/%s.scm', lang, query_name)

cargo run --offline -q -- check --format json tests/reject/E101/main.hd
main.hd:6:40: error[E101]: branches cannot be nested inside another branch
cargo run --offline -q -- check --format json tests/reject/E505/main.hd
{"path": "main.hd", "line": 4, "col": 7, "code": "E505", "message": "binding unused is unused", "canonical": null, "related": null}
```

### 追加レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -2
OK: each issue cause cites a file:line anchor
PASSED: 8/8

# 検査 8 が空でないこと。原因に file:line の無い一時 issue を docs/issues/ に置くと赤になる
NG: each issue cause cites a file:line anchor
PASSED: 7/8

# def の形（付録C の parameters）。1 つの括弧と引数だけの注釈は正当で、括弧の連続だけ E100
cargo run --offline -q -- check target/pr5/t1/main.hd   # def f x : Int => x
exit=0
cargo run --offline -q -- check target/pr5/t2/main.hd   # def f (a: Int): Int => a
exit=0
cargo run --offline -q -- check target/pr5/t3/main.hd   # def f (a: Int) (b: Int): Int => a
main.hd:1:16: error[E100]: expected =>, found (
```

### 四次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 8/8

grep -n '%.17g' runtime/hado_runtime.c
462:  check_stdout(printf("%.17g\n", value) >= 0);
514:  int length = snprintf(buffer, sizeof(buffer), "%.17g", value);

sed -n '9p;31p' stdlib/list.hd
def A sort(values: A List, comparator: Fn(A, A => Int)): A List
def rec sort values comparator => if values | [] => [] | [_] => values | [_, _, ..._] => odds values |> sort comparator |> merge (evens values |> sort comparator) comparator

grep -n 'def main = 1 + 2' crates/hado/tests/walk.rs
15:        ("def main = 1 + 2", "3"),
grep -n 'def main = \$int_value' scripts/verify-r29.sh
108:if ! build_kind int "def main = $int_value" \
grep -n 'def main = Int.abs' scripts/verify-r43.sh
245:    source = f'def main = Int.abs {expression(value)}\n'
```

### 五次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 8/8

grep -n 'fn execute' crates/hado/src/main.rs
266:fn execute(input: &Path, args: &[String]) -> Outcome {
```

### 六次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 8/8

# E101 は腕に直接置いた Boolean branch 以外も含む（tests/reject/E101 は fn の腕形式が引数の中）
sed -n '6p' tests/reject/E101/main.hd
def f (x: Choice) => if x | A => apply fn(| A => 1 | B => 2) A | B => 0
```

### 七次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9        # 検査 9 を足した: issue の現象がコードフェンスか表を持つ

# 受理コーパス最大行と自作ツールの行数（docs/issues/dogfood-text-tool.md の現象）
wc -l tools/gates/main.hd tools/diagnostics/main.hd
  23 tools/gates/main.hd
  30 tools/diagnostics/main.hd
find tests/accept -name main.hd -exec wc -l {} + | sort -rn | grep -v total | head -1
   24 tests/accept/shared_functor/main.hd

# i64 の最小値はリテラルに書けず E112、構成すれば通る（char-and-float-conversions の Float.of_int）
$ cargo run --offline -q -- check main.hd   # def main = -9223372036854775808
main.hd:1:13: error[E112]: integer out of range: 9223372036854775808
$ cargo run --offline -q -- run main.hd     # def main = (-9223372036854775807 - 1)
-9223372036854775808

# 連想リストの線形探索: 語彙 V を 2 倍にすると時間も約 2 倍（50 万回、毎回最後の鍵まで走査）
/usr/bin/time -f '%e s' cargo run --offline -q -- run map/main.hd      # 語彙 250
0.96 s
/usr/bin/time -f '%e s' cargo run --offline -q -- run map2x/main.hd    # 語彙 500
1.77 s
grep -rn 'implement Map' stdlib/ | wc -l
0

# Neovim は filetype=hado / parser=true だが、start() まで highlighter は動かない
nvim --headless -u NONE -i NONE -l hl.lua
filetype=hado
parser=true
active_before_start=false
active_after_start=true
```

### 八次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# 検査 8 を実ファイルと行数で照合するようにし、負の探針で赤を確認する
$ sed -n '23p' docs/issues/zz-probe.md     # 原因が missing.rs:999
AssertionError: zz-probe.md: 原因 anchor file does not exist: missing.rs:999
NG: each issue cause cites a file:line anchor
$ sed -n '23p' docs/issues/zz-probe.md     # 原因が crates/hado/src/emit.rs:99999
AssertionError: zz-probe.md: 原因 anchor line is out of range: crates/hado/src/emit.rs:99999 (file has 773 lines)

# Int は 10 進のみ（Char.of_int の境界は 10 進で書く）
sed -n '214p' docs/spec.md
Int は 10 進のみとする。i64 の範囲を超えるリテラルは E112 とする。

# log_error を足すと既存の網羅 handler Io は腕が 1 つ足りなくなる（verify-r37 の 5 腕）
sed -n '205p' scripts/verify-r37.sh
io_arms = ['once read_file _ => Bytes.of_string "virtual"', 'once write_file _ _ => ()', 'once log _ => ()', 'once read_dir _ => ["virtual"]', 'once read_stdin => Bytes.of_string "virtual"']
```

### 九次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# --format json は check だけでなく build / run / test も受ける（§58.4）
sed -n '58p' crates/hado/src/main.rs
            "--format" if matches!(command, "check" | "build" | "run" | "test") => {

# 既存の col は Unicode スカラ値の数（byte 数ではない）
sed -n '242,244p' crates/hado/src/lexer.rs
        } else {
            self.col += 1;
        }

# verify-r54 は main = 11 / 22 / 33 を成功するテストとして扱っている
sed -n '60,61p' scripts/verify-r54.sh
    'b_test.hd': 'def main = 22\n',
    'a_test.hd': 'def main = 11\n',

# Map の find は既存の List.head と同じ V Option を返せる
sed -n '1p' stdlib/option.hd
type T Option = | None | Some(T)
sed -n '10p' stdlib/list.hd
def A head(values: A List): A Option Pure
```

### 十次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# Int.parse は先頭の - だけを読み、先行 0 は飛ばす（Float.parse の文法を揃える根拠）
sed -n '481,482p' runtime/hado_runtime.c
static IntParseResult parse_int_digits(const HadoString *string, size_t index) {
  while (index < string->length && string->bytes[index] == '0') ++index;
sed -n '495,497p' runtime/hado_runtime.c
  int negative = string->bytes[0] == '-';
  size_t start = (size_t)negative;
  if (start == string->length) return (IntParseResult){0, 0};

# HadoString を手で組んで cast する runtime 門番（bytes の位置が動くと壊れる）
grep -rn 'unsigned char bytes\[' crates/hado/tests/*.rs | head -3
crates/hado/tests/r34_bytes.rs:217:print("typedef struct { size_t length; unsigned char bytes[%d]; } Utf8Probe;" %
crates/hado/tests/r34_bytes.rs:262:    struct { size_t length; unsigned char bytes[3]; } valid = {3, {0xe3, 0x81, 0x82}};
crates/hado/tests/r34_bytes.rs:263:    struct { size_t length; unsigned char bytes[2]; } two = {2, {72, 105}};

# plan.md は L4 で --format json に終端を足す（suggestion と同時に 10 フィールド）
sed -n '1289p' docs/plan.md
| L4 | 診断の位置 | `--format json` に終端の位置を足す | §58.4 |
```

### 十一次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 を閉じた非空のフェンスか完備した表に強めた

# 検査 9 の負の探針: 未閉じフェンス、コマンドだけのフェンス、ヘッダだけの表、孤立したパイプの 4 種が赤
AssertionError: 現象 has no closed nonempty fence or complete table: zz-probe.md
NG: each issue 現象 carries measured evidence in a fence or table

# String の比較は hado_string_compare を通る（Int は derive_scalar_compare が直接 emit する）
sed -n '254,257p' crates/hado/src/runtime_abi.rs
            Self::StringCompare => &Declaration {
                symbol: "hado_string_compare",
                parameters: &[AbiType::Pointer, AbiType::Pointer],
                result: Some(AbiType::I64),
sed -n '3411,3413p' crates/hado/src/mono.rs
    fn derive_scalar_compare(&mut self, ty: Type) -> (FunctionId, Signature) {
        let signature = Signature {
            parameters: vec![ty.clone(), ty.clone()],
```

### 十二次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# 検査 9 の負の探針: 空セルの見出し、列数の不一致、here-doc だけのフェンス、コメントだけの出力の 4 種が赤
AssertionError: 現象 has no closed nonempty fence or complete table: zz-probe.md
NG: each issue 現象 carries measured evidence in a fence or table

# 文字列リテラルは runtime の確保ではなく直列化される（スカラ数を足すと位置が動く）
sed -n '219,220p;237,238p' crates/hado/src/repr.rs
pub fn string_literal_data(value: &str, pointer_bytes: u32, order: ByteOrder) -> Result<Vec<u8>, String> {
    let mut bytes = match pointer_bytes {
    bytes.extend_from_slice(value.as_bytes());
    Ok(bytes)
sed -n '559p' crates/hado/src/emit.rs
            let bytes = crate::repr::string_literal_data(value, module.target_config().pointer_type().bytes(), order)?;
sed -n '146,148p' crates/hado/tests/repr.rs
    use hado::repr::{string_literal_data, ByteOrder};
    for (width, order, prefix) in [
        (4, ByteOrder::Little, vec![5, 0, 0, 0]),
```

### 十三次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# 検査 9 の負の探針: 継続行だけのコマンド、区切りが : だけの表が赤
AssertionError: 現象 has no closed nonempty fence or complete table: zz-probe.md

# 検査 8 の負の探針: リポジトリの外へ出る anchor が赤
AssertionError: zz-probe.md: 原因 anchor escapes the repository: crates/../../AGENTS.md:1
```

### 十四次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# コメントは // だけ（-- と # は E104、ブロックコメントは無い）
sed -n '226p' docs/spec.md
コメントは `//` による行コメントのみとする。`--`（v0.1 の記法）と `#` を含む他の記法によるコメントは E104 とする。

# --show-all は plan 35 の L1 に移り、wants の 7 は消えた
grep -n 'show-all' docs/plan.md | cut -c1-60
1286:| L1 | 診断の件数 | `--show-all` を opt-in で足す。同じ段階の全件を
grep -c '^7\.' docs/wants.md
0
```

### 十五次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9
```

### 十六次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 の表は、全てのデータ行が placeholder でないことを要求する

# 正しいエスケープ（文字列 \n \t \\ \"、文字 \n \t \\ \'）
sed -n '2334,2337p' docs/spec.md
char            := "'" (char_escape | 文字) "'"          ちょうど 1 文字（E111）
char_escape     := "\n" | "\t" | "\\" | "\'"
string          := '"' (string_escape | 文字)* '"'
string_escape   := "\n" | "\t" | "\\" | '\"'           これ以外は E110

# .hdi は project loader が同じ根の source として読む（単体では CLI の入口にできない）
sed -n '232p;344p' crates/hado/src/project.rs
            && matches!(path.extension().and_then(|extension| extension.to_str()), Some("hd" | "hdi")) {
    let interface = path.extension().is_some_and(|extension| extension == "hdi");
```

レビューの `first:100` は 103 スレッドで頭打ちになるので、以後は `pageInfo` で全件を辿って未解決を数える。

### 十七次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 の表は、全てのセルが placeholder でないことを要求する

# 現象の計測を cargo run 経由で取り直した（起動と run のビルドを含む 5 回の中央値）
target/pr5/bench/time.sh の出力
split 5000: median 0.844 s (min 0.840 max 0.849)
split 10000: median 2.963 s (min 2.921 max 2.989)
split 20000: median 11.796 s (min 11.694 max 11.914)
join 20000: median 1.092 s (min 1.047 max 1.157)
join 40000: median 4.103 s (min 3.936 max 4.226)

# split / join の意味を実測（重なる区切り、空の本文、末尾の区切り、空と 1 要素の join）
$ cargo run --offline -q -- run edge.hd
||a

a|b|

a
$ cargo run --offline -q -- run empty.hd; echo exit=$?
exit=1
# stderr: EmptySeparator

# §47 は 1 つの契約に複数の実装を許す
sed -n '1684p' docs/spec.md
同じ契約を複数の実装が満たせる。
```

### 十八次レビュー対応（2026-09-17）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 8 は範囲 anchor の終端を、検査 9 は | と && の継続行を数える

# 検査 8 の負の探針: 範囲の終端が範囲外、逆順の範囲が赤
AssertionError: zz-probe.md: 原因 anchor range end is out of range: crates/hado/src/emit.rs:1-999999 (file has 773 lines)
AssertionError: zz-probe.md: 原因 anchor range is reversed: crates/hado/src/emit.rs:5-2

# 検査 9 の自己検査に | と && の継続行の探針を足した（赤になることを実行時に確認）

# コンパイラ側の印字も HadoString を組む（length だけを設定する）
sed -n '13,17p;39,45p' crates/hado/runtime_print.c
static HadoString *quote_text(const HadoString *value, unsigned char quote) {
  if (value->length > (SIZE_MAX - sizeof(HadoString) - 2) / 2) abort();
  HadoString *result = hado_alloc(sizeof(HadoString) + 2 * value->length + 2);
  if (result == NULL) abort();
  size_t length = 0;
HadoString *hado_canonical_float(double value) {
  unsigned char bytes[400];
  size_t length = hado_format_float(value, bytes, sizeof(bytes));
  if (length == 0 || length > sizeof(bytes)) abort();
  HadoString *result = hado_alloc(sizeof(HadoString) + length);
  if (result == NULL) abort();
  result->length = length;
```

### 十九次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は引用符と括弧の継続状態をコマンドの行をまたいで持ち越す

# 検査 9 の自己検査: 閉じない " ' ( のあとの PS2 行を出力に数えない（赤を確認）

# §7 の比較は非結合
$ cargo run --offline -q -- check target/pr5/cmp/cmp.hd
cmp.hd:1:18: error[E100]: comparison operators are non-associative; found <
$ echo exit=$?
exit=1

# §54 の NaN と無限大の印字
0.0 / 0.0    -> nan
1.0 / 0.0    -> inf
-1.0 / 0.0   -> -inf

# String を返す runtime の関数（length だけを設定するもの）
grep -n '^HadoString \*hado_' runtime/hado_runtime.c | wc -l
16
grep -c 'HadoString' crates/hado/runtime_print.c
8    # quote_text と hado_canonical_float を含む
```

### 二十次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# E100 と E101 の最小再現を cargo run 経由で取り直した
$ cargo run --offline -q -- check main.hd    # def rec bump (entries: Entry List) (word: String): Entry List =>
main.hd:1:36: error[E100]: expected =>, found (
$ cargo run --offline -q -- check main.hd    # 腕の中の if c => a | b
main.hd:4:12: error[E101]: branches cannot be nested inside another branch

# Char / Float.parse / Int.to_float は §55 に member が無い（同じ invocation で取り直した）
main.hd:1:12: error[E204]: unbound name Char
main.hd:1:18: error[E204]: module member is not exported
main.hd:1:16: error[E204]: module member is not exported

# compare はモジュールの型変数の上で派生され、3 つの型で呼べる
$ cargo run --offline -q -- run main.hd
[true, false, true]

# 入れ子の契約（tests/accept/namespaces/store/store.hdi は main.hd の project に属する）
ls tests/accept/namespaces/ tests/accept/namespaces/store/
tests/accept/namespaces/:
id
main.hd
store

tests/accept/namespaces/store/:
backend
memory_store.hd
store.hdi
```

### 二十一次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は if/for/while/until/case と { } の継続を数える

# 検査 9 の自己検査: 複合コマンドの PS2 行だけのフェンスが赤（実行時に確認）
$ if true; then / > echo x / > fi だけのフェンス
AssertionError: if/then/fi compound and PS2 lines must not count as output
$ for i in a; do / > echo $i / > done だけのフェンス
AssertionError: for/do/done compound and PS2 lines must not count as output

# quote_text は \n \t \ と有効な引用符に \ を 1 つ足す（スカラ数はこの 4 形で +1 される）
sed -n '6,11p;13,26p' crates/hado/runtime_print.c
static unsigned char escape(unsigned char byte, unsigned char quote) {
  if (byte == '\n') return 'n';
  if (byte == '\t') return 't';
  if (byte == '\\' || byte == quote) return byte;
  return 0;
}
static HadoString *quote_text(const HadoString *value, unsigned char quote) {
  if (value->length > (SIZE_MAX - sizeof(HadoString) - 2) / 2) abort();
  HadoString *result = hado_alloc(sizeof(HadoString) + 2 * value->length + 2);
  if (result == NULL) abort();
  size_t length = 0;
  result->bytes[length++] = quote;
  for (size_t i = 0; i < value->length; ++i) {
    unsigned char escaped = escape(value->bytes[i], quote);
    if (escaped) result->bytes[length++] = '\\';
    result->bytes[length++] = escaped ? escaped : value->bytes[i];
  }
  result->bytes[length++] = quote;
  result->length = length;
  return result;
```

### 二十二次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は f() { ... } と function f { ... } の継続を数える

# 検査 9 の自己検査: 関数定義の PS2 行だけのフェンスが赤（実行時に確認）
$ f() { / > echo x / > } だけのフェンス
AssertionError: function definition and PS2 lines must not count as output

# §52 は main の無い .hd をライブラリとして検査し、宣言ディレクトリを独立の根にする
sed -n '1797p;1799p' docs/spec.md
`main` を持たず、他のプログラムから使われることを意図したプログラムをライブラリと呼ぶ。`hado check` に `main` の無い `.hd` を渡すとライブラリとして検査し（第10節、第50節）、`hado build` は受け付けない（E206）。ライブラリの未使用の判定は公開面を根にする（第39節）。
ライブラリであることは `library` 宣言で示す。ディレクトリの 1 つのファイルの外（最初の見出しより前）に `library` を書くと、そのディレクトリがライブラリ根になる。ライブラリ根は独立した根で、パス（第51節）はそこから解く。これにより、ライブラリは利用側の根の下のどの位置に置かれても、内部の参照と外の `def` の規則が変わらない。
```

### 二十三次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は `<<'END-INPUT'` のような区切り語も数える

# 検査 9 の自己検査: hyphen 付き heredoc の本文と終端が出力に数えられない（実行時に確認）
$ cat <<'END-INPUT' / a / END-INPUT だけのフェンス
AssertionError: heredoc with a hyphenated delimiter and its terminator must not count as output

# 検査 8 と 9 を入れたコミットは scripts/lint-docs.sh だけを触る
git show --name-only --format='%h %s' 11a4ece | head -3
11a4ece test: issue の原因アンカーを実ファイルと行数で照合する

scripts/lint-docs.sh
git show --name-only --format='%h %s' 0525f91 | head -3
0525f91 test: issue の現象の実測を lint-docs で検査する

scripts/lint-docs.sh

# main..issues で scripts/lint-docs.sh と docs/issues/・docs/plan.md の両方を触るコミットは無い
for c in $(git rev-list main..origin/issues); do ... done
mixed=0
```

### 二十四次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は継続する heredoc の PS2 行も数える

# 検査 9 の自己検査: 継続する heredoc の後の PS2 行が出力に数えられない（実行時に確認）
$ cat <<EOF | / a / EOF / > wc -l だけのフェンス
AssertionError: a continued heredoc command and its PS2 line must not count as output
$ cat <<EOF | / a / EOF / > wc -l / 1 のフェンスは緑

# 複数の workspace folder の名前が接頭辞になる対照（path 要素で比べる）
file:///work + file:///work/app + 契約 file:///work/application/c.hdi
根は file:///work（文字列の接頭辞では file:///work/app を選び候補 0）
```

### 二十五次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# 最終の木での全走（lint-docs を触ったラウンドなので 1 回だけ回す）
cargo test --offline > target/pr5/gates/cargo-test2.log 2>&1; echo cargo-test2-exit=$?
cargo-test2-exit=0
grep -c FAILED target/pr5/gates/cargo-test2.log
0

# 層別は quick -> core -> full の順に回す
bash scripts/run-gates.sh --tier core --jobs 2 target/pr5/gates/core
TALLY: 561/561 passed; red: none
bash scripts/run-gates.sh --tier full --jobs 4 target/pr5/gates/full2
TALLY: 3742/3742 passed; red: none

# 最初の full 走行は赤だった（wants.md から R51 の記録が消えていた）
bash scripts/run-gates.sh --tier full --jobs 4 target/pr5/gates/full
TALLY: 3741/3742 passed; red: verify-r51.log
NG: 51-d tier, wiring, wants    # AssertionError: wants.md does not record R51
# 記録を残して再走行
bash scripts/verify-r51.sh
PASSED: 5/5

# 付録C の反復する各段の左結合（golden の探針を足した）
a - b - c  -> `-` の下に `-`
a / b / c  -> `/` の下に `/`
a && b && c  -> `&&` の下に `&&`
a || b || c  -> `||` の下に `||`
```

### 二十六次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は `<<` と `<<-` で終端の空白の扱いを分ける

# 検査 9 の自己検査: `<<EOF` のフェンスで空白付きの終端行を出力に数えない
AssertionError: an indented line is body text for << and must not end the heredoc
# `<<-EOF` のフェンスは先頭タブの終端行で閉じるので緑

# 最終の木での全走
cargo test --offline > target/pr5/gates/cargo-test3.log 2>&1; echo cargo-test3-exit=$?
cargo-test3-exit=0
grep -c FAILED target/pr5/gates/cargo-test3.log
0
bash scripts/run-gates.sh --tier full --jobs 4 target/pr5/gates/full3
TALLY: 3742/3742 passed; red: none

# sort の分割は偶数・奇数位置（stdlib/list.hd:22-23）で、:31 の順に整列する
sed -n '22,23p;31p' stdlib/list.hd
def rec odds values => if values | [] => [] | [_] => values | [head, _, ...tail] => [head, ...odds tail]
def rec evens | [] => [] | [_] => [] | [_, next, ...tail] => [next, ...evens tail]
def rec sort values comparator => if values | [] => [] | [_] => values | [_, _, ..._] => odds values |> sort comparator |> merge (evens values |> sort comparator) comparator
```

### 二十七次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# この回は docs/ だけの変更（scripts/lint-docs.sh は二十五次から変えていない）
git diff --stat
 docs/issues/lsp-minimal.md         | 4 ++--
 docs/issues/string-split-linear.md | 4 ++--
 2 files changed, 4 insertions(+), 4 deletions(-)

# 直近の全走（二十六次）は scripts/ を変えていないのでそのまま有効
cargo test --offline -> cargo-test3-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3742/3742 passed; red: none

# quote_text のバイト数の再計算（"あ\n" の引用付き）
1（開き引用符）+ 3（あ の UTF-8）+ 2（\ と n）+ 1（閉じ引用符）= 7 バイト、5 スカラ
```

### 二十八次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は PS2 行で開く heredoc も数える

# 検査 9 の自己検査: 継続行で開く heredoc の本文を出力に数えない
AssertionError: a heredoc opened on a PS2 line must not count its body as output

# 最終の木での全走（検査 9 の実装を変えたので 1 回だけ回す）
cargo test --offline > target/pr5/gates/cargo-test4.log 2>&1; echo cargo-test4-exit=$?
cargo-test4-exit=0
grep -c FAILED target/pr5/gates/cargo-test4.log
0
bash scripts/run-gates.sh --tier full --jobs 4 target/pr5/gates/full4
TALLY: 3742/3742 passed; red: none
```

### 二十九次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 は `name()` の次の行の本文ブレースも数える

# 検査 9 の自己検査: 本文ブレースが次の行の関数定義を出力に数えない
AssertionError: a function header with the body brace on the next line must stay open

# 最大有限値の正準形（指数を使わない 309 桁）
$ cargo run --offline -q -- run main.hd    # main.hd は最大有限値とその負の .0 リテラル
[179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.0, -179769313486231570000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000.0]
# 桁数の確認: 309 桁、末尾の 0 は 292 個、両符号で同じ
pos len 309 zeros_tail 292 canonical_matches_literal True
neg len 309 zeros_tail 292 canonical_matches_literal True

# mono.rs の取り込み後の anchor
grep -n 'fn derive_comparison\|fn derive_scalar_compare' crates/hado/src/mono.rs
3845:    fn derive_comparison(&mut self, ty: Type) -> Result<(FunctionId, Signature), String> {
3962:    fn derive_scalar_compare(&mut self, ty: Type) -> (FunctionId, Signature) {

# 最終の木での全走
cargo test --offline -> cargo-test5-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3742/3742 passed; red: none
```

### 三十次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9    # 検査 9 はバッククォート置換の継続行も数える

# 検査 9 の自己検査: バッククォートの PS2 行を出力に数えない
AssertionError: a backtick substitution across PS2 lines must not count as output

# mono.rs の取り込み後の BytesDisplay の箇所
grep -n 'BytesDisplay' crates/hado/src/mono.rs
4407:        Type::Bytes => (RuntimeFunction::PrintString, Expression::call_runtime(RuntimeFunction::BytesDisplay, vec![value])?),
4532:        Type::Bytes => RuntimeFunction::BytesDisplay,

# 最終の木での全走
cargo test --offline -> cargo-test6-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3742/3742 passed; red: none
```

### 三十一次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# 検査 9 の自己検査（PS2 行・&& の後のブレース・パラメータ展開・複数 heredoc）
AssertionError: a PS2 line after a command is not measured output
AssertionError: a brace group after && must stay open
AssertionError: a parameter expansion across PS2 lines must not count as output
AssertionError: a second heredoc body must not count as output

# 検査 8 の自己検査（ドット付き・拡張子なしの anchor）
assert causes('.gitignore:1') == [('.gitignore', '1', '')]
assert causes('Dockerfile:5') == [('Dockerfile', '5', '')]
assert causes('.cargo/config.toml:1-3') == [('.cargo/config.toml', '1', '3')]
assert causes('12:30') == []

# 不足側の境界（十進表記の丸め）
python3 -c "mn=float.fromhex('0x0.0000000000001p-1022'); [print(e, float('%de-324'%e)==0.0, float('%de-324'%e)==mn) for e in (1,2,3,5)]"
1 True False
2 True False
3 False True
5 False True

# 最終の木での全走
cargo test --offline -> cargo-test7-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3742/3742 passed; red: none
```

### 三十二次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# 検査 9 の自己検査（here-string、select）
AssertionError: a select loop with a heredoc must not count its body as output
# here-string の実測 `$ cat <<<value` + `value` が緑になることを確認

# 最終の木での全走
cargo test --offline -> cargo-test8-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3742/3742 passed; red: none
```

### 三十三次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# 検査 9 の自己検査（表の整列記号）
assert not complete_table(['| metric |', '| --- |', '| :---: |'])
assert not complete_table(['| metric |', '| --- |', '| :--- |'])
assert not complete_table(['| metric |', '| --- |', '| ---: |'])
assert complete_table(['| metric |', '| --- |', '| 1.2 s |'])

# 最終の木での全走
cargo test --offline -> cargo-test9-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3742/3742 passed; red: none
```

### 三十四次レビュー対応（2026-09-18）

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 9/9

# main の Float の印字が通る経路（PrintFloat ではない）
grep -n 'CanonicalFloat' crates/hado/src/mono.rs
4409:        Type::Float => (RuntimeFunction::PrintString, Expression::call_runtime(RuntimeFunction::CanonicalFloat, vec![value])?),
4534:        Type::Float => RuntimeFunction::CanonicalFloat,
grep -rn 'PrintFloat' crates/hado/src/*.rs
crates/hado/src/runtime_abi.rs:19:    PrintFloat,      # 宣言のみ。呼び出しは無い

# 最終の木での全走
cargo test --offline -> cargo-test10-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3742/3742 passed; red: none
```
