# hado 言語仕様 v0.3.1

この文書は hado v0.3.1 の言語仕様である。処理系、`stdlib/`、`tests/accept`・`tests/reject`・`tests/library` のコーパスは、この文書の規則に従う。規則の根拠を別の文書に置かず、食い違ったときはこの文書を正とする。

版 v0.3.1 は git tag `v0.3.1` に対応する。

## 規範性

規則は「〜とする」「〜でなければならない」で書く。違反は診断になり、`E` で始まるコードを持つ。「〜してよい」は許可、「〜しない」「持たない」「禁止する」は禁止を表す。エラーコードの一覧、検査の段階順、診断のアンカーは第56節にある。

## 目次

- **[第I部 言語の狙い](#第i部-言語の狙い)**
  - [1. 設計思想](#1-設計思想)
  - [2. 正準形の原則](#2-正準形の原則)
  - [3. 言語全体の原則](#3-言語全体の原則)
  - [4. 入れないもの](#4-入れないもの)
- **[第II部 字句と文法](#第ii部-字句と文法)**
  - [5. リテラルとコメント](#5-リテラルとコメント)
  - [6. 記号体系](#6-記号体系)
  - [7. 演算子と優先順位](#7-演算子と優先順位)
- **[第III部 定義と式](#第iii部-定義と式)**
  - [8. 名前付き定義](#8-名前付き定義)
  - [9. 再帰](#9-再帰)
  - [10. ローカル束縛](#10-ローカル束縛)
  - [11. 継続 `->`](#11-継続--)
  - [12. 匿名関数](#12-匿名関数)
  - [13. 型注釈と呼び出し](#13-型注釈と呼び出し)
  - [14. パイプライン](#14-パイプライン)
- **[第IV部 型](#第iv部-型)**
  - [15. 型推論](#15-型推論)
  - [16. 型引数](#16-型引数)
  - [17. 代数データ型](#17-代数データ型)
  - [18. レコード](#18-レコード)
  - [19. プリミティブ型](#19-プリミティブ型)
  - [20. 関数型](#20-関数型)
  - [21. 構造比較](#21-構造比較)
  - [22. List リテラル](#22-list-リテラル)
- **[第V部 分岐と網羅性](#第v部-分岐と網羅性)**
  - [23. 分岐](#23-分岐)
  - [24. Boolean 分岐](#24-boolean-分岐)
  - [25. 条件列](#25-条件列)
  - [26. パターン分岐](#26-パターン分岐)
  - [26.1. パターンの腕の条件列](#261-パターンの腕の条件列)
  - [27. パターン分岐の fallback](#27-パターン分岐の-fallback)
  - [28. 網羅性](#28-網羅性)
  - [29. `_` の用途](#29-_-の用途)
  - [30. 分岐のネスト禁止](#30-分岐のネスト禁止)
  - [31. 引数のパターン分岐](#31-引数のパターン分岐)
  - [32. 引数の条件列](#32-引数の条件列)
- **[第VI部 失敗と効果](#第vi部-失敗と効果)**
  - [33. Option と失敗](#33-option-と失敗)
  - [34. `let p? =`](#34-let-p-)
  - [35. `do`](#35-do)
  - [36. 伝播と分岐](#36-伝播と分岐)
  - [37. 効果](#37-効果)
  - [38. handler と `with`](#38-handler-と-with)
- **[第VII部 正準性の検査](#第vii部-正準性の検査)**
  - [39. 未使用束縛](#39-未使用束縛)
  - [40. 値の破棄](#40-値の破棄)
  - [41. 同一束縛](#41-同一束縛)
  - [42. 同一 branch](#42-同一-branch)
  - [43. 到達不能 branch](#43-到達不能-branch)
  - [44. 重複 branch](#44-重複-branch)
  - [45. 不要な再包装](#45-不要な再包装)
- **[第VIII部 モジュールと構成](#第viii部-モジュールと構成)**
  - [46. 契約](#46-契約)
  - [47. 実装](#47-実装)
  - [48. Functor](#48-functor)
  - [49. 名前空間](#49-名前空間)
  - [50. 複数ファイル](#50-複数ファイル)
  - [51. ディレクトリと名前空間](#51-ディレクトリと名前空間)
  - [52. ライブラリ](#52-ライブラリ)
- **[第IX部 実行と入出力](#第ix部-実行と入出力)**
  - [53. 末尾呼び出しの保証](#53-末尾呼び出しの保証)
  - [54. 印字](#54-印字)
- **[第X部 prelude](#第x部-prelude)**
  - [55. prelude](#55-prelude)
- **[第XI部 診断](#第xi部-診断)**
  - [56. エラーコード](#56-エラーコード)
- **[付録](#付録)**
  - [付録A コア構文](#付録a-コア構文)
  - [付録B 典型コード](#付録b-典型コード)
  - [付録C 文法の概形](#付録c-文法の概形)

# 第I部 言語の狙い

## 1. 設計思想

この言語は、OCaml / ML系言語の型システム・ADT・抽象化の良さを核にしつつ、より小さく、より平坦に、より一意に書けることを重視する。

中心思想は次の通り。

* Small language
* Immutable first
* Expression oriented
* Strong static typing
* ML-style type inference
* ADT first
* Exhaustive branching
* Effects are explicit in the type
* Failure is an effect, absence is `Option`
* Propagation is implicit, handling is deliberate
* Flat control flow
* Canonical syntax
* Redundant code is rejected
* Machine-checkable coding conventions belong in the language

特に重要な原則は次の2つ。

> **Control flow should be flat.**

> **If code can be removed without changing meaning, it should not compile.**

## 2. 正準形の原則

同じ意味を持つ複数の表現を、可能な限り許可しない。

より短く、より直接的で、意味的に同値な表現が存在するなら、それをcanonical formとする。

例えば、

```ocaml
let x = value ->
x
```

は禁止。

canonical formは、

```ocaml
value
```

同様に、

```ocaml
if condition => x | x
```

は禁止。

canonical formは、

```ocaml
x
```

この言語では、

> **正しいが冗長なコード**

もコンパイルエラーになり得る。

引数をそのまま別の関数に渡すだけの `def`（eta 展開）は冗長としない。

## 3. 言語全体の原則

この言語では、

> **ネストよりフラット**

> **暗黙より型**

> **例外より効果**

> **分岐より伝播**

> **複数表現よりcanonical form**

> **警告より、書けないこと**

を優先する。

最終的な目標は、

> **間違ったコードだけでなく、冗長なコード、曖昧なコード、構造的に悪いコードまで、可能な限り言語仕様そのもので排除すること。**

そして最も重要な原則は、

> **If code can be removed without changing meaning, it should not compile.**

## 4. 入れないもの

以下は意図的に外す。

```text
mutation
null
unchecked exceptions
objects
classes
inheritance
GADT
polymorphic variants
first-class modules
macros
traits / typeclasses
implicit conversion
operator overloading
async / await
concurrency primitives
```

operator overloading の禁止は維持する（E107）。組み込み算術演算子は、ユーザーが定義できない固定集合への型指向解決である。

必要になった段階で追加を検討する。

# 第II部 字句と文法

## 5. リテラルとコメント

Int は 10 進のみとする。i64 の範囲を超えるリテラルは E112 とする。

Float は小数点の両側に数字を必須とする。`.5` や `5.` は文法に含めない（E103）。

Char は `'a'` の形とする。値は Unicode スカラ値である。エスケープは `\n` `\t` `\\` `\'` に限る。ちょうど 1 文字でない場合と、それ以外のエスケープは E111 とする。

String は `"..."` の形とする。エスケープは `\n` `\t` `\\` `\"` に限る。それ以外のエスケープは E110 とする。

Bool は `true` と `false` とする。

Unit は `()` とする。

コメントは `//` による行コメントのみとする。`--`（v0.1 の記法）と `#` を含む他の記法によるコメントは E104 とする。

第59節の文法に合わない入力で、特定のコードを持つ規則に当たらないものは E100 とする。

## 6. 記号体系

記号の意味を明確に分ける。

```text
=    value binding / value definition / functor application
=>   function body separator / pattern result separator / Boolean result separator / function type
->   local continuation separator
|    branch alternative / fallback marker
|>   data flow
?    failure-aware binding (pattern suffix)
...  rest of a List
::   namespace path separator
.    member access / field access
```

例えば、

```ocaml
fn(x => x + 1)
```

は、

> xを受け取ったならx + 1を返す

```ocaml
def add x y => x + y
```

も、

> xとyを受け取ったならx + yを返す

```ocaml
if user
| Guest => "guest"
| Member(user) => user.name
```

は、

> userがGuestなら `"guest"`、Memberなら名前

```ocaml
let user? = find_user id ->
let account? = find_account user.account_id ->
Some(account)
```

は、

> 成功したら値を束縛して次へ進む

という読みになる。

## 7. 演算子と優先順位

演算子は次に限る。算術 `+ - * / %`、比較 `< <= > >= == !=`、論理 `&&` `||` `not`。

算術 `+ - * /` と順序比較 `< <= > >=` は Int / Float 両用の組み込み多相であり、単相化時に確定する。未確定なら Int とする。`def` の一般化の時点で確定しない場合もその `def` の中で Int に固定するので、`def div x y => x / y` は `Fn(Int, Int => Int {Error})` であり、`div 1.0 2.0` は E305 である。`%` は Int 専用であり、Float に当てると E305 である。`/` と `%` の戻り型は第19節に従う。

文字列連結演算子は置かない。

優先順位は高い方から、field access `.`、適用、単項 `-` と `not`、`* / %`、`+ -`、比較、`&&`、`||`、`|>` とする。比較は非結合である。

`&&` と `||` は短絡評価する。左辺が結果を決めるとき、右辺を評価しない。

二項演算子の被演算子は左から右に評価する。適用の引数も、パイプの左辺を第1引数として、書かれた順に評価する。効果を持つ式を並べたときの順序はこれで定まる。

# 第III部 定義と式

## 8. 名前付き定義

外（第50節）や `implement` 節の名前付き定義には `def` を使う。

値の定義：

```ocaml
def pi = 3.14159
```

関数定義：

```ocaml
def add x y => x + y
```

`def` は関数専用ではなく、

> 名前を持つdefinition

を表す。

`def` の構文は次の2種類に統一する。

```text
def name = expression
def name parameters return_type? => expression
```

関数定義では、引数列の終端を `=>` で明示する。引数に型注釈を付ける場合は、括弧付き引数構文を使う。

```ocaml
def add(x: Int, y: Int): Int =>
  x + y
```

したがって、次の形式は同じ関数定義構文として許可する。

```ocaml
def add x y => x + y
```

```ocaml
def add(x: Int, y: Int): Int =>
  x + y
```

`def name = expression` は値定義専用であり、関数定義に `=` を使わない。

関数定義の引数は 1 個以上である。引数を持たない `=>` の定義（`def f => 1`）は E109 とする。使わない引数は `_` で受けるが、書ける場所は第39節が限る。引数が 1 つで、その引数を分岐の入口としてしか使わない関数は、第31節の形で書く。本体が条件列だけの関数は、第32節の形で書く。

## 9. 再帰

再帰は明示する。

```ocaml
def rec factorial n =>
  if n <= 1 => 1 | n * factorial (n - 1)
```

暗黙の自己再帰は許可しない（E201）。

`def rec` は、値定義ではなく関数定義にのみ使用できる。値定義に `rec` を付けると E102 とする。

top-level は定義順に依存しない。呼び出しグラフで異なる2つ以上の定義を通る閉路に含まれる関数は全て `rec` とする（E208）。`rec` のない関数の自己参照には E201 を適用する。閉路に値定義が含まれるとエラーである（E207）。値定義は `rec` を持てない。

多相再帰はコンパイルエラーとする（E307）。E307 は、`rec` 関数の再帰呼び出しにおいて、引数の型と仮引数の型の単一化が出現検査に失敗した場合である。それ以外の出現検査の失敗は E306（第15節）とする。 別の関数への引数や戻り値を経由する間接的な再帰も対象とする。再帰呼び出しの位置を追跡できる場合はその先頭を、関数を経由して位置を追跡できない場合は再帰関数の `def` の先頭を指す。

## 10. ローカル束縛

ローカルな値の束縛には `let` を使う。

```ocaml
let x = calculate ->
use x
```

`let` は名前付きdefinitionには使わない。

```text
def = named definition
let = local binding
```

`let` の本体は `->` の右側に続く継続である。`let` は値を束縛するために使い、成功・失敗の伝播は行わない。

プログラムの入口は、根のファイル（第50節）の外に置く `def main` である。`main` は `let` によるローカルbindingではなく、名前付き定義である。`main` を持つ根のファイルが入口ファイルで、1 つのディレクトリに複数あってよい。`hado build` と `hado check` に渡したファイルの `main` が起点になる。渡したファイルに `main` が無いとき、`hado check` はそのディレクトリをライブラリとして検査し（第52節）、`hado build` は E206 とする。

`main` に残ってよい効果は prelude の効果（`Error` `Env` `Io`。第55節、第37節）に限る。ユーザーが宣言した効果が `main` の型に残れば E706 とする。`main` の効果行は一般化せず、本体が持つ効果で閉じる。

`main` の型は単相で、Fn と、`shared` で解けない抽象型を含んではならない。`def main = []` は型変数が残るため E701、`def main = inc` は印字できず Fn を含むため E702、`def main = UserStore.find 1` は `shared` で解けない `UserStore.T` を含むため E705 とする。`shared` で具体型に解ける抽象型は E705 の対象外で、解けた型として印字してよい（第47節）。

`main` の要求は実行ファイルという成果物の規則である。`main` が無いファイルを `hado build` に渡したとき、または `main` が引数を持つプログラムは E206 とする。`main` の値の印字は第54節に従う。

## 11. 継続 `->`

`->` は、型の関数矢印や分岐結果の矢印には使わない。

`->` はローカルbindingの継続を示す記号としてのみ使う。

```ocaml
let x = foo ->
let y = bar x ->
baz y
```

構造としては、

```text
binding -> continuation
```

となる。

失敗を伝播する `let p? =` と `do` も同じ継続構文を使う。

```ocaml
let user? = find_user id ->
use user
```

```ocaml
do validate input ->
next
```

`->` は、関数型の表記には使わない。関数型は第20節で定める `Fn(...)` を使う。

`main` は `def main = expression` の形で、`=>` を持たない。本体は expression なので、`let` や `do` の継続を含んでよい。

## 12. 匿名関数

匿名関数には `fn` を使い、全体を括弧で閉じる。

```ocaml
fn(x => x + 1)
```

複数引数はカンマで区切る。型注釈は引数ごとに書く。

```ocaml
fn(x, y => x + y)
fn(x: Int, y: Int => x + y)
```

`=>` は、関数の引数列と本体を分ける。

名前付き関数も同じ規則を使うが、`def` は次の項目で終わりが決まるので括弧で閉じない。引数は空白で区切る。

```ocaml
def add x y => x + y
```

`=>` は関数定義と匿名関数と関数型（第20節）においてのみ、入力と結果の対応を表す。

匿名関数も、引数のパターン分岐の形（第31節）と条件列の形（第32節）を取れる。どちらも `)` で閉じる。

```ocaml
fn(
| Guest => "guest"
| Member(user) => user.name
)
```

```ocaml
fn(x
| x > 1 => x
| 0
)
```

括弧で閉じるので、`List.map users fn(user => user.name)` のように並置の引数へ直接置ける。

## 13. 型注釈と呼び出し

関数型の表現に `->` は使わない。

引数と戻り値は、モダンな構文に寄せる。

```ocaml
def add(x: Int, y: Int): Int =>
  x + y
```

型推論できる場合は省略できる。

```ocaml
def add x y => x + y
```

関数型を明示する必要がある場合は、`Fn(...)` を使う。

```text
Fn(Int => String)
Fn(Int, Int => Int)
```

`Fn` は n 引数の関数型である。定義の引数数と呼び出しの引数数は一致しなければならない。部分適用は許可しない（E301）。

`def` で定義した関数名を引数なしで参照すると、`Fn(...)` 型の値になる。

関数の呼び出しは並置 `f x y`、位置payloadのconstructorは `Some(x)`、名前付きfieldのconstructorは `Payload { id = 1 }` と呼び出す。大文字で始まる名前は constructor である。constructor を先頭に置く並置（`Some 1`）は E106 とする。`inc(1)` は `inc (1)` と同じ並置である（空白は意味を持たない。第59節）。

constructor は関数値ではない。payload を持つ constructor は必ず payload を伴い、payload の数と引数の数が異なる形（単独の `Some`、`Some(1, 2)`、`apply Some`）は E301 とする。宣言されたpayloadの形式（位置の `()` と名前付きの `{}`）と呼出し・パターンの形式が違う場合も E301（constructor名）とし、constructorを解決した後の型段階で検査する。

## 14. パイプライン

`|>` を採用する。

```ocaml
x |> f
```

は、

```ocaml
f x
```

と同義。

パイプは第 1 引数を埋める。`x |> f a b` は `f x a b` である。標準ライブラリの引数順は data-first とする。

```ocaml
users
|> List.filter is_active
|> List.map display_name
|> List.sort compare
```

`x |> f a b` は `f x a b` なので、`users |> List.map fn(u => u.name)` は `List.map users fn(u => u.name)` である。

`|>` にOptionや効果の特殊意味は持たせない。

`|>` の右辺は値名前空間の関数（名前、適用、`fn(...)`）に限る。constructor を置くこと（`x |> Some`）は E106 とする。

# 第IV部 型

## 15. 型推論

ML系の型推論を採用する。

```ocaml
def identity x => x
```

は型注釈なしでも多相関数として推論される。

通常のコードでは型注釈を大量に要求しない。

推論の途中で型が食い違う場合は E305 とする。型変数がそれ自身を含む型に束縛される場合
（出現検査の失敗）は E306 とする。ただし再帰呼び出しに起因する出現検査の失敗は E307（第9節）とする。

## 16. 型引数

型引数は型名の前に置く（後置の適用）。2 つ以上は括弧で囲む。

```ocaml
T Option
(K, V) Pair
T List
Int List Option          // (Int List) Option。左から結合する
```

型定義では、型変数を名前の前で宣言する。

```ocaml
type T Option =
  | None
  | Some(T)
```

```ocaml
type (K, V) Pair =
  | Pair(K, V)
```

宣言した型変数が右辺に現れない型定義（`type T Phantom = | Phantom(Int)`）は E518 とする。契約の抽象型の実体化（第47節）と契約の関数の型変数（第46節）にも同じ規則を当てる。

型定義の閉路（自己再帰と相互再帰）の中では、型引数は宣言の型引数と同一でなければならない。

```ocaml
type T Nest =
  | Leaf(T)
  | Node(T List Nest)
```

は E302 とする。派生パスの実体化を有限に保つためである。

## 17. 代数データ型

ADTは言語の中心機能。

payloadなし：

```ocaml
type Color =
  | Red
  | Green
  | Blue
```

payload付き：

```ocaml
type Shape =
  | Circle(Float)
  | Rectangle(Float, Float)
```

`of` は使わない。constructor の列は先頭の `|` を必須とする。ただし名前付きfieldを持つconstructorが1つだけなら `type T = Payload { id: Int; name: String }` と先頭の `|` を省ける。1 行に書く場合も同じである（`type Color = | Red | Green | Blue`）。この省略形以外で先頭の `|` が無いconstructorの列は E108 とする。省略形の `{...}` の後に `|` が続けば、省略形として確定せず E108 とし、最初のconstructor名を指す。

位置で渡すpayloadは `Circle(Float)` のように `()` に型を書く。名前で渡すpayloadは `Rectangle { width: Float; height: Float }` のようにconstructor名の後へ直接 `{}` で書く。同じ型のconstructor列に両方の形を置ける。`Member({ name: String })` のように `()` の中へ無名record型を書くと E100 とする。

値生成も同じ形。

```ocaml
Circle(10.0)
Rectangle(10.0, 20.0)
```

constructor が全て payload なしの型は素の整数とする。payload を一つでも持つ型は、全 constructor を一律に GC ヒープの box にする。一つの型の中で即値と pointer を混在させない。

## 18. レコード

Recordを持つ。

```ocaml
type User = {
  name: String;
  age: Int
}
```

`;` は field の区切りである。record 式（下記）と同じ規則で、最後の field の後には置かない。同じ record 型の中で field 名は一意である（E205）。

アクセス：

```ocaml
user.name
```

構築は次の形とする。

```ocaml
{ name = "a"; age = 3 }
```

型は field 集合から一意に決まる nominal 型である。一意に決まらない（該当する型が無い、または 2 つ以上ある）場合は注釈を要求する（E304）。record 式の中で同じ field を 2 回書くことは E205 とする。

field 参照は、プログラム全体の型推論を終えても参照先の record 型が決まらなければ E304 とする。アンカーは field 参照式の先頭とする。後続の呼び出しや型注釈で型が決まる場合は受理する。型が既に決まっていて、その型に field が無い場合は E204 とする。

パターンは `{ name }`（同名束縛）と `{ name = n }`（別名）とする。書かない field は無視する。field 更新構文は持たない。

record は必ず `type` で名前を持つ。ADTとの組み合わせは、名前で参照する。

```ocaml
type Profile = {
  name: String;
  age: Int
}

type User =
  | Guest
  | Member(Profile)
```

名前付きfieldを持つconstructorは、型名とconstructor名を分けて宣言する。`Payload`は予約語ではなくconstructor名である。

```ocaml
type T = Payload { id: Int; name: String }
def make n => Payload { id = n; name = "a" }
def describe user => let Payload { name } = user -> name
```

宣言・生成・パターンのfield区切りは `;`、末尾の区切りは置かない。宣言と生成は1つ以上のfieldを持つ。生成では宣言の全fieldを1回ずつ書き、順序は自由とする。式の評価順は書かれた順である。名前付きfieldのconstructorを `Payload(1, "a")` と呼ぶこと、位置payloadやpayload無しのconstructorを `Some { value = 1 }` や `Guest { x = 1 }` と呼ぶことは E301 とし、constructor名を指す。payloadを伴わないconstructor参照は従来どおり E301 とする。

生成・パターンの未知fieldは E204（field名）、重複fieldは E205（2回目のfield名）、生成のfield不足は E301（constructor名）、field値の型不一致は E305（値の先頭）とする。宣言の重複fieldも E205 で2回目のfield名を指す。constructor名で型が決まるため、同じfield集合を持つ別型があっても E304 にはならない。

パターンは `Payload { id; name = n }` と書く。fieldを1つ以上書き、同名束縛・別名・入れ子と、書かないfieldの無視はrecordパターンと同じである。名前付きfieldを持つconstructorを `Payload(...)` と位置形式で分解すること、位置payloadやpayload無しのconstructorを `{}` 形式で分解することは E301（constructor名）。payloadを持つconstructorを裸の `Payload` でパターンに書くことも E301 とする。複数constructorを持つ型の分岐網羅性とletの全域性は第28節・第34節の規則で判定する。

constructorが1つだけで名前付きfieldを持つ型は `user.name` でfieldを参照できる。存在しないfieldは E204（field名）。参照先の型が未確定なら、通常のrecordと単一の名前付きfield constructorの型を候補とし、後続の型注釈・呼出しによる制約も反映する。全体の推論後も一意に決まらなければ E304（参照式の先頭）である。複数constructorの型は先にパターンで分解する。複数constructorの型への直接のfield参照は E204（field名）である。契約の抽象型を実装の外からfield参照すると、constructorの形によらず E601（`.`）とする。constructor付きの値を裸のrecord式・パターンからfield集合で選ぶことはしない。

型の別名（`type Id = Int`）は持たない。値を区別したいときは constructor が 1 つの ADT を使う（`type Id = | Id(Int)`）。型 `Id` と constructor `Id` は別の名前空間にある（第49節）。

## 19. プリミティブ型

最低限：

```text
Unit
Bool
Int
Float
Char
String
Bytes
```

Int は i64 とする。加減乗はオーバーフローで wrap する。`/` と `%` は `Int {Error}` を返す。除数 0 は `raise` し、`Exception` の `message` を `"DivisionByZero"` とする（第33節、第54節）。

Int の `/` と `%` は `{Error}`。String、Bytes、Array の範囲を要求する操作も `{Error}` とする（第55節）。§55 が `{Error}` と定める member 以外の組み込みは効果を持たない。

除算の端は次とする。`i64::MIN / -1` は wrap して `i64::MIN`。`i64::MIN % -1` は `0`。

Float は f64 とする。Float の `/` は Float を返す。Float に `%` は置かない。

Bool は i8、Char は i32、Unit は i64 の 0 とする。Cranelift に i1 はない。

Char は Unicode スカラ値とする。String は不変の UTF-8 とする。Bytes は不変のバイト列とする。`String.length` はスカラ値の個数、`Bytes.length` はバイト数である。Array は要素型を 1 つ取る不変の列で、`get` は O(1)、`set` は新しい Array を返す。

parameterized type：

```text
T Option
(K, V) Pair
T List
T Array
```

## 20. 関数型

関数型については `Int -> String` のようなML記法を採用しない。

関数型は `Fn(引数列 => 戻り)` で表す。必ず括弧で閉じる。

単一引数：

```text
Fn(Int => String)
```

複数引数：

```text
Fn(Int, Int => Int)
```

関数を返す関数：

```text
Fn(Int => Fn(Int => Int))
```

`Fn(Int, Int => Int)` の `Int, Int` は引数列であり、tuple は持たない。式やパターンに tuple の構文を書くと E105 とする。型式の `(Int, String) Pair` は 2 つ以上の型引数を囲む括弧で、tuple ではない（第16節）。

`Fn` は n 引数である。定義と呼び出しの引数数は一致必須であり、部分適用は持たない。規則は第13節に従う。

全ての hado 関数は環境を第 1 引数に取る。名前付き関数は環境を無視する。単相の `Fn` 型ごとに `tail` 規約の signature が一つに定まる。

signatureでは通常、

```ocaml
def parse(input: String): User {Error}
```

のように書く。

関数が持つ効果は、戻り型の後ろに波括弧で並べる。

```ocaml
def read_config(path: String): Config {Io, Error}
def parse(input: String): User {Error}
```

行が現れるのは矢印の結果位置だけである。`return_type` と `Fn` の結果に置き、型式の一般の位置には現れない。

```ocaml
Fn(A => B {E})
```

行の順序は意味を持たない。印字の正準形はアルファベット順とし、`{Error, Io}` と出す。同じ効果を 2 回書くと E522 とする。効果変数の実体化で重複が生じた場合は診断せず 1 つに潰す。

行は書かなくてよい。書かなければ推論に任せ、暗黙の効果変数として全称量化する。効果変数は型変数と違い、名前の前に宣言しない。

```ocaml
def (A, B) map(values: A List, f: Fn(A => B)): B List
```

効果を持たないことを主張するには `Pure` を書く。

```ocaml
def A length(values: A List): Int Pure
```

書いた行はその通りの意味である。`{Io}` と書いた関数が他の効果も持てば E305 とする。行の状態は 3 つで、省略は推論、`Pure` は効果なし、具体行はちょうどそれだけである。

`->` は関数型の表記には使わず、継続separatorに限定する。

## 21. 構造比較

`==` と `!=` は、Fn を含まない全型の構造比較である。M5 の派生パスが型ごとに合成する。

Fn を含む型への `==` `!=` は E703、`compare` は E704 とする。単相化時に判明する。

`compare a b` は順序に応じて `-1`、`0`、`1` を返す。`compare` は値の空間の組み込みで、修飾なしで参照する（第49節）。FloatではNaN同士を同順位とし、NaNを他の全ての値より後に置く。`-0.0` と `+0.0` は同順位とする。Floatの `==`・`!=` はIEEE 754に従い、NaN同士の `==` はfalseである。Bytes の `==` `!=` と `compare` はバイト列の構造比較で、String と同じ規則による。Array の `==` `!=` と `compare` は要素型から派生する。要素型が Fn を含めば E703 / E704 とする。

Array の `compare` は要素を先頭から比較して最初に異なる要素の順序を返し、共通部分がすべて等しいときだけ短い方を先とする。

## 22. List リテラル

List は式でもパターンでも `[]` と `[a, b, ...rest]` の形で書く。`[a, b, ...rest]` は `Cons(a, Cons(b, rest))` の糖衣で、`[]` は `Nil`、`[a, b]` は `Cons(a, Cons(b, Nil))` である。`...` は末尾にだけ置ける。

式では `...` の後に任意の式を置ける。

```ocaml
def rec map values f =>
  if values
  | [] => []
  | [head, ...tail] => [f head, ...map tail f]
```

パターンでは `...` の後は名前か `_` である。`[x, ...xs]` は先頭と残りを束縛し、`[x, ..._]` は先頭だけを束縛する。

`...` の後ろに要素を置く形（`[...init, last]`、`[first, ...middle, last]`）は持たない。cons list は末尾に O(1) で触れず、照合の中に走査と `init` `middle` の割り当てが隠れるためである。末尾が要るときは `List.reverse` してから先頭で照合する。

`Nil` と `Cons` を利用者のコードに書くことは冗長である。式に書けば E510、パターンに書けば E516 とする。`List` 型の定義（`type T List = | Nil | Cons(T, T List)`）は prelude が持ち、constructor は糖衣の展開先としてだけ存在する。

# 第V部 分岐と網羅性

## 23. 分岐

`match` は持たない。

分岐は `if` に統一する。

ただし、`if` はBoolean専用ではなく、次の3種類のbranch expressionを持つ。

1. Boolean二分岐
2. 条件列
3. pattern分岐

すべてexpressionであり、値を返す。pattern 分岐の scrutinee は 1 つ以上で、2 つ以上はカンマで並べる（第26節）。

`if` の後に続く構文によって、どの種類の分岐かを決定する。

リテラルパターン（Int / Float / Char / String / Bool）は pattern 分岐の pattern として使える。

## 24. Boolean 分岐

Booleanの二分岐には次の構文を使う。

```ocaml
if condition => yes | no
```

例えば、

```ocaml
if x > 10 => Large | Small
```

これはBooleanの完全な二分岐である。

`then` / `else` は持たない。

Boolean branchでは、`|` の右側は必ずelse側の式であり、fallback構文ではない。

Bool の値を分岐する形はこの 1 つに限る。同じ意味になる Bool リテラルの pattern 分岐（`if b | true => x | false => y`、`if b | true => x | y`）と、腕が 1 本だけの条件列（`if | b => x | y`）は E520 とする。第31節の形（`def f | true => x | false => y`）も同じである。入れ子の中の Bool リテラルパターン（`Some(true)`）と、列が 2 つ以上の pattern 分岐の Bool リテラル（第26節）は対象外である。

## 25. 条件列

複数条件の分岐には、フラットな条件列を使う。

```ocaml
if
| score >= 90 => Excellent
| score >= 70 => Good
| score >= 50 => Pass
| Fail
```

条件列では、各branchの左側はBoolean expressionである。

上から順に評価し、最初に成立したbranchを採用する。

最後の、

```ocaml
| Fail
```

は必須のfallbackである。

条件列にfallbackがない場合はコンパイルエラーとする（E401）。

`if / else if / else` の構文は持たない。

関数の本体が条件列だけのときは、`=> if` を省いた第32節の形で書く。

## 26. パターン分岐

ADT や Option などの値をpatternで分岐する場合は、scrutineeを `if` の直後に置く。

```ocaml
if user
| Guest => "guest"
| Member(user) => user.name
```

Option：

```ocaml
if value
| Some(x) => x
| None => default
```

Optionの分岐：

```ocaml
if slot
| Some(value) => value
| None => fallback
```

pattern branchでは、各branchはpatternと `=>` と式で構成する。

```text
| pattern => expression
```

scrutinee はカンマで 2 つ以上並べられる。腕の pattern も同じ数をカンマで並べ、左から順に対応する。数が合わない腕は E125 である。scrutinee の列と pattern の列は関数型の引数列（第20節）と同じ読みで、tuple の値は作らない。fallback は列の数に関わらず式 1 つである。

```ocaml
if left, right
| [], _ => right
| _, [] => left
| [a, ..._], [b, ..._] => a + b
```

式の側の `=>` の後には条件列を置ける（第26.1節）。

pattern branchは、ADTの構造に基づいて網羅的でなければならない。

リテラルパターンを持つ。Int / Float / Char / String / Bool と、入れ子の中で使える。Int と Float のリテラルパターンは先頭に `-` を置け、それはリテラルの一部である。Bool 以外のリテラルパターンを使う分岐は fallback 必須である。Bool の値そのものを分岐する形は第24節の Boolean branch に限る（E520）。

最上位の変数パターン `| x => ...` は禁止する（E512）。入れ子の中の変数パターンは許可する。列が 2 つ以上のときは、全列が変数か `_` で変数を 1 つでも含む行を E512 とし、一部の列だけが変数か `_` の行は許可する。

条件列の全条件が「同一変数 `==` リテラル」なら、pattern 分岐で書けるため拒否する（E511）。

## 26.1. パターンの腕の条件列

pattern branch の腕の本体には、`=>` の後に条件列（第25節）を置ける。

```ocaml
if left, right
| [], _ => right
| _, [] => left
| [a, ...rest_a], [b, ...rest_b] => if
  | comparator a b <= 0 => [a, ...merge rest_a right comparator]
  | [b, ...merge left rest_b comparator]
```

腕の条件列は fallback で終わり、その後の `|` は外側の次の腕である。したがって腕の条件列の中で fallback より後の腕（E404）は起きない。条件列の他の規則（fallback の必須 E401、重複条件 E406、リテラル比較の列 E511）はそのまま当て、E401 と E511 のアンカーはその腕の `if` である。fallback が必須なので、条件列を持つ腕は必ず値を返す。網羅性（第28節）、到達不能（第43節）、重複（第44節）は、その腕を pattern の行を全域で覆う腕として数える。

条件が 1 本の腕の条件列（`| p => if | c => a | b`）は E520 の対象外である。Boolean branch を腕に置く形（`| p => if c => a | b`）が E101 なので、条件列がこの意味の正準形である。

fallback を持たない腕の条件列は、続く `|` の腕を自分の条件として読む。`| Some(x) => if | x > 0 => x | None => 0` の `None` は条件として読まれ、Bool でないので E305 になる。

腕の本体に置ける分岐はこの条件列だけである。Boolean branch と pattern branch を腕に置く形、および条件列の腕の中の分岐は E101 である（第30節）。

## 27. パターン分岐の fallback

pattern branchでは、catch-all fallbackを次の構文で表す。

```ocaml
| default
```

例えば、

```ocaml
if option
| Some(x) => x
| default
```

これは、`Some(x)` に一致しなかった場合に `default` を評価する。

Optionでも同様である。

```ocaml
if result
| Some(x) => x
| default
```

最後の `| expression` は、pattern branchにおけるfallbackとしてのみ解釈する。

条件列では、最後の `| expression` もfallbackであるが、条件列のfallbackは条件なしの最後のbranchとして扱う。

## 28. 網羅性

pattern branchは必ず網羅的でなければならない。

```ocaml
if value
| Some(x) => x
```

はコンパイルエラー（E402）。

`None` が未処理であり、fallbackも存在しないためである。

次のいずれかを満たす必要がある。

* すべてのconstructorを明示する
* 最後にfallbackを置く

非網羅branchはwarningではなくerror。

`let` のパターンにも同じ規則を当てる。`?` の有無を問わない（第34節）。

## 29. `_` の用途

`_` は完全禁止ではない。

pattern内部の一部だけを無視するときに使う。

```ocaml
| Member(name, _) => name
```

これは許可。

しかし、catch-all patternとしての、

```ocaml
| _ => default
```

は禁止（E503）。

fallbackのcanonical formは、

```ocaml
| default
```

である。

列が 2 つ以上の pattern 分岐（第26節）では、一部の列の `_` は入れ子の一部と同じ扱いで許可し、全列が `_` の行だけを E503 とする。

`()` はパターンではない。Unit の値は名前か `_` で受ける。

## 30. 分岐のネスト禁止

branch expressionの内部にbranch expressionを書くことは禁止する（E101）。

禁止：

```ocaml
if user
| Guest =>
    if enabled => A | B
| Member(user) =>
    C
```

禁止：

```ocaml
if a => (if b => x | y) | z
```

設計原則：

```text
腕の本体に置ける分岐は条件列だけで、条件列の腕には分岐を置けない
```

pattern branch の腕の本体には条件列を置ける（第26.1節）。条件列の腕の中には分岐を置けないので、深さは pattern と条件の 2 段で止まる。

必要であれば、先に状態を分類する。

```ocaml
let state = classify input ->

if state
| A => ...
| B => ...
| C => ...
```

ただし、`classify` の内部で分岐を行うこと自体を禁止するものではない。禁止対象は、branch expression の腕の中に branch expression を置くことである（第26.1節の腕の条件列を除く）。腕の中の `let` の継続や、腕の中の `fn(...)` の本体に置いた branch も同じである。`fn(| ...)` と `fn(x | ...)` の腕形式は branch として数える。

## 31. 引数のパターン分岐

引数が 1 つで、本体がその引数の pattern 分岐だけの関数は、`if` と引数名を省いて腕を並べる。

```ocaml
def display
| Guest => "guest"
| Member(user) => user.name
```

```ocaml
def rec length
| [] => 0
| [_, ...tail] => 1 + length tail
```

arity は 1 である。腕の規則は第26節から第29節と同じで、網羅性（E402）、到達不能（E403）、重複（E405）、fallback の位置（E404）、`_` の禁止（E503）、最上位の変数パターンの禁止（E512）、Bool リテラルだけの腕の禁止（E520）をそのまま当てる。`fn` も同じ形を取り、`fn(| Guest => "guest" | Member(user) => user.name)` と括弧で閉じる（第12節）。

この形で書ける関数を、引数名と `if` を付けて書くことは冗長であり E515 とする。E515 は次の 4 条件を全て満たすときに当たる。

* 引数が 1 つである
* 引数と戻りに型注釈が無い
* 引数の自由な参照が scrutinee の 1 回だけである（腕の中で同じ名前を再束縛したものは数えない）
* 本体が、その引数を scrutinee にした pattern 分岐ちょうどである

## 32. 引数の条件列

本体が条件列（第25節）だけの関数は、`=> if` を省いて引数列の直後に腕を並べる。

```ocaml
def classify score
| score >= 90 => Excellent
| score >= 70 => Good
| score >= 50 => Pass
| Fail
```

引数の数と型注釈は制限しない。`def sign_of (a: Int, b: Int): Int | a < b => -1 | a > b => 1 | 0` も同じ形である。腕の規則は第25節と同じで、fallback の必須（E401）と重複条件（E406）をそのまま当てる。`if` をアンカーにする診断は `def` か `fn` を指す（§56）。`fn` も同じ形を取り、`fn(x | x > 1 => x | 0)` と括弧で閉じる（第12節）。腕が 1 本の条件列は `if` 形では E520 だが、この形（`def f x | c => a | b`）は `=> if c => a | b` を E517 が退けるので正準形である。

引数を持たない `def main | true => 1 | 0` は第31節の pattern 腕として読まれ、`main` が引数を持つので E206 になる。引数ゼロの条件列の形は存在しない。

引数名の直後の `|` は常に条件列であり、pattern 分岐にはならない。`def unwrap x | Some(y) => y | 0` の `Some(y)` は式として読まれ、`y` が E204 になる。引数を pattern で分岐する関数は第31節の形で書く。

本体が条件列ちょうどである関数を `=> if` を付けて書くことは冗長であり E517 とする。引数の数と型注釈の有無によらず当てる。`let` などの継続を挟んだ後に条件列が来る関数は対象外である。

# 第VI部 失敗と効果

## 33. Option と失敗

値の不在は、

```ocaml
T Option
```

で表現する。

```ocaml
type T Option =
  | None
  | Some(T)
```

理由のある失敗は効果で表現する。prelude は汎用の失敗効果を持つ。

```ocaml
type Exception = { message: String }
effect Error
abort raise(error: Exception)
```

境界は理由の有無である。値が無いだけで理由を持たないものは `Option`、理由を運ぶものは効果とする。`List` の探索は `Option` を返し、パースと I/O と索引の範囲外は効果を持つ。

理由に構造が要るなら、利用者が効果を宣言する（第37節）。

```ocaml
type ParseFailure = | Unexpected(Char) | Eof
effect Parse
abort parse_failure(failure: ParseFailure)
```

`null` は持たない。

unchecked exception も持たない。失敗は効果として型に現れ、処理しなければ呼び出し側へ伝わる。

## 34. `let p? =`

Optionの成功値を使いたい場合は、パターンの末尾に `?` を付けた `let` を使う。

```ocaml
def display_name user =>
  let name? = user.name ->
  name
```

`let p? = e` は、右辺 `e` のOptionを評価し、成功値をpattern `p` に束縛する。`?` はpattern全体の末尾に 1 つ置く（`let { name }? = ...`、`let Id(n)? = ...`）。空白は意味を持たないので `let x ? = e` も同じ形である。

```text
Some(value)
  => valueをbindして継続

None
  => Noneをそのまま伝播
```

`?` を付けた `let` の右辺は Option 型でなければならない。そうでなければコンパイルエラーとする（E303）。

効果による失敗には印を付けない。効果は関数の型が語るので、呼び出し位置には何も書かない（第20節）。

```ocaml
def create_user input =>
  let parsed = parse input ->
  do validate parsed ->
  save parsed
```

`?` の有無が Option と効果を見分ける。`?` があれば値の不在の伝播、無ければ効果の伝播である。

`let` のパターンは、`?` の有無によらず、束縛する値の型の全ての値に一致しなければならない。一致しない値があれば E402 とする。名前、record パターン、constructor が 1 つの型の constructor パターン（`let Id(n) = x`）とその入れ子は一致する。

## 35. `do`

値を使わない式を評価して継続したい場合は `do` を使う。

```ocaml
def publish article =>
  do validate article ->
  do authorize article ->
  do persist article ->
  ()
```

意味は、

```text
do expression -> continuation
```

で、`expression` を評価し、その値を捨てて `continuation` へ進む。

`do` の右辺は効果を持つ式か Option でなければならない。純粋な値の破棄は意味を持たない（第40節）ので、どちらでもない式は E303 とする。

Optionなら、

```text
Some(_)
  => 継続

None
  => Noneを伝播
```

であり、これは、

```ocaml
let _? = ...
```

のcanonical formである。そのため `let _? = ...` は禁止（E504）。

効果を持つ式なら、値を捨てて継続する。失敗は効果として伝わるので、`do` は伝播の印ではない。

## 36. 伝播と分岐

failure propagationにpattern branchを使わない。

効果による失敗は何も書かずに伝わる。

```ocaml
def create_user input =>
  let parsed = parse input ->
  do validate parsed ->
  save parsed
```

Optionの不在は `?` と `do` で伝わる。

```ocaml
if find_user id
| Some(user) => ...
| None => None
```

のような単なる伝播は冗長で、canonical formは、

```ocaml
let user? = find_user id ->
...
```

成功値が不要なら、

```ocaml
do validate user ->
...
```

責務は明確に分ける。

```text
効果    = 何も書かない。型が語る
let p? = Optionのbind + 伝播
do     = 値の破棄 + 伝播
if     = meaningful domain branching
handler = 失敗を別の値へ変える
```

ただし、`if` によるOption分岐が常に禁止されるわけではない。`if` は、成功と失敗を異なるドメイン値へ変換するなど、意味のある分岐に使用できる。単に同じ `None` を再伝播するだけのbranchは冗長として拒否する（E508）。

効果を持つ式を書けない位置は 1 つある。分岐の条件（第24節、第25節）である。ここに置くと E308 とする。

```ocaml
if is_ready () => x | y        // E308
```

canonical formは、

```ocaml
let ready = is_ready () ->
if ready => x | y
```

それ以外の位置には書ける。分岐の腕、pattern branch の scrutinee、適用の引数、演算子の被演算子、record 式の field はいずれも許す。

```ocaml
def choose flag => if flag => fetch_a () | fetch_b ()
def unwrap => if fetch () | Some(x) => x | 0
def f x y => x / y + 1
def n => List.length (fetch_all ())
```

この制限は第42節と第44節の前提を守るためにある。Boolean branch の両方の腕が同じなら条件ごと落とす（E502）、同じ条件の重複を拒む（E406）という規則は、条件が副作用を持たないことに依存している。効果を持つ条件が書けると、これらの規則が効果の削除を要求してしまう。

pattern branch の scrutinee を禁じないのは、scrutinee を落としたり混同したりする規則が無いからである。E507（第45節）は `if x | Some(v) => Some(v) | None => None` を `x` に畳むが、scrutinee はそのまま残る。E403 と E405 は腕を落とすだけで scrutinee に触れない。

引数と被演算子を禁じないのは、そこでの問題が評価順だけだからである。順序は第7節が定めるので、禁じる必要がない。`/` と `%` が効果を持つ（第19節）以上、被演算子を禁じると `x / y + 1` が書けなくなる。

効果を別の値へ変えるには handler を使う（第38節）。

```ocaml
handler Error as ToOption
abort raise _ => None
return value => Some(value)
```

## 37. 効果

効果は `effect` と名前の見出しで始まり、次の見出しかファイル末尾まで続く。契約（第46節）と同じ節の形で、`end` は無く、見出しは入れ子にならない。型引数は名前の前に置く。

```ocaml
effect Error
abort raise(error: Exception)

effect A Yield
again yield(value: A): Unit

effect Ask
once ask: String
```

操作の種別は宣言で固定する。呼び出し位置から見て、どの保証が当たるかが静的に決まる。

| 種別 | `resume` | 第53節の保証 |
|---|---|---|
| `once` | 呼び出し位置で即座に返る | あり |
| `abort` | 再開しない | あり |
| `again` | 節の中のどこでも 1 回 | なし |

`abort` は再開しないので戻り値が存在しない。戻り型を書くと E123 とする。呼び出し位置の式の型は、その位置が要求する型に合わせる。返らないので、どの型とも矛盾しない。`once` と `again` は契約の member と同じ形で書き、引数を取らない操作は `once ask: String` の形とする。

効果の操作は値の名前空間に無修飾で入る（第49節）。値の空間はファイルごとだが、操作はこの規則の唯一の例外で、効果を宣言したディレクトリの全てのファイルから書ける。書けるのはそのディレクトリの中だけである。値の修飾は `実装名.値` の形しか無く（第59節）、小文字の操作名を別のディレクトリから綴る手段が無いためである。同じディレクトリの別の効果が同じ操作名を宣言すると E205 とする。prelude の操作はどこからでも書ける。prelude の操作と同名のユーザーの宣言（外の `def`、実装の `def`、他の効果の操作）は、どのディレクトリでも E205 とする。prelude の効果の handler はどのディレクトリにも置ける。

prelude の操作は、handler が無ければ `once` なら runtime の既定実装で応え、`abort` なら `main` の入口で stderr と終了コード 1 になる（第54節）。`again` は handler が要る。

操作の結果の効果行には、その操作が起こす追加の効果だけを書く（第55節）。

別のディレクトリの効果は、行に現れても操作を起こせない。その効果を持つ関数を呼ぶ形になる。

効果を宣言して一度も使わなければ E513 とする。

## 38. handler と `with`

handler は `handler X as Y` の見出しで始まる。`X` は処理する効果、`Y` は handler の名前で、どちらも省けない。実装（第47節）と同じ形である。

```ocaml
handler Error as ToOption
abort raise _ => None
return value => Some(value)
```

handler は処理する効果と同じディレクトリに置く。腕が操作名を綴るからである（第37節）。別のディレクトリに置くと E204 とする。prelude の効果は除く。その handler はどのディレクトリにも置ける（第37節）。

腕は効果の宣言と同じ種別で書き、その効果の全ての操作を過不足なく実装しなければならない。足りなければ、または効果に無い操作を書けば E605 とする。使わない引数は `_` で受ける（第39節）。

`return` の腕は、handler が包む計算が正常に終わったときの値を変換する。書かなければ恒等とする。

`again` の腕では `resume` を書ける。`resume` は捕まえた継続を 1 回だけ再開する。腕の中の任意の位置に置けるが、値として保存したり返したりはできない。`again` 以外の腕と、handler の外に `resume` を書くと E124 とする。`resume` が書ける場所は構文的な入れ子だけで決まる。

```ocaml
handler A Yield as Collect
again yield value =>
  let rest = resume () ->
  [value, ...rest]
return _ => []
```

効果が型引数を持つなら、handler も名前の前で型変数を宣言する（`handler A Yield as Collect`。位置は第16節と同じ）。

handler は値の引数を取れる。状態を持つ handler はこれで初期値を受ける。引数の形は `def` の型付き引数（第8節）と同じで、契約を取る functor の引数（第48節）とは別である。

```ocaml
handler A Yield as Take(limit: Int)
```

handler を使うには `with` を書く。引数があれば呼び出しの形で渡す。

```ocaml
def main =
  with Collect ->
  each [1, 2, 3]
```

```ocaml
def main =
  with Take(2) ->
  each [1, 2, 3]
```

handler の腕は分岐ではないので第30節の E101 に当たらず、腕の中に `if` を 1 つ書ける。腕の中の `if` の腕にさらに `if` を置けば、その `if` が第30節に当たる。

`with H -> rest` は `rest` を `H` の下で評価する。`let` や `do` と同じ継続の形で、分岐ではないので第30節には触れない。`with` を入れ子にすれば handler を重ねられる。

handler を宣言して一度も `with` に書かなければ E513 とする。

# 第VII部 正準性の検査

## 39. 未使用束縛

未使用bindingはwarningではなくerror。

禁止：

```ocaml
let x = calculate ->
next
```

`x` が継続内で使われていないならコンパイルエラー（E505）。

E505 は `let` の束縛（`?` の有無によらず）、pattern で束縛した変数、関数の引数の全てに適用する。pattern の一部を使わないときは `_` を書く（第29節）。

引数を使わない関数は、引数の数が外から固定される場所でだけ書ける。その引数は `_` で受ける。場所は 3 つで、契約の member を満たす実装の `def`（functor の本文を含む。`def save _ => ()`、`def save(_: T): Unit {Error} => ()`）と、匿名関数 `fn`（`List.map xs fn(_ => 0)`）と、handler の腕（第38節。`abort raise _ => None`）である。それ以外の `def`（外の `def`、契約に無い `def`）に `_` 引数を書くと E519 とする。引数は自分で削れるからである。名前を付けた引数を使わないのは、場所によらず E505 である。

未使用の判定は、外の `def` と `type`、契約、実装、functor、効果（第37節）、handler（第38節）、`implement` 節の中の ADT と record にも適用する（E513）。外の `def` はそのファイルの中だけで見える（第50節）ので、ファイルの中で数える。契約の抽象型の実体化（`shared type T = User` やconstructor付き本体）は契約が使うので対象外である。`implement` 節の中の `def` は E514（第47節）。functor の引数は E521（第48節）。型変数は E518（第16節）。`main` は例外である。

根に数えるものは検査の種類で分かれる。入口ファイルを渡した検査では `main` を根とし、渡さなかった入口ファイルの `main` も根に数える（第50節）。ライブラリ（第52節）では、通常の実装、functor の適用結果、未適用の functor、実装の無い契約、外の型、効果、handler を公開面として根にする。これらは参照されなくても E513 にならない。ライブラリの外の `def` はファイル内で数える（E513）。prelude の項目は公開面として根にする（第55節）。

## 40. 値の破棄

純粋値の暗黙的な破棄は禁止（E506）。

禁止：

```ocaml
let _ = calculate ->
next
```

効果を持つ式と Option の破棄には意味があるので、

```ocaml
do validate input ->
next
```

という専用のcanonical formを使う（第35節）。

## 41. 同一束縛

禁止（E501）：

```ocaml
let x = value ->
x
```

canonical form：

```ocaml
value
```

## 42. 同一 branch

禁止（E502）：

```ocaml
if condition => value | value
```

canonical form：

```ocaml
value
```

Boolean branchの両方の結果が同一なら、条件自体が意味を持たないため、branch全体を削除する。

## 43. 到達不能 branch

到達不能branchはerror（E403）。

禁止：

```ocaml
if value
| Some(x) => ...
| None => ...
| default
```

`Some` と `None` で既に網羅されている。

同様に、条件列でfallbackより後ろにbranchを置くことはできない。fallbackは必ず最後に置く（E404）。

## 44. 重複 branch

同一patternの重複は禁止（E405）。

```ocaml
if value
| Some(x) => ...
| Some(y) => ...
| None => ...
```

はerror。

条件列でも、同一条件または静的に到達不能な条件の重複はerrorとする（E406）。

「静的に到達不能」は次のいずれかに限る。先行条件と、束縛変数の名前替えと括弧を正規化した後の構造が一致する場合。先行条件が `true` の場合。冗長と到達不能の判定は構文的である。

## 45. 不要な再包装

禁止（E507）：

```ocaml
if slot
| Some(x) => Some(x)
| None => None
```

canonical form：

```ocaml
result
```

同じ意味を持つ分解・再構築は許さない。

# 第VIII部 モジュールと構成

## 46. 契約

OCamlのmodule systemの思想を取り入れる。`module` は契約であり、実装（第47節）とは別に宣言する。

契約は `module` と名前の見出しで始まり、次の見出し（`module` `implement` `effect` `handler`）かファイル末尾まで続く。`end` は無く、見出しは入れ子にならない。

```ocaml
module UserStore
type T
def find(id: Int): T Option
def save(user: T): Unit {Error}
```

ここでの `Error` は prelude の失敗効果である（第33節）。契約の中の型名は、実装があるかどうかによらず解決される。

契約の型宣言は本体を持たない `type T` に限る。本体を持つ型宣言は E100 とする。外に見せたい型は最初の見出しより前の「外」（第50節）で定義し、契約にはその名前を書く。抽象型は型引数を持てる。宣言は型定義と同じく名前の前に置く。

```ocaml
module Stack
type A T
def A empty: A T
def A push(stack: A T, value: A): A T
def A top(stack: A T): A Option
```

引数を取らない member は `def name: 型` で宣言し、実装は `def name = expression` で満たす（`Stack.empty`）。

abstract type：

```ocaml
module UserId
type T
def make(value: Int): T Option
def value(id: T): Int
```

契約の関数は、名前の前に型変数を宣言できる。

```ocaml
module Option
def (A, B) map(value: A Option, f: Fn(A => B)): B Option
```

宣言した型変数はその関数の型の中だけで有効で、全称量化される。実装は全ての型引数で使えなければならない。型変数の重複宣言は E205、未宣言の型名は E204、使われない型変数は E518 とする。

契約の中の修飾型 `X.T` は、契約 `X` の抽象型 `T` を指す。契約は実装の名前を書かない。`X.T` が型として定まるのは、プログラムの中で契約 `X` を満たす実装がちょうど 1 つのときで、その実装の `T` である。実装が 0 か 2 つ以上なら E603 とし、その `X.T` を指す。functor の定義は実装に数えず、functor の適用結果は数える（第48節）。`X.T` は、その実装の実体化が `shared` で解けるなら解けた型として扱う。印字と診断は `X.T` ではなく解けた型を出す。

```ocaml
module UserStore
type T
def find(id: UserId.T): T Option        // 契約 UserId の T
```

`val` は使わない。契約側も実装側も、

```text
def = named definition
```

で統一する。

契約への参照は `implement` の見出しと、契約の中の `X.T` である。実装が 1 つも無い契約は E513 とする（ライブラリ検査では公開面として根にする。第39節）。

## 47. 実装

実装は `implement X as Y` の見出しで始まる。`X` は満たす契約、`Y` は実装の名前で、どちらも省けない。契約と実装は別の名前空間（第49節）なので、1 対 1 の module は `implement UserStore as UserStore` と書く。契約が別のディレクトリにあれば、名前は第51節のパスで書く。

```ocaml
implement UserStore as UserStore
type T = User
def find id =>
  ...
def save user =>
  ...
```

実装は契約の全ての member と型を持ち、それぞれの型が一致し、契約の型変数に対して十分に多相でなければならない。満たさなければ E602 とし、`implement` を指す。実装は契約に載らない定義を持てる。`def` に加えて本体付きの型（ADT と record）も書け、それはその実装の中だけで見える。外から `MemoryStore.Entry` のように参照すると E204 とする。契約に載らない `def` も外からは見えない（E204）。

契約の member を満たす `def` は、使わない引数を `_` で受けられる（第39節）。

型の参照 `Y.T` は、実装 `Y` の契約が宣言する抽象型 `T` を指し、外からは不透明である。`type T = User` の実体化は新しい名前を作らず、`Y.T` の中身を外に見せない。`shared type T = User` の実体化は等式を外へ公開し、`Y.T` は `User` と同じ型になる。`shared` の右辺が別の抽象型（`shared type T = M.T`）なら、等式は抽象型どうしの同一性で、適用結果は引数の実体化に従う（第48節）。`shared` で解けた型は field 参照が許され、`def name (t: UserStore.T) => t.name` の E601 の対象外になる。`shared` は契約が宣言する抽象型の実体化にだけ前置でき、右辺が constructor 付き本体の形なら E100、契約に無い型に付ければ E204 とする。型引数付きの抽象型は、実体化の左辺でも型変数を名乗る。

```ocaml
implement Stack as Stack
type A T = A List
def empty = []
def push stack value => [value, ...stack]
def top | [] => None | [value, ..._] => Some(value)
```

契約が `type A T` なら実体化も型引数 1 つで、数が違えば E602 とする。右辺に現れない型変数（`type A T = Int`）は E518 とする。契約が宣言していない抽象型を実体化する `type U = Int` は E204 とし、`U` を指す。

実装の中の ADT と record は実装ごとの局所名前空間にあり、2 つの実装が同名の型を持ってよい。その constructor も実装ごとの局所名前空間にあり、2 つの実装が `Idle` を持ってよい。外の型と同名なら E205 とし、実装の中の宣言を指す。契約の抽象型と同名のconstructor付き型定義は、その抽象型の実体化とする（`type T = Payload { id: Int }`、`type T = | Payload(Int)`）。型引数の個数と公開memberの型は別名による実体化と同じ規則で照合する。constructorと本体は実装の外へ公開しない。外からconstructorを参照すると E204（constructor名）とする。constructorを持たないrecord型定義が契約の抽象型と同名なら E205 とする。

抽象型の実体化で別名を展開した結果、同じ別名へ戻って実体が確定しない場合（`type T = T`、`type T = U` と `type U = T` など）は E602 とし、その `implement` を指す。名前を持つ ADT や record の再帰型は、この別名の循環とは区別する。

```ocaml
def name (t: UserStore.T) => t.name
```

は E601 とする。

実装の中で契約からも内部からも参照されない `def` はエラーとする（E514）。

同じ契約を複数の実装が満たせる。

```ocaml
implement Store as MemoryStore
...

implement Store as FileStore
...
```

参照は実装の名前で行う（`MemoryStore.find`）。`implement X as Y` の `X` に当たる契約が無ければ E204、実装 `Y` がどこからも参照されなければ E513 とする。実装を引数に取る実装（functor）は第48節が定める。

0.1.1 までの `module type X = sig ... end` と `module X : SIG = struct ... end` は文法に無く、E100 となる。`sig` `struct` `end` はキーワードではない。

first-class module は入れない。

## 48. Functor

実装を引数に取る実装を functor と呼ぶ。定義は引数付きの `implement` 見出しで始まり、本文は実装と同じである。引数は「実装名: 契約名」で、本文の中では引数を実装として参照する。引数は 2 つ以上書ける。

```ocaml
implement Store as MakeStore(Id: UserId)
type T = Payload { id: Id.T; name: String }
def create name number => Payload { id = Id.make number; name = name }
def describe user => String.concat user.name (Int.to_string (Id.value user.id))
```

本文は引数の契約だけを見て 1 回検査する。`Id.T` は本文の中でも不透明で、`Id.value` を通してしか中身に触れない。契約 `Store` を満たさなければ E602 で、`implement` を指す。本文で参照されない引数は E521 とし、その引数名を指す。functor は `.hdi` に置けない（E100）。

適用は本文の無い `implement` 見出しで、`=` の右に functor と引数を書く。

```ocaml
implement Store as PlainStore = MakeStore(IntId)
implement Store as OffsetStore = MakeStore(OffsetId)
```

適用結果は普通の実装で、`PlainStore.create`、`PlainStore.T` と参照する。`PlainStore.T` と `OffsetStore.T` は別の型である。見出しに書いた契約と functor の契約が違えば E602、引数がその位置の契約を満たさなければ E602 で、どちらも `implement` を指す。引数には実装と適用結果を書け、functor そのものは書けない（E204）。適用の見出しの後に本文を置くと E100 とする。

同じ functor に同じ引数列（位置ごとに同じ実装名）を 2 回適用すると、2 回目の `implement` で E604 とする。同じ型が要るときは 1 つの適用結果を使う。

functor は適用されて初めて実装になる。`MakeStore.create` のように functor を直接参照すると E204、一度も適用されない functor は E513 とする（ライブラリ検査では公開面として根にする。第39節）。適用結果は普通の実装と同じで、どこからも参照されなければ E513 である。契約 `X` の実装の数（第46節の `X.T`、実装の無い契約の E513）は適用結果で数え、functor の定義は数えない。

functor の本文の `def` は契約の member を満たすので、使わない引数を `_` で受けられる（第39節）。

本文の実体化が引数の抽象型を指すとき、`shared type T = M.T` と書くと等式が適用結果に引き継がれる。`MakeFold(StringMonoid)` の `T` は `StringMonoid.T` と同じ型になり、`StringMonoid.T` が `shared type T = String` なら `String` と同一になる。`shared` を書かない `type T = M.T` は適用ごとに別の抽象型を作る。`PlainStore.T` と `OffsetStore.T` が別の型である、はこの場合である。

## 49. 名前空間

名前空間は型・契約・実装・constructor・値の 5 つとする。効果はこの 5 つとは別の空間、handler は実装の空間、効果の操作は値の空間に入る（第37節、第38節）。型・契約・実装・constructor の 4 つの名前空間と効果はディレクトリごとに持つ。値の空間はファイルごとに持つ（第50節、第51節）。効果の操作は値の空間に入るが、ファイルごとではなくディレクトリごとに見え、書けるのは効果を宣言したディレクトリの中だけである（第37節）。値の空間でこの例外を持つのは操作だけである。どの名前空間でも解決できない名前の参照は E204 とする。`T Option`（型）、`module Option`（契約）、`implement Option as Option`（実装）、`Option.map`（実装の値）は共存する。`implement` の直後の名前と functor の引数の契約は契約として解く。`.` の左の名前は、契約の中では契約（第46節の `X.T`）、それ以外では実装として解く。

型と constructor は別の空間である。同じ名前空間内の同名宣言は E205 とする。`implement` 節の中の ADT と record とその constructor は、その実装だけの局所的な空間にあり、実装同士の同名は E205 にならない。外の型と同名なら E205 とし、実装の中の宣言を指す。契約の抽象型と同名のconstructor付き型定義は実体化であり、constructorを持たないrecord型定義は E205 である（第47節）。修飾した参照 `A.B` と `a::B` が解決できないときの E204 は、解決できない最初の名前を指す（`A` が契約や実装として解決できなければ `A`、`A` は解決して `B` が無ければ `B`）。束縛の shadowing（`Member(user) => user.name` の `user`）は許す。

prelude の constructor `Some` `None` は修飾なしで参照できる。prelude の効果 `Error` は効果の空間にあり、その操作 `raise` は値の空間にあってどこからでも書ける（第37節）。`Nil` `Cons` は名前空間にあるが、利用者のコードに書くと E510 か E516 になる（第22節）。`compare` は値の空間の組み込みで、修飾なしで参照できる（第21節）。prelude の効果 `Env` `Io` の操作も値の空間にあり、どこからでも書ける（第55節、第37節）。組み込み型の名前（`Int` `Float` `Bool` `Char` `String` `Bytes` `Array`）と prelude の `Option` `List` `Exception` は予約で、利用者が同じ名前を宣言すると E205 とする。prelude の効果名 `Error` `Env` `Io` も予約で、利用者の同名宣言は E205 とし、利用者の宣言を指す。

`__` で始まる名前は呼び出し位置でしか参照できず、値化はファイルの場所によらず E203 とする。呼び出し位置では `stdlib/` のファイルでのみ解決し、ユーザーコードでは E202 とする。

既定実装の契約名 `EnvDefault` と `IoDefault` は契約の名前空間で予約し、利用者の同名の契約宣言は E205 とする。

## 50. 複数ファイル

プログラムは、`hado build` か `hado check` に渡したファイルのディレクトリ（根）と、その下の全てのサブディレクトリにある `.hd` `.hdi`、および prelude からなる。設定ファイルと検索パスは無い。読んだファイルは参照の有無によらず全て検査を受ける。サブディレクトリは名前空間になる（第51節）。

各ファイルは第59節の `program` の形で、プログラムはその和である。ファイルの中身は、最初の見出しより前の「外」と、見出しで始まる節の列である。外に置けるのは本体付きの型で、そのディレクトリの名前空間に入る。根のファイルの外にはさらに `def` を置ける。外の `def` はそのファイルの中だけで見え、同じファイルの節から参照できる。根以外のファイルの外に `def` があれば E100 とし、その `def` を指す。ライブラリ根（第52節）のディレクトリのファイルは、根のファイルと同じく外の `def` を置ける。

`main` を持つ根のファイルが入口ファイルである。入口ファイルは 1 つのディレクトリに複数あってよく、渡したファイルの `main` が起点になる。渡したファイルに `main` が無いとき、`hado check` はそのディレクトリをライブラリとして検査し（第52節）、`hado build` は E206 とする。外の `def` はファイルごとの値の空間にあるので、複数のファイルが `main` を持っても E205 にならない。渡さなかった入口ファイルの `main` は、入口検査では、そのファイルの中で参照されなくても E513 にならない（ライブラリ検査では第52節）。`main` を持たない根のファイルも外の `def` を置け、その `def` はファイルの中で使われなければ E513 である。入口に渡せるのは `.hd` だけで、`.hdi` を渡すと CLI が拡張子で拒否する（診断ではない）。

`.hd` には `module` と `implement` の両方の節を置ける。`.hdi` には外の型と `module` の節だけを置け、`implement`（functor と適用を含む）があれば E100 とし、その `implement` を指す。`.hdi` は契約だけを置くファイルである。

ファイル名は自由で、名前はディレクトリ名とソースの見出しだけで決まる。1 ファイルに 1 つの契約か実装を置き、ファイル名をその名前の snake_case にするのを慣習とする。

全ての節はプログラム全体の top-level である。同じ名前空間の同名の宣言はファイルをまたいでも E205 とし、2 回目の宣言を指す。ファイルをまたぐ順序は、入口ファイルを先、残りを根からの相対パスのバイト順とする。第56節の「アンカー位置のソース順」は、ファイルの順序、行、列の辞書式で比べる。実装の無い契約と参照されない実装は E513 とする（第46節、第47節）。ライブラリ検査では第39節の公開面の規則を当てる。

`hado check` に入口以外のファイルを渡すと、そのファイルが起点になり、`main` があれば入口ファイル、無ければライブラリ根（第52節）として扱う。ファイルの順序もそれを先頭に組み直す。サブディレクトリのファイルを渡すとそのディレクトリが根になる。

## 51. ディレクトリと名前空間

根のサブディレクトリ 1 つが名前空間 1 つである。名前はディレクトリ名そのままで、`name` の字句（小文字で始まる）でなければ CLI が拒否する（診断ではない）。入れ子のディレクトリは入れ子の名前空間になる。根は名前を持たない。ライブラリ根（第52節）は独立した根である。ライブラリの中では、参照の綴りはライブラリ根から解く。外からライブラリを参照するときは、利用側の根からの絶対パスでライブラリのディレクトリを辿る。ライブラリの中から利用側の根と兄弟のライブラリは見えない。

```text
app/
  main.hd               根。外の型 Error などは無修飾でどこからも見える
  store/
    store.hdi           契約 Store       → store::Store
    memory_store.hd     実装 MemoryStore → store::MemoryStore
    backend/
      disk.hd           実装 Disk        → store::backend::Disk
  id/
    user_id.hdi         契約 UserId      → id::UserId
    int_id.hd           実装 IntId       → id::IntId
```

参照の綴りは 2 つに限る。自分のディレクトリの名前は無修飾、それ以外は最内の根（ライブラリの中ではライブラリ根、外では利用側の根）からの絶対パスである。親や兄弟のディレクトリの名前を無修飾で参照すると E204 になる。根の名前はどこからも無修飾で参照する。

```ocaml
// store/memory_store.hd
implement Store as MemoryStore              // 同じディレクトリの契約
def find id => ...id::IntId.value id...     // 別のディレクトリの実装

// store/backend/disk.hd
implement store::Store as Disk              // 親のディレクトリでも絶対パス

// main.hd
def main = ... store::MemoryStore.find 1 ... store::NotFound ...
```

`::` は名前空間の区切りにしか使わない。パスの後には契約か実装か型か constructor の大文字名が来る。値と抽象型は実装の後に `.` で続ける（`store::MemoryStore.find`、`Int store::Stack.T`）。パターンの constructor も同じ綴りで書く（`| store::NotFound => ...`）。

型・契約・実装・constructor の 4 つの名前空間はディレクトリごとに持つ。E205 の「同じ名前空間の同名」はディレクトリの中で数える。`color/` と `light/` がそれぞれ `Red` を持ってもよい。根の名前はどこからも無修飾で見えるので、サブディレクトリの中の宣言が根の同じ名前空間の宣言（外の型、契約、実装、constructor、prelude の名前）と同名なら、そのディレクトリの中で無修飾の綴りが 2 つの候補を持つ。これは E205 とし、サブディレクトリ側の宣言を指す。ライブラリ根の中では利用側の根の名前は見えないので、この規則は当たらない。`store/` の中に `type Option` や `type Error`（根に `Error` があるとき）は書けない。

`.hdi` の規則は第50節と同じで、どのディレクトリにも置ける。prelude は根に連結され、`List.map` や `Some` はどこからも無修飾で参照する（第55節）。

## 52. ライブラリ

`main` を持たず、他のプログラムから使われることを意図したプログラムをライブラリと呼ぶ。`hado check` に `main` の無い `.hd` を渡すとライブラリとして検査し（第10節、第50節）、`hado build` は受け付けない（E206）。ライブラリの未使用の判定は公開面を根にする（第39節）。

ライブラリであることは `library` 宣言で示す。ディレクトリの 1 つのファイルの外（最初の見出しより前）に `library` を書くと、そのディレクトリがライブラリ根になる。ライブラリ根は独立した根で、パス（第51節）はそこから解く。これにより、ライブラリは利用側の根の下のどの位置に置かれても、内部の参照と外の `def` の規則が変わらない。

```ocaml
library

type Error = | NotFound

module UserStore
type T
def find(id: Int): T Option
```

`library` は 1 ファイルに 1 つ、1 ディレクトリに 1 つである。2 つ目は E205 とする。`library` は `.hd` にだけ書ける。`.hdi` に書くと E100 とする。`library` を宣言したディレクトリのファイルに `main` を置くと E206 とする。ライブラリとして検査するとき、他のファイルの `main` は入口にならない。宣言の無いディレクトリでライブラリとして検査したときの `main` は普通の `def` として E513 の対象になる。外の `def` はライブラリ根のファイルにも置け、ファイルの中で使われなければ E513 である（第50節）。

利用側は、ライブラリのディレクトリを使うプログラムの根の下に置く。ライブラリの中から利用側の根と兄弟のライブラリの名前は見えず、外からは利用側の根からの絶対パスで参照する（第51節）。依存するライブラリは、自分の下に入れ子にするか、利用側で束ねる。

宣言の無いディレクトリは、埋め込みでは利用側の根の名前空間の一部として扱われる。単体で `hado check` に渡せばライブラリとして検査されるが、埋め込みで内部の絶対パスを使うには `library` の宣言が要る。利用側の入口検査では、ライブラリの参照されない実装は E513 の対象になる。使う実装は利用側から参照する。

# 第IX部 実行と入出力

## 53. 末尾呼び出しの保証

末尾位置は、関数の本体そのものと、そこから次の規則で辿れる位置とする。`let` と `do` と `with` の継続、分岐（第24節から第27節）の腕と fallback である。`fn` の本体はその `fn` にとっての末尾位置であり、囲む関数のものではない。適用の引数、演算子の被演算子、record 式の field、分岐の条件と scrutinee は末尾位置ではない。

`again` 操作（第37節）を経由しない末尾位置の関数呼び出しは、スタックを消費しない。自己再帰・相互再帰・関数値経由を含む。`def rec` が唯一のループであるため、仕様で保証する。

`again` 操作は継続を捕まえるので、その後ろの末尾呼び出しはこの保証の外にある。`again` を含むループはスタックを反復回数に比例して消費する。`once` と `abort` は継続を捕まえないので保証の内側にある。

全ての hado 関数は環境を第 1 引数に取る。名前付き関数は環境を無視する。単相の `Fn` 型ごとに `tail` 規約の signature が一つに定まる。関数値経由の末尾呼び出しは `return_call_indirect` で表せる。

## 54. 印字

`main` の値は次の規則で印字する。

`main` が値を返したら stdout に印字して終了コード 0 とする。`main` まで処理されずに届いた `raise` は、その `Exception` の `message` を stderr に印字して終了コード 1 とする。Option は剥がさない。

印字が失敗したら終了コード 1 とする。失敗とは stdout への書き込みか flush が失敗することで、fd が閉じている・書き込み先が満杯である・パイプの受け手が先に終わっている場合を含む。パイプの受け手が先に終わった場合もシグナルで終了させず、他の書き込み失敗と同じに扱う。

失敗の理由は stderr に印字するが、これは best-effort とし、stderr 自体の失敗は無視する。それ以上報告する先が無いためである。`raise` の `message` の印字が失敗した場合も、終了コードは 1 のままとする。

終了コードは 0 と 1 の二値を保つ。1 は `raise` が `main` まで届いた場合と印字が失敗した場合の両方に当たり、区別は stderr の内容がつける。

中身は canonical なソース表記で印字する。ソースとして再び読める形が一つに定まることを目的とする。

| 型 | 印字 |
|---|---|
| Int | 10 進 |
| Float | 往復可能な最短の 10 進桁を指数を使わず位取りで並べ、小数点の両側に最低 1 桁を置く。`3.0` は `3.0`、`0.1` は `0.1`、10 の 300 乗は `1` の後に `0` が 300 個と `.0`。`-0.0` は `-0.0`。NaN と無限大はソースで書けないので `nan` `inf` `-inf` と出す |
| Bool | `true` `false` |
| Char | 入れ子では `'a'` の形。エスケープは第5節の 4 つ |
| String | 入れ子では `"..."` の形。エスケープは第5節の 4 つ |
| Unit | 入れ子では `()` |
| ADT | `Some(1)` `Guest` |
| List | `[1, 2]` `[]`。`Cons` の形では出さない |
| Bytes | `Bytes.of_list [72, 105]`。10 進の Int のリスト |
| Array | `Array.of_list [1, 2]` |
| record | `{ name = "a"; age = 3 }` |
| 名前付きfieldのconstructor | `Payload { id = 3; name = "a" }`。fieldは宣言順 |

最上位（`main` が返した値そのものと、stderr へ出す `message`）だけ 2 つの例外を置く。String と Char は引用符もエスケープも付けずに生の文字列を出す。Unit は何も出さない（改行も出さない）。

# 第X部 prelude

## 55. prelude

言語が名前で解決する名前は、型の `Option` `List` `Exception` と効果の `Error` `Env` `Io` とする。`/` の戻り型、`let p? =` の対象、List リテラルの展開先、`main` に残せる効果に用いる。

prelude の各 module は、外の型と `module X` と `implement X as X` を 1 ファイルに置く形で書く。第46節と第47節の規則に従う。

prelude は組み込みを次の形で包む。

```ocaml
module String
def length(s: String): Int Pure

implement String as String
def length s => __string_length s
```

M5 の extern 表の各項目は、hado 側の名前（`__string_length` 等）と型を持つ。

関数集合は次とする。

```text
List: map filter fold length reverse append sort head nth take drop
Option: map unwrap_or is_some ok_or
String: length concat slice get split starts_with contains join trim
Bytes: length get of_list to_list to_string of_string
Array: length get set of_list to_list
Int: to_string parse abs min max
Float: to_string
```

`Option.ok_or` は値の不在を理由のある失敗へ持ち上げる。

```ocaml
def A ok_or(value: A Option, error: Exception): A {Error}
```

効果を持たない member は戻り型に `Pure` を書く（第20節）。`List.length` `reverse` `append` `head` `nth` `take` `drop`、`Option.unwrap_or` `is_some`、`String.length` `concat` `starts_with` `contains` `join` `trim`、`Bytes.length` `to_list` `of_string`、`Array.length` `of_list` `to_list`、`Int.to_string` `abs` `min` `max`、`Float.to_string` が対象である。`Fn` を引数に取る `List.map` `filter` `fold` `sort` と `Option.map` は効果多相なので書かない。失敗する member（`String.slice` `get` `split`、`Bytes.get` `of_list` `to_string`、`Array.get` `set`、`Int.parse`）は `{Error}` を持ち、`Pure` を書かない。

`sort` は比較関数を引数に取る merge sort とする。

`String.length` は Unicode スカラ値の個数とする。

member の型は次のとおりとする。`A` は要素型の型変数である。

```ocaml
module String
def length(s: String): Int Pure
def concat(left: String, right: String): String Pure
def slice(s: String, start: Int, end: Int): String {Error}
def get(s: String, index: Int): Char {Error}
def split(s: String, separator: String): String List {Error}
def starts_with(s: String, prefix: String): Bool Pure
def contains(s: String, part: String): Bool Pure
def join(values: String List, separator: String): String Pure
def trim(s: String): String Pure

module Bytes
def length(bytes: Bytes): Int Pure
def get(bytes: Bytes, index: Int): Int {Error}
def of_list(values: Int List): Bytes {Error}
def to_list(bytes: Bytes): Int List Pure
def to_string(bytes: Bytes): String {Error}
def of_string(value: String): Bytes Pure

module Array
def A length(xs: A Array): Int Pure
def A get(xs: A Array, index: Int): A {Error}
def A set(xs: A Array, index: Int, value: A): A Array {Error}
def A of_list(xs: A List): A Array Pure
def A to_list(xs: A Array): A List Pure

module Int
def to_string(value: Int): String Pure
def parse(value: String): Int {Error}
def abs(value: Int): Int Pure
def min(left: Int, right: Int): Int Pure
def max(left: Int, right: Int): Int Pure
```

`String` の索引はスカラ値の単位である。`slice` は `start`（含む）から `end`（含まない）までを返し、範囲外は `{Error}` とする。`String.length` は Unicode スカラ値の個数、`Bytes.length` はバイト数である。

prelude は効果 `Env` と `Io` を持ち、操作を値の空間に置く。操作は `once` で、handler が無ければ runtime の既定実装が応える。既定実装は失敗を `Error` として raise する。利用者は `raise` と同じく無修飾で呼ぶ。

```ocaml
effect Env
once args: String List {Error}
once env(name: String): String Option {Error}
once cwd: String {Error}

effect Io
once read_file(path: String): Bytes {Error}
once write_file(path: String, contents: Bytes): Unit {Error}
once log(contents: String): Unit {Error}
once read_dir(path: String): String List {Error}
once read_stdin: Bytes {Error}
```

操作の結果の効果行には、その操作が起こす追加の効果だけを書く（`Io` の操作なら `Error`）。`args` はプログラム名を除く引数である。`env` は未設定を `None` とする。非 UTF-8 の名前は `Error` を raise する。

組み込みの失敗の `message` は次の綴りとする。

| 失敗 | `message` |
|---|---|
| Int の `/` `%` の除数 0 | `"DivisionByZero"` |
| String・Bytes・Array の索引の範囲外 | `"IndexOutOfRange"` |
| `String.split` の空セパレータ | `"EmptySeparator"` |
| `Bytes.of_list` の 0..255 の範囲外 | `"ByteOutOfRange"` |
| `Bytes.to_string` の不正 UTF-8 | `"InvalidUtf8"` |
| `Int.parse` の不正（桁溢れを含む） | `"InvalidInt"` |
| argv・env・cwd・ディレクトリ名の非 UTF-8 | `"InvalidUtf8"` |
| ファイルが無い | `"NotFound"` |
| 権限が無い | `"PermissionDenied"` |
| ディレクトリでない | `"NotADirectory"` |
| stdout への書き込みの失敗（`log`） | `"IoFailure"` |
| その他の I/O の失敗 | `"IoFailure"` |

prelude は全プログラムに連結され、全ての検査を受ける。prelude の項目は公開面として E513 の根にする（第39節）。

`Int.parse` の受理文法は、往復から導いた下限として `Int.to_string` が返す全ての Int の10進表記を含み、本節が決めた上限として ASCII の `-?[0-9]+` に一致して値が -9223372036854775808 以上 9223372036854775807 以下である文字列だけを受理し、先行ゼロの個数は制限せず、`+`・空白・`_`・小数点・全角数字は拒否する。

`read_dir` は指定したディレクトリの各項目の名前だけを返し、`.` と `..` を除いて UTF-8 のバイト順に整列する。

`read_stdin` は標準入力を EOF まで読み、同じプロセスでの2回目の呼び出しは空の `Bytes` を返す。

`env` の値が非 UTF-8 のときも、`message` が `"InvalidUtf8"` の `Error` を raise する。

`log` は `contents` の後に改行 1 つを stdout へ書く。書き込みか flush が失敗したら、`message` が `"IoFailure"` の `Error` を raise する。これは第54節の `main` の値の印字とは別の経路で、失敗は捕捉できる値として観測できる。パイプの受け手が先に終わっていても、シグナルで終了させず同じ `Error` とする。

`String.join` は区切りを要素の間にだけ置く。`[]` は `""`、要素が 1 つならその要素である。`String.trim` は ASCII の空白（SPACE、TAB、LF、VT、FF、CR）を両端から除く。全角空白は除かない。

`Int.abs` は 0 以上ならその値、0 未満なら符号を反転した値である。i64 の最小値では反転が表現できないので、その値自身を返す。`Int.min` と `Int.max` は 2 つの値の小さい方と大きい方を返す。

`List.head` は `[]` で `None`、先頭があれば `Some(先頭)` である。`List.nth` は `index` が負か長さ以上なら `None` である。`List.take` は `count` が 0 以下なら `[]`、長さ以上なら全件である。`List.drop` は `count` が 0 以下なら全件、長さ以上なら `[]` である。`List` のこの 4 つの型は次のとおりとする。

```ocaml
def A head(values: A List): A Option Pure
def A nth(values: A List, index: Int): A Option Pure
def A take(values: A List, count: Int): A List Pure
def A drop(values: A List, count: Int): A List Pure
```

# 第XI部 診断

## 56. エラーコード

拒否コーパスは次のコードで照合する。

検査は 字句 → 構文 → 名前解決 → 型 → 網羅性 → 冗長 → 単相化 の順に走る。1 つのプログラムに複数の欠陥があるとき、報告するのはこの順で最初の段階に当たる 1 件である。同一段階に 2 つ以上あるときは、アンカー位置のソース順で先のものを報告する。アンカーは規則が名指す最小の構文要素の先頭で、表の「アンカー」列に定める。「節」列は主となる節を 1 つ書き、拒否コーパスの `// spec:` はこれと一致させる。診断は `パス:行:列: error[コード]: 説明` の 1 行で、パスはアンカーを含むファイルである。コードの百の位は段階に対応する（E1xx 字句と構文、E2xx 名前解決、E3xx 型、E4xx 網羅性、E5xx 冗長、E6xx 契約と効果の型、E7xx 単相化）。効果の検査は 1 つの段階に集まらない。効果行の型付けは型、`resume` の位置は構文、行の重複は冗長、`main` に残る効果は単相化で当たる。

| コード | 短い名前 | 節 | 段階 | アンカー |
|---|---|---|---|---|
| E100 | syntax-error | 付録C | 構文 | 文法に合わない最初のトークン。付録Cの腕 0 本の列は先頭の `|` |
| E101 | nested-branch | §30 | 構文 | 内側の `if` か `fn` |
| E102 | rec-on-value | §9 | 構文 | `rec` |
| E103 | float-literal-form | §5 | 字句 | リテラル |
| E104 | comment-form | §5 | 字句 | コメントの先頭（`--` `#` の 1 文字目） |
| E105 | no-tuple | §20 | 構文 | `(` |
| E106 | constructor-call-form | §13 | 構文 | constructor 名 |
| E107 | operator-overloading | §4 | 構文 | `def` の直後の演算子 |
| E108 | adt-leading-pipe | §17 | 構文 | 最初の constructor 名 |
| E109 | zero-parameter-arrow | §8 | 構文 | `=>` |
| E110 | string-escape-form | §5 | 字句 | `\` |
| E111 | char-literal-form | §5 | 字句 | `'` |
| E112 | int-literal-range | §5 | 字句 | リテラル |
| E123 | abort-return-type | §37 | 構文 | `abort` の操作の `:` |
| E124 | resume-outside-again | §38 | 構文 | `resume` |
| E125 | pattern-column-count | §26 | 構文 | scrutinee と数の合わない pattern の列を持つ腕の `\|` |
| E201 | implicit-recursion | §9 | 名前解決 | 自分を参照する名前 |
| E202 | dunder-in-user | §49 | 名前解決 | `__` 名 |
| E203 | dunder-as-value | §49 | 名前解決 | `__` 名 |
| E204 | unbound-name | §49 | 名前解決 | 名前 |
| E205 | duplicate-name | §49 | 名前解決 | 2 回目の宣言の名前。ファイルをまたぐ順序は第50節。名前空間の単位と、根との同名でサブディレクトリ側を指す規則は第51節。`library` の重複は 2 回目の `library`（第52節） |
| E206 | entry-form | §10 | 名前解決 | `def main`。`build` に `main` が無ければ 1:1。`library` を宣言したディレクトリの `main` はその `def`（第52節） |
| E207 | rec-cycle-value | §9 | 名前解決 | 閉路に含まれる値定義の `def` |
| E208 | missing-rec | §9 | 名前解決 | `rec` の無い `def` |
| E301 | arity-mismatch | §13 | 型 | 適用の先頭 |
| E302 | recursive-type-arg | §16 | 型 | 宣言と異なる型引数 |
| E303 | rail-type | §34 | 型 | `let p? =` の右辺は Option、`do` の右辺は効果を持つ式か Option。どちらでもない式 |
| E304 | record-ambiguous | §18 | 型 | record 式か record パターンの `{`。型が未確定の field 参照では参照式の先頭 |
| E305 | type-mismatch | §15 | 型 | 型が食い違った式の先頭。適用の引数については下記 |
| E306 | occurs-check | §15 | 型 | 同上 |
| E307 | poly-recursion | §9 | 型 | 再帰呼び出しの先頭。間接呼び出しの位置を追跡できない場合は再帰関数の `def` の先頭（§9） |
| E308 | effect-position | §36 | 型 | 許されない位置に置かれた効果を持つ式の先頭 |
| E401 | missing-cond-fallback | §25 | 網羅性 | `if`。第32節の形では `def` か `fn`。腕の条件列（第26.1節）ではその腕の `if` |
| E402 | non-exhaustive | §28 | 網羅性 | `if`。`let` ではそのパターン |
| E403 | unreachable-branch | §43 | 網羅性 | 到達不能な腕の `|` |
| E404 | fallback-not-last | §43 | 網羅性 | fallback より後の腕の `|` |
| E405 | duplicate-pattern | §44 | 網羅性 | 重複する 2 番目の腕の `|` |
| E406 | duplicate-condition | §44 | 網羅性 | 同上 |
| E501 | identity-binding | §41 | 冗長 | `let` |
| E502 | identical-branches | §42 | 冗長 | `if`。第32節の形では `def` か `fn` |
| E503 | catch-all-underscore | §29 | 冗長 | `_` |
| E504 | let-question-underscore | §35 | 冗長 | `_` |
| E505 | unused-binding | §39 | 冗長 | 束縛名 |
| E506 | implicit-discard | §40 | 冗長 | `_` |
| E507 | rewrap | §45 | 冗長 | `if`。第31節と第32節の形では `def` か `fn`。Option の再包装に当てる |
| E508 | repropagate | §36 | 冗長 | 再伝播する腕の `|`。Option の再伝播に当てる |
| E510 | list-literal-form | §22 | 冗長 | 式の `Cons` か `Nil` |
| E511 | cond-to-literal-pattern | §26 | 冗長 | `if`。第32節の形では `def` か `fn`。腕の条件列（第26.1節）ではその腕の `if` |
| E512 | top-var-pattern | §26 | 冗長 | 変数パターン |
| E513 | unused-toplevel | §39 | 冗長 | 未使用の宣言の先頭。効果（第37節）と handler（第38節）にも当てる |
| E514 | unused-implement-def | §47 | 冗長 | `def` |
| E515 | if-on-sole-parameter | §31 | 冗長 | `def` か `fn` |
| E516 | list-pattern-form | §22 | 冗長 | パターンの `Cons` か `Nil` |
| E517 | if-chain-on-parameters | §32 | 冗長 | `def` か `fn` |
| E518 | unused-type-parameter | §16 | 冗長 | 使われない型変数の宣言 |
| E519 | underscore-parameter | §39 | 冗長 | `_` |
| E520 | boolean-branch-form | §24 | 冗長 | `if`。第31節の形では `def` か `fn` |
| E521 | unused-functor-parameter | §48 | 冗長 | 引数名 |
| E522 | duplicate-effect | §20 | 冗長 | 行の 2 番目に現れた同じ効果 |
| E601 | opaque-type | §47 | 型 | field access の `.` |
| E602 | contract-mismatch | §47 | 型 | `implement` |
| E603 | contract-type-unresolved | §46 | 型 | 契約修飾の型 `X.T` |
| E604 | functor-reapplied | §48 | 型 | 2 回目の `implement` |
| E605 | handler-mismatch | §38 | 型 | 過不足のある `handler` |
| E701 | main-polymorphic | §10 | 単相化 | `def main` |
| E702 | main-contains-fn | §10 | 単相化 | `def main` |
| E703 | eq-on-fn | §21 | 単相化 | 演算子 |
| E704 | compare-on-fn | §21 | 単相化 | `compare` |
| E705 | main-contains-abstract | §10 | 単相化 | `shared` で解けない抽象型を含む `def main` |
| E706 | main-user-effect | §10 | 単相化 | `def main` |

関数・constructor の適用で、仮引数型と実引数型の単一化が E305 または E306 になる場合は、問題の実引数の先頭を指す。複数の引数は渡される順に照合する。パイプの左辺は第1引数として扱い、その元の式の位置を使う。例えば `add1 "hi"` は `"hi"`、`x x` の出現検査失敗は第2の `x` を指す。呼び出す値が関数でない場合は、その値の式の先頭を指す。E301 と E307 のアンカーは表の規則に従う。

# 付録

## 付録A コア構文

中心となる構文はかなり小さい。

```text
def
def rec

let
let p? =
do
->
with
resume

fn(...)
=>
Fn(...)

type
T Option
ADT
record
{Io, Error}
Pure

if
|
_

|>

module
implement
effect
handler
once
abort
again
return
as
::
```

## 付録B 典型コード

失敗しうる処理：

```ocaml
def create_user input =>
  let parsed = parse input ->
  do validate parsed ->
  do authorize parsed ->
  save parsed
```

値を返さない処理：

```ocaml
def delete_user id =>
  let user = find_user id ->
  do authorize_delete user ->
  delete user
```

失敗を値へ変える処理：

```ocaml
handler Error as ToOption
abort raise _ => None
return value => Some(value)

def try_create input =>
  with ToOption ->
  create_user input
```

ADT分岐：

```ocaml
def display
| Guest => "guest"
| Member(user) => user.name
```

Option分岐：

```ocaml
def display_name user =>
  if user.name
  | Some(name) => name
  | "anonymous"
```

Boolean：

```ocaml
def size x =>
  if x > 10 => Large | Small
```

複数条件：

```ocaml
def classify score
| score >= 90 => Excellent
| score >= 70 => Good
| score >= 50 => Pass
| Fail
```

pattern fallback：

```ocaml
def name
| Member(user) => user.name
| "guest"
```

pipeline：

```ocaml
def active_names users =>
  users
  |> List.filter fn(user => user.active)
  |> List.map fn(user => user.name)
```

契約と実装：

```ocaml
module UserStore
type T
def find(id: Int): T Option
def save(user: T): Unit {Error}
def delete(user: T): Unit {Error}

implement UserStore as UserStore
type T = User
def find id => ...
def save user => ...
def delete user => ...
```

Functor：

```ocaml
implement Store as MakeStore(Id: UserId)
type T = Payload { id: Id.T; name: String }
def create name number => Payload { id = Id.make number; name = name }

implement Store as PlainStore = MakeStore(IntId)
```

## 付録C 文法の概形

字句と構文の形はこの節を正とし、第5節と第7節は値の意味だけを定める。

字句:

```text
int             := digit+                              10 進。i64 の範囲（E112）
float           := digit+ "." digit+                   小数点の両側に数字（E103）
char            := "'" (char_escape | 文字) "'"          ちょうど 1 文字（E111）
char_escape     := "\n" | "\t" | "\\" | "\'"
string          := '"' (string_escape | 文字)* '"'
string_escape   := "\n" | "\t" | "\\" | '\"'           これ以外は E110
bool            := "true" | "false"
unit            := "()"
name            := lower (letter | digit | "_")*       値・field・引数・名前空間の名前
upper_name      := upper (letter | digit | "_")*       型・constructor・契約・実装・型変数の名前
dunder_name     := "__" name                           stdlib 限定の組み込み名（第49節）
comment         := "//" 行末までの文字                   `--` と `#` は E104
```

`_` 単独は wildcard であり名前ではない。`::` は名前空間の区切り（第51節）、`.` は member と field の区切りで、どちらも 1 つのトークンである。空白とコメントはトークンの区切りで、それ以外の意味を持たない。

キーワードは `def` `rec` `let` `do` `fn` `if` `type` `module` `implement` `effect` `handler` `with` `resume` `return` `once` `abort` `again` `as` `library` `shared` `not` `true` `false` と `Fn` `Pure` である。名前には使えない。

プログラム:

```text
program         := outer_item* section*
outer_item      := "library"
                 | type_definition
                 | definition                                             根のファイルに限る（第50節）
section         := module_section | implement_section | functor_section | apply_section
                 | effect_section | handler_section
```

最初の見出し（`module` `implement` `effect` `handler`）より前が「外」、以後が節の列である。見出しは式や項目の中に現れないので、節の終わりは次の見出しかファイル末尾で決まる。`program` は 1 ファイルの文法で、複数ファイルとディレクトリの結合は第50節と第51節が定める。

名前付き定義:

```text
definition      := "def" name "=" expression
                 | "def" ["rec"] name parameters return_type? "=>" expression
                 | "def" ["rec"] name parameters return_type? condition_arms                  第32節
                 | "def" ["rec"] name pattern_arms                                            第31節。arity は 1
parameters      := param+
                 | "(" typed_param ("," typed_param)* ")"
param           := name
                 | "_"                                                    第39節の条件を満たす def と fn と handler の腕に限る（E519）
typed_param     := param ":" type_expr
return_type     := ":" result
```

`def rec` は関数定義にのみ使用できる（E102）。`parameters` は 1 個以上である（E109）。`parameters` の直後が `=>` なら本体、`|` なら条件列である。

型定義:

```text
type_definition := "type" type_vars? upper_name "=" ("|" constructor)+
                 | "type" type_vars? upper_name "=" record_type
                 | "type" type_vars? upper_name "=" upper_name record_type
type_vars       := upper_name
                 | "(" upper_name ("," upper_name)* ")"                   2 つ以上
constructor     := upper_name ["(" type_expr ("," type_expr)* ")" | record_type]
record_type     := "{" field_type (";" field_type)* "}"
field_type      := name ":" type_expr
```

型変数は名前の前で宣言する（`type T Option = ...`、`type (K, V) Pair = ...`）。型の右辺はconstructor列、record型、単一の名前付きfield constructorの3形に限る。別名（`type Id = Int`）は無い（第18節）。

型式:

```text
type_expr       := type_atom type_ref*                                     後置の適用。左から結合
type_atom       := type_ref
                 | "(" type_expr "," type_expr ("," type_expr)* ")"       2 つ以上の型引数
                 | "Fn" "(" type_expr ("," type_expr)* "=>" result ")"     関数型（第20節）
result          := type_expr effect_row?                                   矢印の結果
effect_row      := "{" type_expr ("," type_expr)* "}"                      効果の行（第20節）
                 | "Pure"
type_ref        := path? upper_name ["." upper_name]                       型か型変数。`X.T` は抽象型（第46節、第47節）
path            := (name "::")+                                            名前空間（第51節）
```

`Int List` は `List` に `Int` を与えた型で、`Int List Option` は `(Int List) Option` である。型引数が 1 つのとき括弧は書かない（`(Int) List` は E100）。`Fn` はキーワードなので、`Fn(` が後置の適用に読まれることはない。型の印字（`hado check --print-type` と診断）はこの形の正準形で、カンマの後に空白を 1 つ置く（`(Int, String) Pair`、`Fn(Int, Int => Int)`）。効果行の印字は第20節に従う。

module:

```text
module_section  := "module" upper_name module_item*
module_item     := "type" type_vars? upper_name                                          抽象型
                 | "def" type_vars? name ":" result                                      値 member
                 | "def" type_vars? name "(" contract_param ("," contract_param)* ")" ":" result
contract_param  := name ":" type_expr
implement_section := "implement" contract_ref "as" upper_name implement_item*
functor_section := "implement" contract_ref "as" upper_name "(" functor_param ("," functor_param)* ")" implement_item*
functor_param   := upper_name ":" contract_ref
apply_section   := "implement" contract_ref "as" upper_name "=" impl_ref "(" impl_ref ("," impl_ref)* ")"
contract_ref    := path? upper_name
impl_ref        := path? upper_name
implement_item  := definition
                 | type_definition
                 | "type" type_vars? upper_name "=" type_expr             契約の抽象型の実体化に限る
                 | "shared" "type" type_vars? upper_name "=" type_expr    等式を公開する実体化
effect_section  := "effect" type_vars? upper_name effect_item*
effect_item     := op_kind name "(" contract_param ("," contract_param)* ")" [":" result]
                 | op_kind name [":" result]                              引数を取らない操作
op_kind         := "once" | "abort" | "again"
handler_section := "handler" type_vars? contract_ref "as" upper_name handler_params? handler_item*
handler_params  := "(" typed_param ("," typed_param)* ")"
handler_item    := op_kind name param* "=>" expression
                 | "return" param "=>" expression
```

契約の `type` は本体を持たない（E100）。契約の `def` は全ての引数と戻りに型を書き、型変数は名前の前で宣言する（`def (A, B) map(...)`）。`as` はキーワードで、名前には使えない。`shared` は実体化の等式を公開する印で、契約の抽象型にだけ前置できる（第47節）。`library` は外にだけ書け、そのディレクトリをライブラリ根にする（第52節）。1 ファイルに 1 つ、1 ディレクトリに 1 つで、2 つ目は E205 とする。`functor_section` と `apply_section` は第48節が定め、`apply_section` の後に `implement_item` を置くと E100 である。

式（優先順位の低い方から）:

```text
expression      := pipe | prefix_form
prefix_form     := binding | rail_binding | rail_discard | with_form | branch
pipe            := or ("|>" or)*
or              := and ("||" and)*
and             := compare ("&&" compare)*
compare         := additive [("<" | "<=" | ">" | ">=" | "==" | "!=") additive]     非結合
additive        := multiplicative (("+" | "-") multiplicative)*
multiplicative  := unary (("*" | "/" | "%") unary)*
unary           := "-" unary | "not" unary | application
application     := postfix postfix*                                        並置。先頭が constructor なら E106
postfix         := primary ("." name)*                                     field access
primary         := int | float | char | string | bool | unit
                 | name
                 | dunder_name                                            呼び出しの先頭に限る（E203）
                 | path? upper_name "." name                              実装の値（List.map、store::MemoryStore.find）
                 | constructor_expr
                 | record_expr
                 | list_expr
                 | lambda                                                 `fn(` で始まり `)` で閉じる
                 | "resume"                                               again の腕の中に限る（第38節）
                 | "(" expression ")"
constructor_expr := path? upper_name ["(" expression ("," expression)* ")" | record_expr]
record_expr     := "{" name "=" expression (";" name "=" expression)* "}"
list_expr       := "[" "]"
                 | "[" expression ("," expression)* ["," "..." expression] "]"
```

constructor名に続く `()` または `{}` は空白によらずconstructorのpayloadとして先に読み、constructor名とrecord式の並置には分けない。

`...` の後は `]` まで 1 つの式である。

`prefix_form` は括弧で囲めば `primary` になる。`lambda` は自分の括弧で閉じるので `primary` であり、`List.map xs fn(x => x + 1)` のように並置の引数に直接置ける。

継続と匿名関数:

```text
with_form       := "with" impl_ref with_args? "->" expression                    第38節
with_args       := "(" expression ("," expression)* ")"
binding         := "let" pattern "=" expression "->" expression
rail_binding    := "let" pattern "?" "=" expression "->" expression
rail_discard    := "do" expression "->" expression
lambda          := "fn" "(" lambda_params "=>" expression ")"
                 | "fn" "(" lambda_params condition_arms ")"                     第32節
                 | "fn" "(" pattern_arms ")"                                     第31節
lambda_params   := param ("," param)*
                 | typed_param ("," typed_param)*
```

`fn` の引数はカンマで区切る（`fn(x, y => x + y)`、`fn(x: Int, y: Int => x + y)`）。型注釈の有無を混ぜることはできない（E100）。3 つの形はいずれも `)` で終わるので、分岐の腕の中に置いても後続の腕を飲まない。

分岐:

```text
branch          := boolean_branch | condition_branch | pattern_branch
boolean_branch  := "if" expression "=>" arm_body "|" arm_body
condition_branch := "if" condition_arms
condition_arms  := (condition_arm | pattern_fallback)+
condition_arm   := "|" expression "=>" arm_body
pattern_branch  := "if" expression ("," expression)* pattern_arms       scrutinee の列（第26節）
pattern_arms    := (pattern_arm | pattern_fallback)+
pattern_arm     := "|" pattern ("," pattern)* "=>" (arm_body | arm_conditions)  pattern の列は scrutinee と同数（E125）
arm_conditions  := "if" condition_arms                                    腕の条件列（第26.1節）。最初の fallback で終わる
pattern_fallback := "|" arm_body
arm_body        := pipe | binding | rail_binding | rail_discard            branch を直接置けない（E101）
```

`arm_conditions` は最初の `pattern_fallback` で終わり、その後の `|` は外側の pattern の腕である。fallback を持たない `arm_conditions` は続く `|` の腕を条件として読む（第26.1節）。`arm_body` の継続や `lambda` の本体に branch を置くことも E101 である（第30節）。`fn(| ... )` と `fn(x | ...)` の腕形式も branch であり、腕の中に置けば E101 である。文法は直接の入れ子だけを除く。腕の列は `=>` を持つ腕と持たない腕（fallback）の並びとして緩く読み、fallback が無い条件列（E401）、fallback より後の腕（E404）、2 つ目の fallback（E404）は網羅性の段階で当てる（第56節）。`=>` を持つ腕が 1 本も無い列（`if | 5`、`def f x | 5`、`fn(| 5)`）は E100 とし、先頭の `|` を指す。

パターン:

```text
pattern         := "_"
                 | name
                 | path? upper_name ["(" pattern ("," pattern)* ")" | record_pattern]
                 | record_pattern
                 | list_pattern
                 | literal_pattern
list_pattern    := "[" "]"
                 | "[" pattern ("," pattern)* ["," "..." (name | "_")] "]"
record_pattern  := "{" field_pattern (";" field_pattern)* "}"
field_pattern   := name
                 | name "=" pattern
literal_pattern := ["-"] int | ["-"] float | char | string | bool
```

`pattern_fallback` は必ず最後に 1 つ置く（E404）。pattern branch に fallback が無い場合は網羅性を検証する（E402）。

`if` の 3 形は直後の構文で区別する。

* `if` の直後にexpressionがあり、その後に `=>` が続く場合はBoolean branch
* `if` の直後にexpressionがあり、その後に `| pattern =>` が続く場合はpattern branch。expression がカンマで続く場合も pattern branch
* `if` の直後が `|` なら条件列

パーサは改行を見ない。
