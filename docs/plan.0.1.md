# 開発計画 v0.1

対象は `docs/spec.0.1.md`（以下「仕様」）。仕様の節番号は、M1 で改訂する前の現行版を指す。

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
| バックエンド | LLVM 22 |
| 実装言語と結合 | Rust、inkwell 0.10（feature `llvm22-1`）。llvm-sys 221 に解決される |
| LLVM の入手 | apt.llvm.org の noble-22（`llvm-22-dev` `libpolly-22-dev` `clang-22`） |
| 入口 | `def main = expression`。ランタイムが `main` の値を stdout に印字し終了コード 0。値が `Error(e)` なら stderr に印字し終了コード 1。言語内に副作用はない |
| 多相の実現 | 単相化。実行時型情報を持たない。多相再帰はコンパイルエラー |
| メモリ管理 | Boehm GC（`libgc-dev`）。ランタイムの確保関数は一つに絞る。参照カウントは v0.2 |
| 標準ライブラリ | prelude を shin 自身で書き、全プログラムに暗黙に連結する。組み込みは Int / Float の算術と比較、String の長さ・連結・比較、Char と String の相互変換、構造比較 `==` `compare`、印字に限る |
| prelude の関数集合 | List: map filter fold length reverse append sort。Option: map unwrap_or is_some ok_or。Result: map map_error is_ok。String: length concat。Int / Float: to_string。sort は比較関数を引数に取る merge sort |
| Int | `i64`。加減乗はオーバーフローで wrap。`/` `%` は `Result<Int, ArithmeticError>` を返し、除数 0 で `Error(DivisionByZero)`。Float の `/` は IEEE 754 |
| 拡張子 | `.shin` |
| 関数値の表現 | 「関数ポインタ + 環境」のクロージャに統一。名前付き関数は環境なし |

### 2.2 言語意味論（M1 で仕様に書き込む）

| 項目 | 決定 |
|---|---|
| 関数適用 | n 引数。定義の引数数と呼び出しの引数数は一致必須。部分適用なし |
| パイプ | `x \|> f a b` は `f x a b`。標準ライブラリの引数順は data-first |
| 関数値 | `def` で定義した関数名を引数なしで参照すると `Fn<...>` 型の値 |
| 呼び出し構文 | 関数は並置 `f x y`、constructor は括弧 `Some(x)`。大文字始まりが constructor |
| 冗長・到達不能の判定 | 構文的。束縛変数の名前替えと括弧を正規化した後の構造一致。条件列の到達不能は「先行条件との構造一致」と「先行条件が `true`」に限る |
| module | `module X : SIG = struct ... end` を v0.1 に入れる。signature 注釈は必須。struct は sig に載らない定義を持てる。sig の `type T` は外から不透明。struct 内で sig からも内部からも参照されない `def` はエラー |
| record | 構築は `{ name = "a"; age = 3 }`。型は field 集合から一意に決まる nominal 型で、曖昧なら注釈を要求。パターンは `{ name }`（同名束縛）と `{ name = n }`（別名）。書かない field は無視。field 更新構文は v0.2 |
| tuple | 持たない。`Fn<(Int, Int), Int>` の `( )` は引数列 |
| 演算子 | 算術 `+ - * / %`、比較 `< <= > >= == !=`、論理 `&& \|\| not`。算術と比較は Int / Float 両用の組み込み多相で、単相化時に確定し、未確定なら Int（`/` `%` の戻り型は 2.3）。文字列連結演算子は置かない。優先順位は高い方から 適用、単項 `-`、`* / %`、`+ -`、比較、`&&`、`\|\|`、`\|>`。比較は非結合 |
| リテラル | Int は 10 進。Float は小数点の両側に数字必須。Char は `'a'`。String は `"..."`、エスケープは `\n \t \\ \"`。Bool は `true` `false`。Unit は `()` |
| Unit payload | `Ok()` が canonical。`Ok(())` は拒否 |
| コメント | `--` 行コメントのみ |
| 変数パターン | 最上位の `\| x => ...` は禁止。入れ子内は許可 |
| リテラルパターン | 持つ。Int / Float / Char / String / Bool と入れ子で使える。Bool 以外は fallback 必須。条件列の全条件が「同一変数 `==` リテラル」ならパターン分岐で書けるため拒否 |
| 条件列の判別 | パーサは改行を見ない。`if` の直後が `\|` なら条件列。仕様 1289 行の改行要求は削除 |
| 名前空間 | 型と constructor は別。仕様 965 行の `Error` 型はユーザー定義 |
| List リテラル | `[a, b, c]` は `Cons` 連鎖の糖衣。空は `Nil`。末尾がリテラルの `Cons(a, Nil)` `Cons(a, [b])` は拒否 |
| 相互再帰 | top-level は定義順に依存しない。呼び出しグラフの閉路に含まれる関数は全て `rec` |

### 2.3 M1 で確定させる推定既定

対話で扱わなかった点。M1 の着手時に本表の既定で仕様に書き、ユーザーの確認を得る。

| 項目 | 既定（推定） |
|---|---|
| 名前空間の全体 | 型・module・constructor・値の 4 つ。`Option<T>`（型）と `Option.map`（module）は共存する。prelude の constructor `Some` `None` `Ok` `Error` `Nil` `Cons` は修飾なしで参照できる |
| 言語が知る prelude の型 | `Option` `Result` `List` `ArithmeticError` はコンパイラが名前で解決する（`/` の戻り型、`let?` の対象、List リテラルの展開先） |
| `main` の型 | 単相でなければならない。`def main = Nil` は型変数が残るためエラー |
| 文字 | Char は Unicode スカラ値（`i32`）。String は不変の UTF-8。`String.length` はスカラ値の個数 |
| top-level の未使用 `def` | エラー（仕様 §27 の適用範囲を top-level にも広げる）。`main` は例外 |
| ADT のレイアウト | payload なし constructor は即値、payload あり constructor は GC 管理のヒープ。再帰型 `List<T>` はこれで間接参照になる |
| `/` `%` の戻り型 | Int では `Result<Int, ArithmeticError>`。Float の `/` は `Float`。Float に `%` は置かない |
| ArithmeticError の名前 | より具体的な名前（`DivisionError` 等）を候補に、M1 で決める |
| prelude の module 構成 | prelude の各 module も `module X : SIG = struct` の形で書く（2.2 の注釈必須規則に従う） |
| JIT の適用範囲 | JIT はヒープを使わない部分集合（Int / Float / Bool / Unit のみ）の単体テストに限る。コーパスのテストは AOT で実行ファイルを生成しサブプロセスで実行する。Boehm GC と Rust テストプロセスの共存を避けるため |
| エラーコード | `E1xx` 構文、`E2xx` 名前解決、`E3xx` 型、`E4xx` 網羅性・到達可能性、`E5xx` canonical・冗長、`E6xx` module、`E7xx` 単相化。各コードに短い名前を付け、拒否コーパスはコードで照合する |

## 3. マイルストーン

完了条件の（確定）は対話または実測から導いたもの、（推定）は本計画で置いたもの。

リポジトリ構成:

```text
Cargo.toml            ワークスペース
crates/shin/          コンパイラ本体と CLI
runtime/              C ランタイム（GC 確保・文字列・印字）。cc crate でビルドして静的リンク
stdlib/               prelude（.shin）
tests/accept/         受理コーパス
tests/reject/         拒否コーパス
docs/                 spec.0.1.md、plan.0.1.md
```

### M0 環境と歩く骨格

1. LLVM 22 と Boehm GC を導入する。

   ```sh
   wget https://apt.llvm.org/llvm.sh
   chmod +x llvm.sh
   sudo ./llvm.sh 22
   sudo apt install llvm-22-dev libpolly-22-dev clang-22 libgc-dev
   ```

2. `.cargo/config.toml` に `[env] LLVM_SYS_221_PREFIX = "/usr/lib/llvm-22"` を置く。
3. ワークスペースを作る。`crates/shin` の依存は `inkwell = { version = "0.10", features = ["llvm22-1"] }`、build 依存は `cc = "1"`。
4. `runtime/` に `shin_alloc`（`GC_malloc` の薄い包み）と Int 印字を書き、`cc` で静的ライブラリにする。
5. `def main = 1 + 2` を lexer → parser → 型 → LLVM → オブジェクト → `clang-22` リンクの順で通し、実行ファイルが `3` を印字する。

完了条件:

- `cargo build` が LLVM 22 に対して成功する（確定。ただし現時点で未実走）。
- `cargo test` に、`def main = 1 + 2` を AOT で実行ファイルにし、サブプロセスで走らせて stdout が `3` と一致するテストがあり、通る（確定）。
- 同じ式を JIT で評価して `3` を返す単体テストが通る（推定）。

### M1 仕様の穴埋めとコーパス

1. 2.2 と 2.3 の内容を `docs/spec.0.1.md` に書き込む。既存の節番号は変えず、新設の節は §42 の後に追加する。コーパスの `-- spec:` はこの番号を指す。

   | 対象 | 内容 |
   |---|---|
   | §5〜6 | `main` |
   | §8、§37 | `Fn` の n 引数 |
   | §12 | record の構築とパターン |
   | §13〜19 | リテラルパターン、変数パターン |
   | §25 | パイプの第 1 引数 |
   | §32 | 「静的に到達不能」の定義 |
   | §34〜35 | struct |
   | §36 | Int、Char、String |
   | §38 | struct を除外一覧から外す。operator overloading の禁止は維持し、組み込み算術演算子はユーザーが定義できない固定集合への型指向解決であると注記する |
   | §41 | 文法 |
   | 新設 | リテラルとコメント、演算子と優先順位、List リテラル、名前空間、prelude、エラーコード |

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
- `cargo test --test corpus` がコーパス全件を列挙し、件数を出力する。この時点で通る件数は問わない（確定）。

### M2 構文

1. lexer と parser を書く。全ノードに位置情報を持たせる。

完了条件:

- 受理コーパス全件が構文解析を通る（確定）。
- 拒否コーパスのうち `E1xx` を期待するものが指定コードで落ちる（確定）。
- 拒否コーパスの残りが構文解析を通る（確定）。

### M3 型

1. 名前解決。4 名前空間、module の階層、sig による隠蔽。
2. HM 型推論。`def` 単位の一般化、出現検査、ADT / record / generics。
3. module の signature matching と抽象型。
4. 組み込み多相（算術・比較・`compare` `==`）。
5. prelude のうち型定義のみ（`Option` `Result` `List` `ArithmeticError`）を `stdlib/` に置いて連結する。関数は M7。

完了条件:

- `shin check` が、`-- needs: prelude` を持たない受理コーパス全件を通す（確定）。
- 拒否コーパスの `E2xx` `E3xx` `E6xx` が指定コードで落ちる（確定）。
- unify、generalize、出現検査、signature matching の Rust 単体テストがある（推定）。

### M4 検査器

1. 網羅性（`E4xx`）: 構造に基づく網羅性、到達不能 branch、重複パターン、fallback の位置。
2. canonical（`E5xx`）: identity binding、未使用 binding、純粋値の破棄、`let? _`、同一 branch、再包装、単純再伝播、`Ok(())`、`Cons` 末尾リテラル、条件列からリテラルパターンへ、最上位変数パターン、top-level と struct の未使用 `def`、`rec` の閉路規則。
3. 式の正規化と構造比較を一つの module にまとめ、§24 §30 §32 §33 から共用する。

完了条件:

- 拒否コーパス全件が指定コードで落ちる（確定）。
- `-- needs: prelude` を持たない受理コーパス全件が検査を通る（確定）。
- 各エラーが「どの branch や binding と同一か」を位置付きで出す（推定）。

### M5 単相化とクロージャ変換

1. 多相 `def` を使用型ごとに実体化する。多相再帰を検出してエラーにする。
2. `fn` と関数値をクロージャ（関数ポインタ + 環境）に変換する。
3. 型付き中間表現を定義し、`shin dump-mono` で出力する。

完了条件:

- `-- needs: prelude` を持たない受理コーパス全件で `shin dump-mono` が成功し、出力に型変数が残らない（確定）。
- 多相再帰の拒否コーパスが `E7xx` で落ちる（確定）。
- `main` が単相でない例が `E7xx` で落ちる（推定）。

### M6 コード生成

1. inkwell で中間表現を LLVM IR に落とす。ADT は 2.3 のレイアウト。
2. ランタイムに文字列操作、構造比較の補助、全型の印字を足す。
3. オブジェクト出力と `clang-22` によるリンクを `shin build x.shin` にまとめる。

完了条件:

- `-- needs: prelude` を持たない受理コーパス全件が、実行ファイルとして期待 stdout を返す（確定）。
- `main` が `Error(e)` を返す受理コーパスで、stderr に `e` が印字され終了コードが 1（確定）。
- `module.verify()` が受理コーパス全件で通る（確定）。

### M7 prelude

1. 2.1 の関数集合を `stdlib/` に shin で書く。各 module に sig を付ける。
2. 全プログラムへの暗黙連結を実装する。

完了条件:

- 受理コーパス全件が期待 stdout を返す（確定）。
- prelude 自体が M4 の検査を通る（確定）。
- `cargo test` 全件が通る（確定）。

## 4. 実走したコマンドと未検証

| 内容 | 状態 | 結果 |
|---|---|---|
| inkwell 0.10 `llvm22-1` の依存解決 | 実走 | `cargo fetch` 成功。llvm-sys 221.1.0 に解決 |
| llvm-sys の prefix 変数名 | 実走 | build.rs より `LLVM_SYS_221_PREFIX` |
| apt.llvm.org noble-22 の package 名 | 実走 | `clang-22` `llvm-22` `llvm-22-dev` `libpolly-22-dev` `lld-22` を確認 |
| libgc-dev の内容 | 実走 | `/usr/include/gc.h` `libgc.a` `libgc.so` `bdw-gc.pc` を確認 |
| `sudo ./llvm.sh 22` と `apt install` | 未検証 | sudo を要するため計画段階では実行していない |
| LLVM 22 に対する `cargo build` | 未検証 | LLVM 未導入のため |
| Boehm GC と JIT の共存 | 未検証 | 2.3 で JIT をヒープなし部分集合に限定して回避する |

## 5. 判断の解説

インタプリタではなくコンパイラを v0.1 の成果物にした。検査器だけでは「動くプログラム」が一つもなく、型検査の正しさを実行結果で確かめる手段がないため。

バックエンドは C ソース生成ではなく LLVM にした。ユーザーの選好である。inkwell を選んだのは、JIT でテストの回転を速くできることと、型付き Builder が IR の型不一致を構築時に捕まえるため。テキスト IR 生成なら LLVM 23 に即座に乗れるが、inkwell が 22 に追従済みなので差は小さい。

関数適用を n 引数にした。パイプが第 1 引数を埋める規則である以上、部分適用が仕様の例を支える場面はない。残るのは表現力の追加だけで、それは「同じ意味に複数の表現を許さない」原則と衝突する。呼び出し規約も 1 定義 1 関数で済む。

多相を単相化で実現した。HM の rank-1 多相と単一コンパイル単位という条件下では常に可能で、実行時型情報が不要になる。ランタイムは確保・文字列・印字だけになり、LLVM の型付き IR をそのまま使える。

メモリ管理を v0.1 では Boehm GC に委ねた。この言語は mutation がなく再帰も top-level の `def rec` に限るため参照が循環せず、参照カウントが最終形として適している。しかし retain / release の挿入は use-after-free を生みやすく、検査器・コード生成と同時に立ち上げると原因の切り分けが難しい。確保関数を一つに絞っておけば、v0.2 での差し替え範囲は明確である。

prelude を shin 自身で書く。`sig` `struct` `def rec` パターン分岐の最初の本格的な利用者になり、コンパイラの実地テストを兼ねる。prelude も canonical 検査を通るので、検査器の妥当性も同時に試される。

コーパスをコンパイラより先に書く。この言語では「何を落とすか」が仕様そのものであり、先に固定しないと検査器の完了条件が書けない。

工程順（構文 → 型 → 検査器 → 単相化 → コード生成）を機能ごとの縦割りより優先した。型検査器と検査器は unify や網羅性行列のように機能横断の設計を先に置かないと作り直しになる。ただし M0 で LLVM 22 と inkwell の結合を最初に実測する。外部依存のうち失敗の影響が最も大きいため。

## 6. リスクと v0.2 への保留

リスク:

- inkwell の LLVM 追従。LLVM 23 が stable になったとき、inkwell 0.10.0（2026-08-06 公開）は 22 までしか対応しない。追従の遅れの実績は計測していない。
- 多相再帰。型注釈付きで多相再帰が推論される場合、単相化が停止しない。M5 で検出してエラーにする。

v0.2 へ保留:

- 参照カウントによるメモリ管理。
- 条件列の到達不能判定への区間推論。
- record の field 更新構文。
- functor、first-class module。
- I/O と効果の順序付け。

