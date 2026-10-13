# field の base が構築子の括弧呼び出しのとき余分な括弧が付く（field-base-call-over-parenthesized）
優先度: 低。

## 現象

`fmt` の印字し直しが、base が `Call`（構築子の括弧呼び出し）の field に要らない括弧を付ける。同じ判定を持つ `canonical_field` は適用と整数だけを囲むので、2 つの印字器で綴りが分かれる。診断の列も 3:9 から 3:10 へずれる。

```
$ printf 'type R = { a: Int }\ntype B = | B(R)\ndef v = B({ a = 1 }).a\ndef main = "x"\n' > target/issue-fbc/main.hd
$ ./target/debug/hado check target/issue-fbc/main.hd
main.hd:3:9: error[E305]: field access requires a single record constructor
$ ./target/debug/hado fmt target/issue-fbc/main.hd
fmt: main.hd (1 rewrites)
$ cat target/issue-fbc/main.hd
type R = { a: Int }
type B = | B(R)
def v = (B({ a = 1 })).a
def main = "x"
$ ./target/debug/hado check target/issue-fbc/main.hd
main.hd:3:10: error[E305]: field access requires a single record constructor
```

元のソースの `B({ a = 1 }).a` に括弧は要らない。構文解析はもともと `Field(Call)` として読む。影響は型が通らないプログラムだけで、受理コーパスには出てこない。

## 原因

`crates/hado/src/postfix_print.rs` の `field_text` が `NodeKind::Call(..)` を適用と同じに扱い、base を括弧で囲む。`crates/hado/src/parser.rs` の `canonical_field` は適用と整数だけを囲む。§7 は `.` と適用の優先順位を定めるが、構築子の括弧呼び出しが適用に当たるかを定めていない。

## 案

- **A: `field_text` の対象から `NodeKind::Call(..)` を外す（推奨）。** `canonical_field` と同じ判定に揃える。PR #92 の審査が推奨した。
- B: `canonical_field` に `Call` を足す。構文解析では括弧が要らないので、案内の側が余分な括弧を出すことになる。

推奨は A。

## 完了条件

- `B({ a = 1 }).a` に `fmt` を当てても括弧が付かないことを測るテストと、変異で赤になる門番（`# tier: core`、`# round: <slug>`）を足す。
- `fmt` の出力と `check` の `canonical form:` の綴りが base の `Call` で一致することを測る。

## 由来

PR #92（`field-base-parentheses`）の審査 `.worktrees/pr92/target/reviews/pr92-review.md` の欠陥 1。仕様は §7。
