# `fn … /fn` の本体の字下げが `fn` より 2 段深い（fn-body-indent-is-two-levels-deep）
優先度: 中。

## 現象

`fn` の本体が複数行になるとき、本体は `fn` の行より 2 段深く、`/fn` は 1 段深い。§58.1 の配置の規則は「字下げの幅は 2 とし、入れ子のたびに 2 ずつ深くする」と定めるので、本体と `/fn` は同じ段に読める。

```
$ cat target/indent/main.hd
def find_user (id: Int): Int Option Pure => if id > 0 => Some(id) | None
def main = (fn(| Some(user) => Some(user + 1) | None => None)) (find_user 5)
$ ./target/debug/hado fmt target/indent/main.hd
fmt: main.hd (2 rewrites)
$ cat target/indent/main.hd
def find_user (id: Int): Int Option Pure => if id > 0 => Some(id) | None
def main = fn value =>
    value as user? ->
    Some(user + 1)
  /fn (find_user 5)
```

本体が字下げ 4、`/fn` が字下げ 2 である。`def main =` の行が字下げ 0 なので、本体だけが 2 段深い。

## 原因

`crates/hado/src/postfix_print.rs` の `fn_text` が、本体を `indent + 2`、`/fn` を `indent` に置く。呼ぶ側の `crates/hado/src/postfix_print.rs` の `apply_text` は呼び手を `indent` のまま `fn_text` へ渡すので、`fn` が `def` の本体の位置にあるとき、本体だけがさらに 1 段深くなる。

## 案

- **A: `fn` を本体の位置に置くとき、`fn_text` へ渡す `indent` を 1 段深くする（推奨）。** 本体と `/fn` が同じ段に並び、「`fn` の行の字下げ + 2」で読める。§12.1 の閉じの印は、開いた行の字下げに揃えると読めるので、こちらの方が対称である。
- B: 規則 1 を「本体は 2 段、閉じは 1 段」と読み、現行の出力を正準とする。仕様の文をその形に書き直す。

推奨は A。現行の出力は、`fn` の本体と `/fn` の字下げが食い違って読める。

## 完了条件

- 複数行の `fn` を持つ検体で、本体と `/fn` の字下げを測る門番（`# tier: core`、`# round: <slug>`）を足す。
- 受理コーパスか `stdlib` に複数行の `fn` を持つ検体を足し、印字し直しの往復が保たれることを測る。
- 変異（本体の字下げを 1 にする）で門番が赤になる。

## 由来

PR #76（`fmt-layout`、F5）のレビューで見つけた。受理コーパス・`stdlib`・`tools` に複数行の `fn` が 1 件も無いので、この形はどの門番も測っていない。仕様は §58.1（一括整形）・§12.1（閉じの印）・§6（後置の綴り）。
