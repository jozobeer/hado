# `Char` module と `Float.parse` `Int.to_float`（char-and-float-conversions）
優先度: 中。

## 現象

`String.get` で Char は取れるが、Char から先へ進む member が無い（2026-09-17）。

```
$ hado check main.hd    # def main = Char.to_int 'a'
main.hd:1:12: error[E204]: unbound name Char
$ hado check main.hd    # def main = Float.parse "1.5"
main.hd:1:18: error[E204]: module member is not exported
$ hado check main.hd    # def main = Int.to_float 3
main.hd:1:16: error[E204]: module member is not exported
```

Char のリテラル比較（`c == 'a'`）と `Float.to_string`、`Int.parse` は通る。

## 原因

§55 の関数集合に `Char` module が無く、`Float` は `to_string` だけである。`stdlib/float.hd` は 4 行。

## 案

- `Char`: `to_int`、`of_int`、`to_string`。`of_int` は Unicode scalar value の `0x0000..=0xD7FF` と `0xE000..=0x10FFFF` だけを受理し、それ以外は `message` を `"InvalidScalar"` とする `{Error}` にする。
- `Float`: `parse`（受理文法は `Int.parse` と同じく仕様で決め、不正入力は `message` を `"InvalidFloat"` とする `{Error}`）、`of_int`。`Int.to_float` か `Float.of_int` かは prelude の命名（`Bytes.of_string` `Array.of_list`）に合わせて `of_int`。
- 入れない: 大文字化などの Unicode の性質。必要になってから。

## 完了条件

- 各 member の型と、`Char.of_int`・`Float.parse` の失敗の `message` を §55 の失敗の表に書き、受理と拒否のケースを足す。
- `Char.of_int` は `0`、`0xD7FF`、`0xE000`、`0x10FFFF` を受理し、`-1`、`0xD800`、`0xDFFF`、`0x110000` は `message` が `"InvalidScalar"` の `{Error}` になる。受理した値の `Char.to_string` が abort しないことも測る。
- `Float.parse` の受理文法と往復（`to_string` → `parse`）を仕様に書き、門番で測る。文法の外の入力（空文字列、`"1."`、`"."`、`"1e3"`、`" 1"`、`"0x1"`）は `message` が `"InvalidFloat"` の `{Error}` になり、有限値の往復は `parse (to_string x) == x` になる。

## 由来

`docs/wants.md` に無い。§19、§55。
