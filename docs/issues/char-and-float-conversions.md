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

- `Char`: `to_int`、`of_int`（範囲外は `{Error}`）、`to_string`。
- `Float`: `parse`（受理文法は `Int.parse` と同じく仕様で決める）、`of_int`。`Int.to_float` か `Float.of_int` かは prelude の命名（`Bytes.of_string` `Array.of_list`）に合わせて `of_int`。
- 入れない: 大文字化などの Unicode の性質。必要になってから。

## 完了条件

- 各 member の型を §55 に書き、受理と拒否のケースを足す。
- `Float.parse` の受理文法と往復（`to_string` → `parse`）を仕様に書き、門番で測る。

## 由来

`docs/wants.md` に無い。§19、§55。
