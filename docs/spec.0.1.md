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

---

# 4. 再帰

再帰は明示する。

```ocaml
def rec factorial n =>
  if n <= 1 => 1 | n * factorial (n - 1)
```

暗黙の自己再帰は許可しない（E201）。

`def rec` は、値定義ではなく関数定義にのみ使用できる。値定義に `rec` を付けると E102 とする。

top-level は定義順に依存しない。呼び出しグラフの閉路に含まれる関数は全て `rec` とする（E516）。閉路に値定義が含まれるとエラーである（E514）。値定義は `rec` を持てない。

多相再帰はコンパイルエラーとする（E701）。

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

`main` の型は単相で、Fn を含んではならない。`def main = Nil` は型変数が残るため E702、`def main = inc` は印字できず Fn を含むため E703 とする。

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

関数の呼び出しは並置 `f x y`、constructor の呼び出しは括弧 `Some(x)` とする。大文字で始まる名前は constructor である。

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

`of` は使わない。

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
  age: Int;
}
```

アクセス：

```ocaml
user.name
```

構築は次の形とする。

```ocaml
{ name = "a"; age = 3 }
```

型は field 集合から一意に決まる nominal 型である。曖昧なら注釈を要求する（E304）。

パターンは `{ name }`（同名束縛）と `{ name = n }`（別名）とする。書かない field は無視する。field 更新構文は持たない。

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

リテラルパターンを持つ。Int / Float / Char / String / Bool と、入れ子の中で使える。Bool 以外のリテラルパターンを使う分岐は fallback 必須である。

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

`x` が継続内で使われていないならコンパイルエラー（E505）。

未使用の判定は top-level の `def` にも適用する（E513）。`main` は例外である。

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

ここでの `Error` は prelude の constructor `Error` とは別物であり、signature を書く側が定義する型である。

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

v0.1 の module 実装は次の形とする。signature 注釈は必須である（E601）。

```ocaml
module UserStore : USER_STORE = struct
  type T = User

  def find id =>
    ...

  def save user =>
    ...
end
```

struct は sig に載らない定義を持てる。sig の `type T` は外から不透明である。

```ocaml
def name (t: UserStore.T) => t.name
```

は E602 とする。

struct 内で sig からも内部からも参照されない `def` はエラーとする（E515）。

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

`Fn<(Int, Int), Int>` の `( )` は引数列であり、tuple は持たない。

`Fn` は n 引数である。定義と呼び出しの引数数は一致必須であり、部分適用は持たない。規則は第8節に従う。

全ての shin 関数は環境を第 1 引数に取る。名前付き関数は環境を無視する。単相の `Fn` 型ごとに `tail` 規約の signature が一つに定まる。

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
struct
module
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

pattern
  := "_"
   | name
   | name "(" pattern ("," pattern)* ")"
   | record_pattern
   | literal_pattern
```

record：

```text
record_expr
  := "{" name "=" expression (";" name "=" expression)* "}"

record_pattern
  := "{" name "}"
   | "{" name "=" pattern "}"
```

List リテラル：

```text
list_expr
  := "[" "]"
   | "[" expression ("," expression)* "]"
```

module：

```text
module_impl
  := "module" name ":" name "=" "struct" (type_definition | definition)* "end"

type_definition
  := "type" name "=" type_expr
```

リテラルパターン：

```text
literal_pattern
  := int | float | char | string | bool
```

意味上、`pattern_fallback` は必ず最後に置く。pattern branchにfallbackがない場合は、pattern coverage checkerによって網羅性を検証する。

条件列とpattern branchは、直後の構文によって区別する。

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

Int は 10 進のみとする。

Float は小数点の両側に数字を必須とする。`.5` や `5.` は文法に含めない。

Char は `'a'` の形とする。値は Unicode スカラ値である。

String は `"..."` の形とする。エスケープは `\n` `\t` `\\` `\"` に限る。

Bool は `true` と `false` とする。

Unit は `()` とする。

コメントは `--` による行コメントのみとする。

---

# 44. 演算子・優先順位・短絡評価

演算子は次に限る。算術 `+ - * / %`、比較 `< <= > >= == !=`、論理 `&&` `||` `not`。

算術と順序比較 `< <= > >=` は Int / Float 両用の組み込み多相であり、単相化時に確定する。未確定なら Int とする。`/` と `%` の戻り型は第36節に従う。Float に `%` は置かない。

文字列連結演算子は置かない。

優先順位は高い方から、適用、単項 `-`、`* / %`、`+ -`、比較、`&&`、`||`、`|>` とする。比較は非結合である。

`&&` と `||` は短絡評価する。左辺が結果を決めるとき、右辺を評価しない。

---

# 45. 構造比較の対象型

`==` と `!=` は、Fn を含まない全型の構造比較である。M5 の派生パスが型ごとに合成する。

Fn を含む型への `==` `!=` は E704、`compare` は E705 とする。単相化時に判明する。

---

# 46. List リテラル

`[a, b, c]` は `Cons` 連鎖の糖衣である。空は `Nil` とする。

末尾がリテラルである `Cons(a, Nil)` と `Cons(a, [b])` は拒否する（E510）。

---

# 47. 名前空間

名前空間は型・module・constructor・値の 4 つとする。`Option<T>`（型）と `Option.map`（module）は共存する。

型と constructor は別の空間である。

prelude の constructor `Some` `None` `Ok` `Error` `Nil` `Cons` は修飾なしで参照できる。

`__` で始まる名前は `stdlib/` のファイルでのみ解決する。ユーザーコードでは E203 とする。呼び出し位置でしか参照できず、値化は E204 とする。

---

# 48. 末尾呼び出しの保証

末尾位置の関数呼び出しはスタックを消費しない。自己再帰・相互再帰・関数値経由を含む。`def rec` が唯一のループであるため、仕様で保証する。

全ての shin 関数は環境を第 1 引数に取る。名前付き関数は環境を無視する。単相の `Fn` 型ごとに `tail` 規約の signature が一つに定まる。関数値経由の末尾呼び出しは `return_call_indirect` で表せる。

---

# 49. prelude

言語が名前で解決する型は `Option` `Result` `List` `DivisionError` とする。`/` の戻り型、`let?` の対象、List リテラルの展開先に用いる。

prelude の各 module は `module X : SIG = struct ... end` の形で書く。第35節の規則に従う。

prelude は組み込みを次の形で包む。

```ocaml
def length s => __string_length s
```

M5 の extern 表の各項目は、shin 側の名前（`__string_length` 等）と型を持つ。

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

---

# 50. エラーコード

拒否コーパスは次のコードで照合する。

| コード | 短い名前 | 節 |
|---|---|---|
| E101 | nested-branch | §20 |
| E102 | rec-on-value | §4 |
| E107 | operator-overloading | §38 |
| E201 | implicit-recursion | §4 |
| E203 | dunder-in-user | §47 |
| E204 | dunder-as-value | §47 |
| E301 | arity-mismatch | §8 |
| E302 | recursive-type-arg | §10 |
| E303 | let-question-type | §22 |
| E304 | record-ambiguous | §12 |
| E401 | missing-cond-fallback | §15 |
| E402 | non-exhaustive | §18 |
| E403 | unreachable-branch | §31 |
| E404 | fallback-not-last | §31 |
| E405 | duplicate-pattern | §32 |
| E406 | duplicate-condition | §32 |
| E501 | identity-binding | §29 |
| E502 | identical-branches | §30 |
| E503 | catch-all-underscore | §19 |
| E504 | let-question-underscore | §23 |
| E505 | unused-binding | §27 |
| E506 | implicit-discard | §28 |
| E507 | rewrap | §33 |
| E508 | repropagate | §24 |
| E509 | ok-unit-parens | §21 |
| E510 | cons-literal-tail | §46 |
| E511 | cond-to-literal-pattern | §16 |
| E512 | top-var-pattern | §16 |
| E513 | unused-toplevel-def | §27 |
| E514 | rec-cycle-value | §4 |
| E515 | unused-struct-def | §35 |
| E516 | missing-rec | §4 |
| E601 | missing-signature | §35 |
| E602 | opaque-type | §35 |
| E701 | poly-recursion | §4 |
| E702 | main-polymorphic | §5 |
| E703 | main-contains-fn | §5 |
| E704 | eq-on-fn | §45 |
| E705 | compare-on-fn | §45 |

