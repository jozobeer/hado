# 書き始める

必要なものは [README.md](../README.md)。リポジトリの根でコンパイラを作る。

```
$ cargo build --offline
```

根に `hello/` ディレクトリを作り、その中に `hello.hd` を書く。以後のコマンドも根から `cargo run` 経由で叩き、バイナリの出力先に依存しない。

```
$ cargo run --offline -q -- run hello/hello.hd
$ cargo run --offline -q -- check [--print-type] hello/hello.hd
$ cargo run --offline -q -- build hello/hello.hd
```

`run` は実行ファイルを一時ディレクトリに作り、隣に何も残さない。作業ディレクトリは変えない（[仕様 §58](spec.md#58-実行と出力先)）。`build` は `hello/hello` を入力の隣に置く。`hello/./hello.hd` でも同じである。

## hello

```ocaml
def main = "hello"
```

```
$ cargo run --offline -q -- run hello/hello.hd
hello
$ cargo run --offline -q -- check --print-type hello/hello.hd
String
$ cargo run --offline -q -- build hello/hello.hd
$ ./hello/hello
hello
```

`main` の値が stdout に出る。String の最上位は引用符を付けない（[仕様 §54](spec.md#54-印字)）。

## 関数

```ocaml
def greet name => name
def main = greet "hello"
```

呼び出しは並置である（[仕様 §13](spec.md#13-型注釈と呼び出し)）。`run` の結果は同じ `hello`。

## 失敗のレール

空の入力を保存して `"ok"` を返す。

```ocaml
def parse (input: Int List): Int List {Error} => input
def validate | [] => () | [_, ..._] => raise { message = "invalid" }
def save | [] => "ok" | [_, ..._] => raise { message = "save failed" }
def create_user input =>
  let parsed = parse input ->
  do validate parsed ->
  save parsed
def main = create_user []
```

```
$ cargo run --offline -q -- run hello/hello.hd
ok
$ cargo run --offline -q -- check --print-type hello/hello.hd
String {Error}
```

`let` は値を残す。`do` は `()` の段で、値を捨てて次へ進む。失敗は型の `{Error}` に現れ、呼び出し位置には印を書かない（[仕様 §34](spec.md#34-let-p-)、[仕様 §35](spec.md#35-do)）。

## 失敗

`def main = create_user [1]` に変える。

```
$ cargo run --offline -q -- run hello/hello.hd
invalid
```

終了コードは 1 で、`message` は stderr に出る（[仕様 §54](spec.md#54-印字)）。

## 冗長

```ocaml
def main =
  let x = "hello" ->
  x
```

```
$ cargo run --offline -q -- check hello/hello.hd
hello.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: "hello"
```

意味を変えずに消せる束縛はコンパイルできない（[仕様 §2](spec.md#2-正準形の原則)、[仕様 §41](spec.md#41-同一束縛)）。

言語の原則は [仕様 §1](spec.md#1-設計思想)。典型コードは [付録B](spec.md#付録b-典型コード)。
