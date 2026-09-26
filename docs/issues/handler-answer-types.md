# 1 つの本体で同じ handler を別の答え型で使うと build が落ちる（handler-answer-types）
優先度: 中。

## 現象

1 つの関数の本体で、同じ handler を答え型の違う 2 つの `with` に使うと、`check` は通るのに `build` と `run` が内部エラーで止まる（2026-09-26）。prelude の `Error` だけで再現する。

```
$ cat target/issue/answer-two/main.hd
// needs: prelude
handler Error as ToOption
abort raise _ => None
return value => Some(value)
type R = { a: Int Option; b: String Option }
def main = { a = with ToOption -> Int.parse "1"; b = with ToOption -> String.slice "ab" 0 1 }
$ cargo run --offline -q -- check target/issue/answer-two/main.hd; echo "exit=$?"
exit=0
$ cargo run --offline -q -- run target/issue/answer-two/main.hd; echo "exit=$?"
handler arms disagree on the answer type
exit=1
```

`with` を多相関数に括り出すと、同じ 2 つの答え型で通る。

```
$ cat target/issue/answer-poly/main.hd
// needs: prelude
handler Error as ToOption
abort raise _ => None
return value => Some(value)
def attempt f => with ToOption -> f ()
type R = { a: Int Option; b: String Option }
def main = { a = attempt fn(_ => Int.parse "1"); b = attempt fn(_ => String.slice "ab" 0 1) }
$ cargo run --offline -q -- run target/issue/answer-poly/main.hd
{ a = Some(1); b = Some("a") }
```

2 つの `with` を入れ子にして、同じ本体で両方の順序に並べる形では、`done payload mismatch` で止まる（`Absent` と `Error` の handler で確認）。

## 原因

止まる場所は `crates/hado/src/mono.rs` の `interpret_outcome`（腕の型が `outgoing` と食い違う）と `done`（`done payload mismatch`）である。答え型は `handler_answers` に積まれる。単相化が handler の腕を答え型ごとに具体化していないとみられるが、未確認。型検査は同じ形を受理するので、`check` と `build` の判定が食い違う。

## 案

- 推奨: 単相化で、handler の腕を `with` ごとの答え型で具体化する（多相関数の具体化と同じ扱いにする）。
- 退ける: 型検査で同じ本体の 2 つ目の答え型を拒否する。§38 は handler に答え型の制限を置いておらず、多相関数を経由すれば通るので、拒否する根拠が無い。

## 完了条件

- 上の `answer-two` が `{ a = Some(1); b = Some("a") }` を印字して終了コード 0 になる。
- 入れ子の両方の順序を同じ本体に並べる形が、それぞれ単独で動かしたときと同じ値を返す。
- `cargo test --offline` が 0 failed。

## 由来

2026-09-26 の実験（不在を効果にする案、`do-option-absent`）で当たった。prelude に `None` へ戻す handler を置くと、1 つの本体の中で多くの型に使われるので、その案の前提になる。§38。
