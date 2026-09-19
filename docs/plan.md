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
| リテラル | Int は 10 進。Float は小数点の両側に数字必須。Char は `'a'`。String は `"..."`、エスケープは `
 \t \\ \"`。Bool は `true` `false`。Void は `()` |
| Void payload | `Ok()` が canonical。`Ok(())` は拒否 |
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
| スカラの機械語型 | Int は i64、Float は f64、Bool は i8、Char は i32、Void は i64 の 0。Cranelift に i1 はない |
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
- v0.2 の `again` 操作は末尾再帰でもスタックを消費する。Koka 3.2.3 の実測で約 330 バイト / 反復、既定スタック 8 MB では約 3 万反復で SIGSEGV し、メッセージは出ない（17.8）。生成器やイテレータを長い列に使うときの上限になる。回避には segmented stacks が要り、専用ランタイムを持つことになる。

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

## 9. v0.1.1 の範囲と決定（2026-09-08）

`docs/wants.md` の 6 項目のうち 4 項目（言語名の変更、`//` コメント、List の `[x, ...xs]`、引数の条件列の短縮形）を 0.1.1 とする。grilling（14 問）で確定し、経緯は `docs/ledger.md` の「0.1.1 の計画」にある。仕様 `docs/spec.md` は本節を反映済み。2.1〜2.3 と 7 節と食い違う箇所は本節が優先する。

判断基準は 7 節と同じ。拒否か許容かで迷ったら拒否し、新機能は最小形で入れる。受理を広げる変更は後から安全に足せるので、後回しにした拡張は 9.3 に置く。

### 9.1 決定

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| Q1 | 0.1.1 の位置づけ | v0.1 を tag `v0.1` で凍結し、wants の一部を 0.1.1 とする | — |
| Q2 | 範囲 | 言語名、コメント、List 構文、引数の条件列。ファイル名 module と `let value? =` は次の版 | — |
| Q3 | 改名の位置 | 最初のラウンド（R10）。以降の仕様・コーパス・コミットを新しい語彙で書くため | — |
| Q4 | 改名の範囲 | 過去の記録（`ledger.md`、本文書の 1〜6 節、`docs/rounds/r6.md`、`docs/rounds/r8-r9.md`、git 履歴）以外の全て。CLI `hado`、拡張子 `.hd`、crate `crates/hado`、ランタイム記号 `hado_entry` `hado_alloc` | — |
| Q5 | 文書の版 | tag の後、6 文書を版接尾辞なしの `docs/spec.md` などに `git mv` して直接編集する。0.1 の姿は tag が持つ。（当初は `docs/0.1.1/` に置いたが、版は tag が持つのでディレクトリも外した） | — |
| Q6 | 手順書の置き場 | `PLAN.md` を廃止し `docs/plan.md` に統合する。`AGENTS.md` と `README.md` は進行中の開発に依存しない | — |
| Q7 | 再編の時期 | tag の前。tag が「0.1 完了で文書も整合した状態」を指すため | — |
| Q8 | コメント | `//` に置き換える。`--` と `#` は E104 | §41, §43 |
| Q9 | List の書き方 | 式もパターンも `[]` `[x, ...xs]` が唯一の形。式の `Nil` `Cons` は E510（`list-literal-form` に改名して範囲を広げる）、パターンは E516。prelude も書き直す | §46, §47, §50 |
| Q10 | 展開の位置 | 末尾のみ。中間展開は 9.3 | §41, §46 |
| Q11 | 条件列の短縮形 | `def classify score \| score >= 90 => ... \| Fail`。名前あり + `\|` は条件列、名前なし + `\|` は pattern（§52）。`fn` も同じ | §7, §15, §53 |
| Q12 | 従来形 `=> if \| ...` | E517 で拒否。arity と型注釈の例外なし。継続を挟んだ形は対象外 | §53 |
| Q13 | 実行 | `docs/rounds/r10-r13.md` を手順書としてエージェントに渡す。仕様は本セッションが改訂済み | — |
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
| Q6 | 種別と §48 | 種別は 3 つ（18 節 Q12 で `once` / `abort` / `again` と綴る）。§48 は「`again` を経由しない末尾呼び出しはスタックを消費しない」に限定する。`again` の下では末尾呼び出しが失われ、スタックを反復回数に線形に消費する（17.8 の実測） | §48, 新節 |
| Q7 | Option の境界 | 理由の有無で分ける。Option は値の不在、効果は理由のある失敗。今の Option / Result の区別をそのまま使う | §21 |
| Q8 | `?` と `do` | 効果的な呼び出しの束縛に印を付けない。`?` は Option 専用の見分け札として存続し、`do` は「効果を持つ式の値を捨てて継続する」に意味を移して存続する | §22, §23, §24 |
| Q9 | handler の形 | §34 / §35 と同じ節構文。`effect E` が契約側、`handler E as H` が実装側で、使用は `with H -> rest`。宣言なので §20 の branch のネスト禁止に触れず、handler 節の中で `if` が書ける | 新節, §34, §35 |
| Q10 | `main` と印字 | `main` に残せるのは組み込み効果だけ。raise された値は stderr で終了コード 1、正常値は stdout で 0 とし、観測される振る舞いを現行のまま保つ。ユーザー定義効果が残れば新しい E コード | §5, §51 |
| Q11 | 効果多相 | 入れる。`Fn` の型に効果行を持たせ、単相化で実体化する | §37, §10 |
| Q12 | 効果行の綴り | 後置の行。区切りと効果名は 18 節が定める（`Int {Error}`、`Fn(A => B {E})`） | §37, §41 |
| Q13 | 省略 | 効果行だけ結果型と独立に省略できる | §8, §9 |
| Q14 | 省略の意味 | 省略は暗黙の効果変数（効果多相）。契約でも外の `def` でも同じ意味とし、純粋の主張には `Pure` を書く | §34, §9 |
| Q15 | `again` を含むループ | 継続の表現は short-cut resumption とする。ただしこれは実行時間の最適化であって、§48 の保証は回復しない（17.8 の実測） | §48, 新節 |
| Q16 | 操作の名前空間 | 値空間に無修飾で入れる（`raise error`）。`compare` と同じ扱いで、名前空間は 5 つのまま増やさない | §47 |
| Q17 | 進め方 | 未確認をプロトタイプで潰してから、効果を足し `Result` を最後に撤去する。各ラウンドで門番を green に保つ | 手順書 |
| Q18 | 版 | v0.2 | — |
| Q19 | prelude の `Pure` | 純粋な 9 本に `Pure` を書く。高階の 5 本（`List.map` `filter` `fold` `sort`、`Option.map`）は省略のまま効果多相にする | §49 |
| Q20 | 効果を書ける位置 | 効果を持つ式を書けないのは分岐の条件だけとする。scrutinee・適用の引数・演算子の被演算子・record field は許し、評価順を §44 が左から右と定める（2 巡目と 3 巡目の審査で 2 段階に絞った。下記 17.3） | §14, §15, §24, §44 |

### 17.2 種別ごとの保証

| 種別 | `resume` | §48 の保証 | コード生成 |
|---|---|---|---|
| `once` | 呼び出し位置で即時 | あり | その場で評価し、yield しない |
| `abort` | 不可（中断のみ） | あり | tag 検査と分岐。現行の Result と同形 |
| `again` | 節内のどこでも 1 回 | なし | short-cut resumption。継続を捕まえるため末尾呼び出しが失われ、スタックを反復ごとに約 330 バイト消費する |

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

Koka 3.2.3 で、`again` に相当する `ctl` 操作を含む末尾再帰ループを走らせた。測定は docker の ubuntu 22.04、公式インストーラ。プログラムは `docs/ledger.md` の「0.2 のプロトタイプ」に置いた形。

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

`once` に相当する tail-resumptive 操作は効果を持たないループと同じ挙動で、§48 の保証はこの種別で完全に成り立つ。

### 17.9 実測（17.6 の 2、2026-09-10）

17.6 の未確認 2（short-cut resumption の prompt / under フレーム復元が、hado の設定でも必要か）を、
本体の実測・現物の読み・最小モデルの 3 つで確かめた。

**本体の実測。** `again` handler を他の handler と組み合わせた 15 形を `hado build` にかけた。
**15 形すべて `hado check` は通る。破れるのは単相化だけである。**

| 形 | 実測 |
|---|---|
| `Collect` 単独 | ok。`[3]` |
| `Collect` 単独・yield 2 回 | ok。`[1, 2]` |
| `Outer(A)` ⊃ `Collect` | NG `done payload mismatch` |
| `Outer(A、return 腕あり)` ⊃ `Collect` | NG `done payload mismatch` |
| `CollectY` ⊃ `CollectZ`（again 同士） | NG `done payload mismatch` |
| 同じ効果 `Y` の handler の入れ子 | NG `done payload mismatch` |
| `Collect` ⊃ `Trap`（abort の handler） | NG `done payload mismatch` |
| `resume` の中から外側の `again` を起こす | NG `done payload mismatch` |
| `resume` の中から外側の `once` を起こす | NG `done payload mismatch` |
| 内側 handler の `return` が最終結果に残る形 | NG `done payload mismatch` |
| `Collect` ⊃ `Inner(B)` | NG `continuation result type mismatch` |
| `Collect` ⊃ `Inner(A)` | NG `continuation result type mismatch` |
| `Collect` ⊃ `Inner(B)`・yield 2 回 | NG `continuation result type mismatch` |
| 再開した継続の中で内側の `once` を呼ぶ形 | NG `continuation result type mismatch` |
| `Outer(A)` ⊃ `Collect` ⊃ `Inner(B)`（B5） | NG `continuation result type mismatch` |

**B5 は 3 段固有ではない。2 段で既に壊れ、`again` handler が内側か外側かで症状が変わる。**
内側なら forwarding が handler の答えを外側の Outcome の `$Done` に包み、外側なら継続が
最内 handler で閉じる。`docs/bug.md` の B5 は 3 段の例だけを記録しており、範囲を過小に書いていた。
**測った 15 形のうち、通ったのは `again` handler が単独の 2 形だけである**
（15 形は網羅ではないので、これ以外に通る合成があるかは未確認）。
`hado check` が 15 形すべてを通すことから、仕様も型システムもこの形を認めており、
追いついていないのは単相化の畳み方だけだと分かる。

**現物の読み。** `done payload mismatch` の経路は `crates/hado/src/mono.rs` で追える。
`interpret_outcome` は `forwarding = handler_stack.len() > 1`（`:1756`）のとき
`align_switch_arms` に**元の Outcome をそのまま fallback として渡す**（`:1758`）。
`align_switch_arms` は fallback の型を基準に、Outcome でない腕を `done()` で包む（`:1915-1917`）。
`done()` は `value.ty() != success` で拒否する（`:1935-1937`）。元の Outcome の success 欄は
中の計算の成功型（`Void`）のままで、handler の答えは `List Int` なので、ここで必ず落ちる。
**forwarding が Outcome を変換していないことが原因で、包み方の細部ではない。**

`continuation result type mismatch` の側は `make_continuation` の `body.ty() != *answer`
（`:1613`）で、`answer` は `again_answers.last()`、すなわち**最内の答えを要する handler**であって
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
| 再開後に内側の `once` を呼ぶ | `[1, 2]` | unhandled operation |
| 内側の `return` が結果に残る | `[1, 42]` | continuation result type mismatch |
| B5（3 段） | `[3]` | continuation result type mismatch |
| `resume` の中から外側の `again` | NG unhandled operation | NG unhandled operation |

**モデルが確かめたこと。** `under` の切り方なら、11 形が**畳み切れて**、handler に設定した
型ラベルが最後まで矛盾なく繋がる。**モデルは型検査を持たない**ので、これは「型が正しい」の
証明ではなく「型ラベルの流れに切れ目が無い」の確認である。`$Outcome` の型付けそのものは
確かめていない。移植では型の設計を別に立てる必要がある。

再開した継続の中で内側の `once` を呼ぶ形（`[1, 2]`）と、内側の `return` が最終結果に残る形
（`[1, 42]`）は、**under フレームの復元が実際に効いていないと出ない値**である。前者のトレースは
`Collect.again y(1) を 2 段で捕捉（復元する内側 handler 1 段）` の後に
`Inner.once b() = 2` が続く。

**モデルが確かめていないこと。** `hado` の側は 15 形のうち 5 形しか症状を再現しない。
モデルは forwarding を実行時に決め、本体は単相化の時点で型を決めるためである。
**負の側の根拠は本体の実測と現物の読みであって、モデルではない。**

**`resume` の中から `H` より外側の handler が扱う効果を起こす形は、`under` でも畳めない。**
モデルは外側のフレームを再開時の探索対象から外しているのでこの結果になるが、
これはモデル固有の制限ではなく、本体の表現からも同じ結論が出る。
`interpret_outcome` は継続を `Closure([Void], answer)`（`:1747-1753`）として射影する。
つまり `resume : Void -> A_H` で、**答えの型そのものを返す**。

境界は「内側か外側か」で決まる。**`H` 自身への yield は閉じる** — `H` の解釈が再帰的に
処理して `A_H` を作る（単独 handler の yield 2 回がこれで、実測でも通っている）。
**`H` より内側の handler への yield も閉じる** — 内側の解釈を合成すれば `A_H` に届く。
**`H` より外側へ yield したときだけ、`A_H` の値が存在しない。** 一般化するには継続が
Outcome を返す形（`Void -> Outcome<A_H, ...>`）が要り、腕の本体が Outcome を扱う形を
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
| Q12 | 操作の種別の綴り | `once` / `abort` / `again`。継続に何が起きるかを名指しする | 新節 |
| Q13 | `abort` の戻り型 | 書かせない。再開しないので戻り値が存在しない | 新節, §50 |
| Q14 | handler の未使用引数 | `_` を許す（13.1 Q4 の範囲を handler 節へ広げる） | §27 |
| Q15 | `Error` の印字 | payload の `message` field だけを stderr へ出す | §51 |
| Q16 | `DivisionError` | 廃止する。`/` と `%` は `Error` を raise する | §36, §44, §49 |
| Q17 | `with` の位置 | `prefix_form`。`let` `do` と同じ継続の形 | §41 |
| Q18 | `resume` | `again` の腕の中でだけ書ける primary | §41, 新節 |
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
again yield(value: A): Void

handler A Yield as Collect
again yield value =>
  let rest = resume () ->
  Cons(value, rest)
return _ => Nil

def rec each(values: Int List): Void {Int Yield}
| [] => ()
| [head, ...tail] =>
    do yield head ->
    each tail

def main =>
  with Collect ->
  each [1, 2, 3]
```

`main` の型は `Int List`、出力は `[1, 2, 3]`。`resume ()` の後ろに `Cons(value, rest)` があるので `again` が要る。§48 の保証を失うのはこの種別だけで、`{Int Yield}` という行から呼び出し側が静的に判別できる。

即答:

```ocaml
effect Ask
once ask: String

handler Ask as Fixed
once ask => "there"
```

### 18.3 文法の差分（§41）

```text
section         := module_section | implement_section | functor_section | apply_section
                 | effect_section | handler_section

effect_section  := "effect" type_vars? upper_name effect_item*
effect_item     := op_kind name "(" contract_param ("," contract_param)* ")" [":" type_expr]
                 | op_kind name [":" type_expr]
op_kind         := "once" | "abort" | "again"

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

キーワードは `effect` `handler` `with` `resume` `return` `once` `abort` `again` `Pure` の 9 個を足し、14 個から 23 個になる。

`abort` は戻り型を書けない。文法では表せないので本文で定める。

### 18.4 仕様の項目

コーパスの `item:` タグに使う項目名。2.2、9.2、11.2、13.2 の項目に加える。

| 項目 | 仕様の節 | 内容 |
|---|---|---|
| 効果の宣言 | §57 | `effect` 節、`once` / `abort` / `again`、型引数、0 引数の操作 |
| handler と with | §58 | `handler` 節、`return` の腕、`resume`、`with H -> rest`、引数付き handler |
| 効果行 | §37, §41 | 波括弧の行、`Pure`、省略と推論、アルファベット順の正準形 |
| 失敗の効果 | §21, §49 | `Error` と `Exception`、Option との境界、`Option.ok_or` |
| 効果の位置 | §24, §44 | 効果を持つ式を書けないのは分岐の条件だけ。scrutinee・引数・被演算子・record field は許し、評価順を §44 が定める |
| 末尾呼び出しの限定 | §48 | `again` を経由しない呼び出しだけを保証する |

### 18.5 新しいエラーコード

| 段階 | 検査 | 由来 |
|---|---|---|
| 構文 | `abort` の操作に戻り型を書いた | Q13 |
| 構文 | `resume` を `again` 以外の腕で書いた。書ける場所は構文的な入れ子だけで決まる | Q18 |
| 型 | 効果を持つ式を分岐の条件に置いた | 17 節 Q20 |
| 型（契約） | handler が効果の操作を実装していない、または効果に無い操作を書いた | 17 節 Q9 |
| 冗長 | 効果行に同じ効果を 2 回書いた | Q4 |
| 単相化 | `main` にユーザー定義効果が残った | 17 節 Q10 |

既存コードの変更は 17.4 の 3 件に加えて、E513 を effect と handler に広げる 1 件である。

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
| D2 | 操作 | 全て `once`。操作の結果の効果行は `{Error}` で、handler が無ければ runtime の既定実装が応え、失敗は `Error` を raise する。利用者は `raise` と同じく無修飾で呼ぶ（包みと `module` は置かない）。handler は差し替えに使え、E605 は全操作の実装を要求する | §49, §57, §58 |
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
| R37 | 世界との境界（D） | `stdlib/` の効果宣言、`runtime/`、`mono.rs` の既定実装、§5 §47 §51 §57 §58 | `Env` / `Io` の操作（`once`、結果の効果行 `{Error}`、handler 無しの既定実装）、argv / env / cwd / stdin / ファイル / ディレクトリ、非 UTF-8 は `Error`。E2 が完成して check と build を通る。受理ケースは空 argv・閉じた stdin・ケース内の相対パスで起動する |
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

組み込みは `runtime_abi.rs` の 16 本（印字 5、変換 3、String 3、他）で、`mono::Type` は `Int Float Bool Char String Void Data Abstract Closure` の 9 variant。配列と Bytes の variant は無い。

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
- R31 の台帳の未確認 3 件（型引数付き効果の重ね、`fn` の本体の `again`、handler の値引数）は本版の範囲外で、D の設計で当たったら `docs/bug.md` に置く

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

- 所属の唯一の正は各門番の `# tier:` 行（`docs/rounds.md`）。`scripts/lib/gate-tiers.sh` の
  `hado_tier_minimal_gates <層>` がそれを走査して各門番を最小の層に 1 回ずつ割り当て、`hado_tier_gates <層>` が累積（quick / quick+core / 全 38 本）を
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

## 35. v0.5 の範囲と決定（2026-09-17）

slug `prelude-tail-recursion`、`highlight-treesitter`、`lsp-minimal` を次の版にする。着手前の根拠は各 `docs/issues/<slug>.md` の現象欄、着手後の手順と実測は `docs/rounds/<slug>.md` が持つ。順は prelude → highlight → lsp で、lsp の門番が highlight のファイルタイプ判定に乗る。判断基準は 7 節と同じ（迷ったら拒否、新機能は最小形）。

| ID | 論点 | 決定 | 仕様の節 |
|---|---|---|---|
| P1 | prelude の再帰 | `stdlib/` の非末尾再帰を累積 + `reverse` の末尾形にする。新しい言語構成は足さない。`map` と `filter` は先頭から順に適用すると書く（`filter` の適用順が変わり、§55 に足す仕様変更になる）。添字が取れる列（Array・Bytes・argv・`read_dir`）は末尾から先頭へ累積して `reverse` を省く（同じ末尾形。slug `prelude-tail-recursion`）。`sort` の比較子は `Pure` とし、分割と比較の順は仕様に書かない（§36 により効果を持つ比較子は条件に置けず、`Pure` にしても受理されるプログラムは変わらない。`docs/rounds/prelude-tail-recursion.md` の決定 S2） | §55 |
| L1 | 診断の件数 | `--show-all` を opt-in で足す。同じ段階の全件をアンカー順に出す。E1xx と E3xx の致命は 1 件のまま。既定と拒否コーパスの判定は変えない | §56, §58.4 |
| L2 | 止まった後の型 | 冗長・網羅性・契約の検査で止まっていても型を返す。`--print-type` と LSP の hover は同じ意味。字句・構文・名前解決・型の致命で止まったときは型が無い | §58.3 |
| L3 | サーバの形 | `hado lsp` サブコマンド。JSON-RPC は `serde_json` で書く。機能は診断・hover（最上位の `def`）・正準形への code action の 3 つ。補完は入れない | §58 |
| L4 | 診断の位置 | `--format json` に終端の位置を足す | §58.4 |
| H1 | ハイライト | Neovim 用の手書き tree-sitter 文法を `editors/tree-sitter-hado/` に置く。受理コーパスと stdlib が `ERROR` 無しで読めることを門番にする。付録C からの生成は形が固まってから | 付録C |
