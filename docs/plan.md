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

1 は 17.8 で、2 は 17.9 で解決した。

2. short-cut resumption の prompt / under フレーム復元が、hado の設定（scoped、1 回限り、値化禁止）でも必要か。**必要である**（17.9）

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

### 17.9 実測（17.6 の 2、2026-09-10）

17.6 の未確認 2（short-cut resumption の prompt / under フレーム復元が、hado の設定でも必要か）を、
本体の実測・現物の読み・最小モデルの 3 つで確かめた。

**本体の実測。** `capture` handler を他の handler と組み合わせた 15 形を `hado build` にかけた。
**15 形すべて `hado check` は通る。破れるのは単相化だけである。**

| 形 | 実測 |
|---|---|
| `Collect` 単独 | ok。`[3]` |
| `Collect` 単独・yield 2 回 | ok。`[1, 2]` |
| `Outer(A)` ⊃ `Collect` | NG `done payload mismatch` |
| `Outer(A、return 腕あり)` ⊃ `Collect` | NG `done payload mismatch` |
| `CollectY` ⊃ `CollectZ`（capture 同士） | NG `done payload mismatch` |
| 同じ効果 `Y` の handler の入れ子 | NG `done payload mismatch` |
| `Collect` ⊃ `Trap`（abort の handler） | NG `done payload mismatch` |
| `resume` の中から外側の `capture` を起こす | NG `done payload mismatch` |
| `resume` の中から外側の `reply` を起こす | NG `done payload mismatch` |
| 内側 handler の `return` が最終結果に残る形 | NG `done payload mismatch` |
| `Collect` ⊃ `Inner(B)` | NG `continuation result type mismatch` |
| `Collect` ⊃ `Inner(A)` | NG `continuation result type mismatch` |
| `Collect` ⊃ `Inner(B)`・yield 2 回 | NG `continuation result type mismatch` |
| 再開した継続の中で内側の `reply` を呼ぶ形 | NG `continuation result type mismatch` |
| `Outer(A)` ⊃ `Collect` ⊃ `Inner(B)`（B5） | NG `continuation result type mismatch` |

**B5 は 3 段固有ではない。2 段で既に壊れ、`capture` handler が内側か外側かで症状が変わる。**
内側なら forwarding が handler の答えを外側の Outcome の `$Done` に包み、外側なら継続が
最内 handler で閉じる。`docs/bug.md` の B5 は 3 段の例だけを記録しており、範囲を過小に書いていた。
**測った 15 形のうち、通ったのは `capture` handler が単独の 2 形だけである**
（15 形は網羅ではないので、これ以外に通る合成があるかは未確認）。
`hado check` が 15 形すべてを通すことから、仕様も型システムもこの形を認めており、
追いついていないのは単相化の畳み方だけだと分かる。

**現物の読み。** `done payload mismatch` の経路は `crates/hado/src/mono.rs` で追える。
`interpret_outcome` は `forwarding = handler_stack.len() > 1`（`:1756`）のとき
`align_switch_arms` に**元の Outcome をそのまま fallback として渡す**（`:1758`）。
`align_switch_arms` は fallback の型を基準に、Outcome でない腕を `done()` で包む（`:1915-1917`）。
`done()` は `value.ty() != success` で拒否する（`:1935-1937`）。元の Outcome の success 欄は
中の計算の成功型（`Unit`）のままで、handler の答えは `List Int` なので、ここで必ず落ちる。
**forwarding が Outcome を変換していないことが原因で、包み方の細部ではない。**

`continuation result type mismatch` の側は `make_continuation` の `body.ty() != *answer`
（`:1613`）で、`answer` は `capture_answers.last()`、すなわち**最内の答えを要する handler**であって
その操作を扱う handler ではない。単独 handler では両者が一致するので通る。

**最小モデル。** mono.rs から切り離し、生成器で表した計算とフレーム列だけの解釈器を書いた
（プログラムは `docs/ledger.md` の「B5 の最小モデル」に置いた形）。生成器の中断点がそのまま
one-shot 継続になり、§58 の「`resume` は捕まえた継続を 1 回だけ再開する」と構造が一致する。
畳み方を 2 つ用意した。

- `hado`: 継続を最内 handler で閉じ、その handler より下に handler が残っていれば
  答えを外側の Outcome の `$Done` に包む
- `under`: prompt 自身とその上の内側 prompt を継続に含めて再設置し、答えを包まずに prompt の下へ渡す

| 形 | モデル `under` | モデル `hado` |
|---|---|---|
| `Collect` 単独 | `[3]` | `[3]` |
| `Collect` 単独・yield 2 回 | `[1, 2]` | `[1, 2]` |
| `Outer(A)` ⊃ `Collect` | `[1]` | done payload mismatch |
| `CollectY` ⊃ `CollectZ` | `[3]` | `[3]` |
| 同じ効果の入れ子 | `[103]`（内側が捕まえる） | done payload mismatch |
| `Collect` ⊃ `Trap` | `[7]` | `[7]` |
| `Collect` ⊃ `Inner(B)` | `[2]` | continuation result type mismatch |
| 同上・yield 2 回 | `[2, 5]` | unhandled operation |
| 再開後に内側の `reply` を呼ぶ | `[1, 2]` | unhandled operation |
| 内側の `return` が結果に残る | `[1, 42]` | continuation result type mismatch |
| B5（3 段） | `[3]` | continuation result type mismatch |
| `resume` の中から外側の `capture` | NG unhandled operation | NG unhandled operation |

**モデルが確かめたこと。** `under` の切り方なら、11 形が**畳み切れて**、handler に設定した
型ラベルが最後まで矛盾なく繋がる。**モデルは型検査を持たない**ので、これは「型が正しい」の
証明ではなく「型ラベルの流れに切れ目が無い」の確認である。`$Outcome` の型付けそのものは
確かめていない。移植では型の設計を別に立てる必要がある。

再開した継続の中で内側の `reply` を呼ぶ形（`[1, 2]`）と、内側の `return` が最終結果に残る形
（`[1, 42]`）は、**under フレームの復元が実際に効いていないと出ない値**である。前者のトレースは
`Collect.capture y(1) を 2 段で捕捉（復元する内側 handler 1 段）` の後に
`Inner.reply b() = 2` が続く。

**モデルが確かめていないこと。** `hado` の側は 15 形のうち 5 形しか症状を再現しない。
モデルは forwarding を実行時に決め、本体は単相化の時点で型を決めるためである。
**負の側の根拠は本体の実測と現物の読みであって、モデルではない。**

**`resume` の中から `H` より外側の handler が扱う効果を起こす形は、`under` でも畳めない。**
モデルは外側のフレームを再開時の探索対象から外しているのでこの結果になるが、
これはモデル固有の制限ではなく、本体の表現からも同じ結論が出る。
`interpret_outcome` は継続を `Closure([Unit], answer)`（`:1747-1753`）として射影する。
つまり `resume : Unit -> A_H` で、**答えの型そのものを返す**。

境界は「内側か外側か」で決まる。**`H` 自身への yield は閉じる** — `H` の解釈が再帰的に
処理して `A_H` を作る（単独 handler の yield 2 回がこれで、実測でも通っている）。
**`H` より内側の handler への yield も閉じる** — 内側の解釈を合成すれば `A_H` に届く。
**`H` より外側へ yield したときだけ、`A_H` の値が存在しない。** 一般化するには継続が
Outcome を返す形（`Unit -> Outcome<A_H, ...>`）が要り、腕の本体が Outcome を扱う形を
要求する。R30 のスコープ外とし、22 節に記す。

**その他。** 継続は one-shot なので、フレーム列を複製する必要はない。中断した生成器を
そのまま 1 回だけ再開する形で足りた。ICFP'21 の multi-shot 前提のコストは hado には掛からない。

**17.6 の未確認 2 は「必要」で決着する。** scoped・1 回限り・値化禁止という hado の設定でも、
prompt / under フレームの復元は省けない。

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

## 22. R30 `capture` handler の合成の手順書（2026-09-10）

`docs/bug.md` の B5 を直す。**`v0.2.2` の tag は R30 の完了後に打つ。**

17.9 の実測で範囲が広がった。B5 は 3 段固有ではなく、**`capture` handler を他の handler と
組み合わせると内外どちらでも build に失敗する**。測った 15 形のうち通ったのは単独の 2 形だけである。
17.6 の未確認 2（prompt / under フレームの復元）は「必要」で決着した。

**読む順:** 本節 → 17.9 → `docs/bug.md` の B5 → `docs/spec.md` の §58 → 17.7 の p.71:10 §2.8。

**着手時点で赤の門番:** 無し。R29 の終了時点で全 28 本が満点・終了コード 0、
`cargo test --offline --no-fail-fast` が 0 failed。**R30 の終了時も同じでなければならない。**

### 版を `v0.2.2` とする理由

仕様本文は変わらない。`hado check` は 15 形すべてを通しており（17.9）、§58 が既に許している形が
ビルドできるようになるだけである。構文も意味論も足さない。変わるのは `$Outcome` の内部表現と
単相化の畳み方だけなので、パッチ版とする。

### 決定

現物の該当箇所は 17.9 の「現物の読み」にある。

- **`$Yield` に効果の同一性を持たせる。** R28 が `$Fail` に対して行ったのと同型で、単相化の
  時点で現れる効果の集合から密な連番を割り当てる。ハッシュは使わない（R28 で衝突する名前の組が
  実際に見つかっている）。handler は自分の効果の yield だけを扱い、他は forwarding する。
  **`$Fail#e` と `$Yield#e` のタグは 1 本の連番から配って衝突を原理的に断つ**
  （現行の `fail_tag_for` が 0 と 2 を避けているのは `$Yield` が固定タグ 2 だったためで、
  連番を 1 本にすればこの回避は要らなくなる）
- **forwarding は Outcome を変換する。** 現行は `align_switch_arms` に元の Outcome を
  そのまま fallback として渡し（`mono.rs:1758`）、success 欄が中の計算の成功型のままなので
  `done()` が必ず拒否する（`:1935-1937`）。`Outcome<S, ...>` を `Outcome<A_H, ...>` へ
  変換したうえで fallback に渡すこと。**変換は success 欄の付け替えと、forwarding する
  `$Yield#e` の継続の包み直しの両方を含む**
- **forwarding は継続を組み直す。** 中間の handler `H'` が自分宛でない `$Yield#e` を見たら、
  素通しせずに継続を `λu. convert_{H'}(k(u))` の形に包み直す。これが under フレームの復元にあたる。
  素通しすると、再開したときに `H'` が再設置されず、`H'` が扱う効果が継続の中で unhandled になる
- **`resume` は解釈の前段だけから作る。** 「handler として解釈して答え `A_H` を出す」と
  「答えを外側の Outcome に包む」を 2 段に分け、`resume` は前段だけを使う。
  現行の `interpret_outcome` は forwarding のときに両方を一度にやるので、この関数で
  `resume` を組むと `resume ()` が答えではなく外側の Outcome を返し、
  `let rest = resume ()` と合わない
- **継続の答え型は、その操作を扱う handler の答えとする。** `capture_answers.last()`
  （最内の答えを要する handler、`mono.rs:1579`）ではない。単独 handler では両者が一致するので
  現状が通り、入れ子で乖離する
- **継続の型の仮説:** `$Yield#e(payload, k)` の内部の `k` は、囲んでいる計算と同じ Outcome を
  返す（`Unit -> Outcome<S, ...>`）ものとし、各 handler の解釈が `k` を 1 段ずつ変換する。
  利用者から見える `resume : Unit -> A_H`（`mono.rs:1747-1753` の射影）は、`e` を扱う handler の
  位置で、内側の変換を合成して作る。**この仮説が成り立たないと分かったら、そこで止めて
  報告すること**（17.9 のモデルは畳み方の意味論を確かめただけで、型付けは確かめていない）
- **`resume` は 1 回限りのまま。** 17.9 で、フレーム列を複製せず中断点を 1 回だけ再開する形で
  足りることを確かめた。multi-shot は 17.5 のまま採らない

### 成果物

`crates/hado/src/mono.rs`、`tests/accept/`（合成の実行例）、`tests/corpus.sha256`、
`scripts/verify-r30.sh`、`docs/bug.md`、tag

### 工程

**2 段に切る。症状が 2 種類で根が違うので、深い方から通す。**

- 工程 1: `continuation result type mismatch` の側（`capture` handler が外側）。判定 30-a〜30-d。
  **under フレームの復元そのものがここに当たる**
- 工程 2: `done payload mismatch` の側（`capture` handler が内側）と 3 段。判定 30-e〜30-j。
  **工程 1 の機構がこの側も副作用で通したので、工程 2 の実体は判定器・受理コーパス・
  スコープ外の形の拒否（判定 30-k）になった**

工程 1 が緑になったことを指揮者が実測してから工程 2 を発注する。

### 判定

期待値は 17.9 の最小モデルの `under` から導いた。**本体では 1 つも通っていないので、
期待値そのものが仮説である。実測と食い違ったら、実装を疑う前に期待値を疑うこと。**

**判定 30-a〜30-b は under フレームの復元を直接固定する。**
内側の handler を捨てて `capture` handler で継続を閉じるだけの不完全な修正は、ここで落ちる。

判定 30-a: **再開した継続の中で内側の handler の操作を呼ぶ。2 つを別に判定する。**

  - (1) 内側が `reply`: `Collect` ⊃ `Inner(B)` で `do y 1 -> do y b -> ()`。実行して `[1, 2]`
  - (2) 内側が `capture`: `Collect`（`return x => x`）⊃ `Inner`（`capture w value => [value + 100, ...resume ()]`）
    で `do y 1 -> do w 2 -> ()`。実行して `[1, 102]`

  **包み直しを固定するのは (2) だけである。** `reply` は tail-resumptive で、単相化の時点で
  腕がその場に展開され `$Yield` を作らない。したがって継続の包み直しを外しても (1) は通る
  （実測: 包み直しを外すと (1) は通り、判定 30-b だけが落ちた）。**`$Yield` を実際に作る操作、
  つまり内側の `capture` でしか固定できない。**
判定 30-b: **内側の handler の `return` の変換が最終結果に残る。**
  `Collect`（`return x => [x]`）⊃ `Inner`（`return _ => 42`）で `do y 1 -> ()`。実行して `[1, 42]`。
  `resume` の後に `Inner` の `return` が当たっていないと `42` が現れない
判定 30-c: `Collect` ⊃ `Inner(B)` がビルド可。実行して `[2]`
判定 30-d: `Collect` ⊃ `Inner(A)` がビルド可
判定 30-e: `Outer(A)` ⊃ `Collect` がビルド可。実行して `[1]`
判定 30-f: `CollectY` ⊃ `CollectZ`（capture 同士）がビルド可。実行して `[3]`。
  導出: `y` を `CollectY` が捕まえ、`resume ()` が残りの `()` を `CollectZ` の下で走らせて
  `CollectZ.return → []`、それが `CollectY` の本体の値になり `CollectY.return(_) → []`、
  腕の結果が `[3, ...[]]`
判定 30-g: `Collect` ⊃ `Trap`（abort の handler）がビルド可。実行して `[7]`
判定 30-h: B5 の 3 段（`Outer` ⊃ `Collect` ⊃ `Inner`）がビルド可。実行して `[3]`
判定 30-i（**負の対照**）: 同じ効果の handler の入れ子では**内側が捕まえる**。
  外側を `return x => x`、内側の腕を `value + 100` にして、実行して `[103]`。
  内側を素通しさせる直し方はここで `[3]` になって落ちる。
  導出: 内側の `capture` の腕の結果 `[103]` が外側 handler の**正常完了値**になり、
  外側の `return x => x` がそれに当たる。外側を `return _ => []` にすると `[]` になる
判定 30-j（**負の対照**）: 単独 handler の既存の形が壊れない。`Collect` 単独が `[3]`、
  yield 2 回が `[1, 2]`
判定 30-k（**実測で判明。工程 1 の後に必須になった**）: `resume` した継続の中から、
  その handler より外側の handler が扱う `capture` を起こす形は、**単相化の時点で拒否する**。
  工程 1 の機構を入れると、この形は `hado check` も `hado build` も通ったうえで
  **実行時に SIGILL でクラッシュする**（終了コード 132、印字なし）。工程 1 の前は
  `done payload mismatch` でビルドが失敗していたので、**受理してクラッシュするのは後退である**。
  `v0.2` を tag できた根拠は「B5 は誤った答えを黙って返さない」だった。診断を出して拒否すること
判定 30-l: `verify-r30.sh` を新設し、30-a〜30-j を判定する。実行例は `tests/accept/` に置き、
  判定器はその出力を突き合わせる。期待値を `.hd` の側に書かない。
  **加えて、判定器は 30-a・30-e・30-h の payload を実行時に生成した整数に差し替えた `.hd` を
  一時ディレクトリに書き、期待するリストを判定器の側で計算して突き合わせること。**
  固定の入力形だけを見ていると、入力の形を認識して定数リストを返す実装で B5 の根を直さずに
  通せる（R29 でも同じ穴が見つかっている）。生成した値がソースにも `.hd` にも定数として
  現れないこと。1 形 1 ディレクトリにすること（同じディレクトリの `.hd` は名前空間を共有し
  E205 になる）
判定 30-m: 全門番が満点・終了コード 0。`cargo test --offline --no-fail-fast` が 0 failed
判定 30-n: `docs/bug.md` から B5 の節を消し、下記のスコープ外を新しい節として記録する
判定 30-o: `v0.2.2` を tag する

### 許容される赤

**無い。** 着手時点で全門番が緑なので、1 本でも赤くしたら R30 の欠陥である。

### スコープ外

- **`resume` の中から、その handler `H` より外側の handler が扱う `capture` を起こす形。**
  **`reply` は含まない。** 工程 1 の後に実測したところ `[9, 1]` を正しく返した。`reply` は
  tail-resumptive で、単相化の時点で腕がその場に展開され `$Yield` を作らないためである。
  当初この節は `reply` も含めていたが、実測で誤りと分かった。
  `hado check` は通るので §58 はこれを禁じていない。
  `interpret_outcome` は継続を `Closure([Unit], answer)`（`mono.rs:1747-1753`）として射影する。
  つまり `resume : Unit -> A_H` で答えの型そのものを返す。
  **`H` 自身への yield（判定 30-a の 2 回目の `y`、単独 handler の yield 2 回）と、
  `H` より内側の handler への yield（判定 30-a の `b`）は閉じる** — 前者は `H` の解釈が
  再帰的に処理し、後者は内側の解釈を合成すれば `A_H` に届く。**閉じないのは `H` より
  外側へ yield したときだけで、そのとき `A_H` の値が存在しない。** 一般化するには継続が
  Outcome を返す形が要り、腕の本体が Outcome を扱う形を要求する。**R30 では直さず、
  単相化で拒否したうえで `docs/bug.md` に新しい節として残す**（判定 30-k と 30-n）。
  仕様は変えない。拒否は単相化が返す診断であって新しいエラーコードではないので、
  §50 の表は変わらない
- multi-shot resumption と第一級の resumption（17.5 のまま）
- 効果の推論の最適化
- 末尾再帰の深さの限界（17.8 の実測のまま。C スタックが束縛条件）

### 中断の条件

**継続の表現をヒープのフレーム列に置き換える必要があると判断したら、直さずに中断せよ。**
想定しているのは、`$Yield` のタグ付け・Outcome の変換・解釈と包みの分離・
forwarding の包み直しという、単相化の中の変更である。それで届かないと分かったら、
どこまで確認したか・何が足りないかを具体的に書いて止める。R26・R28 と同じく、
止めることは正常な終了経路である。

## 23. R31 yield 地点の継続と `$Outcome` の型の手順書（2026-09-11）

`docs/bug.md` の B6〜B9 を直す。**`v0.2.3` の tag は R31 の完了後に打つ。**

4 件の根は 2 本である。**継続** — yield 地点の後ろの計算を継続に切り出す
仕組みが `do <capture> -> rest` の形にしか無い（B6・B7）。**型** — `$Outcome` が payload の型を
1 つしか持たず、継続の引数型が Unit に固定されている（B8・B9）。B8 は両方に当たる。

**読む順:** 本節 → `docs/bug.md` の B6〜B9 → 22 節 → `docs/spec.md` の §57・§58・§24・§44 →
17.7 の p.71:11 §2.10（bind-inlining）。

**着手時点で赤の門番:** 無し。2026-09-11 に全 29 門番を単独で走らせて満点・終了コード 0、
`cargo test --offline --no-fail-fast` が 0 failed（`v0.2.2` の判定 30-o と同じ）。
**R31 の終了時も同じでなければならない。** 門番の全走中に `docs/` を書くと m1-corpus と m2 が
保護ファイルの検査で赤になるので、文書の作業と全走は直列にすること。

### 版を `v0.2.3` とする理由

仕様本文は変わらない。B6〜B9 の全形を `hado check` が通しており、§24・§57・§58 が既に許している
形がビルドできる、または正しい値を返すようになるだけである。構文も意味論も足さない。変わるのは
単相化の畳み方と `$Outcome` の内部表現だけなので、パッチ版とする。

### 決定

- **根が 2 本なので工程を 3 つに切り、同じラウンドで直す。** 工程 0 は B7 を「受理して誤答」から
  「拒否」に変える暫定措置で、初日に入れる。工程 1 は継続、工程 2 は型。工程 1 の緑を指揮者が
  実測してから工程 2 を発注する
- **yield 地点の後ろを常に継続にする。** 単相化の入口で、`capture` の操作の呼び出しと `resume` を
  `let t = <呼び出し> -> 残り` の形に持ち上げる（A 正規化）。持ち上げる位置は §44 の評価順に従い、
  適用の引数、パイプの左辺、演算子の被演算子、リストの要素、record の field、`if` の scrutinee。
  持ち上げない位置は 4 つ。分岐の腕（末尾文脈）、`&&` と `||` の右辺（左辺が決まってから評価する）、
  `fn` の本体（持ち上げると定義時に評価してしまう）、`with H ->` の本体（持ち上げると yield が
  handler の外へ出る）。`let` と `do` の右辺は既にこの形である。正規化は `mono.rs` の中、または
  単相化だけが使う新しいモジュールに置く。`normalize.rs` は冗長検査の正準形比較で、別物である
- **`let t = <capture> -> rest` の継続は `cont : R -> $Outcome`。** `R` は操作の戻り型。今の
  `capture_do` → `continuation_under` → `make_continuation` の経路を、`do` 以外の位置と値の束縛に
  広げる。末尾位置は `rest` が無いので `λt. $Done(t)`
- **`resume v` の結果を bind する。** 腕が forwarding の状態（外側に handler がある）で走るとき、
  `resume v` は `convert_H(k(v)) : $Outcome<A_H, …>` を返す。腕の残り `rest` を継続として付け直す

  ```text
  match convert_H(k(v))
  | $Done(a)         => ⟦rest⟧[a]
  | $Yield#e(p, k')  => $Yield#e(p, λu. bind(k'(u), λa. ⟦rest⟧[a]))    -- e は H より外側の効果
  | $Fail#e(p)       => $Fail#e(p)
  ```

  `bind` はこの `match` そのもので、site ごとに生成する。`k'` は `forward_yield_arms` が既に
  `convert_H` で包み直した継続である。外側に handler が無いとき（`forwarding` が偽）は `convert_H` が
  答えの型そのものを返すので、今までどおり `resume : R -> A_H` の値呼び出しでよい
- **`capture_escapes_prompt`（`mono.rs:1657` 付近）の拒否を消す**
- **`$Outcome` は効果ごとの payload 型と戻り型を持つ（仮説）。** `$Yield#e` の payload と継続の
  引数型を、効果 `e` の操作の宣言から取る。`repr.rs:20` は `$Outcome` を一様なポインタに写して
  いるので、機械表現は変わらず、単相化の `Type` の付け方だけが変わる見込み。`outcome_parts` の
  1 つの payload 欄と、それを参照する箇所（`mono.rs` に 52 箇所）が動く。**この仮説が成り立たないと
  分かったら、そこで止めて報告すること**（下記「中断の条件」）
- **30-k の 2 行は `check_prints` に置き換える。** `verify-r30.sh:321-322` の `rejects_build k2` と
  `rejects_build k3` を、`check_prints k2 "[2]"` と `check_prints k3 "[5]"` にする。30-k が約束したのは
  「この形を受理してクラッシュしない、誤答を黙って返さない」で、値の一致はそれより強い。門番を
  緩めたのではないことをスクリプトのコメントに残す（R28 の門番の件と同じ扱い）。**判定 31-0 も
  同じ運命である。** 工程 1 で 31-d が通った時点で意図して赤になるので、同じコミットで 31-d に
  置き換える
- **`resume` は 1 回限りのまま。** multi-shot は 17.5 のまま採らない

### 期待値の根拠

意味論は §58（`with H -> rest` は `rest` を `H` の下で評価する。`resume` は継続を 1 回だけ再開する。
`return` の腕は正常完了の値を変換する）と 17.5（shallow handler を採らない）で一つに決まる。
判定 30-f の導出と同じ流儀で導き、Koka 3.2.3（17.8 と同じ `koka-probe` コンテナ）で独立に確かめた。
**本体では 1 つも通っていないので、期待値そのものは仮説である。実測と食い違ったら、実装を疑う前に
期待値を疑うこと。**

| 形 | 導出 | Koka 3.2.3 |
|---|---|---|
| B6 の再現例（Outer `return _ => []`） | `[2]` | `[2]` |
| 同、Outer `return x => x` | `[2, 1]` | `[2, 1]` |
| 外へ渡った後にもう一度 `y 3` | `[2, 1, 3]` | `[2, 1, 3]` |
| 引数位置 `[y 1, y 2]`（B7） | `[1, 2]` | `[1, 2]` |
| 非 Unit の `ask`（B8） | `[42]` | `[42]` |
| payload が String と Int（B9） | `["a", "n1"]` | `["a", "n1"]` |
| 30-k の k3（真ん中に `reply`） | `[5]` | `[5]` |
| 展開位置の `resume` が外へ yield | `[3, 102]` | `[3, 102]` |
| payload 混在で外へ yield | `["b", "1"]` | `["b", "1"]` |

Koka のソースは `docs/ledger.md` の「B6 の対応方針」にある。`List String` の印字は hado も
`["a", "n1"]` の形で、Koka と同じ（実測）。

### 成果物

`crates/hado/src/mono.rs`（工程 2 で `repr.rs` に及ぶことがある）、`scripts/verify-r30.sh`
（30-k の置き換え）、`scripts/verify-r31.sh`、`tests/accept/`（各形の実行例）、`tests/corpus.sha256`、
`docs/bug.md`、tag。

受理コーパスの `// spec:` は、B6 の 3 形・k3・展開位置・payload 混在の 2 形が §58、引数位置が §24、
非 Unit が §57。

### 工程

- 工程 0: B7 の暫定拒否。判定 31-0。`do` の右辺と末尾以外の位置に `capture` の操作が現れたら
  単相化で拒否する。診断は 30-k と同じく §50 のコードを足さない。末尾は構文で決める（`let`・`do` の
  継続、分岐の腕、`with` の本体を辿った先の最後の式）
- 工程 1: 継続。A 正規化、`R -> $Outcome` の継続、`resume` の bind、`capture_escapes_prompt` の削除、
  30-k と 31-0 の置き換え。判定 31-a〜31-g
- 工程 2: 型。`$Outcome` の効果ごとの payload 型と戻り型。判定 31-h〜31-j

工程 1 が緑になったことを指揮者が実測してから工程 2 を発注する。

### 判定

判定 31-0（工程 0。**工程 1 で 31-d に置き換える**）: `with Collect -> [y 1, y 2]` がビルドで
  拒否される。負の対照: `with Collect -> y 1` が `[1]`、`do y 1 -> y 2` が `[1, 2]`

**判定 31-a〜31-c と 31-f は、「`let rest = resume () ->` の形だけ直した部分修正」と「Outer の
`resume` を越えた継続で Collect を再設置しない修正」を落とす。**

判定 31-a: B6 の再現例（30-k の k2）。実行して `[2]`
判定 31-b: 同じ形で Outer を `return x => x`。実行して `[2, 1]`
判定 31-c: `do y 1 -> do z 2 -> do y 3 -> ()`、Outer は `x => x`。実行して `[2, 1, 3]`
判定 31-d: `with Collect -> [y 1, y 2]`。実行して `[1, 2]`。31-0 の拒否を外す
判定 31-e: 30-k の k3（`with Outer -> with Collect -> with Inner -> do y b -> do z 5 -> ()`、
  `Inner` は `reply b => 2`）。実行して `[5]`
判定 31-f: `resume` が展開位置。`Inner`（`capture w value => [value + 100, ...resume ()]`、
  `return _ => []`）の上に `Outer`（`return x => x`）を置き、`do w 2 -> do z 3 -> ()`。実行して `[3, 102]`
判定 31-g（負の対照）: 30-k の対照 2 本（`resume` を経ない外側の capture が `[4]`、外側の `reply` が
  `[9, 1]`）はそのまま通る
判定 31-h: 非 Unit の `ask`（B8 の再現例）。実行して `[42]`
判定 31-i: payload が String と Int（B9 の再現例）。実行して `["a", "n1"]`
判定 31-j: B6 の再現例の `z` を `String` にし、Outer を `x => x`、Collect の腕を
  `[Int.to_string value, ...rest]`、本体を `do y 1 -> do z "b" -> ()`。実行して `["b", "1"]`
判定 31-k: `verify-r31.sh` を新設し、31-a〜31-j を判定する。実行例は `tests/accept/` に置く。
  **加えて、31-a・31-c・31-d・31-f の payload を実行時に生成した整数に差し替えた `.hd` を
  一時ディレクトリに書き、期待するリストを判定器の側で計算して突き合わせること**（30-l と同じ規約。
  生成した値がソースにも `.hd` にも定数として現れない。1 形 1 ディレクトリ）
判定 31-l: 全門番が満点・終了コード 0。`cargo test --offline --no-fail-fast` が 0 failed
判定 31-m: 受理コーパスに実行例を足し、`tests/corpus.sha256` を更新する
判定 31-n: `docs/bug.md` から B6〜B9 の節を消す
判定 31-o: `v0.2.3` を tag する

### 許容される赤

**無い。** 着手時点で全門番が緑なので、1 本でも赤くしたら R31 の欠陥である。例外は 31-0 で、
工程 1 で 31-d に置き換える。

### スコープ外

- multi-shot resumption と第一級の resumption（17.5 のまま）
- 効果の推論の最適化
- 末尾再帰の深さの限界（17.8 の実測のまま。C スタックが束縛条件）
- `reply` と `abort` の経路の変更

### 中断の条件

**継続の表現をヒープのフレーム列に置き換える必要があると判断したら、直さずに中断せよ。**
想定しているのは、A 正規化・継続の引数型・`resume` の bind・`$Outcome` の効果ごとの型という、
単相化の中の変更である（`repr.rs` の型の写しを含む）。

**`$Outcome` の型引数に効果ごとの型の列を持たせる必要があり、それが `align_outcomes` や
`convert_arms` の型合わせを壊すと分かったら、工程 2 を止めて報告せよ。** 工程 1 の成果はそのまま残す。

それで届かないと分かったら、どこまで確認したか・何が足りないかを具体的に書いて止める。
R26・R28・R30 と同じく、止めることは正常な終了経路である。

## 24. v0.3 の範囲と決定（2026-09-12）

`docs/wants.md` の最優先「核がライブラリの下で閉じること」を次の版にする。v0.1.3 の完走・v0.2・B1 は済んでいる（`v0.2.3` を tag 済み、`docs/bug.md` は空）。判断基準は 7 節と同じ（迷ったら拒否、新機能は最小形）。grilling（25 問）と独立レビュー（claude-fable-5-1、致命 1・重大 8・軽微 6）を経て確定した。

意図（明示）: 小さな CLI を「核の上の最初の 1 本」として書けること。そのために核が、`main` を持たないライブラリとして検査でき、実装をまたいで型を共有でき、文字列・バイト・配列を機械的に扱え、argv・stdin・ファイル・ディレクトリに触れること。
意図（推定）: 4 テーマの受け入れは個別の門番ではなく、wants 53「自分用のプログラムを 1 本 hado で書く」が通ることで測る（E2）。

進め方は 0.1.3 と同じ 5 手順。1（実測、24.7）と 2（grilling、24.1、レビュー反映は 24.4）が済んだ。3 文書（本節の書き直し、仕様、対応表、門番）→ 4 全門番の実走（約 20 分、文書の編集と直列）→ 5 判定者レビュー（別文脈、決定の反映・EBNF からの導出・§50 の一致・節の対の矛盾・手順書の実行可能性の 5 観点、PASS まで繰り返す）。

### 24.1 決定

**A. ライブラリ単位**

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| A1 | 起動の形 | `hado check` に `main` の無い `.hd` を渡すと、E206 の代わりにライブラリとして検査する。フラグは足さない。`build` は E206 のまま（A4）。ライブラリかどうかは渡したファイルの属性 | §5, §54 |
| A2 | E513 の根 | ライブラリでは、通常の実装、functor の適用結果、未適用の functor、実装の無い契約、外の型、効果、handler を E513 の根にする（公開面）。契約に載らない実装の `def` は E514、実装局所の ADT・record は E513 | §27 |
| A3 | 外の def | 外の `def` はファイル局所で、そのファイル内で使われなければ E513。ライブラリ根のディレクトリのファイルにも置ける | §27, §54 |
| A4 | build | ライブラリを `build` に渡すと E206 のまま。成果物は作らない。メッセージは entry 向けに分ける | §5 |
| A5 | 利用の形 | 使うプログラムの根の下のサブディレクトリに置く。検索パス・依存宣言は足さない | §55 |
| A6 | prelude | prelude の項目は公開面として根にする（単体検査でも連結時でも）。§49 の「E513 を除く」免除を外す。E514 は今までどおり適用 | §49, §27 |
| A7 | ライブラリ根の印 | ディレクトリの 1 つのファイルが `library` を宣言すると、そのディレクトリがライブラリ根になる。1 ディレクトリに 1 つ | §59 |
| A8 | パス解決 | ライブラリ内の絶対パスは最内のライブラリ根から解く。外からはディレクトリ名で修飾して参照する。兄弟ライブラリは参照しない（依存は利用側で束ねるか、依存を自分の下に入れ子にする） | §55, §59 |

**B. 型の共有**

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| B1 | 印の一様化 | `shared type T = ε` が等式を公開する唯一の印。functor 本文の `shared type T = M.T` は、適用結果の `T` を引数の実装の `T` と同一にする。印なしの実体化は不透明のまま（`type T = M.T` は適用ごとに別の抽象型） | §35, §56 |
| B2 | 既定 | 実装の抽象型は不透明が既定（§35 のまま） | §35 |
| B3 | 綴り | 実体化の行に `shared` を前置する（`shared type A T = A List`）。1 字違いの記号にはしない | §35, §41 |
| B4 | E705 | `shared` で具体型に解ける抽象型は `main` に残して印字してよい。解けない抽象型には E705 が残る | §5 |
| B5 | E603 / E604 | 変更しない。共有は等式を足すだけで、実装の数え方と再適用の禁止は変えない | §34, §56 |

**C. データの機械的表現**

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| C1 | 失敗の分類 | 索引の範囲外・`Int.parse` の不正・空セパレータの split は `{Error}`。探索の不在だけ `Option` | §21, §36 |
| C2 | Bytes | `Bytes` を組み込み型にする。I/O は Bytes。`String.length` はスカラ個数、`Bytes.length` はバイト数。不変。`U8` は足さない。`Bytes -> String` は `{Error}`、`String -> Bytes` は常に成功 | §36, §49 |
| C3 | 配列 | `T Array` を v0.3 で導入する。不変、`get` は O(1)・範囲外 `{Error}`、`set` は新しい配列。リテラルなし。型多相 intrinsic と要素幅は R35 で設計する | §36, §45, §49 |
| C4 | String 操作 | slice / 索引 / split / starts_with / contains の 5 つ。索引はスカラ値の単位。補間と多行は入れない | §49 |
| C5 | Int.parse | `String -> Int {Error}` のみ。Float.parse は入れない | §49 |
| C6 | 順序 | Bytes → String 操作 → Array → Int.parse | 24.5 |
| C7 | 印字 | Array は `Array.of_list [1, 2]`、Bytes は `Bytes.of_list [72, 105]`（10 進の Int リスト） | §51 |
| C8 | 関数集合 | Array: `of_list` `to_list` `length` `get` `set`。Bytes: `length` `get` `of_list` `to_list` `to_string` `of_string`。String: `length` `concat` `slice` `get` `split` `starts_with` `contains`。型は §49 に固定した | §49 |

**D. 世界との境界**

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| D1 | 効果 | `Env`（argv / env / cwd）と `Io`（stdin / ファイル / ディレクトリ）。時刻・乱数は入れない | §49, §57 |
| D2 | 操作 | 全て `reply`。操作の結果の効果行は `{Error}` で、handler が無ければ runtime の既定実装が応え、失敗は `Error` を raise する。利用者は `raise` と同じく無修飾で呼ぶ（包みと `module` は置かない）。handler は差し替えに使え、E605 は全操作の実装を要求する | §49, §57, §58 |
| D3 | main | E706 の免除を prelude の効果に広げる（`Error` `Env` `Io`） | §5 |
| D4 | 単位 | stdin とファイルは Bytes、パスと argv は String。非 UTF-8 の名前は `Error` | §36, §57 |
| D5 | 終了コード | 0 / 1 のまま | §51 |
| D6 | 操作名 | `args` `env` `cwd` `read_file` `write_file` `read_dir` のような素直な名前。同名のユーザー宣言は E205 | §47, §49, §57 |

**E. 版と受け入れ**

| ID | 論点 | 決定 |
|---|---|---|
| E1 | tag | 4 テーマを v0.3 に入れ、tag は 1 つ。中間 tag は打たない |
| E2 | 受け入れ | 門番結果の要約プログラム 1 本を `tools/` に置く。argv のディレクトリのログを読み `PASSED: n/N` を集計し、合計と赤の一覧を印字する。ライブラリ単位（A）と shared（B）を同じ 1 本で使う。判定はシェル側 oracle との一致 |
| E3 | 診断と fmt | 入れない（言語意味を増やさない項目は版を分ける） |

### 24.2 仕様の項目

コーパスの `item:` タグに使う項目名。2.2、9.2、11.2、13.2、18.4 の項目に加える。

| 項目 | 仕様の節 | 内容 |
|---|---|---|
| ライブラリ単位 | §5, §27, §54, §59 | `library` 宣言、check と build、公開面の根、非公開 def の E513 / E514 |
| 型の共有 | §5, §34, §35, §56 | `shared` 実体化、functor の同一性、印なしの不透明 |
| Bytes | §36, §45, §49, §51 | 組み込み型、長さと変換、I/O、印字 |
| 配列 | §36, §45, §49, §51 | `T Array`、get / set、派生、印字 |
| String 操作 | §49 | slice / 索引 / split / starts_with / contains |
| Int.parse | §49 | `String -> Int {Error}` |
| Env と Io | §5, §47, §49, §57, §58 | 効果、操作と既定実装、失敗の綴り、handler、main に残せる効果 |

### 24.3 やらないこと

- wants の「高」以降: 診断の改善（1〜9）、fmt / run / LSP（10〜21）、終了コード（25）、時刻・乱数（26）、文字列補間（30）、多行文字列（31）、連想配列（33）、スタック溢れの `{Error}` 化（35）、ビット演算（36）、加減乗の再設計（37）、inliner（46）、Cranelift 版上げの手順化（47）
- 6 節の「v0.2 へ保留」の残り（参照カウント、区間推論、record の field 更新、functor の first-class 化、JIT、`as` パターン、型の別名）
- 11.3 と 13.3 のやらないこと（親ディレクトリの無修飾参照、幽霊型、List の中間展開、first-class module）
- 兄弟ライブラリの参照、ライブラリのリンク可能成果物、設定ファイル、検索パス
- 効果の推論の最適化、multi-shot resumption、segmented stacks

### 24.4 細則（可逆。異議があれば戻す）

- `library` は外（最初の見出しより前）の宣言。1 ディレクトリに 1 つで、複数ファイルの宣言は重複として拒否する。ライブラリに `main` は無い。`.hdi` の規則は §54 のまま
- `shared` は契約の抽象型の実体化にだけ前置できる。constructor 付き本体への `shared` は E100、契約に無い型への `shared` は E204。正準形の印字と診断は解けた型を出す（`StringMonoid.T` ではなく `String`）
- `shared` に冗長の規則は当てない（型注釈と同じ宣言的な印）
- ライブラリの `--print-type` は何も印字しない。`build` の E206 メッセージは entry 向けに分ける
- `Bytes` / `Array` は予約名。ユーザーの同名宣言は E205
- `Bytes -> String` など組み込みの失敗は prelude の包みで `Error` に持ち上げ、`message` の綴りを `"DivisionByZero"` の並びで仕様に書く
- `Env` / `Io` の操作の型は §49 に固定した（1 層。結果の効果行は `{Error}`、利用者は無修飾で呼ぶ。`module`・包み・`Attempt` は置かない）。組み込みの失敗の `message` の綴りも §49 の表に固定した
- E206 コーパスは `library` を宣言したディレクトリの `main` に差し替え、`// item: ライブラリ単位` を付ける。「複数の入口」と「型の共有」は拒否形が無いので `docs/corpus.md` の免除表に足し、判定器のプローブで測る
- `verify-m1-spec.sh` の検査 2 の節番号の上限を §59 まで広げる。`r9_grammar` の生成器を R32 / R33 で更新する。`tests/library` を回す門番を R32 で決める。`verify-m1-corpus.sh` の検査 6 の `// needs: prelude` の member 一覧を R34 で §49 から導く形に更新する
- E206 を期待する既存の判定器（`r17_files.rs:119`、`r20_entries.rs:160` と `:169`、`r22_namespaces.rs:186`、`check_cli.rs:237`、`r9_names.rs:29`、`verify-r20.sh` の 20-b(4)）を A1 の結果に書き換える（R32）。`tests/accept/effect_row` の `effect Io` と `r24_effects.rs` の 8 件は prelude の `Io` と E205 になるので、R37 の前に書き換える
- 受理の `// item:` は、ライブラリ単位を R32、型の共有を R33、Bytes と String 操作を R34、配列を R35、Int.parse を R36、Env と Io を R37 で置く
- 新しいエラーコードは足さない見込み（`library` の重複は E205、`shared` の不正形は E100 / E204 で足りる）。必要になったら R32 で報告する
- 24.2 の Bytes・配列・String 操作・Int.parse・Env と Io は失敗が実行時の `Error` なので拒否コーパスを持たない（`docs/corpus.md` の免除表に足した）。受理側の `// expect-error:` で測る
- `docs/corpus.md` の `// item:` は 24.2 を足し、ライブラリの受理は `tests/library/` に置く。lock の `find` に `tests/library` を足す
- `docs/wants.md` の最優先 6 項目のうち 22 23 24 27 28 29 32 43 は本版、残りと 25 26 30 31 33 34〜37 は次の版

**レビューで変わった点（claude-fable-5-1、2026-09-12）**

| 指摘 | 対応 |
|---|---|
| 致命: A3 × A5（外の def とサブディレクトリが埋め込みで壊れる） | A7 A8 を追加。`library` 宣言と最内根のパス解決でポータブルにし、兄弟参照はしない |
| 重大: A1 が E206 コーパスを壊す | `tests/reject/E206` を `library` を宣言したディレクトリの `main` に差し替え、`// item: ライブラリ単位` を付ける。引数付き main と `main` 無しの build 拒否は `verify-r32.sh` のプローブへ移す |
| 重大: A6 は 1 文で閉じない | A6 を「単体検査」と「連結時の根」の 2 規則にした |
| 重大: A2 の根に公開面が載らない | A2 に型・効果・handler を足した |
| 重大: B1 の非対称が 3 状態を作る | B1 を `shared` の一様な印に変えた |
| 重大: C3 は関数 5 つの最小より大きい | R35 に型多相 intrinsic と要素幅、`of_list` / `to_list` の派生を明記 |
| 重大: D2 の失敗は C から raise できない | 文書レビューを経て 1 層にした（操作が結果に `{Error}` を持つ） |
| 重大: E2 が R38 まで書けない | E2 を R32 で起票し、各ラウンドの判定に E2 の該当部分を足す |
| 重大: 操作名が識別子と衝突する | D6 |
| 重大: Bytes / Array の印字が未決 | C7 |
| 軽微: 非 UTF-8 の名前 | D4 |
| 軽微: ライブラリのコーパス形式 | `tests/library` を新設（check のみの判定器） |
| 軽微: `--print-type` / build メッセージ | 24.4 |
| 未確認 7 件 | 24.9 に置き、実装ラウンドで確認する |

**文書レビュー（2026-09-12、claude-fable-5-1）で変わった点。** 致命 1 は「§49 の包み `def read_file` の本体が同名の操作 `read_file` を指せず E201 になる」で、1 層にして `module`・包み・`Attempt` を消して解いた。重大 13・軽微 15 のうち決定が要ったのは公開面だけで、A2 に未適用の functor と実装の無い契約を足した。残りは文言・表・節ポインタ・門番の追随として直した（§49 の member の型と失敗の `message` の表、`String List Attempt` の括弧の解消、§47 の効果の空間、§58 の prelude 効果の handler の位置、E206 の item の付け替え、等）。

### 24.5 ラウンドの骨格

各ラウンドの判定器 `verify-r32.sh` 以降はこれから作る。**未検証。**

| ラウンド | 内容 | 主に触る所 | 判定の柱 |
|---|---|---|---|
| R32 | ライブラリ単位（A） | `check.rs` `project.rs` `dependencies.rs:42` `names.rs` `crates/hado/tests/r9_grammar.rs` `stdlib/`、§5 §27 §54 §59 | `library` 宣言、check と build の分岐、公開面の根、`tests/library` の新設と門番、埋め込みの受理 1 本を `tests/accept/` に、E206 の書き換え、E2 のソース起票、`--print-type` と build メッセージ、生成器の `library` |
| R33 | 型の共有（B） | `infer/functors.rs` `type_realizations.rs` `repr.rs:19` `crates/hado/tests/r9_grammar.rs`、§35 §56 | `shared` 実体化が 24.7 の探針を受理し実行値が一致。印なしの `type T = M.T` は適用ごとの別抽象型に戻る。`shared` の診断と印字、生成器の `shared` |
| R34 | Bytes と String の操作（C2 C4） | `runtime/` `runtime_abi.rs` `stdlib/string.hd` 新 `stdlib/bytes.hd`、§36 §43 §49 §51 | Bytes の型（長さ・変換・索引・of_list / to_list・比較・印字）と String の 5 操作。範囲外は `{Error}`。E2 の集計部（純粋）を `tests/accept/` に置く |
| R35 | 配列（C3） | `mono.rs` `repr.rs` `emit.rs` 派生パス、§36 §45 §49 §51 | 型多相 intrinsic と要素幅、`of_list` / `to_list` の派生。3 万要素の索引と `set` が SIGSEGV しない。E2 の集計部を通す |
| R36 | Int.parse（C5） | `stdlib/int.hd` `runtime/`、§49 | 不正と桁溢れが `Error`、`Int.parse "42"` が `Some(42)` 相当（18.2 の例）。E2 の集計部を通す |
| R37 | 世界との境界（D） | `stdlib/` の効果宣言、`runtime/`、`mono.rs` の既定実装、§5 §47 §51 §57 §58 | `Env` / `Io` の操作（`reply`、結果の効果行 `{Error}`、handler 無しの既定実装）、argv / env / cwd / stdin / ファイル / ディレクトリ、非 UTF-8 は `Error`。E2 が完成して check と build を通る。受理ケースは空 argv・閉じた stdin・ケース内の相対パスで起動する |
| R38 | 仕上げ | E2 の実行、`docs/`、lock、tag | 全門番が満点、`cargo test` 0 failed、oracle 一致、`v0.3` を tag |

### 24.6 壊すもの

- §49 の E513 免除（A6）と §27 の根の規則（A2 / A3）
- §5 の E206 の意味（A1 / A4）と E705 の範囲（B4）
- §35 の不透明（B1 の `shared` が例外）、§56 の functor の型（`shared` が同一性を作る）
- §54 の「渡したファイルに `main` が無ければ E206」（check と build で分かれる）と §55 の「根は 1 つ」（A7 / A8）
- `tests/reject/E206`、`docs/spec-trace.md` のコーパスのパスと「固定するもの」、`docs/spec-coverage.md` の表題、`docs/corpus.md` の配置
- `verify-m1-spec.sh` の検査 4 と `verify-m1-corpus.sh` の検査 3 は表番号を直書きしている。24.2 の番号を足す
- `docs/wants.md` の優先度の行

### 24.7 着手時点の実測（2026-09-12、`a004704`）

`cargo run --offline -q -- check <file>` の出力の末尾。

| 探針 | 結果 |
|---|---|
| `main` の無い `.hd` を `check` / `build` | `E206: main must be a value definition`（1:1） |
| `main` があり、根の別ファイルの実装が未参照 | `E513: unused top-level declaration`（実装の見出し） |
| functor `MakeFold(M: Monoid)` の `type T = M.T` を適用し `StringFold.fold ["a", "b"]` | `E305: cannot unify Fn(StringFold.T List => StringFold.T) with Fn(String List => t328)` |
| functor 無し。`implement Monoid as StringMonoid` の `type T = String` で `StringMonoid.combine "a" "b"` | `E305: cannot unify Fn(StringMonoid.T, StringMonoid.T => StringMonoid.T) with Fn(String, String => t307)` |
| 同上の functor に `StringMonoid.of "a"` を渡す | `E305: cannot unify Fn(StringFold.T List => StringFold.T) with Fn(StringMonoid.T List => t342)` |

組み込みは `runtime_abi.rs` の 16 本（印字 5、変換 3、String 3、他）で、`mono::Type` は `Int Float Bool Char String Unit Data Abstract Closure` の 9 variant。配列と Bytes の variant は無い。

### 24.8 文書改訂後の門番（2026-09-12 実測）

本節の決定で仕様と計画を改訂した直後に全 30 門番を回した（`CARGO_TARGET_DIR=/tmp/hado-v03/target`、ログは `/tmp/opencode/v03-gates/`）。赤は 7 本で、すべて「仕様先行」の期待どおりである。R32〜R38 で green に戻す。

| 門番 | 赤のチェック | 原因 | 戻すラウンド |
|---|---|---|---|
| `cargo test` | `r23_syntax::keywords_cannot_be_names`（1 failed、他 430 passed） | このテストは §41 のキーワード列を仕様から読む。`library` `shared` が増え、v0.2.3 の lexer が名前として受理する | R32（`library`）、R33（`shared`） |
| `verify-m0.sh` | 2 cargo-test、3 walk、4 tail（4/7） | `cargo test` の終了コードを見る | 同上 |
| `verify-m1-spec.sh` | 1 headings（4/5） | 表題 `v0.2` → `v0.3` と §59 の新設を `git diff` が削除と数える。この改訂をコミットすると解消する | 本改訂のコミット |
| `verify-m1-corpus.sh` | 3 pairs（accept と reject が無い）、7 no-regressions（m0 が赤）（5/7） | 24.2 の 7 項目にコーパスが無い | R32〜R37 |
| `verify-m2.sh` | 5 gates（7/8） | m0 m1-spec m1-corpus が赤 | 同上 |
| `verify-m3.sh` | 9 gates（11/12） | 同上 | R32〜R37 |
| `verify-m7.sh` | 3（1/3） | stdlib に String の 5 操作・Bytes・Array・`Int.parse` の宣言が無い。§49 の関数集合ブロック全体を読む | R34〜R36 |
| `verify-r23.sh` | 23-c（2/3） | `r23_syntax::keywords_cannot_be_names` の連鎖 | R32 / R33 |
| 残り 22 本 | 無し | — | — |

`m2` の 5 と `m3` の 9 は `m0` の連鎖で、独立の赤は 5 本（keywords、m1-spec の diff、24.2 のコーパス、stdlib の関数、それらの連鎖）である。

**R32 の終了時（2026-09-12、`7c95f25`）:** 赤は `verify-m1-corpus.sh` 6/7（3 pairs の 6 項目）、`verify-m2.sh` 7/8、`verify-m3.sh` 11/12（どちらも m1-corpus の連鎖）、`verify-m7.sh` 2/3（stdlib の関数）の 4 本。`cargo test`、m0、m1-spec、r23 は緑に戻った。

**R33 の終了時（2026-09-12、`2133cae`）:** 赤は R32 と同じ 4 本で、内訳が縮んだ。`verify-m1-corpus.sh` 6/7 の 3 pairs は 5 項目（Bytes・配列・String 操作・`Int.parse`・Env と Io）になり、**「型の共有」が消えた**。`verify-m2.sh` 7/8 と `verify-m3.sh` 11/12 はその連鎖、`verify-m7.sh` 2/3 は stdlib の関数。受理コーパスが 4 本増えたので `verify-m4.sh` 114/114、`verify-m5.sh` 145/145、`verify-m6.sh` 71/71（いずれも満点）。`cargo test --offline --no-fail-fast` は 0 failed。

**R34 の終了時（2026-09-13、`e976c6f`）:** 赤は 4 本のまま、内訳がさらに縮んだ。`verify-m1-corpus.sh` 6/7 の 3 pairs は 3 項目（配列・`Int.parse`・Env と Io）になり、**Bytes と String 操作が消えた**。`verify-m2.sh` 7/8 と `verify-m3.sh` 11/12 はその連鎖。`verify-m7.sh` 2/3 は `prelude inventory: 32/39` で、残る赤は Array の 6 項目（signature と 5 member）と `Int.parse` だけ（同じ門番の中の `PASSED: 94/94` は受理コーパスの本数を数える別の検査で、inventory ではない）。`verify-m4.sh` 123/123、`verify-m5.sh` 145/145、`verify-m6.sh` 71/71、`cargo test --offline --no-fail-fast` 0 failed。なお `verify-m3.sh` の `cargo test` の上限を 180 秒から 600 秒に延ばした（実測 250 秒。時間切れは `NG: timeout` として出るようにし、判定の条件は変えていない）。

**R35 の終了時（2026-09-13、`9450261`）:** 赤は 4 本のまま、内訳がさらに縮んだ。`verify-m1-corpus.sh` 6/7 の pairs は 2 項目（`Int.parse`・Env と Io）になり、**配列が消えた**。`verify-m2.sh` 7/8 と `verify-m3.sh` 11/12 はその連鎖。`verify-m7.sh` 2/3 は `prelude inventory: 38/39` で、残る NG は `Int.parse` だけ。`verify-m4.sh` 128/128、`verify-m5.sh` 145/145、`verify-m6.sh` 71/71、`verify-r34.sh` 1360/1360、`verify-r35.sh` 714/714、`cargo test --offline --no-fail-fast` 0 failed。

**R36 の終了時（2026-09-13、`fa8fddf`）:** 赤は 3 本に減った。`verify-m1-corpus.sh` 6/7 の pairs は 1 項目（Env と Io）だけになり、**`Int.parse` が消えた**。`verify-m2.sh` 7/8 と `verify-m3.sh` 11/12 はその連鎖。**`verify-m7.sh` が 3/3 で満点に戻った**（`prelude inventory: 39/39`。§49 の関数集合をすべて満たした）。`verify-m4.sh` 131/131、`verify-m5.sh` 145/145、`verify-m6.sh` 71/71、`verify-r34.sh` 1360/1360、`verify-r35.sh` 714/714、`verify-r36.sh` 388/388、`cargo test --offline --no-fail-fast` 0 failed。

**R37 の終了時（2026-09-14、`d8f6594`。全走 04:23:56→05:56:51 の 93 分）: 赤はゼロ。全門番が満点になった。** 実装審査は 3 工程が 1 巡ずつ、工程 3 が 3 巡で合格した。 `verify-m1-corpus.sh` が **7/7**（pairs から Env と Io が消えた）、`verify-m2.sh` が **8/8**、`verify-m3.sh` が **12/12**。m0 7/7、m1-spec 5/5、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r9〜r37 の 24 本すべて満点（`verify-r34.sh` 1360/1360、`verify-r35.sh` 714/714、`verify-r36.sh` 388/388、`verify-r37.sh` 254/254）、`cargo test --offline --no-fail-fast` 0 failed。**24.5 が R38 の入口に置いた「全門番が満点」がここで成立した。**

**`verify-m3.sh` の検査 11（`stdlib-types`）が一度赤になった。門番ではなく `stdlib` の形を直した。** この検査は `stdlib/*.hd` の最初の `module` / `implement` より前に `def` があると落ちる。既定実装を素の `def` として並べる形がこれに当たった。`stdlib/env.hd` と `stdlib/io.hd` を `module EnvDefault` / `implement EnvDefault as EnvDefault` の形に直して解消した（台帳の R37 に理由）。

### 24.9 不確実性

- 確定: 前提（`v0.2.3`、`docs/bug.md` 空）、24.7 の探針、24.1 の決定、24.4 のレビュー対応
- 推定: 版番号 v0.3、24.5 のラウンド分割と判定の柱、24.6 の追随
- R32 で確定（`docs/ledger.md` の R32）: `trusted_start` は行の閾値で、prelude 無しの検査は `stdlib/` を渡したときだけ存在し、E205 / E202 で通らない（門番は使っていない）。`types.rs:515-519` の `Abstract` の単一化は名前比較。ユーザーの `type Bytes` / `type Array` は今は受理（R34 / R35 で E205）。全門番の `PASSED: n/N` 書式は一致（`tools/gates` が集計できた）。不透明な抽象型どうしの `==` は受理して `true`。`.hdi` の `library` は `role_error` で E100。外の `def` の E100 は `Project::load` で決める（単一ファイルの parse では決められない）
- R33 で確定（`docs/ledger.md` の R33）: `types.rs:515-519` の単一化（名前比較）は触らずに済んだ。`shared` の適用の循環は E602（v0.2.3 に無い形なので `bug.md` 行きではない）。functor の契約の arity は `abstract_types` からは取れず署名（`ModuleSig`）から取る。前処理の等式は `Ty::reveal` の再帰置換で実効的に固定点になる。**教訓 2 つ:** 不透明型に等式を公開するときは等式の収集を型と効果の登録より前に置く（消費点に散らすと閉じない）。同じ意味の型解決を 2 本持つと片方の検査の写し落としが panic になる（`shared_body_type` は `annotation_inner` の別実装で、`Fn` の枝・arity・契約名の 3 種を写し落とした）
- R34 で確定（`docs/ledger.md` の R34）: `BUILTIN_TYPES` に足すだけで `type Bytes` は E205（位置は 2 回目の宣言の名前）。`Bytes.of_list` は確保 + 充填で 3 万要素が 76ms。組み込み型の解決は `types::builtin_type` の 1 本に寄せた。**prelude の効果の操作がサブディレクトリから見えない R32 由来の欠陥を直した**（`namespaces.rs` の `prelude_operations`、§47・§57）。**教訓 4 つ:** 入力の形が列挙できる判定は手順書の段階で生成規則として書く。境界値は乱数任せにせず決定的に含める。期待値は独立した oracle（python3 の decoder や同名操作）から導く。門番の時間切れは判定の失敗と区別する
- R35 で確定（`docs/ledger.md` の R35）: emit は cranelift なので、要素の load 型を `repr::machine_type` から決められ、C の ABI の幅問題は存在しない。`type A Arr = A Box` の別名は parser が拒否する（`expected a record or a leading | for an ADT`）。`type_arities` に名前を直接登録するだけで引数付きの注釈が通り、`infer.rs` の「引数付きは未実装」の門の手前で解決できる。`BUILTIN_TYPES` に足すだけで E205 になる（Bytes と同じ）。`infer/functors.rs` の `shared_body_type` は既存の arity 参照で引数付きの組み込み型を通すので、Array では写し落としが起きなかった。`composite_types` の span 地図（`infer.rs:945` が Apply / Call を記録）は結果しか多相でない intrinsic の要素型を運べる（`specialization.resolve` が要る）。**教訓 1 つ:** R34 の教訓（分類からの生成・決定的な境界値・独立した oracle）を手順書の段階で書くと、判定器が 1 巡目から穴を持たない。R34 は審査 14 巡・指摘 26 件だったが、R35 は 3 工程とも 1 巡で合格し、注入 35 件をすべて検出した
- R36 で確定（`docs/ledger.md` の R36）: 単相の intrinsic は `runtime_abi.rs` の 5 箇所（enum / `intrinsic_name` / `value_signature` / `ALL` / `declaration`）に足すだけで通り、R35 の `is_polymorphic()` の 4 分岐を触らずに済む。`HadoString` は NUL 終端でないので `strtoll` に渡せず、先行ゼロを無制限に受理する決定があると固定長のバッファにも写せない（直接走査になる）。C の整数除算は 0 方向に切り捨てるので、負の値では `ceil` になり `acc < (INT64_MIN + digit) / 10` がちょうど境界を表す。**教訓 1 つ:** 数値の境界は、式を自分で展開することと審査に出すことの両方が要る。R36 の致命（正の入力で符号を戻す段の溢れ）は、指揮者が式を展開して見つけたのと、審査が算法を C に書いて実測したのとで、独立に同じ結論に達した
- R37 で確定（`docs/ledger.md` の R37）: handler の腕は `with` で被せたときしか推論されない（`infer.rs:1969` の `handler_answer` は `with_handler` からしか呼ばれず、`dependencies.rs:17` と `infer.rs:175` `:186` は `Item::Handler` を無視する）ので、**handler を既定実装には使えない**。E605 は腕名しか見ないので型の保証にならない。既定実装は普通の `def` にし、`module <効果名>Default` / `implement` の中に置く（`stdlib/*.hd` は最初の `module` / `implement` より前に `def` を置けない。`verify-m3.sh` の検査 11）。module member は `values` ではなく `module_definitions` に入る。prelude の契約名は自動で E205 になるので `namespaces.rs` に足す必要は無い。ローカル束縛の影は mono（`operations` を `scope` より先に見る 4 箇所）と infer（`unwrap_operation`）の両方にあり、infer 側は「外側の `Fn` の結果が登録済みの効果 ADT か」を見る 4 行で足りる。**教訓 2 つ:** 直す層は、症状の出る層ではなく実測で特定する（`namespaces.rs` → mono → infer と 2 度外した）。受理ケースを起動する経路は `scripts/` だけでなく `crates/hado/tests/corpus.rs` にもある（grep の範囲を先に確かめる）
- 未確認（実装ラウンドで確認する）:
  - `%` を使う未注釈の再帰生成器が E306 になる（R35 の工程 0 で踏んだ。原因未調査。R38 で判断する）
  - 長さが違う Array の `compare` の順序を §45 が定めていない（R35 は python3 のリスト比較に合わせた。R38 で §45 に 1 文足すか判断する）
  - 効果が `dependencies.rs` の宣言に数えられておらず未使用の効果が E513 にならない件（§50 は効果にも当てる。R37 の D で当たったら `docs/bug.md` に置く）
- R31 の台帳の未確認 3 件（型引数付き効果の重ね、`fn` の本体の `capture`、handler の値引数）は本版の範囲外で、D の設計で当たったら `docs/bug.md` に置く

## 25. R32 ライブラリ単位の手順書（2026-09-12）

24.1 の A（A1〜A8）と 24.4 の細則を実装する。`v0.3` の最初のラウンドで、tag は R38 まで打たない。

**読む順:** 本節 → 24.1 の A 表と 24.4 → `docs/spec.md` の §59・§54・§55・§27・§5・§41 の `program` → `docs/corpus.md` の冒頭（`tests/library/` の規約）。

**着手時点で赤の門番:** 24.8 の 7 本（`cargo test` の `keywords_cannot_be_names`、m0、m1-spec、m1-corpus、m2、m3、m7、r23）。m1-spec の見出しは 24 節の改訂のコミット `8f49366` で解消しているはずで、R32 の入口で単独で測り直す。

### 決定

- **工程は 4 つ。字句と構文（工程 0）、check と build の分岐と公開面（工程 1）、ライブラリ根（工程 2）、コーパスと道具（工程 3）。** 工程ごとに指揮者が緑を実測してから次を発注する
- **`library` と `shared` を同時に予約語にする。** `keywords_cannot_be_names`（`r23_syntax.rs:144`）は §41 のキーワード列を 1 つのテストで回すので、`library` だけ予約しても赤のまま m0・m2・m3・r23 に連鎖する。`shared` の構文と意味は R33 で、R32 では字句だけ予約する（`shared type T = ...` は R32 では E100 のまま）
- **check と build の分岐は `Project` に「検査の種類」を持たせて作る。** 今は `main.rs:5-16` が 4 つの bool に畳んだ後、`Project::check` / `check_for_print` / `dump_mono` / `compile_to_object`（`project.rs:37-56`）が全て `check_parsed_filtered`（`lib.rs:174`）に落ちる。命令に依存する唯一の項は E706 を落とす `reject_user_main_effects` である。渡したファイル（file 0）に `main` という `def` があれば入口検査（引数付きなら §5 の E206 でその `def`）、無ければライブラリ検査とし、`build` と `dump-mono` はライブラリ検査のとき E206 を 1:1 で返す（メッセージは entry 向け: `build requires an entry file with main; check treats this directory as a library` の趣旨）。`--print-type` はライブラリ検査では何も印字せず終了コード 0
- **E206 は 2 箇所にある。** `names.rs:413-422` の `entry_form`（`main` 無し → 1:1、引数付き → その `def`）と `infer.rs:206-212` の `main is not defined`（`?` で推論を打ち切る）。ライブラリ検査では両方とも「`main` 無し」を通し、引数付き `main` の E206 は残す（§5 は変わらない）。`library` を宣言したディレクトリの `main` は、どのファイルにあっても E206 でその `def` を指す（他のファイルの `main` は `fileN$main` に改名されている。`file_scopes.rs:41-47`）
- **ライブラリ検査の E513 の根は公開面（A2）。** `dependencies.rs:24-27` は `"main"` と `other_entry_roots` を根にする。ライブラリ検査の根は、通常の実装・functor の適用結果・未適用の functor・実装の無い契約・外の型・効果・handler の全てである。外の `def` は根にしない（ファイル内で実装や他の外の `def` から辿れなければ E513。今の `fileN$` の機構のまま）。`other_entry_roots`（`file_scopes.rs:10-14`）はライブラリ検査では空にし、宣言の無いディレクトリの `main` を普通の `def` にする（§59）。E514（`infer.rs:606-608`）と実装局所の型の E513（`dependencies.rs:81-87`）は変えない
- **prelude の根（A6）は `.skip(prelude_items)` のままでよい。** 振る舞いは「prelude の項目は E513 にならない」で同じである。§49 の文言に合わせて、`dependencies.rs:34` と `:44` のコメントに「prelude は公開面として根」と書く。これ以上の構造変更はしない
- **ライブラリ根（A7 / A8）は `Project::load` で決める。** `library` は外の宣言なので、`role_error`（`project.rs:184-197`）と同じトークン走査で「そのディレクトリのどのファイルが `library` を持つか」を組み立ての前に集められる。ディレクトリごとに 1 つ（2 つ目は E205、ファイル順で後のもの。同一ファイルの 2 つ目も E205）。`.hdi` の `library` は E100（`role_error` の `interface` の枝）。`library` 自身は `outer` を false にしない（`project.rs:195` の見出し集合に足さない）
- **ライブラリ根の中のパス解決。** `namespaces.rs:145-161` の `resolve_global` は、無修飾を「自分のディレクトリ → 根 `[]`」、修飾を根からの絶対パスで解く。ライブラリ根の中の項目は、無修飾を「自分のディレクトリ → 最内のライブラリ根」、修飾を最内のライブラリ根からの絶対パスで解く。利用側の根 `[]` の宣言は見えない（E204）。prelude の項目は `[]` に置かれているが、どこからも無修飾で見える（§49）ので、prelude だけは別扱いで最後に引く。根との同名の E205（`namespaces.rs:108-113`）は、ライブラリ根の中ではライブラリ根の宣言と prelude の名前に対して数える
- **外の `def` の E100（A3）は `role_error` の `root_file` を「根のディレクトリ、またはライブラリ根のディレクトリ」に広げる。** `root_file` は今も「入口ファイル」ではなく「根のディレクトリのファイル」（`project.rs:175`）なので、集合を広げるだけでよい。24.9 の「単一ファイルの parse で決められるか」の答えは「決められない。組み立て（`Project::load`）で決める」で、既にそうなっている
- **`Item` に足すか `Ast` の欄にするか。** `library` は名前も本体も持たないので、`Ast` に `library: Option<Span>` を持たせる形を推す。`Item::Library` にすると `dependencies.rs:11-19`、`namespaces.rs:42-70`、`file_scopes.rs:15-23` の網羅 match に腕が増える。builder が選んでよいが、選んだ理由を報告に書く
- **E206 コーパスは工程 0 で差し替える。** 今の `tests/reject/E206/main.hd` は「`main` 無しの check」で、工程 1 の A1 で受理に変わる。工程 0 で `library` を宣言したディレクトリの `main` に差し替え（`// spec: §59`、`// item: ライブラリ単位`）、lock を更新する。新形は工程 0 の時点で拒否される。ただし `parse_corpus.rs:121-129` は拒否コーパスの `main.hd` を単一ファイルとしてコンパイルして失敗を確かめるので、E206 の根拠が `lib.hd` にある新形では通ってしまう。この検査を `corpus.rs` の `try_reject` と同じくディレクトリ単位（`Project::load`）に変える（工程 0 の builder が捕まえた計画の漏れ）。**AGENTS.md の「lock を実装の工程で更新しない」は、工程 0 と工程 3 に限って解除する**
- **E2 のソースは `tools/gates/` に起票する。** `tools/gates/main.hd` が入口、`tools/gates/report/report.hd` が `library` を宣言するライブラリで、契約 `Report` と実装 `Report` を持つ。R32 では純粋な集計だけ（`Tally` record、`add`、`render`）で、`main` は固定の 3 つ組のリストを畳んで印字する。R34 で `PASSED: n/N` の行の解析、R36 で `Int.parse`、R37 で `read_dir` / `read_file` を足す。R33 で `Tally` を契約の抽象型にし `shared` で公開する

### 期待値の根拠

| 形 | 導出 | 根拠 |
|---|---|---|
| `main` 無しの `.hd` を `check` | 受理（診断なし、終了コード 0） | §5、§54、A1 |
| 同じファイルを `build` / `dump-mono` | E206、1:1 | §5、§54、A4 |
| 同じファイルを `check --print-type` | 何も印字せず終了コード 0 | 24.4 |
| `library` のディレクトリの `main` | E206、その `def` | §59、§50 |
| `library` の 2 つ目（同一ファイル / 別ファイル） | E205、2 つ目 | §59、§50 |
| `.hdi` の `library` | E100 | §59 |
| ライブラリ検査で未参照の実装・functor・契約・外の型・効果・handler | 受理 | §27、A2 |
| ライブラリ根のファイルの外の `def` が未使用 | E513 | §54、§59 |
| 根でもライブラリ根でもないファイルの外の `def` | E100 | §54 |
| 宣言の無いディレクトリを check、他ファイルに `main` | その `main` は普通の `def`（未使用なら E513） | §59 |
| 埋め込み: `app/main.hd` から `app/lib/`（`library`）の実装を `lib::store::X.f` で参照 | 受理して実行 | §55、§59、A8 |
| 埋め込み: `app/lib/` の中から `app/main.hd` の外の型を無修飾で参照 | E204 | §55 |
| 埋め込み: `app/lib/` の中から兄弟 `app/other/` を参照 | E204 | §55、§59 |
| 埋め込み: 入口検査で `app/lib/` の未参照の実装 | E513 | §59 |
| `def library = 1`、`def shared = 1` | E100 | §41 |

**本体では 1 つも通っていないので、期待値は仮説である。実測と食い違ったら、実装を疑う前に期待値を疑うこと。**

### 成果物

`crates/hado/src/`（`lexer.rs` `parser.rs` `project.rs` `lib.rs` `main.rs` `names.rs` `infer.rs` `dependencies.rs` `file_scopes.rs` `namespaces.rs`）、`crates/hado/tests/r32_library.rs`（新設）、既存テストの書き換え（下記）、`crates/hado/tests/parse_corpus.rs`（拒否の検査をディレクトリ単位に）、`crates/hado/tests/r9_grammar.rs`、`crates/hado/tests/corpus.rs`、`scripts/verify-r32.sh`（新設）、`scripts/verify-m2.sh:14`（`tests/library` を足す）、`tests/reject/E206/`、`tests/accept/`（埋め込みの受理 1 本）、`tests/library/`（新設）、`tests/corpus.sha256`、`tools/gates/`。

### 工程

- 工程 0: 字句と構文。`lexer.rs:539-563` に `library` `shared`。parser の `outer_item` に `library`。`.hdi` の `library` は E100、2 つ目は E205、`library` のディレクトリの `main` は E206。E206 コーパスの差し替えと lock。判定 32-b 32-c 32-g。`keywords_cannot_be_names` が緑になる
- 工程 1: check と build の分岐（A1 A4）と公開面（A2）。`entry_form` と `infer.rs:206-212` の条件化、`main.rs` からの検査の種類の伝搬、`--print-type`、build のメッセージ、`dependencies.rs:24` の根、`other_entry_roots` の扱い。E206 を期待する既存テストの書き換え。判定 32-a 32-d 32-e
- 工程 2: ライブラリ根（A3 A7 A8）。`Project::load` でのライブラリ根の集合、`resolve_global` のパス解決、可視性、E205 の単位、`role_error` の `root_file`。判定 32-f
- 工程 3: コーパスと道具。`tests/library/` と runner、埋め込みの受理 1 本を `tests/accept/`、lock、`verify-m2.sh:14`、`r9_grammar` の `library`、`tools/gates/` の起票。判定 32-h 32-i 32-j

### 判定

判定 32-a: `main` の無い `.hd`（契約 1 つと実装 1 つ、外の `def` 1 つを実装が使う）を `check` して受理（stderr が空、終了コード 0）。`check --print-type` は stdout も空。`build` と `dump-mono` は E206 で 1:1、メッセージに `check` と `library` の語を含み、実行ファイルを作らない
判定 32-b: `library` を宣言した `lib.hd` と `def main = 1` の `main.hd` を同じディレクトリに置き、`main.hd` を `check` / `build` して E206 でその `def main` の位置。`lib.hd` を渡しても同じ（`main` は他ファイル）
判定 32-c: `library` の重複。(1) 同一ファイルに 2 つ → E205 で 2 つ目の位置。(2) `a.hd` と `b.hd` に 1 つずつ → E205 で `b.hd` の位置（渡したのが `a.hd` のとき）。(3) `.hdi` に `library` → E100
判定 32-d: 公開面。ライブラリに未参照の実装、未適用の functor、実装の無い契約、外の型、効果、handler を 1 つずつ置いて受理。負の対照: (1) 契約に無い実装の `def` → E514、(2) 実装局所の ADT が未使用 → E513、(3) ライブラリ根のファイルの外の `def` が未使用 → E513、(4) 根でもライブラリ根でもないサブディレクトリのファイルの外の `def` → E100
判定 32-e: 宣言の無いディレクトリ。`a.hd`（`def main = 1`）と `lib.hd`（契約と実装）を置き `lib.hd` を `check` して E513 で `a.hd` の `def main`。`a.hd` の `main` を実装から参照する形は書けない（外の `def` はファイル局所）ので、`a.hd` を消せば受理
判定 32-f: 埋め込み。`app/main.hd`、`app/lib/lib.hd`（`library`、外の型）、`app/lib/store/memory.hd`（実装。`id::IntId` をライブラリ根からの絶対パスで参照）、`app/lib/id/int_id.hd`（実装）。`app/main.hd` の `main` が `lib::store::Memory.find` を呼ぶ形を `check` → `build` → 実行して値が一致。負の対照: (1) `app/lib/store/memory.hd` から `app/main.hd` の外の型を無修飾で参照 → E204、(2) `app/lib/` の中から兄弟 `app/other/` の実装を `other::X` で参照 → E204、(3) `app/lib/extra/spare.hd` に誰も参照しない実装を置く → E513 でその実装（`Memory` が `IntId` を辿るので、`IntId` を未参照にする形では測れない）。**実行で印字する値は実行時に生成した整数をライブラリの `def` に埋め、期待値を判定器側で計算すること**（30-l・31-k と同じ規約）
判定 32-g: `def library = 1` と `def shared = 1` が E100（`keywords_cannot_be_names` が緑）
判定 32-h: `r9_grammar` の生成器が `outer_item` の `library` を生成し、生成した全プログラムを parser が受理する（`cargo test --offline --test r9_grammar`）
判定 32-i: `tests/library/` に 1 ケース以上。判定器は各ケースの代表ファイル（`library` を宣言したファイル。無ければパスのバイト順の先頭）を `check` して受理。`corpus.rs`（または新設のテスト）が同じ規則で回し、`cargo test` に含まれる
判定 32-j: E2 の起票。(1) `tools/gates/report/report.hd` を単体で `check` して受理。(2) 判定器が一時ディレクトリに `report/` を写し、実行時に生成した 3 つ組（名前・通過数・総数）の `main.hd` を書いて `build` → 実行し、`render` の出力が判定器側で計算した文字列と一致。書式は `<通過の合計>/<総数の合計> passed; red: <赤の名前を空白区切り>`、赤が無ければ `red: none`
判定 32-k: `cargo test --offline --no-fail-fast` が 0 failed
判定 32-l: 全門番を単独で走らせ、許容される赤（下記）以外が満点・終了コード 0
判定 32-m: `tests/reject/E206` の差し替え、`tests/accept/` の埋め込み 1 本（`// item: ライブラリ単位`、`// spec: §59`）、`tests/library/` の 1 本以上、`tests/corpus.sha256` の更新。`sha256sum --strict -c tests/corpus.sha256` が全件 OK

### 既存テストの書き換え（工程 1）

| 場所 | 今 | A1 の後 |
|---|---|---|
| `r17_files.rs:114-120` | 空の `other.hd` を渡すと 3 命令とも E206 1:1 | `check` は E513 で `main.hd` の `def helper`（`main` は普通の `def` で未参照、`helper` はそこからしか辿れないので、ファイル順で最初の未使用宣言は `helper`）。`build` `dump-mono` は E206 1:1 |
| `r17_files.rs:171-183` | `def helper x => x + 1` だけの `main.hd` を `check` して E206 | `check` は E513 で `def helper`（外の `def` が未使用）。`hado::check_source` も同じ |
| `r20_entries.rs:153-161` | `a.hd` `z.hd` に `main`、`empty.hd` を渡すと E206 | `check` は E513 で `a.hd` の `def main`、`build` `dump-mono` は E206 1:1 |
| `r20_entries.rs:163-170` | `support.hd`（契約の実装と外の `def`）を渡すと E206 | `check` は E513 で `entry.hd` の `def main`（宣言の無いディレクトリの `main` は普通の `def`）、`build` `dump-mono` は E206 1:1 |
| `r22_namespaces.rs:181-187` | `nested/empty.hd` を渡すと E206（根の付け替えの観測） | `check` は受理、`build` `dump-mono` は E206。根の付け替えは、`nested/` のファイルが `main.hd` の外の**型**を無修飾で参照する形を対にして観測する（`main.hd` を渡せば受理、`nested/` のファイルを渡せば E204。外の `def` はファイル局所なので型で測る） |
| `check_cli.rs:236-243` | E206 は 1:1 固定なので先頭改行で動かない | 新しい E206 コーパスは `def main` を指すので、この例外は到達しない。ヘルパを消してよい |
| `r9_names.rs:22-32` | `def f x => x + true` が E206 1:1 | この 1 件は `build` 相当の API で測るか消す。引数付きの 2 件は残す |
| `r13_conditions.rs:79` | 引数付き `main` が E206 | 変えない |

### 許容される赤

R32 の終了時に残ってよい赤は、24.8 のうち R33〜R37 に割り当てたものだけである:

- `verify-m1-corpus.sh` の 3 pairs（型の共有・Bytes・配列・String 操作・Int.parse・Env と Io の受理が無い）と、その連鎖の `verify-m2.sh` 5、`verify-m3.sh` 9
- `verify-m7.sh` の 3（stdlib に String の 5 操作・Bytes・Array・`Int.parse` が無い）

`cargo test`、m0、m1-spec、r23 は R32 で緑に戻す。それ以外の門番を 1 本でも赤くしたら R32 の欠陥である。

### スコープ外

- `shared` の構文と意味（R33）。R32 では予約語にするだけ
- 兄弟ライブラリの参照、ライブラリのリンク可能成果物、検索パス、設定ファイル（24.3）
- 診断の文言の統一、`--print-type` の書式の変更
- `tools/gates/` の I/O 部分（R37）

### 確認して報告（24.9 から）

- `trusted_start` はライブラリ検査でも同じ意味か（`lib.rs:135` の `stdlib` の判定と、prelude を prelude 無しで検査する経路 `trusted_start.is_none()`）
- `library` を `.hdi` に置いたときに E100 になるか（`role_error` で決まるか）
- 外の `def` の E100 が組み立てで決まること（`Project::load` の順序）
- ユーザーの `type Bytes` / `type Array` が今どう扱われるか（R34・R35 で E205 にする前の現状）

### 中断の条件

**namespaces の対称表（`(Directory, Space)` の表）を捨てて、ファイルごとの別の解決器に置き換える必要があると判断したら、直さずに中断せよ。** 想定しているのは、項目ごとに「最内のライブラリ根」を持たせ、`resolve_global` の根の参照 `[]` をその値に差し替える変更である。

**E206 の 2 箇所を条件化するために `check_parsed_filtered` を 2 本に複製する必要があると分かったら、工程 1 を止めて報告せよ。** 検査の種類は 1 つの値として持ち回る。

それで届かないと分かったら、どこまで確認したか・何が足りないかを具体的に書いて止める。止めることは正常な終了経路である。

## 26. R33 型の共有の手順書（2026-09-12）

24.1 の B（B1〜B5）と 24.4 の細則（`shared` の 3 項）を実装する。R32 で `shared` は予約語になっており、今は `implement` の中に書くと E100 `expected a declaration, found shared` になる（`parser.rs:685` の既定腕）。

**読む順:** 本節 → 24.1 の B 表と 24.4 → `docs/spec.md` の §35・§56・§34（`X.T`）・§5（E705）・§41 → 24.7 の探針 3 本。

**着手時点で赤の門番:** R32 の終了時の許容される赤（m1-corpus の pairs、m7、m2 / m3 の連鎖）だけ。R33 の入口で単独で測り直す。

### 決定

- **工程は 4 つ。構文（工程 0）、通常の実装の `shared`（工程 1）、functor と契約修飾型（工程 2）、コーパスと道具（工程 3）。** 工程ごとに指揮者が緑を実測してから次を発注する
- **`shared` は `TypeDef` の欄。** `parser.rs:384-390` の `TypeDef` に `shared: bool` を足す。`parse_items`（`parser.rs:664-696`）の `TypeContext::Implementation` でだけ `KwShared` を受理し、続く `type` の実体化に印を付ける。それ以外の文脈（外、契約、効果、handler、`Signature`）の `shared` は今までどおり E100。`functor_instances.rs:82` の `item` は `TypeDef` を clone するので、欄は適用結果に自動で乗る。`source_print.rs:42-49` は `shared ` を前置して印字し、`r9_grammar.rs:727` の `Shape::Type` は欄を持つ
- **`shared` + constructor 付き本体は E100、契約に無い型への `shared` は E204。** 前者は `parse_type_body`（`parser.rs:1141-1168`）で本体が `{` / `|` / `Ctor{` に入る枝の手前で決める（構文段階）。後者は `type_realizations.rs:50-56` の既存の E204 の述語（契約の抽象型に無い名前）をそのまま使う。`shared` の有無で E204 の条件は変わらない（`type U = Int` も `shared type U = Int` も E204）
- **意味は「解けた型への置き換え」で作る。不透明の機構（`types.rs:515-519` の名前比較）は触らない。** 検査器に `shared_realizations: HashMap<String, TypeRealization>`（修飾名 → 本体）を持たせ、`Ty::reveal`（`types.rs:719-726`）でそこだけを展開する `reveal_shared` を通す。通す場所は 4 つ。(1) 抽象型の参照の解決（`infer.rs:1633-1635` が `Ty::Abstract(qualified, args)` を返す所）、(2) 実装の公開面（`signature_type` `infer.rs:618-643` の結果。`module_abstract_parameters` `infer.rs:432-446` の仮置き値 `Abstract(qualified, vec![])` は型引数が無いので展開せず、型引数が付いた後の型だけを展開する。`TypeRealization::apply` の個数検査 `types.rs:697` を踏まないため）、(3) functor 適用の写し（`functors.rs:134-137` の後、改名した公開面と表現）、(4) `checked.main`（`infer.rs:260`。E705 と `--print-type` の前）。これで `shared` の抽象型は型の中に残らず、単一化・E601・E705・印字の全てが「解けた型」で動く。**`unify` に unfold を足さない**（残っていたら (1)〜(4) の漏れであり、そこを直す）。`records` / `named_constructors` / 効果の操作の型 / handler の腕の型など 5 箇所目が見つかったら、中断せず報告して通す（中断の条件は単一化への unfold だけ）
- **functor の本文の `shared type T = M.T`。** 本文検査では `M.T` は `Make$M.T`（`functors.rs:69-74`）で不透明のまま。`Template` に `shared` の集合（`representations` と同じ流儀で `owned`）を持たせ、適用（`functors.rs:99-121`）で改名した後、`shared_realizations["A.T"] = 改名した本体` を登録してから公開面を `reveal_shared` する。引数の `T` が `shared type T = String` なら `A.T` は `String`、引数の `T` が印なしなら `A.T` は `Concrete.T`（同じ不透明型。`shared_realizations["A.T"] = Abstract("Concrete.T")` の登録が §56 の「引数の実装の `T` と同一」の実装である）。印なしの `type T = M.T` は今までどおり適用ごとに別の抽象型（`r21_functors.rs:109` の固定）
- **契約修飾型 `X.T`（§34）。** `contract_types.rs:25-26` は `X.T` を唯一の実装の `Y.T` に写す。`signature_type` の結果を `reveal_shared` に通せば、`Y.T` が `shared` なら解けた型になる。E603（0 個か 2 個以上）は変えない
- **E705（B4）と E601。** `contains_abstract`（`infer.rs:1749-1770`）と `project_type`（`infer.rs:1103-1106`）は `Ty::Abstract` の形で判定している。`shared` の抽象型は (1)〜(4) で消えているので、判定は変えない。`shared` で record に解けた型は field 参照が通る（§35）
- **印字。** `Display for Ty`（`types.rs:609-631`）は変えない。E305 の `cannot unify {a} with {b}` も、型が解けているので `String` と出る。`--print-type` は `checked.main` が解けた型なのでそのまま
- **循環。** `check_realization_cycles`（`infer.rs:1732-1747`）の E602 は `shared` の有無によらず当てる（`shared type T = T`、`shared` の相互参照）
- **codegen は変えない。** `Checked::for_codegen`（`lib.rs:82-100`）は全ての表現を展開してから mono に渡す。`shared` は型検査の性質であり、機械表現は既に解けた型で決まっている
- **等式の収集は型と効果の登録より前に置く（工程 1 の審査 3 巡目で確定した訂正）。** 当初の決定は「`reveal_shared` を通す場所は 4 つ、5 箇所目が見つかったら中断せず通す」だったが、これは閉じなかった。原因は消費点の網羅ではなく順序である。`shared_realizations` が埋まるのは `infer.rs:579`（`checker.module()` の中、`inference_groups` のループ内）と `functors.rs:155` だが、`register_type`（`infer.rs:163` 付近）と `register_effects`（`:176`）はそのループより前に走る。よって `type Box = { value: Store.T }` の record schema と `capture send(value: Store.T)` の操作型は、`annotation_inner`（`:1672`）の展開を通っても等式が空のまま `Ty::Abstract` で登録される。実装より前のグループで推論される未注釈の定義（`def read box => box.value`）も、`resolve_projections(false)`（`:632` `:714`）の時点で等式が空である。**AST から `shared type T = body` を集める前処理を `register_type` の前に置き、消費点への展開は置かない。** 連鎖は固定点まで反復し、輪は E602（`functors.rs:154` の `introduces_cycle`）。`reveal_shared` の呼び出しとして残すのは入口（`:226` `:660` `:664` `:1672`）と E705 / E702 の検査（`:1805` `:1827`）だけで、消費点の展開を外しても判定器が緑であることを工程 1 の refactor コミットで測る

- **E2 のソースは R33 で `Tally` を契約の抽象型にする。** `tools/gates/report/report.hd` の契約 `Report` に `type T` を足し、実装で `shared type T = Tally` とする。`main.hd` は `Report.T` の値を作って `render` する。`shared` が無ければ `main` が E601 / E705 で書けない形にし、`shared` の効きを E2 で測る

### 期待値の根拠

| 形 | 導出 | 根拠 |
|---|---|---|
| `implement Monoid as StringMonoid` の `shared type T = String`、`StringMonoid.combine "a" "b"` | 受理して `"ab"` | §35、24.7 の探針 2 |
| 同じ形で `shared` 無し | E305（`StringMonoid.T` と `String`） | §35、24.7 の探針 2（現状） |
| functor `MakeFold(M: Monoid)` の `shared type T = M.T` を `StringMonoid` に適用し `StringFold.fold ["a", "b"]` | 受理して `"ab"` | §56、24.7 の探針 1 |
| 同上で `StringFold.fold` に `StringMonoid.of "a"` を渡す | 受理 | §56、24.7 の探針 3 |
| 印なし `type T = M.T` を 2 回適用し、一方の値を他方に渡す | E305 | §56、`r21_functors.rs:109` |
| `shared type T = M.T` で引数の `T` が印なし | `A.T` は `Concrete.T` と同じ型（不透明のまま）。`Concrete.T` の値を `A` の member に渡せる | §56 |
| `def main = Store.find 1` で `shared type T = Int` | 受理して整数を印字 | §5（B4） |
| 同上で `shared` 無し | E705 | §5 |
| `def name (t: UserStore.T) => t.name` で `shared type T = User`（record） | 受理 | §35 |
| 同上で `shared` 無し | E601 | §35 |
| `shared type T = \| A \| B`、`shared type T = { x: Int }` | E100。位置は本体の先頭のトークン（`\|` / `{`）。`shared type T =` までは §41 に合う | 24.4、§50 |
| `shared type U = Int`（契約に `U` が無い） | E204 | §35 |
| `shared` を外・契約・効果・handler に書く | E100 | §41 |
| `shared type T = T`、`shared type T = U` と `shared type U = T` | E602 | §35 |
| `shared type A T = A List` の `Stack.push` に `[1]` を渡す | 受理 | §35 |
| `--print-type` で `main` が `StringMonoid.T` の値 | `String` と印字 | 24.4 |
| `X.T`（契約修飾）で唯一の実装が `shared type T = Int` | `Int` として単一化 | §34 |

**本体では 1 つも通っていないので、期待値は仮説である。実測と食い違ったら、実装を疑う前に期待値を疑うこと。**

### 成果物

`crates/hado/src/`（`parser.rs` `source_print.rs` `type_realizations.rs` `infer.rs` `infer/functors.rs` `functor_instances.rs` `lib.rs`）、`crates/hado/tests/r33_shared.rs`（新設）、`crates/hado/tests/r9_grammar.rs`、`scripts/verify-r33.sh`（新設）、`tests/accept/`（`// item: 型の共有` の受理。通常の実装・functor・E705 の緩和の 3 本以上）、`tests/corpus.sha256`、`tools/gates/`。

### 工程

- 工程 0: 構文。`TypeDef.shared`、`parse_items` の `Implementation` 文脈の `shared`、E100（他の文脈、constructor 付き本体）、E204（契約に無い型）、`source_print` の往復、`r9_grammar` の `shared`（生成器は parse と往復だけで型検査を回さない（`r9_grammar.rs:770-782`）ので、意味の無い工程 0 で足してよい）。判定 33-c 33-j。この工程では `shared` は意味を持たない（受理するだけ。`StringMonoid.combine "a" "b"` は E305 のまま）
- 工程 1: 通常の実装の `shared`。`shared_realizations` と `reveal_shared`、(1)(2)(4) の通し、E705 と E601 の緩和、印字。判定 33-a(2) 33-b(2) 33-d 33-e 33-f 33-h 33-i
- 工程 2: functor と契約修飾型。`Template` の `shared`、適用の写し (3)、`X.T` の解決。判定 33-a(1)(3) 33-b(1) 33-g
- 工程 3: コーパスと道具。受理 3 本以上と lock、`tools/gates/` の `Report.T`。判定 33-k 33-l 33-n

### 判定

判定 33-a: 24.7 の探針 3 本が受理され、実行値が一致する。(1) `MakeFold(M: Monoid)` の `shared type T = M.T` を `StringMonoid`（`shared type T = String`）に適用し `StringFold.fold ["a", "b"]` → `"ab"`。(2) functor 無しで `StringMonoid.combine "a" "b"` → `"ab"`。(3) (1) の `fold` に `[StringMonoid.of "a", StringMonoid.of "b"]` を渡す → `"ab"`。**印字する文字列は実行時に生成した値を `of` に渡して作り、期待値を判定器側で計算する**
判定 33-b: 印なしは不透明のまま。(1) `type T = M.T` を 2 回適用し `A.read (B.make 7)` → E305（`r21_functors.rs:109` と同じ）。(2) `type T = String`（印なし）で `StringMonoid.combine "a" "b"` → E305
判定 33-c: 不正形。(1) `shared type T = | A | B` → E100 で最初の `|` の位置（§50: 文法に合わない最初のトークン）。(2) `shared type T = { x: Int }` → E100 で `{` の位置。(3) `shared type U = Int`（契約に無い） → E204 で `U`（探針は 1 欠陥にする。契約の `T` も実体化し、実装は参照されている形で、E204 より先に E513 / E602 が出ないこと）。(4) 外の `shared type T = Int` → E100。(5) 契約の中の `shared type T` → E100。(6) `shared def f = 1` → E100
判定 33-d: E705 の緩和。`def main = Store.find 1` で `shared type T = Int` → 実行して整数。`shared` 無し → E705
判定 33-e: E601 の緩和。`def name (t: UserStore.T) => t.name` で `shared type T = User`（record） → 受理して実行。`shared` 無し → E601
判定 33-f: 印字。`check --print-type` が `String` を出す（`StringMonoid.T` を含まない）。E305 のメッセージに `.T` が現れない（`shared type T = String` の値に `Int` を渡す形で `cannot unify String with Int` の形）
判定 33-g: 契約修飾型。契約 `Store` の member の型に `Id.T`（契約 `Id`）を書き、唯一の実装 `IntId` が `shared type T = Int` → `Store.find` に `Int` を直接渡せる。実装が 2 つなら E603 のまま
判定 33-h: 型引数。`shared type A T = A List` の `Stack.push` に `[1]` と `Int Stack.T` の注釈の両方が通る
判定 33-i: 循環。`shared type T = T` → E602。`shared type T = U` と `shared type U = T` → E602
判定 33-j: 構文の往復と生成。`r9_grammar` が `shared type T = ε` を `implement_item` と functor 本文で生成し、往復（`source_print`）で `shared` が落ちない。`keywords_cannot_be_names` は緑のまま
判定 33-k: E2。`tools/gates/report/report.hd` の契約 `Report` が `type T` を持ち、実装が `shared type T = Tally`。`main.hd` が `Report.T` の値を `render` する。判定器が一時ディレクトリに写して実行時生成の 3 つ組で `build` → 実行し、R32 の 32-j と同じ書式で一致。負の対照: `shared` を消した写しは `check` が E705 か E601 で落ちる
判定 33-l: 受理コーパスに `// item: 型の共有`（`// spec: §35` または §56）を 3 本以上。lock 更新
判定 33-m: `cargo test --offline --no-fail-fast` が 0 failed。全門番を単独で走らせ、許容される赤以外が満点
判定 33-n: `verify-r33.sh` が 33-a〜33-k を判定し満点

### 許容される赤

R33 の終了時に残ってよい赤は、R32 の終了時の許容される赤から「型の共有」の pairs が消えたもの:

- `verify-m1-corpus.sh` の 3 pairs（Bytes・配列・String 操作・Int.parse・Env と Io）と、その連鎖の m2 / m3
- `verify-m7.sh` の 3

### スコープ外

- Bytes / Array / String 操作 / Int.parse / Env / Io（R34〜R37）
- 実装の数え方（E603）と再適用の禁止（E604）の変更（B5）
- `shared` の冗長の規則（24.4: 当てない）
- first-class module、型の別名

### 確認して報告（24.9 と工程で出た疑義から）

- `Ty::reveal` が型引数付きの `shared`（`shared type A T = A List`）で `TypeRealization::apply` を通ること
- functor の本文で `shared type T = M.T` と書いたとき、本文検査中の `M.T` が不透明のままであること（`Arg.make x + 1` が E305。`r21_functors.rs:226`）
- 効果が `dependencies.rs` の宣言に数えられていない件（R32 の工程 1 で判明。§50 は E513 を効果にも当てる）: R33 では触らないが、探針 1 つで現状を報告する

### 中断の条件

**`types.rs:515-519` の単一化に `shared` の展開を足す必要があると分かったら、直さずに中断せよ。** 想定しているのは、`reveal_shared` を 4 箇所に通す変更で、単一化は名前比較のままである。

**functor の `Template` に `shared` の集合を持たせても、適用の改名（`functors.rs:116-117`）で引数の実装名に届かないと分かったら、工程 2 を止めて報告せよ。** 工程 1 の成果はそのまま残す。

それで届かないと分かったら、どこまで確認したか・何が足りないかを具体的に書いて止める。止めることは正常な終了経路である。

## 27. R34 Bytes と String の操作の手順書（2026-09-12）

24.1 の C2（Bytes）・C4（String の 5 操作）・C7（印字）・C8（関数集合）と C1（失敗の分類）を実装する。R35（配列）と R36（`Int.parse`）は別ラウンド。

**読む順:** 本節 → 24.1 の C 表と 24.4 → `docs/spec.md` の §36・§49（関数集合、member の型、失敗の `message` の表）・§51・§45・§43 → `docs/corpus.md` の免除表（Bytes と String 操作は拒否コーパスを持たない）。

**着手時点で赤の門番:** R33 の終了時の許容される赤（m1-corpus の pairs のうち Bytes・配列・String 操作・Int.parse・Env と Io、m7 の stdlib、m2 / m3 の連鎖）。R34 の入口で単独で測り直す。

### 決定

- **工程は 3 つ。Bytes の型と runtime（工程 0）、String の 5 操作（工程 1）、門番とコーパスと E2（工程 2）。** 工程ごとに指揮者が緑を実測してから次を発注する
- **Bytes の機械表現は String と同じ `HadoString`（`runtime/hado_runtime.h:6-9`: `size_t length; unsigned char bytes[]`）。** String は UTF-8 を仮定した同じ構造で、`length` はバイト数である（`hado_string_length` `hado_runtime.c:45-54` がスカラ値を数える）。`Bytes.of_string` はポインタをそのまま返し（Pure、O(1)）、`Bytes.to_string` は UTF-8 の検証だけを行う。String と Bytes は型が違うので、同じポインタを別の型として返す変換の intrinsic を 2 本置く: `__bytes_of_string(s: String): Bytes`（恒等）と `__bytes_to_string_unchecked(b: Bytes): String`（恒等。prelude の `to_string` が `__bytes_is_utf8` で検証した後にだけ呼ぶ）。`mono::Type::Bytes`（`mono.rs:4-14`）は `repr.rs:12-45` で `Pointer`、`Ty::Bytes`（`types.rs:3-16`）、`names.rs:4` の `BUILTIN_TYPES` に `"Bytes"`、`infer.rs:1672` の注釈の解決（`annotation_inner`）に `"Bytes" => Ty::Bytes`、`Display` に `Bytes`。**組み込み型の match は 2 本ある。** `grep -rn '"String" => ' crates/hado/src/` の全命中（`infer.rs:1672` の `annotation_inner` と `infer/functors.rs:271` の `shared_body_type`）に足すこと。後者を落とすと `shared type T = Bytes` の等式が前処理で収集されず、R33 で直した順序の欠陥が `Bytes` に限って再発する（24.9 の教訓 2 の実例）。両方が呼ぶ 1 本の helper に寄せるのが望ましい
- **予約名。** `BUILTIN_TYPES` に `Bytes` を足すと、R32 の名前解決（利用側の根の宣言も prelude / 組み込みの名前と比べる）で `type Bytes = ...` が E205 になる見込み。判定 34-f で確かめ、ならなければ同じ場所に足す
- **組み込みの失敗は prelude の包み（hado 側）で `raise` する（24.4）。** `stdlib/bytes.hd` と `stdlib/string.hd` の member は、前提条件（索引の範囲、空セパレータ、0..255、UTF-8 の妥当性）を hado で検査して `raise { message = "IndexOutOfRange" }` の形で `Error` を起こし、成立するときだけ intrinsic を呼ぶ。`Option.ok_or`（`stdlib/option.hd`）の `raise error` が手本。DivisionByZero（`emit.rs:662-707`）のような Cranelift 直書きの経路は**使わない**（handler で捕まえられないため）。`message` の綴りは §49 の表に固定: `"IndexOutOfRange"` `"EmptySeparator"` `"ByteOutOfRange"` `"InvalidUtf8"`
- **intrinsic は最小にし、C は hado の ADT（List の cell）を触らない。** 足す intrinsic（`runtime_abi.rs` の 5 箇所 + `hado_runtime.c` + `hado_runtime.h`）: `__bytes_length`（バイト数）、`__bytes_get`（範囲内前提、Int を返す）、`__bytes_is_utf8`（Bool）、`__bytes_display`（`Bytes.of_list [72, 105]` の形の String を C で組む。印字用）、`__string_slice`（スカラ値の索引 `start`..`end`、範囲内前提）、`__string_get`（スカラ値の索引、Char）、`__string_find`（`sep` が `from` 以降に最初に現れるスカラ値の索引、無ければ -1）。`Bytes.of_list` / `to_list` と `String.split` は hado 側で書く（`to_list` は `__bytes_get` の再帰、`of_list` は下記、`split` は `__string_find` と `slice` の再帰）。`starts_with` と `contains` は §49 で `Pure` なので、`{Error}` の `slice` は呼ばず、Pure な intrinsic `__string_find` / `__string_slice` を直接呼んで書く（`Pure` の member の中で `{Error}` の member は呼べない）。`prefix` / `part` が空ならどちらも `true`（python と同じ。`__string_find` は空の `sep` で呼ばない）
- **`Bytes.of_list` の構築。** hado は不変なので、素直に書くと 1 要素ごとに複製して O(n²) になる。許す形は 2 つ: (a) `__bytes_of_list_step`（既存の Bytes に 1 バイト足した新しい Bytes を返す）で O(n²) を受け入れる、(b) `__bytes_alloc n` と `__bytes_fill_unchecked b i v`（prelude の中でだけ使う、構築中の未公開の値への書き込み）で O(n)。builder が選び、判定 34-c(3) の 3 万要素が 2 秒以内に通ることを実測して報告する。(b) を選ぶなら、構築中の値が `of_list` の外に出ないことをコメントに書く。どちらでも、0..255 の範囲検査はリストを先に 1 周して済ませ、通ったら構築する（構築の途中で `raise` しない。O(n) を 2 回で単純にする）
- **印字（C7、§51）。** `derive_scalar_printer`（`mono.rs:3036-3062`）と `display_value`（`mono.rs:3136-3201`）に `Type::Bytes => __bytes_display` を足す。最上位でも入れ子でも `Bytes.of_list [72, 105]`、空は `Bytes.of_list []`
- **比較（§45）。** `emit.rs:447-455` の `string` フラグと `mono.rs:2560` の `matches!` に `Type::Bytes` を足し、`hado_string_compare`（memcmp）を共有する。`Bytes` の `==` `!=` `compare` は String と同じ規則
- **`String.length` の意味は変えない**（スカラ値の個数）。`String.get` と `slice` の索引もスカラ値の単位（§49）。`split` の結果は `String List` で、`separator` が空なら `EmptySeparator`、見つからなければ元の文字列 1 つのリスト、先頭・末尾・連続の一致は空文字列を要素として残す（`"a,,b"` を `","` で割ると `["a", "", "b"]`、`",a"` は `["", "a"]`）
- **stdlib の登録。** `lib.rs:145-151` の hard-coded なリストに `stdlib/bytes.hd` を足す（型宣言が無いので `:135-140` には足さない）。`trusted_start` は自動で続く
- **門番の追随。** `verify-m1-corpus.sh:317-322` の `// needs: prelude` の member 一覧（15 個の直書き）を、`verify-m7.sh:22-32` と同じく §49 の「関数集合は次とする」のブロックから導く形にする。`verify-m7.sh` は既に §49 から導いているので触らない（20/39 → 31/39 になる見込み。Array と `Int.parse` は R35 / R36）
- **E2 の集計部。** `tools/gates/report/report.hd` に `def fields(line: String): String List {Error}`（`"<名前> PASSED: <n>/<N>"` の 1 行を `[名前, n, N]` の 3 つの文字列に割る。`split` と `slice` は §49 で `{Error}` なので `fields` も `{Error}`。形が合わなければ `raise { message = "InvalidLine" }`）を足す。`Int.parse` が無いので整数への変換は R36。`main.hd` の出力は変えない。純粋な集計の受理を `tests/accept/`（`// item: String 操作`）に置く

### 期待値の根拠

| 形 | 導出 | 根拠 |
|---|---|---|
| `String.length "あ😀"` | `2`、`Bytes.length (Bytes.of_string "あ😀")` は `7` | §36、§49 |
| `String.slice "héllo" 1 3` | `"él"`（スカラ値の単位） | §49 |
| `String.slice s 0 (String.length s)` | `s` | §49 |
| `String.slice s 2 1`、`String.slice s 0 (length + 1)`、`String.get s length`、`String.get s (-1)` | `Error`、`message` は `"IndexOutOfRange"` | §49 |
| `String.split "a,,b" ","` | `["a", "", "b"]` | §49（本節の決定） |
| `String.split "abc" ""` | `Error` `"EmptySeparator"` | §21、§49 |
| `String.starts_with "héllo" "hé"`、`String.contains "héllo" "ll"` | `true`、`true` | §49 |
| `String.starts_with s ""`、`String.contains s ""` | `true`、`true` | 本節の決定 |
| `Bytes.of_list [72, 105]` を印字 | `Bytes.of_list [72, 105]`（最上位も入れ子も） | §51 |
| `Bytes.of_list [256]`、`[-1]` | `Error` `"ByteOutOfRange"` | §49 |
| `Bytes.to_string (Bytes.of_list [255])` | `Error` `"InvalidUtf8"` | §49 |
| `Bytes.to_string (Bytes.of_string s)` | `s` | §49 |
| `Bytes.get b (Bytes.length b)` | `Error` `"IndexOutOfRange"` | §49 |
| `Bytes.of_list [1, 2] == Bytes.of_list [1, 2]`、`compare (Bytes.of_list [1]) (Bytes.of_list [2])` | `true`、`-1` | §45 |
| `type Bytes = \| A` | E205、位置は 2 回目の宣言の名前 `Bytes` | 24.4、§50 |
| `def main = Bytes.get (Bytes.of_string "x") 0` | `120` | §49 |

**本体では 1 つも通っていないので、期待値は仮説である。実測と食い違ったら、実装を疑う前に期待値を疑うこと。**

### 成果物

`runtime/hado_runtime.c` `runtime/hado_runtime.h`、`crates/hado/src/`（`runtime_abi.rs` `types.rs` `mono.rs` `repr.rs` `emit.rs` `names.rs` `infer.rs` `lib.rs`）、`stdlib/bytes.hd`（新設）、`stdlib/string.hd`、`crates/hado/tests/r34_bytes.rs`（新設）、`scripts/verify-r34.sh`（新設）、`scripts/verify-m1-corpus.sh`（検査 6 の member 一覧）、`tests/accept/`（`// item: Bytes` と `// item: String 操作` の受理。`// expect-error:` の失敗形を含む）、`tests/corpus.sha256`、`tools/gates/`。

### 工程

- 工程 0: Bytes。型の配管（`Ty` `Type` `repr` `BUILTIN_TYPES` 注釈 `Display`）、intrinsic 4 本（`__bytes_length` `__bytes_get` `__bytes_is_utf8` `__bytes_display`）と変換 2 本と構築の intrinsic、`stdlib/bytes.hd` の 6 member と失敗、印字、比較、E205、**lock に `stdlib/bytes.hd` の 1 行を足す**（工程ごとに緑を保つため。他の行は変えない）。判定 34-c 34-d 34-e 34-f
- 工程 1: String の 5 操作。intrinsic 3 本（`__string_slice` `__string_get` `__string_find`）、`stdlib/string.hd` の 5 member と失敗、**lock の `stdlib/string.hd` の 1 行を更新する**。判定 34-a 34-b
- 工程 2: 門番とコーパスと E2。`verify-m1-corpus.sh` の検査 6、受理コーパス（Bytes 3 本以上、String 操作 3 本以上、失敗形を各 1 本以上）、lock（コーパスの行を足す）、`tools/gates/` の `fields`。判定 34-g 34-h 34-i 34-j

### 判定

判定 34-a: String の 5 操作。**判定は個別の値ではなく分類から生成する。** 文字列の分類（空 / ASCII のみ / 多バイトのみ / 混在 / `0x00` を含む。**多バイトは 2・3・4 バイトの全長を含め、各長さの最大コードポイント（U+007F / U+07FF / U+FFFF / U+10FFFF）を決定的に入れる**（乱数だけだと、マスクを 1 ビット狭める欠陥が偶然回避される）。2 バイトを落とすと `String.get` の 2 バイト用マスクが未検査になる）× 索引の分類（`-1` / `0` / 中間 / `length - 1` / `length` / `length + 1`）を回し、`split` はセパレータの位置（先頭 / 末尾 / 連続 / 中間 / 無い / 全体一致）× 種類（1 スカラ値の ASCII / 1 スカラ値の多バイト / 2 スカラ値以上）、`starts_with` は接頭辞の長さ（0 / 1 / 全体 / 全体 + 1）、`contains` は部分列の位置で回す。期待値は判定器側の python3 の同名操作で導く（実装の写しを書かない）
判定 34-b: String の失敗。`slice` の `start > end`、`end > length`、`start < 0`、`get` の `length` と `-1`、`split` の空セパレータが、stderr に `message` を出して終了コード 1（`// expect-error:` と同じ観測）。負の対照: 境界（`slice s 0 0`、`slice s length length`、`get s (length - 1)`）は受理
**入力の形が列挙できる判定は、個別の値ではなく分類からの生成で書く。** 工程 0 では個別の探針で書いたため、審査が 8 巡で 15 件の穴を挙げた（UTF-8 の不正列、比較の接頭辞、範囲外の位置、0x00 の扱い）。分類の例は判定 34-a と 34-c(4b) を見よ。

判定 34-c: Bytes。(1) `of_string` → `length` がバイト数（`String.length` と異なる文字列で）。(2) `of_list` → `to_list` の往復、`get` の各要素、`to_string` → `of_string` の往復（実行時生成）。(3) 3 万要素の `of_list` → `length` と `get 29999` が 2 秒以内。**リストはリテラルで書かず hado 側の生成器で作る**（3 万要素のリテラルは `build` でコンパイラのスタックが溢れる。`List.length [3 万個]` でも再現するので Bytes 固有ではない。リテラルの深さの限界は v0.3 の範囲外）。(4) 失敗: `of_list [256]` `[-1]` が `ByteOutOfRange`、`to_string (of_list [255])` が `InvalidUtf8`、`get b (length b)` が `IndexOutOfRange`。**(5) 失敗を handler で捕まえる形は、body と abort の腕の型を揃える**（§58 により abort の腕の型は `with` 全体の結果型に一致する。`with H -> Bytes.get b 99` は body が `Int`、`abort raise e => e.message` が `String` なので E305 になる。`Bytes.to_string` の `InvalidUtf8` のように String を返す member で測る）
判定 34-d: 印字。最上位の `Bytes.of_list [<生成した Int 列>]`、入れ子（`[Bytes.of_list [1]]`、record の field）、空 `Bytes.of_list []`
判定 34-e: 比較。`==` `!=` `compare` が String と同じ規則（生成した 2 列で判定器側で計算）
判定 34-f: `type Bytes = | A` → E205 で 2 回目の宣言の名前（`Bytes`。§50 の E205 の位置）。
判定 34-f2: `shared type T = Bytes` の等式が型の登録より前に収集される。`type Box = { value: Store.T }` の field を、`Store` に構文的に依存しない未注釈の関数が読み、`Bytes.length` で整数を印字して一致。`shared` を消した対照は E305（R33 の判定は String しか使っていないので、これが無いと `verify-r33.sh` 満点では捕まらない）`def f (b: Bytes) => b` の注釈は受理
判定 34-g: `bash scripts/verify-m7.sh` の inventory が String と Bytes の全 member を OK にする（残る NG は Array と `Int.parse` だけ）
判定 34-h: `verify-m1-corpus.sh` の検査 6 の member 一覧が §49 のブロックから導かれる（§49 に 1 行足した写しで一覧が増えることを判定器の側で確かめる。写しは一時ディレクトリで、本物の `docs/` は触らない）
判定 34-i: E2。`Report.fields "<名前> PASSED: <n>/<N>"`（実行時生成）が `[名前, n, N]`。`tools/gates/main.hd` の出力は R33 と同じ
判定 34-j: 受理コーパス（`// item: Bytes` 3 本以上、`// item: String 操作` 3 本以上、`// expect-error:` の失敗形を各 1 本以上）、lock 更新、`verify-m1-corpus.sh` の pairs から Bytes と String 操作が消える
判定 34-k: `cargo test --offline --no-fail-fast` が 0 failed
判定 34-l: 全門番を単独で走らせ、許容される赤（下記）以外が満点

### 許容される赤

- `verify-m1-corpus.sh` の 3 pairs（配列・Int.parse・Env と Io）と、その連鎖の m2 / m3
- `verify-m7.sh` の inventory の Array と `Int.parse`

### スコープ外

- 配列（R35）、`Int.parse`（R36）、Env / Io（R37）
- `U8` 型、文字列補間、多行文字列（24.3）
- `String.length` の O(1) 化

### 確認して報告（24.9 と本節の決定から）

- `BUILTIN_TYPES` に足すだけで `type Bytes` が E205 になるか
- `Bytes.of_list` の構築に選んだ形（(a) か (b)）と 3 万要素の所要
- `split` の空セパレータ以外の縁（セパレータが文字列より長い、文字列が空）の実測

### 中断の条件

**C の runtime が hado の List の cell（`repr.rs:117-162` の配置）を読み書きする必要があると判断したら、直さずに中断せよ。** 想定しているのは、C は `HadoString` と Int / Bool / Char だけを扱い、リストの構築と走査は hado 側で書く形である。

**`raise` を prelude の包みから起こす経路（`mono.rs:1419-1453` の `abort_raise` と handler がある場合の `abort_outcome`）が intrinsic の呼び出しの後ろで型を合わせられないと分かったら、工程 0 を止めて報告せよ。**

それで届かないと分かったら、どこまで確認したか・何が足りないかを具体的に書いて止める。止めることは正常な終了経路である。

## 28. R35 配列の手順書（2026-09-13）

24.1 の C3（配列）と C7（印字）・C8（関数集合）・C1（失敗の分類）を実装する。`Int.parse`（R36）と Env / Io（R37）は別ラウンド。

**読む順:** 本節 → 24.1 の C 表と 24.4 → `docs/spec.md` の §36（`get` は O(1)、`set` は新しい Array、1287 行の `T Array`）・§10（型の書き方）・§45（`==` `!=` `compare` は要素型から派生、Fn を含めば E703 / E704）・§49（関数集合、member の型、失敗の `message` の表）・§50・§51（`Array.of_list [1, 2]`）→ 27 節（R34 の手順書。Bytes の `__bytes_alloc` + `__bytes_fill_unchecked` が本節の手本）→ `docs/corpus.md` の免除表。

**着手時点で赤の門番:** R34 の終了時の許容される赤（`verify-m1-corpus.sh` 6/7 の 3 pairs は配列・`Int.parse`・Env と Io、その連鎖の `verify-m2.sh` 7/8 と `verify-m3.sh` 11/12、`verify-m7.sh` 2/3 で `prelude inventory: 32/39`）。R35 の入口で単独で測り直す。

### 決定

- **工程は 3 つ。型と機械表現と 4 つの intrinsic と `stdlib/array.hd`（工程 0）、印字と比較（工程 1）、門番とコーパスと E2（工程 2）。** 工程ごとに指揮者が緑を実測してから次を発注する
- **Array は hado の record や ADT では作らない。** 実測（2026-09-13）で 3 つの道が閉じた: (1) `type A Arr = A Box` の別名は parser が拒否する（`expected a record or a leading | for an ADT`）ので、`module Array` の `shared type A T` を `A Array` と綴らせる道は無い。(2) トップに `type A Array = { items: A List; count: Int }` と書くと綴りは §49 に合うが、`a.items` が利用者に漏れて不正な `count` を持つ値を組める。(3) List で索引を引く形は `get` が O(n) で §36 に反する。よって **Array は組み込み型とし、機械表現を持たせる**
- **型は名前キーの `Data`。** `Ty::Adt("Array", [A])` / `mono::Type::Data("Array", [T])` とし、`mono::OUTCOME`（`mono.rs:64` の `"$Outcome"`）の隣に `pub const ARRAY: &str = "Array"` を置く。**`grep -rn 'OUTCOME' crates/hado/src/` の全命中が、Array の腕を足すか足さないかを判断する場所の地図である**（`repr.rs` の `machine_type`、`mono.rs` の型の合成と分解）。新しい `Ty` / `Type` の variant は作らない（`TryFrom<&Ty>`・`Display`・`repr` の全経路を触らずに済む）
- **`Int Array` の注釈を通す道は既にある。** `infer.rs:1668` の `if self.type_arities.get(name) == Some(&args.len()) { return Ok(Ty::Adt(name.clone(), args)) }` がそれで、`List` は `stdlib/list.hd` の `type T List = ...` から `type_arities` に入る。Array は hado の宣言を持たないので、**infer の初期化で `type_arities` に `"Array" → 1` を直接登録する**。`:1669` の `if !args.is_empty() { return Err("type annotation is not implemented") }` は `types::builtin_type` が nullary しか返さないための門なので、その手前で拾わせること。**`Int Array` は現状 E204（`unknown type Array`）である**（実測 2026-09-13）
- **予約名。** `names.rs` の `BUILTIN_TYPES` に `"Array"` を足す（§1865）。**現状 `type A Array = { ... }` は E205 にならず E513（未使用）である**（実測 2026-09-13）。R34 の Bytes と同じく、足すだけで E205 になる見込み。判定 35-f で確かめ、ならなければ `namespaces.rs` の同じ場所に足す
- **`infer/functors.rs` の `shared_body_type` が `A Array` を受けること。** R34 の 27 節が警告した「組み込み型の match が 2 本ある」件がそのまま当てはまる。ここは `types::builtin_type`（nullary）とは別に、引数付きの型を解く経路が要る。落とすと `shared type T = Int Array` の等式が前処理で収集されず、R33 で直した順序の欠陥が Array に限って再発する。判定 35-f2 で固定する
- **機械表現は `{ int64 count; slot[count] }` の 1 つの GC ブロック。stride は 8 バイト固定、slot は offset 8 から始まる。** 要素の機械型（`repr::machine_type`）は I8 / I32 / I64 / F64 / Pointer のいずれかで、どれも 8 バイトに収まり 8 バイト整列を満たす。リトルエンディアンなので、8 バイト slot の先頭から要素の幅で load / store すればよい（`repr::aggregate_layout` は使わない。Array は専用の配置である）。`hado_alloc` は `GC_malloc`（`hado_runtime.c:31`）で、返るブロックはゼロ埋めされ、保守的走査が slot の中のポインタを追う。**要素を跨ぐ拡縮ノードは要らない**
- **intrinsic は 4 本。`value_signature`（`runtime_abi.rs:77`）には載せない。** `__array_length: A Array -> Int`、`__array_get: (A Array, Int) -> A`（範囲内前提）、`__array_alloc: Int -> A Array`（count 個の slot を確保。中身はゼロ）、`__array_fill_unchecked: (A Array, Int, A) -> A Array`（構築中の未公開の値への書き込み。prelude の中でだけ使う）。R34 の `__bytes_alloc` + `__bytes_fill_unchecked` と同じ形である。4 本とも型が多相なので `value_signature` の `mono::Signature` では書けない。`RuntimeFunction::is_polymorphic()` を足して、次の **4 箇所**を分岐させる:
  - **name_resolution（`name_resolution.rs:167`）**: `__` で始まる名前は `from_intrinsic(name).and_then(|f| f.value_signature()).is_none()` なら E204 になる。**ここを直さないと `stdlib/array.hd` の中の 4 本すべてが E204 になり、prelude が壊れて工程 0 の最初の `check` から一切通らない**（計画審査 1 巡目の致命 1）。条件を `is_polymorphic() || value_signature().is_some()` にする
  - **infer（`infer.rs:1392-1395`）**: 固定署名の代わりに fresh な型変数 `a` を 1 本引いて `Ty` を組む（`__array_get` なら `Ty::fn_of(vec![Ty::Adt("Array", vec![a.clone()]), Ty::Int], a)`）。引数と結果で同じ変数を使うことが本質である
  - **mono（`mono.rs:1387-1389`）**: `Expression::call_runtime`（`mono.rs:193-199`）は引数型を固定署名と厳密比較するので通せない。結果型を実引数から導く別の構築関数を足す（`__array_get` は引数 0 の `Data("Array",[T])` から `T`、`__array_length` は `Int`、`__array_fill_unchecked` は引数 0 の型）
  - **`__array_alloc` だけは結果しか多相でない。** 実引数からは要素型を導けない。**`composite_types` の span 地図から引く**（`infer.rs:945` が `Apply | Call | Pipe | List | Record | NamedConstructor | With` の span に結果型を記録し、`mono.rs:1394` の constructor が同じ地図を同じ鍵で引いている。intrinsic の呼び出しは `Call` / `Apply` なので記録される）。**引いた `Ty` は `self.specialization.resolve` に通すこと**（`mono.rs:409` `:422` `:1363` と同じ理由。多相な `of_list` の中では要素型が推論変数のままである）。phantom の引数（`__array_alloc: (A List, Int) -> A Array` にして要素型を第 1 引数から導く）は採らない。`stdlib/array.hd` に嘘の引数を書かせないためである
- **emit（`emit.rs:265` の `Expression::CallRuntime`）は 4 本を 2 通りに分ける。** `__array_length` と `__array_alloc` は C の関数呼び出し（`hado_array_length` は header の `count` を読む、`hado_array_alloc` は `hado_alloc(8 + 8 * count)` を呼んで `count` を書く。どちらも要素型を知らないので C で書ける）。**size の計算は C の中で閉じるので、`emit.rs:630-636` の `allocate`（size が `u32` の定数）は触らない。**`__array_get` と `__array_fill_unchecked` は **C を呼ばず emit がその場で load / store を出す**（`imul_imm(index, 8)` → `iadd` で base + 8 + index*8 を作り、`builder.ins().load(要素の clif 型, ...)` / `store`）。既存の `emit.rs:368` `:393` が定数 offset の load を出しているので、動的 offset を足すだけである
- **`of_list` `to_list` `set` は hado で書く（`stdlib/array.hd`）。** `of_list` は `__array_alloc (List.length xs)` してから再帰で `__array_fill_unchecked` する（R34 の `Bytes.of_list` の写し。構築中の値が外に出ないことをコメントに書く）。`to_list` は `__array_get` の再帰。`set` は範囲を検査してから `to_list` → 索引の位置だけ差し替え → `of_list` で新しい Array を作る（O(n)。§36 は `set` に O(1) を求めていない）。**C は hado の List の cell を触らない**（R34 と同じ中断の条件）
- **失敗は hado 側の包みで `raise`。** `Array.get` と `Array.set` の索引が `index < 0` または `index >= length` なら `raise { message = "IndexOutOfRange" }`（§49 の表）。member の型は §49 のとおり（`length` `of_list` `to_list` が `Pure`、`get` `set` が `{Error}`）
- **印字（§51）。** `display_value`（`mono.rs:3141`）の `Type::Data(name, parameters) if name == "List"` の腕の隣に Array の腕を置き、`display_list_items`（`mono.rs` の再帰 helper）を写して、`Cons` の projection の代わりに索引を 1 増やす再帰 helper を合成する（終端は `index == __array_length`）。書式は `Array.of_list [1, 2]`、空は `Array.of_list []`。`derive_scalar_printer`（`mono.rs:3039`）はスカラ専用なので触らない
- **比較（§45）。** `derive_comparison`（`mono.rs:2562`）と `structural_equality` に Array の腕を、`records` / `constructors` を見る前に置く。要素ごとに再帰し、共通部分がすべて等しいときだけ短い方を先に置く（python3 のリスト比較と同じ辞書順。長さを要素より先に見ない）。
- **要素型が Fn を含む Array の E703 / E704 は、`infer.rs` の `contains_function` を直さないと出ない（計画審査 1 巡目の致命 2）。** `contains_function` は `let Ty::Adt(name, _) = ty else { return false }` の後、その名前を結果に持つ constructor / record の payload だけを辿る。`List` は `Cons` の payload で拾えるが、**Array は constructor を持たないので `Adt("Array", [Fn])` は常に false になる**。同じファイルの `contains_abstract` は `Ty::Adt(_, args) if args.iter().any(...)` の腕で型引数を再帰しているので、`contains_function` にも同じ腕を足す。落とすと `Array.of_list [fn(x => x)] == ...` が §45 に反して受理され、`build` で `derive_comparison` / `structural_equality` が Fn に達して内部エラーになる。判定 35-e2 で固定する
- **stdlib の登録。** `lib.rs:145-151` のリストに `stdlib/array.hd` を足す（型宣言は組み込みなので `:135-140` には足さない）。`stdlib/array.hd` は `module Array` と `implement Array as Array` の形（`stdlib/string.hd` が手本）。**工程 0 だけ lock に `stdlib/array.hd` の 1 行を足すことを許す**（他の行は変えない）
- **門番の追随。** `verify-m7.sh` の inventory は §49 から導く形になっている（R34）ので触らない。Array の 5 member が入れば残る NG は `Int.parse` だけになる見込み
- **E2 の集計部。** `tools/gates/report/report.hd` は R35 では触らない（`Int.parse` が要るので R36）

### 期待値の根拠

| 形 | 導出 | 根拠 |
|---|---|---|
| `Array.length (Array.of_list [1, 2, 3])` | `3` | §49 |
| `Array.get (Array.of_list [10, 20]) 1` | `20` | §49 |
| `Array.to_list (Array.of_list xs)` | `xs` | §49 |
| `Array.set (Array.of_list [1, 2]) 0 9` を印字 | `Array.of_list [9, 2]`。元の Array は `Array.of_list [1, 2]` のまま | §36、§51 |
| `Array.get xs (-1)`、`Array.get xs (Array.length xs)`、`Array.set xs (Array.length xs) v` | `Error`、`message` は `"IndexOutOfRange"` | §49 |
| `Array.of_list []` を印字 | `Array.of_list []` | §51 |
| `[Array.of_list [1], Array.of_list []]` を印字 | `[Array.of_list [1], Array.of_list []]` | §51 |
| `Array.of_list [1, 2] == Array.of_list [1, 2]` | `true` | §45 |
| `compare (Array.of_list [1, 2]) (Array.of_list [1, 3])` | `-1` | §45 |
| `compare (Array.of_list [1]) (Array.of_list [1, 2])` | `-1`（共通部分が等しいので短い方が先） | §45 + 本節の決定（下記の疑義） |
| `compare (Array.of_list [2]) (Array.of_list [1, 2])` | `1`（長さより要素が先） | §45 + 本節の決定 |
| 要素型が Fn を含む Array の `==` / `compare` | E703 / E704 | §45 |
| `type A Array = ...` を利用者が書く | E205、位置は 2 回目の宣言の名前 `Array` | §50、§1865 |
| `def f (xs: Int Array) => xs` の注釈 | 受理 | §36（`docs/spec.md:1287` の `T Array`）、§10 |
| 3 万要素の `of_list` → `length` と `get 29999` | 一致し、`build` → 実行が 2 秒以内 | 実測（同じ長さの List の 3 万段の再帰は build 0.13 秒・実行 0.01 秒） |

**本体では 1 つも通っていないので、期待値は仮説である。実測と食い違ったら、実装を疑う前に期待値を疑うこと。**

### 仕様への疑義（R38 で判断する）

§45（`docs/spec.md:1834`）は Array の `compare` を「要素型から派生する」としか定めず、**長さが違う Array の順序を定めていない**。本節は python3 のリスト比較・`List` の辞書順に合わせて「共通部分がすべて等しいときだけ短い方を先」と決めた。仕様は変えない。R38 で §45 に 1 文足すかどうかを判断する。

### 成果物

`runtime/hado_runtime.c` `runtime/hado_runtime.h`、`crates/hado/src/`（`runtime_abi.rs` `mono.rs` `repr.rs` `emit.rs` `names.rs` `infer.rs` `infer/functors.rs` `lib.rs` `namespaces.rs` `name_resolution.rs`）、`stdlib/array.hd`（新設）、`crates/hado/tests/r35_array.rs`（新設）、`scripts/verify-r35.sh`（新設）、`tests/accept/`（`// item: 配列` の受理。`// expect-error:` の失敗形を含む）、`tests/corpus.sha256`、`docs/ledger.md`。

### 工程

- 工程 0: 型と機械表現と 4 つの intrinsic と `stdlib/array.hd`。`type_arities` の登録、`BUILTIN_TYPES`、`shared_body_type`、多相 intrinsic の 4 分岐（name_resolution を含む）、emit の load / store、C の 2 本、`stdlib/array.hd` の 5 member と失敗、**lock に `stdlib/array.hd` の 1 行を足す**。判定 35-a 35-b 35-c 35-f 35-f2
- 工程 1: 印字と比較。`display_value` の Array の腕と索引の再帰 helper、`derive_comparison` と `structural_equality` の Array の腕、E703 / E704。判定 35-d 35-e 35-e2
- 工程 2: 門番とコーパス。受理コーパス（`// item: 配列`）、lock、`verify-m7.sh` の inventory と `verify-m1-corpus.sh` の pairs。判定 35-g 35-h 35-i

### 判定

`scripts/verify-r35.sh` を新設する。構造は `scripts/verify-r34.sh` に倣う。**入力の形が列挙できる判定は、個別の値ではなく分類から生成する**（24.9 の R34 の教訓 1）。**境界値は乱数任せにせず決定的に含める**（教訓 2）。**期待値は判定器側の python3 のリスト操作から導く**（教訓 3。実装の写しを判定器に書かない）。成功時の stderr と失敗時の stdout が空であることを `wc -c` で測る。生成器自身の失敗と manifest の欠落を非ゼロ終了にする（R34 工程 0 の審査 1 の指摘）。

### 分類（すべての判定でこの積を回す）

- **要素型:** `Int` / `Float` / `Bool` / `String` / `Char` / record / `Int Array`（入れ子）。要素の機械型が I64 / F64 / I8 / Pointer / I32 の全部を通ること（8 バイト slot の load 幅の欠陥は、I8 と I32 を落とすと見逃す）
- **長さ:** `0` / `1` / 中間（乱数）/ `3` / `30000`
- **索引:** `-1` / `0` / 中間（乱数）/ `length - 1` / `length` / `length + 1`

判定 35-a: 5 member の正常形。要素型 × 長さ × 有効な索引の積で、`length`・`get`・`to_list (of_list xs)` の往復・`set` の結果を、python3 のリスト操作で計算した期待値と照合する。**工程 0 では Array の印字がまだ無いので、観測は `Array` の値そのものではなく `Array.length` / `Array.get` / `Array.to_list` の結果で行う**（`set` の結果も `to_list` を通して観測する。計画審査 1 巡目の重大 3）。**空の Array は要素型が決まらず `main` が E701 になる**（`infer.rs:230`、`types.rs:208`）ので、`def f (xs: Int List) => Array.length (Array.of_list xs)` のように注釈で要素型を固定した関数を `f []` で呼ぶ形にする。要素型に `Int Array`（入れ子）を含む観測と、長さ 30000 の観測は工程 0 では `length` と `get` に限り、印字を伴う形は工程 1 の判定 35-d に寄せる
判定 35-b: 失敗。**35-a と同じく、工程 0 では `main` の型を印字できる型に保つこと。** `emit.rs:528` は `main` の型から無条件に printer を合成するので、`main` が `T Array` のまま（`Array.set` の結果や入れ子の `Array.get` を素直に書いた形）だと、Array の腕が無い `display_value` が generic な `Data` の腕に落ちて `unknown display type` で build が落ちる。`Array.length (Array.set xs i v)` のように包んで `main` を `Int` にする。`get` と `set` の無効な索引（`-1` / `length` / `length + 1`）を要素型 × 長さの全部で回し、stderr の `message` が `IndexOutOfRange`、終了コード 1。**負の対照**: すぐ隣の有効な形（`get xs (length - 1)`、`set xs 0 v` を長さ 1 以上で、`of_list []` の `length`）は受理して実行。**handler で捕まえる形は body と abort の腕の型を揃える**（§58。R34 で E305 を踏んだ罠。`Array.get` は `A` を返すので、腕も同じ型にするか `Int` の要素で測る）
判定 35-c: 3 万要素。hado 側の生成器で `Int List` を作り（**リテラルで書かない**。3 万要素のリテラルは build でコンパイラのスタックが溢れる。R34 で実測）、`of_list` → `length` と `get 29999` と `get 0` を印字。`build` → 実行が 2 秒以内（`timeout 2` を build と実行の両方を含む 1 つの期限にする。R34 工程 0 の審査 1 の指摘）。**この判定で配列そのものを印字しない**（印字は索引の再帰 helper で 3 万段を使う。2 秒の期限を印字で汚さない）。`set` も 3 万要素で 1 回測り、SIGSEGV しないことを終了コードで確かめる
判定 35-d: 印字。最上位（`Array.of_list [<生成した列>]`）、空（`Array.of_list []`）、入れ子（List の要素、record の field、Array の Array）を要素型の分類すべてで。期待値は python3 で組む
判定 35-e: 比較。`==` `!=` `compare` を要素型 × (同内容の別インスタンス / 先頭が違う / 途中が違う / 末尾が違う / 一方が他方の接頭辞 / 空どうし / 空と非空) の積で回し、期待値は python3 のタプル比較から導く。**同内容の別インスタンスの `==` が `true`、`compare` が `0` になることを必ず含める**（R34 工程 0 の審査 1 が「同値の構造比較を確実に検査しない」を挙げた）。大小の両方向を測る
判定 35-e2: 要素型が Fn を含む Array の `==` と `compare` が E703 / E704。**入れ子（`Int Array Array` の中の Fn、record の field の Fn を要素に持つ Array）も含める**（`contains_function` の型引数の再帰が 1 段で止まる欠陥を捕まえるため）。負の対照として Fn を含まない同じ形（`Int Array Array`）は受理
判定 35-f: 利用者の `type A Array = ...` が E205 で、位置は 2 回目の宣言の名前（§50）。`def f (xs: Int Array) => xs` の注釈は受理
判定 35-f2: `shared type T = Int Array` の等式が型の登録より前に収集される。R34 の判定 34-f2 を写す（`type Box = { value: Store.T }` の field を、`Store` に構文的に依存しない未注釈の関数が読み、`Array.length` で整数を印字して一致。`shared` を消した対照は E305）
判定 35-g: `bash scripts/verify-m7.sh` の `prelude inventory` が 32/39 から 38/39 になる（Array の signature と 5 member が OK になり、残る NG は `Int.parse` だけ）
判定 35-h: 受理コーパス（`// item: 配列` 3 本以上、`// expect-error:` の失敗形を 1 本以上）、lock 更新、`verify-m1-corpus.sh` の pairs から「配列」が消える
判定 35-i: `cargo test --offline --no-fail-fast` が 0 failed
判定 35-j: 全門番を単独で走らせ、許容される赤（下記）以外が満点

Rust 側: `crates/hado/tests/r35_array.rs` を新設し、`type_arities` の登録・`Int Array` の注釈・E205・`repr::machine_type` の Array・`hado_array_alloc` と `hado_array_length` の C レベルの動作（`tests/runtime_string.rs` の流儀）をテストする。

### 許容される赤

- `verify-m1-corpus.sh` の 2 pairs（`Int.parse`・Env と Io）と、その連鎖の m2 / m3
- `verify-m7.sh` の inventory の `Int.parse`

### スコープ外

- `Int.parse`（R36）、Env と Io（R37）、仕上げと tag（R38）
- 配列のリテラル構文（24.1 の C3 で「リテラルなし」と決めた）
- `Array.set` の O(1) 化（永続配列・trie）。§36 は `get` にだけ O(1) を求める
- `tools/gates/` の集計部（`Int.parse` が要るので R36）

### 確認して報告（24.9 と本節の決定から）

- `type_arities` に `"Array" → 1` を登録するだけで `Int Array` の注釈が通ったか、`infer.rs:1669` の手前で拾えたか
- `BUILTIN_TYPES` に足すだけで `type A Array` が E205 になったか
- `__array_alloc` の要素型が `composite_types` の span 地図から引けたか（引けなければ何が足りなかったか）
- 3 万要素の `of_list` と `set` の所要（`time`）と、`to_list` の 3 万段の再帰が C スタックで通ったか
- `contains_function` に型引数の再帰を足して E703 / E704 が出るようになったか、`contains_abstract` の腕をそのまま写せたか、既存の `List` / record の E703 の判定が後退しないか

### 中断の条件

**C の runtime が hado の List の cell（`repr.rs:117-162` の配置）を読み書きする必要があると判断したら、直さずに中断せよ。** C が扱うのは Array のブロックの header（`count`）と確保だけで、slot の読み書きは emit が出す。

**`composite_types` の span 地図から `__array_alloc` の要素型が引けず、かつ `infer.rs:945` の記録の条件を広げる以外に手が無いと分かったら、工程 0 を止めて報告せよ。** phantom の引数に切り替えるかどうかは指揮者が決める。

**多相 intrinsic の 4 分岐（name_resolution の E204 の門、infer の fresh 変数、mono の結果型の導出、emit の load / store）のどれかが、既存の単相の intrinsic の経路を壊さずには通せないと分かったら、止めて報告せよ。** R34 で足した 11 本の Bytes / String の intrinsic が後退しないことが前提である。

それで届かないと分かったら、どこまで確認したか・何が足りないかを具体的に書いて止める。止めることは正常な終了経路である。

## 29. R36 Int.parse の手順書（2026-09-13）

24.1 の C5（`Int.parse`）と C1（失敗の分類）を実装し、E2（`tools/gates/`）の集計部に数への変換を足す。Env と Io（R37）と仕上げ（R38）は別ラウンド。

**読む順:** 本節 → 24.1 の C 表（C5）と 24.4 → `docs/spec.md` の §49（`module Int` の宣言 1952〜1954 行、`Pure` を書く member の一覧 1919 行、失敗の `message` の表 1985 行の `"InvalidInt"`）・§21 → 27 節（R34 の手順書。`__bytes_is_utf8` + `__bytes_to_string_unchecked` が本節の手本）→ 24.5 の R36 の行。

**着手時点で赤の門番:** R35 の終了時の許容される赤（`verify-m1-corpus.sh` 6/7 の 2 pairs は `Int.parse` と Env と Io、その連鎖の `verify-m2.sh` 7/8 と `verify-m3.sh` 11/12、`verify-m7.sh` 2/3 で `prelude inventory: 38/39`）。R36 の入口で単独で測り直す。

### 決定

- **工程は 2 つ。`Int.parse`（工程 0）、E2 の集計部とコーパス（工程 1）。** R35 より 1 つ少ない。`Int` は既存の型なので `Ty` / `Type` / `repr` / `emit` は無傷で、intrinsic は単相なので `value_signature` に載せる普通の経路を使う（R35 の `is_polymorphic()` の 4 分岐は要らない）。`display_value` も `derive_comparison` も触らない
- **受理する文法は `-?[0-9]+` で、値が i64 の範囲に収まること。それ以外はすべて `raise { message = "InvalidInt" }`。** この形は 1 つの導出と 2 つの決定からなる。**往復が導くのは受理の下限だけで、字種の上限と数字列の形は本節の決定である**（R38 で §49 に 1 文足すときは、どれが導出でどれが決定かを分けて書く）。
  - **導出: 受理の下限は往復が決める。** `Int.parse (Int.to_string n) == n` が全 i64 で成立しなければならない。`Int.to_string` が出す文字列は `-?[0-9]+` の形で i64 全域を覆う（実測 2026-09-13: `9223372036854775807` / `-9223372036854775808` / `0` / `-7`。`hado_int_to_string` は `snprintf("%lld")`）
  - **決定 1: 使う字は `Int.to_string` が出す字に限る。** `+`・空白・`_`・小数点・全角数字は `to_string` が出さないので、受理する根拠が無い（`-` と ASCII 数字だけ）
  - **決定 2: 数字列の形（先行ゼロの可否）は往復では決まらないので、ここで定める。先行ゼロは長さに上限なく受理して数値にする**（`"007"` → 7、`"-007"` → -7、`"00"` → 0、`"0" * 25 + "5"` → 5）。`[0-9]+` に含まれるためである。**受理を長さで打ち切らない**（打ち切ると oracle と食い違う）
- **intrinsic は 2 本。失敗は hado 側の包みで `raise`**（R34 の `Bytes.to_string` と同じ形）: `__int_parse_is_valid(s: String): Bool`（文法と i64 の範囲の両方を検査）と `__int_parse_unchecked(s: String): Int`（`is_valid` が真のときだけ呼ぶ）。`stdlib/int.hd` の `parse` は `if __int_parse_is_valid s => __int_parse_unchecked s | raise { message = "InvalidInt" }`。DivisionByZero（`emit.rs:662-707`）のような Cranelift 直書きの経路は使わない（handler で捕まえられないため）
- **C は `length` と `bytes` を直接走査する。`strtoll` は使わず、一時バッファも作らない。** `HadoString` は NUL 終端ではない（`runtime/hado_runtime.h:6-9` は `size_t length; unsigned char bytes[]`）ので、`bytes` を `strtoll` や `atoll` に渡すと領域を越える。NUL 付きの一時バッファに写す形も採らない。**先行ゼロを無制限に受理するので、固定長のバッファに写すには先頭ゼロを外す前処理が要る。前処理を書くくらいなら直接走査する方が単純だからである。** 文法の検査はバイト走査で行い（先頭の `-` は 0 個か 1 個、残りは 1 個以上の ASCII 数字。埋め込み NUL も数字でないので同じ走査で落ちる）、範囲の検査は桁ごとの積み上げで行う
- **桁溢れの検査は桁ごとに負の側へ累積し、最後に符号を戻す。** 正の側に積むと `-9223372036854775808` が受理できない。**先行ゼロは有効桁に数えない**（読み飛ばしてから積む）ので、`"0" * 25 + "5"` は 5 として受理される。累積の各段で `acc < (INT64_MIN + digit) / 10` を先に検査して溢れる前に止める（C の整数除算は 0 方向に切り捨てるので、負の値では `ceil` になり、この比較がちょうど境界を表す）。**符号を戻すときにもう 1 段の検査が要る。** 負の側の累積は `-9223372036854775808` まで表せるが、入力が正で累積が `INT64_MIN` に達した場合、`-acc` は i64 に収まらない。`"9223372036854775808"` はこの経路で拒否する（`acc == INT64_MIN && 符号が正` を `InvalidInt` にする）。**符号を戻す前にこの検査を置かないと、`-acc` が未定義動作になるか `INT64_MIN` のまま返って `"9223372036854775808"` が受理される。** `"-9223372036854775808"` を受理し `"9223372036854775808"` を拒否することを判定で固定する
- **E2 の集計部。** `tools/gates/report/report.hd` に `def parse_line(line: String): T {Error}` を 1 本足す（`fields` が返す `[名前, n, N]` の String を `Int.parse` で数に変えて `add` に渡す）。`fields` の `"InvalidLine"` と `Int.parse` の `"InvalidInt"` は別の失敗で、どちらが出るかは入力の形で決まる。`tools/gates/main.hd` は R37 まで固定リストのままにする（argv とファイルの読みは R37）。**`tools/gates/` は利用者のコードなので §49 の関数集合の対象外**であり、`Report` に member を足してよい
- **`Int.to_string` は変えない**（`Pure`、§49）

### 期待値の根拠

| 形 | 導出 | 根拠 |
|---|---|---|
| `Int.parse "42"` | `42` | §49 |
| `Int.parse (Int.to_string n)` を全 i64 の代表で | `n` | 本節の決定（往復） |
| `Int.parse "0"`、`"-0"`、`"007"`、`"-007"`、`"00"` | `0`、`0`、`7`、`-7`、`0` | 本節の決定（先行ゼロは受理） |
| `Int.parse ("0" を 25 個並べた後に "5")` | `5` | 本節の決定（先行ゼロを長さで打ち切らない） |
| `Int.parse ("0" を 25 個並べた後に "9223372036854775808")` | `Error` `"InvalidInt"` | 本節の決定（有効桁で範囲を見る） |
| `Int.parse "9223372036854775807"`、`"-9223372036854775808"` | `9223372036854775807`、`-9223372036854775808` | 本節の決定（i64 の範囲） |
| `Int.parse "9223372036854775808"`、`"-9223372036854775809"` | `Error` `"InvalidInt"` | §49（桁溢れ） |
| `Int.parse ""`、`"-"`、`"+42"`、`" 42"`、`"42 "`、`"1_000"`、`"4.2"`、`"４２"`、`"--5"`、`"4a2"`、`"a42"`、`"42a"` | `Error` `"InvalidInt"` | 本節の決定 |
| `Int.parse` を handler で捕まえる | `message` が `"InvalidInt"` | §58 |
| `Report.parse_line "m0 PASSED: 7/7"` を `render` | `7/7 passed; red: none` | 24.1 の E2、32 節の判定 32-j の書式 |
| `Report.parse_line "m0 PASSED: +7/7"` | `Error` `"InvalidInt"`（`"InvalidLine"` ではない） | 本節の決定 |
| `Report.parse_line "m0 7/7"`（区切りが無い） | `Error` `"InvalidLine"` | 27 節の決定 |

**本体では 1 つも通っていないので、期待値は仮説である。実測と食い違ったら、実装を疑う前に期待値を疑うこと。**

### 仕様への疑義（R38 で判断する）

§49（`docs/spec.md:1985`）は `Int.parse` の失敗を「不正（桁溢れを含む）」としか書かず、**何を受理するかを定めていない**。本節は往復（`Int.parse (Int.to_string n) == n`）から `-?[0-9]+` と i64 の範囲を導き、先行ゼロを受理し、`+`・空白・`_`・小数点・全角数字を拒否すると決めた。仕様は変えない。R38 で §49 に 1 文足すかどうかを判断する。R35 の「長さが違う Array の `compare` の順序」と同じ扱いである。

### 成果物

`runtime/hado_runtime.c` `runtime/hado_runtime.h`、`crates/hado/src/runtime_abi.rs`、`stdlib/int.hd`、`tools/gates/report/report.hd`、`crates/hado/tests/r36_int.rs`（新設）、`scripts/verify-r36.sh`（新設）、`tests/accept/`（`// item: Int.parse` の受理。`// expect-error:` の失敗形を含む。**§49 の member を使うので `// needs: prelude` のヘッダが要る**。`verify-m1-corpus.sh:341-345`）、`tests/corpus.sha256`、`docs/ledger.md`。

### 工程

- 工程 0: `Int.parse`。C の 2 本（`hado_int_parse_is_valid` `hado_int_parse_unchecked`）、`runtime_abi.rs` の 5 箇所、`stdlib/int.hd` の `parse`、**lock の `stdlib/int.hd` の 1 行を更新する**。判定 36-a 36-b 36-c 36-d
- 工程 1: E2 とコーパス。`tools/gates/report/report.hd` の `parse_line`、受理コーパス（`// item: Int.parse`）、lock、門番の追随。判定 36-e 36-f 36-g 36-h

### 判定

`scripts/verify-r36.sh` を新設する。構造は `scripts/verify-r35.sh` に倣う。**入力の形が列挙できる判定は分類から生成する。境界値は乱数任せにせず決定的に含める。期待値は独立した oracle から導く。** 生成器自身の失敗と manifest の欠落・空を非ゼロ終了にし、成功時の stderr と失敗時の stdout が空であることを `wc -c` で測る。探針を絞り込める環境変数を置かない（R35 の `FILTER` を消した経緯）。

### oracle の作り方（この判定の要）

**python3 の `int()` をそのまま oracle にしてはならない。** `int()` は `"+42"` `" 42 "` `"1_000"` `"４２"` を受理するので、本節の決定より寛容である。判定器の oracle は次の 2 段で書く。

```python
valid = re.fullmatch(r'-?[0-9]+', s) is not None and -2**63 <= int(s) < 2**63
expected = int(s) if valid else 'InvalidInt'
```

**oracle が仕様より寛容なときは、判定器の側で仕様の形に絞る。** これは 24.9 の R34 の教訓 3（期待値は独立した oracle から導く）の裏面である。

### 分類（すべての判定でこの積を回す）

- **符号:** 無し / `-` / `+`（拒否）/ `--`（拒否）/ 符号だけ（拒否）
- **本体:** 空 / `0` / 1 桁 / 複数桁（乱数）/ 先行ゼロ付き（`"007"` `"-007"` `"00"`）/ 非数字を含む（先頭 / 中間 / 末尾）/ 空白を含む（先頭 / 中間 / 末尾）/ `_` を含む / 小数点を含む / 全角数字（`４２`）/ 多バイト文字を含む
- **境界（決定的に含める。乱数任せにしない）:** `9223372036854775807`、`9223372036854775808`、`-9223372036854775808`、`-9223372036854775809`、`18446744073709551616`（2^64）、`10000000000000000000`（10^19）、`100000000000000000000`（10^20）、21 桁以上の数字列、**先行ゼロを 25 個付けた `5` と `9223372036854775807` と `9223372036854775808`**（長さで打ち切る実装を落とすため）、埋め込み NUL を含む文字列

判定 36-a: 受理。分類のうち有効な形すべてを、oracle の `int(s)` と照合。**往復を必ず置く**: 乱数の i64（両端 `9223372036854775807` と `-9223372036854775808` と `0` を決定的に含む）を `Int.to_string` → `Int.parse` して元の値と一致。**`-9223372036854775808` は hado の整数リテラルとして書けない**（`lexer.rs:457-462` が数字列を単体で i64 に parse するので `E112: integer out of range`）。`(-9223372036854775807 - 1)` と書く（`crates/hado/tests/emit_mono.rs:411` に同じ慣用句がある）か、`Int.parse "-9223372036854775808"` から作る
判定 36-b: 拒否。分類のうち無効な形すべてが、stderr に `InvalidInt` を出して終了コード 1。**`"9223372036854775808"`（2^63 ちょうど）の拒否を必ず含める**（負の側に累積する実装では符号を戻す段でしか落ちないので、ここを落とすと受理されるか未定義動作になる。計画審査 2 巡目の致命）。**負の対照**: 各拒否形のすぐ隣の有効な形（`"42"` に対する `" 42"`、`"9223372036854775808"` に対する `"9223372036854775807"`、`"-9223372036854775809"` に対する `"-9223372036854775808"`）
判定 36-c: handler で捕まえる。§58 により abort の腕の型は `with` 全体の結果型に一致するので、`Int.parse` が返す `Int` に腕を揃えるか、`Int.to_string` で包んで String にする（R34・R35 で繰り返し踏んだ罠）
判定 36-d: `stdlib/int.hd` の 2 member の型を §49 の宣言と照合（引数型・戻り型・効果行。`to_string` は `Pure`、`parse` は `{Error}`）
判定 36-e: `Report.parse_line`。名前と数を実行時に生成し、`render` の出力を判定器側で計算した文字列と照合。**`n` に `Int.parse` が拒否する形（`"+7"` `" 7"` `"7 "`）を渡したとき `InvalidInt` が出て `InvalidLine` に化けないこと**、区切りが無い行では `InvalidLine` が出ることを、それぞれ 1 本以上
判定 36-f: `bash scripts/verify-m7.sh` の `prelude inventory` が 38/39 から 39/39 になる
判定 36-g: 受理コーパス（`// item: Int.parse` 3 本以上、`// expect-error:` の失敗形 1 本以上。各ファイルに `// needs: prelude` のヘッダを置く）、lock 更新、`verify-m1-corpus.sh` の pairs から `Int.parse` が消える（残るのは Env と Io の 1 項目）
判定 36-h: `cargo test --offline --no-fail-fast` が 0 failed。全門番を単独で走らせ、許容される赤（下記）以外が満点

Rust 側: `crates/hado/tests/r36_int.rs` を新設し、`crates/hado/tests/runtime_string.rs` の流儀で `hado_int_parse_is_valid` と `hado_int_parse_unchecked` の C レベルの動作（境界・NUL を含む入力・21 桁以上）をテストする。**`HadoString` の `bytes` が NUL 終端でないことを突く入力（`"42"` の直後に別の文字が続く領域）を 1 本置く**

### 許容される赤

- `verify-m1-corpus.sh` の 1 pair（Env と Io）と、その連鎖の m2 / m3

### スコープ外

- Env と Io（R37）、仕上げと tag（R38）
- `Float.parse`（24.1 の C5 で「入れない」と決めた）
- `tools/gates/main.hd` の argv とファイルの読み（R37）
- 16 進・8 進・2 進のリテラル、`Int.parse` の基数引数

### 確認して報告（24.9 と本節の決定から）

- 桁ごとの積み上げで溢れる前に止めた形（比較の式）と、`"-9223372036854775808"` を受理し `"9223372036854775808"` を拒否することの実測
- 先行ゼロを 25 個付けた入力が受理されること（受理を長さで打ち切っていないこと）と、埋め込み NUL を含む入力が拒否されることの実測
- `Report.parse_line` で `InvalidInt` と `InvalidLine` がどちらも出ることの実測
- `%` を使う未注釈の再帰生成器が E306 になる件（R35 の工程 0 で踏んだ）に、この工程でも当たったか

### 中断の条件

**C の runtime が hado の List の cell（`repr.rs:117-162` の配置）を読み書きする必要があると判断したら、直さずに中断せよ。**

**`HadoString` を NUL 終端と仮定する C（`bytes` を `strtoll` / `atoll` に渡す、固定長のバッファに写す）を書かないと通せないと分かったら、止めて報告せよ。** 受理を長さで打ち切って通す形は採らない（先行ゼロを無制限に受理する決定と食い違う）。

**`raise` を prelude の包みから起こす経路が intrinsic の呼び出しの後ろで型を合わせられないと分かったら、止めて報告せよ。**

それで届かないと分かったら、どこまで確認したか・何が足りないかを具体的に書いて止める。止めることは正常な終了経路である。

## 30. R37 世界との境界（Env と Io）の手順書（2026-09-13）

24.1 の D 表（D1〜D6）を実装する。prelude が効果 `Env` と `Io` を持ち、操作は `reply` で、handler が無ければ runtime の既定実装が応える。E2（`tools/gates/`）が argv とファイルを読んで完成する。仕上げと tag は R38。

**読む順:** 本節 → 24.1 の D 表と 24.4 → `docs/spec.md` の §5（167〜173 行。`main` の形と E706 の免除）・§47（1861〜1873 行。名前空間と `__` の解決）・§49（1959〜1994 行。`Env` と `Io` の宣言と失敗の `message` の表）・§51（2078〜2110 行）・§57（2261〜2284 行。操作の可視性、handler 無しの既定実装、E513）・§58（2296〜2298 行。handler の置き場と E605）→ 27 節（R34。`Bytes` と `is_utf8` の形）と 29 節（R36。C の走査と hado 側の `raise` の形）→ `docs/corpus.md` の 40〜60 行と 93〜95 行。

**着手時点で赤の門番:** R36 の終了時（`verify-m1-corpus.sh` 6/7 の 1 pair = Env と Io、その連鎖の `verify-m2.sh` 7/8 と `verify-m3.sh` 11/12）。R37 の入口で単独で測り直す。**R37 が完了すると全門番が満点になる見込みで、R38 の入口がそこである。**

### 手順書を書く前の実測（2026-09-13、指揮者）

| 測ったこと | 結果 |
|---|---|
| 引数を取る `reply`（`reply greet(name: String): String`）が check・build・実行を通るか | **通る。** `hi x` を印字して終了 0。コーパスに例が無いだけだった |
| 操作の宣言の効果行（`reply pick: Int {Error}`）が呼び出し位置に届くか | **届かない。** 腕が `raise` しない形で `main` の型を測ると、`{Error}` 付きの宣言でも効果行なしの宣言でも同じ `String` になる。**parser は保持するが infer が捨てている** |
| `effect Io` を書いている既存の場所 | `tests/accept/effect_row/main.hd:6`（コーパス。lock の対象）と `crates/hado/tests/r24_effects.rs` の 8 箇所。`crates/hado/tests/r23_syntax.rs:135` `:137` の `{Io}` は `roundtrip`（parse と canonical 印字の往復だけ。check も prelude も通さない）なので**触らなくてよい** |
| 受理コーパスを実行する門番 | **`scripts/verify-m6.sh:26` の 1 箇所だけ**（`subprocess.run([str(copied.with_suffix(""))], capture_output=True, timeout=60)`）。**cwd も stdin も固定していない**ので、cwd は門番のシェルのもの（リポジトリの根）を継ぎ、stdin も継ぐ |

### 決定

- **工程は 4 つ。地ならし（工程 0）、`Env`（工程 1）、`Io`（工程 2）、E2 とコーパスと門番（工程 3）。** 工程ごとに指揮者が緑を実測してから次を発注する
- **決定 1: 既定実装は prelude の普通の `def` にし、mono はその `def` を呼ぶ。** `stdlib/env.hd` に効果の宣言と並べて操作ごとの `def` を置く（`def default_args = ...`、`def default_env name => ...`）。`register_effects`（`infer.rs:1866-1873`）の後、prelude の範囲で **`op 名 → 既定の def の完全名`** の対応を作って `Checked` に載せる。mono は `OpKind::Reply` で `handler_for(name)`（`mono.rs:1682-1688`）が `None` のとき、その `def` を呼ぶ。今は `None` で `Err("unhandled operation {name}")` になる（`mono.rs:1573-1577`）。**呼び方は arity で 2 通りに分ける**（計画審査 4 巡目の指摘）。
  - **引数のない操作（`args` `cwd` `read_stdin`）**: infer は `Name` として `value_types` に記録する（`infer.rs:998`）ので `definition_reference`（`mono.rs:494`）の経路でよい
  - **引数を取る操作（`env` `read_file` `write_file` `read_dir`）**: infer は `Apply` として `composite_types` に記録する（`infer.rs:944`）。`definition_reference` は `value_types` を読み（`mono.rs:2864`）、`DefKind::Function` では `MakeClosure` を返して**呼ばない**（`mono.rs:2873-2881`）ので通らない。**`application` と同じ形で `specialize_function` + `call_direct` を使い、期待する結果型は `composite_types` から引く**
  - **既定の `def` の arity は操作と同じにする。** `args` は引数 0 なので `def default_args = ...`（ダミー引数を付けない）
  - **`handler` を既定実装にする案は採らない（計画審査 3 巡目の致命）。** handler の腕は `with` で被せたときしか推論されない（`infer.rs:1969` の `handler_answer` は `with_handler` からしか呼ばれず、`infer.rs:175` `:186` と `dependencies.rs:17` は `Item::Handler` を無視する）。審査は `handler Foo as H` の `reply pick => "not an int"` が `reply pick: Int` の宣言と食い違っても `check` が終了 0 になることを実測した。腕が推論されないと mono は span の型（`composite_types`）を引けず、`missing list type` / `missing record type` / `missing abort result type` で落ちる。既定実装には argv の分類・`Bytes.to_string`・`raise { message = ... }` が必ず入るので確実に踏む。**E605（`infer.rs:1884-1891`）は腕名しか見ないので型の保証にはならない**
  - 普通の `def` なら推論は `Item::Def` の既存経路で済み、mono は通常の関数呼び出しになる。`with_params` も `rewriting_return` も `finish_handler` も触らない。既定の `def` の中の `raise` は普通の関数の中の `raise` で、利用者の `handler Error` に届くのは `tests/accept/nested_unrelated_abort` と同じ機構である
  - **既定の `def` の名前は `__` で始めない**（`name_resolution.rs:167` の intrinsic の門に当たる）。対応は名前の規約ではなく `register_effects` が prelude の範囲で `default_<op 名>` を探して `Checked` に載せる。**見つからなければ infer が診断を出す**（黙って mono で落とさない）
  - **既定実装は `module <効果名>Default` と `implement <効果名>Default as <効果名>Default` の中に置く**（`stdlib/string.hd` が手本）。対応は `op 名 → <効果名>Default.<op 名>`。**工程 3 の全走で `verify-m3.sh` の検査 11（`stdlib-types`）が赤になって分かった**: この検査は `stdlib/*.hd` の最初の `module` / `implement` より前に `def` があると落ちる。素の `def` を並べる形がこれに当たる。**門番は変えない**（緩める形になるうえ、赤を出した側が門番を書き換えるのは判定者と作成者の分離に反する）。契約の宣言が効果の宣言の写しになるので、既定の効果行の検査が署名の照合としても書ける。helper は `implement` の中の `def` になり外に出ない（`stdlib/string.hd` の `split` の helper と同じ）。**`<効果名>Default` は契約名の空間に入るので `EnvDefault` と `IoDefault` を予約名に足す**
  - **既定の `def` の効果行が操作の宣言の効果行より広ければ infer が診断を出す。** 操作が `{Error}` で `def` が `Pure` なら通る（狭い方は広い方に含まれる）。逆は落とす。**この検査は `register_effects`（`infer.rs:179`）には置けない。** 既定の `def` の scheme は `definition_group`（`:180-190`）で推論されるので、**名前の対応は `register_effects`、効果と型の検査は推論の後**に置く
  - **E605 の代わりの保証:** `crates/hado/tests/r37_world.rs` に「`Checked` の prelude の `Reply` 操作すべてに既定の `def` の対応がある」を 1 本置く。対応が欠けた操作を prelude に足す注入で落ちること。判定 37-d(4) の「利用者の `handler Env` で 1 操作を落とすと E605」は既存機構なのでそのまま残る
- **決定 2: C は scalar と Bytes だけを返し、`raise` は hado で書く。** R34 の `Bytes.of_list`・R36 の `Int.parse` の写しである。
  - **argv・env・cwd・ディレクトリの名前は `Bytes` で返し、hado 側が `Bytes.to_string` に通す。** 非 UTF-8 の `"InvalidUtf8"` は R34 の機構がそのまま出すので、**C は UTF-8 を一切見ない**
  - **すべての intrinsic は少なくとも 1 引数を取る。引数のない intrinsic は今の構文で呼べない。** `name_resolution.rs:163-169` は呼び出し位置にない `__` 名を E203 にし、parser の application は引数 1 個以上でしか `Apply` を作らない。**引数が要らないものは無視する `Int` のダミーを 1 つ取り、`0` を渡す**（`__argv_count(ignored: Int): Int` を `__argv_count 0` と呼ぶ）。C 側は引数を使わないことをコメントに書く
  - `args` は `__argv_count 0` + `__argv_get(Int): Bytes` の再帰で組む（`stdlib/bytes.hd` の `elements` が手本）。`env` は `__env_has(String): Bool` + `__env_get(String): Bytes`
  - **C は `hado_abort_error` を絶対に呼ばない。** 呼ぶと利用者の `handler Error` を飛び越える。失敗は必ず hado 側の `raise { message = ... }` から起こす
  - **I/O の失敗の分類は status を Int で返し、payload は別の intrinsic で取る。** `__io_read_file(path): Int`（0 成功 / 1 NotFound / 2 PermissionDenied / 3 NotADirectory / 4 IoFailure。syscall はここで 1 回だけ）→ 0 なら `__io_take_bytes 0`。`is_valid` + `unchecked` の 2 回 syscall にしない（2 回の間に状態が変わると失敗の分類がずれる）。`read_dir` も同じ形（`__io_read_dir(path): Int` → `__io_dir_count 0` + `__io_dir_get(Int): Bytes`）。status → `message` の対応は hado 側の 1 本の関数に書く
- **決定 3: 操作の結果の効果行を infer で通す。** 実測のとおり `reply pick: Int {Error}` の `{Error}` は呼び出し位置に届かない。`payload_type`（`infer.rs:892-899`）→ `annotation_inner`（`infer.rs:1627-`）は `Fn` の枝でしか `.effects` を読まず、`unwrap_operation`（`infer.rs:2039-2051`）が `Row::Closed(vec![*effect])` で効果 1 つに固定する。**`register_effects` で `op.ret` の効果行を op ごとに保存し、`unwrap_operation` でその行と合併する。**
- **決定 4: `main` の効果の免除を prelude の効果の集合に一般化する。** `types.rs:76-78` の `has_user_effect` は `effect_ctor(ty) != "Error"` の直書きである。`Env` と `Io` を直書きで足すと次に増えたとき同じ穴になるので、**prelude の効果名の集合との照合に変える**。§5（`docs/spec.md:169`）の「`main` に残ってよい効果は prelude の効果（`Error` `Env` `Io`）に限る」がそのまま出る。**集合の出所は `lib.rs` の `prelude_items` の範囲から作る**（`infer` に `["Error", "Env", "Io"]` を直書きすると、次に prelude の効果が増えたとき同じ穴になる）。**`Checked` ではなく `Checker` のフィールドに持たせる。** `has_user_effect` は `infer.rs:239` で呼ばれ、`Checked` の構築（`:251`）より前である。`infer_with_diagnostics`（`infer.rs:72`）は今 `prelude_items` を受け取らないので、引数として渡す 1 本が要る。名前は `effect_ctor`（`types.rs:659`。`$` を剥がし `::` は残す）と揃える
- **決定 4a: ローカル束縛が prelude の操作名を影にできるようにする（工程 0）。** `namespaces.rs:390-397` の `operation` は bare 名が `prelude_operations` にあれば無条件に prelude の操作へ書き換え、呼び出し元の `node`（`:399-402`）はローカルの束縛を見ない。計画審査 3 巡目の実測: `def f (args: Int): Int => args` は `check` を通り、build 後に `unhandled operation args` で落ちる。`raise` でも同じ（`def f (raise: Int): Int => raise`）。**直す場所は `namespaces.rs` ではなく mono である**（計画審査 4 巡目が実測で特定した）。`namespaces.rs:390-397` の `operation` は prelude の操作名に対して `*name = bare` を代入するだけで、bare 名では恒等変換なので、束縛の集合を渡しても影には効かない。実際に操作として扱っているのは **`mono.rs:482-485`（`Name`）と `mono.rs:1424-1427`（application）が `self.operations.get(name)` を `self.scope.contains_key(name)`（`:493` `:1452-1453`）より先に判定している**ところである。審査はこのコピーで mono の 2 箇所だけを scope 優先に変えて再ビルドし、`def f (raise: Int): Int => raise` / `def main = f 3` が `3` を印字して終了 0 になることを実測した（直す前は終了 1 で空メッセージ＝ `abort_raise`）。**`is_abort_node` / `is_capture_node`（`mono.rs:1715-1739`）も `operations` を見るので同じ順序の見直しが要る。** **mono だけでは足りない。infer 側にも影がある**（計画審査 5 巡目の実測）: `infer.rs:996` → `unwrap_operation`（`:2039-2051`）は名前が `operations` にあればローカルの束縛を無視し、`Ty::Fn` を操作の型として解釈する。`def f (pick: Fn(Int => Int)): Int => pick 3`（`pick` が prelude の操作なら）は `E305: cannot unify Int with Fn(Int => t...)` になる。`Int` 型のローカルでは `unwrap_operation` が素通しするので、判定 37-b2 の `def f (raise: Int)` の形では捕まらない。**判定 37-b2 に関数型のローカル（`def f (raise: Fn(Int => Int)): Int => raise 3`）を必ず含める。** **infer 側の直し方は 2 つある。** (a) ローカル束縛の集合を `Checker` に持たせて save / restore する（`values` は `function_body`（`:1360`）・`pattern`（`:1255`）・`bind_param`（`:2021`）で上書きされ、`:996` の時点で操作かローカルかを `values` からは区別できない）。(b) **`unwrap_operation`（`:2039`）で、外側の `Fn` の結果が `self.effects` に載る効果の ADT であることを確かめる**（操作の scheme は `Fn([op_ty], Adt(効果名))` に固定されるので、ローカルはここで弾ける）。計画審査 6 巡目が (b) を 4 行でコピーに入れて再ビルドし、関数型のローカルが `check` を通り `cargo test --offline` が全体で緑のままであることを実測した。**(b) を採る。** `namespaces.rs` の束縛集合の対応は、サブディレクトリで同じディレクトリの効果の操作名を影にする場合にだけ要る（そのとき `operation_key` が `dir$name` に書き換える）。§47 は shadowing を許す。R37 は 7 名を prelude に足すので、増やす前に直す。これは 24.9 の「ローカル束縛が prelude 操作名を shadow すると昇格が崩れる」で、R38 に回していた穴である。**工程 0 で `raise` を使って測れる**（`Env` / `Io` が入る前）
- **決定 4b: prelude の操作と同名の利用者の宣言を E205 にする（D6、§57 の `docs/spec.md:2274`）。** 今は実装されていない。`namespaces.rs:69-84` の `declarations(item)` が `Item::Def` / `Item::SigDef` を返さないので、`:136-143` の prelude との衝突検査に載らない。しかも `namespaces.rs:390-397` の `operation` は bare 名が `prelude_operations` に一致すると無条件で prelude の操作に書き換えるので、**利用者の `def` は黙って影になる**（実測: 外の `def raise = 1` と `def main = raise` は E205 ではなく E513）。R37 は `args` `env` `cwd` `read_file` `write_file` `read_dir` `read_stdin` の 7 名を prelude に足すので影響が最大になる。**汎用の `declarations` に値の空間を足すのではなく、専用の検査を置く。** 理由は 2 つある（計画審査 2 巡目の指摘）。(1) `file_scopes.rs:40-47` がトップレベルの `def` 名を `file{N}$name` に改名するので、`declarations` に `Item::Def` を足して `has_prelude(Value, name)` と比べても `file0$args` と `args` は一致せず E205 にならない。(2) prelude の操作は `prelude_operations`（`namespaces.rs:126`）にあり `prelude[Space]` には入らないので、汎用経路に載せると比較先が違う。しかも `Item::ModuleImpl` の `def` を足すと `namespaces.rs:136-141` の `directory != root` の比較で、既存の `tests/library/store`（根の `implement Store as Memory` の `def find` とサブディレクトリの `def find`）が新たに E205 になる。**検査は「`Item::Def` と実装の `def` の bare 名（`file_value` で `file{N}$` を剥がす。`namespaces.rs:90-94`）が `prelude_operations` に含まれるなら E205」の 1 本にする。** 根の同名との比較や prelude の通常の `def` は混ぜない。**他の効果の操作（`reply args`）は根では既存の機構で E205 になるが、サブディレクトリでは漏れる**（計画審査 5 巡目の実測: `effect Foo` / `reply raise: Int` を根に置くと E205、同じものを `sub/` に置くと終了 0）。`namespaces.rs:293` が効果の操作名を `operation_key(directory, name)` で `sub$raise` に書き換えるので、後段の `names.rs:456-461` の値の空間の衝突（bare を前提にする）に載らないためである。§57（`docs/spec.md:2274`）と D6 は「外の `def`、実装の `def`、他の効果の操作」を**どのディレクトリでも** E205 と定める。**E205 の検査（`namespaces.rs:136-142`）に、効果の操作名を `prelude_operations` と bare で比べる 1 本を足す。** 計画審査 6 巡目が実測で確かめた: この検査は `resolver.item(item, ...)`（`:144`）より前に走り、`op.name = operation_key(directory, &op.name)`（`:293`）は `item()` の中なので、**検査の時点では `effect.ops[].name` はまだ素の名前である**。コピーに足して再ビルドすると `sub/` の `effect Foo` / `reply raise: Int` が `sub/foo.hd:2:7: error[E205]`（前は終了 0）、衝突しない `reply zap` の対照は終了 0 になった。**根では既存の機構（`names.rs:456-461`）が既に E205 を出すので、新しい検査は `directory` が空でないときだけ出す**（同じ段階で 2 経路から出ても報告は 1 件に畳まれるが、経路を分けておく）。**既存に衝突する宣言は無い**（実測: `raise` `args` `env` `cwd` `read_file` `write_file` `read_dir` `read_stdin` の 8 名で `def` の grep がすべて 0 件）。判定 37-e で固定する
- **決定 4c: prelude の効果の handler をどのディレクトリにも置けるようにする（§58 の `docs/spec.md:2296`）。** `namespaces.rs:300-302` は `declared_directory(&handler.effect) != directory` で全 handler を効果と同じディレクトリに強制し、prelude の効果（bare 名なので `declared_directory` は空）はサブディレクトリで必ず不一致になる。**prelude の効果を除外する。** これは 24.9 に「サブディレクトリの中の `handler Error` が E204」として残していた既知の穴で、R37 の判定 37-e が「`handler Env as X` はどのディレクトリにも置ける」と主張する以上、ここで直す。**除外は E204 の検査だけでは足りない。** `namespaces.rs:308` は handler の腕の名前を無条件に `operation_key(directory, name)`（`:86-88`。`dir$name`）で前置する。prelude の効果の操作は bare のままなので、E204 を外しただけだと腕名が `sub$raise` になり、`infer.rs:1884-1891` の E605 の照合（期待 `("abort", "raise")` 対 実際 `("abort", "sub$raise")`）で落ちる（計画審査 2 巡目が E204 の検査だけを外して再ビルドし、`sub/h.hd:1:1: error[E605]` を実測した）。**`:301-303` の E204 と `:308` の前置の両方を、prelude の効果かどうかで分岐させる**（判定は `has_prelude(Space::Contract, &handler.effect)` で、その箇所で手に入る）
- **決定 5: `main` は引数を取らない（E206 のまま）。** argv は `Env.args` で取る。`names.rs:419-426` の `entry_form` は触らない
- **決定 6: C の `int main` だけを `int main(int argc, char **argv)` に変え、`argc` / `argv` をファイルスコープの static に退避する。** `hado_entry(void)` の宣言（`runtime/hado_runtime.c:11`）も `emit.rs:592-595` と `:64-70` の signature も**触らない**。これが最小の変更である。`scripts/verify-m0.sh:94-99` は `GC_INIT` `hado_entry` `hado_alloc` `GC_malloc` の綴りを `runtime/` に求めるだけなので緑のまま
- **決定 7: `effect Io` の衝突を工程 0 で改名する。** `tests/accept/effect_row/main.hd`（コーパス。lock も更新する）と `crates/hado/tests/r24_effects.rs` の `Io` を `Log` にする。**`effect Io` の 8 箇所だけでなく、`{Io}` `{Ask, Io}` `{Io, Int Yield}` の型注釈と `print_type` の期待文字列を含む全出現を直す**（13 行・25 箇所。片方だけ直すと E205 か E204 になる）。`crates/hado/tests/r23_syntax.rs:135` `:137` は `roundtrip`（parse と canonical 印字の往復）なので触らない
- **決定 8: 受理コーパスの起動条件を門番に固定する。** `scripts/verify-m6.sh:26` は cwd も stdin も固定していない。24.5 の R37 の行が「受理ケースは空 argv・閉じた stdin・ケース内の相対パスで起動する」と定めているので、**`cwd=str(case)` と `stdin=subprocess.DEVNULL` を足す**（門番を強くする変更で、判定の条件は変えない）。実行ファイルのパスは絶対なので cwd を変えても動く
- **決定 8b: `scripts/verify-r32.sh` の 32-j(d) を工程 3 で追随させる。** `verify-r32.sh:169-184` の `report_case_real` は実物の `tools/gates/main.hd` をコピーして**引数なしで**実行し、`33/35 passed; red: m7` を期待する。E2 を argv 駆動にすると空 argv は `MissingDirectory` になるので終了コード 1 で落ちる。**判定器がディレクトリを作り、`PASSED:` の行を持つログを実行時に生成して、そのディレクトリを引数に渡す形に直す**（期待値は判定器側で計算する。固定文字列 `33/35 passed; red: m7` は消える）。`report_build_run`（`verify-r32.sh:99`）は引数を渡さないので、**引数付きで実行する変種を 1 本足す**。門番を緩める変更ではなく、E2 の入口が変わったことへの追随である
- **決定 8c: `scripts/verify-r36.sh` の 36-g を工程 3 で追随させる。** `verify-r36.sh:381-390` の `pairs_remaining` は `verify-m1-corpus.sh` が終了 1 で `NG: 3 pairs: accept-missing:Env と Io` と `PASSED: 6/7` を出すことに依存する。R37 が `// item: Env と Io` の受理を足すと 7/7 になってこの検査が落ちる。**残る pairs が 0 であること（`PASSED: 7/7`）を期待する形に直す。** 門番を緩める変更ではなく、R36 が「残り 1 項目」を測っていたものを「残り 0」に更新する追随である
- **決定 9: `path` と env の `name` は libc に直接渡さない。** `HadoString` は NUL 終端でない（`runtime/hado_runtime.h:6-9`）。`fopen` `opendir` `getenv` `stat` に渡す前に `length + 1` の領域を確保して NUL を付ける。R36 は「写さない」が正解だったが、**R37 は逆で「写さないと越境する」**

### 仕様への疑義（5 件。R38 で判断する。§49 が定めていない）

- **`read_dir` は名前だけを返し、`.` と `..` を除き、バイト順に整列する。** `readdir` の順序はファイルシステム依存なので、整列しないと判定器が期待値を持てない。**整列は決定ではなく判定可能性の要件である**
- **`read_stdin` の 2 回目は空の `Bytes`**（EOF）。24.7 の未確定
- **`env` の値が非 UTF-8 なら `"InvalidUtf8"` を raise する**（`None` ではない）。§49 の失敗の表（1980 行付近）は「argv・env・cwd・ディレクトリ名の非 UTF-8」を `"InvalidUtf8"` とするが、**env の「名前」と「値」のどちらを指すかが曖昧である**。本節は値にも当てる
- **既定実装の契約名 `EnvDefault` と `IoDefault` を予約にする。** §47 の予約名の一覧（`docs/spec.md:1861-1873`）には無い。既定実装を `module` に入れる決定から出た副産物である
- **E2 の空 argv は `raise { message = "MissingDirectory" }`。** `tools/gates/` は利用者のコードなので §49 の表の外でよい（R34 の `"InvalidLine"` が先例）

**前提（疑義ではない）:** `args` はプログラム名を除く（§49 の `docs/spec.md:1974`）。`verify-m6.sh` が実行ファイル 1 つだけで起動する形では空リストになる。

### 期待値の根拠

| 形 | 導出 | 根拠 |
|---|---|---|
| `Env.args` を引数なしで起動 | `[]` | §49（1974 行）、決定 8 |
| `Env.args` を引数 3 つで起動 | その 3 つの String List | §49 |
| argv に非 UTF-8 のバイトを含めて起動して `args` | `Error` `"InvalidUtf8"` | §49 の表（1980 行付近） |
| `Env.env "<設定した名前>"`、`Env.env "<未設定の名前>"` | `Some("<値>")`、`None` | §49（1974 行） |
| `Env.cwd` | プロセスの cwd の String | §49 |
| `Io.read_file "<存在するファイル>"` → `Bytes.to_string` | ファイルの中身 | §49 |
| `Io.read_file "<存在しないパス>"` | `Error` `"NotFound"` | §49 の表 |
| `Io.read_file "<ディレクトリ>"` | `Error` `"IoFailure"`（EISDIR） | 本節の決定 |
| `Io.read_dir "<通常ファイル>"` | `Error` `"NotADirectory"`（ENOTDIR） | §49 の表 |
| `Io.read_dir "<ディレクトリ>"` | 名前だけを整列した String List（`.` と `..` を除く） | 本節の決定（疑義） |
| `Io.write_file` → `read_file` の往復 | 書いた `Bytes` と一致 | §49 |
| `Io.read_stdin` を閉じた stdin で | 空の `Bytes` | 本節の決定（疑義）と決定 8 |
| `Io.read_stdin` の 2 回目 | 空の `Bytes` | 本節の決定（疑義） |
| 利用者の `handler Env as MyEnv` を `with` で被せる | 既定ではなく利用者の腕が応える | §58、D2 |
| `with H -> args`（`H` は `handler Error`）で argv が非 UTF-8 | `H` の腕が `message` を受け取る | 決定 2（C が `hado_abort_error` を呼ばないことの固定） |
| `main` の型が `{Env, Error, Io}` | E706 にならない | §5（169 行）、決定 4 |
| 利用者の `effect Env` / `effect Io` の宣言 | E205 | §47（1865 行）、D6 |
| `reply pick: Int {Error}` の呼び出し位置 | `{Error, Foo}`（印字は整列順。`types.rs:52-57`） | 決定 3 |
| `handler Io as X` で 4 操作のうち 1 つを落とす | E605 | §58（2298 行） |

**本体では 1 つも通っていないので、期待値は仮説である。実測と食い違ったら、実装を疑う前に期待値を疑うこと。**

### 成果物

`runtime/hado_runtime.c` `runtime/hado_runtime.h`、`crates/hado/src/`（`runtime_abi.rs` `mono.rs` `infer.rs` `types.rs` `lib.rs`）、`stdlib/env.hd`（新設）`stdlib/io.hd`（新設）、`tools/gates/main.hd` `tools/gates/report/report.hd`、`crates/hado/tests/r37_world.rs`（新設）、`crates/hado/tests/r24_effects.rs`（`Io` → `Log`）、`scripts/verify-r37.sh`（新設）、`scripts/verify-m6.sh`（決定 8）、`scripts/verify-r32.sh`（決定 8b）、`scripts/verify-r36.sh`（決定 8c）、`crates/hado/src/namespaces.rs`（決定 4b・4c）、`tests/accept/`（`// item: Env と Io` の受理と `effect_row` の改名）、`tests/corpus.sha256`、`docs/ledger.md`。

### 工程

- 工程 0: 地ならし。決定 7（`effect Io` の改名と lock 更新）、決定 3（操作の結果の効果行）、決定 4（`has_user_effect` の一般化）、**決定 4a（ローカル束縛が prelude の操作名を影にできる。mono と infer の両方）**。**`Env` と `Io` はまだ入れない。** 判定 37-a 37-b2（**37-b は工程 1 に移す**。prelude の効果が `Error` 1 つだけの工程 0 では、集合との照合と `!= "Error"` の直書きを区別できない）
- 工程 1: `Env`。`stdlib/env.hd`（効果の宣言 + 操作ごとの既定の `def`）と **`lib.rs:145-150` の prelude の一覧への追加**（足さないと prelude に入らない）、C の `main(argc, argv)` と static（決定 6）、intrinsic（`__argv_count` `__argv_get` `__env_has` `__env_get` `__cwd`）、**mono の `Reply` の既定の `def` へのフォールバック**（決定 1。新しい機構はここだけ）、**決定 4b（prelude の操作と同名の宣言を E205）と決定 4c（prelude の効果の handler をどのディレクトリにも置ける）**、lock。判定 37-b 37-c 37-d 37-e
- 工程 2: `Io`。`stdlib/io.hd` と `lib.rs:145-150` の一覧への追加、C の 4 操作と status（決定 2）、lock。判定 37-f 37-g
- 工程 3: E2 とコーパスと門番。`tools/gates/main.hd` を argv + `read_dir` + `read_file` に、決定 8（`verify-m6.sh` の cwd と stdin）、**決定 8b（`verify-r32.sh` の 32-j(d) の追随）**、受理コーパス（`// item: Env と Io`）、lock。判定 37-h 37-i 37-j 37-k

### 判定

`scripts/verify-r37.sh` を新設する。構造は `scripts/verify-r36.sh` に倣う。**入力の形が列挙できる判定は分類から生成する。境界値は乱数任せにせず決定的に含める。期待値は独立した oracle（python3 の `sorted(os.listdir(...))`、`os.environ`、`open(p, 'rb').read()`）から導く。** 生成器自身の失敗と manifest の欠落・空を非ゼロ終了にし、成功時の stderr と失敗時の stdout が空であることを `wc -c` で測る。探針を絞り込める環境変数を置かない。

**fixture は判定器が `mktemp -d` に作り、判定器が cwd をその中に移して実行ファイルを起動する**（受理コーパスと同じ条件にする）。非 UTF-8 は bash の `$'\xff'` で argv・env の値・ファイル名の 3 箇所に決定的に入れる。**`main` の値は印字できる型に包む**（R35・R36 と同じ罠。`String List` は印字できるが、`Bytes` は `Bytes.of_list [...]` の形で出る）。

判定 37-a: 操作の結果の効果行。`reply pick: Int {Error}` の呼び出し位置が `{Error, Foo}` になる（`--print-type` で測る）。**効果行の印字はラベルを `BTreeSet` で整列する**（`types.rs:52-57` の `Row::suffix`）ので、期待値は必ず整列順で書くか集合で比べる。`{Foo, Error}` の順で固定文字列と比べると落ちる。**腕が `raise` しない形で測る**（腕の `raise` から来る `{Error}` と区別するため。指揮者の実測ではこれで区別できた）。効果行なしの宣言との対照、効果行が 2 つ以上の形、`abort` の操作（戻り型を書けないので効果行も無い）の対照
判定 37-b（**工程 1**）: `main` の効果の免除。利用者の効果が `main` に残れば E706、prelude の効果なら通る。**`Env` を足した後に `main` の型が `{Env, Error}` で通ることを測る。** 工程 0 では prelude の効果が `Error` 1 つだけなので、集合との照合と `!= "Error"` の直書きを区別できない
判定 37-b2: ローカル束縛の影（決定 4a）。`def f (raise: Int): Int => raise` が受理されて `f 3` が `3` を印字する。`let raise = 5 -> raise + 1` も同じ（**`let raise = 5 -> raise` は束縛をそのまま返す形なので E501 になる**。審査の実測）。**対照**: 束縛の外の `raise` は prelude の操作のままで、`raise { message = "x" }` が `Error` を起こす。工程 1 以降は `args` `env` `cwd` `read_file` `write_file` `read_dir` `read_stdin` の 7 名でも同じ形を回す
判定 37-c: `Env` の 3 操作。argv の分類（空 / 1 個 / 複数 / 空文字列の引数 / 非 UTF-8）、`env` の分類（設定 / 未設定 / 値が空文字列 / 値が非 UTF-8 / 名前が非 UTF-8）、`cwd`。期待値は python3 から導く
判定 37-d: 既定と差し替え。(1) handler 無しで `args` が既定の `def` で応える。(2) 利用者の `handler Env as MyEnv` を `with` で被せると利用者の腕が応える。(3) **`with H -> args`（`H` は `handler Error`）で argv が非 UTF-8 のとき `H` の腕が `message` を受け取る**（C が `hado_abort_error` を呼んでいないことの固定）。(4) 利用者の `handler Env` で 3 操作のうち 1 つを落とすと E605（既存機構）。(5) **prelude の `Reply` 操作すべてに既定の `def` の対応がある**（`crates/hado/tests/r37_world.rs` で `Checked` を直接見る）
判定 37-e: 予約名と可視性。(1) 利用者の `effect Env` の宣言が E205。(2) **prelude の操作と同名の利用者の宣言が E205**（決定 4b。外の `def args`、実装の `def args`、他の効果の操作 `reply args`。**単一ファイルと複数ファイルの両方で測る**。今は複数ファイルで黙って影になる）。(3) **`handler Env as X` をサブディレクトリに置いても通る**（決定 4c。今は E204）。同じ形の `handler Error as X` も通ることを対照に置く。(4) サブディレクトリからも `args` が無修飾で見える（R34 の `prelude_operations` が `Env` にも効くことの固定）
判定 37-f: `Io` の 4 操作。`read_file`（存在する / 空ファイル / バイナリ / 大きいファイル）、`write_file` → `read_file` の往復、`read_dir`（空 / 1 個 / 複数 / 名前が非 UTF-8 / `.` と `..` を除く / 整列）、`read_stdin`（閉じた stdin / 内容がある stdin / 2 回目）
判定 37-g: `Io` の失敗の分類。**4 分類すべてに決定的な入力を置く。** `NotFound`（存在しないパス）、`NotADirectory`（通常ファイルを `read_dir`）、`IoFailure`（ディレクトリを `read_file`。EISDIR）、`PermissionDenied`（`chmod 000` のファイル）。**`PermissionDenied` は root では起きないので、`id -u` が 0 なら `NG: skipped-as-root` として失敗にする**（skip で緑にしない）。`write_file` の失敗（存在しないディレクトリ → `NotFound`、読み取り専用ディレクトリ → `PermissionDenied`）
判定 37-h: E2。`tools/gates/main.hd` が argv でディレクトリを受け、`read_dir` で列挙し、`read_file` で読み、`Bytes.to_string` → `String.split "\n"` → 各行を `Report.parse_line` に通して集計し、`render` を印字する。**`scripts/verify-r32.sh` の 32-j(d) を決定 8b の形に直し、実物の `main.hd` が実行時に生成したディレクトリを引数に受けて動くことを測る。** **行の選び方は R38 のシェル側 oracle と同じ規則にする**（R38 で作る集計スクリプトが `grep -E '^PASSED:' | tail -1` で最後の 1 行を取る。**`scripts/run-gates.sh` はまだリポジトリに無い**（指揮者のスクラッチにある同名のものは別物）。E2 も `String.starts_with line "PASSED: "` に一致する**最後の**行を選び、名前はファイル名から作る。`tail -1` と一致させる）。**`Report.parse_line` は `<名前> PASSED: <n>/<N>` の 1 行だけを受ける**（`report.hd` の `fields` が `String.split line " PASSED: "` で 2 分割するので、名前の無い行をそのまま渡すと 1 要素になって `InvalidLine` で落ちる）。**E2 は選んだ行の `PASSED: ` の前にファイル名を付けて組み立ててから `parse_line` に渡す。**空 argv は `MissingDirectory`
判定 37-i: 受理コーパス（`// item: Env と Io` 1 本以上。`// needs: prelude` 付き）、lock 更新、`verify-m1-corpus.sh` の pairs から `Env と Io` が消える。**受理コーパスは E2 とは別の小さな fixture 読みのケースにする**（E2 を受理コーパスに入れると、空 argv の既定と実行ファイルの `main` の混入を捌く必要が出る）
判定 37-j: 決定 8。`verify-m6.sh` が cwd と stdin を固定する。**固定を外す注入で受理ケースが落ちること**を報告に書く
判定 37-k: `cargo test --offline --no-fail-fast` が 0 failed。全門番を単独で走らせ、**すべて満点**（R37 の完了で許容される赤が無くなる見込み）。**判定 37-g の `PermissionDenied` は root では成立しないので、`verify-r37.sh` は root で失敗する。** これは環境の要件であって門番の緩めではない（実測: このマシンの `id -u` は 1000）

Rust 側: `crates/hado/tests/r37_world.rs` を新設し、C レベルの動作（`hado_argv_count` と static の退避、status の分類、NUL を付ける写し）と、`check_source` での E205・E605・E706 をテストする。

### 許容される赤

**無い見込みである。** R37 の完了で `verify-m1-corpus.sh` が 7/7、`verify-m2.sh` が 8/8、`verify-m3.sh` が 12/12 になり、全門番が満点になるはずである。ならなければ、残った赤の理由を報告する。

### スコープ外

- 仕上げと tag（R38）。`docs/spec.md` への追記（疑義は R38 で判断する）
- `Float.parse`、`Io` の追加操作（`append_file` `remove` `mkdir` 等）、`Env.set`
- 効果が E513 の対象外である件（`dependencies.rs:17` `:177` `:247`）。当たったら `docs/bug.md` に書いて報告する
- `Space::Contract` に効果名が入っている件（`namespaces.rs:80`）。§47 は効果を別空間と言う。**利用者の `effect Env` が E205 になることだけ判定 37-e で固定し、`module Env`（契約）がどうなるかは「確認して報告」に置く**

### 確認して報告（24.9 と本節の決定から）

- 決定 1 の mono の経路（引数なしは `definition_reference`、引数ありは `specialize_function` + `call_direct`）が成立したか、`op 名 → 既定の def 名` の対応をどこに載せたか、既定の `def` が見つからないときの診断をどう出したか
- 決定 3 で `register_effects` と `unwrap_operation` のどちらをどう直したか、`abort` の操作（戻り型を書けない）に影響が無いか
- `module Env`（契約）を利用者が書くとどうなるか（E205 か、通るか）
- 決定 4c の分岐を `has_prelude(Space::Contract, ...)` で書いたとき、prelude の**契約名**（`List` `Option` `String` 等）に対する `handler` でも E204 を飛ばして E605 になる。判定 37-e の負の対照に 1 本置いて、何が出るかを報告する
- 効果を宣言して使わないと E513 になるか（`dependencies.rs` が効果をスキップしている件）
- `read_stdin` を 2 回呼んだときの 2 回目の値の実測
- C で NUL を付けて写した箇所（`fopen` `opendir` `getenv` `stat` に渡す前）

### 中断の条件

**決定 1 の「`OpKind::Reply` で handler が無いとき prelude の既定の `def` を呼ぶ」が通らないと分かったら、止めて報告せよ。** `handler` を既定実装にする案は計画審査 3 巡目で否決されている（腕が推論されない）ので、そこへは戻らない。

**C が `hado_abort_error` を呼ばないと失敗を起こせないと分かったら、止めて報告せよ。** 呼ぶと利用者の `handler Error` を飛び越えるので、その形は採らない。

**ADT（`String List` や `String Option`）を C から直接返さないと通せないと分かったら、止めて報告せよ。** `value_signature`（`runtime_abi.rs:93-124`）の戻り型はスカラと `String` / `Bytes` だけで、ADT を返す項目は 1 つも無い。組み立ては hado 側で書く。

**C の runtime が hado の List の cell（`repr.rs:117-162` の配置）を読み書きする必要があると判断したら、直さずに中断せよ。**

それで届かないと分かったら、どこまで確認したか・何が足りないかを具体的に書いて止める。止めることは正常な終了経路である。

## 31. R38 仕上げと v0.3 の tag の手順書（2026-09-14）

24.5 の R38 を実装する。**全門番が満点、`cargo test` 0 failed、E2 とシェル側 oracle の一致、`v0.3` を tag。** 加えて R32〜R37 が積み上げた**仕様への疑義と実装の穴を裁定する**。新しい言語機能は足さない。

**読む順:** 本節 → 24.5 の R38 行と 24.9 の未確認 → 28 節（R35）・29 節（R36）・30 節（R37）の各「仕様への疑義」→ `docs/ledger.md` の R32〜R37 → `docs/bug.md`（今は空）→ `docs/spec.md` の §45（1834 行付近）・§47（1861〜1873 行）・§49（1959〜1994 行）。

**着手時点で赤の門番:** **無い。** R37 の終了時（2026-09-14）に全門番が満点になった。R38 の入口で単独で測り直し、赤があればそれを先に報告する。

### 決定

- **工程は 3 つ。工程 0（E2 の実走と oracle 一致）、工程 1（疑義の裁定と `docs/` の追記）、工程 2（版・lock・tag の準備）。** 工程ごとに指揮者が緑を実測してから次を発注する。**tag を打つのは指揮者**で、builder は打たない
- **決定 1: `scripts/run-gates.sh` をリポジトリに置く。** 今は指揮者のスクラッチにしかない（30 節の判定 37-h が「まだリポジトリに無い」と書いている）。全門番を直列に走らせ、門番ごとに `<門番名>.log` を 1 つのディレクトリに**平坦に**吐く。**E2 はそのディレクトリを argv で受ける。** **入れ子にしない**（計画審査 1 巡目が実測: E2 は `read_dir` の各名を `read_file` に渡すので、`<門番名>/<門番名>.log` の木では `EISDIR` から `IoFailure` になって終了 1。直下のファイルなら `8/9 passed; red: m1.log` を正しく出す）。**ログのディレクトリにファイル以外を置かない**のが E2 の入力の契約である。 走らせるのに 90 分以上かかるので、**`run-gates.sh` 自体を判定器の中で全走させない**。判定は生成したログのディレクトリに対して行い、全走は指揮者が別に 1 回だけやる
- **決定 2: oracle はシェル側に独立に書く。** 各ログから `grep -E '^PASSED:' | tail -1` で最後の 1 行を取り、`n/N` を足し、`n < N` の門番名を赤の一覧にする。**E2 の実装を写さない**（R34 の教訓「期待値は独立した oracle から導く」）。E2 の行の選び方（`String.starts_with line "PASSED: "` に一致する最後の行）はこの `tail -1` と一致する規約で、30 節の判定 37-h がすでにその形で測っている
- **決定 3: 疑義の裁定は 3 つに分ける。** (a) **仕様に 1 文足す**（言語の意味が定まっていないもの）、(b) **`docs/bug.md` に 1 節足す**（実装の穴で v0.3 では直さないもの）、(c) **何も足さない**（実装が仕様どおり、または仕様の対象外）。**裁定の前に必ず現状を再現して確かめる。** 記録は他ラウンドの伝聞なので、再現しないものを仕様や bug.md に書かない
- **決定 4: 仕様を変えるのは R38 だけである。** 24.4 は「仕様に疑義が出たら仕様を変えず報告に書く」と定めており、R32〜R37 はそれに従って溜めてきた。ここでまとめて当てる。**追記は「1 文足す」に留め、既存の文を書き換えない。** 書き換えが要ると判断したら、変えずに報告する

#### 裁定の対象（14 件。builder は 1 件ずつ再現してから裁定する。**計画審査 1 巡目が 2 件を (b) から (c) へ移した**）

**仕様に足す見込みのもの（a の候補）:**

| # | 疑義 | 出所 | 節 |
|---|---|---|---|
| 1 | 長さが違う Array の `compare` の順序。R35 は「共通部分がすべて等しいときだけ短い方を先」にした | 28 節 | §45 |
| 2 | `Int.parse` の受理文法（`-?[0-9]+` かつ i64 の範囲。先行ゼロは無制限に受理、`+`・空白・`_`・小数点・全角数字は拒否）。**1 文の中で「往復から導いた下限」と「本節が決めた上限」をラベルで分ける**（29 節がそう書いている） | 29 節 | §49 |
| 3 | `read_dir` は名前だけを返し、`.` と `..` を除き、バイト順に整列する | 30 節 | §49 |
| 4 | `read_stdin` の 2 回目は空の `Bytes` | 30 節 | §49 |
| 5 | `env` の**値**が非 UTF-8 なら `"InvalidUtf8"` を raise する（名前だけでなく） | 30 節 | §49 |
| 6 | 既定実装の契約名 `EnvDefault` `IoDefault` が予約になる | 30 節 | §47 |

**実装の穴の見込みのもの（b の候補）:**

| # | 症状 | 出所 |
|---|---|---|
| 8 | 関数を返す handler の build が `callee is not a closure` で落ちる | R33 の審査 |
| 9 | module member が効果行付きの関数値を返すと `specialization failed: Mismatch` | R33 の台帳（`docs/ledger.md` の 2226 行付近） |
| 11 | `%` を使う**効果行を書かない**再帰生成器が E306（計画審査 2 巡目の実測: `def rec generate (n: Int): Int List` でも `E306`。`Int List {Error}` と書けば build できて `5`） | R35 の工程 0 |
| 12 | 効果が `dependencies.rs` の宣言に数えられておらず、未使用の効果が E513 にならない（§50 は効果にも当てる） | R32 |

**#8 は計画審査 1 巡目で再現しなかった**（`apply (with H -> raise ...) 3` は `7` / `3` を印字して終了 0。利用者効果の `reply` が関数を返す形は E706）。**再現手順を先に 1 本立ててから裁定せよ。立たなければ (c) にして、その旨を台帳に書く。**

**すでに解けているもの（c。確かめて、解けていることを台帳に書くだけ）:**

| # | 症状 | 解いたところ |
|---|---|---|
| 13 | ローカル束縛が prelude の操作名を影にすると昇格が崩れる | R37 工程 0（決定 4a） |
| 14 | サブディレクトリの中の `handler Error` が E204 | R37 工程 1（決定 4c） |
| 15 | 外の `def raise = 1` が E513（§57 は E205 と定める） | R37 工程 1（`namespaces.rs:156-162` の `check_prelude_operation_declarations`）。**計画審査 1 巡目の実測: `error[E205]: declaration conflicts with prelude operation`。E513 は出ない** |
| 16 | `type T = (Int, Bool)` がコード無しの診断 | **計画審査 1 巡目の実測: `error[E100]: expected a record or a leading \| for an ADT`（`parser.rs:1176-1179`）。コードは付いている。§37 の E105 は式とパターンの tuple で、型本体には当たらない** |

**E2 の空 argv の `MissingDirectory`（30 節の疑義）は (c) とする。** `tools/gates/` は利用者のコードなので §49 の失敗の表の外でよい（R34 の `"InvalidLine"` が先例）。**この判断が誤りだと思ったら、変えずに報告せよ。**

**R31 の台帳の未確認 3 件**（型引数付き効果の重ね、`fn` の本体の `capture`、handler の値引数）は本版の範囲外である。**再現するなら `docs/bug.md` に置く。しないなら何も足さない。**

### 判定

`scripts/verify-r38.sh` を新設する。構造は `scripts/verify-r37.sh` に倣う。

判定 38-a: `run-gates.sh`。(1) 門番を 2〜3 本だけ選んで走らせる引数を持ち、ログが `<名前>.log` の平坦な形になる。(2) 実行時に生成した**偽のログのディレクトリ**（`PASSED: n/N` を含む行を持つファイル群。緑のみ / 赤混在 / 空のログ / `PASSED:` を持たないログ / **`PASSED:` が複数本（最後の行だけが正）** / 1 本 / 多数）に対して、シェル側 oracle が正しい合計と赤の一覧を出す。**期待値は生成のときに控えた `n/N` から独立に計算する**（oracle の出力をそのまま期待値にしない）。**(3) `run-gates.sh` は全門番の全走を判定器の中でしない**（時間）。

判定 38-b: E2 と oracle の一致。**偽のログのディレクトリの分類**（門番の本数 1 / 2 / 多数 × 緑のみ / 赤のみ / 混在 × `PASSED:` 行が 0 本 / 1 本 / 複数本 × 名前が ASCII / 多バイト）の積を実行時に生成し、**E2 の出力・oracle の出力・生成時に控えた期待値の 3 つが一致する**ことを測る（一致検査だけにしない。計画審査 1 巡目の指摘: 2 者の一致だけだと、oracle が `head -1` に壊れたとき E2 を先頭行に合わせる直し方が通ってしまう）。数は乱数化する。**空のディレクトリ**と**空 argv**（`MissingDirectory`）も含める。**負の対照として、ログのディレクトリの中にサブディレクトリを 1 つ置いた形を 1 本入れ、`IoFailure` で終了 1 になることを測る**（決定 1 の「平坦に吐く」が E2 の入力の契約であることの固定）。

判定 38-c: 仕様の追記（**探針は工程 0 で先に置く**。工程 1 は `docs/spec.md` と `docs/bug.md` しか触れないので、`verify-r38.sh` への追加は工程 0 で済ませる。被覆は節番号（§45 / §47 / §49）で見る）。裁定 (a) で足した文ごとに、**その文が定めることを測る探針を 1 本**置く（例: 長さが違う Array の `compare` が `-1`、`Int.parse "007"` が `7`、`read_dir` の結果が整列している）。**探針は `docs/spec.md` の文言を写すのではなく、実装の振る舞いを測る。** 足した文と探針の対応が全被覆であることも測る。

判定 38-d: `docs/bug.md`。裁定 (b) で足した節ごとに、**節に書いた再現手順をそのまま実行して再現する**ことを測る。**再現しない節があれば落ちる。** 節が 0 件なら判定も 0 件でよい（その場合は「1 件も再現しなかった」ことを報告に書く）。

判定 38-e: 仕上げ（**工程 2 で緑にする**。工程 0 の終わりに `verify-r38.sh` 全体が緑になる必要は無い。工程 0 は 38-a と 38-b と 38-c の探針、工程 1 は 38-d、工程 2 は 38-e）。(1) `sha256sum --strict -c tests/corpus.sha256` が全件 OK。(2) `docs/spec.md` の版が `v0.3`。(3) `docs/ledger.md` に R38 の節がある。**(4) 全門番が満点であることは判定器の中では測らない**（時間。指揮者が全走で見る）。

### 受け入れ条件（R38 全体）

1. `bash scripts/verify-r38.sh` が満点・終了コード 0
2. **全門番が満点・終了コード 0**（指揮者が全走で測る）
3. `cargo test --offline --no-fail-fast` が 0 failed
4. `sha256sum --strict -c tests/corpus.sha256` が全件 OK
5. E2 とシェル側 oracle が、38-b の全分類で一致
6. 裁定 14 件が (a) / (b) / (c) のどれかに落ちており、根拠（再現の実測）が報告にある
7. `v0.3` の tag（**指揮者が打つ**）

### 中断の条件

- **仕様の追記が「1 文足す」で済まないと分かったら、止めて報告せよ。** 既存の文の書き換えが要るなら、それは v0.3 の範囲を超える
- **裁定の対象が再現しないときは、仕様にも bug.md にも書かず、報告に「再現しない」と書け。** 伝聞をそのまま文書にしない
- **門番を緩めて満点にしない。** 満点にならないなら、理由を報告せよ

### 触ってよい場所

工程 0: `scripts/run-gates.sh`（新設）、`scripts/verify-r38.sh`（新設）、`tools/gates/`（oracle と食い違ったときだけ。**食い違いは既定で E2 の側を直す**）。
工程 1: `docs/spec.md`（追記だけ）、`docs/bug.md`、`scripts/verify-r38.sh`（**判定 38-d の分だけ**。38-d は bug.md の節の再現を測るので、節が決まってからでないと書けない。38-a・38-b・38-c は工程 0 で済んでいるので触らない）。
工程 2: `docs/ledger.md`、`tests/corpus.sha256`。

**`crates/hado/src/` と `runtime/` と `stdlib/` は触らない。** 触る必要が出たら、それは実装の穴を直すことなので、止めて報告する。

### 確認して報告

- 裁定 14 件それぞれの再現の実測（コマンドと出力の末尾）と、(a) / (b) / (c) のどれにしたか
- E2 と oracle が食い違った分類があれば、その分類とどちらを直したか
- `run-gates.sh` の全走の所要（指揮者が測るので、builder は部分走の所要でよい）

### スコープ外

- 新しい言語機能、実装の穴の修正（bug.md に置くだけ）、v0.4 の設計

## 32. 門番の高速化（2026-09-14）

R38 の全走は 81 分、本節の実測でも 76.8 分かかった。原因は門番の入れ子である。m3 は m0・m1-spec・m1-corpus・m2 を呼び、m2 は m0・m1-spec・m1-corpus を呼び、m1-corpus は m0 を呼ぶ。r36 と r37 は m1-corpus・m7 を呼び、m7 は m6 を呼ぶ。同じ内容の `cargo test` が 1 回の全走で 8 回実走され、m3 の単一スレッド全走がさらに 6 分近くを占めていた。**判定を 1 文字も緩めずに、同じ内容の実走を 1 回に畳む。**

### 決定

- `scripts/lib/gate-cache.sh` を置く。`gate_cached <label> <command...>` は、作業ツリーの内容ハッシュ（`target/`・`.git/`・`__pycache__/` を除く全ファイル）と `label` + 引数のハッシュで結果を鍵付けする。**成功（終了コード 0）だけを保存し、失敗は保存しない。**
- 走行スコープは 1 回の走行に閉じる。`scripts/run-gates.sh` が `HADO_GATE_CACHE_RUN` を発行し、全走の終わりに消す。乱数で探針を作る門番を走行間で凍結せず、内容に現れない環境差（root / 非 root など）を結果に持ち越さないためである。単独で門番を走らせたときは、その門番の入れ子の範囲だけで畳む。
- 入れ子で門番を呼ぶ箇所を `gate_cached` に通す: m1-corpus → m0、m2 → m0/m1-spec/m1-corpus、m3 → m0/m1-spec/m1-corpus/m2、m7 → m6、r36 → m1-corpus/m7/m4/m5/m6、r37 → m1-corpus/m7。
- cargo test の呼び出しを `cargo_cached` に通す。全走の全体テストは `cargo test --offline --no-fail-fast` に統一し、m0 / m2 / m3 / m7 のどこで最初に走っても 1 回で済ませる。
- m3 の単一スレッド全走（`-- --nocapture --test-threads=1`）を、**並列の全走**（終了コードと HM のテスト名）と **lib の単体テスト `--nocapture --test-threads=1`**（`lower_extra_def_ng` の印字）に分ける。判定 8 と 12 が見る条件は変えない。
- `scripts/verify-gate-cache.sh` を新設し、キャッシュの 6 性質（同一内容の再利用、引数の分離、失敗を保存しない、内容への追随、走行スコープの分離、古い走行の掃除）を測る。
- `scripts/run-gates.sh` は `HADO_GATE_TIMES` を設定すると門番ごとの所要と終了コードを追記する（任意。通常の出力は変えない）。
- m3 も lib を読むので、単独で走らせても m3 の入れ子全体で 1 つの走行スコープを共有する。古い走行のスコープ（既定 1 日より古いもの）は、新しい走行が始まるときに消す。

### 判定

- 32-a. 全門番（38 本）が満点・終了コード 0。`cargo test --offline --no-fail-fast` が 0 failed。
- 32-b. 短縮前後の所要の実測。短縮前は 76.8 分（2026-09-14、37 本の直列全走）。
- 32-c. 各門番の `PASSED: n/N` と `NG:` の数が短縮前と同じ（判定条件を緩めていない）。
- 32-d. `verify-gate-cache.sh` が 6/6。
- 32-e. `scripts/run-gates.sh` が r38 の合成木（`scripts/lib/gate-cache.sh` の無い木）でも従来どおり動く。

### 触ってよい場所

`scripts/`、`README.md`、`docs/plan.md`、`docs/ledger.md`。`crates/`、`runtime/`、`stdlib/`、`tests/`、`tests/corpus.sha256` は触らない。

### スコープ外

門番の並列実行（`docs/ledger.md` の「門番を選んで直列に走らせ」は維持する）、`cargo run` の置き換え、`crates/hado/tests/` の分割。

## 33. 門番の層別と検査の棚卸し（2026-09-14）

32 節で全走は 76.8 分 → 16.7 分（1002 秒、38 本、赤 0）になった。残る時間の内訳は、本体のビルドと 734 テスト（m0 195 秒）、コーパスの実行（m1-corpus〜m7 で約 168 秒）、生成探針（r34 1360 件 133 秒、r35 714 件 135 秒、r9 77 秒、r36 77 秒、r37 38 秒などで約 580 秒）、固定費（約 60 秒）である。目的ごとに回す頻度を分け、検査そのものを見直す。

### 背景（32 節で分かっていること）

- キャッシュは走行 1 回に閉じる。同じ内容の実走は全走の中で 1 度に畳まれる。検査は 1 つも消していない。
- r36 は m1-corpus・m7・m4・m5・m6 を、r37 は m1-corpus・m7 を呼び直す。固有の検査は上乗せ分だけ。
- 生成探針（r18〜r35）は乱数サンプルで、1 回の緑は証明ではなく標本である。
- `docs/spec.md` の読み方は 2 種類ある。表として読む検査（m1-spec 検査3、m7、r34、r36、r23〜r26）と、文章を逐語で固定する検査（m1-spec 検査1・2・4・5、r38-c、r38-e）。
- 全走の実測は `docs/ledger.md` の「門番の高速化」にある（`target/gate-timing/` は gitignore 対象）。

### 決めること

1. 門番を目的で 3 層に分ける。層は「いつ回すか」だけを決め、判定は変えない。
2. ラウンド門番（r9〜r38）を 1 本ずつ棚卸しする。
3. spec.md の文章固定検査を棚卸しする。

### 層（案。builder は異議があれば報告に書く）

| 層 | いつ | 中身 | 目標 |
|---|---|---|---|
| quick | 保存ごと | `cargo build`、`verify-m1-spec`、`verify-gate-cache` | ≦ 2 分 |
| core | コミット前 | m0（ビルドと 734 テスト）、m1-corpus、m1-spec、m2、m3、m4、m5、m6、m7、r38 | ≦ 7 分 |
| full | 節目・夜間 | 38 本すべて | ≦ 18 分 |

- 実装は薄く。門番名の一覧は 1 か所に持ち、`scripts/run-gates.sh --tier quick|core|full` か薄いラッパで渡す。plan とコードで二重管理しない。
- 開発中は変更に関係する門番を core に足して回す。core の終わりに「full でのみ回した門番の一覧」を表示し、取り違えで赤を見逃さない。
- 32 節のキャッシュはそのまま使う。層はキャッシュの外側の話である。

### 実装（2026-09-15）

- 一覧の唯一の正は `scripts/lib/gate-tiers.sh`。`hado_tier_minimal_gates <層>` が各門番を
  最小の層に 1 回ずつ割り当て、`hado_tier_gates <層>` が累積（quick / quick+core / 全 38 本）を
  返す。上の表で m1-spec が quick と core の両方にあるのは、この累積の意味である。**層は分割、
  実行は累積**なので、3 層は全 38 本を重複なく覆う（33-f）。表の m1-spec を最小層の quick に
  置いたので、core は quick を含む。
- `scripts/run-gates.sh --tier quick|core|full` が層を回す。層に `verify-<名前>` を足すと
  その門番も回る（重複は落とす）。`--print-tier` は一覧を出すだけで、出力ディレクトリも
  走行スコープも作らない。層も明示も無い引数なしの全走は full と同じ扱いにし、従来の
  `run-gates.sh <出力先>` の呼び方でも文書層まで回る。
- quick は門番の前に `cargo build --offline` を 1 回実走する。ログの末尾は集計器
  （`tools/gates/main.hd`）に合わせて `PASSED: n/N` に揃える。
- core の終わりに、full でのみ回す門番の一覧を表示する。
- 文書層は `scripts/lint-docs.sh` として分離し、full の最後（と引数なしの全走）だけが回す。
  `verify-*.sh` ではないので 38 本には数えない。
- 部分 `cargo test` の実走は `cargo_test_cached` が畳む。畳むのは、同じ走行の全走
  （`cargo test --offline --no-fail-fast`）が成功し、そのログに「対象の `tests/<名前>.rs` の
  実行」と続く「`test result: ok`」の証拠があるときだけである。`--test <名前>` 以外の引数
  （`--` 以降、`-q`、`--lib`、`--features`、`--release` など）があれば実走する。証拠を要求する
  ので、対象の改名・削除や赤の全走で検出力は落ちない。

### ラウンド門番の棚卸し

- 門番 1 本ごとに、その回の決定がどこで守られているかを表にする（corpus のケース、`crates/hado/tests/`、spec の表、他の門番の検査）。
- 重複が確認できたものだけ統合・削除する。削除前後の検出力の差を報告に書く。判断がつかないものは残す。
- r36/r37 の再走部分は「全走で 1 回」に整理する。r36/r37 固有の検査は残す。

### 文章固定検査の棚卸し

- spec.md の検査を「表として読む（処理系との契約）」と「文章を逐語で固定する（文書管理）」に分類する。
- 後者は (i) 構造検査へ置換、(ii) `docs` 層（full のみ）へ移す、(iii) 削除、のいずれかにする。
- 削除・移動は、処理系の検出力を落とさない根拠を報告に書く。**門番を緩めて green にしない。** 削除は「赤を隠すため」ではなく「処理系を守らない検査だから」に限る。
- 文書の整合（spec↔plan↔spec-coverage）を守る必要があるなら、門番とは別の `scripts/lint-docs.sh` として分離し、core では回さない。

### 棚卸しの結果（2026-09-15）

- r9・r11〜r26・r28 の個別 `cargo test` は、m0 の全走（同じテストバイナリ）と重複していた。
  実走を `cargo_test_cached` に通して全走へ畳んだ。**検査は 1 つも消していない。**
- r23-b・r24-f・r25-b・r26-d の spec↔corpus の突き合わせは `verify-m1-corpus.sh` と
  `verify-m2.sh` が持つ。これも消さず、重複の記録だけ残す。
- r25-e（accept に `Ok(`/`Error(` が無い）は r9〜r28 で唯一の固有検査なので残した。
- r29 以降は固有（SIGPIPE/EPIPE/`/dev/full` の実プロセス、生成探針の広さ、r38 の probe・
  E2 oracle・bug.md の再現）。r36/r37 の再走はキャッシュで既に 1 回になっている。
- `verify-m1-spec.sh` は §50 のエラーコード表（処理系との契約）だけを残し、見出し・新設節・
  対応表・条件列の逐語は `scripts/lint-docs.sh` へ移した（5/5 → 1/1）。
- `verify-r38.sh` の 38-c は probe の key と §番号の対応・見出しの実在・否定対照（manifest
  不一致、見出し削除）に置き換え、逐語 regex は lint-docs へ移した。38-e は corpus lock を
  残し、版文字列と台帳節は lint-docs へ移した（140/140 → 138/138）。
- lint-docs の検査 7 本は、対応表（m1-spec 検査4）と 38-c の逐語 regex を元のまま持つ。
  版は spec と spec-coverage の一致、台帳は最新ラウンドの節の存在として一般化した。
  r38-c(6) の EnvDefault/IoDefault の E205 は probe の実走が r38 に残るため保護は落ちない。
- 分類の詳細と実測は `docs/ledger.md` の「門番の層別と検査の棚卸し」にある。

### 判定

- 33-a. 3 層それぞれが赤 0。実測が上の目標以内（マシン差は報告に書く）。
- 33-b. `full` の門番ごとの `PASSED: n/N` が 32 節の実測と一致する。検査を消した門番は N が減る理由を報告に書く。
- 33-c. 削除・移動した検査の一覧と、それぞれ「処理系の検出力を落とさない根拠」。削除前後で corpus の本数と `cargo test` の件数が変わらない。
- 33-d. `tests/corpus.sha256` を更新していない。`tests/`・`stdlib/`・`crates/`・`runtime/` を触っていない。
- 33-e. `verify-gate-cache.sh` が通る（6/6 のまま。層の自己判定を足すなら増えてよい）。
- 33-f. 層の門番の一覧が実在の門番と一致し、3 層が全 38 本を重複なく覆う（機械で検査する）。

### 触ってよい場所

`scripts/`、`README.md`、`docs/plan.md`、`docs/ledger.md`。`tests/`、`stdlib/`、`crates/`、`runtime/`、`tests/corpus.sha256` は触らない。

### スコープ外

言語機能、実装の穴の修正、門番の判定条件を緩める変更（検査の追加は可）、`cargo run` のバイナリ置換（別ラウンド）。

### 報告

(1) 層ごとの実測（コマンドと出力の末尾）、(2) 赤の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、削除・移動した検査と根拠、迷った点。

## 34. 門番の並列実行（2026-09-15）

33 節の直列 full は 13 分 39 秒、門番ごとの秒の総和は 826 秒で wall とほぼ等しい。シェルが 1 本ずつ待っているだけで、マシンは 12 コアある。**判定を 1 つも緩めずに、門番を N 並列で走らせる `--jobs N` を足す。既定は 1（直列）のまま残す。**

### 背景（実測 2026-09-15）

- 直列 full の内訳（`target/tier/full-times.tsv`。秒）: m0 192、r35 141、r34 138、r36 79、m2 52、m1-corpus 48、r37 39、m3 32、r38 20、r25 16、r33 14、m7 13、残りは 9 秒以下。合計 826。
- 作業ツリーの内容ハッシュは 370 ファイルで 0.053 秒（`time (source scripts/lib/gate-cache.sh; hado_gate_fingerprint)`）。キャッシュの鍵計算は並列でも軽い。
- `cargo` の target ロック（`target/debug/.cargo-lock`）は実在する。このロックを外から 2 秒握ると `cargo run -- check` は 1.87 秒待つ。ただしビルドが最新のときの保持は短く、`cargo run -- check tests/accept/abort_arm_value/main.hd` 8 本は直列 0.754 秒・並列 0.167 秒（4.5 倍）で、ロックで直列化しない。
- 期限付きの探針は余裕が大きい。r34 の `timeout 2` は実測 110ms・122ms、r35 の `timeout 2` は 0.13 秒・0.15 秒（いずれも 33 節の full ログ `target/tier/full/verify-r34.log`・`verify-r35.log`）。並列負荷で 13〜18 倍遅くならない限り赤にならない。
- **入れ子の label が 3 通りに割れている。** `run-gates.sh:183` は `gate_cached "verify-$name"` を呼び、`$name` は既に `verify-` 付きなので label は `verify-verify-m0` になる。入れ子は `verify-m0`（`verify-m1-corpus.sh:369`・`verify-m3.sh:146`・`verify-r36.sh:387`・`verify-r37.sh:568`）と `verify-m0.sh`（`verify-m2.sh:220`）を使う。id は実測で `39be3685`・`0970eaef`・`cdceb8f3` と別。このため同じ門番が 1 回の走行で複数回実走する（m0 は 3 通り、m1-corpus は run-gates 直下と r36 の 2 回。33 節の full ログでも `already covered` が 18 本のログに 23 行ある）。
- **共有ツリーを改変する門番がある。** `verify-m1-corpus.sh:266-271` が `tests/accept/__probe__` を作り、`verify-m2.sh:50-53`・`81` が `tests/accept/__probe_bad__` と `tests/reject/E103` を入れ替え、`verify-m3.sh:61-77` が reject コーパスを入れ替え・行ずらしし、`verify-gate-cache.sh:97` がリポジトリ直下に `.gate-cache-selftest-probe` を作る。鍵は作業ツリー全体のハッシュなので、これらが並列に走ると他門番のキャッシュが別鍵になり、`tests/` を glob する門番（m3・m4・m5・m6・r34〜r37）と `tests/corpus.sha256` を照合する門番が偽の赤を出し得る。

### 決定

1. **`--jobs N`（既定 1、正の整数）。** 1 のときは 33 節の直列ループをそのまま実行する。2 以上で前段（直列）＋後段（fan-out）に分ける。`--jobs=N` も受ける。0・負・非整数は usage を stderr に出して exit 1（出力ディレクトリを作らない）。`--print-tier` は今までどおり一覧だけを出す。
2. **入れ子の label を `verify-<名>` に揃える。** `run-gates.sh` は `gate_cached "$name"`（`$name` は `verify-` 付き）、`verify-m2.sh:220` は `gate_cached "${script%.sh}"` に直す。これで同一門番の実走は走行内で 1 回になる。**既定の直列でも入れ子の再利用が増えて wall は縮む**（r36 が抱える m1-corpus の再走 48 秒など）。ログの内容と `PASSED: n/N` はキャッシュの再生なので一致する。**34-a の「変わらない」は stdout・stderr・ログの内容に限る**（実走の回数と wall は減る）。この差を報告に書く。
3. **前段（直列）に共有ツリーを改変する門番を置く。** 一覧は `scripts/lib/gate-tiers.sh` の `hado_tier_serial_gates` が唯一の正で、`gate-cache m0 m1-corpus m2 m3` を返す。run-gates は「直列の一覧 ∩ 走行する門番」を一覧の順に完走させ、残りをファンアウトする。一覧の無い木（判定器の合成木）では `verify-m0` だけを前段に置く。quick の `cargo build --offline` は今までどおり前段で実走する。
4. **後段は N 並列。** 各ワーカーは `gate_cached` 経由で `bash scripts/<門番>.sh` を走らせ、`$output/<門番名>.log` に書く。終了コードと所要は `mktemp -d` の作業ディレクトリへ門番ごとの 1 ファイルとして残す（出力ディレクトリには `.log` 以外を置かない。r38 の平坦なログの契約を守る）。親は全部を待ってから集約し、1 本でも赤なら exit 1。ワーカーの status が無い（OOM kill など）門番は赤として扱う。`HADO_GATE_TIMES` は親が一覧の順に 1 回だけ書く。親の trap はワーカーを**子孫ごと** kill してから走行スコープと作業ディレクトリを消す（kill は親の subshell にしか当たらず、flock を握る subshell と門番本体が孤児で残るため）。`lint-docs` は今までどおり最後に直列で回す。
5. **木の安定性を前段・後段の境界で確かめる。** ファンアウトの前後で内容ハッシュを取り、違えば「改変する門番の分類漏れ」として stderr に報告し exit 1 にする（フェイルクローズド）。**走行自身がログを書く出力先はハッシュから外す**（門番の入力ではないので、出力先がツリーの中でも誤爆させない）。**前段に赤があれば後段を直列へ落とす**（赤い門番が probe を戻し切らずに終わり得る。その状態で並列にすると鍵が揺れて入れ子が再走する。赤は既に確定しているので判定は緩まない）。このとき安定性 guard は回さない。
6. **キャッシュの並行安全化。** `gate_cached` は (a) `status` が 0 の高速路、(b) `$dir/lock` を `flock` で取ってから再確認、(c) 実走、(d) 成功なら `out` を temp → rename、`status` も temp → rename、の順にする。`label` と `argv` の記録も lock の中に置く。実走するコマンドへは lock の fd を継がせない（`9>&-`）。実走の直前に任意の記録先へ 1 行追記する（決定 8）。**同じ id を保持したまま再入したら flock で待たずに赤で止める**（保持 id を子へ継がせ、循環を永久待ちにしない）。`flock` が無い環境では (b) を省く（直列は安全）。`gate-cache.sh` がある木で `--jobs>1` を指定して `flock` が無ければ明示的に落ちる。合成木のように `gate-cache.sh` が無い（`gate_cached` が素通しの）木では並列はそのまま動く。
7. **full-test ログも temp → rename。** `cargo_cached` の全走は成功したときだけ `full-test-<内容>.log` へ rename で公開する。赤い全走のログは公開しない。既に公開済みなら再公開しない。`cargo_test_cached` は公開済みの完全なログだけを読む。
8. **`HADO_GATE_RUNS`（任意の診断。既定は未設定で何も書かない）。** 設定すると `gate_cached` が実走と再利用を label 付きで 1 行ずつ追記する。34-b・34-g の「1 回だけ実走」を機械で数える根拠にする。入れ子の run-gates には `HADO_GATE_TIMES` と同じく継がせない。
9. **新しい門番は足さない。** 並行性は `scripts/lib/gate-cache.sh`・`scripts/run-gates.sh`・`scripts/lib/gate-tiers.sh` の性質なので、既存の `scripts/verify-gate-cache.sh` を拡張する。門番は 38 本のまま。別門番にすると合成木の足場が二重になり、直列 full がその分だけ延びる。
10. **探針系も後段に残す。** 背景のとおり期限の余裕が大きく、ロックも実効の妨げにならない。実測で r34/r35 が並列負荷で赤になれば報告し、そのときは直列フェーズへ戻す判断を書く（今は戻さない）。

### 判定

- 34-a. 既定（`--jobs` なし）の出力が 33 節と 1 文字も変わらない。合成木で 33 節版と 34 節版の `run-gates.sh` の stdout・stderr・ログ・trace を byte 比較する。`scripts/verify-r38.sh` の 38-a（合成木の直列実行・平坦なログ・stderr が空）が緑のまま。**label 統一で直列の wall は縮むが、出力と `PASSED` は不変**であることを報告に書く。
- 34-b. 直列 full と並列 full（`--jobs 2`・`4`・`8`）で、38 本の `PASSED: n/N` がすべて一致し、`^NG:` が 0、終了コード 0。
- 34-c. `cargo test --offline --no-fail-fast` が 734 passed / 0 failed のまま（m0 の `cargo-test passed=734`）。
- 34-d. corpus の本数が変わらない（accept のファイル数・reject のコード数・stdlib のファイル数を `tests/corpus.sha256` と仕様の表から導いて直列・並列で比較）。`tests/corpus.sha256` を更新していない。`tests/`・`stdlib/`・`crates/`・`runtime/` の差分が空。
- 34-e. 並列で赤を隠さない。合成木に赤を 1 本混ぜた `--jobs 4` が exit 1 で、赤の門番のログと終了コードが残る。
- 34-f. `verify-gate-cache.sh` の自己判定が並行性を測る: 同一コマンドの並行呼び出しが 1 回だけ実走、キャッシュの公開が原子的（out → status の順、部分と赤を公開しない）、直列の一覧 ∩ 走行の門番が fan-out より先に完走、後段の同時実行が `--jobs` を超えない、出力先が平坦、前段に赤があると後段が直列へ落ちる、`HADO_GATE_TIMES` が門番ごとに 1 行（`env -u HADO_GATE_RUN_ACTIVE -u HADO_GATE_RUNS` で測る）、`--jobs` の不正値の拒否、`--jobs 2` と直列の `PASSED` が一致。増えた分だけ N が増える（33-e が許す。実装後は 26）。
- 34-g. 実走回数。`HADO_GATE_RUNS` を付けた並列 full で、門番の label ごとの実走が 1 回であること（入れ子の再走が無いこと）。直列 full の同じ記録と比べる。
- 34-h. 木の安定性 guard が並列 full で発火しない（前段・後段の境界で内容ハッシュが一致）。後段にツリーを恒久的に変える門番を混ぜた対照で guard が発火し、出力先がツリーの中にある対照では発火しないことも測る。
- 34-i. 速度の実測。直列と `--jobs 2/4/8` の wall・CPU（user+sys）・門番ごとの秒を表にする。理論下限は m0 192 秒＋直列の改変門番 133 秒＋後段の最長 r35 141 秒 ≒ 466 秒。効かなかった場合は原因を書く。**実測（2026-09-15、台帳「門番の並列実行」）: 直列 761.39 秒、`--jobs 2` 553.55・`4` 472.44・`8` 466.73 秒（1.63 倍）。`--jobs 8` は下限にほぼ一致した。**

### 触ってよい場所

`scripts/`（`run-gates.sh`、`lib/gate-cache.sh`、`lib/gate-tiers.sh`、`verify-gate-cache.sh` が主）、`README.md`、`docs/plan.md`、`docs/ledger.md`。`tests/`、`stdlib/`、`crates/`、`runtime/`、`tests/corpus.sha256` は触らない。

### スコープ外

門番の判定条件を緩める変更、門番の削除、`crates/`・`runtime/`・`stdlib/` の変更、r38 の「門番を選んで直列に走らせ」の検査の置き換え、`cargo run` のバイナリ置換、`--jobs` の既定を 1 以外にすること、m1-corpus・m2・m3 の probe を一時ディレクトリへ移す改修（今ラウンドは直列化で閉じ、改修は別ラウンド）。

### 報告

(1) 34-a〜34-i の実測（コマンドと出力の末尾）、(2) 赤の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

## 35. R39 仕様書の再構成の手順書（2026-09-15）

### 背景

`docs/spec.md` は v0.1 以来、節を末尾に足す形で育った。文法（§41）の後ろに原則（§42）と字句（§43〜§46）が続き、型（§36・§37）が module（§34・§35）の後ろにあり、引数の分岐形（§52・§53）がエラーコード表（§50）と印字（§51）の後ろにある。内容のまとまりと物理順がずれており、単体の言語仕様書として読めない。

`§N` は見出し番号であると同時に、コーパスの `// spec: §N`、§50 のエラーコード表の「節」列、`docs/spec-coverage.md`、`docs/spec-trace.md`、門番、Rust テストが共有する識別子である。並びを直すには番号を振り直し、これらの参照を追随させるしかない。

### 決定

1. **節番号は 1..59 の十進整数のまま**にする。`§N` の字句を変えないので、コーパスと門番の `§[0-9]+` がそのまま使える。
2. **文書の構造**は、タイトル `# hado 言語仕様 v0.3`、部 `# 第X部 …`、節 `## N. …` の 3 層にする。節は部の中に読み順で 1..59 と並ぶ。節の下に小節を置くときは `### N.M` とする（今回の再構成では小節を割らない）。
3. **節の数は 59 のまま**（統合・分割はしない）。本文は節単位で丸ごと移し、節内の `第k節`・`§k` の参照だけを写像で置き換える。移動と番号替えだけであることを機械で照合できる。
4. 旧番号から新番号への写像と、各節がどこへ移ったかは `docs/spec-map.md` に置く。`docs/plan.md`・`docs/ledger.md` の §N は決定と実行の記録なので書き換えない。それらを読むときは `docs/spec-map.md` を通す。
5. 版は v0.3 のままとし、tag は打たない。言語規則は変えず、文書の並びだけを変える。

### 手順

1. `docs/spec.md` を再構成する（前書き・目次・部・節の並び・節内参照の書き換え）。
2. `docs/spec-map.md` を作る（旧→新の写像と、そこから読むための注意）。
3. `docs/spec-coverage.md`・`docs/spec-trace.md`・`README.md`・`docs/bug.md`・`docs/wants.md` の §参照を写像で置き換える。
4. `tests/accept`・`tests/reject`・`tests/library` の `// spec: §N` を写像で置き換え、`tests/corpus.sha256` の該当行のハッシュを更新する。コーパスのファイル名・本数・`// error:`・`// item:` は変えない。AGENTS.md の「実装の工程で lock を更新しない」の例外を本項が与える。
5. `docs/spec.md` の見出しと節本文を読む門番と Rust テストを追随させる: `scripts/lint-docs.sh`、`scripts/verify-m1-spec.sh`、`scripts/verify-m1-corpus.sh`、`scripts/verify-m3.sh`、`scripts/verify-m7.sh`、`scripts/verify-r34.sh`、`scripts/verify-r36.sh`、`scripts/verify-r38.sh`、`crates/hado/tests/r9_grammar.rs`、`crates/hado/tests/r23_syntax.rs`。
6. `scripts/verify-r39.sh` を新設し、`scripts/lib/gate-tiers.sh` の full 層に `r39` を足す。`scripts/verify-gate-cache.sh` の自己判定が「層の一覧 == 実在の門番」を照合するので追随は必須である。
7. `docs/corpus.md` の門番の説明を直す。
8. `docs/ledger.md` に R39 の実測を書く。

### 判定

- 39-a. `scripts/verify-r39.sh` が満点。内容は (1) `docs/spec.md` が `## 1.`〜`## 59.` を 1 回ずつ読み順に持ち、各部の下にある、(2) `docs/spec-map.md` の写像が旧 1..59 と新 1..59 の全単射である、(3) tag `v0.3` の各節の本文（節内参照を写像で置き換えたもの）が、新しい対応する節の本文に部分文字列として現れる、(4) コーパス・`docs/spec-coverage.md`・`docs/spec-trace.md` の §参照が全て実在の節を指し、拒否コーパスのタグは §57 の表の節と一致する。
- 39-b. `scripts/lint-docs.sh` が満点。見出しの検査は「§1..§59 が `## N.` として 1 回ずつ読み順に存在する」に置き換える。`git diff` による見出しの削除の検査は番号の振り直しと両立しないので外す。内容の欠落は 39-a(3) が測る。
- 39-c. `scripts/verify-m1-spec.sh`・`scripts/verify-m1-corpus.sh`・`scripts/verify-m3.sh`・`scripts/verify-m7.sh` が満点。
- 39-d. `scripts/run-gates.sh --tier core <出力先>` と `--tier full <出力先>` が exit 0 で、`^NG:` が 0。
- 39-e. `cargo test --offline --no-fail-fast` が 0 failed。
- 39-f. `sha256sum --strict --status -c tests/corpus.sha256` が exit 0。コーパスのパス集合と本数は再構成の前後で変わらない。
- 39-g. 言語規則を変えていない。§57 のエラーコード表の 69 行は、コード・短い名・段階が同じで、節の列が写像の像、アンカー欄が節参照の置換の像である。`stdlib/`・`runtime/` の差分は空、`crates/` の差分は節番号の参照（コメントとテストが見る見出し文字列）の追随だけである。

### 触ってよい場所

`docs/spec.md`、`docs/spec-map.md`（新設）、`docs/spec-coverage.md`、`docs/spec-trace.md`、`docs/corpus.md`、`docs/plan.md`、`docs/ledger.md`、`README.md`、`docs/bug.md`、`docs/wants.md`、`tests/accept`・`tests/reject`・`tests/library` の `// spec:` 行、`tests/corpus.sha256`、`scripts/`（`verify-r39.sh`（新設）、`lib/gate-tiers.sh`、`lint-docs.sh`、`verify-m1-spec.sh`、`verify-m1-corpus.sh`、`verify-m3.sh`、`verify-m7.sh`、`verify-r34.sh`、`verify-r36.sh`、`verify-r38.sh`）、`crates/hado/tests/r9_grammar.rs`、`crates/hado/tests/r23_syntax.rs`、`crates/` と `tools/` の節番号のコメント。

### スコープ外

節の統合・分割、本文の命題の追加・削除・書き換え（重複した事実の統合は別ラウンド）、エラーコードの追加・変更、処理系の振る舞いの変更（`crates/` は節番号のコメントだけを変える）、`runtime/`・`stdlib/` の変更、`docs/plan.md`・`docs/ledger.md` の §N の書き換え、`docs/spec.md` の版上げと tag。

### 報告

(1) 39-a〜39-g の実測（コマンドと出力の末尾）、(2) 赤の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

## 36. R40 文法の概形を付録へ移す手順書（2026-09-15）

### 背景

R39 は `docs/spec.md` を部と節に再構成し、文法の概形（旧 §41）を §5 として第II部 字句と文法の先頭に置いた。言語仕様を読む開発者が最初に文法の全体を読むことは少なく、字句の部の入口を文法の一覧が占めるのは読み順として重い。文法の概形は参照用の付録に置くのが素直である。節を 1 つ付録へ移すので、残る節の番号を繰り上げる必要がある。

### 決定

1. **「文法の概形」を付録の最後（§59）へ移す。** 付録は コア構文（§57）・典型コード（§58）・文法の概形（§59）の順になる。
2. **写像は、§1〜§4 はそのまま、§5（文法の概形）は §59、§6〜§57 は 1 つ繰り上げて §5〜§56、§58 は §57、§59 は §58。** 節の数は 59 のまま。本文は節単位で丸ごと移し、節内の `第k節`・`§k` だけを写像で置き換える。
3. **`docs/spec-map.md` は v0.3 の番号・R39 の番号・現行の番号の 3 列にする。** `docs/plan.md` 35 節と `docs/ledger.md` の R39 は R39 の番号を使うので、そこからも引けるようにする。
4. **版を v0.3.1 に上げ、tag `v0.3.1` を打つ。** `v0.3` は動かさない。言語規則は変えず、文書の並びだけを変えるので、v0.3.1 は並びの版である。tag は `v0.1.1` などと同じ軽量 tag にする。`scripts/verify-r40.sh` の本文保存の検査は `v0.3` を基準にしたままにする。

### 手順

1. `docs/spec.md` を再構成する（文法の概形を付録の最後へ、残りを 1 つ繰り上げ、前書きの版と目次を直し、節内参照を写像で置換）。
2. `docs/spec-map.md` を 3 列に作り直す。
3. `docs/spec-coverage.md`・`docs/spec-trace.md`・`docs/corpus.md`・`README.md`・`docs/bug.md`・`docs/wants.md` の §参照を写像で置き換える。
4. `tests/accept`・`tests/reject`・`tests/library` の `// spec: §N` を写像で置き換え、`tests/corpus.sha256` の該当行のハッシュを更新する。ファイル名・本数・`// error:`・`// item:` は変えない。AGENTS.md の「実装の工程で lock を更新しない」の例外を本項が与える。
5. 門番を追随させる。`lint-docs.sh` の見出しの範囲は §1〜§59 のまま（本数は変わらない）で、R38 の逐語固定の節は 22→21・50→49・56→55。`verify-m1-spec.sh` は §57→§56。`verify-m1-corpus.sh` は §59 の例→§58、prelude の §56→§55。`verify-m3.sh`・`verify-m7.sh`・`verify-r34.sh`・`verify-r36.sh` は prelude の節番号。`verify-r23.sh`・`verify-r24.sh`・`verify-r25.sh`・`verify-r26.sh` は表示のラベル。`verify-r39.sh` を `verify-r40.sh` に置き換え、`scripts/lib/gate-tiers.sh` の full の `r39` を `r40` にする。
6. Rust テストを追随させる。`r9_grammar.rs` と `r23_syntax.rs` は文法が付録の最後になったので、文法の節の終端を EOF にする。`r23_syntax.rs` の効果と handler の例は 38→37・39→38・40→39。`crates/` と `tools/` の節番号のコメントも追随させる。
7. `docs/ledger.md` に R40 の実測を書く。
8. `scripts/run-gates.sh` の quick・core・full と `cargo test --offline --no-fail-fast` を回す。
9. tag `v0.3.1` を打つ。

### 判定

- 40-a. `scripts/verify-r40.sh` が満点。内容は (1) `docs/spec.md` が `## 1.`〜`## 59.` を 1 回ずつ読み順に持ち、各部の下にある、(2) `docs/spec-map.md` の v0.3→現行と R39→現行の写像がどちらも全単射で、合成が一致する、(3) tag `v0.3` の各節の本文（v0.3→現行の写像で節内参照を置き換えたもの）が、対応する節の本文に部分文字列として現れる、(4) コーパスのタグが §56 の表の節と一致し、参照が全て実在の節を指す、(5) 現行の文書の §参照が全て実在の節を指す。
- 40-b. `scripts/lint-docs.sh` が満点。
- 40-c. 変更した門番（`verify-m1-spec.sh`・`verify-m1-corpus.sh`・`verify-m2.sh`・`verify-m3.sh`・`verify-m7.sh`・`verify-r34.sh`・`verify-r36.sh`・`verify-r37.sh`・`verify-r38.sh`）が満点。
- 40-d. `scripts/run-gates.sh --tier quick`・`--tier core`・`--tier full` が exit 0 で、`^NG:` が 0。
- 40-e. `cargo test --offline --no-fail-fast` が 0 failed。
- 40-f. `sha256sum --strict --status -c tests/corpus.sha256` が exit 0。コーパスのパス集合と本数は再構成の前後で変わらない。
- 40-g. 言語規則を変えていない。§56 のエラーコード表の 69 行は、コード・短い名・段階が同じで、節の列が写像の像、アンカー欄が節参照の置換の像である。`stdlib/`・`runtime/` の差分は空。
- 40-h. `docs/spec.md` と `docs/spec-coverage.md` の版が v0.3.1 で、tag `v0.3.1` が最終コミットを指す。tag `v0.3` は動いていない。

### 触ってよい場所

`docs/spec.md`、`docs/spec-map.md`、`docs/spec-coverage.md`、`docs/spec-trace.md`、`docs/corpus.md`、`docs/plan.md`、`docs/ledger.md`、`README.md`、`docs/bug.md`、`docs/wants.md`、`tests/accept`・`tests/reject`・`tests/library` の `// spec:` 行、`tests/corpus.sha256`、`scripts/`（`verify-r40.sh`（新設、`verify-r39.sh` を置き換える）、`lib/gate-tiers.sh`、`lint-docs.sh`、`verify-m1-spec.sh`、`verify-m1-corpus.sh`、`verify-m2.sh`、`verify-m3.sh`、`verify-m7.sh`、`verify-r23.sh`〜`verify-r26.sh`、`verify-r32.sh`、`verify-r34.sh`、`verify-r36.sh`、`verify-r37.sh`、`verify-r38.sh`）、`crates/hado/tests/`、`crates/` と `tools/` の節番号のコメント。

### スコープ外

節の統合（文法の概形とコア構文の重複など）・分割、本文の命題の追加・削除・書き換え、エラーコードの追加・変更、処理系の振る舞いの変更（`crates/` は節番号の参照だけを変える）、`runtime/`・`stdlib/` の変更、`docs/plan.md`・`docs/ledger.md` の §N の書き換え、tag `v0.3` の移動。

### 報告

(1) 40-a〜40-h の実測（コマンドと出力の末尾）、(2) 赤の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

## 37. R41 旧仕様の記載を外す手順書（2026-09-15）

### 背景

R40 は節番号を振り直した後も、v0.3 と R39 の番号を現行に写す `docs/spec-map.md` と、tag `v0.3` の本文との照合を門番に残した。文書には最新の snapshot だけを置けばよいという判断になった。過去の版は git tag が持つので、対応表と保存の検査はリポジトリの記載から外す。

### 決定

1. **`docs/spec-map.md` を削除する。** 旧番号を現行に写す表は持たない。
2. **`docs/spec.md` の前書きから旧 tag・旧番号の説明を外す。** 版と現行 tag の対応だけを残す。
3. **門番の保存の検査を落とす。** `scripts/verify-r40.sh` の 3 列の写像と tag `v0.3` との本文照合を外し、`scripts/verify-r41.sh` にする。以後は最新の仕様の自己整合（節の並び・コーパスのタグ・参照・版）だけを見る。
4. **`docs/plan.md` 35・36 節と `docs/ledger.md` の R39・R40 の記録は当時のまま残す。** 当時の判断が何を根拠にしたかの記録であり、参照する旧番号はその時点のものである。
5. **版は v0.3.1 のままとし、tag `v0.3.1` を最終コミットに付け直す。** 仕様の本文は変えないが、前書きが変わるので tag を現行の snapshot に合わせる。

### 手順

1. `docs/spec-map.md` を消す。
2. `docs/spec.md` の前書きを直す（旧 tag・旧番号・`docs/spec-map.md` の言及を外す）。
3. `scripts/verify-r40.sh` を `scripts/verify-r41.sh` にし、写像と保存の検査を落として構造・最新のみ・タグ・参照・版の検査にする。
4. `scripts/lib/gate-tiers.sh` の full の `r40` を `r41` にする。
5. `docs/ledger.md` に R41 を書く。
6. `scripts/run-gates.sh` の quick・core・full と `cargo test --offline --no-fail-fast` を回す。
7. tag `v0.3.1` を最終コミットに付け直す。

### 判定

- 41-a. `scripts/verify-r41.sh` が満点。内容は (1) `docs/spec.md` が `## 1.`〜`## 59.` を 1 回ずつ読み順に持ち、各部の下にあり、文法の概形が §59 である、(2) `docs/spec-map.md` が無く、`docs/spec.md` の前書きに v0.3.1 以外の版と `spec-map` の言及が無い、(3) コーパスのタグが §56 の表の節と一致し、参照が全て実在の節を指す、(4) 現行の文書の §参照が全て実在の節を指す、(5) `docs/spec.md` と `docs/spec-coverage.md` の版が v0.3.1 である。
- 41-b. `scripts/lint-docs.sh` が満点。
- 41-c. `scripts/verify-gate-cache.sh` が満点（層の一覧と実際の門番が一致する）。
- 41-d. `scripts/run-gates.sh --tier quick`・`--tier core`・`--tier full` が exit 0 で、`^NG:` が 0。
- 41-e. `cargo test --offline --no-fail-fast` が 0 failed。
- 41-f. `docs/spec-map.md` が存在せず、リポジトリの門番と現行文書がそれを参照しない。
- 41-g. tag `v0.3.1` が最終コミットを指す。

### 触ってよい場所

`docs/spec.md`、`docs/plan.md`、`docs/ledger.md`、`docs/spec-map.md`（削除）、`scripts/verify-r41.sh`（新設、`verify-r40.sh` を置き換える）、`scripts/lib/gate-tiers.sh`。

### スコープ外

言語規則の変更、`docs/plan.md` と `docs/ledger.md` の当時の番号の書き換え、コーパスの変更、tag `v0.3` の移動。

### 報告

(1) 41-a〜41-g の実測（コマンドと出力の末尾）、(2) 赤の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。
