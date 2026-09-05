# 言語仕様案 v0.1

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

---

# 4. 再帰

再帰は明示する。

```ocaml
def rec factorial n =>
  if n <= 1 => 1 | n * factorial (n - 1)
```

暗黙の自己再帰は許可しない。

`def rec` は、値定義ではなく関数定義にのみ使用できる。

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

---

# 9. 型推論

ML系の型推論を採用する。

```ocaml
def identity x => x
```

は型注釈なしでも多相関数として推論される。

通常のコードでは型注釈を大量に要求しない。

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

`of` は使わない。

値生成も同じ形。

```ocaml
Circle(10.0)
Rectangle(10.0, 20.0)
```

---

# 12. Record

Recordを持つ。

```ocaml
type User = {
  name: String;
  age: Int;
}
```

アクセス：

```ocaml
user.name
```

ADTとの組み合わせ：

```ocaml
type User =
  | Guest
  | Member({
      name: String;
      age: Int;
    })
```

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

条件列にfallbackがない場合はコンパイルエラーとする。

`if / else if / else` の構文は持たない。

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

はコンパイルエラー。

`None` が未処理であり、fallbackも存在しないためである。

次のいずれかを満たす必要がある。

* すべてのconstructorを明示する
* 最後にfallbackを置く

非網羅branchはwarningではなくerror。

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

は禁止。

fallbackのcanonical formは、

```ocaml
| default
```

である。

---

# 20. 分岐のネストは禁止

branch expressionの内部にbranch expressionを書くことは禁止する。

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

`let?` の右辺は、対応するOptionまたはResult型でなければならない。

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

は禁止。

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

`do?` は成功値を破棄することを明示するための構文であり、通常の純粋値に対しては使用できない。

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

ただし、`if` によるResult分岐が常に禁止されるわけではない。`if` は、成功と失敗を異なるドメイン値へ変換するなど、意味のある分岐に使用できる。単に同じ失敗を再伝播するだけのbranchは冗長として拒否する。

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

```ocaml
users
|> List.filter is_active
|> List.map display_name
|> List.sort compare
```

`|>` にOption / Resultの特殊意味は持たせない。

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

`x` が継続内で使われていないならコンパイルエラー。

---

# 28. 値の破棄

純粋値の暗黙的な破棄は禁止。

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

禁止：

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

禁止：

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

到達不能branchはerror。

禁止：

```ocaml
if value
| Some(x) => ...
| None => ...
| default
```

`Some` と `None` で既に網羅されている。

同様に、条件列でfallbackより後ろにbranchを置くことはできない。fallbackは必ず最後に置く。

---

# 32. 重複branch

同一patternの重複は禁止。

```ocaml
if value
| Some(x) => ...
| Some(y) => ...
| None => ...
```

はerror。

条件列でも、同一条件または静的に到達不能な条件の重複はerrorとする。

---

# 33. 不要な再包装

禁止：

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

abstract type：

```ocaml
module type USER_ID = sig
  type T

  def make(value: Int): Option<T>
  def value(id: T): Int
end
```

`val` は使わない。

signature側もimplementation側も、

```text
def = named definition
```

で統一する。

---

# 35. Module implementation

v0.1では `sig` を優先し、実装側の詳細なmodule syntaxは後回し。

将来的には、

```ocaml
module UserStore : USER_STORE = struct
  type T = User

  def find id =>
    ...

  def save user =>
    ...
end
```

のようなOCaml的なmodule implementationを検討できる。

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
def display user =>
  if user
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
def classify score =>
  if
  | score >= 90 => Excellent
  | score >= 70 => Good
  | score >= 50 => Pass
  | Fail
```

pattern fallback：

```ocaml
def name user =>
  if user
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
```

---

# 41. 文法の概形

名前付き定義：

```text
definition
  := "def" name "=" expression
   | "def" ["rec"] name parameters return_type? "=>" expression
```

`def rec` は関数定義にのみ使用できる。

ローカルbinding：

```text
binding
  := "let" pattern "=" expression "->" expression

rail_binding
  := "let?" pattern "=" expression "->" expression

rail_discard
  := "do?" expression "->" expression
```

匿名関数：

```text
lambda
  := "fn" parameters "=>" expression
```

Boolean branch：

```text
boolean_branch
  := "if" expression "=>" expression "|" expression
```

条件列：

```text
condition_branch
  := "if"
     ("|" expression "=>" expression)+
     "|" expression
```

pattern branch：

```text
pattern_branch
  := "if" expression pattern_arm+ pattern_fallback?

pattern_arm
  := "|" pattern "=>" expression

pattern_fallback
  := "|" expression
```

意味上、`pattern_fallback` は必ず最後に置く。pattern branchにfallbackがない場合は、pattern coverage checkerによって網羅性を検証する。

条件列とpattern branchは、先頭の構文によって区別する。

* `if` の直後にexpressionがあり、その後に `=>` が続く場合はBoolean branch
* `if` の直後にexpressionがあり、その後に `| pattern =>` が続く場合はpattern branch
* `if` の直後に改行された `| condition =>` が続く場合は条件列

実装上の曖昧性を避けるため、条件列は必ず改行後の `|` から開始する。

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

