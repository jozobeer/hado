# `do` が Option の不在を `?` なしで伝播する（do-option-absent）
優先度: 中。

## 現象

`do` の右辺が Option なら、`None` を伝播する。`?` の印は付かない。§34 は「`?` の有無が Option と効果を見分ける」とするので、Option に対する `do` だけがこの原則の例外になる。`let _? =` は E504 で `do` に寄せられる（2026-09-26）。

```
$ cat target/issue/opt/main.hd
def pass (n: Int): Void Option => if n > 0 => Some(()) | None
def f (n: Int) =>
  do pass n ->
  Some(n)
def main = f 0
$ cargo run --offline -q -- run target/issue/opt/main.hd
None
$ mkdir -p target/issue/e504
$ sed 's/do pass n ->/let _? = pass n ->/' target/issue/opt/main.hd > target/issue/e504/main.hd
$ cargo run --offline -q -- check target/issue/e504/main.hd
main.hd:3:7: error[E504]: use do to discard a success value; canonical form: do
```

受理コーパスと stdlib の `do` は、どれも効果の呼び出しである。Option に `do` を使う箇所は無い。

```
$ grep -rn '^\s*do ' tests/accept stdlib --include=*.hd
tests/accept/rail_forms/main.hd:11:    do pass n ->
tests/accept/rail_forms/main.hd:12:    do pass (n + 1) ->
tests/accept/delete_user/main.hd:9:  do authorize_delete user ->
tests/accept/env_io_not_directory/main.hd:6:  do write_file "ordinary" (Bytes.of_string "file") ->
tests/accept/env_io_roundtrip/main.hd:8:  do write_file "fixture" source ->
tests/accept/create_user/main.hd:10:  do validate parsed ->
tests/accept/create_user/main.hd:11:  do authorize parsed ->
```

不在を効果で表す形は、今の言語で書けて動く。`?` を書かずに `let` と `do` が不在を伝える。

```
$ cat target/issue/absent/main.hd
// needs: prelude
effect Absent
abort absent

handler Absent as Maybe
abort absent => None
return value => Some(value)

type User = { id: Int; name: String }
type Outcome = { found: String Option; missing: String Option }

def find_user (id: Int): User {Absent} =>
  if id == 1 => { id = 1; name = "alice" } | absent

def check_member (id: Int): Void {Absent} =>
  if id == 1 => () | absent

def greet id =>
  let user = find_user id ->
  do check_member user.id ->
  String.concat "hi " user.name

def lookup id =>
  with Maybe ->
  greet id

def main = { found = lookup 1; missing = lookup 2 }
$ cargo run --offline -q -- run target/issue/absent/main.hd
{ found = Some("hi alice"); missing = None }
```

## 原因

`do` は、Result があった版では Option と Result の成功値を捨てて失敗を伝播する構文だった。Result を効果に置き換えたとき、`do` の意味を「効果を持つ式の値を捨てて継続する」に移したが（`docs/plan.md` の `Q8`）、Option に対する伝播はそのまま残った。検査は `crates/hado/src/check.rs` の `discarded_binding` が `let _? =` を E504 にして `do` へ寄せ、`crates/hado/src/fixes.rs` の `discard_let_fix` がその書き換えを出す。

`do` の設計意図は「戻り値ではなく、実行することに意味がある」ことで、それを形にしているのは純粋な式の破棄を拒む E303 である。Option に対する `do` は、Option を Maybe モナドのように扱うため（`let x? =` が bind、`do opt` が then）に置かれた。効果に対しては「実行」、Option に対しては「中身の検査」と、`do` 1 語が 2 つの意味を兼ねている。

## 案

- 推奨: 不在も効果にする。prelude に `Absent` 効果と `None` へ戻す handler を置き、`let p? =` と Option に対する `do` を廃止する。Option はデータ（record の field やコレクションの要素）としてだけ残す。伝播は、どれも `->` の先へ進まないことになり、`do` は「効果を実行する」の 1 つの意味になる。Maybe の bind・then・run は `let`・`do`・`with` が受け持つ。決めることは次のとおり。
  - 関数の不在の表し方を効果に一本化する規則（§2）と、Option を返す stdlib の関数（`List.head` `List.nth` `Env.env`）の戻り型
  - `main` に `Absent` が残ったときの扱い（ユーザー定義の効果なら今は E706）
  - §33 の境界「理由の有無」の言い直し（Option と効果の境界から、`Absent` と `Error` の境界へ）。`docs/plan.md` の 17 節 Q7 と Q8 の書き直しになる
  - E303・E504・E507・E508 の再定義
  - 先に `handler-answer-types` を直す。`None` へ戻す handler は 1 つの本体の中で多くの型に使われるので、今は `build` が落ちる。既定値を handler で持ち込む形（`Option.unwrap_or` にあたるもの）は `handler-value-args` に依存する
  - 2026-09-26 の実験: 受理コーパスで `?` を使う 2 本（`rail_forms`・`user_store`）は効果版に書き直せて、同じ出力になった（`?` は 3 から 0）。データの Option を変換する形も `with Maybe -> f (present value)`（`def present | Some(x) => x | absent`）で `?` なしに書ける。不在を毎回 handler で回復する 10^6 回の検索は `?` 版の約 2 倍（1 回あたり約 16 ns）。`docs/ledger.md` の 17.9 の表の `Collect ⊃ Trap` は再現しない
- 次善: `do` から Option だけを外す。`let p? =` は残し、中身を使わずに `None` だけ伝播する形は `if o | Some(_) => … | None => None` を正準形にする（今は E508）。使用が 0 件の形なので、長く書くことを許す。
- 退ける: `let _? =` を正準形に戻す（綴りを却下）。`do` を Void 限定にし、Void 以外の値は `ignore` 関数で捨てる（冗長として却下。効果を持つ部分関数の値を捨てて検証だけする用途は `do Int.parse s ->` で足りる）。

## 完了条件

- 選んだ案が仕様（§33 から §36、§40、エラー表）に書かれ、Option に対する `do` が拒否されることを拒否コーパスが固定する。
- 推奨を選んだ場合、上の効果版が `effect` と `handler` の宣言なしに prelude だけで通り、同じ出力になる。
- `cargo test --offline` が 0 failed、`bash scripts/lint-docs.sh` が PASSED。

## 由来

2026-09-26 の相談。§33、§34、§35、§36、§38、§40、E303、E504、E508。関連する issue は `postfix-binding`（束縛を後置にすると、`do` の役割の見直しと重なる）。
