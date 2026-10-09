# fmt が適用の後ろの field 参照を内側の引数に付ける（parenthesize-field-base-after-application）
優先度: 中。

## 現象

`(f a).name` の形を `fmt` が `f a.name` に印字し直す。`.name` が `a` に付き、
意味が変わる。分岐に限らない。

```
$ mkdir -p target/fieldbase
$ cat > target/fieldbase/main.hd <<'EOF'
type R = { a: Int }
def g x => { a = x }
def f (x: R): R => x
def v = (f (g 1)).a
def main = Int.to_string v
EOF
$ ./target/debug/hado run target/fieldbase/main.hd
1
$ ./target/debug/hado fmt target/fieldbase/main.hd
$ cat target/fieldbase/main.hd
def v = f g 1.a
$ ./target/debug/hado check target/fieldbase/main.hd
main.hd:4:13: error[E103]: float must have digits after the '.'
```

`if` を引数にした `(f (if true => { a = 1 } | { a = 2 })).a` も同じで、こちらは
`f if(true => { a = 1 } | { a = 2 }).a` になり、`main.hd:3:11: error[E305]:
expected R, found Int` で落ちる。

main の `print-postfix` でも同じ出力を出す。印字器（`crates/hado/src/postfix_print.rs`
の `field_text`）の欠陥で、F5（PR #76）の印字し直しが `fmt` の経路へ広げた。

## 原因

`crates/hado/src/postfix_print.rs` の `field_text` が base を
`expr_prec(l, indent, false, 9)` で印字する。base が `Apply` のとき、適用は
優先度 9 なので括弧が付かない。`format!("{base}.{}", r.name)` の `.name` が、
適用の最後の引数の `)` の直後に付き、その引数に読まれる。`Field(Apply(..), name)`
の base は適用全体なので、適用全体を括弧で包む必要がある。

## 案

- **A: `field_text` の base が `Apply`（と `Call`）のとき、base 全体を括弧で
  包む（推奨）。** `format!("({})", expr_prec(l, indent, false, 9))`。印字器の
  欠陥そのものを直すので、`print-postfix` の出力も正しくなる。
- B: `Field` の base を常に括弧で包む。不要な括弧が増えるので推奨しない。

## 完了条件

- `(f (g 1)).a` と `(f (if true => … | …)).a` を `fmt` に通しても意味が変わらない。
  `fmt` の前後の `run` の出力が同じであることを門番で測る。
- `print-postfix` の出力を読み直して `run` した結果が、元のソースの `run` と同じ。
- 受理コーパス・`stdlib`・`tools` の往復が保たれる。

## 由来

PR #81 のラウンド（`docs/rounds/tail-operand-branch-parentheses.md`）の実測で
見つけた。`if(...)` の括弧の欠陥とは別の、`.name` の結合先の欠陥である。
仕様は §36（適用）、§54（印字）。
