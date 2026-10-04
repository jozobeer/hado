# `String.get` の範囲外を `Char Option` にする（string-get-option）
優先度: 低。

## 現象

文字の字句を判定する述語を分岐の条件に置けない。`String.get` は範囲外を `{Error}` で返すので、`Char` を見る述語が効果を持ち、条件に効果を置けない規則（§36）で E308 になる。

```
$ cat main.hd
def digit text => String.get text 0 as ch -> Char.to_int ch as n -> if 48 <= n => n <= 57 | false
def main = if digit "1" => 1 | 0
$ cargo run --offline -q -- check main.hd
main.hd:2:15: error[E308]: effects are not allowed in branch conditions
```

`tools/dogfood-text-tool/main.hd` は同じ理由で、`e as p ->` の束縛を積んで効果を条件の外へ出している（`is_digit_char`・`has_code_chars`）。

## 原因

`stdlib/string.hd` の `get` は `__string_get` の範囲外を `{Error}` で返す。§55 は `List.head`・`List.nth`・`Env.env` の不在を `Option`（データの不在）と定め、索引の範囲外だけを失敗の表の `IndexOutOfRange` に残す。同じ索引でも `List.nth` は `Option`、`String.get` は `Error` で、規則が 2 つある。`Error` は §36 により条件に置けないので、`String.get` を使う字句の述語はすべて効果を持ち、条件の外へ出す書き方を強いる。

## 案

- 推奨 A: `String.get` の型を `(String, Int): Char Option Pure` にし、範囲外を `None` にする（`List.nth` と同じ）。利用箇所は `stdlib/string.hd` と探針に限られる。
- 退ける B: 効果を持つ `String.get` を残し、`String.get_opt` を足す。範囲外の扱いが 2 つの綴りに分かれ、§55 の失敗の表と member の型の両方に同じ意味が並ぶ。
- 退ける C: 現行のまま `as` の束縛で回避する。効果の順序は変わらないが、条件に置けない規則が字句の述語すべてに漏れる。

## 完了条件

- 採る案を決め、`docs/spec.md` §55 の member の型と失敗の表を追随させる。
- 文字の字句を条件に置く受理コーパスと、範囲外が `None` になることを測る門番を足す。
- 既存の `String.get` の利用箇所（`stdlib/` と各 `scripts/` の探針）を追随させる。

## 由来

`docs/wants.md` 53。`docs/rounds/dogfood-text-tool.md`。§55・§36、E308。
