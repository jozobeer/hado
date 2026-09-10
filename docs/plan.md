# 開発計画 v0.1 → v0.2

対象は `docs/spec.md`（以下「仕様」）。仕様の節番号は、M1 で改訂する前の現行版を指す。

## 1. 意図

- shin 言語 v0.1 を、Rust で書いたコンパイラとして実装する（明示）。
- 目的は、仕様の主張「意味を変えずに削れるコードはコンパイルできない」を、実行ファイルを生成するコンパイラで実証すること。v0.2 以降の言語設計は、この実装で得た手触りを根拠に決める（推定）。
- 利用者は言語設計者本人。他者への配布・移植性は v0.1 の条件に含めない（推定）。

## 2. 決定記録

対話（2026-09-06）で確定した事項。理由は 5 節に置く。

### 2.1 成果物と実装方式

| 項目 | 決定 |
|---|---|
| 成果物 | 検査器と実行ファイル生成を持つコンパイラ。CLI 名 `shin` |
| バックエンド | Cranelift 0.135（純 Rust）。`cranelift-codegen` `-frontend` `-module` `-object` `-native` を Cargo.lock で固定 |
| 外部ツール | リンクに `cc`、Boehm GC に `libgc-dev` |
| 入口 | `def main = expression`。emit が system_v の `shin_entry` を生成し、`main` を呼んで値を stdout に印字し終了コード 0 を返す。値が `Error(e)` なら stderr に印字し終了コード 1。C の `main()` は `GC_INIT()` と `shin_entry()` の呼び出しのみ。言語内に副作用はない |
| 多相の実現 | 単相化。実行時型情報を持たない。多相再帰はコンパイルエラー |
| メモリ管理 | Boehm GC（`libgc-dev`）。ランタイムの確保関数は一つに絞る。参照カウントは v0.2 |
| 標準ライブラリ | prelude を shin 自身で書き、全プログラムに暗黙に連結する。組み込みは Int / Float の算術と比較、String の長さ・連結・比較、Char と String の相互変換、スカラと String の印字に限る。構造比較 `==` `compare` と Fn を含まない全型の印字は M5 の派生パスが型ごとに合成する |
| prelude の関数集合 | List: map filter fold length reverse append sort。Option: map unwrap_or is_some ok_or。Result: map map_error is_ok。String: length concat。Int / Float: to_string。sort は比較関数を引数に取る merge sort |
| Int | `i64`。加減乗はオーバーフローで wrap。`/` `%` は `Result<Int, DivisionError>` を返し、除数 0 で `Error(DivisionByZero)`。Float の `/` は IEEE 754 |
| 拡張子 | `.shin` |
| 関数値の表現 | 「関数ポインタ + 環境」のクロージャに統一。名前付き関数の環境は null |
| 呼び出し規約 | shin の関数は全て Cranelift の `tail` 規約。末尾位置の shin 関数呼び出しは `return_call`。ランタイム extern は system_v なので `return_call` できず（verifier が caller と同じ規約を要求する）、末尾位置でも `call` + `return`。C から呼ぶ入口だけ system_v で、通常の `call` で `tail` 側に入る。`preserve_frame_pointers=true` が必須 |
| top-level 値定義 | `def pi = 3.14159` は 0 引数関数として扱い、参照ごとに呼ぶ。`main` も同じ |
| 性能 | v0.1 の条件に含めない。`opt_level=none`、inliner は使わない |

### 2.2 言語意味論（M1 で仕様に書き込む）

| 項目 | 決定 |
|---|---|
| 関数適用 | n 引数。定義の引数数と呼び出しの引数数は一致必須。部分適用なし |
| パイプ | `x \|> f a b` は `f x a b`。標準ライブラリの引数順は data-first |
| 関数値 | `def` で定義した関数名を引数なしで参照すると `Fn<...>` 型の値 |
| 呼び出し構文 | 関数は並置 `f x y`、constructor は括弧 `Some(x)`。大文字始まりが constructor |
| 末尾呼び出し | 末尾位置の関数呼び出し（自己再帰・相互再帰・関数値経由を含む）はスタックを消費しない。`def rec` が唯一のループなので仕様で保証する |
| 冗長の対象外 | 引数をそのまま別の関数に渡すだけの `def`（eta 展開）は冗長としない。prelude が組み込みを包む形がこれに当たる |
| 冗長・到達不能の判定 | 構文的。束縛変数の名前替えと括弧を正規化した後の構造一致。条件列の到達不能は、先行条件と構造一致する場合か、先行条件が `true` の場合に限る |
| module | `module X : SIG = struct ... end` を v0.1 に入れる。signature 注釈は必須。struct は sig に載らない定義を持てる。sig の `type T` は外から不透明。struct 内で sig からも内部からも参照されない `def` はエラー |
| record | 構築は `{ name = "a"; age = 3 }`。型は field 集合から一意に決まる nominal 型で、曖昧なら注釈を要求。パターンは `{ name }`（同名束縛）と `{ name = n }`（別名）。書かない field は無視。field 更新構文は v0.2 |
| tuple | 持たない。`Fn<(Int, Int), Int>` の `( )` は引数列 |
| 演算子 | 算術 `+ - * / %`、比較 `< <= > >= == !=`、論理 `&& \|\| not`。算術と順序比較 `< <= > >=` は Int / Float 両用の組み込み多相で、単相化時に確定し、未確定なら Int（`/` `%` の戻り型は 2.3）。`==` `!=` は Fn を含まない全型の構造比較で、M5 の派生パスが型ごとに合成する。Fn を含む型への `==` `!=` `compare` は単相化時に判明するので `E7xx`。`&&` `\|\|` は短絡評価で、左辺が結果を決めるとき右辺を評価しない。文字列連結演算子は置かない。優先順位は高い方から 適用、単項 `-`、`* / %`、`+ -`、比較、`&&`、`\|\|`、`\|>`。比較は非結合 |
| リテラル | Int は 10 進。Float は小数点の両側に数字必須。Char は `'a'`。String は `"..."`、エスケープは `\n \t \\ \"`。Bool は `true` `false`。Unit は `()` |
| Unit payload | `Ok()` が canonical。`Ok(())` は拒否 |
| コメント | `--` 行コメントのみ |
| 変数パターン | 最上位の `\| x => ...` は禁止。入れ子内は許可 |
| リテラルパターン | 持つ。Int / Float / Char / String / Bool と入れ子で使える。Bool 以外は fallback 必須。条件列の全条件が「同一変数 `==` リテラル」ならパターン分岐で書けるため拒否 |
| 条件列の判別 | パーサは改行を見ない。`if` の直後が `\|` なら条件列。仕様 1289 行の改行要求は削除 |
| 名前空間 | 型と constructor は別。仕様 965 行の `Error` 型はユーザー定義 |
| List リテラル | `[a, b, c]` は `Cons` 連鎖の糖衣。空は `Nil`。末尾がリテラルの `Cons(a, Nil)` `Cons(a, [b])` は拒否 |
| 再帰型 | 型定義の閉路（自己再帰と相互再帰）の中では、型引数は宣言の型引数と同一でなければならない。`type Nest<T> = Leaf(T) \| Node(Nest<List<T>>)` は `E3xx`。派生パスの実体化を有限にするため |
| 相互再帰 | top-level は定義順に依存しない。呼び出しグラフの閉路に含まれる関数は全て `rec`。閉路に値定義が含まれるとエラー（値定義は `rec` を持てない、仕様 §4） |
| prelude から組み込みへの到達 | M5 の extern 表の各項目に shin 側の名前（`__string_length` 等）と型を持たせる。`__` 名は `stdlib/` のファイルでのみ解決し、ユーザーコードでは `E2xx`。呼び出し位置でしか参照できず、値化は `E2xx`。prelude は `def length s => __string_length s` で包む |

### 2.3 M1 で確定させる推定既定

対話で扱わなかった点。M1 の着手時に本表の既定で仕様に書き、ユーザーの確認を得る。

| 項目 | 既定（推定） |
|---|---|
| 名前空間の全体 | 型・module・constructor・値の 4 つ。`Option<T>`（型）と `Option.map`（module）は共存する。prelude の constructor `Some` `None` `Ok` `Error` `Nil` `Cons` は修飾なしで参照できる |
| 言語が知る prelude の型 | `Option` `Result` `List` `DivisionError` はコンパイラが名前で解決する（`/` の戻り型、`let?` の対象、List リテラルの展開先） |
| `main` の型 | 単相で、Fn を含まない型でなければならない。`def main = Nil` は型変数が残るため、`def main = inc` は印字できないため、どちらも `E7xx` |
| 文字 | Char は Unicode スカラ値。String は不変の UTF-8。`String.length` はスカラ値の個数 |
| top-level の未使用 `def` | エラー（仕様 §27 の適用範囲を top-level にも広げる）。`main` は例外 |
| スカラの機械語型 | Int は i64、Float は f64、Bool は i8、Char は i32、Unit は i64 の 0。Cranelift に i1 はない |
| ADT のレイアウト | constructor が全て payload なしの型は素の整数。payload を一つでも持つ型は全 constructor を一律に GC ヒープの box にする。一つの型の中で即値と pointer を混在させない |
| `/` `%` の戻り型 | Int では `Result<Int, DivisionError>`。Float の `/` は `Float`。Float に `%` は置かない |
| 除算の端 | `i64::MIN / -1` は wrap して `Ok(i64::MIN)`、`i64::MIN % -1` は `Ok(0)`。除数 0 と併せて `sdiv` `srem` の前に分岐で除く（Cranelift の `sdiv` は両方でトラップする） |
| クロージャの呼び出し規約 | 全ての shin 関数が環境を第 1 引数に取る一律規約。名前付き関数は環境を無視する。単相の `Fn` 型ごとに `tail` 規約の signature が一つに定まる（機械語型の列で決まる）ので、関数値経由の末尾呼び出しを `return_call_indirect` で表せる。代替案は「名前付き関数は直接規約、値化時に thunk を生成」 |
| DivisionError の名前 | より具体的な名前（`DivisionError` 等）を候補に、M1 で決める |
| prelude の module 構成 | prelude の各 module も `module X : SIG = struct` の形で書く（2.2 の規則に従う） |
| エラーコード | `E1xx` 字句・構文、`E2xx` 名前解決、`E3xx` 型、`E4xx` 網羅性・到達可能性、`E5xx` canonical・冗長、`E6xx` module の型、`E7xx` 単相化。各コードに短い名前を付け、拒否コーパスはコードで照合する（7 節 Q31 で百の位を段階に一致させた） |

## 3. マイルストーン

完了条件の（確定）は対話または実測から導いたもの、（推定）は本計画で置いたもの。

リポジトリ構成:

```text
Cargo.toml            ワークスペース
crates/shin/          コンパイラ本体と CLI（lexer, parser, types, check, mono, repr, emit）
runtime/              C ランタイム（C の main、GC 確保・String primitive・スカラ印字）。cc crate でビルドして静的リンク
stdlib/               prelude（.shin）
tests/accept/         受理コーパス
tests/reject/         拒否コーパス
docs/            spec.md、plan.md
```

### M0 環境と歩く骨格

1. Boehm GC を導入する。C ツールチェーン（`cc`）は導入済みであることを確認する。

   ```sh
   sudo apt install libgc-dev
   ```

2. ワークスペースを作る。`crates/shin` の依存は `cranelift-codegen` `cranelift-frontend` `cranelift-module` `cranelift-object` `cranelift-native` の `"0.135"`、build 依存は `cc = "1"`。
3. `runtime/` に C の `main()`（`GC_INIT()` と `shin_entry()` の呼び出し）、`shin_alloc`（`GC_malloc` の包み）、Int 印字を書き、`cc` で静的ライブラリにする。
4. `def main = 1 + 2` を lexer → parser → 型 → Cranelift → オブジェクト → `cc` リンクの順で通し、実行ファイルが `3` を印字する。ISA flags は `is_pic=true` `preserve_frame_pointers=true` `opt_level=none`。`enable_verifier` は既定の true のまま。
5. `crates/shin/tests/` に手組みの CLIF テストを置く。`tail` 規約と `return_call` で引数数の異なる相互再帰を組み、system_v の入口から 1 億深さを走らせて完走を確認する。shin の構文はまだ無いので、M0 ではコンパイラを通さず Cranelift の API を直接呼ぶ。実行はオブジェクト出力 → `cc` リンク → サブプロセス。

完了条件:

- `cargo build` が成功する（確定）。
- `cargo test` に、`def main = 1 + 2` を AOT で実行ファイルにし、サブプロセスで走らせて stdout が `3` と一致するテストがあり、通る（確定）。
- 手組み CLIF の相互再帰 1 億深さがスタックを溢れずに完走する（確定）。

### M1 仕様の穴埋めとコーパス

1. 2.2 と 2.3 の内容を `docs/spec.md` に書き込む。既存の節番号は変えず、新設の節は §42 の後に追加する。コーパスの `-- spec:` はこの番号を指す。

   | 対象 | 内容 |
   |---|---|
   | §5〜6 | `main` |
   | §8、§37 | `Fn` の n 引数 |
   | §10〜11 | 再帰型の型引数の制約 |
   | §12 | record の構築とパターン |
   | §13〜19 | リテラルパターン、変数パターン |
   | §25 | パイプの第 1 引数 |
   | §32 | 「静的に到達不能」の定義 |
   | §34〜35 | struct |
   | §36 | Int、Char、String、除算の端 |
   | §38 | struct を除外一覧から外す。operator overloading の禁止は維持し、組み込み算術演算子はユーザーが定義できない固定集合への型指向解決であると注記する |
   | §41 | 文法 |
   | 新設 | リテラルとコメント、演算子と優先順位と短絡評価、構造比較の対象型、List リテラル、名前空間、末尾呼び出しの保証、prelude、エラーコード |

2. コーパスを書く。ファイル冒頭に根拠と期待値をコメントで置く。

   ```text
   -- spec: §29
   -- error: E501
   ```

   ```text
   -- spec: §39
   -- expect: guest
   ```

   受理コーパスのうち prelude の関数を使うものには `-- needs: prelude` を付ける。
3. コーパスを列挙して実行する統合テストを `crates/shin/tests/corpus.rs` に置く。受理は「コンパイル成功、実行して stdout 一致」、拒否は「指定コードで失敗」。

完了条件:

- 仕様の「禁止」「コンパイルエラー」と書かれた例が全て拒否コーパスに 1 ファイルずつある（確定）。
- §39 の例が全て受理コーパスにある（確定）。
- 2.2 の各項目に、受理と拒否のペアが少なくとも 1 組ある（推定）。
- `&&` `||` の評価されない側に停止しない再帰を置いた受理コーパスがある（確定）。
- `cargo test --test corpus` がコーパス全件を列挙し、件数を出力する。この時点で通る件数は問わない（確定）。

### M2 構文

1. lexer と parser を書く。全ノードに位置情報を持たせる。

完了条件:

- 受理コーパス全件が構文解析を通る（確定）。
- 拒否コーパスのうち `E1xx` を期待するものが指定コードで落ちる（確定）。
- 拒否コーパスの残りが構文解析を通る（確定）。

### M3 型

1. 名前解決。4 名前空間、module の階層、sig による隠蔽、`__` 名の `stdlib/` 限定。
2. HM 型推論。`def` 単位の一般化、出現検査、ADT / record / generics、再帰型の型引数制約。
3. module の signature matching と抽象型。
4. 組み込み多相（算術・順序比較・`==` `!=` `compare`）。
5. prelude のうち型定義のみ（`Option` `Result` `List` `DivisionError`）を `stdlib/` に置いて連結する。関数は M7。

完了条件:

- `shin check` が、`-- needs: prelude` を持たない受理コーパス全件を通す（確定）。
- 拒否コーパスの `E2xx` `E3xx` `E6xx` が指定コードで落ちる（確定）。
- unify、generalize、出現検査、signature matching の Rust 単体テストがある（推定）。

### M4 検査器

1. 網羅性（`E4xx`）: 構造に基づく網羅性、到達不能 branch、重複パターン、fallback の位置。
2. canonical（`E5xx`）: identity binding、未使用 binding、純粋値の破棄、`let? _`、同一 branch、再包装、単純再伝播、`Ok(())`、`Cons` 末尾リテラル、条件列からリテラルパターンへ、最上位変数パターン、top-level と struct の未使用 `def`、`rec` の閉路規則（値定義を含む閉路はエラー）。
3. 式の正規化と構造比較を一つの module にまとめ、§24 §30 §32 §33 から共用する。

完了条件:

- 拒否コーパスの `E4xx` `E5xx` が指定コードで落ちる（確定）。
- `-- needs: prelude` を持たない受理コーパス全件が検査を通る（確定）。
- 各エラーが「どの branch や binding と同一か」を位置付きで出す（推定）。

### M5 単相化・クロージャ変換・派生

1. 多相 `def` を使用型ごとに実体化する。多相再帰を検出してエラーにする。
2. `fn` と関数値をクロージャに変換する。`MakeClosure` `CallDirect` `CallClosure` を別ノードにし、各関数は固定 arity の signature を持つ。
3. 派生パス。型ごとに `eq_T` `compare_T` `print_T` を通常の中間表現の関数として合成する。
4. 型付き中間表現を定義する。性質は次の 5 つ。単相で型変数を持たない。値はスカラーか参照のどちらかで、集約は全て参照。データ操作は構造的（`Construct` `Project` constructor の switch `RecordGet`）で、バイトサイズとオフセットを持たない。`Call` と `TailCall` を区別し、`TailCall` の callee は shin 関数（直接か closure 経由）に限る。ランタイム外部関数は（名前、機械語型の引数列と戻り、確保し得るか、prelude から呼ぶものは shin 側の名前と型）の表で一箇所に宣言する。
5. `shin dump-mono` で出力する。

完了条件:

- `-- needs: prelude` を持たない受理コーパス全件で `shin dump-mono` が成功し、出力に型変数・バイトサイズ・オフセットが現れない（確定）。
- 派生関数 `eq_T` `compare_T` `print_T` が `dump-mono` に現れる（推定）。
- `dump-mono` の `TailCall` に extern が現れない（確定）。
- 拒否コーパス全件が指定コードで落ちる（確定）。`E7xx` の例は多相再帰、`main` が単相でない、`main` が Fn を含む、Fn を含む型への `==` `compare`。

### M6 コード生成

1. `repr`。M5 の型から機械語型、box 判定、フィールドオフセットを決める。backend 中立で単体テスト可能にする。
2. `emit`。中間表現と `repr` から CLIF を生成する。関数は `tail` 規約、`TailCall` は `return_call`（関数値経由は `return_call_indirect`）。入口 `shin_entry` は system_v で emit が生成し、`main` を `call` して結果を `print_T` に渡し（`Result` なら `Ok` / `Error` で stdout / stderr を分ける）、終了コードを返す。
3. ランタイムに String primitive とスカラ印字を足す。
4. オブジェクト出力とランタイム静的ライブラリを `-lgc` と共に `cc` でリンクする工程を `shin build x.shin` にまとめる。

完了条件:

- `-- needs: prelude` を持たない受理コーパス全件が、実行ファイルとして期待 stdout を返す（確定）。
- `main` が `Error(e)` を返す受理コーパスで、stderr に `e` が印字され終了コードが 1（確定）。
- `define_function` が受理コーパス全件の全関数で verifier エラーなく通る（確定）。
- 末尾再帰 1 億深さの受理コーパスがスタックを溢れずに完走する。自己再帰、引数数の異なる相互再帰、捕捉環境を持つ関数値経由の 3 経路（確定）。

### M7 prelude

1. 2.1 の関数集合を `stdlib/` に shin で書く。各 module に sig を付ける。組み込みの wrapper は `__` 名を呼ぶ。
2. 全プログラムへの暗黙連結を実装する。

完了条件:

- 受理コーパス全件が期待 stdout を返す（確定）。
- prelude 自体が M4 の検査を通る（確定）。
- `cargo test` 全件が通る（確定）。

## 4. 実走したコマンドと未検証

| 内容 | 状態 | 結果 |
|---|---|---|
| Cranelift 0.135 のフレッシュビルド | 実走 | 20.7 秒、crate 57、システム依存なし |
| オブジェクト出力（`is_pic=true`）を C ランタイムと `-lgc` で `cc` リンク | 実走 | `1 + 2` の実行ファイルが `3` を印字 |
| `tail` 規約 + `return_call`、2 引数と 9 引数の相互再帰 | 実走 | system_v の入口から 1 億深さを完走。`preserve_frame_pointers=false` では emit が panic |
| Boehm GC 単独 | 実走 | `GC_malloc` 64 B × 200 万回でヒープ 128 KB に留まる |
| Cranelift API の変化 | 実走 | 0.135 で `finalize` が引数を取り、`icmp_imm` `iadd_imm` が非推奨 |
| `return_call` の verifier 規則 | ソース確認 | callee は caller と同じ規約かつ `tail`、戻り型も caller と一致（`verifier/mod.rs` 1717 行） |
| `opt_level` の既定と inliner | ソース確認 | 既定は `none`。inliner は `Context::inline` に `Inline` 実装を渡す library 形式で、cranelift-module 経由では自動適用されない |
| LLVM 22 + inkwell、テキスト LLVM IR | 実走 | 同じ probe（`1 + 2`、末尾再帰 1 億深さ）を通したうえで不採用 |

## 5. 判断の解説

インタプリタではなくコンパイラを v0.1 の成果物にした。検査器だけでは「動くプログラム」が一つもなく、型検査の正しさを実行結果で確かめる手段がないため。

バックエンドは Cranelift にした。ユーザーの選好である「ツールチェーンを cargo に閉じる」を満たす唯一の案で、システム LLVM の導入・環境変数・版追従が消え、外部依存は `cc` と libgc だけになる。比較した LLVM 22 + inkwell とテキスト LLVM IR も同じ probe を通しており、v0.1 の範囲で機能差はなかった。Boehm GC と単相化により、コード生成に残るのは算術・call・tail call・確保・load / store・switch・String literal の静的データ・closure 用の関数アドレスだけで、バックエンドは M5 の中間表現の後ろに閉じた葉になる。Cranelift の代償は、月次のメジャー更新（Cargo.lock で固定し、上げるのは意図したときだけ）、末尾呼び出しが `tail` 規約と `preserve_frame_pointers=true` を要求すること、フレッシュビルド 20 秒である。後で inkwell やテキスト IR に移る場合は `emit` の書き直しで済む。

性能を v0.1 の条件に含めない。目的は検査器の実証で、利用者は設計者本人である。`opt_level=none` は egraph 最適化を無効にするだけだが、CLIF の命令列に近い機械語が出るので emit の不具合を逆アセンブルで追いやすい。

関数適用を n 引数にした。パイプが第 1 引数を埋める規則である以上、部分適用が仕様の例を支える場面はない。残るのは表現力の追加だけで、それは「同じ意味に複数の表現を許さない」原則と衝突する。呼び出し規約も 1 定義 1 関数で済む。

多相を単相化で実現した。HM の rank-1 多相と単一コンパイル単位という条件下では常に可能で、実行時型情報が不要になる。ランタイムは確保・String primitive・スカラ印字だけになる。

構造比較と印字は M5 の派生パスで型ごとに合成する。ランタイムに置くと実行時型情報が要り、単相化と矛盾するため。レイアウトは `repr` 一箇所が決め、`emit` は消費するだけにする。中間表現にサイズやオフセットを入れないのは、バックエンドを交換しても `repr` を無傷で残すため。

メモリ管理を v0.1 では Boehm GC に委ねた。参照カウントの retain / release 挿入は use-after-free を生みやすく、検査器・コード生成と同時に立ち上げると原因の切り分けが難しいため。参照カウントは、この言語が mutation を持たず再帰も top-level の `def rec` に限るため参照が循環せず、最終形として適している。確保関数を一つに絞っておけば、v0.2 での差し替え範囲は明確である。

prelude を shin 自身で書く。`sig` `struct` `def rec` パターン分岐の最初の本格的な利用者になり、コンパイラの実地テストを兼ねる。prelude も canonical 検査を通るので、検査器の妥当性も同時に試される。

組み込みは `__` 名で包む。extern 宣言の構文を足すと真実源が M5 の表と stdlib の 2 か所になる。String / Int / Float を組み込み module にすると、`List.map String.length` のような値化のために M5 が wrapper を合成することになり、shin で書く wrapper と同じ物をコンパイラが持つだけになる。

top-level 値定義は 0 引数関数にした。一度だけ評価して global に置くには、単相化の実体ごとの global と初期化順が要る。

コーパスをコンパイラより先に書く。この言語では「何を落とすか」が仕様そのものであり、先に固定しないと検査器の完了条件が書けない。

工程順（構文 → 型 → 検査器 → 単相化 → コード生成）を機能ごとの縦割りより優先した。型検査器と検査器は unify や網羅性行列のように機能横断の設計を先に置かないと作り直しになる。ただし M0 で歩く骨格を先に通す。バックエンドとランタイムの結合は、失敗したときの影響が最も大きい外部要因であるため。

## 6. リスクと v0.2 への保留

リスク:

- Cranelift の API 変化。月次でメジャーが上がり、0.135 でも非推奨化があった。Cargo.lock で固定し、上げるときはコーパスを回して確かめる。
- 非末尾再帰はスタックを消費する。`List.map` のような cons list の素直な再帰は、既定のスタック（Linux では通常 8 MB）を超える深さで SIGSEGV になり、メッセージは出ない。v0.1 では許容し、prelude も素直な再帰で書く。
- Boehm GC の保守的走査。ポインタをタグ埋め込み等で隠すと回収されて壊れる。`repr` はポインタを隠さず 1 語で置く。
- 多相再帰。型注釈付きで多相再帰が推論される場合、実体化の集合が有限に閉じず単相化が停止しないことがある。v0.1 では M5 で検出して一律にエラーにする。
- v0.2 の `capture` 操作は末尾再帰でもスタックを消費する。Koka 3.2.3 の実測で約 330 バイト / 反復、既定スタック 8 MB では約 3 万反復で SIGSEGV し、メッセージは出ない（17.8）。生成器やイテレータを長い列に使うときの上限になる。回避には segmented stacks が要り、専用ランタイムを持つことになる。

v0.2 へ保留:

- 参照カウントによるメモリ管理。
- 条件列の到達不能判定への区間推論。
- record の field 更新構文。
- functor、first-class module。
- I/O と効果の順序付け。
- JIT。Cranelift の `JITModule` は `ObjectModule` と同じ `Module` trait なので `emit` を共有できる。
- 引数のパターン分岐で、名前付きの引数を先に並べて最後の引数に照合する形（7 節 Q16）。
- 腕の中で照合対象全体を束縛する `as` パターン（7 節 Q17'）。
- 型の別名（7 節 Q34）。
- 効果と I/O を入れるときの、値を捨てる `do`（7 節 Q23）。
- ライブラリ単位。`main` を持たず、公開 sig の member を E513 の根にする（7 節 Q25）。

## 7. R6 で確定した事項（2026-09-07）

M3〜M7 の成果物を実測して見つかった未決事項を、grilling 形式で確定した。仕様 `docs/spec.md` はこの表を反映済み。経緯は `docs/ledger.md` の R6。2.2 と 2.3 の表と食い違う箇所は本節が優先する。

判断基準として繰り返し使ったもの: 拒否か許容かで迷ったら拒否（error → 許容は既存コードを壊さない緩和なので後から安全にできる）。新機能は最小形で入れ、将来形の特殊ケースになっていることを確かめる。

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | E505 と pattern 束縛 | 及ぶ。未使用は `_` | §27 |
| Q2 | E505 と引数 | 及ぶ。`_` 引数は足さず、入力を見ない関数は書けない | §27 |
| Q3 | 注釈なし `poly x => poly [x]` | E307（旧 E701）。再帰呼び出しの引数型と仮引数型の単一化が出現検査に失敗した場合 | §4, §9 |
| Q4 | Boolean branch と E511 | 対象外のまま | §16 |
| Q5 | ADT の先頭 `\|` | 必須。E108 | §11 |
| Q6 | record 型の `;` | 区切り形に統一 | §12 |
| Q7 | `main` の印字 | Result の外側だけ剥がし、中身は canonical なソース表記。最上位の String と Char は生 | §51 |
| Q7' | 最上位の Unit | 無印字 | §51 |
| Q8 | 検査の段階順序 | §50 に規範として書く | §50 |
| Q9 | record 型が 0 件 | E304 を「一意に決まらない」に広げる | §12 |
| Q10 | 未使用の `type` `module type` `module` | E513 に含める。prelude は免除 | §27, §49 |
| Q11 | §41 | 完全な EBNF。字句と優先順位も取り込む | §41 |
| Q12 | 同一段階内の順序 | アンカー位置順 | §50 |
| Q13 | 進め方 | 仕様 → コーパス → 実装（`PLAN.md`） | — |
| Q14 | 引数ゼロの `=>` | E109 | §3 |
| Q15 | 引数のパターン分岐の形 | `def [rec] name pattern_arm+ pattern_fallback?` | §52 |
| Q16 | 引数 2 つ以上 | v0.1 は arity 1 に限る | §52 |
| Q17 | 従来形の扱い | E515 if-on-sole-parameter | §52 |
| Q17' | 腕で引数全体を使う関数 | E515 の対象外（自由な参照が scrutinee の 1 回だけのとき当たる） | §52 |
| Q18 | `fn` | 同じ形と E515 | §7, §52 |
| Q19 | 型注釈が要る関数 | E515 の対象外 | §52 |
| Q20 | 規則に対応しない構文失敗 | E100。明文規則に E110 E111 E112 | §43 |
| Q21 | 同名の宣言 | E205 duplicate-name | §47 |
| Q22 | struct が sig を満たさない | E602 signature-mismatch | §35 |
| Q23 | `do?` の対象 | E303 を `do?` にも広げ rail-type に改名。`do` は無い | §23 |
| Q24 | Float に `%` | `%` は Int 専用。E305 | §44 |
| Q25 | `main` が無い・引数を持つ | E206 entry-form。実行ファイルの規則 | §5 |
| Q26 | `let` `let?` のパターン | 型の全ての値に一致しなければ E402。newtype の `let Id(n) = x` は通る | §22 |
| Q27 | 負のリテラルパターン | 許す | §16, §41 |
| Q28 | パイプの右辺に constructor | E106 | §25 |
| Q29 | Float の印字 | 指数なしの最短往復、小数点の両側に 1 桁以上 | §51 |
| Q30 | prelude の免除 | E513 のみ | §49 |
| Q31 | 番号と段階 | 百の位を段階に一致させ、空きを詰める（E701 も型段階なので E307 へ） | §50 |
| Q32 | アンカー位置 | 規則が名指す最小の構文要素の先頭 | §50 |
| Q33 | ADT payload の無名 record | 書けない | §11, §12 |
| Q34 | 型の別名 | 許さない。newtype は単一 constructor の ADT | §12 |
| Q35 | sig の型 | 抽象型 `type T` のみ | §34 |
| Q36（R7 の審査で追加） | `inc(1)` と空白 | 空白は意味を持たない。E106 は constructor の側だけ（`Some 1`、`x \| Some`）。`inc(1)` は `inc (1)` と同じ並置として通す | §8, §41 |

grilling を経ずに commander が決めた点（可逆。異議があれば戻す）:

- Char のエスケープは `\n` `\t` `\\` `\'` の 4 つ（§43）。実装は Char にエスケープを持たないが、String と対称でないと改行の Char が書けない。
- パターンに List リテラルの糖衣は無い（§46）。実装どおり。
- `()` はパターンではない（§19）。実装どおり。
- shadowing は許す（§47）。§39 の例が依存している。
- constructor は関数値ではない（§8）。単独の `Some` を渡す形は E301。実装は `Some` を `Fn` に型付けしており `apply Some` が通る。§47 の名前空間の分離から導いた。

## 8. R8〜R9 の手順書（旧 `PLAN.md`、2026-09-07）

リポジトリ直下の `PLAN.md` として実行エージェントに渡した手順書。0.1 の完了時に本節へ移した（`PLAN.md` は廃止し、以降は `docs/plan.md` に統合する）。

この計画を実行するエージェント向けの手順書。R6（2026-09-07）の決定（`docs/plan.md` 7 節）を実行する。手順 0 から R9 まで、下の表の順に止まらずに進める。

**読む順:** 本ファイル → `docs/corpus.md` → `docs/spec.md`（§50 §41 §51 §52 を先に）→ `docs/plan.md` 7 節。2.2 と 2.3 の表と食い違えば 7 節が、7 節と仕様本文が食い違えば仕様本文が正で、食い違いは報告に書く。
**判定者:** 各工程の成果物は、作成した本人以外が判定基準に沿って審査する。別モデルのエージェントを呼べる環境ならそれに、呼べなければ人に出す。手順 0 の仕様は実行者が書いていないので、実行者が判定者になる。
**触ってよい場所:** 下の表の「成果物」列のみ。`docs/` と `PLAN.md` は全工程で禁止。R8 は `crates/` `scripts/` も、R9 は `tests/accept` `tests/reject` `stdlib/` も禁止。
**判定に現行コンパイラを使わない:** R8 では新形式を parse できず、検査順が §50 と違う。R8 の根拠は仕様の読みだけである。
**仕様に疑義が出たら:** 仕様を変えず、解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める。
**コミット:** 工程ごとに既存の形式（`test:` `feat:` と日本語の命令形）で。push しない。
**報告:** 完了時と、依存する作業で止まったときに返す。(1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

| 工程 | 成果物 | 判定 |
|---|---|---|
| 0. 仕様の審査 | 指摘の一覧（報告に含める） | 実行者。観点は下記 |
| R8 コーパス | `tests/accept` `tests/reject` `stdlib/` | 8-a〜8-d。8-c は別の判定者 |
| R8 の lock | `tests/corpus.sha256` | R8 の審査合格後に実行者が作る |
| R9 実装 | `crates/shin/**` `scripts/` `crates/shin/tests/**` | 9-a〜9-e。審査は別の判定者 |

**着手時点で `verify-m1-corpus.sh` チェック 1（§50 と拒否コーパスの 1 対 1）とチェック 2（§39 の例の新形式）、それらに依存する `verify-m2.sh` チェック 5、`verify-m3.sh` チェック 9 は既に赤である。R8 の終了時点でさらに増える。** 新形式（`def display | ...`）を parse できず、新しいコードを拒否できず、`-- expect:` 空を通せないため。R8 の合否は下記「R8 の判定」で測る。門番を緩めて green にすることは禁止。


### 手順 0. 仕様の審査

`docs/spec.md` は R7（2026-09-07）で改訂され、まだ審査を受けていない。観点: §41 の EBNF から §39 の例と受理コーパスの全構文が導出できるか、§50 の 56 行に段階とアンカーが全て書かれているか、`docs/spec-trace.md` の各行が実在する節を指すか、7 節の決定が本文に漏れなく反映されているか。

指摘は直さず報告に書く。R8 の特定のファイルが書けなくなる指摘だけ、そのファイルを止めて他を進める。`verify-m1-spec.sh` が `PASSED: 6/6` であることを実測する。


### R8 コーパス

R7 の仕様を根拠に書き直す。件数: 受理 23 → 27、拒否 46 → 56（E1xx 7 → 14、非 E1xx 39 → 42）。

#### 番号の付け直し（§50。ファイル名とヘッダを追随させる）

| 旧 | 新 | 短い名前 |
|---|---|---|
| E203 | E202 | dunder-in-user |
| E204 | E203 | dunder-as-value |
| E205 | E204 | unbound-name |
| E514 | E207 | rec-cycle-value |
| E516 | E208 | missing-rec |
| E515 | E514 | unused-struct-def |
| E601 | E113 | missing-signature |
| E602 | E601 | opaque-type |
| E701 | E307 | poly-recursion |
| E702〜E705 | E701〜E704 | main-polymorphic, main-contains-fn, eq-on-fn, compare-on-fn |

E303 は番号そのまま、短い名前を rail-type に。

#### 受理の書き直し

| ファイル | 変更 | 根拠 |
|---|---|---|
| `user_store.shin` | `type Error = \| NotFound` を足し、`struct` 実装を持たせて `main` から呼ぶ | §34, §27（未使用の `module type` は E513） |
| `recursive_type.shin` | 先頭 `\|` を付け、`List2` を `main` で使う | §11, §27 |
| `create_user.shin` | `Cons(head, tail)` → `Cons(_, _)` ×3 | §27 |
| `delete_user.shin` `unit.shin` | `-- expect:` を空に | §51 |
| `display.shin` `name.shin` `record.shin` `literal_pattern.shin` | 第52節の形に | §52 |
| `short_and.shin` | `-- item: 末尾呼び出し` を `tail_self.shin` へ移す | `docs/corpus.md` |

#### 受理の新設（`verify-m6.sh` の生成プローブを移す）

| ファイル | 内容 | ヘッダ |
|---|---|---|
| `division_error.shin` | `def main = 7 / 0` | `-- expect-error: DivisionByZero` |
| `tail_self.shin` | 自己再帰 1 億 | `-- expect: 42`、`-- item: 末尾呼び出し` |
| `tail_mutual.shin` | 引数数の異なる相互再帰 1 億 | `-- expect: 42`、`-- item: 相互再帰` |
| `tail_closure.shin` | 捕捉環境を持つ関数値経由 1 億 | `-- expect: 42` |

#### 拒否の書き直し

| ファイル（新番号） | 変更 | 根拠 |
|---|---|---|
| `E508.shin` | `def find_user (id: Int): Result<Int, String> => Ok(id)` を足し、成功枝を `Ok(user + 1)` に。失敗枝はそのまま | 二重欠陥の解消 |
| `E511.shin` | `one` `two` `other` → `"one"` `"two"` `"other"` | 同上 |
| `E509.shin` `E510.shin` | 引数を使う形に（`def f x => if x > 0 => Ok(()) \| Error("neg")`、`def f x => Cons(x, Nil)`） | §27 の引数、§50 の位置順で E505 が先に当たる |
| `E503.shin` `E507.shin` `E512.shin` | 第52節の形に。同一段階の E515 が位置順で先に当たるため | §52 |
| `E302.shin` | 先頭 `\|` を付ける | §11 |
| `E106.shin` | `def main = inc(1)` → `def main = Some 1`。空白は意味を持たず `inc(1)` は正当な並置になった | §8, §41 |
| `E307.shin`（旧 E701） | 内容そのまま。ヘッダの番号だけ | §4 |

E402 E403 E405 は網羅性段階で E515 より先に当たるので変えない。E303 と E207 の引数未使用は、型段階・名前解決段階が冗長段階より先なので変えない。

#### 拒否の新設（10 本）

| ファイル | 内容 |
|---|---|
| `E100.shin` | `def main = 1 < 2 < 3`（比較の連鎖） |
| `E108.shin` | `type Size = Large \| Small` |
| `E109.shin` | `def f => 1` |
| `E110.shin` | `def main = "a\q"` |
| `E111.shin` | `def main = ''` |
| `E112.shin` | `def main = 99999999999999999999` |
| `E205.shin` | `def f x => x + 1` を 2 回と、`f` を呼ぶ `main` |
| `E206.shin` | `main` が無い（`def f x => x + 1` のみ）。引数を持つ `main` は判定器側のプローブ |
| `E515.shin` | `def display user => if user \| Guest => "guest" \| Member(m) => m.name`（`type` 定義付き。`display` を呼ぶ `main`） |
| `E602.shin` | sig の member を struct が欠く。module を使う `main` |

非 E1xx の拒否は全件、全ての定義を使う `main` を持つ（既存の 39 本と同じ）。`main` が無いと E206 が 1:1 に当たって同一段階の E205 より先になり、定義が未使用だと E513 が E515 と同じ段階・同じ位置に当たる。

#### prelude（`stdlib/`）

| 定義 | 変更 | 根拠 |
|---|---|---|
| `List.odds` `evens` `sort` | 未使用の pattern 束縛を `_` に | §27 |
| `List.length` | `fold` + lambda をやめ、第52節の再帰に: `def rec length \| Nil => 0 \| Cons(_, tail) => 1 + length tail` | §27（引数）, §52 |
| `Option.is_some` `Result.is_ok` | 未使用束縛を `_` にし、第52節の形に | §27, §52 |
| `Result.map_error` | `Ok(x) => Ok(x)` は片腕だけの再包装で E507 の対象外（E507 は全腕が再包装のとき）。変更なし。実行者は `check_rewrap` の条件で確認する | §33 |

#### lock

R8 の審査合格後に実行者が `tests/corpus.sha256` を新設する（`sha256sum tests/accept/*.shin tests/reject/*.shin stdlib/*.shin`）。R9 では更新しない。`corpus.rs` は R9-6 で書き換えるので lock に含めず、`verify-m3.sh` チェック 8 の暗記排除の走査対象に入れる（R9-7）。

#### R8 の判定

- 8-a. `verify-m1-corpus.sh` のチェック 1〜4 と 6 が OK（件数は §50 の行数から導く。チェック 5 と 7 は `cargo test` を含むので R9 まで赤）。
- 8-b. 全件の `-- spec: §N` が実在する見出しを指し、`-- item:` が `docs/spec-coverage.md` と文字列一致。
- 8-c. 別の判定者が全 83 本を読み、「1 ファイル 1 欠陥」（§50 の段階順とアンカー位置順で判定）と「タグを寄せていない」を確認する。
- 8-d. R8 終了時に赤になる門番を報告に列挙する（見込み: `verify-m1-corpus` の 5 と 7、`verify-m2` と `parse_corpus.rs` の件数、`verify-m3` の accept-20、`verify-m4`〜`m7` の新形式と新コード）。


### R9 実装

順序に依存あり。各項目の判定器は「入力を実行時に生成し、期待値を判定器側で計算する」（`docs/corpus.md`）。`docs/spec-trace.md` の「固定するもの」列が、各項目の判定器の名前を参照する。

1. **字句と構文**（`lexer.rs` `parser.rs`）: E100（規則に対応しない失敗）、E108、E109、E110〜E112、Char のエスケープ 4 つ、負のリテラルパターン、`def` と `fn` の腕形式、sig の型は本体なし、型の別名と無名 record を E100。E106 を constructor を先頭に置く並置とパイプの右辺の constructor に変え、識別子と `(` の隣接判定（`parser.rs:1112`）を消す。診断コードの付け直し。判定器: §41 の EBNF から乱数でプログラムを生成し、parse → 正規形印字 → 再 parse の往復（`parse_corpus.rs` の式木往復を全文法に広げる）。E106 は乱数の constructor 名と引数で `C x` と `x |> C` を生成し、全件 E106。空白の摂動: 生成した `f (x)` から識別子と `(` の間の空白を抜いても同じ木になる（既存の式木往復は隣接形を出さないので、これが Q36 を見る唯一の検査）。
2. **検査順**（`lib.rs` `check_source`）: §50 の段階順。同一段階内はアンカー位置順。判定器: 型エラーと冗長欠陥を同居させた `def` を 20 本生成し、全件 E305 か E204。位置順は、同一段階の欠陥 2 つを位置を入れ替えて生成し、先の方が報告されること。`%` を Int 専用に型付けし、Float への適用が E305 になること。
3. **名前解決**（`names.rs` `infer.rs`）: 実装されない `module type` 内の型名も E204。E205（同名宣言。record の field と sig の型変数を含む）。E206（`main` が無い・引数を持つ）。E513 を `type` `module type` `module` に広げ、prelude の免除を E513 に閉じる。E602（sig の不一致 4 種類）。constructor を `Fn` に型付けせず、payload の数と引数の数の不一致を E301 に（単独の `Some` を含む）。判定器: 乱数の型名で sig を生成、乱数の名前で重複を生成、乱数の arity の constructor に 0 個と arity+1 個の引数を渡して全件 E301。
4. **網羅性と冗長**（`coverage.rs` `check.rs`）: `let` `let?` のパターンに網羅性（E402）。E505 を pattern 束縛と引数に（束縛を考慮した参照計数）。E515（自由な参照の計数は `names::references` 系を使う）。E304 の 0 件。E303 を `do?` に。判定器: E515 の 4 条件を 1 つずつ外した 4 プローブ、refutable な `let` の生成。
5. **印字**（`mono.rs` の `print_T`、`emit.rs`、`runtime/`）: §51。List、入れ子の String と Char、最上位 Unit、Float。判定器: 型ごとの値を乱数で作り、期待文字列を判定器側で組み立てて実行結果と照合する。
6. **コーパス走行**（`corpus.rs`）: `-- expect:` 空、`-- expect-error:`。
7. **門番**: 件数の直書き（`verify-m2` 23/39、`parse_corpus.rs` 7/39、`verify-m3` 20）を §50 の行数とディスクの実数から導く形に。`verify-m3.sh` チェック 10 を `tests/corpus.sha256` との照合に、チェック 8 の暗記排除（コーパスのファイル名と 20 文字以上の行が現れない）の走査に `crates/shin/tests/corpus.rs` を含める。`verify-m4.sh` `verify-m7.sh` の末尾を `PASSED: n/N` に。`verify-m6.sh` の生成プローブ 4 本を消す。上記 1〜5 の判定器を `scripts/verify-r9.sh` に置く。

R9 の受け入れ条件:

- 9-a. 全門番（m0〜m7、r9）が終了コード 0、`cargo test` 全件通過。
- 9-b. 受理 27 本が期待 stdout / stderr / 終了コードを返す（`verify-m7.sh`）。
- 9-c. 拒否 56 本が指定コードと指定アンカーで落ちる。位置の摂動プローブ（`-- error:` 行の直後に空行）を全コードに当てる。
- 9-d. `tests/corpus.sha256` が一致する（R9 でコーパスと `stdlib/` を触っていない）。
- 9-e. `docs/spec-trace.md` の「固定するもの」に書かれた判定器が全て実在する。


### スコープ外

`docs/plan.md` 6 節「v0.2 へ保留」の項目。warning の導入（緩和は「規則から外す」で行う）。

### 分からなかったこと

- E307 の判定（`infer.rs:405-415`）が注釈付きの多相再帰をどこまで捕まえるかは未検証。`def` に型変数を書く構文が無く、プローブが書けなかった。§4 の定義はこの判定を写している。
- prelude が全検査を通ることは、全プログラムへの連結経由で間接的にしか確かめられていない。R9-3 で免除を E513 に閉じたあと、判定者が免除の範囲を見る。


## 9. v0.1.1 の範囲と決定（2026-09-08）

`docs/wants.md` の 6 項目のうち 4 項目（言語名の変更、`//` コメント、List の `[x, ...xs]`、引数の条件列の短縮形）を 0.1.1 とする。grilling（14 問）で確定し、経緯は `docs/ledger.md` の「0.1.1 の計画」にある。仕様 `docs/spec.md` は本節を反映済み。2.1〜2.3 と 7 節と食い違う箇所は本節が優先する。

判断基準は 7 節と同じ。拒否か許容かで迷ったら拒否し、新機能は最小形で入れる。受理を広げる変更は後から安全に足せるので、後回しにした拡張は 9.3 に置く。

### 9.1 決定

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | 0.1.1 の位置づけ | v0.1 を tag `v0.1` で凍結し、wants の一部を 0.1.1 とする | — |
| Q2 | 範囲 | 言語名、コメント、List 構文、引数の条件列。ファイル名 module と `let value? =` は次の版 | — |
| Q3 | 改名の位置 | 最初のラウンド（R10）。以降の仕様・コーパス・コミットを新しい語彙で書くため | — |
| Q4 | 改名の範囲 | 過去の記録（`ledger.md` と本文書の 1〜8 節、git 履歴）以外の全て。CLI `hado`、拡張子 `.hd`、crate `crates/hado`、ランタイム記号 `hado_entry` `hado_alloc` | — |
| Q5 | 文書の版 | tag の後、6 文書を版接尾辞なしの `docs/spec.md` などに `git mv` して直接編集する。0.1 の姿は tag が持つ。（当初は `docs/0.1.1/` に置いたが、版は tag が持つのでディレクトリも外した） | — |
| Q6 | 手順書の置き場 | `PLAN.md` を廃止し `docs/plan.md` に統合する。`AGENTS.md` と `README.md` は進行中の開発に依存しない | — |
| Q7 | 再編の時期 | tag の前。tag が「0.1 完了で文書も整合した状態」を指すため | — |
| Q8 | コメント | `//` に置き換える。`--` と `#` は E104 | §41, §43 |
| Q9 | List の書き方 | 式もパターンも `[]` `[x, ...xs]` が唯一の形。式の `Nil` `Cons` は E510（`list-literal-form` に改名して範囲を広げる）、パターンは E516。prelude も書き直す | §46, §47, §50 |
| Q10 | 展開の位置 | 末尾のみ。中間展開は 9.3 | §41, §46 |
| Q11 | 条件列の短縮形 | `def classify score \| score >= 90 => ... \| Fail`。名前あり + `\|` は条件列、名前なし + `\|` は pattern（§52）。`fn` も同じ | §7, §15, §53 |
| Q12 | 従来形 `=> if \| ...` | E517 で拒否。arity と型注釈の例外なし。継続を挟んだ形は対象外 | §53 |
| Q13 | 実行 | 本文書 10 節を手順書としてエージェントに渡す。仕様は本セッションが改訂済み | — |
| Q14 | 式側の `...` の対象 | 任意の式。`[f head, ...map tail f]` と書ける。パターン側は名前か `_` | §41, §46 |

前提として置いたもの: `[...xs]` 単独と末尾以外の `...` は E100。引数ゼロの `def main | true => 1 | 0` は §52 の pattern 腕として読まれ E206 になる（grilling 中は E109 と示したが、§52 の文法に一致するので新しい規則は要らない）。名前あり + `|` を pattern のつもりで書いた `def f x | Some(y) => y | 0` は `y` の E204 のままで、専用の診断は足さない。印字（§51）は既に `[1, 2, 3]` 形なので変えない。

### 9.2 仕様の項目

コーパスの `item:` タグに使う項目名。2.2 の項目に加える。

| 項目 | 仕様の節 | 内容 |
|---|---|---|
| List 構文 | §46 | `[]` `[x, ...xs]` が式とパターンの唯一の形。`Nil` `Cons` は E510 / E516 |
| 引数の条件列 | §53 | `def f x \| 条件 => ... \| fallback`。`=> if` を付けた形は E517 |

コメントは 2.2 の「コメント」（§43）をそのまま使う。言語名は仕様の節を持たない。

### 9.3 次の版へ保留

- ファイル名を module 名にし、トップレベルの `module` 宣言を省く（wants 2）。6 節の「ライブラリ単位」と同じ設計になる。
- `let value? =` の形で `let?` `do?` を `let` `do` に統一する（wants 6）。§26 の記号体系を変えるので単独で決める。
- List パターンの中間展開 `[first, ...middle, last]`。cons リストでは O(n) の走査と `middle` の割り当てが照合の中に入り、網羅性の判定に長さの推論が要る。「パターン照合が走査と割り当てを持ってよいか」を先に決める。
- `def f x | Some(y) => ...` のように名前あり + `|` を pattern として書いた入力への専用診断。頻発するなら足す。

## 10. R10〜R13 の手順書

この節を実行するエージェント向けの手順書。9 節の決定を実行する。R10 から R13 まで表の順に進める。

**読む順:** 本節 → `docs/corpus.md` → `docs/spec.md`（§41 §43 §46 §50 §53 を先に）→ 9 節。表と仕様本文が食い違えば仕様本文が正で、食い違いは報告に書く。
**判定者:** 各工程の成果物は、作成した本人以外が判定基準に沿って審査する。別モデルのエージェントを呼べる環境ならそれに、呼べなければ人に出す。
**仕様は確定済み:** 9 節の決定は仕様に反映済みで、各ラウンドは コーパス → lock → 実装 の順に進む。判定に現行コンパイラを使えるのは R10 だけである。R11 以降は新形式を parse できないので、コーパスの根拠は仕様の読みだけである。
**仕様に疑義が出たら:** 仕様を変えず、解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める。
**過渡状態:** 本節を書いた時点で `docs/` の仕様・コーパス規約・追跡表は hado の語彙、コードとコーパスは shin の語彙である。R10 が解消する。
**コミット:** 工程ごとに既存の形式（`test:` `feat:` と日本語の命令形）で。push しない。
**報告:** 完了時と、依存する作業で止まったときに返す。(1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

| 工程 | 成果物 | 判定 |
|---|---|---|
| R10 改名 | リポジトリ全体（`docs/ledger.md` と本文書の 1〜8 節を除く）、`tests/corpus.sha256` | 10-a〜10-d。審査は別の判定者 |
| R11 コメント | `tests/accept` `tests/reject` `stdlib/` のヘッダ、`docs/corpus.md`、`crates/hado/**`、`scripts/`、lock | 11-a〜11-c |
| R12 List 構文 | `tests/accept` `tests/reject` `stdlib/`、`crates/hado/**`、`scripts/`、lock | 12-a〜12-e |
| R13 引数の条件列 | `tests/accept` `tests/reject`、`crates/hado/**`、`scripts/`、lock | 13-a〜13-d |

**着手時点で赤の門番（実測）:** `cargo test` の 2 本（`parse_corpus::corpus_counts` は §50 が 58 行で拒否が 56 本、`r9_grammar` は §41 の新形式を現行パーサが読めない）。`verify-m1-corpus.sh` の 1（E516 E517 が無い）、2（§39 の `classify` が短縮形）、3（9.2 の項目に受理と拒否が無い）、7。`cargo test` の終了コードを見る `verify-m0.sh` の 2〜4、`verify-m2.sh` の 1・3・5、`verify-m3.sh` の 9、`verify-r9.sh` の R9-1。内訳は `docs/ledger.md` の「0.1.1 の計画」。R13 の終了時点で全て green に戻す。門番を緩めて green にすることは禁止。

### R10 改名（shin → hado、`.shin` → `.hd`）

機械的な変更だけを含む。振る舞いの変更を混ぜない。

- `crates/shin` → `crates/hado`（`git mv`）。package 名、bin 名、workspace の members、`Cargo.lock`
- `tests/accept` `tests/reject` `stdlib/` の全ファイルを `.hd` に `git mv`。`main.rs` の拡張子検査と usage、`build.rs` の一時ディレクトリ名、`lib.rs` の `include_str!` のパス
- ランタイム記号 `shin_entry` `shin_alloc` → `hado_entry` `hado_alloc`。`runtime/` `runtime_print.c` `runtime_abi.rs` `emit.rs` と、記号を検査する `verify-m0.sh`
- 門番スクリプトとテストの文字列、`README.md`、`AGENTS.md`、`docs/cranelift-0.135.md`、`docs/` の `corpus.md` `spec-trace.md` `spec-coverage.md`（仕様と本節は hado で書いてある）
- **lock の再生成。** `tests/corpus.sha256` はファイル名を含むので改名で必ず不一致になる。`sha256sum tests/accept/*.hd tests/reject/*.hd stdlib/*.hd` で作り直す。`AGENTS.md` の「実装の工程で更新しない」の例外を本項が与える

R10 の判定:

- 10-a. `grep -rIl shin --exclude-dir=.git --exclude-dir=target . | grep -v '^./docs/ledger.md$' | grep -v '^./docs/plan.md$'` が空。`plan.md` は 1〜8 節が 0.1 の記録で、10 節は改名の記述として旧名を含むので除く
- 10-b. 全門番（m0〜m7、r9）が改名前と同じ `PASSED: n/N`、`cargo test` が同数で通過
- 10-c. lock のハッシュ列が改名前と同一（`cut -d' ' -f1` の diff が空）。内容を変えていない証拠
- 10-d. `hado build x.shin` が拡張子で拒否され、`hado build x.hd` が実行ファイルを作る

### R11 コメント（`//`）

仕様は §41 §43 §50 に反映済み。

コーパス:

- 全件のヘッダ `-- spec:` `-- error:` `-- expect:` `-- expect-error:` `-- item:` `-- needs:` `-- short-circuit:` を `// ` に。`docs/corpus.md` のヘッダタグ表も
- `E104.hd` の本命を `#` から `--`（0.1 の記法）に。`#` は判定器のプローブに移す
- `comment.hd` を `//` に

実装（`lexer.rs`）: `//` を行末まで読み飛ばす。`--` と `#` を見たら E104 で、アンカーは 1 文字目。`/` 単独は除算のまま。

門番とテスト: `-- ` 接頭辞に依存する箇所（`grep -rn '"-- \|-- spec\|-- expect\|-- error\|-- item' scripts crates/hado/tests`。2026-09-08 時点で 47 箇所）を `// ` に。`verify-m3.sh` の位置摂動プローブ（`error:` 行の直後に空行）も同じ

R11 の判定:

- 11-a. 全門番と `cargo test` が通過（R12 R13 に依存するチェックを除く。除いたものを報告に列挙する）
- 11-b. 判定器（`scripts/verify-r11.sh`）: `--` と `#` を行頭・式の途中・行末に置いた入力を乱数で生成し、全件 E104 で列がその 1 文字目。同じ位置に `//` を置くと通る。`def main = 6 / 2` は除算、`def main = 1 // 2` は `1`
- 11-c. 審査合格後に lock を更新

### R12 List 構文（`[]` `[x, ...xs]`）

仕様は §26 §41 §46 §47 §50 §52 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| `E510.hd` | `Cons(x, Nil)` → `Cons(x, xs)`（末尾が変数）。範囲が「式の `Cons` `Nil` 全て」に広がったことを示す形に | §46 |
| `E516.hd` | 新設。パターンに `Nil` を使う関数と、それを呼ぶ `main`。§52 の名前なし形か引数 2 つ以上で書く（`def f xs => if xs \| Nil => ...` は E515 が `def` の位置で先に当たる）。`// item: List 構文` | §46 |
| `E701.hd` | `def main = Nil` → `def main = []`。E510 が冗長段階で E701 の単相化より先に当たる | §50 |
| `create_user.hd` | `Nil` → `[]`、`Cons(_, _)` → `[_, ..._]` | §46 |
| 受理の新設 `list_pattern.hd` | 末尾展開・固定長・入れ子のパターンと、展開式で List を組む `main`。`// item: List 構文` | §46 |
| `stdlib/list.hd` | `List` module の全関数を `[]` `[head, ...tail]` に。`type List` の定義行だけ `Nil` `Cons` が残る | §46, §49 |

拒否コーパスの他のファイルに `Nil` `Cons` が無いことを確かめる（2026-09-08 時点で `grep -l 'Cons\|Nil' tests/reject` は E510 と E701 のみ）。冗長段階（E5xx）の他のファイルにあれば、アンカー位置順で E510 か E516 が先に当たる。

実装の注意（決定ではない）:

- E510 と E516 は利用者が**書いた形**を見る検査で、網羅性・型・emit は展開後の `Cons` `Nil` で動く。式側は `NodeKind::List` が別ノードなので既存の E510 が判定できている。パターン側にも List パターンのノード（か展開前の印）が要る。`coverage.rs` は展開後を渡せば触らない
- `Nil` `Cons` の名前解決は残す。E510 と E516 は冗長段階なので、名前解決・型・網羅性が先に走る（§50）
- 正規形の印字（`source_print.rs`）は List 形で出す。往復の判定器がそれを見る

R12 の判定:

- 12-a. 全門番と `cargo test` が通過（R13 に依存するチェックを除く）
- 12-b. 判定器（`scripts/verify-r12.sh`）: (1) 乱数の長さと入れ子で List の式とパターンを生成し、parse → 正規形印字 → 再 parse の往復。(2) 網羅性: `[]` と `[_, ..._]` は受理、`[]` と `[_]` は E402、`[_, ..._]` の後の `[_, _, ..._]` は E403、同じ形 2 回は E405。(3) `Nil` `Cons` を式とパターンの乱数の位置（入れ子を含む）に置いて全件 E510 か E516、アンカーはその名前の先頭。(4) `[...xs]` の両側、中間展開、`...` の後の名前でないパターンは E100。(5) 展開式で組んだ List を実行し、印字が判定器側で組んだ `[1, 2, 3]` 形と一致
- 12-c. `stdlib/` に `Nil` `Cons` が `type List` の定義行以外に無い
- 12-d. 審査合格後に lock を更新
- 12-e. 位置摂動プローブが E510 E516 に当たる

### R13 引数の条件列

仕様は §7 §15 §39 §41 §50 §53 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| `classify.hd` | 短縮形に。`verify-m1-corpus.sh` チェック 2 が §39 と照合する | §39, §53 |
| `E517.hd` | 新設。`def classify score => if \| ...` と `main`。`// item: 引数の条件列` | §53 |
| 受理の新設 `condition_params.hd` | 引数 2 つと型注釈付きの短縮形、`fn x \| ...`。`// item: 引数の条件列` | §53 |

実装の注意（決定ではない）:

- parser: `parameters` の直後が `|` なら条件列の腕。AST は `=> if | ...` と区別できる印を持つ（E517 の判定と正規形印字のため）
- E517: `def` か `fn` の本体が条件列のノードちょうどで、短縮形で書かれていないもの。arity と注釈を見ない
- E515 との関係: 名前あり + `|` は条件列で pattern 腕にならないので、E515 の判定は変えない
- 正規形の印字は短縮形で出す

R13 の判定:

- 13-a. 全門番（m0〜m7、r9、r11〜r13）が終了コード 0、`cargo test` 全件通過
- 13-b. 判定器（`scripts/verify-r13.sh`）: (1) 乱数の arity（1〜3）と注釈の有無で `=> if | ...` を生成し、全件 E517 でアンカーが `def` か `fn`。(2) 同じ腕の短縮形が通り、実行結果が判定器側で計算した値と一致。(3) `let` を挟んだ形は通る。(4) `def main | true => 1 | 0` は §52 形として parse され E206。(5) `def f x | Some(y) => y | 0` は E204
- 13-c. 審査合格後に lock を更新
- 13-d. 位置摂動プローブが E517 に当たる

R13 の終了時に `git tag v0.1.1` を打つ。

### スコープ外

9.3 の項目と 6 節の「v0.2 へ保留」。warning の導入。

## 11. v0.1.2 の範囲と決定（2026-09-08）

`docs/wants.md` に残った 2 項目（ファイル名 module、`let value? =`）と 9.3 の 4 項目を、grilling（30 問）で確定した。経緯は `docs/ledger.md` の「0.1.2 の計画」にある。仕様 `docs/spec.md` は本節を反映済み。2.1〜2.3、7 節、9 節と食い違う箇所は本節が優先する。

判断基準は 7 節と 9 節と同じ。拒否か許容かで迷ったら拒否し、新機能は最小形で入れる。

### 11.1 決定

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | 範囲 | 9.3 の 4 項目を全て検討する。結果は入れる 2、やらない 2（11.3 の先頭 2 項目） | — |
| Q2 | module の探索 | 入口ファイルと同じディレクトリ。サブディレクトリと検索パスは無い。Q16 で「全て読む」に改めた | §54 |
| Q3 | ファイル名と module 名 | 当初は UpperCamel → snake_case の導出。Q19 で削除し、名前は見出しだけで決める | §54 |
| Q4〜Q8 | 契約の書き方 | `.hdi` は残す。契約ファイルは実装を知らず、満たす契約は実装側が宣言する | §34, §54 |
| Q9 | 1 ファイル 1 module | 慣習とする。ファイル内に複数の節を置ける | §54 |
| Q10, Q12 | キーワード | `module`（契約）と `implement`（実装）。見出しの足場 `module type X = sig ... end` と `module X : SIG = struct ... end` が消える。型注釈の `:` と実体化の `=` は残る | §34, §35, §40 |
| Q11 | 契約の無い `.hd` | 入口ファイルだけ。入口の外に `def` と `main` を置く | §54 |
| Q13 | 1 対 1 の宣言 | 省かない。`.hd` の節は必ず見出しで始まる | §35 |
| Q14 | 節形式 | 見出しは `module X` と `implement X as Y`。次の見出しかファイル末尾まで本文。`end` と入れ子は無い | §34, §41 |
| Q15 | 契約の型 | 抽象型 `type T` のみ（7 節 Q35 のまま） | §34 |
| Q16 | 外の型 | 最初の見出しより前に本体付きの型を置き、修飾なしで全体に見える。ディレクトリの `.hd` `.hdi` は全て読む | §41, §54 |
| Q17 | 節の可視性 | 全ての節は top-level。`as` は必須 | §35, §54 |
| Q18 | 1 対 1 の名前 | `implement UserStore as UserStore`。契約と実装は別の名前空間 | §35, §47 |
| Q19 | `.hdi` | 見出しを書く。`module` 節と外の型だけを置ける。ファイル名は自由 | §54 |
| Q20 | 外の `def` | 入口ファイルだけ。他は E100 | §54 |
| Q21 | 旧構文 | E100。専用コードは足さない | §35 |
| Q22 | コーパスの単位 | 1 ケース 1 ディレクトリ（`tests/accept/classify/main.hd`） | — |
| Q23 | `let?` `do?` | `let p? = e ->` と `do e ->`。`?` はパターン末尾に 1 つ。旧形は E100 | §22, §23, §26 |
| Q24 | List の中間展開 | やらない（11.3） | §46 |
| Q25 | 名前あり + `\|` の専用診断 | やらない（11.3） | — |
| Q26 | ラウンド | R14 `let p? =` → R15 ディレクトリ化 → R16 節構文 → R17 複数ファイル | — |
| Q28 | `implement` 節の本体付きの型 | 書ける。その実装の中だけで見える | §35 |
| Q29 | `.hdi` の外の型 | 書ける。規則は `.hd` と同じ | §54 |
| Q30 | 入口ファイルの節 | 置ける。入口の特別扱いは「外に `def` を置ける」だけ | §54 |

Q27 は一覧の確認。

### 11.2 仕様の項目

コーパスの `item:` タグに使う項目名。2.2 と 9.2 の項目に加える。

| 項目 | 仕様の節 | 内容 |
|---|---|---|
| 節構文 | §34, §35, §41, §54 | `module X` と `implement X as Y` の見出し。外の型。`.hdi`。ディレクトリ全体を読む |
| let p? = | §22, §23 | `let p? = e ->` と `do e ->`。旧形 `let?` `do?` は E100 |

### 11.3 やらないこと

- List パターンの中間展開 `[first, ...middle, last]` と末尾展開 `[...init, last]`。cons list は末尾に O(1) で触れず、走査と割り当てが照合の中に隠れる。cons list を持つ OCaml、Haskell、Elixir、F# も先頭マッチしか持たない。List の表現を変える最適化をした後に再考する
- `def f x | Some(y) => ...` への専用診断。E204 が `y` を指すので止まる位置は正しい。足さない
- 6 節の「効果と I/O を入れるときの、値を捨てる `do`」は、`do` が失敗の伝播を意味するようになったので、入れるときに別の綴りを決める

### 11.4 commander が置いた細則（可逆。異議があれば戻す）

- ファイルをまたぐ順序は、入口ファイルを先、残りをパスのバイト順。E205 の「2 回目」と §50 の「ソース順」はこの順で比べる
- `?` は独立したトークンで、空白は意味を持たない（7 節 Q36）ので `let x ? = e` も受理する
- E113（missing-signature）は新構文で発生しないので表から削る。番号は欠番のままにする。§50 の行数は 57
- E514 の短い名前を `unused-struct-def` から `unused-implement-def` に、E602 を `signature-mismatch` から `contract-mismatch` に変える。`struct` と signature が仕様の語彙に無いため
- 診断の形式は現行の `パス:行:列: error[コード]: 説明` のまま。§50 に書いた。位置摂動プローブはファイルの次元を足す
- 型の参照 `Y.T` は実装 `Y` の契約の抽象型で不透明。実装の中の ADT・record は外から E204。契約で修飾した `X.T` は無い（E204）
- 契約の中では実装の名前空間を解決しない（E204）。契約の参照は `implement` の見出しだけなので、「参照されない契約」と「実装の無い契約」は同じ
- 未使用の型は外でも `implement` 節の中でも E513。`implement` 節の `def` は E514
- 契約に無い抽象型の実体化 `type U = Int` は E204。実装の中の ADT・record は実装ごとの局所名前空間で、外の型や抽象型と同名なら E205。修飾参照の E204 は解決できない最初の名前を指す。E513 の対象に実装の中の ADT・record を含め、実体化は含めない
- `hado check` に入口以外のファイルを渡すと、本来の入口ファイルが外の `def` で E100 になる。§54 に書いた

## 12. R14〜R17 の手順書

この節を実行するエージェント向けの手順書。11 節の決定を実行する。R14 から R17 まで表の順に進める。

**読む順:** 本節 → `docs/corpus.md` → `docs/spec.md`（§22 §23 §34 §35 §41 §54 を先に）→ 11 節。表と仕様本文が食い違えば仕様本文が正で、食い違いは報告に書く。
**判定者:** 各工程の成果物は、作成した本人以外が判定基準に沿って審査する。別モデルのエージェントを呼べる環境ならそれに、呼べなければ人に出す。
**門番:** `scripts/verify-*.sh` の全て。着手時点は m0、m1-spec、m1-corpus、m2〜m7、r9、r11〜r13 の 13 本で、R14、R16、R17 が verify-r14、r16、r17 を足す（R15 は判定器を持たない）。「全門番が通過」は、その時点の全本が終了コード 0 で、`cargo test --offline --no-fail-fast` が 0 failed の意味である。
**仕様は確定済み:** 11 節の決定は仕様に反映済みで、各ラウンドは コーパス → lock → 実装 の順に進む。現行コンパイラで判定できるのは R15 だけである。R14 と R16 は新形式を parse できず、R17 はディレクトリを読めないので、コーパスの根拠は仕様の読みだけである。
**仕様に疑義が出たら:** 仕様を変えず、解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める。
**過渡状態:** 本節を書いた時点で `docs/` の仕様・追跡表・対応表は 0.1.2 の語彙、コードとコーパスと stdlib と `docs/corpus.md` の配置説明は 0.1.1 の語彙である。R14〜R17 が解消する。
**コミット:** 工程ごとに既存の形式（`test:` `feat:` と日本語の命令形）で。push しない。
**報告:** 完了時と、依存する作業で止まったときに返す。(1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

| 工程 | 成果物 | 判定 |
|---|---|---|
| R14 `let p? =` と `do` | `tests/accept` `tests/reject` `stdlib/result.hd`、`crates/hado/**`、`scripts/`、lock | 14-a〜14-d |
| R15 ディレクトリ化 | `tests/accept/*/main.hd` `tests/reject/*/main.hd`、`crates/hado/tests/**`、`scripts/`、`docs/corpus.md`、`docs/spec-trace.md`、lock | 15-a〜15-c。審査は別の判定者 |
| R16 節構文 | `tests/accept` `tests/reject` `stdlib/`、`crates/hado/**`、`scripts/`、lock | 16-a〜16-e |
| R17 複数ファイル | `tests/accept` `tests/reject`、`crates/hado/**`、`scripts/`、`docs/corpus.md`、lock | 17-a〜17-d |

**着手時点で赤の門番（実測）:** 12 節の末尾「着手時点の実測」に置く。R17 の終了時点で全て green に戻す。門番を緩めて green にすることは禁止。

### R14 `let p? =` と `do`

仕様は §6 §18 §22 §23 §24 §26 §27 §28 §39 §40 §41 §49 §50 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| `create_user.hd` `delete_user.hd` | `let? x =` → `let x? =`、`do? e` → `do e` | §22, §23 |
| `E303.hd` `E504.hd` | 同上。期待するコードは維持 | §22, §23 |
| `stdlib/result.hd` | 同上 | §49 |
| 受理の新設 `rail_forms.hd` | record パターンと constructor パターンに `?` を付けた `let`、`do` の連鎖、`main` で実行。`// spec: §22`、`// expect:`（実行値）、`// item: let p? =` | §22, §23 |
| 拒否の `item:` | `E303.hd` に `// item: let p? =` を付ける | §22 |
| `E113.hd` | 削除。§50 の行数は 57 で、E1xx は 13 本。E113 は新構文で発生しないので、コーパスの削除を先に行う（実装の削除は R16） | §50 |

実装の注意（決定ではない）:

- lexer: `?` を独立したトークンにする。`let?` は `let` `?` に分かれ、`?` の位置でパターンが無いので E100 になる。`do?` も `?` で E100
- parser: `let pattern "?" "="`。AST は `?` の有無を持つ。正規形の印字は `let p? =`
- 文法生成器（`r9_grammar`）の EBNF を §41 に合わせる。§41 の `"?"` と `module_item` の記法を生成器が読めなければ、生成器側を直す（parser の拒否と取り違えない）
- E508 のメッセージ文（`use let? or do?`）と、それを引用する `README.md` の例を新形に

R14 の判定:

- 14-a. 全門番と `cargo test` が通過。ただし節構文に依存する次の赤は R16 まで残る: `cargo test` の `r9_grammar`（§41 の節形式を生成する）、`verify-r9.sh` の R9-1、`verify-m1-corpus.sh` の 2（§39 の `module UserStore`）と 3（節構文のペア）、および `cargo test` の終了コードを見る m0 の 2〜4、m1-corpus の 7、m2 の 1 と 5、m3 の 9、m7 の 3。これ以外の赤があれば R14 の欠陥
- 14-b. 判定器（`scripts/verify-r14.sh`）: (1) 名前・record・constructor・入れ子のパターンを乱数で生成し、`let p? = e ->` の parse → 正規形印字 → 再 parse の往復。`let p ? =` は同じ木。(2) `let? x = e ->` と `do? e ->` は E100 でアンカーが `?`。(3) `let x? = 純粋値` と `let _? = 純粋値` は E303、`let _? = e`（`e` は Option か Result）は E504、`do 純粋値` は E303。(4) `let p? =` と `do` の連鎖を実行し、成功と失敗の両経路で判定器側の値と一致。(5) 網羅的でないパターン（`let Some(x)? = e`）を生成して E402 で、アンカーはそのパターン
- 14-c. 審査合格後に lock を更新
- 14-d. 位置摂動プローブが E303 E504 に当たる

### R15 コーパスのディレクトリ化

機械的な変更だけを含む。振る舞いの変更を混ぜない。

- `tests/accept/X.hd` → `tests/accept/X/main.hd`、`tests/reject/EXXX.hd` → `tests/reject/EXXX/main.hd`（`git mv`）
- `crates/hado/tests/**` と `scripts/*.sh` の `tests/accept/*.hd` `tests/reject/*.hd` を `tests/accept/*/main.hd` `tests/reject/*/main.hd` に。ケース名はディレクトリ名から取る
- `docs/corpus.md` の配置説明を「1 ケース 1 ディレクトリ。ヘッダタグは `main.hd` に置く」に。`docs/spec-trace.md` のコーパスのパス（`tests/reject/E111.hd` など）も新しい配置に
- **lock の再生成。** `sha256sum tests/accept/*/main.hd tests/reject/*/main.hd stdlib/*.hd`。`AGENTS.md` の「実装の工程で更新しない」の例外を本項が与える（R10 と同じ）

R15 の判定:

- 15-a. 各門番スクリプトの末尾の `PASSED: n/N` と、`cargo test` の passed / failed の数が、R14 終了時と同一。R14 で残った赤は同じ赤のまま
- 15-b. lock のハッシュ列が R14 終了時と同一（`cut -d' ' -f1` の diff が空）
- 15-c. `tests/accept` `tests/reject` の直下に `.hd` が無く、各ディレクトリに `main.hd` がちょうど 1 つ

### R16 節構文

仕様は §3 §5 §27 §34 §35 §39 §40 §41 §47 §49 §50 に反映済み。このラウンドは入口ファイル 1 つの範囲で新構文を固める。ディレクトリの読み込みと `.hdi` は R17。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| `user_store/` `namespace/` `E601/` `E514/` `E602/` | 節形式に書き直す。`module type` `sig` `struct` `end` を外し、`implement X as X` に。期待するコードは維持 | §34, §35 |
| 受理の新設 `sections/` | 外の record、`module Store`、`implement Store as MemoryStore` と `implement Store as FileStore`、`implement` 節の中だけの本体付きの型、両方を呼ぶ `main`。`// item: 節構文` | §34, §35, §41 |
| 拒否の `item:` | `E602/` に `// item: 節構文` を付ける | §35 |
| `stdlib/*.hd` | 外の型 + `module X` + `implement X as X` に書き直す | §49 |

実装の注意（決定ではない）:

- lexer: `implement` `as` をキーワードに足し、`sig` `struct` `end` をキーワードから外す
- parser: `program := outer_item* section*`。見出しを見て節を切る。`module type` と `module X :` は文法に無いので E100 で落ちる
- 名前解決: 契約と実装を別の名前空間に。`implement X as Y` の `X` は契約、`Y.f` の `Y` は実装。top-level は定義順に依存しない（2.2 の相互再帰）ので、`implement` が `module` より前にあってもよい
- E513: 実装の無い契約と、参照されない実装。E204: 契約の無い `implement`。E602 のアンカーは `implement`。短い名前を E514 は `unused-implement-def`、E602 は `contract-mismatch` に
- 外の `def` は入口ファイルなので R16 では常に許す
- 正規形の印字は節形式
- 門番の述語を、厳しさを保って新構文に置き換える。`verify-m3.sh` の 11（stdlib に行頭 `def` が無い）は「stdlib の `def` は全て見出しの後にある（外に `def` が無い）」に。`verify-m7.sh` の signature 検査（`module X : SIG = struct` と `module type SIG = sig` の正規表現）は `^module X$` と `^implement X as X$` の 2 行の在庫照合に。member の宣言と定義が 2 回現れる検査はそのまま

R16 の判定:

- 16-a. 全門番と `cargo test` が通過。R17 に依存する赤は無い
- 16-b. 判定器（`scripts/verify-r16.sh`）: (1) 契約と実装の数と順序を乱数で生成し、parse → 正規形印字 → 再 parse の往復。`implement` が `module` より前でも同じ。(2) `module type X = sig ... end` と `module X : SIG = struct ... end` は E100。(3) 1 つの契約を 2 つの実装が満たし、両方を `main` から呼んで実行結果が判定器側の値と一致。(4) 実装の無い契約と参照されない実装は E513、契約の無い `implement` は E204、契約の本体付きの型は E100、契約の型に実装修飾 `Y.T` を書くと E204、契約に無い抽象型の実体化 `type U = Int` は E204 で `U` を指す、契約と実装の型の不一致は E602 で `implement` を指す。(5) `implement` 節の ADT・record を外から `Y.Entry` と参照すると E204、`Y.T` の field access は E601、契約で修飾した `X.T` は E204、2 つの実装が同名の内側型を持つのは適法、内側型が外の型か契約の抽象型と同名なら E205 で内側を指す。修飾参照の E204 は解決できない最初の名前を指す（`X.T` は `X`、`Y.Entry` は `Entry`）
- 16-c. `grep -rEn '\<(sig|struct|end)\>|module type' stdlib/` が空（`append` のような部分一致は対象外）
- 16-d. 審査合格後に lock を更新
- 16-e. 位置摂動プローブが E602 E514 に当たる

### R17 複数ファイル

仕様は §54 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| 受理の新設 `multi_file/` | `main.hd`（外の record と `main`）、`store.hdi`（外の型と `module Store`）、`memory_store.hd` `file_store.hd`（それぞれ `implement Store as ...`）。`// item: 節構文` | §54 |
| `docs/corpus.md` | 「複数ファイルのケースは同じディレクトリに `.hd` `.hdi` を足す。タグは `main.hd` に置く」を足す | — |

拒否コーパスは §50 の行と 1 対 1 なので、複数ファイルの E100 E205 E513 は判定器のプローブに置く。

実装の注意（決定ではない）:

- CLI: 入口ファイルのディレクトリの `.hd` `.hdi` を全て読む。順序は入口を先、残りをパスのバイト順。`.hdi` に `implement` があれば E100。入口以外の外に `def` があれば E100
- 診断: ファイルのパスを持つ。既存の 1 ファイルの出力形式（行と列）は入口ファイルでは変えない
- lock はケースディレクトリの全ファイルを対象にする（`tests/accept/*/*.hd tests/accept/*/*.hdi tests/reject/*/*.hd tests/reject/*/*.hdi stdlib/*.hd`）

R17 の判定:

- 17-a. 全門番（m0、m1-spec、m1-corpus、m2〜m7、r9、r11〜r14、r16、r17）が終了コード 0、`cargo test` 全件通過
- 17-b. 判定器（`scripts/verify-r17.sh`）: (1) 契約の数、実装の数、ファイルへの配り方を乱数で生成してディレクトリを作り、build して実行結果が判定器側の値と一致。(2) 同じ名前空間の同名の宣言（契約同士、実装名同士、外の型同士）を 2 ファイルに置くと E205 で、アンカーはファイル順（入口、パスのバイト順）で 2 番目。同じ契約を別名の実装で 2 ファイルから満たすのは適法。(3) どこからも参照されない `.hd` の実装は E513。(4) `.hdi` の `implement` と、入口以外の外の `def` は E100 で、診断の先頭がそのファイルのパス、行と列はその `implement` か `def`（`パス:行:列: error[E100]`）。`.hdi` を入口に渡すと CLI が拡張子で拒否。(5) 入口以外のファイルを `hado check` に渡すと、本来の入口のパスと最初の外の `def` の行・列で E100。(6) 無関係な壊れた `.hd` を同じディレクトリに置くとビルドが失敗する。(7) `.hdi` の契約の型に `Y.T` を書くと E204
- 17-c. 審査合格後に lock を更新。lock の行数が `tests/accept` `tests/reject` `stdlib` のディスク上の `.hd` `.hdi` の実数と一致
- 17-d. 位置摂動プローブが、複数ファイルのケースの `main.hd` 以外のファイルでも行と列を保つ

R17 の終了時に `git tag v0.1.2` を打つ。

### スコープ外

11.3 の項目と 6 節の「v0.2 へ保留」。warning の導入。

### 着手時点の実測

2026-09-08、仕様と 11〜12 節を書いた後に全門番を順に回した。

| 門番 | 赤のチェック | 原因 | 戻すラウンド |
|---|---|---|---|
| `cargo test` | `parse_corpus::corpus_counts`、`r9_grammar::grammar_generated_programs_preserve_every_declaration`（2 failed） | §50 が 57 行で拒否が 58 本（E113）。§41 の EBNF から生成した `let p? =` と節形式を現行パーサが読めない | R14（E113、`let p? =`）、R16（節形式） |
| `verify-m0.sh` | 2 cargo-test、3 walk、4 tail（4/7） | `cargo test` の終了コードを見る | 同上 |
| `verify-m1-spec.sh` | 1 headings（5/6） | 見出しの改名（表題 §22 §23 §34 §35）を `git diff` が削除と数える。コミット `d89262d` の後は 6/6（実測） | — |
| `verify-m1-corpus.sh` | 1 reject-1:1（E113 が §50 に無い）、2 sec39（`module UserStore` が受理に無い）、3 pairs（11.2 の 2 項目に受理と拒否が無い）、7 no-regressions（m0 が赤）（3/7） | 仕様先行 | R14（1、3 の let p? =）、R16（2、3 の節構文、7） |
| `verify-m2.sh` | 1 accept-29（`cargo` が exit 101）、2 e1xx-13（E1xx が 14 → 13）、5 gates（5/8） | 同上。初回は 6 protected-files も赤だったが、実走中に文書を編集したためで、編集を止めて回すと 5/8 | R14, R16 |
| `verify-m3.sh` | 9 gates（11/12） | m0 m1-spec m1-corpus m2 が赤 | R16 |
| `verify-r9.sh` | R9-1 字句・全文法の往復（4/5） | §41 の新形式 | R16 |
| `verify-m7.sh` | 3（2/3） | `cargo test` の終了コード | R16 |
| `verify-m4`〜`m6`、`r11`〜`r13` | 無し | — | — |

## 13. v0.1.3 の範囲と決定（2026-09-09）

0.1.2 の仕様を全文読み、独立レビュアー 1 体に「書けない正当なプログラム」と「矛盾する節の対」を探させ、測れるものは 0.1.1 と R16 途中のコンパイラで実測した。致命は無く、重大が 5 件で、全て「冗長を拒否する規則」と「契約が形を固定する規則」の継ぎ目にあった。その 5 件と、検討の中で出た型引数の綴り・Functor・ディレクトリ名前空間を grilling（25 問）で確定した。経緯は `docs/ledger.md` の「0.1.3 の計画」にある。仕様 `docs/spec.md` は本節を反映済み。2.1〜2.3、7 節、9 節、11 節と食い違う箇所は本節が優先する。

判断基準は 7 節、9 節、11 節と同じ。拒否か許容かで迷ったら拒否し、新機能は最小形で入れる。

### 13.1 決定

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | 0.1.2 との切れ目 | 0.1.2 は R17 まで完走して tag。修正は全て 0.1.3 | — |
| Q2 | 契約の値 member | `def A empty: A T` を契約に書け、実装は `def empty = ...` で満たす | §34, §41 |
| Q3 | 使われない実装 | 入口ファイルを 1 ディレクトリに複数置ける。外の `def` はファイル局所。試験用の実装の参照元は「もう 1 つのプログラム」 | §54 |
| Q4 | 使わない引数 | `_` 引数を、契約 member の実装と `fn` だけで許す。それ以外は E519 | §27, §41 |
| Q5 | 型引数付き抽象型 | `type A T` と `type A T = A List`。右辺に現れない型変数は E518 | §10, §34, §35 |
| Q6 | 型引数の順番 | 0.1.3 に入れる | — |
| Q7 | 型式 | OCaml 式の後置。`Int List`、`(Int, String) Result`、左結合 | §10, §41 |
| Q8 | 型定義の左辺 | `type T Option =`、`type (T, E) Result =` | §10, §41 |
| Q9 | 関数の型変数 | `def (A, B) map(...)`。名前の前に置く | §34, §41 |
| Q10 | 関数型 | `Fn(Int, Int => Int)`。括弧必須。`:` を持たない | §37 |
| Q11 | 匿名関数 | `fn(x, y => x + y)`。3 形とも `)` で閉じ、`primary` になる。`def` は変えない | §7, §41 |
| Q12 | 契約の中の `X.T` | 契約修飾。実装がちょうど 1 つならその T、0 か 2 つ以上は E603。契約は実装を名指ししない | §34, §47 |
| Q13 | Functor | 0.1.3 に入れる | §56 |
| Q14 | Functor 定義 | `implement Store as MakeStore(Id: UserId)`。本文は引数の契約だけを見て 1 回検査 | §56 |
| Q15 | Functor 適用 | `implement Store as IntStore = MakeStore(IntId)`。本文は無い | §56 |
| Q16 | 再適用 | 同じ functor に同じ引数列は 2 回目で E604 | §56 |
| Q17 | Functor の未使用 | 未適用は E513、未参照の引数は E521 | §56 |
| Q18 | 名前空間 | 根の下を再帰的に読む。サブディレクトリ 1 つが名前空間 1 つ。区切りは `::` | §55 |
| Q19 | 綴り | 自分のディレクトリは無修飾、他は根からの絶対パス。根は無名でどこからも無修飾 | §55 |
| Q20 | 空間に入るもの | 型・契約・実装・constructor。根との同名は E205 | §47, §55 |
| Q21 | prelude | 変えない | §49 |
| Q22 | 根 | 渡したファイルのディレクトリ。設定ファイル無し | §54 |
| Q23 | 軽微 | `main` の抽象型は E705、Bool の 2 形は E520、他は文書修正 | §5, §14 |
| Q24 | ラウンド | R18 綴り → R19 契約 → R20 複数入口 → R21 Functor → R22 名前空間 → tag | 14 節 |
| Q25 | 版 | v0.1.3 | — |

### 13.2 仕様の項目

コーパスの `item:` タグに使う項目名。2.2、9.2、11.2 の項目に加える。

| 項目 | 仕様の節 | 内容 |
|---|---|---|
| 型の綴り | §10, §37, §41 | 後置の型引数と `Fn(...)`。`<T>` と `Fn<...>` は E100 |
| fn の括弧 | §7, §41 | `fn(x => ...)` と腕形式 2 つ。括弧の無い `fn` は E100 |
| 契約の拡張 | §27, §34, §35 | 値 member、型引数付き抽象型、`_` 引数、契約修飾 `X.T` |
| 複数の入口 | §54 | 入口ファイルの複数化と外の `def` のファイル局所 |
| Functor | §56 | 定義、適用、E604、E521 |
| 名前空間 | §55 | `::`、再帰読み込み、絶対パス、ディレクトリ単位の E205 |

### 13.3 やらないこと

- first-class module（§38）
- 型引数付き抽象型の幽霊型 `type A T = Int`。E518 で拒否し、必要になったら緩める
- 同じ functor に同じ引数列を 2 回適用したときの型の同一性。E604 で拒否するので問題が起きない
- 親ディレクトリの名前の無修飾参照。同じ名前に綴りが 2 つできる
- 根を示す設定ファイルと上方向の探索
- prelude の名前空間化（`std::`）と関数集合の変更
- `f Red (g 1)` の専用診断。E301 のまま。部分的な失敗回復の E508 も変えない

### 13.4 commander が置いた細則（可逆。異議があれば戻す）

- E518 は実体化だけでなく、型定義（`type T Phantom = | Phantom(Int)`）と契約の関数の型変数（`def (A, B) f`）にも当てる。同じ規則「宣言した型変数が右辺に現れない」で 3 か所を揃えた
- E603 は実装が 0 のときも当てる。段階順で型が冗長より先なので、契約の E513 より先に `X.T` が止まる。functor の定義は実装に数えず、適用結果は数える
- `.` の左は、契約の中では契約、それ以外では実装として解く。`implement UserId as UserId` では同じ綴りが位置で意味を変える
- `_` は `parameters` と `lambda_params` の `param` にだけ現れる。第52節の `def f | ...` には引数列が無いので関係しない
- `main` を持たない根のファイルも外の `def` を置ける（ファイル局所、未使用は E513）。渡さなかった入口ファイルの `main` は E513 の免除を保つ
- 適用の見出しの後に本文を置くと E100。functor の引数に functor は書けない（E204）
- 型の印字（`--print-type` と診断）は §41 の正準形で、カンマの後に空白 1 つ。`verify-m3.sh` の型の往復の期待値は判定器側で新しい形を作る
- 単一の型引数を括弧で囲む `(Int) List` は E100。型引数の数の不一致（`Int Bool`）は現行の扱いのまま
- ディレクトリ名が `name` の字句でなければ CLI が拒否する（診断ではない）。`.hdi` の拡張子の拒否と同じ扱い
- 無修飾の名前は自分のディレクトリと根（prelude を含む）の両方で探し、両方にあれば宣言側で E205 にする。解決順で shadowing を作らない
- 13.2 の各項目に拒否コーパスを 1 本ずつ割り当てる。型の綴りは `E100/`（旧綴り）、fn の括弧は `E101/`（腕の中の `fn(| ...)`）、契約の拡張は `E518/` `E519/` `E603/` `E705/`、複数の入口は `E206/`（`main` を持つ別ファイルを添える）、Functor は `E521/` `E604/`、名前空間は `E204/`（親の名前の無修飾参照）
- lock の対象を `find tests/accept tests/reject -type f \( -name '*.hd' -o -name '*.hdi' \)` と `stdlib/*.hd` にする（R22。サブディレクトリを含める）
- E520 は `if b | true => x | false => y`、`if b | true => x | y`、`if | b => x | y`、第52節の `def f | true => x | false => y` と `fn(| true => x | false => y)` に当てる。`def f x | c => a | b` は E517 との関係で正準形のまま

## 14. R18〜R22 の手順書

この節を実行するエージェント向けの手順書。13 節の決定を実行する。R18 から R22 まで表の順に進める。

**読む順:** 本節 → `docs/corpus.md` → `docs/spec.md`（§41 §50 §7 §10 §37 §27 §34 §35 §54 §55 §56 を先に）→ 13 節。表と仕様本文が食い違えば仕様本文が正で、食い違いは報告に書く。
**判定者:** 各工程の成果物は、作成した本人以外が判定基準に沿って審査する。別モデルのエージェントを呼べる環境ならそれに、呼べなければ人に出す。
**門番:** `scripts/verify-*.sh` の全て。着手時点は m0、m1-spec、m1-corpus、m2〜m7、r9、r11〜r14、r16、r17 の 16 本で、R18〜R22 がそれぞれ verify-r18〜r22 を足す。「全門番が通過」は、その時点の全本が終了コード 0 で、`cargo test --offline --no-fail-fast` が 0 failed の意味である。
**仕様は確定済み:** 13 節の決定は仕様に反映済みで、各ラウンドは コーパス → lock → 実装 の順に進む。現行コンパイラは R18 の綴りを parse できないので、R18 のコーパスの根拠は仕様の読みだけである。R19 以降は R18 を終えたコンパイラで旧規則の挙動を確かめられる。
**仕様に疑義が出たら:** 仕様を変えず、解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める。
**過渡状態:** 本節を書いた時点で `docs/` の仕様・対応表は 0.1.3 の語彙、コードとコーパスと stdlib と `README.md` は 0.1.2 の語彙である。R18〜R22 が解消する。
**コミット:** 工程ごとに既存の形式（`test:` `feat:` と日本語の命令形）で。push しない。
**報告:** 完了時と、依存する作業で止まったときに返す。(1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

| 工程 | 成果物 | 判定 |
|---|---|---|
| R18 型の綴りと `fn(...)` | `tests/accept` `tests/reject` `stdlib/`、`crates/hado/**`、`scripts/`、`README.md`、lock | 18-a〜18-d |
| R19 契約の拡張 | `tests/accept` `tests/reject` `stdlib/`、`crates/hado/**`、`scripts/`、lock | 19-a〜19-d |
| R20 複数の入口 | `tests/accept` `tests/reject`、`crates/hado/**`、`scripts/`、lock | 20-a〜20-c |
| R21 Functor | `tests/accept` `tests/reject`、`crates/hado/**`、`scripts/`、lock | 21-a〜21-d |
| R22 名前空間 | `tests/accept` `tests/reject`、`crates/hado/**`、`scripts/`、`docs/corpus.md`、lock | 22-a〜22-d |

**着手時点で赤の門番（実測）:** 本節の末尾「着手時点の実測」に置く。R22 の終了時点で全て green に戻す。門番を緩めて green にすることは禁止。

### R18 型の綴りと `fn(...)`

仕様は §7 §8 §10 §14 §16 §20 §21 §25 §26 §36 §37 §39 §40 §41 §44 §46 §49 §50 §52 §53 に反映済み。このラウンドは綴りの変更と E520 だけで、意味を変えない。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| 受理全件、拒否全件、`stdlib/*.hd` | `Option<T>` → `T Option`、`Result<T, E>` → `(T, E) Result`、`Fn<...>` → `Fn(...)`、`type X<T> =` → `type T X =`、`def f<A, B>(...)` → `def (A, B) f(...)`、`fn x y => e` → `fn(x, y => e)`、`(fn ...)` の外側の括弧を外す。期待するコードと `expect:` は維持 | §10, §37, §41 |
| 受理の新設 `type_spelling/` | 後置の型引数を全部の位置（型定義、契約、実体化、注釈、`Fn` の入れ子）で使い、`main` で実行。`// item: 型の綴り` | §10, §37 |
| 受理の新設 `fn_forms/` | 3 形の `fn(...)` を並置の引数と腕の中で使う。`// item: fn の括弧` | §7, §41 |
| 拒否の新設 `E520/` | `def f b => if b \| true => 1 \| false => 0` と `f` を呼ぶ `main` | §14 |
| `E100/` | 内容を旧綴り `def id(x: Option<Int>): Option<Int> => x` と `def main = id (Some(1))` に差し替え、`// item: 型の綴り` を付ける。比較の連鎖 `1 < 2 < 3` は判定器のプローブへ移す | §10, §41 |
| `E101/` | 腕の中に `fn(\| A => 1 \| B => 2)` を置く形に差し替え、アンカーは `fn`。`// item: fn の括弧` を付ける | §20, §41 |

`README.md` の `--print-type` の例を新しい形に。

実装の注意（決定ではない）:

- lexer: `::` をトークンに足す（R22 まで parser では E100 のまま）。`<` `>` は比較演算子だけになる
- parser: `type_expr` を後置に。`Fn` はキーワード。`lambda` を `primary` へ移し、`arm_body` から `lambda` を外す。`type_vars` を名前の前に
- 正規形の印字と `--print-type` を §41 の形に。E70x と E305 の型の印字も同じ
- `r9_grammar` の生成器を §41 に合わせる。`verify-m3.sh` の型の往復は期待型の文字列を新しい形で組む
- 診断の文言（E508 の `use let p? = or do` など）に型が入る箇所を確認する

R18 の判定:

- 18-a. 全門番と `cargo test` が通過。R19 以降に依存する赤は無い
- 18-b. 判定器（`scripts/verify-r18.sh`）: (0) 比較の連鎖 `1 < 2 < 3` が E100。(1) 型式を乱数で生成し（後置、2 つ以上の括弧、`Fn` の入れ子、抽象型 `Y.T`）、注釈に置いた `def` の parse → 正規形印字 → 再 parse の往復と、`--print-type` の文字列が判定器側の正準形と一致。(2) 旧綴り `Option<Int>`、`Fn<Int, Int>`、`type Option<T> =`、`def f<A>(...)`、`fn x => x`、`(Int) List` は E100 で、アンカーは `<` か `fn` の直後か `(`。(3) `fn(x, y => x + y)` を並置の引数と腕の中に置いて実行結果が判定器側の値と一致し、腕の中の `fn(| ...)` が後続の腕を飲まないこと（E101 のアンカーが `fn`）。(4) E520 の 5 形が全て E520 で、`Some(true)` の入れ子と `def f x | c => a | b` は受理
- 18-c. 審査合格後に lock を更新
- 18-d. 位置摂動プローブが E520 に当たる

### R19 契約の拡張

仕様は §3 §5 §27 §34 §35 §41 §47 §50 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| 受理の新設 `stack/` | `module Stack` に `type A T`、`def A empty: A T`、`def A push(...)`。`implement Stack as Stack` に `type A T = A List`。`main` で `Stack.empty` と `Stack.push` を使う。`// item: 契約の拡張` | §34, §35 |
| 受理の新設 `null_store/` | 契約の引数を `_` で受ける実装と、`List.map xs fn(_ => 0)`。`// item: 契約の拡張` | §27 |
| 受理の新設 `contract_type/` | 契約 `UserId` と契約 `UserStore` を分け、`def find(id: UserId.T)`。実装は 1 つずつ | §34 |
| 拒否の新設 `E518/` `E519/` `E603/` `E705/` | 幽霊型 `type A T = Int`、外の `def f _ => 1`、`UserId` の実装が 2 つあるときの `UserId.T`、`def main = Store.find 1` が `Store.T Option` | §10, §27, §34, §5 |
| `user_store/` | `type T = Error` と `Error(user)` の契約を `def save(_: T)` の形に戻し、§39 の例と同じ意味にする | §27 |

実装の注意（決定ではない）:

- 契約の member に値（引数なし）を足す。実装側の `def name = e` と照合する
- 抽象型に型引数を持たせる。`Abstract(name, args)` の `args` を使う。実体化の展開で型引数を置換する
- `_` 引数: `param` に `_` を許し、束縛を作らない。契約 member の実装と `fn` 以外は E519
- 契約の中の `X.T`: 名前解決で契約 X の実装集合を数える。1 なら `Abstract("Y.T")` に置換、0 か 2 以上は E603。functor の定義は数えない（R21 で確認）
- E705: 単相化で `main` の型に `Abstract` が残れば止める

R19 の判定:

- 19-a. 全門番と `cargo test` が通過。ただし後のラウンドに依存する次の赤は残る: `verify-m1-corpus.sh` の 1（E521 E604 が無い。R21 まで）、2（§39 の functor 例。R21 まで）、3（複数の入口・Functor・名前空間のペア。R22 まで）、7 と、`cargo test` の `corpus_counts` とその終了コードを見る m0 の 2〜4、m2 の 1 と 5、m3 の 9、m7 の 3。これ以外の赤があれば R19 の欠陥
- 19-b. 判定器（`scripts/verify-r19.sh`）: (1) 型引数の数を乱数で決めた抽象型と値 member を持つ契約と実装を生成し、build して実行結果が判定器側の値と一致。数が違えば E602。(2) 契約 member の実装と `fn` の `_` 引数は受理、外の `def` と契約に無い `def` の `_` は E519 で `_` を指す。名前付きの未使用引数は場所によらず E505。(3) 契約 `X` の実装が 0、1、2 のときの `X.T` が E603、受理、E603。(4) `main` の型に抽象型が入る形が E705
- 19-c. 審査合格後に lock を更新
- 19-d. 位置摂動プローブが E518 E519 E603 に当たる

### R20 複数の入口

仕様は §5 §27 §54 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| 受理の新設 `two_entries/` | `main.hd` が `MemoryStore` を、`store_test.hd` が `FileStore` を使う。両方が `main` を持つ。`// item: 複数の入口`。判定器は両方を入口にして実行する | §54 |
| `multi_file/` | 変更なし | — |
| `E206/` | `main.hd` は `main` を持たないまま、`other.hd`（`main` を持つ）を添える。渡した `main.hd` に `main` が無いので E206。`// item: 複数の入口` | §54 |

拒否コーパスは §50 の行と 1 対 1 なので、E100（サブディレクトリの外の `def`）は判定器のプローブに置く。

実装の注意（決定ではない）:

- 外の `def` の可視性をファイル単位に。名前解決の値の空間をファイルごとに持つ。E513 の対象もファイルごと。`main` は渡したファイル以外でも免除
- 渡したファイルに `main` が無ければ E206 で 1:1

R20 の判定:

- 20-a. 全門番と `cargo test` が通過。19-a と同じ赤のうち、13.2 の「複数の入口」のペアだけが緑に変わる。これ以外の赤があれば R20 の欠陥
- 20-b. 判定器（`scripts/verify-r20.sh`）: (1) 入口ファイルの数と実装への参照の配り方を乱数で生成し、各入口を build して実行結果が判定器側の値と一致。(2) 2 つの入口が同名の外の `def` を持っても受理。別ファイルの外の `def` を参照すると E204。(3) `main` を持たない根のファイルの外の `def` はファイル内で使われれば受理、使われなければ E513。(4) `main` の無いファイルを渡すと E206
- 20-c. 審査合格後に lock を更新

### R21 Functor

仕様は §34 §35 §38 §39 §41 §47 §50 §54 §56 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| 受理の新設 `functor/` | §39 の例（`MakeStore(Id: UserId)`、`PlainStore`、`OffsetStore`）。functor の定義行 `implement Store as MakeStore(Id: UserId)` は `verify-m1-corpus.sh` の 2 が `main.hd` を走査するので `main.hd` に置き、契約と適用は別ファイルでよい。`// item: Functor` | §56 |
| 拒否の新設 `E521/` `E604/` | 本文で使わない引数、同じ引数列での 2 回目の適用 | §56 |
| `E602/` | 変更なし。functor の E602（契約違い、引数の契約違い）は判定器のプローブ | §56 |

実装の注意（決定ではない）:

- parser: `functor_section` と `apply_section`。適用の後の項目は E100
- 名前解決: functor の引数を実装の空間にファイル局所で置く。本文の `Id.T` は `Abstract("Id.T")`。適用で引数を実装に束縛し、適用結果を新しい実装として登録する
- 型: 本文を引数の契約で 1 回検査。適用は E602 を契約の照合で出す。E604 は適用の（functor 名、引数名列）の集合で判定
- 単相化: 適用ごとに本文を実体化する。generics と同じ仕組み
- E603 の実装の数に functor の定義を含めない

R21 の判定:

- 21-a. 全門番と `cargo test` が通過。R21 で拒否が 64 本に揃うので `cargo test` と m0 は緑になり、残る赤は `verify-m1-corpus.sh` の 3（名前空間のペア）と、それを見る m2 の 5 と m3 の 9 だけ。これ以外の赤があれば R21 の欠陥
- 21-b. 判定器（`scripts/verify-r21.sh`）: (1) 引数の数と適用の数を乱数で生成し、build して実行結果が判定器側の値と一致。適用結果の `T` 同士が別の型であること（混ぜると E305）。(2) 契約違いの見出し、引数の契約違い、引数に functor は、それぞれ E602 E602 E204。(3) 同じ引数列の 2 回目は E604 で 2 回目の `implement` を指し、引数の順序が違えば受理。(4) 未適用の functor は E513、未参照の引数は E521、`MakeStore.create` は E204、`.hdi` の functor は E100、適用の後の本文は E100。(5) 契約 `X` の functor 定義が 1 つと実装が 1 つのとき `X.T` は受理（functor を数えない）
- 21-c. 審査合格後に lock を更新
- 21-d. 位置摂動プローブが E521 E604 に当たる

### R22 名前空間

仕様は §41 §47 §54 §55 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| 受理の新設 `namespaces/` | `main.hd`、`store/store.hdi`、`store/memory_store.hd`、`store/backend/disk.hd`、`id/int_id.hd`。絶対パス、無修飾、`::` 付き constructor のパターン。`// item: 名前空間` | §55 |
| `docs/corpus.md` | 「ケースのディレクトリにサブディレクトリを置ける。タグは `main.hd` に置く」を足す。lock の対象を 13.4 の `find` に | — |

`E204/` は、`store/store.hdi` に契約を置き、`store/backend/disk.hd` が `implement Store as Disk` と親の名前を無修飾で参照する形に差し替え、`// item: 名前空間` を付ける。E205（根との同名）は判定器のプローブに置く。

実装の注意（決定ではない）:

- CLI: 根の下を再帰的に読む。ディレクトリ名が `name` の字句でなければ CLI が拒否。ファイルの順序は入口を先、残りを根からの相対パスのバイト順
- parser: `path` を `type_ref`、`contract_ref`、`impl_ref`、実装の値、constructor（式とパターン）に
- 名前解決: 型・契約・実装・constructor の 4 空間をディレクトリごとに持つ。解決は「自分のディレクトリ → 根」の順で、両方にあれば E205（宣言側で先に検出する）。パスは根から辿る。親や兄弟は無修飾で見えない
- 診断のパスは根からの相対パスのまま

R22 の判定:

- 22-a. 全門番（m0、m1-spec、m1-corpus、m2〜m7、r9、r11〜r14、r16〜r22）が終了コード 0、`cargo test` 全件通過
- 22-b. 判定器（`scripts/verify-r22.sh`）: (1) ディレクトリの木と契約・実装・型の配り方を乱数で生成し、build して実行結果が判定器側の値と一致。(2) 親・兄弟の無修飾参照は E204、絶対パスは受理、`store::MemoryStore` を `store/` の中で書いても受理（無修飾と同じ意味かは問わず、E204 にならないことだけ）。(3) 別ディレクトリの同名（型、契約、実装、constructor）は受理、同じディレクトリの同名は E205、根と同名のサブディレクトリの宣言は E205 でサブディレクトリ側を指す。(4) `name` の字句でないディレクトリ名は CLI が拒否。(5) サブディレクトリのファイルを渡すとそのディレクトリが根になり `main` が無ければ E206。(6) 診断のパスが根からの相対パス
- 22-c. 審査合格後に lock を更新。lock の行数が `find` の実数と一致
- 22-d. 位置摂動プローブが、サブディレクトリのファイルでも行と列を保つ

R22 の終了時に `git tag v0.1.3` を打つ。

### スコープ外

13.3 の項目と 6 節の「v0.2 へ保留」。warning の導入。

### 着手時点の実測

2026-09-09、仕様と 13〜14 節を書いた後に全門番を順に回した（`CARGO_TARGET_DIR=/tmp/hado-v013/target`、ログは `/tmp/hado-v013/gates/`）。

| 門番 | 赤のチェック | 原因 | 戻すラウンド |
|---|---|---|---|
| `cargo test` | `parse_corpus::corpus_counts`、`r9_grammar::grammar_generated_programs_preserve_every_declaration`（2 failed） | §50 が 64 行で拒否が 57 本。§41 の EBNF から生成した後置の型と `fn(...)` を現行パーサが読めない | R18（EBNF、E520）、R19〜R22（残りのコード） |
| `verify-m0.sh` | 2 cargo-test、3 walk、4 tail（4/7） | `cargo test` の終了コードを見る | 同上 |
| `verify-m1-spec.sh` | 1 headings（5/6） | 表題の改版（`v0.1.2` → `v0.1.3`）を `git diff` が削除と数える。コミット後は 6/6 | — |
| `verify-m1-corpus.sh` | 1 reject-1:1（E518〜E521 E603 E604 E705 が無い）、2 sec39（§39 の functor 例が受理に無い）、3 pairs（13.2 の 6 項目に受理と拒否が無い）、7 no-regressions（m0 が赤）（3/7） | 仕様先行 | R18〜R22 |
| `verify-m2.sh` | 1 accept-32（`cargo` が exit 101）、3 non-e1xx-51、5 gates（5/8） | 同上 | R18〜R22 |
| `verify-m3.sh` | 9 gates（11/12） | m0 m1-spec m1-corpus m2 が赤 | R22 |
| `verify-r9.sh` | R9-1 字句・全文法の往復（4/5） | §41 の新形式 | R18 |
| `verify-m7.sh` | 3（2/3） | `cargo test` の終了コード | R18 |
| `verify-m4`〜`m6`、`r11`〜`r14`、`r16`〜`r17` | 無し | — | — |

R18〜R22 で green に戻す。


## 15. R21追加決定: 名前付きfieldのconstructor（2026-09-09）

ユーザー承認: `type T = Payload { id: Id.T; name: String }` のTは型名、Payloadはconstructor名。位置payloadを `()`、名前付きfieldを `{}` で直接書き、値生成とパターンにも同じconstructor名を使う。§35/47と§39/56の旧record例の疑義は、局所record＋型別名への回避ではなく、この構文とconstructor付き抽象型実体化の追加で解消する。

この工程をR21の完了判定より前に挿入する。仕様→独立審査→コーパス候補の独立審査→lock→判定器のred→実装→全門番とCursor実走審査の順とする。成果物は `docs/spec.md`、本計画、`docs/spec-coverage.md`、`docs/ledger.md`、受理/拒否コーパス、lock、`crates/hado/**`、`scripts/`。既存record全件の構文移行は要求しない。

判定条件:

1. 単一constructorの省略形、ADT列の名前付き/位置payload混在、型引数、生成・印字の往復を検証する。
2. field順を変えて生成し、書かれた順の評価と宣言順のfield対応を独立した期待値で確認する。同じfield集合の別型を混同しない。
3. constructor付きパターンの同名束縛・別名・省略・入れ子、分岐網羅性とlet全域性のE402を検証する。
4. 未知field E204、重複E205、不足E301、型不一致E305、位置/名前形式の取り違えE301をcheck/build/dump-monoの位置摂動で確認する。
5. 単一constructorのfield参照、複数constructorの直接field拒否、公開抽象型の不透明性E601とconstructor非公開E204を検証する。
6. §39/56のFunctor例を新構文へ揃え、異なる適用の型の分離と全R21判定器を通す。新規判定器 `verify-record-constructors.sh` を全門番に含める。

R21の基礎実装は `2a7b1ef`。Cursor再審査は既知旧record疑義を除く範囲でPASS。全門番はまだ赤であり、R21完了を意味しない。追加工程の終了後、R22とv0.1.3の全条件へ戻る。

## 16. v0.2 の I/O と `do`（2026-09-09）

6 節「v0.2 へ保留」の「効果と I/O を入れるときの、値を捨てる `do`（7 節 Q23）」と、11.3 の同項目への細則「入れるときに別の綴りを決める」は、本節が優先する。

標準出力は常に存在するとは限らない。現在の runtime は書き込みの失敗を捨てている。`hado_print_string`（`runtime/hado_runtime.c:55-59`）は `fwrite` と `fflush` の戻り値を見ない。`hado build` した実行ファイルを `>&-` で走らせると、何も出力せずに終了コード 0 を返す。§42 の「例外より値」に反する状態が既にある。

### 16.1 決定

| ID | 論点 | 決定 |
|---|---|---|
| Q1 | I/O の型 | プログラムの外の環境に依存する操作は Option か Result を返す。失敗しない I/O は作らない |
| Q2 | `do` の意味 | 「Option / Result の成功値を捨て、失敗を伝播する」の 1 つに固定する。第二の意味を持たせない |
| Q3 | I/O の書き方 | 全ての I/O は `let p? =` か `do` を通る。値を使わない I/O は `do` を必須とする |
| Q4 | 6 節の保留項目 | 「値を捨てる `do`」は不要。効果の破棄というカテゴリが生まれないので、E506 に例外を作らない |
| Q5 | 出力関数の綴り | 未決。`print_line` は stdlib の命名（`String.concat`、`Int.to_string`）に比べて長い |
| Q6 | `let` と `do` の綴り | 両方残す。キーワードを外して `x = e ->`、`e -> body` と書く案は採らない |

### 16.2 決定が依存している性質

Q1 と Q2 は、Result 型の値を条件位置に置けないことに依存する。`if say "x" => n | n + 1` は E305 で、条件列でも同じである。§30 の E502 は `if c => v | v` の canonical form として条件の評価ごと落とすので、条件に I/O を書けると効果が消える。I/O が Result を返す限り、§14 の「条件は Bool」がこの経路を塞ぐ。

コンパイラは共通部分式除去を持たないので、同じ呼び出しを 2 回書けば 2 回実行される。

### 16.3 申し送り

optimizer を入れるとき、型から純粋性を仮定してはならない。`(Int, ClockError) Result` と `(Int, DivisionError) Result` は型で区別できないが、前者は 2 回呼べば違う値を返し、後者は同じ値を返す。非決定的な呼び出しの識別方法を、最適化を入れる時点で決める。

### 16.4 Q6 の根拠

`do` は Q2 と Q3 から残る。

外すと文法規則が要る。`do` だけで 3 本（`->` の右結合と値位置での括弧強制、`expression := pipe ["->" expression]` への書き換え、宙に浮いた `->` の解決）、`let` だけで 3 本（式からパターンへの再解釈、`=` を走査するときの括弧深度スコープ、`arm_body` の書き換え）、両方で 6 本以上になる。`_` は式の atom に無いので `Pair(x, _) = e ->` は式として読み切れない。LL(1) の 1 トークン判別（`crates/hado/src/parser.rs:1066`）を失い、診断の位置が劣化する。テストは診断の位置を 101 箇所のアサーションで固定している（`crates/hado/tests` の `assert` 行のうち `.line` か `.col` を比較するもの）。拒否コーパスの判定器（`crates/hado/tests/corpus.rs:85-87`）はコードだけを比べ、位置を見ない。

得られるのは `tests/accept` と `stdlib` の 61 ファイル 13,950 文字のうち 58 文字である。ただしコーパスは仕様網羅用で、§39 の ROP の例では `create_user` が本体 5 行中 4 行、`delete_user` が 4 行中 3 行を `let` と `do` が占める。

`let` が冗長でない位置が実在する。

- `tests/accept/tail_closure/main.hd:3` — `|` の直後で `=` と `=>` が 1 文字違いで別の意味になる
- `tests/accept/named_record/main.hd:5` — `Payload { id = n }` は分解にも構築にも妥当
- `tests/accept/rail_forms/main.hd:8` — 行が `{` で開き、record リテラルとして読み始める

## 17. v0.2 の範囲と決定（代数エフェクト、2026-09-09）

`Result` 型を廃止し、Koka 系の代数エフェクトを入れる。grilling（20 問）で確定した。経緯は `docs/ledger.md` の「0.2 の計画」にある。

§38「入れないもの」の `effect system`、§1 の思想の「Explicit failure with `Option` / `Result`」と「Railway Oriented Programming by default」、および 16 節全体は、本節が優先する。判断基準は 7 節・9 節・11 節・13 節と同じで、拒否か許容かで迷ったら拒否し、新機能は最小形で入れる。

仕様への反映は R23 以降で行う。

### 17.1 決定

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | 動機 | 純粋性を型に載せる。(a) 効果を持つ式が冗長規則で消えないこと、(b) 最適化が型から純粋性を読めること。16.2 の「E502 が条件の評価ごと落とすので効果が消える」経路は、今は「I/O が Result を返し §14 が条件を Bool に限る」という間接的な経路でしか塞がれていない | §1, §42 |
| Q2 | 廃止の範囲 | 失敗は効果にする。`Option` はデータ型として残し、`Result` は廃止する | §21, §38 |
| Q3 | 効果の深さ | 一般 `ctl`（1 回再開）まで。multi-shot は入れない | 新節 |
| Q4 | `resume` の形 | handler 節内限定のキーワード。値化・保存・返却は禁止し、節内なら位置は問わない。全ての再開が scoped になり、静的 evidence（EPT）が使える | 新節 |
| Q5 | 種別の位置 | 操作の種別は効果宣言で固定する。「この呼び出しが yield しうるか」が呼び出し位置ごとに静的に決まる | 新節 |
| Q6 | 種別と §48 | 種別は 3 つ（18 節 Q12 で `reply` / `abort` / `capture` と綴る）。§48 は「`capture` を経由しない末尾呼び出しはスタックを消費しない」に限定する。`capture` の下では末尾呼び出しが失われ、スタックを反復回数に線形に消費する（17.8 の実測） | §48, 新節 |
| Q7 | Option の境界 | 理由の有無で分ける。Option は値の不在、効果は理由のある失敗。今の Option / Result の区別をそのまま使う | §21 |
| Q8 | `?` と `do` | 効果的な呼び出しの束縛に印を付けない。`?` は Option 専用の見分け札として存続し、`do` は「効果を持つ式の値を捨てて継続する」に意味を移して存続する | §22, §23, §24 |
| Q9 | handler の形 | §34 / §35 と同じ節構文。`effect E` が契約側、`handler E as H` が実装側で、使用は `with H -> rest`。宣言なので §20 の branch のネスト禁止に触れず、handler 節の中で `if` が書ける | 新節, §34, §35 |
| Q10 | `main` と印字 | `main` に残せるのは組み込み効果だけ。raise された値は stderr で終了コード 1、正常値は stdout で 0 とし、観測される振る舞いを現行のまま保つ。ユーザー定義効果が残れば新しい E コード | §5, §51 |
| Q11 | 効果多相 | 入れる。`Fn` の型に効果行を持たせ、単相化で実体化する | §37, §10 |
| Q12 | 効果行の綴り | 後置の行。区切りと効果名は 18 節が定める（`Int {Error}`、`Fn(A => B {E})`） | §37, §41 |
| Q13 | 省略 | 効果行だけ結果型と独立に省略できる | §8, §9 |
| Q14 | 省略の意味 | 省略は暗黙の効果変数（効果多相）。契約でも外の `def` でも同じ意味とし、純粋の主張には `Pure` を書く | §34, §9 |
| Q15 | `capture` を含むループ | 継続の表現は short-cut resumption とする。ただしこれは実行時間の最適化であって、§48 の保証は回復しない（17.8 の実測） | §48, 新節 |
| Q16 | 操作の名前空間 | 値空間に無修飾で入れる（`raise error`）。`compare` と同じ扱いで、名前空間は 5 つのまま増やさない | §47 |
| Q17 | 進め方 | 未確認をプロトタイプで潰してから、効果を足し `Result` を最後に撤去する。各ラウンドで門番を green に保つ | 手順書 |
| Q18 | 版 | v0.2 | — |
| Q19 | prelude の `Pure` | 純粋な 9 本に `Pure` を書く。高階の 5 本（`List.map` `filter` `fold` `sort`、`Option.map`）は省略のまま効果多相にする | §49 |
| Q20 | 効果を書ける位置 | 効果を持つ式を書けないのは分岐の条件だけとする。scrutinee・適用の引数・演算子の被演算子・record field は許し、評価順を §44 が左から右と定める（2 巡目と 3 巡目の審査で 2 段階に絞った。下記 17.3） | §14, §15, §24, §44 |

### 17.2 種別ごとの保証

| 種別 | `resume` | §48 の保証 | コード生成 |
|---|---|---|---|
| `reply` | 呼び出し位置で即時 | あり | その場で評価し、yield しない |
| `abort` | 不可（中断のみ） | あり | tag 検査と分岐。現行の Result と同形 |
| `capture` | 節内のどこでも 1 回 | なし | short-cut resumption。継続を捕まえるため末尾呼び出しが失われ、スタックを反復ごとに約 330 バイト消費する |

`raise` は `abort` なので、最も頻出する効果が最も安いパスを通る。種別の綴りは 18 節 Q12 が定める。

### 17.3 Q20 の根拠

効果を入れると `is_ready ()` が `Bool [Io]` を返せるようになり、条件位置に書けてしまう。§30 は「Boolean branch の両方の結果が同一なら、条件自体が意味を持たないため、branch 全体を削除する」（E502）と定めるので、`if is_ready () => x | x` に対して効果を持つ呼び出しの削除を要求することになる。条件列では E403 と E406 も条件を見るため、`if | read_flag () => a | read_flag () => b` という別々の 2 つの呼び出しが重複条件として拒否される。

Q20 はこれを構文で塞ぐ。

**2 巡目の審査による修正。** 当初は引数位置と演算子の被演算子も禁じ、「効果を持つ呼び出しは必ず行頭に現れる」を利点として挙げていた。しかし `/` と `%` は `Int {Error}` を返す（18 節 Q16）ので、被演算子を禁じると `x / y + 1` が E308 になり、除算を含む算術が書けなくなる。

禁止していた位置は、性質の違う理由で並んでいた。分岐の条件は E502 と E406 の前提（条件が副作用を持たない）を守るためで、禁止しなければ直せない。引数と被演算子は評価順が未規定だったためで、順序を定めれば済む。scrutinee は 3 巡目の審査で根拠が無いと分かった。scrutinee を落としたり混同したりする冗長規則が 1 つも無く、E507 は scrutinee をそのまま残し、E403 と E405 は腕を落とすだけである。結果として禁止は分岐の条件だけに残り、引数と被演算子は §44 の評価順で解き、scrutinee は許した。

失うのは「効果を持つ呼び出しが必ず行頭に現れる」性質である。これは Q8 で印を外したことを後から補う副産物であって、Q20 の目的ではなかった。

### 17.4 機械的に決まること

- `/` の戻り型は `(Int, DivisionError) Result` から `Int {Error}` へ（18 節 Q16 で `DivisionError` を廃止した）
- E509（ok-unit-parens）は Result と共に消滅する
- E504 / E507 / E508 は Option 限定に縮小して存続する
- E303 は `?` 側のみ存続する（右辺は Option）。`do` の右辺は「空でない効果行を持つ式」を要求する新しい規則になり、純粋な式を置いたら §28 の値の破棄と同じ扱いになる
- `main` の効果行は一般化せず、本体の効果で閉じる。Q10 の検査に必要で、Q14 の唯一の例外
- `Option.ok_or` は `(A, E) Result` から `A {Error}` へ。値の不在を理由のある失敗へ持ち上げる、Q7 の境界をまたぐ唯一の prelude 関数になる
- `Result.map` `map_error` `is_ok` は消滅し、契約内の `def` は 18 本から 15 本になる
- Q20 の違反には新しい E コードが要る。効果は型の一部なので型段階（E3xx）に置き、handler の契約不一致は §35 の E602 と同類なので E6xx に置く

### 17.5 やらないこと

- multi-shot resumption（非決定性）
- 第一級の resumption。shallow handler と、継続を別の handler の下へ運ぶ使い方
- 効果のための 6 つ目の名前空間
- 効果行の `<...>` 綴り（13.2 が `<T>` を E100 に予約済み）と `{}` 単独（R21 の record constructor と衝突する）
- `Result` 型の存続

### 17.6 未確認

1 は 17.8 で解決した。2 は残る。

2. short-cut resumption の prompt / under フレーム復元が、hado の設定（scoped、1 回限り、値化禁止）でも必要か

### 17.7 一次資料

Ningning Xie and Daan Leijen. *Generalized Evidence Passing for Effect Handlers: Efficient Compilation of Effect Handlers to C.* Proc. ACM Program. Lang. 5, ICFP, Article 71 (2021). <https://doi.org/10.1145/3473576>

- p.71:2 項 4 — monadic 翻訳は特別なランタイムを要求せず、C / LLVM / WASM 等に直接コンパイルできる
- p.71:11 §2.10 — 「any direct tail-recursive calls are no longer directly tail-recursive as they occur under a lambda now!」。bind-inlining と join-point sharing による Pure 分岐の fast path で末尾再帰が保たれる
- p.71:12 §2.11 — 「all effectful functions return a monadic result, either Pure or Yield」
- p.71:12 §2.12 — EPT は scoped resumption に限られ、範囲外は実行時エラーになる
- p.71:10 §2.8 — short-cut resumptions。継続をフレーム列として持ち、末尾から 1 つずつ畳む。Koka が採用している

### 17.8 実測（17.6 の 1、2026-09-09）

Koka 3.2.3 で、`capture` に相当する `ctl` 操作を含む末尾再帰ループを走らせた。測定は docker の ubuntu 22.04、公式インストーラ。プログラムは `docs/ledger.md` の「0.2 のプロトタイプ」に置いた形。

| ケース | N | 結果 | 最大 RSS |
|---|---|---|---|
| 効果なしの末尾再帰 | 10,000,000 | 完走 | 4,744 KB |
| tail-resumptive な操作を含む末尾再帰 | 10,000,000 | 完走 | 4,624 KB |
| `ctl` 操作を含む末尾再帰 | 30,000 | 完走 | — |
| 同上 | 50,000 | SIGSEGV | — |

スタック上限を外すと全て完走し、消費は線形になる。N=20,000 で 10,316 KB、100,000 で 37,752 KB、1,000,000 で 333,288 KB。基準 4.6 MB を引くと約 330 バイト / 反復で、既定スタック 8 MB では約 3 万反復が上限である。

分かったこと。

- 消費先は**ヒープではなく C スタック**である。`--ulimit stack=-1:-1` を付けると全て完走するので、束縛条件がスタックだと確定する。
- `resume` を末尾位置に置いても救われない。handler 側ではなく、操作を起こす側の末尾呼び出しが bind のラムダの下に入ることが原因である（17.7 の p.71:11）。
- short-cut resumption はこの問題を解かない。Koka は実装済みで（17.7 の p.71:3）この結果である。論文の「minimal stack space」は定数の意味ではない。

`reply` に相当する tail-resumptive 操作は効果を持たないループと同じ挙動で、§48 の保証はこの種別で完全に成り立つ。

## 18. v0.2 の構文（2026-09-09）

17 節の決定を仕様本文に落とすため、未決だった構文を grilling（19 問）で確定した。経緯は `docs/ledger.md` の「0.2 の構文」にある。17 節と食い違う箇所は本節が優先する。

### 18.1 決定

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | 失敗の効果の置き場 | stdlib に置く。`module` ではなく `effect` として宣言する | §49, 新節 |
| Q2 | 効果名 | `Error` | §49 |
| Q3 | 行の順序 | 意味を持たない | §37 |
| Q4 | 行の重複 | 拒否する。ただし効果変数の実体化で生じた重複は診断せず潰す | §37, §50 |
| Q5 | 純粋の主張 | 後置の `Int Pure` | §37 |
| Q6 | 効果変数 | 宣言しない。常に推論する | §9, §34 |
| Q7 | 書く義務 | 無い。`Pure` も具体行も任意で、書けば「ちょうどそれだけ」を意味する | §8, §9 |
| Q8 | 行の出現位置 | 矢印の結果位置だけ（`return_type` と `Fn` の結果）。`type X = ...` の右辺には現れない | §37, §41 |
| Q9 | 区切り記号 | 波括弧。`Config {Io, Error}` | §37, §41 |
| Q10 | `return` 節 | 持たせる。handler が本体の型を変えられる | 新節 |
| Q11 | `Error` の型引数 | 持たない。payload は `type Exception = { message: String }`。他の効果は型引数を取れる | §49, 新節 |
| Q12 | 操作の種別の綴り | `reply` / `abort` / `capture`。継続に何が起きるかを名指しする | 新節 |
| Q13 | `abort` の戻り型 | 書かせない。再開しないので戻り値が存在しない | 新節, §50 |
| Q14 | handler の未使用引数 | `_` を許す（13.1 Q4 の範囲を handler 節へ広げる） | §27 |
| Q15 | `Error` の印字 | payload の `message` field だけを stderr へ出す | §51 |
| Q16 | `DivisionError` | 廃止する。`/` と `%` は `Error` を raise する | §36, §44, §49 |
| Q17 | `with` の位置 | `prefix_form`。`let` `do` と同じ継続の形 | §41 |
| Q18 | `resume` | `capture` の腕の中でだけ書ける primary | §41, 新節 |
| Q19 | 行の正準順 | アルファベット順。`{Error, Io}` | §37, §41 |

### 18.2 構文の確定案

```ocaml
// stdlib/error.hd
type Exception = { message: String }
effect Error
abort raise(error: Exception)
```

失敗を Option に落とす:

```ocaml
handler Error as ToOption
abort raise _ => None
return value => Some(value)

def main =>
  with ToOption ->
  Int.parse "42"
```

`main` の型は `Int Option`、出力は `Some(42)`。

生成器:

```ocaml
effect A Yield
capture yield(value: A): Unit

handler A Yield as Collect
capture yield value =>
  let rest = resume () ->
  Cons(value, rest)
return _ => Nil

def rec each(values: Int List): Unit {Int Yield}
| [] => ()
| [head, ...tail] =>
    do yield head ->
    each tail

def main =>
  with Collect ->
  each [1, 2, 3]
```

`main` の型は `Int List`、出力は `[1, 2, 3]`。`resume ()` の後ろに `Cons(value, rest)` があるので `capture` が要る。§48 の保証を失うのはこの種別だけで、`{Int Yield}` という行から呼び出し側が静的に判別できる。

即答:

```ocaml
effect Ask
reply ask: String

handler Ask as Fixed
reply ask => "there"
```

### 18.3 文法の差分（§41）

```text
section         := module_section | implement_section | functor_section | apply_section
                 | effect_section | handler_section

effect_section  := "effect" type_vars? upper_name effect_item*
effect_item     := op_kind name "(" contract_param ("," contract_param)* ")" [":" type_expr]
                 | op_kind name [":" type_expr]
op_kind         := "reply" | "abort" | "capture"

handler_section := "handler" contract_ref "as" upper_name handler_item*
                 | "handler" contract_ref "as" upper_name "(" functor_param ("," functor_param)* ")" handler_item*
handler_item    := op_kind name param* "=>" expression
                 | "return" param "=>" expression

prefix_form     := binding | rail_binding | rail_discard | branch | with_form
with_form       := "with" impl_ref "->" expression
primary         := ... | "resume"

return_type     := ":" type_expr effect_row?
type_atom       := ... | "Fn" "(" type_expr ("," type_expr)* "=>" type_expr effect_row? ")"
effect_row      := "{" type_expr ("," type_expr)* "}"
                 | "Pure"
```

キーワードは `effect` `handler` `with` `resume` `return` `reply` `abort` `capture` `Pure` の 9 個を足し、14 個から 23 個になる。

`abort` は戻り型を書けない。文法では表せないので本文で定める。

### 18.4 仕様の項目

コーパスの `item:` タグに使う項目名。2.2、9.2、11.2、13.2 の項目に加える。

| 項目 | 仕様の節 | 内容 |
|---|---|---|
| 効果の宣言 | §57 | `effect` 節、`reply` / `abort` / `capture`、型引数、0 引数の操作 |
| handler と with | §58 | `handler` 節、`return` の腕、`resume`、`with H -> rest`、引数付き handler |
| 効果行 | §37, §41 | 波括弧の行、`Pure`、省略と推論、アルファベット順の正準形 |
| 失敗の効果 | §21, §49 | `Error` と `Exception`、Option との境界、`Option.ok_or` |
| 効果の位置 | §24, §44 | 効果を持つ式を書けないのは分岐の条件だけ。scrutinee・引数・被演算子・record field は許し、評価順を §44 が定める |
| 末尾呼び出しの限定 | §48 | `capture` を経由しない呼び出しだけを保証する |

### 18.5 新しいエラーコード

| 段階 | 検査 | 由来 |
|---|---|---|
| 構文 | `abort` の操作に戻り型を書いた | Q13 |
| 構文 | `resume` を `capture` 以外の腕で書いた。書ける場所は構文的な入れ子だけで決まる | Q18 |
| 型 | 効果を持つ式を分岐の条件に置いた | 17 節 Q20 |
| 型（契約） | handler が効果の操作を実装していない、または効果に無い操作を書いた | 17 節 Q9 |
| 冗長 | 効果行に同じ効果を 2 回書いた | Q4 |
| 単相化 | `main` にユーザー定義効果が残った | 17 節 Q10 |

既存コードの変更は 17.4 の 3 件に加えて、E513 を effect と handler に広げる 1 件である。

## 19. R23〜R27 の手順書（2026-09-10）

この節を実行するエージェント向けの手順書。17 節と 18 節の決定を実行する。R23 から R27 まで表の順に進める。

**読む順:** 本節 → `docs/corpus.md` → `docs/spec.md`（§21 §22 §23 §24 §37 §41 §48 §50 §57 §58 を先に）→ 17 節 → 18 節。表と仕様本文が食い違えば仕様本文が正で、食い違いは報告に書く。
**判定者:** 各工程の成果物は、作成した本人以外が判定基準に沿って審査する。別モデルのエージェントを呼べる環境ならそれに、呼べなければ人に出す。
**門番:** `scripts/verify-*.sh` の全て。「全門番が通過」は、その時点の全本が終了コード 0 で、`cargo test --offline --no-fail-fast` が 0 failed の意味である。
**仕様は確定済み:** 17 節と 18 節の決定は仕様に反映済みで、各ラウンドは コーパス → lock → 実装 の順に進む。
**ラウンド名は主題である:** 検査段階とは一致しない。R24「型」が冗長段階の E522 を実装するように、1 つのラウンドが複数の段階の検査を持つことがある。段階順は §50 が定め、ラウンドの分割はそれと独立である。
**仕様に疑義が出たら:** 仕様を変えず、解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める。
**コミット:** 工程ごとに既存の形式（`test:` `feat:` と日本語の命令形）で。push しない。
**報告:** 完了時と、依存する作業で止まったときに返す。(1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

**過渡状態:** 本節を書いた時点で `docs/spec.md` は 0.2 の語彙、コードとコーパスと `stdlib/` と `README.md` と `docs/spec-coverage.md` `docs/spec-trace.md` `docs/corpus.md` は 0.1.3 の語彙である。R23〜R27 が解消する。README の実行例（`--print-type` が `(String, String) Result` を出す）は現行の実装を正しく記述しており、R25 で移す。

**着手時点で赤の門番:** 7 本。内訳と理由は `docs/ledger.md` の「0.2 の仕様改訂」にある。R27 の終了時点で全て green に戻す。門番を緩めて green にすることは禁止。ただし `verify-m1-spec.sh` の検査 3 は `DivisionError` の存在を要求する v0.1 期の改名確認で、その型を廃止した以上もう緑にならない。これは緩和ではなく役目を終えた検査の削除として R25 で落とす。

**17 節 Q17 の訂正:** 決定の第 2 句「各ラウンドで門番を green に保つ」は成り立たない。判定器が仕様から期待値を導出するため、仕様を全面改訂した時点で全ラウンドが赤から始まる。0.1.2 と 0.1.3 と同じ形である。決定の第 1 句（プロトタイプを先に、Result の撤去を後に）は保つ。

| 工程 | 成果物 | 判定 |
|---|---|---|
| R23 構文 | `crates/hado/src/{lexer,parser}.rs`、`tests/reject`、`scripts/`、lock | 23-a〜23-d |
| R24 型 | `crates/hado/src/{infer,types,check}.rs`、`crates/hado/src/{names,namespaces}.rs`、`tests/accept` `tests/reject`、`scripts/`、lock | 24-a〜24-f |
| R25 失敗の効果 | `stdlib/`、`tests/accept` `tests/reject`、`crates/hado/**`、`scripts/`、lock | 25-a〜25-e |
| R26 種別ごとのコード生成 | `crates/hado/src/{mono,emit}.rs`、`runtime/`、`tests/accept` `tests/reject`、`scripts/`、lock | 26-a〜26-f |
| R27 仕上げ | `tests/`、`scripts/`、`docs/`、`README.md`、lock、tag | 27-a〜27-c |

### R23 構文

仕様は §40 §41 §57 §58 に反映済み。このラウンドは字句と構文だけで、型は見ない。

- 9 個のキーワード（`effect` `handler` `with` `resume` `return` `reply` `abort` `capture` `Pure`）を字句に足す
- `effect_section` `handler_section` `with_form` を節と式の文法に足す
- `resume` を primary に足す
- `result := type_expr effect_row?` と `effect_row := "{" ... "}" | "Pure"` を型式に足す
- 拒否コーパスに `E123/`（`abort` の操作に戻り型）と `E124/`（`capture` の外の `resume`）を新設する。どちらも構文段階なので、このラウンドで完結する

判定 23-a: `verify-r9.sh` が緑に戻る。§41 の EBNF から生成したプログラムが往復する。生成器は `abort` の操作に戻り型を付けず、`resume` を `capture` の腕の外に置かない。E123 と E124 は文法では表せず本文が定めるので、生成対象から外す。
判定 23-b: E1xx の本数が §50 の表の行数と一致する（`verify-m2.sh` の検査 2）。
判定 23-c: §57 と §58 の例が全て parse できる。E123 のアンカー（`abort` の操作の `:`）と E124 のアンカー（`resume`）を位置摂動プローブで確かめる。
判定 23-d: `verify-r23.sh` を新設し、上記を判定する。

### R24 型

- 効果行の推論と単一化を `infer.rs` に足す。行は集合として扱い、順序を持たない
- 効果変数は宣言せず、一般化のときに暗黙に量化する
- `Pure` を空の行として扱う。書いた行はその通りに閉じる
- E305 を効果の不一致にも当てる
- E308（効果を持つ式の位置）、E522（行の重複）、E605（handler の過不足）を実装する
- 拒否コーパスに `E308/` `E522/` `E605/` を新設する。`E303/` の狭めは R25 に置く（prelude がまだ Result を使うため）

判定 24-a: `--print-type` が効果行をアルファベット順で印字し、書き戻せる。
判定 24-b: 省略・`Pure`・具体行の 3 状態が区別される。省略した関数が効果を持てること、`Pure` と書いた関数が効果を持てば E305 になることを確かめる。E305 は既存コードなので拒否コーパスを増やさず、`verify-r24.sh` の判定に置く。
判定 24-c: 効果多相が通る。`List.map` に効果を持つ関数を渡し、呼び出し側の行に伝わることを確かめる。
判定 24-d: E308 E522 E605 のアンカーを位置摂動プローブで確かめる。
判定 24-e: 効果の操作を宣言したディレクトリの外から書くと E204 になり、handler を効果と別のディレクトリに置いても E204 になる（§57、§58）。
判定 24-f: `verify-r24.sh` を新設する。

### R25 失敗の効果

- `stdlib/error.hd` を新設し、`stdlib/result.hd` と `stdlib/division_error.hd` を削除する
- `stdlib/option.hd` の `ok_or` を `A {Error}` にし、純粋な 9 本に `Pure` を書く
- `mono.rs` の `propagation`（660 行目）を Option 限定にする
- `/` と `%` を `Int {Error}` にする
- 受理コーパス 54 本を新しい綴りへ移す。`Ok(` と `Error(` は 0 件になる。移行先は 2 つに限る。値の不在は `Option` と `?` へ、理由のある失敗は `raise` を `main` まで通す形へ移す。**handler を使う形へは移さない。** handler の `abort` と `return` のコード生成は R26 の成果物なので、R25 で使うと前方依存になる。handler を使う受理コーパスは R26 が新設する
- 拒否コーパスから `E509/` を削除し、`E508/` を Option の再伝播に書き直す。現在の `tests/reject/E508/main.hd` は `(Int, String) Result` を使っており、Result が消えると型名が解決できず E204 で落ちて E508 として機能しない。`E303/` の Option 限定への狭めもこのラウンドで入れる。R24 で狭めると `stdlib/result.hd:7` の `let x? = value`（右辺が Result）が落ちて prelude が検査不能になる
- §51 の印字（`main` の値は stdout、届いた `raise` の `message` は stderr で終了コード 1）
- `verify-m1-spec.sh` の検査 3（`DivisionError` の存在確認）を落とす

判定 25-a: 受理コーパス全件が `check` を通る。
判定 25-b: 拒否コーパスから `E509/` が消え、残りが §50 の表と 1 対 1 で対応する。本数が表の行数に揃うのは E706 を足す R26 なので、`parse_corpus.rs` の `corpus_counts` はこのラウンドでは赤のままでよい。
判定 25-c: `main` の観測される振る舞いが 0.1.3 と同じである。成功は stdout と終了コード 0、失敗は stderr と 1。受理コーパスの `expect:` タグを変えずに通ることで確かめる。失敗系は `tests/accept/division_error/main.hd`（`expect-error: DivisionByZero`）が持つので、`abort` の実行経路はこのラウンドで要る。R26 は 3 種別を揃えるラウンドで、`abort` の最小の経路は R25 が置く。
判定 25-d: `verify-m1-spec.sh` が緑に戻る。
判定 25-e: `verify-r25.sh` を新設する。

### R26 種別ごとのコード生成

- `reply` はその場で評価し、yield しない
- `abort` は tag 検査と分岐に落とす。0.1.3 の Result のコード生成と同形になる。最小の経路は R25 が置くので、このラウンドは handler 経由の中断と `return` の腕を足す
- `capture` は evidence passing と short-cut resumption で実装する。継続はヒープのフレーム列
- E706（`main` にユーザー定義効果）を実装し、拒否コーパスに `E706/` を新設する
- 受理コーパスに 3 種別それぞれの実行例と、handler を使う例（`with` で `abort` を受けて値に変える形）を新設する。R25 は handler を使わない形までを担うので、handler の実行はこのラウンドで初めて確かめる

判定 26-a: `reply` を含む末尾再帰が 1000 万反復で定数メモリに収まる（17.8 の対照 2 と同じ形）。
判定 26-b: `abort` を含むプログラムが 0.1.3 の Result 版と同じ出力と終了コードを出す。
判定 26-c: `capture` の生成器が正しい値を返す。17.8 のとおりスタックを消費するので、反復数は 1 万で測る。
判定 26-d: 拒否コーパスの本数が §50 の表の行数（69）と一致する。R23 が 2 本、R24 が 3 本、R26 が 1 本を足し、R25 が `E509/` を 1 本消すので、64 − 1 + 6 = 69 になる。
判定 26-e: E706 のアンカー（`def main`）を位置摂動プローブで確かめる。
判定 26-f: `verify-r26.sh` を新設する。

### R27 仕上げ

- lock を更新する
- `docs/corpus.md`、`docs/spec-coverage.md`、`docs/spec-trace.md`、`README.md`、§39 の典型コードを追随させる
- `verify-m1-spec.sh` の検査 5 が項目を導出する表に 18.4 を足す。今は 2.2 2.3 9.2 11.2 13.2 だけを読んでいる
- 全門番と `cargo test` を実走する

判定 27-a: 全門番が終了コード 0 で、`cargo test --offline --no-fail-fast` が 0 failed。
判定 27-b: `docs/` の対応表が仕様の節と一致する（`verify-m1-spec.sh` の検査 5）。
判定 27-c: `v0.2` を tag する。

### 18.4 の項目の割り当て

`verify-m1-corpus.sh` の検査 3 は、計画の項目ごとに受理と拒否のファイルを要求する。R27 で判定器に 18.4 を足すので、6 項目それぞれに `// item:` タグを付けたファイルを置く。R18〜R22 の表と同じ形である。

| 項目 | 受理 | 拒否 | ラウンド |
|---|---|---|---|
| 効果の宣言 | `effect_forms/` | `E123/` | R23 |
| handler と with | `handler_forms/` | `E124/` | R23 |
| 効果行 | `effect_row/` | `E522/` | R24 |
| 失敗の効果 | `error_effect/` | `E605/` | R24 |
| 効果の位置 | `effect_position/` | `E308/` | R24 |

`E308/` は分岐の条件に効果を置く。緩和で許した scrutinee・引数位置・被演算子・record field を書くと受理されて E308 として機能しない。受理側の `effect_position/` には、許した 4 つの位置（scrutinee、引数、被演算子、record field）に効果を置く例を並べ、将来の狭めを回帰として検出できるようにする。
| 末尾呼び出しの限定 | `capture_loop/` | `E706/` | R26 |

### 各ラウンドで許容される赤

着手時点の 7 本に加えて、そのラウンドが触っていない段階の門番は赤のままでよい。列挙にない門番が赤くなったら、そのラウンドの欠陥として報告する。

| ラウンド | 終了時に緑へ戻すもの | 赤のまま許すもの |
|---|---|---|
| R23 | `verify-r9`、`verify-m2` の検査 2 | m0 m1-corpus m2 の他、m3 m7 |
| R24 | 無し（型の実装だけでは判定器が揃わない） | R23 と同じ |
| R25 | `verify-m1-spec`、`verify-m1-corpus` の検査 1 | m0 m2 m3 m7 |
| R26 | `verify-m0`、`parse_corpus` の `corpus_counts` | m2 m3 m7 |
| R27 | 全て | 無し |

### スコープ外

- 17.6 の未確認 2（`under` フレームの復元）。実装方式の問題で、R26 で必要になれば扱う
- segmented stacks。`capture` のスタック消費は 6 節のリスクとして受け入れる
- 効果の推論の最適化。まず正しく動くことを優先する

## 20. R28 入れ子の handler の手順書（2026-09-10）

R26 が残した `docs/bug.md` の B4 を直す。**`v0.2` の tag は R28 の完了後に打つ。**

19 節の R27 の判定 27-c は保留した。判定 26-a〜26-f は全て満たされ全門番が緑だが、B4 の 1 件目は §58 が禁じていない形で**合法なプログラムに誤った値を返す**。版が何を約束するかの判断としてユーザーが R28 を選んだ（`docs/ledger.md` の R27 の節）。

**読む順:** 本節 → `docs/bug.md` の B4 → `docs/ledger.md` の R26 の節 → `docs/spec.md` の §57 §58 → 17.2「種別ごとの保証」。

**着手時点で赤の門番:** 無し。全 26 本が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**R28 の終了時も同じでなければならない。**

### 成果物

`crates/hado/src/{mono,emit,infer}.rs`、`runtime/`、`tests/accept`、`tests/reject`、`scripts/`、lock、tag

### やること

- **`$Fail` の tag に効果の同一性を持たせる。** 現在は tag が対象の効果を区別しないため、
  内側の handler が外側の効果の `abort` を捕まえる。tag の表現と evidence の受け渡しに関わる
- **handler の腕の中の `with` の `return` を抑制しない。** 腕の評価中に `return` の書き換えを
  抑制する仕組みが、腕の中の別の `with` にまで及んでいる
- **`abort` の腕から効果付き関数を呼べるようにする。** 腕が受け取った tagged 値を
  境界で処理せず、そのまま腕の結果にしている
- 受理コーパスに入れ子の実行例を新設する（別効果の `with` の入れ子、腕の中の `with`）

### 判定

判定 28-a: 別の効果の handler を入れ子にしても `abort` が正しい handler に届く。B4 の再現 1 が `7` を返す。
判定 28-b: handler の腕の中の `with` の `return` の腕が当たる。B4 の再現 2 が `5` を返す。
判定 28-c: `abort` の腕から効果付き関数を呼べる。B4 の再現 3 がビルドでき、実行できる。
判定 28-d: R26 の 11 行の回帰（`crates/hado/tests/r26_effects.rs`）が緑のまま。
判定 28-e: R24 の 14 セル（`r24_effects.rs`）と R25 の 4 形（`r25_effects.rs`）が緑のまま。
判定 28-f: 全門番が終了コード 0 で、`cargo test --offline --no-fail-fast` が 0 failed。
判定 28-g: `verify-r28.sh` を新設し、28-a〜28-c を判定する。
判定 28-h: `docs/bug.md` から B4 の節を消す（直したら節を消し、経緯は git 履歴に残す）。
判定 28-i: `v0.2` を tag する。

### 許容される赤

**無い。** 着手時点で全門番が緑なので、1 本でも赤くしたら R28 の欠陥である。

### スコープ外

- multi-shot resumption と第一級の resumption（17.5 のまま）
- 17.6 の未確認 2（`under` フレームの復元）。B4 の修正で必要になれば扱う
- 効果の推論の最適化

### 17.5 との関係

17.5「やらないこと」の「継続を別の handler の下へ運ぶ使い方」は `capture` 固有である。
B4 の再現 1 と 3 は継続を伴わない素の `abort` なので 17.5 では覆えず、
**`abort` の入れ子について計画も仕様も沈黙していた。** 本節がその穴を埋める。

## 21. R29 印字の失敗の手順書（2026-09-10）

`docs/bug.md` の B1 を直す。**`v0.2.1` の tag は R29 の完了後に打つ。**

§51 は印字の失敗を定めておらず、runtime のどの出力経路も書き込みの戻り値を見ていない。
仕様の穴埋めと runtime の修正を一つのラウンドで行う。

**読む順:** 本節 → `docs/bug.md` の B1 → `docs/spec.md` の §51 → `docs/ledger.md` の「バグ修正方針」の節。

**着手時点で赤の門番:** 無し。全 27 本が満点・終了コード 0、`cargo test --offline --no-fail-fast` が
0 failed。**R29 の終了時も同じでなければならない。**

### 決定

- 印字が失敗したら終了コードを 1 とする。raise と同じ値で、§51 の 0/1 の二値を保つ。
  区別は stderr の診断文でつける
- stderr への理由出力は best-effort とする。stderr 自体の失敗は無視する（それ以上報告する先がない）
- runtime は SIGPIPE を無視し、EPIPE を通常の書き込み失敗として扱う。§51 にパイプだけの
  例外条項を作らない。実行中の print 効果を将来足すときはこの決定を再考する

### 成果物

`runtime/hado_runtime.c`、`docs/spec.md` の §51、`scripts/`（EPIPE 再現の helper と `verify-r29.sh`）、`docs/bug.md`、tag

### やること

- `main`（`runtime/hado_runtime.c` の 20-23 行）で SIGPIPE を無視する
- 出力経路（`hado_print_int` `hado_print_string` `hado_print_char` `hado_print_float`）の
  書き込み失敗を終了コードへ反映し、stderr へ理由を best-effort で出す
- §51 に失敗時の規範を足す
- EPIPE を決定的に再現する helper（パイプの読み側を先に閉じてから exec する）を `scripts/` に置く

### 判定

判定 29-a: fd を閉じた実行（`>&-`）が終了コード 1。B1 の再現が 0 を返さなくなる。
判定 29-b: `/dev/full` へのリダイレクトが終了コード 1（ENOSPC）。
判定 29-c: 読み側を閉じたパイプへの書き込みが終了コード 1（EPIPE）。シグナル死（141）ではない。
判定 29-d: stdout と stderr の両方を閉じても終了コード 1。
判定 29-e: 正常な実行は従来どおり印字して終了コード 0。全門番が緑のまま。
判定 29-f: `verify-r29.sh` を新設し、29-a〜29-e を判定する。**印字する値は判定器が実行時に生成し、
期待値の oracle は判定器側に置く。Int と String の 2 経路以上を回す。** 固定値だと、書き込みの
結果を一切見ずにその値のときだけ終了コードを 1 にする偽実装で全部通せる（plan-reviewer が
実際に作り、新旧の判定器と全門番で実測した）。
判定 29-g: `cargo test --offline --no-fail-fast` が 0 failed。
判定 29-h: `docs/bug.md` から B1 の節を消す。
判定 29-i: `v0.2.1` を tag する。

### 許容される赤

**無い。** 着手時点で全門番が緑なので、1 本でも赤くしたら R29 の欠陥である。

### スコープ外

- 実行中の印字（print 効果）。v0.2 の言語に存在しない
- B5（`capture` を挟む 3 段の入れ子）。プロトタイプ実測を経て R30 で扱う
  （`docs/ledger.md` の「バグ修正方針」の節）
