# 書き始める

必要なものは [README.md](../README.md)。リポジトリの根でコンパイラを作る。

```
$ cargo build --offline
```

以後は同じ根から、空ディレクトリに書いたファイルのパスを付ける。以下では `path/to/hello.hd` と書く。

```
$ target/debug/hado run path/to/hello.hd
$ target/debug/hado check [--print-type] path/to/hello.hd
$ target/debug/hado build path/to/hello.hd
```

`run` は一時ディレクトリで実行し、隣に何も残さない。`build` は入力の隣に実行ファイルを置く。渡したファイルのディレクトリがプログラムの根で、その下の `.hd` は全部検査される（[仕様 §50](spec.md#50-複数ファイル)）。このリポジトリの根や `tests/` `stdlib/` `tools/` に置くと、隣のプログラムと一つの検査になる。

## hello

```ocaml
def main = "hello"
```

```
$ target/debug/hado run path/to/hello.hd
hello
$ target/debug/hado check --print-type path/to/hello.hd
String
$ target/debug/hado build path/to/hello.hd
$ path/to/hello
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
$ target/debug/hado run path/to/hello.hd
ok
$ target/debug/hado check --print-type path/to/hello.hd
String {Error}
```

`let` は値を残す。`do` は `()` の段で、値を捨てて次へ進む。失敗は型の `{Error}` に現れ、呼び出し位置には印を書かない（[仕様 §34](spec.md#34-let-p-)、[仕様 §35](spec.md#35-do)）。

## 失敗

`def main = create_user [1]` に変える。

```
$ target/debug/hado run path/to/hello.hd
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
$ target/debug/hado check path/to/hello.hd
hello.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: "hello"
```

意味を変えずに消せる束縛はコンパイルできない（[仕様 §2](spec.md#2-正準形の原則)、[仕様 §41](spec.md#41-同一束縛)）。

言語の原則は [仕様 §1](spec.md#1-設計思想)。典型コードは [付録B](spec.md#付録b-典型コード)。
