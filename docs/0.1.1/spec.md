# 言語仕様案 v0.1.1

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
* Explicit failure with `Option` / `Result`
* Railway Oriented Programming by default
* Flat control flow
* Canonical syntax
* Redundant code is rejected
* Machine-checkable coding conventions belong in the language

特に重要な原則は次の2つ。

> **Control flow should be flat.**

> **If code can be removed without changing meaning, it should not compile.**

---

# 2. Canonical Code Principle

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

---

# 3. 名前付き定義

トップレベルやmodule内の名前付き定義には `def` を使う。

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

関数定義の引数は 1 個以上である。引数を持たない `=>` の定義（`def f => 1`）と、引数を持たず `|` が続く定義（`def f | true => 1 | 0`）は E109 とする。引数が 1 つで、その引数を分岐の入口としてしか使わない関数は、第52節の形で書く。本体が条件列だけの関数は、第53節の形で書く。

---

# 4. 再帰

再帰は明示する。

```ocaml
def rec factorial n =>
  if n <= 1 => 1 | n * factorial (n - 1)
```

暗黙の自己再帰は許可しない（E201）。

`def rec` は、値定義ではなく関数定義にのみ使用できる。値定義に `rec` を付けると E102 とする。

top-level は定義順に依存しない。呼び出しグラフで異なる2つ以上の定義を通る閉路に含まれる関数は全て `rec` とする（E208）。`rec` のない関数の自己参照には E201 を適用する。閉路に値定義が含まれるとエラーである（E207）。値定義は `rec` を持てない。

多相再帰はコンパイルエラーとする（E307）。E307 は、`rec` 関数の再帰呼び出しにおいて、引数の型と仮引数の型の単一化が出現検査に失敗した場合である。それ以外の出現検査の失敗は E306（第9節）とする。 別の関数への引数や戻り値を経由する間接的な再帰も対象とする。再帰呼び出しの位置を追跡できる場合はその先頭を、関数を経由して位置を追跡できない場合は再帰関数の `def` の先頭を指す。

---

# 5. ローカルbinding

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

プログラムの入口はトップレベルの `def main` である。`main` は `let` によるローカルbindingではなく、名前付き定義である。

`main` の型は単相で、Fn を含んではならない。`def main = Nil` は型変数が残るため E701、`def main = inc` は印字できず Fn を含むため E702 とする。

`main` の要求は実行ファイルという成果物の規則である。`main` が無い、または `main` が引数を持つプログラムは E206 とする。`main` の値の印字は第51節に従う。

---

# 6. `->` の用途

`->` は、型の関数矢印や分岐結果の矢印には使わない。

v0.1では、`->` をローカルbindingの継続を示す記号としてのみ使う。

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

`let?` と `do?` も同じ継続構文を使う。

```ocaml
let? user = find_user id ->
use user
```

```ocaml
do? validate input ->
next
```

`->` は、関数型の表記には使わない。関数型は第37節で定める `Fn<...>` を使う。

`main` の本体は expression であり、`let` の継続に使う `->` では書かない。入口は `def main = expression` である。

---

# 7. 匿名関数

匿名関数には `fn` を使う。

```ocaml
fn x => x + 1
```

複数引数：

```ocaml
fn x y => x + y
```

`=>` は、関数の引数列と本体を分ける。

名前付き関数も同じ規則を使う。

```ocaml
def add x y => x + y
```

`=>` は関数定義と匿名関数においてのみ、入力と結果の対応を表す。

匿名関数も、引数のパターン分岐の形（第52節）と条件列の形（第53節）を取れる。

```ocaml
fn
| Guest => "guest"
| Member(user) => user.name
```

```ocaml
fn x
| x > 1 => x
| 0
```

---

# 8. 型注釈

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

関数型を明示する必要がある場合は、`Fn<...>` を使う。

```text
Fn<Int, String>
Fn<(Int, Int), Int>
```

`Fn` は n 引数の関数型である。定義の引数数と呼び出しの引数数は一致しなければならない。部分適用は許可しない（E301）。

`def` で定義した関数名を引数なしで参照すると、`Fn<...>` 型の値になる。

関数の呼び出しは並置 `f x y`、constructor の呼び出しは括弧 `Some(x)` とする。大文字で始まる名前は constructor である。constructor を先頭に置く並置（`Some 1`）は E106 とする。`inc(1)` は `inc (1)` と同じ並置である（空白は意味を持たない。第41節）。

constructor は関数値ではない。payload を持つ constructor は必ず payload を伴い、payload の数と引数の数が異なる形（単独の `Some`、`Some(1, 2)`、`apply Some`）は E301 とする。

---

# 9. 型推論

ML系の型推論を採用する。

```ocaml
def identity x => x
```

は型注釈なしでも多相関数として推論される。

通常のコードでは型注釈を大量に要求しない。

推論の途中で型が食い違う場合は E305 とする。型変数がそれ自身を含む型に束縛される場合
（出現検査の失敗）は E306 とする。ただし再帰呼び出しに起因する出現検査の失敗は E307（第4節）とする。

---

# 10. Generics

型パラメータには `< >` を使う。

```ocaml
Option<T>
Result<T, E>
List<T>
```

型定義：

```ocaml
type Option<T> =
  | None
  | Some(T)
```

```ocaml
type Result<T, E> =
  | Ok(T)
  | Error(E)
```

型定義の閉路（自己再帰と相互再帰）の中では、型引数は宣言の型引数と同一でなければならない。

```ocaml
type Nest<T> =
  | Leaf(T)
  | Node(Nest<List<T>>)
```

は E302 とする。派生パスの実体化を有限に保つためである。

---

# 11. Algebraic Data Types

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

`of` は使わない。constructor の列は先頭の `|` を必須とする。1 行に書く場合も同じである（`type Color = | Red | Green | Blue`）。先頭の `|` が無い型定義は E108 とする。

payload は型を名前で参照する。無名の record を payload に書くことはできない（第12節）。

値生成も同じ形。

```ocaml
Circle(10.0)
Rectangle(10.0, 20.0)
```

constructor が全て payload なしの型は素の整数とする。payload を一つでも持つ型は、全 constructor を一律に GC ヒープの box にする。一つの型の中で即値と pointer を混在させない。

---

# 12. Record

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

型の別名（`type Id = Int`）は持たない。値を区別したいときは constructor が 1 つの ADT を使う（`type Id = | Id(Int)`）。型 `Id` と constructor `Id` は別の名前空間にある（第47節）。

---

# 13. 分岐

`match` は持たない。

分岐は `if` に統一する。

ただし、`if` はBoolean専用ではなく、次の3種類のbranch expressionを持つ。

1. Boolean二分岐
2. 条件列
3. pattern分岐

すべてexpressionであり、値を返す。

`if` の後に続く構文によって、どの種類の分岐かを決定する。

リテラルパターン（Int / Float / Char / String / Bool）は pattern 分岐の pattern として使える。

---

# 14. Boolean branch

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

---

# 15. 条件列

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

関数の本体が条件列だけのときは、`=> if` を省いた第53節の形で書く。

---

# 16. Pattern branch

ADT、Option、Resultなどの値をpatternで分岐する場合は、scrutineeを `if` の直後に置く。

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

Result：

```ocaml
if result
| Ok(value) => value
| Error(error) => handle error
```

pattern branchでは、各branchはpatternと `=>` と式で構成する。

```text
| pattern => expression
```

pattern branchは、ADTの構造に基づいて網羅的でなければならない。

リテラルパターンを持つ。Int / Float / Char / String / Bool と、入れ子の中で使える。Int と Float のリテラルパターンは先頭に `-` を置け、それはリテラルの一部である。Bool 以外のリテラルパターンを使う分岐は fallback 必須である。

最上位の変数パターン `| x => ...` は禁止する（E512）。入れ子の中の変数パターンは許可する。

条件列の全条件が「同一変数 `==` リテラル」なら、pattern 分岐で書けるため拒否する（E511）。

---

# 17. Pattern branchのfallback

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

Resultでも同様である。

```ocaml
if result
| Ok(x) => x
| default
```

最後の `| expression` は、pattern branchにおけるfallbackとしてのみ解釈する。

条件列では、最後の `| expression` もfallbackであるが、条件列のfallbackは条件なしの最後のbranchとして扱う。

---

# 18. Exhaustiveness

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

`let` と `let?` のパターンにも同じ規則を当てる（第22節）。

---

# 19. `_` の用途

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

`()` はパターンではない。Unit の値は名前か `_` で受ける。

---

# 20. 分岐のネストは禁止

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
branch depth <= 1
```

必要であれば、先に状態を分類する。

```ocaml
let state = classify input ->

if state
| A => ...
| B => ...
| C => ...
```

ただし、`classify` の内部で分岐を行うこと自体を禁止するものではない。禁止対象は、同一のexpression内にbranch expressionを直接ネストすることである。

---

# 21. Option / Result

失敗は主に、

```ocaml
Option<T>
Result<T, E>
```

で表現する。

```ocaml
type Option<T> =
  | None
  | Some(T)

type Result<T, E> =
  | Ok(T)
  | Error(E)
```

`null` は持たない。

unchecked exceptionもv0.1では持たない。

`Ok()` が canonical である。`Ok(())` は拒否する（E509）。

---

# 22. `let?`

成功値を使いたいROPには `let?` を使う。

```ocaml
def load_account id =>
  let? user = find_user id ->
  let? account = find_account user.account_id ->
  Ok(account)
```

`let?` は、右辺のOptionまたはResultを評価し、成功値をpatternに束縛する。

Resultの場合：

```text
Ok(value)
  => valueをbindして継続

Error(error)
  => Error(error)をそのまま伝播
```

Optionの場合：

```text
Some(value)
  => valueをbindして継続

None
  => Noneをそのまま伝播
```

`let?` の右辺は、対応するOptionまたはResult型でなければならない。そうでなければコンパイルエラーとする（E303）。

`let` と `let?` のパターンは、束縛する値の型の全ての値に一致しなければならない。一致しない値があれば E402 とする。名前、record パターン、constructor が 1 つの型の constructor パターン（`let Id(n) = x`）とその入れ子は一致する。

---

# 23. `do?`

成功値は不要だが、失敗だけ伝播したい場合は `do?` を使う。

```ocaml
def publish article =>
  do? validate article ->
  do? authorize article ->
  do? persist article ->
  Ok()
```

これは、

```ocaml
let? _ = validate article ->
...
```

のcanonical form。

そのため、

```ocaml
let? _ = ...
```

は禁止（E504）。

意味は、

```text
do? expression -> continuation
```

Resultなら、

```text
Ok(_)
  => 継続

Error(error)
  => Error(error)を伝播
```

Optionなら、

```text
Some(_)
  => 継続

None
  => Noneを伝播
```

`do?` は成功値を破棄することを明示するための構文であり、通常の純粋値に対しては使用できない。式が Option または Result でなければ E303 とする。

`?` の無い `do` は無い。純粋値の破棄は意味を持たない（第28節）ためである。

---

# 24. Railway Oriented Programming

failure propagationにはpattern branchを使わない。

例えば、

```ocaml
if find_user id
| Ok(user) => ...
| Error(error) => Error(error)
```

のような単なる伝播は冗長。

canonical formは、

```ocaml
let? user = find_user id ->
...
```

成功値が不要なら、

```ocaml
do? validate user ->
...
```

責務は明確に分ける。

```text
let? = bind + propagate
do?  = discard success value + propagate
if   = meaningful domain branching
```

ただし、`if` によるResult分岐が常に禁止されるわけではない。`if` は、成功と失敗を異なるドメイン値へ変換するなど、意味のある分岐に使用できる。単に同じ失敗を再伝播するだけのbranchは冗長として拒否する（E508）。

---

# 25. Pipeline operator

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

`|>` にOption / Resultの特殊意味は持たせない。

`|>` の右辺は値名前空間の関数（名前か適用）に限る。constructor を置くこと（`x |> Some`）は E106 とする。

---

# 26. 記号体系

記号の意味を明確に分ける。

```text
=    value binding / value definition
=>   function body separator / pattern result separator / Boolean result separator
->   local continuation separator
|    branch alternative / fallback marker
|>   data flow
?    failure-aware operation
...  rest of a List
```

例えば、

```ocaml
fn x => x + 1
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
let? user = find_user id ->
let? account = find_account user.account_id ->
Ok(account)
```

は、

> 成功したら値を束縛して次へ進む

という読みになる。

---

# 27. Unused binding

未使用bindingはwarningではなくerror。

禁止：

```ocaml
let x = calculate ->
next
```

`x` が継続内で使われていないならコンパイルエラー（E505）。

E505 は `let` `let?` の束縛、pattern で束縛した変数、関数の引数の全てに適用する。pattern の一部を使わないときは `_` を書く（第19節）。引数に `_` は書けないので、引数を使わない関数は書けない。

未使用の判定は top-level の `def` `type` `module type` `module` にも適用する（E513）。`main` は例外である。prelude の定義は E513 の対象外である（第49節）。

---

# 28. 値の破棄

純粋値の暗黙的な破棄は禁止（E506）。

禁止：

```ocaml
let _ = calculate ->
next
```

成功値の破棄が意味を持つROPの場合は、

```ocaml
do? validate input ->
next
```

という専用のcanonical formを使う。

---

# 29. Identity binding

禁止（E501）：

```ocaml
let x = value ->
x
```

canonical form：

```ocaml
value
```

---

# 30. 同一branch

禁止（E502）：

```ocaml
if condition => value | value
```

canonical form：

```ocaml
value
```

Boolean branchの両方の結果が同一なら、条件自体が意味を持たないため、branch全体を削除する。

---

# 31. 到達不能branch

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

---

# 32. 重複branch

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

---

# 33. 不要な再包装

禁止（E507）：

```ocaml
if result
| Ok(x) => Ok(x)
| Error(e) => Error(e)
```

canonical form：

```ocaml
result
```

同じ意味を持つ分解・再構築は許さない。

---

# 34. Module Signature

OCamlのmodule systemの思想を取り入れる。

v0.1ではまず `sig` によるcontractを持つ。

```ocaml
module type USER_STORE = sig
  type T

  def find(id: Int): Option<T>
  def save(user: T): Result<Unit, Error>
end
```

ここでの `Error` は prelude の constructor `Error` とは別物であり、signature を書く側が定義する型である。signature の中の型名も、struct が実装されるかどうかによらず解決される。

signature の型宣言は本体を持たない `type T` に限る。本体を持つ型宣言は E100 とする。外に見せたい型は module の外で定義し、signature にはその名前を書く。

abstract type：

```ocaml
module type USER_ID = sig
  type T

  def make(value: Int): Option<T>
  def value(id: T): Int
end
```

signatureの関数は、名前の直後に型変数を宣言できる。

```ocaml
module type OPTION = sig
  def map<A, B>(value: Option<A>, f: Fn<A, B>): Option<B>
end
```

宣言した型変数はその関数のsignature内だけで有効で、全称量化される。実装は全ての型引数で使えなければならない。型変数の重複宣言は E205、未宣言の型名は E204 とする。

`val` は使わない。

signature側もimplementation側も、

```text
def = named definition
```

で統一する。

---

# 35. Module implementation

v0.1 の module 実装は次の形とする。signature 注釈は必須である（E113）。

```ocaml
module UserStore : USER_STORE = struct
  type T = User

  def find id =>
    ...

  def save user =>
    ...
end
```

struct は sig の全ての member と型を持ち、それぞれの型が一致し、sig の型変数に対して十分に多相でなければならない。満たさなければ E602 とする。struct は sig に載らない定義を持てる。sig の `type T` は外から不透明である。

抽象型の実体化で別名を展開した結果、同じ別名へ戻って実体が確定しない場合（`type T = T`、`type T = U` と `type U = T` など）は E602 とし、その module 定義の先頭を指す。名前を持つ ADT や record の再帰型は、この別名の循環とは区別する。

```ocaml
def name (t: UserStore.T) => t.name
```

は E601 とする。

struct 内で sig からも内部からも参照されない `def` はエラーとする（E514）。

Functorやfirst-class moduleはv0.1には入れない。

---

# 36. Primitive Types

最低限：

```text
Unit
Bool
Int
Float
Char
String
```

Int は i64 とする。加減乗はオーバーフローで wrap する。`/` と `%` は `Result<Int, DivisionError>` を返す。除数 0 は `Error(DivisionByZero)` とする。

`DivisionError` の variant は `DivisionByZero` のみとする。返すのは Int の `/` と `%` に限る。

除算の端は次とする。`i64::MIN / -1` は wrap して `Ok(i64::MIN)`。`i64::MIN % -1` は `Ok(0)`。

Float は f64 とする。Float の `/` は Float を返す。Float に `%` は置かない。

Bool は i8、Char は i32、Unit は i64 の 0 とする。Cranelift に i1 はない。

Char は Unicode スカラ値とする。String は不変の UTF-8 とする。

parameterized type：

```text
Option<T>
Result<T, E>
List<T>
```

---

# 37. Function Types

関数型については `Int -> String` のようなML記法を採用しない。

関数型は `Fn<...>` で表す。

単一引数：

```text
Fn<Int, String>
```

複数引数：

```text
Fn<(Int, Int), Int>
```

`Fn<(Int, Int), Int>` の `( )` は引数列であり、tuple は持たない。式やパターンに tuple の構文を書くと E105 とする。

`Fn` は n 引数である。定義と呼び出しの引数数は一致必須であり、部分適用は持たない。規則は第8節に従う。

全ての hado 関数は環境を第 1 引数に取る。名前付き関数は環境を無視する。単相の `Fn` 型ごとに `tail` 規約の signature が一つに定まる。

signatureでは通常、

```ocaml
def parse(input: String): Result<User, Error>
```

のように書く。

`->` は関数型の表記には使わず、継続separatorに限定する。

---

# 38. v0.1では入れないもの

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
functors
first-class modules
macros
effect system
traits / typeclasses
implicit conversion
operator overloading
async / await
concurrency primitives
```

operator overloading の禁止は維持する（E107）。組み込み算術演算子は、ユーザーが定義できない固定集合への型指向解決である。

必要になった段階で追加を検討する。

---

# 39. 典型コード

Resultを使った処理：

```ocaml
def create_user input =>
  let? parsed = parse input ->
  do? validate parsed ->
  do? authorize parsed ->
  let? saved = save parsed ->
  Ok(saved)
```

値を返さない処理：

```ocaml
def delete_user id =>
  let? user = find_user id ->
  do? authorize_delete user ->
  do? delete user ->
  Ok()
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
  |> List.filter (fn user => user.active)
  |> List.map (fn user => user.name)
```

module signature：

```ocaml
module type USER_STORE = sig
  type T

  def find(id: Int): Option<T>
  def save(user: T): Result<Unit, Error>
  def delete(user: T): Result<Unit, Error>
end
```

---

# 40. コア構文

v0.1の中心となる構文はかなり小さい。

```text
def
def rec

let
let?
do?
->

fn
=>

type
<T>
ADT
record

if
|
_

|>

sig
struct
module
```

---

# 41. 文法の概形

字句と構文の形はこの節を正とし、第43節と第44節は値の意味だけを定める。

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
name            := lower (letter | digit | "_")*       値・field・引数の名前
upper_name      := upper (letter | digit | "_")*       型・constructor・module・module type・型変数の名前
dunder_name     := "__" name                           stdlib 限定の組み込み名（第47節）
comment         := "//" 行末までの文字                   `--` と `#` は E104
```

`_` 単独は wildcard であり名前ではない。空白とコメントはトークンの区切りで、それ以外の意味を持たない。

プログラム:

```text
program         := item*
item            := definition | type_definition | module_type | module_impl
```

名前付き定義:

```text
definition      := "def" name "=" expression
                 | "def" ["rec"] name parameters return_type? "=>" expression
                 | "def" ["rec"] name parameters return_type? condition_arm+ pattern_fallback?   第53節
                 | "def" ["rec"] name pattern_arm+ pattern_fallback?       第52節。arity は 1
parameters      := name+
                 | "(" typed_param ("," typed_param)* ")"
typed_param     := name ":" type_expr
return_type     := ":" type_expr
```

`def rec` は関数定義にのみ使用できる（E102）。`parameters` は 1 個以上である（E109）。`parameters` の直後が `=>` なら本体、`|` なら条件列である。

型定義:

```text
type_definition := "type" upper_name type_params? "=" ("|" constructor)+
                 | "type" upper_name type_params? "=" record_type
type_params     := "<" upper_name ("," upper_name)* ">"
constructor     := upper_name ["(" type_expr ("," type_expr)* ")"]
record_type     := "{" field_type (";" field_type)* "}"
field_type      := name ":" type_expr
```

型の右辺はこの 2 形に限る。別名（`type Id = Int`）は無い（第12節）。

型式:

```text
type_expr       := upper_name ["<" type_expr ("," type_expr)* ">"]        型か型変数
                 | upper_name "." upper_name                              module の抽象型（UserStore.T）
                 | "Fn" "<" fn_params "," type_expr ">"
fn_params       := type_expr
                 | "(" type_expr ("," type_expr)* ")"
```

module:

```text
module_type     := "module" "type" upper_name "=" "sig" sig_item* "end"
sig_item        := "type" upper_name
                 | "def" name type_params? "(" typed_param ("," typed_param)* ")" ":" type_expr
module_impl     := "module" upper_name ":" upper_name "=" "struct" struct_item* "end"
struct_item     := definition
                 | type_definition
                 | "type" upper_name "=" type_expr                        sig の抽象型の実体化に限る
```

sig の `type` は本体を持たない（E100）。sig の `def` は全ての引数と戻りに型を書く。

v0.1 の `module` と `module type` はトップレベルでのみ宣言できる。`struct` 内の入れ子宣言は E100 とし、内側の `module` を指す。

式（優先順位の低い方から）:

```text
expression      := pipe | prefix_form
prefix_form     := lambda | binding | rail_binding | rail_discard | branch
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
                 | upper_name "." name                                    module の値（List.map）
                 | constructor_expr
                 | record_expr
                 | list_expr
                 | "(" expression ")"
constructor_expr := "Ok" "(" ")"                                          Unit payload の正準形（第21節）
                 | upper_name ["(" expression ("," expression)* ")"]     引数は括弧（E106）
record_expr     := "{" name "=" expression (";" name "=" expression)* "}"
list_expr       := "[" "]"
                 | "[" expression ("," expression)* ["," "..." expression] "]"
```

`...` の後は `]` まで 1 つの式である。要素を伴わない `[...xs]` は文法に無い（E100）。

`prefix_form` は括弧で囲めば `primary` になる（`(fn x => x + 1)`）。

継続と匿名関数:

```text
binding         := "let" pattern "=" expression "->" expression
rail_binding    := "let?" pattern "=" expression "->" expression
rail_discard    := "do?" expression "->" expression
lambda          := "fn" parameters "=>" expression
                 | "fn" parameters condition_arm+ pattern_fallback?       第53節
                 | "fn" pattern_arm+ pattern_fallback?                    第52節
```

分岐:

```text
branch          := boolean_branch | condition_branch | pattern_branch
boolean_branch  := "if" expression "=>" arm_body "|" arm_body
condition_branch := "if" condition_arm+ pattern_fallback
condition_arm   := "|" expression "=>" arm_body
pattern_branch  := "if" expression pattern_arm+ pattern_fallback?
pattern_arm     := "|" pattern "=>" arm_body
pattern_fallback := "|" arm_body
arm_body        := pipe | lambda | binding | rail_binding | rail_discard    branch を直接置けない（E101）
```

`arm_body` の継続や lambda の本体に branch を置くことも E101 である（第20節）。文法は直接の入れ子だけを除く。

パターン:

```text
pattern         := "_"
                 | name
                 | upper_name ["(" pattern ("," pattern)* ")"]
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

`...` の後は名前か `_` に限り、他は E100 とする。要素を伴わない `[...xs]` は文法に無い（E100）。

`pattern_fallback` は必ず最後に置く（E404）。pattern branch に fallback が無い場合は網羅性を検証する（E402）。

`if` の 3 形は直後の構文で区別する。

* `if` の直後にexpressionがあり、その後に `=>` が続く場合はBoolean branch
* `if` の直後にexpressionがあり、その後に `| pattern =>` が続く場合はpattern branch
* `if` の直後が `|` なら条件列

パーサは改行を見ない。

---

# 42. 言語全体の原則

この言語では、

> **ネストよりフラット**

> **暗黙より型**

> **例外より値**

> **分岐より伝播**

> **複数表現よりcanonical form**

> **警告より、書けないこと**

を優先する。

最終的な目標は、

> **間違ったコードだけでなく、冗長なコード、曖昧なコード、構造的に悪いコードまで、可能な限り言語仕様そのもので排除すること。**

そして最も重要な原則は、

> **If code can be removed without changing meaning, it should not compile.**

---

# 43. リテラルとコメント

Int は 10 進のみとする。i64 の範囲を超えるリテラルは E112 とする。

Float は小数点の両側に数字を必須とする。`.5` や `5.` は文法に含めない（E103）。

Char は `'a'` の形とする。値は Unicode スカラ値である。エスケープは `\n` `\t` `\\` `\'` に限る。ちょうど 1 文字でない場合と、それ以外のエスケープは E111 とする。

String は `"..."` の形とする。エスケープは `\n` `\t` `\\` `\"` に限る。それ以外のエスケープは E110 とする。

Bool は `true` と `false` とする。

Unit は `()` とする。

コメントは `//` による行コメントのみとする。`--`（v0.1 の記法）と `#` を含む他の記法によるコメントは E104 とする。

第41節の文法に合わない入力で、特定のコードを持つ規則に当たらないものは E100 とする。

---

# 44. 演算子・優先順位・短絡評価

演算子は次に限る。算術 `+ - * / %`、比較 `< <= > >= == !=`、論理 `&&` `||` `not`。

算術 `+ - * /` と順序比較 `< <= > >=` は Int / Float 両用の組み込み多相であり、単相化時に確定する。未確定なら Int とする。`%` は Int 専用であり、Float に当てると E305 である。`/` と `%` の戻り型は第36節に従う。

文字列連結演算子は置かない。

優先順位は高い方から、field access `.`、適用、単項 `-` と `not`、`* / %`、`+ -`、比較、`&&`、`||`、`|>` とする。比較は非結合である。

`&&` と `||` は短絡評価する。左辺が結果を決めるとき、右辺を評価しない。

---

# 45. 構造比較の対象型

`==` と `!=` は、Fn を含まない全型の構造比較である。M5 の派生パスが型ごとに合成する。

Fn を含む型への `==` `!=` は E703、`compare` は E704 とする。単相化時に判明する。

`compare a b` は順序に応じて `-1`、`0`、`1` を返す。FloatではNaN同士を同順位とし、NaNを他の全ての値より後に置く。`-0.0` と `+0.0` は同順位とする。Floatの `==`・`!=` はIEEE 754に従い、NaN同士の `==` はfalseである。

---

# 46. List リテラル

List は式でもパターンでも `[]` と `[a, b, ...rest]` の形で書く。`[a, b, ...rest]` は `Cons(a, Cons(b, rest))` の糖衣で、`[]` は `Nil`、`[a, b]` は `Cons(a, Cons(b, Nil))` である。`...` は末尾にだけ置ける。

式では `...` の後に任意の式を置ける。

```ocaml
def rec map values f =>
  if values
  | [] => []
  | [head, ...tail] => [f head, ...map tail f]
```

パターンでは `...` の後は名前か `_` である。`[x, ...xs]` は先頭と残りを束縛し、`[x, ..._]` は先頭だけを束縛する。

`Nil` と `Cons` を利用者のコードに書くことは冗長である。式に書けば E510、パターンに書けば E516 とする。`List` 型の定義は prelude が持ち、constructor は糖衣の展開先としてだけ存在する。

---

# 47. 名前空間

名前空間は型・module・constructor・値の 4 つとする。どの名前空間でも解決できない名前の参照は E204 とする。`Option<T>`（型）と `Option.map`（module）は共存する。

型と constructor は別の空間である。同じ名前空間内の同名宣言は E205 とする。束縛の shadowing（`Member(user) => user.name` の `user`）は許す。

prelude の constructor `Some` `None` `Ok` `Error` は修飾なしで参照できる。`Nil` `Cons` は名前空間にあるが、利用者のコードに書くと E510 か E516 になる（第46節）。

`__` で始まる名前は呼び出し位置でしか参照できず、値化はファイルの場所によらず E203 とする。呼び出し位置では `stdlib/` のファイルでのみ解決し、ユーザーコードでは E202 とする。

---

# 48. 末尾呼び出しの保証

末尾位置の関数呼び出しはスタックを消費しない。自己再帰・相互再帰・関数値経由を含む。`def rec` が唯一のループであるため、仕様で保証する。

全ての hado 関数は環境を第 1 引数に取る。名前付き関数は環境を無視する。単相の `Fn` 型ごとに `tail` 規約の signature が一つに定まる。関数値経由の末尾呼び出しは `return_call_indirect` で表せる。

---

# 49. prelude

言語が名前で解決する型は `Option` `Result` `List` `DivisionError` とする。`/` の戻り型、`let?` の対象、List リテラルの展開先に用いる。

prelude の各 module は `module X : SIG = struct ... end` の形で書く。第35節の規則に従う。

prelude は組み込みを次の形で包む。

```ocaml
def length s => __string_length s
```

M5 の extern 表の各項目は、hado 側の名前（`__string_length` 等）と型を持つ。

関数集合は次とする。

```text
List: map filter fold length reverse append sort
Option: map unwrap_or is_some ok_or
Result: map map_error is_ok
String: length concat
Int / Float: to_string
```

`sort` は比較関数を引数に取る merge sort とする。

`String.length` は Unicode スカラ値の個数とする。

prelude は全プログラムに連結され、E513 を除く全ての検査を受ける。

---

# 50. エラーコード

拒否コーパスは次のコードで照合する。

検査は 字句 → 構文 → 名前解決 → 型 → 網羅性 → 冗長 → 単相化 の順に走る。1 つのプログラムに複数の欠陥があるとき、報告するのはこの順で最初の段階に当たる 1 件である。同一段階に 2 つ以上あるときは、アンカー位置のソース順で先のものを報告する。アンカーは規則が名指す最小の構文要素の先頭で、表の「アンカー」列に定める。「節」列は主となる節を 1 つ書き、拒否コーパスの `-- spec:` はこれと一致させる。コードの百の位は段階に対応する（E1xx 字句と構文、E2xx 名前解決、E3xx 型、E4xx 網羅性、E5xx 冗長、E6xx module の型、E7xx 単相化）。

| コード | 短い名前 | 節 | 段階 | アンカー |
|---|---|---|---|---|
| E100 | syntax-error | §41 | 構文 | 文法に合わない最初のトークン |
| E101 | nested-branch | §20 | 構文 | 内側の `if` |
| E102 | rec-on-value | §4 | 構文 | `rec` |
| E103 | float-literal-form | §43 | 字句 | リテラル |
| E104 | comment-form | §43 | 字句 | コメントの先頭（`--` `#` の 1 文字目） |
| E105 | no-tuple | §37 | 構文 | `(` |
| E106 | constructor-call-form | §8 | 構文 | constructor 名 |
| E107 | operator-overloading | §38 | 構文 | `def` の直後の演算子 |
| E108 | adt-leading-pipe | §11 | 構文 | 最初の constructor 名 |
| E109 | zero-parameter-arrow | §3 | 構文 | `=>`。`|` が続く形では最初の `|` |
| E110 | string-escape-form | §43 | 字句 | `\` |
| E111 | char-literal-form | §43 | 字句 | `'` |
| E112 | int-literal-range | §43 | 字句 | リテラル |
| E113 | missing-signature | §35 | 構文 | `module` |
| E201 | implicit-recursion | §4 | 名前解決 | 自分を参照する名前 |
| E202 | dunder-in-user | §47 | 名前解決 | `__` 名 |
| E203 | dunder-as-value | §47 | 名前解決 | `__` 名 |
| E204 | unbound-name | §47 | 名前解決 | 名前 |
| E205 | duplicate-name | §47 | 名前解決 | 2 回目の宣言の名前 |
| E206 | entry-form | §5 | 名前解決 | `def main`。無ければ 1:1 |
| E207 | rec-cycle-value | §4 | 名前解決 | 閉路に含まれる値定義の `def` |
| E208 | missing-rec | §4 | 名前解決 | `rec` の無い `def` |
| E301 | arity-mismatch | §8 | 型 | 適用の先頭 |
| E302 | recursive-type-arg | §10 | 型 | 宣言と異なる型引数 |
| E303 | rail-type | §22 | 型 | `let?` `do?` の右辺の式 |
| E304 | record-ambiguous | §12 | 型 | record 式か record パターンの `{`。型が未確定の field 参照では参照式の先頭 |
| E305 | type-mismatch | §9 | 型 | 型が食い違った式の先頭。適用の引数については下記 |
| E306 | occurs-check | §9 | 型 | 同上 |
| E307 | poly-recursion | §4 | 型 | 再帰呼び出しの先頭。間接呼び出しの位置を追跡できない場合は再帰関数の `def` の先頭（§4） |
| E401 | missing-cond-fallback | §15 | 網羅性 | `if` |
| E402 | non-exhaustive | §18 | 網羅性 | `if`。`let` `let?` ではそのパターン |
| E403 | unreachable-branch | §31 | 網羅性 | 到達不能な腕の `|` |
| E404 | fallback-not-last | §31 | 網羅性 | fallback より後の腕の `|` |
| E405 | duplicate-pattern | §32 | 網羅性 | 重複する 2 番目の腕の `|` |
| E406 | duplicate-condition | §32 | 網羅性 | 同上 |
| E501 | identity-binding | §29 | 冗長 | `let` |
| E502 | identical-branches | §30 | 冗長 | `if` |
| E503 | catch-all-underscore | §19 | 冗長 | `_` |
| E504 | let-question-underscore | §23 | 冗長 | `_` |
| E505 | unused-binding | §27 | 冗長 | 束縛名 |
| E506 | implicit-discard | §28 | 冗長 | `_` |
| E507 | rewrap | §33 | 冗長 | `if`。第52節の腕形式では `def` か `fn` |
| E508 | repropagate | §24 | 冗長 | 再伝播する腕の `|` |
| E509 | ok-unit-parens | §21 | 冗長 | `Ok` |
| E510 | list-literal-form | §46 | 冗長 | 式の `Cons` か `Nil` |
| E511 | cond-to-literal-pattern | §16 | 冗長 | `if` |
| E512 | top-var-pattern | §16 | 冗長 | 変数パターン |
| E513 | unused-toplevel | §27 | 冗長 | 未使用の宣言の先頭 |
| E514 | unused-struct-def | §35 | 冗長 | `def` |
| E515 | if-on-sole-parameter | §52 | 冗長 | `def` か `fn` |
| E516 | list-pattern-form | §46 | 冗長 | パターンの `Cons` か `Nil` |
| E517 | if-chain-on-parameters | §53 | 冗長 | `def` か `fn` |
| E601 | opaque-type | §35 | 型 | field access の `.` |
| E602 | signature-mismatch | §35 | 型 | `module` |
| E701 | main-polymorphic | §5 | 単相化 | `def main` |
| E702 | main-contains-fn | §5 | 単相化 | `def main` |
| E703 | eq-on-fn | §45 | 単相化 | 演算子 |
| E704 | compare-on-fn | §45 | 単相化 | `compare` |

関数・constructor の適用で、仮引数型と実引数型の単一化が E305 または E306 になる場合は、問題の実引数の先頭を指す。複数の引数は渡される順に照合する。パイプの左辺は第1引数として扱い、その元の式の位置を使う。例えば `add1 "hi"` は `"hi"`、`x x` の出現検査失敗は第2の `x` を指す。呼び出す値が関数でない場合は、その値の式の先頭を指す。E301 と E307 のアンカーは表の規則に従う。

---

# 51. 印字

`main` の値は次の規則で印字する。

Result の外側だけ剥がす。`Ok(x)` は `x` を stdout に印字して終了コード 0、`Error(e)` は `e` を stderr に印字して終了コード 1 とする。Option は剥がさない。

中身は canonical なソース表記で印字する。ソースとして再び読める形が一つに定まることを目的とする。

| 型 | 印字 |
|---|---|
| Int | 10 進 |
| Float | 往復可能な最短の 10 進桁を指数を使わず位取りで並べ、小数点の両側に最低 1 桁を置く。`3.0` は `3.0`、`0.1` は `0.1`、10 の 300 乗は `1` の後に `0` が 300 個と `.0`。`-0.0` は `-0.0`。NaN と無限大はソースで書けないので `nan` `inf` `-inf` と出す |
| Bool | `true` `false` |
| Char | 入れ子では `'a'` の形。エスケープは第43節の 4 つ |
| String | 入れ子では `"..."` の形。エスケープは第43節の 4 つ |
| Unit | 入れ子では `()` |
| ADT | `Some(1)` `Guest` |
| List | `[1, 2]` `[]`。`Cons` の形では出さない |
| record | `{ name = "a"; age = 3 }` |

最上位（Result を剥がした後の値そのもの）だけ 2 つの例外を置く。String と Char は引用符もエスケープも付けずに生の文字列を出す。Unit は何も出さない（改行も出さない）。

---

# 52. 引数のパターン分岐

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

arity は 1 である。腕の規則は第16節から第19節と同じで、網羅性（E402）、到達不能（E403）、重複（E405）、fallback の位置（E404）、`_` の禁止（E503）、最上位の変数パターンの禁止（E512）をそのまま当てる。`fn` も同じ形を取る（第7節）。

この形で書ける関数を、引数名と `if` を付けて書くことは冗長であり E515 とする。E515 は次の 4 条件を全て満たすときに当たる。

* 引数が 1 つである
* 引数と戻りに型注釈が無い
* 引数の自由な参照が scrutinee の 1 回だけである（腕の中で同じ名前を再束縛したものは数えない）
* 本体が、その引数を scrutinee にした pattern 分岐ちょうどである

---

# 53. 引数の条件列

本体が条件列（第15節）だけの関数は、`=> if` を省いて引数列の直後に腕を並べる。

```ocaml
def classify score
| score >= 90 => Excellent
| score >= 70 => Good
| score >= 50 => Pass
| Fail
```

引数の数と型注釈は制限しない。`def sign_of (a: Int) (b: Int): Int | a < b => -1 | a > b => 1 | 0` も同じ形である。腕の規則は第15節と同じで、fallback の必須（E401）と重複条件（E406）をそのまま当てる。`fn` も同じ形を取る（第7節）。

引数名の直後の `|` は常に条件列であり、pattern 分岐にはならない。`def unwrap x | Some(y) => y | 0` の `Some(y)` は式として読まれ、`y` が E204 になる。引数を pattern で分岐する関数は第52節の形で書く。

本体が条件列ちょうどである関数を `=> if` を付けて書くことは冗長であり E517 とする。引数の数と型注釈の有無によらず当てる。`let` などの継続を挟んだ後に条件列が来る関数は対象外である。
