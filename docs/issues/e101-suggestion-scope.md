# 腕に置いた Boolean branch の suggestion の適用範囲（e101-suggestion-scope）
優先度: 低。

## 現象

腕の本体の `if` の else が、続く腕の綴りを食う形では、E101 に
`| p => if | c => a | b` を添えるかどうかが読み方で変わる。2026-09-26 の
六次レビュー（PR #44）の指摘。

```
$ cat e/main.hd
def f x =>
  if x
    | A => if true => 1
    | B => 2
$ cargo run --offline -q -- check --format json e/main.hd
{"path": "main.hd", "line": 3, "col": 12, "end_line": 3, "end_col": 12, "code": "E101", "message": "branches cannot be nested inside another branch", "canonical": null, "suggestion": "| p => if | c => a | b", "related": null}
```

## 原因

`crates/hado/src/parser.rs` の `boolean_branch_ahead` は、先読みで `BoolBranch` が
読めるかどうかだけを見る。§24 は `|` の右を必ず else の式と定めるので、腕の本体は
`if true => 1 | B`（else が `B`）の Boolean branch として成り、直後の `=>` が外側の
腕の綴りとして余る。§26.1 も、fallback を持たない腕の条件列が続く `|` の腕を自分の
条件として読むと明記しており、この形の曖昧さは貪欲に読む。読み方を変えるなら、
先読みの終端が `=>` でないことも条件に足す（`boolean_branch_ahead`）。

## 案

- 推奨: 現行のまま（§24 と §26.1 の貪欲な読み）。else を欠く `| 0 => if true => 1 |`
  には添えず、`| B` が続く形には添える。
- 退ける: 先読みの終端が `=>` のときは添えない。直す場所は 1 か所で済み、ill-formed
  な入力でだけ差が出る（valid な入力では両案一致）。ただし §58.4 の「Boolean branch
  を pattern の腕に置いた形」の範囲を spec の明文より狭める。

## 完了条件

- どちらの読みを採るかを決め、採った側を `docs/rounds/canonical-in-syntax-errors.md`
  に書く。
- 決めた側を門番の検体（`incomplete-boolean-branch` の隣）で測る。

## 由来

`docs/wants.md` に無い。PR #44 の六次レビュー（2026-09-26）。§24、§26.1、§58.4。
