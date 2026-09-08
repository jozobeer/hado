# 開発計画 v0.1 → v0.1.1

対象は `docs/0.1.1/spec.md`（以下「仕様」）。仕様の節番号は、M1 で改訂する前の現行版を指す。

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
docs/<版>/            spec.md、plan.md
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

1. 2.2 と 2.3 の内容を `docs/0.1.1/spec.md` に書き込む。既存の節番号は変えず、新設の節は §42 の後に追加する。コーパスの `-- spec:` はこの番号を指す。

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

M3〜M7 の成果物を実測して見つかった未決事項を、grilling 形式で確定した。仕様 `docs/0.1.1/spec.md` はこの表を反映済み。経緯は `docs/0.1.1/ledger.md` の R6。2.2 と 2.3 の表と食い違う箇所は本節が優先する。

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

リポジトリ直下の `PLAN.md` として実行エージェントに渡した手順書。0.1 の完了時に本節へ移した（`PLAN.md` は廃止し、以降は `plan.<版>.md` に統合する）。

この計画を実行するエージェント向けの手順書。R6（2026-09-07）の決定（`docs/0.1.1/plan.md` 7 節）を実行する。手順 0 から R9 まで、下の表の順に止まらずに進める。

**読む順:** 本ファイル → `docs/0.1.1/corpus.md` → `docs/0.1.1/spec.md`（§50 §41 §51 §52 を先に）→ `docs/0.1.1/plan.md` 7 節。2.2 と 2.3 の表と食い違えば 7 節が、7 節と仕様本文が食い違えば仕様本文が正で、食い違いは報告に書く。
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

`docs/0.1.1/spec.md` は R7（2026-09-07）で改訂され、まだ審査を受けていない。観点: §41 の EBNF から §39 の例と受理コーパスの全構文が導出できるか、§50 の 56 行に段階とアンカーが全て書かれているか、`docs/0.1.1/spec-trace.md` の各行が実在する節を指すか、7 節の決定が本文に漏れなく反映されているか。

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
| `short_and.shin` | `-- item: 末尾呼び出し` を `tail_self.shin` へ移す | `docs/0.1.1/corpus.md` |

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
- 8-b. 全件の `-- spec: §N` が実在する見出しを指し、`-- item:` が `docs/0.1.1/spec-coverage.md` と文字列一致。
- 8-c. 別の判定者が全 83 本を読み、「1 ファイル 1 欠陥」（§50 の段階順とアンカー位置順で判定）と「タグを寄せていない」を確認する。
- 8-d. R8 終了時に赤になる門番を報告に列挙する（見込み: `verify-m1-corpus` の 5 と 7、`verify-m2` と `parse_corpus.rs` の件数、`verify-m3` の accept-20、`verify-m4`〜`m7` の新形式と新コード）。


### R9 実装

順序に依存あり。各項目の判定器は「入力を実行時に生成し、期待値を判定器側で計算する」（`docs/0.1.1/corpus.md`）。`docs/0.1.1/spec-trace.md` の「固定するもの」列が、各項目の判定器の名前を参照する。

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
- 9-e. `docs/0.1.1/spec-trace.md` の「固定するもの」に書かれた判定器が全て実在する。


### スコープ外

`docs/0.1.1/plan.md` 6 節「v0.2 へ保留」の項目。warning の導入（緩和は「規則から外す」で行う）。

### 分からなかったこと

- E307 の判定（`infer.rs:405-415`）が注釈付きの多相再帰をどこまで捕まえるかは未検証。`def` に型変数を書く構文が無く、プローブが書けなかった。§4 の定義はこの判定を写している。
- prelude が全検査を通ることは、全プログラムへの連結経由で間接的にしか確かめられていない。R9-3 で免除を E513 に閉じたあと、判定者が免除の範囲を見る。


## 9. v0.1.1 の範囲と決定（2026-09-08）

`docs/wants.md` の 6 項目のうち 4 項目（言語名の変更、`//` コメント、List の `[x, ...xs]`、引数の条件列の短縮形）を 0.1.1 とする。grilling（14 問）で確定し、経緯は `docs/0.1.1/ledger.md` の「0.1.1 の計画」にある。仕様 `docs/0.1.1/spec.md` は本節を反映済み。2.1〜2.3 と 7 節と食い違う箇所は本節が優先する。

判断基準は 7 節と同じ。拒否か許容かで迷ったら拒否し、新機能は最小形で入れる。受理を広げる変更は後から安全に足せるので、後回しにした拡張は 9.3 に置く。

### 9.1 決定

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | 0.1.1 の位置づけ | v0.1 を tag `v0.1` で凍結し、wants の一部を 0.1.1 とする | — |
| Q2 | 範囲 | 言語名、コメント、List 構文、引数の条件列。ファイル名 module と `let value? =` は次の版 | — |
| Q3 | 改名の位置 | 最初のラウンド（R10）。以降の仕様・コーパス・コミットを新しい語彙で書くため | — |
| Q4 | 改名の範囲 | 過去の記録（`ledger.md` と本文書の 1〜8 節、git 履歴）以外の全て。CLI `hado`、拡張子 `.hd`、crate `crates/hado`、ランタイム記号 `hado_entry` `hado_alloc` | — |
| Q5 | 文書の版 | tag の後、6 文書を `git mv` で `docs/0.1.1/` に移して直接編集する。0.1 の姿は tag が持つ | — |
| Q6 | 手順書の置き場 | `PLAN.md` を廃止し `docs/<版>/plan.md` に統合する。`AGENTS.md` と `README.md` は進行中の開発に依存しない | — |
| Q7 | 再編の時期 | tag の前。tag が「0.1 完了で文書も整合した状態」を指すため | — |
| Q8 | コメント | `//` に置き換える。`--` と `#` は E104 | §41, §43 |
| Q9 | List の書き方 | 式もパターンも `[]` `[x, ...xs]` が唯一の形。式の `Nil` `Cons` は E510（`list-literal-form` に改名して範囲を広げる）、パターンは E516。prelude も書き直す | §46, §47, §50 |
| Q10 | 展開の位置 | 末尾のみ。中間展開は 9.3 | §41, §46 |
| Q11 | 条件列の短縮形 | `def classify score \| score >= 90 => ... \| Fail`。名前あり + `\|` は条件列、名前なし + `\|` は pattern（§52）。`fn` も同じ | §3, §7, §15, §53 |
| Q12 | 従来形 `=> if \| ...` | E517 で拒否。arity と型注釈の例外なし。継続を挟んだ形は対象外 | §53 |
| Q13 | 実行 | 本文書 10 節を手順書としてエージェントに渡す。仕様は本セッションが改訂済み | — |
| Q14 | 式側の `...` の対象 | 任意の式。`[f head, ...map tail f]` と書ける。パターン側は名前か `_` | §41, §46 |

前提として置いたもの: `[...xs]` 単独と末尾以外の `...` は E100。引数ゼロで `|` が続く `def main | ...` は E109。名前あり + `|` を pattern のつもりで書いた `def f x | Some(y) => y | 0` は `y` の E204 のままで、専用の診断は足さない。印字（§51）は既に `[1, 2, 3]` 形なので変えない。

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

**読む順:** 本節 → `docs/0.1.1/corpus.md` → `docs/0.1.1/spec.md`（§41 §43 §46 §50 §53 を先に）→ 9 節。表と仕様本文が食い違えば仕様本文が正で、食い違いは報告に書く。
**判定者:** 各工程の成果物は、作成した本人以外が判定基準に沿って審査する。別モデルのエージェントを呼べる環境ならそれに、呼べなければ人に出す。
**仕様は確定済み:** 9 節の決定は仕様に反映済みで、各ラウンドは コーパス → lock → 実装 の順に進む。判定に現行コンパイラを使えるのは R10 だけである。R11 以降は新形式を parse できないので、コーパスの根拠は仕様の読みだけである。
**仕様に疑義が出たら:** 仕様を変えず、解釈を選ばず、報告の「仕様への疑義」に書く。依存しない作業は進め、依存する作業は止める。
**過渡状態:** 本節を書いた時点で `docs/0.1.1/` の仕様・コーパス規約・追跡表は hado の語彙、コードとコーパスは shin の語彙である。R10 が解消する。
**コミット:** 工程ごとに既存の形式（`test:` `feat:` と日本語の命令形）で。push しない。
**報告:** 完了時と、依存する作業で止まったときに返す。(1) 判定条件ごとの実測（コマンドと出力の末尾）、(2) 赤の門番の一覧と理由、(3) `git log --oneline` の増分と `git status --short`、(4) 仕様への疑義、審査の指摘と対応、迷った点。

| 工程 | 成果物 | 判定 |
|---|---|---|
| R10 改名 | リポジトリ全体（`docs/0.1.1/ledger.md` と本文書の 1〜8 節を除く）、`tests/corpus.sha256` | 10-a〜10-d。審査は別の判定者 |
| R11 コメント | `tests/accept` `tests/reject` `stdlib/` のヘッダ、`docs/0.1.1/corpus.md`、`crates/hado/**`、`scripts/`、lock | 11-a〜11-c |
| R12 List 構文 | `tests/accept` `tests/reject` `stdlib/`、`crates/hado/**`、`scripts/`、lock | 12-a〜12-e |
| R13 引数の条件列 | `tests/accept` `tests/reject`、`crates/hado/**`、`scripts/`、lock | 13-a〜13-d |

**着手時点で赤の門番:** `verify-m1-corpus.sh` チェック 1（§50 が 58 行、拒否コーパスが 56 本）、チェック 2（§39 の `classify` が短縮形、コーパスが従来形）、チェック 3（9.2 の項目に受理と拒否が無い）と、それらに依存する `verify-m2.sh` `verify-m3.sh` のチェック。赤の一覧は 9 節を書いたセッションが実測して `ledger.md` に書いた。R13 の終了時点で全て green に戻す。門番を緩めて green にすることは禁止。

### R10 改名（shin → hado、`.shin` → `.hd`）

機械的な変更だけを含む。振る舞いの変更を混ぜない。

- `crates/shin` → `crates/hado`（`git mv`）。package 名、bin 名、workspace の members、`Cargo.lock`
- `tests/accept` `tests/reject` `stdlib/` の全ファイルを `.hd` に `git mv`。`main.rs` の拡張子検査と usage、`build.rs` の一時ディレクトリ名、`lib.rs` の `include_str!` のパス
- ランタイム記号 `shin_entry` `shin_alloc` → `hado_entry` `hado_alloc`。`runtime/` `runtime_print.c` `runtime_abi.rs` `emit.rs` と、記号を検査する `verify-m0.sh`
- 門番スクリプトとテストの文字列、`README.md`、`AGENTS.md`、`docs/cranelift-0.135.md`、`docs/0.1.1/` の `corpus.md` `spec-trace.md` `spec-coverage.md`（仕様と本節は hado で書いてある）
- **lock の再生成。** `tests/corpus.sha256` はファイル名を含むので改名で必ず不一致になる。`sha256sum tests/accept/*.hd tests/reject/*.hd stdlib/*.hd` で作り直す。`AGENTS.md` の「実装の工程で更新しない」の例外を本項が与える

R10 の判定:

- 10-a. `grep -rIl shin --exclude-dir=.git --exclude-dir=target . | grep -v '^./docs/0.1.1/ledger.md$' | grep -v '^./docs/0.1.1/plan.md$'` が空。`plan.md` は 1〜8 節が 0.1 の記録なので除く（9 節以降に shin が無いことは目視で確かめる）
- 10-b. 全門番（m0〜m7、r9）が改名前と同じ `PASSED: n/N`、`cargo test` が同数で通過
- 10-c. lock のハッシュ列が改名前と同一（`cut -d' ' -f1` の diff が空）。内容を変えていない証拠
- 10-d. `hado build x.shin` が拡張子で拒否され、`hado build x.hd` が実行ファイルを作る

### R11 コメント（`//`）

仕様は §41 §43 §50 に反映済み。

コーパス:

- 全件のヘッダ `-- spec:` `-- error:` `-- expect:` `-- expect-error:` `-- item:` `-- needs:` `-- short-circuit:` を `// ` に。`docs/0.1.1/corpus.md` のヘッダタグ表も
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
| `E516.hd` | 新設。パターンに `Nil` を使う関数と、それを呼ぶ `main`。`// item: List 構文` | §46 |
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

仕様は §3 §7 §15 §39 §41 §50 §53 に反映済み。

コーパス:

| ファイル | 変更 | 根拠 |
|---|---|---|
| `classify.hd` | 短縮形に。`verify-m1-corpus.sh` チェック 2 が §39 と照合する | §39, §53 |
| `E517.hd` | 新設。`def classify score => if \| ...` と `main`。`// item: 引数の条件列` | §53 |
| 受理の新設 `condition_params.hd` | 引数 2 つと型注釈付きの短縮形、`fn x \| ...`。`// item: 引数の条件列` | §53 |
| `E109.hd` | そのまま。`def f \| ...` の変種は判定器のプローブ | §3 |

実装の注意（決定ではない）:

- parser: `parameters` の直後が `|` なら条件列の腕。AST は `=> if | ...` と区別できる印を持つ（E517 の判定と正規形印字のため）
- E517: `def` か `fn` の本体が条件列のノードちょうどで、短縮形で書かれていないもの。arity と注釈を見ない
- E109: 引数ゼロで `|` が続く形。アンカーは最初の `|`
- E515 との関係: 名前あり + `|` は条件列で pattern 腕にならないので、E515 の判定は変えない
- 正規形の印字は短縮形で出す

R13 の判定:

- 13-a. 全門番（m0〜m7、r9、r11〜r13）が終了コード 0、`cargo test` 全件通過
- 13-b. 判定器（`scripts/verify-r13.sh`）: (1) 乱数の arity（1〜3）と注釈の有無で `=> if | ...` を生成し、全件 E517 でアンカーが `def` か `fn`。(2) 同じ腕の短縮形が通り、実行結果が判定器側で計算した値と一致。(3) `let` を挟んだ形は通る。(4) `def main | true => 1 | 0` は E109 でアンカーが `|`。(5) `def f x | Some(y) => y | 0` は E204
- 13-c. 審査合格後に lock を更新
- 13-d. 位置摂動プローブが E517 に当たる

R13 の終了時に `git tag v0.1.1` を打つ。

### スコープ外

9.3 の項目と 6 節の「v0.2 へ保留」。warning の導入。
