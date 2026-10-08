# 診断の `canonical form:` と `fmt` の印字し直しが別の印字器を使う（canonical-form-hint-uses-a-second-printer）
優先度: 中。

## 現象

診断が示す `canonical form:` と、`fmt` が残す正準形の綴りが分かれる。括弧の要らない位置で、案内だけが括弧を残す。

```
$ cat target/pipe/main.hd
def id_opt (x: Int Option): Int Option Pure => x
def f (x: Int Option): Int Option Pure =>
  if (x |> id_opt)
  | Some(v) => Some(v)
  | None => None
def main = f Some(3)
$ ./target/debug/hado check target/pipe/main.hd
main.hd:3:3: error[E507]: branches reconstruct the value at 3:9; canonical form: (x |> id_opt)
$ ./target/debug/hado fmt target/pipe/main.hd
fmt: main.hd (2 rewrites)
$ grep 'def f' target/pipe/main.hd
def f (x: Int Option): Int Option Pure => x |> id_opt
```

案内は `(x |> id_opt)`、`fmt` は `x |> id_opt` を残す。§58.1 は「印字し直した形が正準形である」と定めるので、正準形が 2 つあることになる。

## 原因

`crates/hado/src/parser.rs` の `canonical` が案内の綴りを作り、`crates/hado/src/postfix_print.rs` の `program` が `fmt` の印字し直しを作る。`crates/hado/src/parser.rs` の `canonical` は括弧の節をそのまま写すが、`crates/hado/src/postfix_print.rs` の `expr` は優先度から括弧を付け直す。`crates/hado/src/fixes.rs` の `rewrap_if_fix` が書き換えの本文と案内を `canonical` で作り、`crates/hado/src/postfix_print.rs` の `node_text` を通していない。

## 案

- **A: `fixes.rs` の案内を `node_text` に揃える（推奨）。** 案内と `fmt` が同じ印字器を使う。`node_text` は「正準形の案内（`canonical form:`）に使う」と書かれた入口で、`parser.rs` の診断の一部はすでにこれを使っている。`fixes.rs` の 13 か所のうち、どれを `node_text` へ移すかを書き換えごとに決める。
- B: `parser.rs` の `canonical` を `postfix_print` へ委譲し、印字器を 1 つにする。範囲は広いが、正準形が 2 つある状態は残らない。

推奨は A。B は `parser.rs` の `canonical_*` の一群を消すことになり、この不具合より大きい。

## 完了条件

- 各書き換えの案内と `fmt` の出力が同じ綴りであることを測る門番（`# tier: core`、`# round: <slug>`）を足す。
- 受理コーパス・`stdlib`・`tools` の全域で、`check` の案内が `fmt` の出力と一致することを測る。一致しない検体は、どちらの綴りが正準かを決めてから足す。
- 変異（案内を `canonical` に戻す）で門番が赤になる。

## 由来

PR #76（`fmt-layout`、F5）のレビューで見つけた。F5 が `fmt` に全体の印字し直しを入れたので、書き換えの本文がそのまま正準形だった関係が崩れて表に出た。仕様は §58.1（一括整形）・§57（書き換え）・§36（適用）。
