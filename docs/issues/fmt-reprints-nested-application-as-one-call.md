# fmt が入れ子の適用を 1 回の呼び出しに印字し直し、意味を変える（fmt-reprints-nested-application-as-one-call）
優先度: 高。

## 現象

返ってきた閉包をその場で適用する `(f a) b` を、`fmt` が `f a b` に印字し直す。2 回の適用が 1 回の 2 引数呼び出しに変わり、意味が変わる。

```
$ cat > target/fatal/main.hd <<'EOF'
def main = fn(a => fn(b => a + b)) as add -> Int.to_string ((add 1) 41)
EOF
$ ./target/debug/hado run target/fatal/main.hd
42
$ ./target/debug/hado fmt target/fatal/main.hd
fmt: main.hd (1 rewrites)
$ cat target/fatal/main.hd
def main = fn(a => fn(b => a + b)) as add ->
  Int.to_string (add 1 41)
$ ./target/debug/hado run target/fatal/main.hd
exit=1
main.hd:2:18: error[E301]: cannot unify Fn(A => Fn(B => B {C}) {D}) with Fn(Int, Int => E {F})
```

main では `fmt` がこのファイルを変えず、`run` は `42` のままである。`((fn(a => fn(b => a + b)) 1) 41)` と `adder as mk -> ((mk 1) 41)` も同じように壊れる。

`fmt` は `build --fix` からも当たるので、利用者のソースが `build --fix` で壊れる。

## 原因

`crates/hado/src/postfix_print.rs` の `apply_text`（764 行）が、呼び手を `expr_prec(f, indent, false, 8)`（適用と同じ優先度）で印字する。そのため呼び手が `Apply` のとき括弧が付かない。

この印字器の欠陥は以前からある。main でも `print-postfix` は同じ出力を出す。

```
$ ./target/debug/hado print-postfix target/m1 target/m2
$ cat target/m2/main.hd
def add a => fn(b => a + b)
def main = Int.to_string (add 1 41)
$ ./target/debug/hado run target/m3/main.hd   # 出力を読み直す
error[E301]: cannot unify Fn(Int => Fn(Int => Int {A}) {B}) with Fn(Int, Int => C {D})
```

受理コーパスに `(f a) b` の形が無いので表に出ていなかった。`crates/hado/src/fixes.rs` の `format` が `canonical_program` で全体を印字し直すようになり、`fmt` と `build --fix` から任意の入力に当たるようになって、**main で動いていたプログラムを壊す**形で現れた。

```
$ cat > target/w1/main.hd <<'EOF'
def main = fn(a => fn(b => a + b)) as add -> Int.to_string ((add 1) 41)
EOF
$ ./target/debug/hado run target/w1/main.hd     # main
42
$ ./target/debug/hado fmt target/w1/main.hd     # main は無音（変えない）
$ ./target/debug/hado run target/w1/main.hd     # この設計では
exit=1
```

つまり印字器の欠陥そのものは main にもあるが、main では `print-postfix` と `dump-*` の中だけに閉じている。この設計はそれを `fmt` の経路へ広げる。

「コーパス 181 ファイルが 1 バイトも変わらない」という確認は、正準形でない入力の AST が往復で保たれるかを見ていない。

## 案

- **A: `apply_text` の呼び手を、適用より強く束縛する形で印字する（推奨）。** 呼び手が `Apply` のときは括弧で囲む。`expr_prec` の優先度を適用より 1 つ強くするか、`Apply` のときだけ明示的に括弧を付ける。印字器の欠陥そのものを直すので、`print-postfix` と `dump-*` の出力も正しくなる。
- B: `fixes::format` が、印字し直した結果を元のソースと比べて意味が変わる形を検出したら印字し直しをやめる。原因を残すので推奨しない。

## 完了条件

- `(f a) b`・`((f a) b) c`・`adder as mk -> ((mk 1) 41)` を `fmt` に通しても意味が変わらない。`fmt` の前後の `run` の出力が同じであることを門番（`# tier: core`、`# round: <slug>`）で測る。
- `print-postfix` の出力を読み直して `run` した結果が、元のソースの `run` と同じであること。
- 受理コーパス・`stdlib`・`tools` に `(f a) b` の形の検体を足し、往復が保たれることを測る。
- 変異（呼び手の括弧を落とす）で門番が赤になる。

## 由来

PR #76（`fmt-layout`、F5）のレビューで見つけた。`docs/plans/issue-backlog.md` の F5。仕様は §58.1（一括整形）、§6・§11（後置の綴り）、§36（適用）。
