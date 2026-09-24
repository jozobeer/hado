# 規範文の追跡表 v0.4

`docs/spec.md` の規範文（「〜とする」「〜でなければならない」「〜は E xxx」）が、どの検査段階で判定され、何が固定するか。エラーコードを持つ規範文は §56 の表と拒否コーパスが 1 対 1 で固定するので、ここにはコードを持たない規範文と、コードだけでは範囲が読めない規範文を置く。

「固定するもの」の列は R9、R11〜R14、R16〜R26、R32〜R38 で作る判定器を指す。R9〜R28 の判定は `crates/hado/tests/` の各テストにあり、m0 の全走（`cargo test --offline --no-fail-fast`）が回す。`R9-1` などの項目名は `docs/rounds/r8-r9.md` の R9 に対応し、`verify-r32.sh` 以降は `docs/plan.md` 24 節の R32〜R38 に対応する。

この表は R81 の行で止める。以後のラウンドの系譜は `docs/rounds/<slug>.md` の「系譜」節に同じ 5 列で書く（`docs/rounds.md`）。

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §2 | eta 展開の `def` は冗長としない | 冗長 | 受理 | `tests/accept/eta/main.hd` |
| §8 | 引数の型注釈は括弧付き引数構文でのみ書ける | 構文 | E100 | EBNF 生成器（R9-1） |
| §9 | E307 は再帰呼び出しの引数型と仮引数型の単一化が出現検査に失敗した場合 | 型 | E307 / E306 | `tests/reject/E307/main.hd`、`E306.hd` |
| §10 | `main` の要求は実行ファイルの規則。`main` の無い `.hd` の `check` はライブラリ（第52節） | 名前解決 | E206 / 受理 | `tests/reject/E206/main.hd`（`library` のディレクトリの `main`、R32）、`verify-r32.sh` のプローブ |
| §13 | constructor を先頭に置く並置は E106。`inc(1)` は通す | 構文 | E106 | `tests/reject/E106/main.hd`、R9-1 の判定器 |
| §13 | constructor は関数値ではない。payload の数の不一致は E301 | 型 | E301 | R9-3 の判定器（0 個と arity+1 個） |
| §18 | record は必ず名前を持つ | 構文 | E100 | EBNF 生成器 |
| §18 | 型の別名は無い | 構文 | E100 | EBNF 生成器 |
| §26 | 負のリテラルパターン | 構文 | 受理 | EBNF 生成器、E405 の判定器 |
| §29 | `()` はパターンではない | 構文 | E100 | EBNF 生成器 |
| §34 | `let` のパターンは `?` の有無によらず網羅的 | 網羅性 | E402 | R9-4 の判定器（refutable な `let`）、R14 の判定器（14-b(5)、`let p? =`） |
| §35 | `do` の右辺は効果を持つ式か Option。純粋な値の破棄は E303。`?` の印は無い | 型 / 構文 | E303 / E100 | R9-2 の ill-typed 生成器、EBNF 生成器（`do?` `let?` は E100、R14） |
| §14 | パイプの右辺に constructor は置けない | 構文 | E106 | R9-1 の判定器 |
| §39 | E505 は pattern 束縛と引数に及ぶ | 冗長 | E505 | R9-4 の判定器 |
| §39 | E513 は `type` `module` `implement` に及ぶ。ライブラリは公開面（実装・適用結果・未適用 functor・実装の無い契約・外の型・効果・handler）を根にする | 冗長 | E513 | R9-3 の判定器、R17 の判定器（未参照の契約と実装）、`verify-r32.sh`（公開面と非公開 def） |
| §46 | 契約の型名は実装の有無によらず解決される | 名前解決 | E204 | R9-3 の判定器 |
| §46 | 契約の型は抽象型のみ | 構文 | E100 | EBNF 生成器 |
| §47 | 実装は契約の全 member と型を持ち、型が一致し、十分に多相 | 型 | E602 | `tests/reject/E602/main.hd`、R9-3 の判定器（4 種類） |
| 付録C | 比較は非結合 | 構文 | E100 | EBNF 生成器 |
| 付録C | 空白とコメントは区切りで、意味を持たない | 字句 | 受理 | R9-1 の空白摂動（識別子と `(` の間の空白の有無で同じ木になる） |
| §5 | Char のエスケープは 4 つ | 字句 | E111 | `tests/reject/E111/main.hd` |
| §7 | `%` は Int 専用 | 型 | E305 | R9-2 の ill-typed 生成器 |
| §7 | 短絡評価 | 実行 | 受理 | `tests/accept/short_and/main.hd` `short_or.hd` |
| §21 | `compare` は Fn を含まない全型 | 単相化 | E704 | `tests/reject/E704/main.hd`、`verify-m5.sh` の comparison プローブ |
| §22 | パターンに List リテラルの糖衣は無い | 構文 | E100 | EBNF 生成器 |
| §49 | shadowing は許す | 名前解決 | 受理 | `tests/accept/display/main.hd`（`Member(user) => user.name`） |
| §53 | 末尾位置の呼び出しはスタックを消費しない（3 経路） | 実行 | 受理 | `tests/accept/tail_self/main.hd` `tail_mutual.hd` `tail_closure.hd` |
| §53.1 | スタックが尽きたら `hado: stack overflow` を stderr に出して終了 1。`raise` の `Error` ではない | 実行 | — | `verify-r50.sh` |
| §55 | prelude は全ての検査を公開面の根として受ける | 全段階 | — | `stdlib/` が全プログラムに連結されて `check_source` を通ること。`verify-r32.sh` |
| §56 | 段階順で最初の 1 件を報告する | 全段階 | — | R9-2 の判定器（同居させた欠陥） |
| §56 | 同一段階内はアンカー位置順 | 全段階 | — | R9-2 の判定器（位置の入れ替え） |
| §56 | アンカーは表の列どおり | 全段階 | — | 位置の摂動プローブ（`-- error:` 行の直後に空行）を全コードに広げる |
| §54 | `raise` は `message` を stderr に出して終了コード 1。Option は剥がさない | 実行 | 受理 | `tests/accept/create_user/main.hd` `division_error.hd` |
| §54 | List は `[1, 2]`、入れ子の String は literal 形 | 実行 | 受理 | R9-5 の印字プローブ |
| §54 | 最上位の String は生、Void は無印字 | 実行 | 受理 | `tests/accept/display/main.hd` `void.hd` |
| §54 | Float は指数なしの最短往復 | 実行 | 受理 | R9-5 の印字プローブ（`3.0` `0.1` `-0.0` と極端な値） |
| §31 | E515 の 4 条件 | 冗長 | E515 | `tests/reject/E515/main.hd`、R9-4 の判定器（条件を 1 つずつ外す 4 プローブ） |
| §32 | 引数を持たない `def main \| true => 1 \| 0` は第31節の pattern 腕として読まれる | 構文 | 受理 → E206 | R13 の判定器 |
| 付録C | `...` の後は、式では任意の式、パターンでは名前か `_` | 構文 | E100 | R12 の判定器 |
| 付録C | 要素を伴わない `[...xs]` は文法に無い | 構文 | E100 | R12 の判定器 |
| §5 | コメントは `//` のみ。`--` と `#` は E104 | 字句 | E104 | `tests/reject/E104/main.hd`、R11 の判定器 |
| §22 | `[a, ...rest]` は `Cons` の糖衣。網羅性は展開後で判定する | 網羅性 | E402 / E403 / E405 | R12 の判定器 |
| §22 | 利用者のコードの `Nil` `Cons` は式で E510、パターンで E516 | 冗長 | E510 / E516 | `tests/reject/E510/main.hd` `E516.hd`、R12 の判定器 |
| §49 | `Nil` `Cons` は名前空間に残る（E204 ではなく E510 / E516 になる） | 名前解決 | — | R12 の判定器 |
| §32 | 名前あり + `\|` は条件列であり pattern にならない | 構文 | — | R13 の判定器（`Some(y)` が E204） |
| §32 | E517 は本体が条件列ちょうどのとき。arity と注釈によらず、継続を挟めば対象外 | 冗長 | E517 | `tests/reject/E517/main.hd`、R13 の判定器 |
| 付録C | 見出しより前に置けるのは本体付きの型（入口は `def` も） | 構文 | E100 | R17 の判定器（17-b(4)）。R16 は入口 1 ファイルなので外の `def` は常に適法 |
| 付録C | 節は入れ子にならず、`end` は無い。旧形は E100 | 構文 | E100 | EBNF 生成器、R16 の判定器 |
| §50 | プログラムは入口のディレクトリの `.hd` `.hdi` 全部。`.` で始まる名前は読まない（渡した入口は読む） | 名前解決 | E205 / E513 / 受理 | R17 の判定器（同名の節、未参照の節）、`verify-r53.sh` |
| §50 | `.hdi` に `implement` は置けない。入口以外の外に `def` は置けない | 構文 | E100 | R17 の判定器 |
| §12 | `fn` は括弧で閉じ、腕形式も `)` で終わる | 構文 | E100 / 受理 | R18 の判定器（18-b(2)(3)） |
| §16 | 型引数は後置。1 つの型引数を括弧で囲む形は無い | 構文 | E100 | R18 の判定器（18-b(2)） |
| §16 | 宣言した型変数が右辺に現れない型定義 | 冗長 | E518 | `tests/reject/E518/main.hd`、R19 の判定器 |
| §15.1 | 診断の型は §16・§20・§41 の正準形。型変数は 1 つの診断で 1 つの対応表を使い `A` `B` … と現れる順。E305 は適用の引数なら `expected <仮引数>, found <実引数>`、それ以外は `cannot unify`。E306 は `cannot unify` | 型 | — | `scripts/verify-r56.sh`（E305 / E306 / 改名）、`scripts/verify-r33.sh`（33-f2 / 33dist） |
| §24 | Bool の分岐は Boolean branch に限る | 冗長 | E520 | `tests/reject/E520/main.hd`、R18 の判定器（18-b(4)） |
| §39 | `_` 引数は契約 member の実装と `fn` に限る | 冗長 | E519 | `tests/reject/E519/main.hd`、R19 の判定器（19-b(2)） |
| §46 | 契約の値 member は `def name: 型` | 構文 / 型 | E100 / E602 | R19 の判定器（19-b(1)） |
| §46 | 契約の中の `X.T` は実装がちょうど 1 つのときだけ定まる | 型 | E603 | `tests/reject/E603/main.hd`、R19 の判定器（19-b(3)） |
| §47 | 型引数付き抽象型の実体化は型引数の数が契約と一致 | 型 | E602 | R19 の判定器（19-b(1)） |
| §20 | 関数型は `Fn(...)` で括弧必須 | 構文 | E100 | R18 の判定器（18-b(2)） |
| §50 | 入口ファイルは複数置け、外の `def` はファイル局所 | 名前解決 / 冗長 | 受理 / E204 / E513 | `tests/accept/two_entries/`、R20 の判定器 |
| §50 | 渡したファイルに `main` が無いとき、`check` はライブラリ、`build` は E206 | 名前解決 | E206 / 受理 | `verify-r32.sh`、`tests/library/` |
| §51 | 自分のディレクトリは無修飾、他は根からの絶対パス | 名前解決 | 受理 / E204 | `tests/accept/namespaces/`、R22 の判定器（22-b(2)） |
| §51 | 名前空間はディレクトリごと。根との同名は E205 | 名前解決 | E205 | R22 の判定器（22-b(3)） |
| §48 | functor の本文は引数の契約だけで 1 回検査 | 型 | E602 / 受理 | `tests/accept/functor/`、R21 の判定器（21-b(1)(2)） |
| §48 | 同じ引数列の再適用 | 型 | E604 | `tests/reject/E604/main.hd`、R21 の判定器（21-b(3)） |
| §48 | 未適用の functor と未参照の引数 | 冗長 | E513 / E521 | `tests/reject/E521/main.hd`、R21 の判定器（21-b(4)） |
| §10 | `main` の型に `shared` で解けない抽象型を含めない | 単相化 | E705 | `tests/reject/E705/main.hd`、`verify-r33.sh` |
| §10 | `main` に残ってよい効果は prelude の効果 | 単相化 | E706 | `tests/reject/E706/main.hd`、`verify-r37.sh` |
| §36 | 効果を持てるのは分岐の条件以外。scrutinee・引数・被演算子・record field は許す | 型 | E308 / 受理 | `tests/reject/E308/main.hd`、`tests/accept/effect_position/` |
| §20 | 効果行の省略は推論、`Pure` は空、書いた行はちょうどそれだけ。省略した行は再帰定義でも推論され、暗黙の効果変数として全称量化される | 型 | E305 / 受理 | `tests/accept/effect_row/`、R24 の判定器、`verify-r61.sh`（61-a〜61-d） |
| §53 | `again` を経由しない呼び出しだけスタックを消費しない | 実行 | 受理 | `tests/accept/again_loop/main.hd`、R26 の判定器 |
| §37 | 操作は値の名前空間に無修飾で入り、書けるのはそのディレクトリだけ | 名前解決 | 受理 / E204 / E205 | `tests/accept/effect_forms/`、R23 の判定器 |
| §38 | `return` の腕を書かなければ恒等。`with H -> rest` は継続の形 | 型 / 実行 | 受理 | `tests/accept/handler_forms/` `to_option/`、R26 の判定器 |
| §52 | `library` は 1 ディレクトリに 1 つ。2 つ目は E205 | 名前解決 | E205 | `verify-r32.sh` |
| §52 | `library` を宣言したディレクトリの `main` は E206 | 名前解決 | E206 | `tests/reject/E206/main.hd`、`verify-r32.sh` |
| §52 | ライブラリは独立した根で、埋め込みで内部の絶対パスと外の `def` が変わらない | 名前解決 | 受理 | `tests/library/`、`verify-r32.sh` |
| §51 | ライブラリの中から親と兄弟の名前は見えない | 名前解決 | E204 | `verify-r32.sh`（兄弟参照）、`tests/reject/E204/main.hd`（親の無修飾参照） |
| §47 | `shared` は契約の抽象型の実体化にだけ前置できる | 構文 / 名前解決 | E100 / E204 | `verify-r33.sh` |
| §47 | `shared` で解ける実体化は外と同一の型。印字は解けた型 | 型 / 単相化 | 受理 / E705 | `verify-r33.sh` |
| §48 | `shared type T = M.T` は適用結果の `T` を引数の `T` と同一にする | 型 | 受理 | `verify-r33.sh`（24.7 の探針） |
| §19 | `Bytes` と `Array` は組み込み型。利用者の同名宣言は E205 | 名前解決 | E205 | `verify-r34.sh` `verify-r35.sh` |
| §55 | Bytes の長さ・変換・索引・of_list / to_list | 型 / 実行 | 受理 / E305 | `verify-r34.sh` |
| §55 | String の slice / 索引 / split / starts_with / contains。範囲外と空セパレータは実行時 `Error` | 実行 | 受理 | `verify-r34.sh` |
| §55 | Array の get / set / of_list / to_list。get と set の範囲外は実行時 `Error`、set は新配列 | 実行 | 受理 | `verify-r35.sh`（3 万要素） |
| §55 | `Int.parse` の不正と桁溢れは実行時 `Error` | 実行 | 受理 | `verify-r36.sh` |
| §55 | `Env` と `Io` の操作（無修飾、`{Error}`、handler 無しの既定実装） | 型 / 実行 | 受理 | `verify-r37.sh` |
| §55 | Env と Io は handler で差し替えられる（操作の全実装を要求） | 型 | E605 / 受理 | `verify-r37.sh` |
| §55 | 組み込みの失敗の `message` の綴り | 実行 | 受理 | R34〜R37 の判定器（`// expect-error:`） |
| §55 | `Io.log` は String と改行 1 つを stdout へ書き、失敗は `Error`（`IoFailure`） | 型 / 実行 | 受理 | `verify-r43.sh` |
| §55 | `String.join` は区切りを要素の間に置き、`String.trim` は ASCII の空白 6 種を両端から除く | 実行 | 受理 | `verify-r43.sh` |
| §55 | `Int.abs` は i64 の最小値でその値自身を返し、`min` `max` は大小を返す | 実行 | 受理 | `verify-r43.sh` |
| §55 | `List.head` / `nth` / `take` / `drop` は全域で、範囲外は `None` か `[]` | 型 / 実行 | 受理 | `verify-r43.sh` |
| §39 | ライブラリでは top-level を公開面として根にする（functor と契約を含む） | 冗長 | 受理 | `verify-r32.sh` |
| §10 | `main` に残せる効果に prelude の `Env` `Io` を足す | 単相化 | 受理 | `verify-r37.sh` |
| §54 | Bytes は `Bytes.of_list [72, 105]`、Array は `Array.of_list [1, 2]` の形で印字 | 実行 | 受理 | `verify-r34.sh` `verify-r35.sh` の印字プローブ |
| §21 | Bytes はバイト列、Array は要素型の構造比較 | 単相化 | E703 / E704 / 受理 | `verify-r34.sh` `verify-r35.sh` |
| §26 | scrutinee の列と pattern の列は同数（E125）。列が 2 つ以上の行は一部の列だけが `_` か変数なら受理、全列なら E512 / E503 | 構文 / 冗長 | E125 / E512 / E503 / 受理 | `tests/reject/E125/main.hd`、`verify-r45.sh`（45-c） |
| §26.1 | 腕の条件列は最初の fallback で終わり、その後の `\|` は外側の腕。条件が 1 本の腕の条件列は E520 の対象外。fallback の無い腕の条件列は E401 で腕の `if` を指す | 構文 / 網羅性 / 冗長 | E401 / 受理 | `tests/accept/arm_conditions/main.hd`、`verify-r45.sh`（45-b、45-d） |
| §28 | 列が 2 つ以上の網羅性は列の直積で判定する | 網羅性 | E402 / E403 | `verify-r45.sh`（45-a） |
| §30 | 腕の本体に置ける分岐は条件列だけで、条件列の腕には分岐を置けない | 構文 | E101 / 受理 | `verify-r45.sh`（45-b (4)）、EBNF 生成器 |
| §57 | `check --fix` は報告したアンカーの局所的な書き換えを当ててやり直す。範囲に `//` を含む書き換えは出さない。既定の `check` / `build` はソースを書き換えない | CLI | — | `verify-r46.sh`（E510 / E516 / E517）、`verify-r47.sh`（E515 / E501）、`verify-r57.sh`（E507 / E508）、`crates/hado/tests/r46_fixes.rs` `r47_fixes.rs` |
| §57 | E507 の正準形は scrutinee。第31節の形は `def f value => value` / `fn(value => value)`。fallback を持つ形は対象外 | 冗長 | E507 | `tests/reject/E507/main.hd`、`verify-r57.sh`（57-a / 57-b / 57-e） |
| §57 | E508 は関数の本体ちょうどの分岐だけ `let <内側>? = <scrutinee> -> <本体>` にする。束縛名が本体で使われ、本体が `value` を参照しない形に限る | 冗長 | E508 | `tests/reject/E508/main.hd`、`verify-r57.sh`（57-c / 57-d / 57-e） |
| §58 | `hado run` は一時ディレクトリで作って実行し、子プロセスの終了コードを返す。`build -o` は出力先を明示する。`build` と `run` の失敗にも `canonical form:` を付ける | CLI | — | `verify-r48.sh`、`crates/hado/tests/run_cli.rs` |
| §58.1 | `hado fmt` は入力根のソースの局所的な書き換えを診断を待たずに一括で当て、`--check` は書かずに門番になる。`build --fix` は作る前に同じ整形をする | CLI | — | `verify-r52.sh` |
| §58.2 | `hado test` は根の直下の `*_test.hd` を順に走らせ、終了コードで合否を決めて 1 本ごととまとめを出す。落ちても残りを走らせる | CLI | — | `verify-r54.sh` |
| §58.3 | `--print-type=<名前>` はその定義の型を §16・§20・§41 の正準形で印字する。変数は `A` `B` … と改名し、開いた行は `| A` を足す。member は契約が宣言する型。無い名前は終了 1 | CLI | — | `scripts/verify-r55.sh` |
| §56 | E205 の関連位置は同じ名前の 1 回目の宣言。prelude と組み込み型の名前との衝突では出さない | 名前解決 | E205 | `verify-r58.sh`（58-a / 58-b / 58-c / 58-d / 58-g） |
| §56 | E602 の関連位置は契約の member の宣言。`.hdi` の契約でもファイルをまたいで指す。functor と循環では出さない | 型 | E602 | `verify-r58.sh`（58-e / 58-f / 58-g） |
| §37 | 効果を使うのは、名前が型や効果行に現れるか、操作を参照するか、handler を宣言するとき。操作の型に出る名前も道連れに使われる | 冗長 | E513 | `verify-r59.sh`（59-a / 59-b / 59-c / 59-d / 59-f）、`tests/reject/E522` |
| §38 | handler を使うのは `with` に名前を書くとき。宣言された handler は腕が辿る先と同じく根である | 冗長 | E513 | `verify-r59.sh`（59-b / 59-d） |
| §15.1 | E305 の 2 形。役割が確定する箇所は expected/found と、書かれている期待の位置 | 型 | E305 | `verify-r60.sh`（60-a / 60-b / 60-c / 60-d / 60-e）、`verify-r56.sh`（56-a / 56-b / 56-c / 56-d） |
| §56 | E305 の関連位置のラベルは `expected here` | 型 | E305 | `verify-r60.sh`（60-a / 60-b）、`verify-r56.sh`（56-d） |
| §29 | catch-all の `_` の腕は fallback の正準形 `\| body` に書き換える | 冗長 | E503 | `verify-r62.sh`（62-a / 62-b / 62-d） |
| §24 | Bool リテラルの腕の分岐は Boolean 分岐に書き換える | 冗長 | E520 | `verify-r63.sh`（63-a / 63-b / 63-c / 63-d） |
| §58.1 | fmt は行末の空白を消し、ファイルの終わりに改行を 1 つ置く | 整形 | 受理 | `verify-r64.sh`（64-a / 64-b / 64-c） |
| §58.4 | 診断は 1 行 1 つの JSON でも出せる | 道具 | 受理 | `verify-r65.sh`（65-a〜65-e） |
| §26 | E512 の変数パターンの腕は fallback か scrutinee に書き換わる | 冗長 | 受理 | `verify-r68.sh`（68-a〜68-d） |
| §42 | E502 の同じ結果の腕と fallback は共通の本体に書き換わる | 冗長 | 受理 | `verify-r69.sh`（69-a〜69-d） |
| §58.4 | 診断を 1 行 1 件の JSON で出し、Hado の道具がコードごとに数える | 道具 | 受理 | verify-r70.sh（70-a〜70-d） |
| §35 | `let _? = e ->` の破棄は `do e ->` に書き換わる | 冗長 | 受理 | verify-r71.sh（71-a〜71-d） |
| §40 | `let _ = e ->`（純粋値）の破棄は束縛ごと削除する | 冗長 | 受理 | verify-r72.sh（72-a〜72-d） |
| §41 | `let x = v -> x` は値に畳まれる | 冗長 | 受理 | verify-r73.sh（73-a〜73-d） |
| §26 | 同じ変数とリテラルの等値比較の条件列はリテラルパターンに畳まれる | 冗長 | 受理 | verify-r74.sh（74-a〜74-d） |
| §40 | 腕の未使用束縛は `_` に改名され、参照される束縛は残る | 冗長 | 受理 | verify-r75.sh（75-a〜75-d） |
| §54 | 失敗は raise が main まで届くことで表し、stderr と終了コード 1 を返す | 実行 | 失敗 | verify-r76.sh（76-a〜76-d） |
| §50, §54 | 試し書きはドット始まりで隔離でき、赤は正準形つきで直り、実行・テスト・型表示まで 1 本で回る | 体験 | 成功 | verify-r77.sh（77-a〜77-e） |
| §5, §19, §54 | 型名は Void。値は `()`。入れ子は `()`、最上位は無印字 | 型 / 実行 | 受理 | verify-r81.sh（81-a / 81-c） |
