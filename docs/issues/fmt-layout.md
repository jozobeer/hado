# fmt に字下げと改行を持たせる（fmt-layout）
優先度: 中。

## 現象

字下げと改行だけが違う 2 つのソースが、どちらも `fmt --check` を通る。正準のレイアウトが無いので、同じプログラムに複数の見た目がある。

```
$ cat target/issue/fmt-layout/flat/main.hd
def size x => if x > 10 => "large" | "small"
def main = size 3
$ cargo run --offline -q -- fmt --check target/issue/fmt-layout/flat/main.hd; echo "exit $?"
exit 0
$ cat target/issue/fmt-layout/ragged/main.hd
def size x =>
      if x > 10
  => "large"
            | "small"
def main = size 3
$ cargo run --offline -q -- fmt --check target/issue/fmt-layout/ragged/main.hd; echo "exit $?"
exit 0
```

閉じ括弧が行末に重なる行は、何を閉じているかを字下げから読めない。受理コーパスと stdlib と `tools` で、行末に `)` が 2 つ以上並ぶ行は 13 行ある（2026-09-26）。

```
$ grep -rhE '\)\)+\s*$' tests/accept stdlib tools --include=*.hd | wc -l
13
```

## 原因

`crates/hado/src/fixes.rs` の `format` が書き換えの後に呼ぶ `layout` は、行末の空白とファイル末尾の改行だけを整える。§58.1 が字下げと改行を変えないと定めている。全体を印字し直す関数は `crates/hado/src/source_print.rs` の `canonical_program` にあるが、`crates/hado/src/lexer.rs` の `lex_full` がコメントを読み飛ばすので構文木にコメントが無く、印字し直すとコメントが消える。§57 は書き換えがコメントを消さないと定めている。

## 案

- 推奨: コメントを構文木かトークン列に保持し、`canonical_program` に配置の規則を持たせて、`fmt` が全体を印字し直す。§58.1 の「字下げは変えない」を改める。
  - 決める規則: 字下げの幅、行幅の上限、`if` の腕を 1 行に収めるか腕ごとに改行するかの境目、複数行の括弧（`fn(`、`postfix-binding` で `if(...)` を採れば `if(`）の `)` を単独の行に置いて開いた行の字下げに揃えること。
- 字下げだけを入れ子の深さから付け直し、改行の位置は書き手に任せる。行単位で済むのでコメントを保持する仕組みが要らないが、1 行と複数行の両方が正準のまま残る。
- 退ける: 字下げの食い違いを `check` の診断にする。linter は置かないと決めている（`docs/wants.md` の「診断と正準形」、2026-09-10）。
- 着手は `postfix-binding` の後にする。構文が変わると配置の規則も印字器も作り直しになる。`postfix-binding` で `if(...)` を採るなら、長い本体の `)` の読みやすさがこの issue に依存する。

## 完了条件

- 現象の `flat` と `ragged` に `fmt` を当てると、同じバイト列になる。
- コメントを含むソースに `fmt` を当てても、コメントが同じ定義の前後に残る。
- `tests/accept`・`stdlib`・`tools` の全ファイルで、`fmt` を 2 回当てた結果が 1 回目と同じで、`cargo test --offline` が 0 failed。
- `bash scripts/lint-docs.sh` が PASSED。

## 由来

`docs/wants.md` の 11（`hado fmt` の「レイアウトの正規化は残り」）。2026-09-26 の `postfix-binding` の相談で、`if(...)` の閉じ括弧が長い本体で何を閉じるか分かりにくいと挙がった。§57、§58.1。関連する issue は `postfix-binding`。
