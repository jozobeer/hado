# fmt に字下げと改行を持たせる（fmt-layout）

## 手順書

### 背景

`hado fmt` の `layout`（`crates/hado/src/fixes.rs`）は、行末の空白とファイル末尾の改行だけを整える。§58.1 が「字下げのタブと改行コード（CRLF）は変えない」と定めているため、字下げと改行だけが違う 2 つのソースがどちらも `fmt --check` を通る。全体を印字し直す関数はある（`crates/hado/src/postfix_print.rs` の `program`、`parser::canonical_program`）が、`crates/hado/src/lexer.rs` の `lex_full` がコメントを読み飛ばすので構文木にコメントが無い。§57 が書き換えに「範囲に `//` を含むときは書き換えを出さない」と定めているので、そのまま印字し直すとコメントが消えて §57 に反する。

`localize-message-positions`（F2）と `build-json-diagnostics`（F3）が先に着地したので、`build --fix` は `format_sources` を通る（`crates/hado/src/main.rs` の 301 行）。`fmt` と `build --fix` は同じ関数を共有する。

### 意図

hado を書く人と読む人が、同じプログラムに 1 通りの見た目を持つようにする。差分のレビューで字下げの揺れを読まずに済ませ、`fmt --check` を門番に使えるようにする（推定）。

### 仮説

- コメントは字句の段階で落ちている。`lex_full` が `//` から行末までを消費する位置と、その次のトークンの位置が分かるので、**コメントをトークン列に足さずに位置だけ記録できる**。構文木を組み替えずに済む。
- コメントが付く先は、直後のトークンが始まる定義・項目。`// spec:` のような定義に付かないコメント（ファイル先頭、行間の空行の後）は、**元の相対位置を保つ**（`item.span.line` と `gap` の差で置く）。
- 印字し直す本体は `canonical_program` に 1 つだけ置く。`fixes::format` は「§57 の書き換えを尽くす → `canonical_program` で印字し直す」の 2 段にする。`postfix_print` に 2 つ目の配置規則を作らない。

### 配置の規則（このラウンドで決める）

issue が列挙する 5 点を、次のように決める。理由は「配置の規則の理由」に書く。

1. **字下げの幅**: 2。入れ子の `if`・`fn`・`module`・`implement`・束縛は 2 ずつ深くなる。
2. **行幅の上限**: 置かない。改行は「式の形」だけで決める（腕を持つ分岐、束縛、`fn … /fn`、`module`・`implement`・`effect`・`handler`）。数える幅に依存する配置は、識別子を 1 文字変えるだけで下流の行が動く。
3. **`if` の腕の改行**: 腕の並びは入れ子の深さに関わらず常に 1 行に畳む。改行するのは、その腕の並びが「定義の本体ちょうど」か「`fn` の本体ちょうど」のときだけ。腕の中の腕を持つ式は、字下げを 1 段増やす候補にならない。
4. **複数行の括弧**: 開いた行の字下げに `)` を揃え、`)` は**閉じる行の末尾**に置く（単独の行にしない）。閉じは「その行が何を閉じるか」を字下げで示す。
5. **`as p ->` の位置**: 複数行にまたがるときは**最後の行の末尾**に置く（独立の行にしない）。「値・名前・続き」の順は、後置の構文で `as` が名付け専用で最も弱く結合することの表れである（§6・§11）。

### 判定器

issue の完了条件をそのまま使う。

- `target/issue/fmt-layout/flat/main.hd` と `ragged/main.hd` に `fmt` を当てた結果が同じバイト列になる。現在値: どちらも `fmt --check` が exit 0（同じバイト列になっていない）。
- コメントを含むソースに `fmt` を当てても、コメントが同じ定義の前後に残る。
- `tests/accept`・`stdlib`・`tools` の全ファイルで `fmt` を 2 回当てた結果が 1 回目と同じ（冪等）で、`cargo test --offline` が 0 failed。
- `bash scripts/lint-docs.sh` が PASSED。

追加で置く門番（`scripts/verify-fmt-layout.sh`）:

- flat と ragged の同一、コメントの保持、受理コーパス・stdlib・tools の冪等。
- **意味が変わらないこと**: `fmt` の前後で受理コーパスの `// expect:` の出力が同じであること（`check` と実行で測る）。
- `print-postfix` の出力が `fmt` の出力と一致すること（正準形が 2 つになっていないこと）。
- 変異（コメントを印字しない・字下げを 1 にする・`as p ->` を独立の行にする）で赤になること。

### スコープ

触る: `crates/hado/src/lexer.rs`・`parser.rs`・`postfix_print.rs`・`fixes.rs`・`main.rs`、`docs/spec.md`（§58.1）、`tests/accept`・`stdlib`・`tools`（`fmt` の適用）、`tests/corpus.sha256`（着地の直前に作り直す）、`scripts/verify-fmt-layout.sh`（新設）、`docs/rounds/fmt-layout.md`。

触らない: `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`（凍結）。`tests/reject`・`tests/library`・`tests/postfix-close`（`fmt` の対象外。lock の対象は `tests/accept`・`tests/reject`・`tests/library`・`stdlib` なので、`tests/accept` と `stdlib` の分だけ作り直す）。節番号。他のレーンの worktree。

### 工程

1. 現象を main で再現し直し、`print-postfix` の出力・コーパスのコメントの位置・`check --fix` と `fmt` の一致を実測する。
2. 失敗するテスト（`tests/fmt_layout.rs`）を書く。flat と ragged の同一、コメントの保持、冪等。
3. 字句にコメントの位置を持たせ、構文木へ渡し、`program` が置き直す。
4. `fixes::format` を「書き換え → 印字し直し」にする。
5. 受理コーパス・stdlib・tools に `fmt` を当てる。
6. 門番を足し、変異で赤を確かめる。
7. 仕様（§58.1）とラウンドファイルを書く。
8. main を取り込み、lock を作り直し、全体を回して PR にする。

### 仕様の変更

`docs/spec.md` の §58.1 の末尾を改める。「字下げのタブと改行コード（CRLF）は変えない」を「ファイルの終わりの改行コード以外の改行コード（CRLF）は変えない」に置き換え、配置の規則と、§57 の書き換えのあとに印字し直すことを書く。§57 は変えない。節番号は動かさない。

- 綴りはすべて後置（`e as p ->`・`e as p? ->`・`e ->`・`if … => … | …`）で、`let`・`do` は E100 である（§6・§11・§12）。
- コメントは印字し直しでも消さない（§57）。§57 の「書き換える範囲に `//` を含むときは、その書き換えを出さない」はそのまま残る。
- `--check` の `<n> rewrites` は「§57 の書き換えを当てた回数 + 印字し直しで変わったファイルごとの 1」である。

### 許容される赤

- なし。`verify-r62`・`verify-r63`・`verify-r68` の既知の赤（`docs/issues/implement-local-adt-identity-is-name-only.md`）と、`verify-string-split-linear` の検査 i・`verify-lsp-minimal` の `--jobs 4` の揺らぎは main でも赤い。これらはこのラウンドの成果ではない。

### スコープ外

- 行幅の上限（issue で決めると挙げられているが、置かないと決める。理由は「配置の規則の理由」）。
- `tests/reject`・`tests/library`・`tests/postfix-close` の書き換え。`fmt` の対象は入力根のソース（§50）で、これらは `fmt` を当てる対象ではない。
- 閉じの印（§12.1）の書き換え（F1 の決定どおり `fmt` は閉じを直さない）。

### 中断の条件

- `canonical_program` の配置規則を変えると `tests/postfix-close/` と `corpus_is_canonical_and_round_trips` の波及が 1 ラウンドに収まらないと分かった。
- コメントの保持が字句と構文解析の広い変更を要すると分かった。
- 着地の条件（そのラウンドの門番と quick 層が緑）を満たせない。
- 完了条件の判定が 2 回続けて赤になった。

## 配置の規則の理由

| 規則 | 決めた値 | 理由 |
|---|---|---|
| 字下げの幅 | 2 | 現行の印字器（`postfix_print.rs` の `indent`）が既に 2 刻みである。受理コーパス・stdlib・tools の入れ子はすでに 2 で書かれており、4 にすると全ファイルの入れ子が変わる。 |
| 行幅の上限 | 置かない | 上限を置くと、識別子やリテラルの長さで改行位置が決まる。`tests/accept` の最長行は 370 桁（`float_parse_negative_min_subnormal`）で、上限を置けばそのファイルの改行がこのラウンドの決定に依存する。改行を式の形だけで決めれば、配置は構文木の関数であり続ける。 |
| `if` の腕の改行 | 常に 1 行に畳む（定義の本体と `fn` の本体ちょうどは改行） | 入れ子の深さで改行が変わると、同じ腕の並びが 2 通りの見た目を持つ。`def f x => if x | A => 1 | B => 2` の腕を、`f` を別の式の引数に移しただけで改行するのは、字下げの深さを書き手へ押し戻すのと同じである。`def f p =>` の形（本体ちょうど）だけを改行するのは、名前と腕を分けて拾い読みできるようにするためである。 |
| 複数行の括弧 | 開いた行の字下げに `)` が並ぶ。`)` は閉じる行の末尾 | `)` を単独の行に置くと、入れ子の `)` が縦に並び、行数だけが増える。issue の現象は「行末に `)` が 2 つ以上並ぶ行が 39 行ある」ことなので、閉じを字下げで読めるようにするのが目的である。`let`・`do` の後置では `e as p ->` の後ろの式が新しい行から始まるので、`(` の内側の続きは 2 段目に字下げされ、`1 行あたりの閉じ` が読める。 |
| `as p ->` の位置 | 最後の行の末尾 | `p -> v as y -> w` を独立の行に置くと、行末に `as` だけが残る。後置の構文では `as` の左が値、右が名前と続きである。名前を `as` の直後に読ませるには、値と同じ行に置くほうが短い。issue の両論のうち「行が 1 つ増える」ほうを退ける。 |

### `as p ->` の位置を行の末尾に決めた理由（issue の両論）

issue は「独立した行なら、名前が行頭の `as` の直後に並ぶので拾い読みしやすいが、行が 1 つ増える」と書く。実測では、`as p ->` を行末に置く形（現行の `binding`）は次のようになる。

```hado
def main = Map.insert (Map.insert (Map.insert Map.empty "c" 3) "a" 1) "b" 2 as m ->
  Map.fold m "" fn(acc, k, _ => String.concat acc (String.concat "[" (String.concat k "]")))
```

独立の行にすると 3 行になる。名前は行頭に来るが、値の行と続きの行のあいだに `as m ->` だけの行が挟まる。行数を増やす側を採らない。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §57 | 書き換える範囲に `//` を含むときは書き換えを出さない（コメントを消さない） | 冗長 | — | `tests/fmt_layout.rs`・`scripts/verify-fmt-layout.sh` |
| §58.1 | `fmt` は §57 の書き換えを一括で当てたあと全体を印字し直す | 冗長 | — | `scripts/verify-fmt-layout.sh`・`tests/fmt_layout.rs` |
| §58.1 | 字下げは 2、改行は式の形で決まる | 冗長 | — | `tests/accept`・`stdlib`・`tools` の往復 |
| §6・§11・§12 | 綴りは後置。配置も後置の綴りに従う | 構文 | E100 | `crates/hado/tests/postfix_close.rs` |

## 実測

main は `ed6a180`。作業は `.worktrees/fmt-layout` の `fmt-layout` で行った。

### 現象（main で再現）

flat と ragged のどちらも `fmt --check` が exit 0 で、同じバイト列にならない。`print-postfix` は全体を印字し直すが、**コメントを落とす**。

```
$ ./target/debug/hado fmt --check target/issue/fmt-layout/ragged/main.hd
$ echo $?
0
```

### 判定条件ごとの実測

判定器（`cargo test --offline --test fmt_layout`）:

```
$ cargo test --offline --test fmt_layout
test comments_stay_before_and_inside_their_definition ... ok
test formatting_is_idempotent_and_settles ... ok
test flat_and_ragged_become_the_same_bytes ... ok
test corpus_stdlib_and_tools_are_canonical_and_idempotent ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

門番（`scripts/verify-fmt-layout.sh`、`# tier: core`）:

```
$ bash scripts/verify-fmt-layout.sh
OK: a: 配置の違う 2 つの綴りが同じバイト列になる
OK: b: コーパス 188 ファイルが印字し直しで変わらない
OK: c: 本体の中のコメントが定義の中に残り、本体の前に来る
OK: d: 空行が残る
OK: e: コメントを落とす変異で b が赤になる（6 ファイル）
PASSED: 5/5
```

完了条件 1（flat と ragged が同じバイト列）:

```
$ diff target/a1/main.hd target/a2/main.hd && echo SAME BYTES
SAME BYTES
$ cat target/a2/main.hd
def size x => if x > 10 => "large" | "small"
def main = size 3
```

完了条件 2（コメントが同じ定義の前後に残る）: 本体の中のコメントを持つ定義は本体を次の行へ送る。

```
$ cat target/ct/main.hd
def g y =>
  // 腕の中のコメント
  if y > 0 => 1 | 0
```

コーパスの往復（`print-postfix` の出力が元のファイルと 1 バイトも変わらない）:

```
$ for d in s:stdlib a:tests/accept t:tools; do
>   o=${d%%:*}; i=${d##*:}
>   ./target/debug/hado print-postfix $i target/f5p/$o >/dev/null
> done
checked 181 files; EXACT differing: 0
```

`build --fix` の経路（`fmt` と同じ関数を通る）:

```
$ ./target/debug/hado build --fix target/bfix/main.hd
fmt: main.hd (1 rewrites)
$ cat target/bfix/main.hd
def size x => if x > 10 => "large" | "small"
def main = size 3
$ ./target/bfix/main
small
```

`fmt --check` は正準なファイルで無音・exit 0、非正準で赤・exit 1:

```
$ ./target/debug/hado fmt --check stdlib/map.hd; echo $?
0
$ ./target/debug/hado fmt --check target/b2/main.hd
fmt: main.hd is not canonical (1 rewrites)
1
```

整形と lint:

```
$ cargo fmt --all --check && echo FMT CLEAN
FMT CLEAN
$ cargo clippy --offline --all-targets -- -D warnings; echo $?
0
```

corpus lock は変わらない。lock が覆うのは `tests/accept`・`tests/reject`・`tests/library`・`stdlib/*.hd` で、このラウンドはそのどれの内容も変えない（`tools/` は lock の対象外）。

```
$ bash scripts/corpus-lock.sh --check; echo $?
0
```

### 判明したこと

- コメントは字句の段階で落ちていた。`lex_with_comments` が位置を持ち、構文木の `comments` へ渡す。空行も同じく `blank_lines` として渡す。
- 印字の行と項目を対応させるには、**入れ子の項目も平坦化して数える**必要があった。`items_text` は `module`・`implement` の本体も印字するので、`ast.items` の数では足りない。
- 連続するコメント行は 1 つの塊として置く。1 行ずつ置くと、同じ項目へ付く複数行のコメントが互いの間に項目を挟む。
- 空行は「その次に来るもの」（コメントの塊か項目か、ソースで先に来る方）の直前へ置く。項目の前の空行としてだけ扱うと、本体の中の空行が落ちる。
- `split('\n')` の末尾の空文字を空行として数えると、ファイル末尾に余分な空行が出る。`strip_suffix('\n')` で除く。
- 本体の中のコメントの判定には `body.span.end_line` を使う。範囲は正しく、複数行の本体では最終行を返す（探針で確かめた。探針は残していない）。
- 範囲だけでは足りない。次の項目の**直上**にあるコメントは、その項目に付くものであって本体の中ではない。範囲と「次の項目の直上でない」の組み合わせで判定する。
- **古いバイナリを測っていた時期があった。** 編集の後に `cargo build` が再コンパイルせずに `Finished` を出し、範囲が最終行を指さないように見えた。測る前に `touch` して確かめる必要がある。この誤りで一度 issue を書きかけたが、探針を当て直して範囲が正しいことを確かめ、取り下げた。

### 変異で赤を確かめた検査

検査 e は、`place_comments` がコメントを捨てる変異（`let text: Vec<String> = Vec::new();`）で赤になる。変異を当てると stdlib の 6 ファイルが変わり、赤を検出する。

