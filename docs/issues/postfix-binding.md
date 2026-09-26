# 束縛の名前を式の後ろに書く（postfix-binding）
優先度: 中。

## 現象

束縛（`let p =`）と、値を使わない一歩（`do`）は、行頭のキーワードで始まる。書く側は、計算を書く前に、結果に付ける名前か、値を捨てることを決めることになる。受理コーパスと stdlib で行頭が `let` の行は 20 行、`do` の行は 7 行ある（2026-09-26）。

```
$ grep -rhE '^\s*let\b' tests/accept stdlib --include=*.hd | wc -l
20
$ grep -rhE '^\s*do\b' tests/accept stdlib --include=*.hd | wc -l
7
```

行頭のキーワードは、`if` の後ろに続く `->` がどこに付くかも決めている。`if` は閉じる印を持たず、最後の腕は右端まで伸びる。`do` を `if` の前に置けば `->` は `if` 全体に付き、腕の中に置けば `->` はその腕に付く。

```
$ cat target/issue/whole/main.hd
// needs: prelude
def main =
  let ok = true ->
  do if ok => log "a" | log "b" ->
  log "finish"
$ cargo run --offline -q -- run target/issue/whole/main.hd
a
finish
$ cat target/issue/inarm/main.hd
// needs: prelude
def main =
  let ok = true ->
  if ok => log "a" | do log "b" ->
  log "finish"
$ cargo run --offline -q -- run target/issue/inarm/main.hd
a
```

キーワードを持たない `|>` は、最後の腕の後ろに置くと腕に付く。`Int.to_string` は `2` にだけ掛かり、腕の型が食い違う。

```
$ cat target/issue/pipe/main.hd
def main =
  if true => 1 | 2 |> Int.to_string
$ cargo run --offline -q -- check target/issue/pipe/main.hd
main.hd:2:20: error[E305]: expected Int, found String; expected here at main.hd:2:14
```

## 原因

`crates/hado/src/parser.rs` の `parse_expr` が、先頭の 1 トークンで `parse_let`・`parse_do`・`parse_with`・`parse_branch` に分ける。束縛と一歩の範囲は、このキーワードが開き括弧の役目をして決まる。キーワードを外して `x = e ->` と `e -> body` と書く案は `docs/plan.md` の `Q6` で退けている。退けた理由は、文法規則が 6 本以上増えることと、行頭の 1 トークンでの判別を失うことである。

## 案

- 推奨: 束縛を `e as p ->` と書く（Option は `e as p? ->`）。値を使わない一歩は、キーワード無しの `e ->` と書く。`let` と `do` は廃止し、`with H ->` は前置のまま残す（計算の結果ではなく、この先を handler の下で動かす宣言なので）。
  - `as` は既にキーワードで、使うのは `implement X as Y` と `handler X as Y` の見出しだけである。式を読み終えた直後の 1 トークン（`as` か `->`）で形が決まるので、`|>` と同じ種類の判別で済む。
  - 行頭の判別は `as` を入れた時点で失われる。そのため、`do` を外す追加の費用は `Q6` の見積もりより小さい。`let` の右辺と `->` の境界の問題は、`let` ごと消える。
  - 「実行に意味がある」という `do` の意図は、純粋な式の破棄を拒む E303 がそのまま担う。
  - 名前が左端に縦に並ばなくなるので、どこで何を定義したかを拾い読みしにくくなる。
- 未決: `if` の後ろの `->` と `as` がどこに付くか。`if` の範囲が字句で決まれば曖昧さは消える。有力は `/キーワード` で閉じる形。
  - 有力: 入れ子になれる構文を `/キーワード` で閉じる（`/if`、`/fn`、`/def`、`/module`、`/implement` など）。閉じるのは、本体に `->` を含む構文（入れ子の中を含む）と、後ろに続き（`->`・`as`・`|>`・後ろの引数）を持つ構文である。`module` と `implement` は項目を含むので常に閉じる。それ以外は §2 に従って書かない（`def size x => if x > 10 => Large | Small` は閉じない）。
    - `fn` は括弧を外して `fn x => x + 1 /fn` と書く。
    - `/` の直後に `if` を置く形は今 E100 なので、`/if` は空白の有無によらず除算と衝突しない。`10 / fn(x => x)` は今は構文として通って E305 になるので、`fn` を二項演算子の被演算子に直接置けない規則を足す。§6 の `/` に閉じの読みが加わる。
    - 閉じる印が名前を持つので、長い本体でも何を閉じるかを字下げに頼らずに読め、閉じ違い（`if … /fn`）をその場で報告できる。
    - 閉じの要否は構文木から決まるので、足りない閉じと余分な閉じは正準形の違反になり、`fmt` が補える。補った `/if` の位置にパーサの読みが見える。
    - 受理コーパス・stdlib・`tools` で本体を持つ `def` 333 個のうち、`->` を含むのは 69 個である（2026-09-26、項目の見出しで切った近似）。残りは閉じずに書ける。
    - 1 行の定義に `->` を 1 つ足すと `/def` が要る（`def f x => g x as y -> h y /def`）。
  - `if` を `fn(...)` と同じく常に括弧で閉じる（`if(x > 10 => Large | Small) as size ->`）。§12 に揃えるだけで規則は増えないが、末尾に置く `if` にも 2 文字が付き、本体が長いと `)` が何を閉じるか読みにくい。後者は `fmt-layout` の配置の規則で補う。
  - 必要なときだけ閉じる印（`fi` など）を書き、書き忘れを拒否する。黙って意味が変わる経路は無い。
  - 常に `if … fi` と閉じる。閉じる印が自分の名前を持つので、長い本体でも `)` と見分けられる。1 行では `Write fi` が適用に見える。
  - 空の腕で合流を示す（`| ->`、`| as p ->`）。新しい字句は要らないが、1 行では fallback の腕に見える。
  - 常に最後の腕に付き、`if` 全体に続けるときは `(if …) ->` と括弧で囲む。複数行のレイアウト（腕ごとに改行する形）の見た目と一致し、どの腕の `->` もその腕に付くので腕が対称になる。受理コーパスで `if` 全体に続けている箇所は 1 つ（`tests/accept/effect_position/main.hd` の `let scrutinee = if …`）である。
  - 最後の腕の後ろなら `if` 全体に付く。1 行で書いたときの見た目には合うが、複数行では `->` が最後の腕の行末に付くので見た目が逆になる。`if ok => log "a" -> 1 | log "b" -> 2` の `-> 1` は腕に、`-> 2` は `if` 全体に付くので、腕が非対称になる。両方の腕が Void なら型検査も通る。
  - `fmt` の正準のレイアウトで誤読しない規則を選ぶ。
- 退ける: `do` を前置のまま残す（計算より先に扱いを決めさせる点で、前置の `let` と同じ問題を持つ）。値を捨てる行を `e as _ ->` と書く（名前を付けないことをわざわざ書く）。すべての行を `do` で始めて `do e as p ->` とする（純粋な計算に `do` が付き、`do` の意図と食い違う）。`if` の範囲を字下げで決める（空白に意味を持たせない原則を覆す）。閉じる印の綴りのうち、`end`（何の終わりか分からず、`String.slice` の引数名と衝突する）、`endif`（長い）、`.if`（フィールド参照に衝突する）。
- 追随するもの: 仕様の §11・§34・§35・§40・§53 と E101 の「継続」の定義、E501・E504・E505・E506 の文言と書き換え、正規形の印字と `fmt`、tree-sitter の文法。閉じる印を採るなら §6・§12・§23〜§26.1・§30 と、受理コーパス・stdlib・`tools` の閉じが要る構文。`crates/hado/src/mono.rs` の `make_continuation` による `again` の継続の切り出しが `do` の形に依っているので、影響の範囲を実測する。
- Option に裸の `->` を付けると、純粋な値の破棄として E303 になる。`as p? ->` を案内する文言が要る。`do-option-absent` の推奨を採れば、この場合は消える。

## 完了条件

- 選んだ構文と `if` の後ろの規則が仕様に書かれ、旧形（`let` と `do`）が E100 になる。
- `if` 全体に続ける形と、最後の腕に続ける形の両方を受理コーパスが固定し、実行結果が規則どおりになる。
- `cargo test --offline` が 0 failed、`bash scripts/lint-docs.sh` が PASSED。

## 由来

2026-09-26 の相談。先例は SQL の `SELECT … AS name`、Clojure の `as->`、Haskell の `where`。§11、§34、§35、§36、§53、E101、E303。関連する issue は `do-option-absent` と `fmt-layout`。
