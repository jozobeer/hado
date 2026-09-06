# M3 型 実装計画

builder 向けの手順書。

**対象:** `docs/plan.0.1.md` 168–180 行の M3。
**実行主体:** builder。判定本体は `scripts/verify-m3.sh`（本ファイルに仕様を書き、スクリプトは builder が書く）。
**触ってよい場所:** `crates/shin/**`、`scripts/verify-m3.sh`、`scripts/verify-m2.sh` の条件 8 の 1 行（後述）、`stdlib/`。`tests/accept` `tests/reject` `crates/shin/tests/corpus.rs` は禁止。

---

## 意図

言語設計者が、構文解析の次に名前解決と HM 型推論を通して、v0.1 の型の主張をコンパイラで実証する（明示: `docs/plan.0.1.md` 7–8、168–174 行）。

`shin check` が入口である。コード生成・単相化・網羅性・canonical・prelude 関数は次ラウンド以降（明示: 同 182–218 行）。

---

## 現状

commander 実測（R5 着手時）:

| 項目 | 値 |
|---|---|
| 受理コーパス | 23 本。`-- needs: prelude` なしが 20 本 |
| E2xx / E3xx / E6xx 拒否 | 9 本（E201 E203 E204 E301 E302 E303 E304 E601 E602） |
| `cargo test` | 38 件通過 |
| 門番 | `verify-m0.sh` 7/7、`verify-m1-spec.sh` 6/6、`verify-m1-corpus.sh` 7/7、`verify-m2.sh` 8/8 |
| `crates/shin/src/types.rs` | 26 行。`Ty::Int` と M0 の `+` だけ（`types.rs:4-25`） |
| `shin` CLI | 無い（`crates/shin/src/main.rs` 無し、`Cargo.toml` に `[[bin]]` 無し） |
| `stdlib/` | 無い |

関連する実装の骨格:

- 全文法 AST は `parser.rs` の `NodeKind`（78–131 行）、`Item`（219–226 行）、`TypeExpr`（187–197 行）。診断の符号付き形式は `Diagnostic` の `Display`（289–296 行）: `<path>:<line>:<col>: error[CODE]: <message>`。
- `TypeDef`（257–261 行）に型パラメータ欄が無い。`parse_type_def` は `<T>` を読むが捨てる（536–561 行）。HM はここを直さないと generics を表現できない。
- `compile_to_object`（`lib.rs:14-24`）は parse → `lower_to_m0` → M0 の `types::check` → emit。`lower_to_m0`（`parser.rs:1378-1382`）は `Ast::main()` だけを見て、余剰のトップレベル `Item` を無視する。
- 出力契約の手本は `crates/shin/tests/parse_corpus.rs` と `scripts/verify-m2.sh`（チェックは独立、最後まで実行、末尾 `PASSED: k/k`）。

`-- needs: prelude` なしの受理 20 本（実測、`tests/accept/*.shin`）:

`classify.shin` `comment.shin` `create_user.shin` `delete_user.shin` `display_name.shin` `display.shin` `eta.shin` `fn_value.shin` `literal_pattern.shin` `literal.shin` `name.shin` `namespace.shin` `record.shin` `recursive_type.shin` `short_and.shin` `short_or.shin` `size.shin` `tuple_args.shin` `unit.shin` `user_store.shin`

除外（prelude 関数、M7）: `active_names.shin` `list.shin` `prelude_reach.shin`。

---

## 戦略

分割して征服する。builder は前ラウンドで出力予算を使い切ったので、フェーズ A → B → C に切る。各フェーズは `verify-m3.sh` が印字するマーカーで独立に測る。全体の合否は 12 項全部が OK のときだけ。

コーパス外の判定器（型推論の往復 20 本 + TMP プローブ）を必ず置く。拒否 9 本のパスと符号を暗記した実装、および型検査を素通しする実装が、両方とも落ちること。

---

## R5 に追加された作業（commander が plan-reviewer の指摘を受けて追加）

§50 に E205 / E305 / E306 を足したため、**拒否コーパスが 43 本から 46 本**になる。
`tests/reject/E205.shin` `E305.shin` `E306.shin` の 3 本を新設し、
`scripts/verify-m1-corpus.sh` が `PASSED: 7/7` に戻ること。

- `E205.shin`: どの名前空間でも解決できない名前を参照する（§47）
- `E305.shin`: 型が食い違う（§9）。例: `def main = 1 + true`
- `E306.shin`: 出現検査に失敗する（§9）

**1 ファイル 1 欠陥**。狙ったコード以外の理由で先に落ちないこと。
この 3 本に限り `tests/reject/` のロックを解除する（他のコーパスは触らない）。

## スコープ外

M4 網羅性・canonical（E4xx E5xx、E508 を含む）、M5 単相化（E7xx、`eq_T` / `compare_T` の合成）、M6 コード生成、M7 prelude 関数。`docs/plan.0.1.md` 182–218 行。

`==` `!=` `compare` が Fn を含む型に当たるかは単相化時（`docs/spec.0.1.md:1486`、E704/E705 は 1595–1596 行）。M3 ではその型付け（多相の `==` と `compare`）まで行い、E7xx は出さない。

`main` が多相・Fn 含み（E702/E703、`docs/spec.0.1.md:166`）も M5。M3 の `shin check` は型が付けば通す。

---

## 決定

### D1. フェーズ

| フェーズ | 中身 | 独立に測る中間 |
|---|---|---|
| A | 4 名前空間、module 階層、sig 隠蔽の骨、`__` 名、`TypeDef.params`、`shin check` CLI、`lower_to_m0` の余剰 `def` | フェーズ A 完了のマーカー |
| B | HM（unify / generalize / 出現検査 / ADT / record / generics / 再帰型の型引数） | 型往復 20、E301/E304、unify 系マーカー |
| C | signature matching と抽象型、組み込み多相、`stdlib/` の型定義のみの連結、受理 20・拒否 9 の完了 | 全体 `PASSED: 12/12` |

### D2. `shin check` の入出力（API 境界）

**`--print-type` を足す（commander が判定器の設計原則に合わせて追記）**:
`shin check --print-type <file>` は、型検査が通ったとき `main` の型を 1 行で標準出力に印字する
（`Int` `Bool` のようにスペースを含まない表記）。落ちたときは診断だけを出す。
チェック 6 がこの出力を使う。

コマンド: `shin check <file>`（ファイルは 1 つ）。

| 場合 | 終了コード | stdout | stderr |
|---|---|---|---|
| 型も名前も通る | 0 | 空（バイト数 0） | 空（バイト数 0） |
| parse または名前または型の失敗 | 1 | 空 | 診断 1 行 + 末尾改行 1 つ。2 行目以降を出さない |
| 引数欠落（`shin` のみ、または `shin check` のみ） | 2 | 空 | ちょうど `usage: shin check <file>` + 末尾改行 1 つ |

診断形式:

- 符号がある（E2xx/E3xx/E6xx および parse の E1xx）: `<path>:<line>:<col>: error[CODE]: <message>`（`parser.rs:292-296` と同じ。`<path>` は引数に渡した文字列）。
- **未束縛名は E205、unify 失敗は E305、出現検査の失敗は E306**（commander が §50 に追加）。
- 仕様の表になお無い失敗（Float の `%` 等）: 符号を発明しない。`<path>:<line>:<col>: error: <message>`（現状の `Display` None 枝 `parser.rs:298` は path を落とすので、M3 では path と位置を付ける）。

`<path>` は `shin check` に渡した引数をそのまま使う（`tests/reject/E201.shin` で呼べばその文字列が先頭に来る）。

公開 API（CLI が呼ぶ）:

- `pub fn check_source(source: &str, path: &str) -> Result<Checked, parser::Diagnostic>`
- `Checked` は少なくとも `main` の推論型を持つ（往復テストが読む）。`compile_to_object` は呼ばない。

`compile_to_object` は M0 経路のまま（`lib.rs:14-24`）。全文の型検査をここに繋がない。`parse_corpus.rs:146-148` が non-E1xx の失敗メッセージに部分文字列 `error` が無いことを要求しているため。

### D3. コーパスは既定でロック

`tests/accept` `tests/reject` `crates/shin/tests/corpus.rs` を builder は変更しない。

型検査が仕様どおりで、コーパスが仕様と食い違うとき: 型検査を緩めない。標準エラーに次を 1 行出して、そのファイルの `shin check` は終了コード 1 のまま止める（受け入れ 20 のチェックは NG になる）。

```
corpus-spec-mismatch: <path>:<line> <識別子> §<節>
```

`<path>` `<line>` `<識別子>` は実測した未束縛または矛盾の位置。`<節>` は仕様の節番号。コーパスを直して通すことは M3 の作業に含めない。commander がロックを外すまで M3 を完了扱いにしない。

### D4. E508 は M3 の対象外

`tests/reject/E508.shin` は `| Ok(user) => user` と `| Error(error) => Error(error)` で両枝の型が違う（3–7 行）。型検査が先に当たるのが仕様どおり（`docs/spec.0.1.md:790` の E508 は冗長であり型の後段、M4）。

verify-m3 の拒否ループの符号列はちょうど次の 9 個で、`E508` は 0 回:

`E201 E203 E204 E301 E302 E303 E304 E601 E602`

`shin check tests/reject/E508.shin` の終了コードは問わない。M4 着手時にコーパスを直す。

### D5. 余剰トップレベル `def` は M3 で直す

`lower_to_m0` は `ast.items` がちょうど 1 件、かつそれが `def main = <Int の加算>` のときだけ `Some`。余剰の `Item` があれば `None`（従来の `M3+` メッセージ、部分文字列 `error` を含めない）。

`shin check` は余剰 `def` を型検査する。未使用は E513（`docs/spec.0.1.md:891`、M4）なので、M3 の `shin check` は `def other = 1` と `def main = 1 + 2` を通す。落とすのは M0 の `lower_to_m0` / `compile_to_object` だけ。

### D6. prelude 型の名前は仕様に従う

`docs/plan.0.1.md:174` は `ArithmeticError` と書いている。仕様は `DivisionError`（`docs/spec.0.1.md:1093-1095`、1520 行。台帳 `docs/ledger.0.1.md:149` で改名済み）。stdlib と検査器は `DivisionError` を使う。`ArithmeticError` という型名は置かない。

言語が名前で解決する型は `Option` `Result` `List` `DivisionError`（1520 行）。コンパイラもこの 4 名を知る。constructor の payload は `stdlib/` の型定義を連結して入れる。関数の `def` は書かない（`docs/plan.0.1.md:174`「関数は M7」）。

### D7. 一般化・組み込み・module

- 一般化はトップレベルと struct 内の `def` だけ（`docs/plan.0.1.md:171`）。局所 `let` はしない。
- 算術 `+ - * / %` と順序比較 `< <= > >=` は Int / Float の組み込み多相。未確定なら Int。`/` `%` の戻りは §36（`docs/spec.0.1.md:1472`、1093–1097 行）。Float に `%` は置かない（符号なし診断）。
- `==` `!=` は Fn を含まない全型の構造比較として型を付ける（1484 行）。`compare` は値名前空間の組み込みで型 `Fn<(a, a), Int>`。E704/E705 は出さない。
- signature 注釈なしの `module` は E601（1052 行）。`sig` の `type T`（本体なし）は外から不透明。フィールド投影は E602（1070–1072 行）。
- `__` で始まる名前: `path` が `stdlib/` をコンポーネントに含むときだけ解決する。ユーザーコードは E203。呼び出し位置以外（値として渡す）は E204（1506 行）。ユーザーコードでは E203 と E204 が両方当たり得るとき、値化なら E204、呼び出しなら E203。根拠は拒否コーパス: `E203.shin:5` は呼び出し、`E204.shin:4` は `apply` の引数。

### D8. `verify-m2.sh` 条件 8

条件 8 は `git diff HEAD -- crates/shin/src/types.rs` が 0 行であることを見ている（`scripts/verify-m2.sh:232-233`）。M3 は `types.rs` を書き換えるので、この 1 述語は消す。代わりに `grep -c 'pub fn check' crates/shin/src/types.rs` が 1（M0 の `types::check` を残す）。条件 8 の残りの glue 述語（`lib.rs` の `pub fn compile_to_object` が 1、`pub use emit::compile_to_object` が 0、`M3+` が 1 以上、`emit.rs` の `lexer::lex` と `parser::parse` が 0）は残す。総数 8、末尾 `PASSED: 8/8` は変えない。

---

## ファイル

| パス | 操作 |
|---|---|
| `crates/shin/src/main.rs` | 作成。`shin check` |
| `crates/shin/Cargo.toml` | `[[bin]]` name `shin` path `src/main.rs` |
| `crates/shin/src/lib.rs` | `check_source` を公開。`compile_to_object` の M0 経路は維持 |
| `crates/shin/src/names.rs` | 作成。4 名前空間 |
| `crates/shin/src/types.rs` | HM。M0 の `pub fn check` は残す |
| `crates/shin/src/parser.rs` | `TypeDef` に `params: Vec<String>`。`lower_to_m0` の D5 |
| `crates/shin/tests/check_infer.rs` | 作成。往復・HM マーカー・名前空間マーカー |
| `stdlib/option.shin` `stdlib/result.shin` `stdlib/list.shin` `stdlib/division_error.shin` | 作成。型定義のみ |
| `scripts/verify-m3.sh` | 作成 |
| `scripts/verify-m2.sh` | D8 の 1 述語だけ置換 |

`check_source` はユーザーソースの前に `stdlib/*.shin` をファイル名順で連結する（型定義のみ。関数 `def` が無いこと）。

stdlib の中身（逐語。これ以外の `def` を足さない）:

`option.shin` — `docs/spec.0.1.md:292-295` の `type Option<T> = | None | Some(T)`
`result.shin` — 同 298–301 行の `type Result<T, E> = | Ok(T) | Error(E)`
`list.shin` — `type List<T> = | Nil | Cons(T, List<T>)`（`docs/spec.0.1.md:1492`）
`division_error.shin` — `type DivisionError = | DivisionByZero`（1095 行）

---

## フェーズ A — 名前解決と CLI

手順:

1. `TypeDef.params` を AST に残す。単体テスト: `type List2<T> = Nil2 | Cons2(T, List2<T>)` の params が `["T"]`。
2. 名前空間を 4 つ（型・module・constructor・値）。`docs/spec.0.1.md:1500`。`Option<T>` と module `Option` は共存できる（同 1500 行）。prelude constructor `Some` `None` `Ok` `Error` `Nil` `Cons` は修飾なし（1504 行）。登録は stdlib 連結後。A の単体テストはフィクスチャで登録してよい。
3. `Field(Name(m), x)`: `m` が module なら module 内の値。`m` が値なら record 投影（型は B）。
4. `__` は D7。
5. 自己再帰が `rec` 無しなら E201（`docs/spec.0.1.md:136`、`E201.shin:3`）。相互再帰の `rec` 欠落は E516（M4、1587 行）なので A では型付け用にトップレベル関数を一括で環境に入れ、E516 は出さない。
6. signature 無し module は E601（1052 行、`E601.shin:4`）。
7. `main.rs` と D2 の終了コード。
8. D5 の `lower_to_m0`。
9. D8 の `verify-m2.sh`。

フェーズ A 完了（`verify-m3.sh` のログで読む。全体はまだ NG でよい）:

- `OK: 1 cli` がある
- 次の 4 行がある: `m3_reject_ok: tests/reject/E201.shin E201` `...E203.shin E203` `...E204.shin E204` `...E601.shin E601`
- `lower_extra_def_ng: 1` がちょうど 1 行
- `ns_ok_n: 4` が 1 行以上
- `typedef_params_ok: 1` が 1 行以上

`ns_ok_n` と `typedef_params_ok` と `lower_extra_def_ng` は `cargo test --lib -- --nocapture` のログでよい。`check_infer` は B から。

---

## フェーズ B — HM

手順:

1. `Ty` を、Int/Float/Bool/Char/String/Unit、型変数、`Fn`（n 引数、`docs/spec.0.1.md:1125-1137`）、ADT、record、`Option`/`Result`/`List`/`DivisionError`、ユーザー型、抽象型（module の `T`）まで広げる。M0 の `Ty::Int` と `pub fn check` は残す。
2. unify、出現検査、`def` 単位の generalize / instantiate。
3. 再帰型の閉路で、型引数が宣言と同一でないなら E302（308–312 行、`E302.shin:4`）。
4. record 構築の型は field 集合から一意の nominal。2 つ以上あれば E304（373 行、`E304.shin:7`）。0 件は符号なし（D3）。
5. 適用の引数数が定義と一致しないなら E301（258 行）。`x |> f a b` は `f x a b`（`docs/spec.0.1.md:810`）。`E301.shin:6`。
6. `let?` の右辺が Option でも Result でもなければ E303（691 行、`E303.shin:4`）。`do?` の右辺も同じ制約（750 行、符号なし。E303 は `let?` にだけ付いている）。
7. 分岐の各腕の型は unify する。網羅性は M4。
8. List リテラルは `Cons` / `Nil` として型を付ける（1492 行）。constructor 表がまだ空でも、往復テストはリテラルを生成しない。
9. `crates/shin/tests/check_infer.rs` に往復 20 本（後述）。

フェーズ B 完了:

- `type_round_ok:` がちょうど 20 行、`type_round_n: 20` が 1 行以上
- `unify_ok` `generalize_ok` `occurs_ok` がそれぞれ 1 行以上
- `m3_reject_ok:` が E301 と E304 についてある

---

## フェーズ C — matching・組み込み多相・stdlib・完了条件

手順:

1. `stdlib/` 4 ファイルを置き、`check_source` が連結する。ユーザーが同名 `type Option` を定義したら符号なし重複。
2. struct が sig の全 `def` / `type` を満たすか。足りない・型が合わないは符号なし。余分な struct 内 `def` は隠蔽（E515 は M4、1074 行）。
3. 抽象型の外からの `.name` は E602（1072 行、`E602.shin:12`）。
4. 組み込み多相 D7。`1 + 2` は Int、`1.0 + 2.0` は Float、`fn x => x + x` の `+` は未確定なので Int（1472 行）。
5. 受理 20 本を `shin check` で通す。食い違いは D3。

フェーズ C 完了 = 下の 12 項がすべて OK。

---

## 型推論の往復（コーパス外）

`crates/shin/tests/check_infer.rs`。固定 seed `0x543E_5EED` の LCG（乗数・加算は `parse_corpus.rs:238-241` と同じ）。式木を 20 本。深さ最大 3。

生成してよいノード: `Int` `Float` `Bool` `Char` `Str` `Unit`、単項 `-`、`not`、二項 `+ - *`（左右が同じ数値型）、`< <= > >=`（左右が同じ数値型、結果 Bool）、`== !=`（左右が同じ、Int/Float/Bool/Char/String/Unit、結果 Bool）、`&&` `||`（Bool）。`/` `%`・名前・適用・record・List・分岐は生成しない。

各木は生成時に型を一緒に返す（oracle）。oracle は `check_source` も `unify` も呼ばない。ソースは `def main = <描画>`。`check_source` が返す `main` の型が oracle と `PartialEq`。一致なら `type_round_ok: <i> <ty>`。20 本終わったら `type_round_n: 20`。

---

## 判定器の設計原則（plan-reviewer の指摘を受けて commander が追加）

plan-reviewer は、**型検査を一切しない結託**（コーパスの内容そのものをキーにした辞書を
持ち、決め打ちの診断を返す実装）が 12 チェックすべてを通すことを実測した。

根は R4 と同じ:

> **判定器の入力がリポジトリに固定されている限り、暗記で満たせる。**

swap・perturb・ill-typed の各プローブは入力がスクリプトに固定されていたため、
内容キー辞書は変更後の内容も暗記できた。

**効く判定器は次の 2 条件をともに満たす:**

1. **入力を実行時に生成する。** リポジトリにも判定スクリプトにも、その入力が固定で
   書かれていない
2. **期待値の oracle を判定器側に置く。** 被験側（`crates/shin/**`）のコードに
   期待値を計算させない

R5 の当初案は「型推論の往復」を builder が書く `crates/shin/tests/check_infer.rs` に
任せていた。これは**被験者が自分の答案を採点する**形なので、判定として成立しない。

以下のチェックはこの原則に従って設計し直してある。

## 判定 — `scripts/verify-m3.sh`

リポジトリルートで実行。チェックは 1 から 12 まで独立、最後まで走らせる。失敗しても止めない。末尾は `PASSED: $ok/12`。`$ok` が 12 のときだけ終了コード 0。

`SHIN=target/debug/shin`。先に `cargo build -p shin --bin shin`。

各チェックは「何を数え、いくつなら OK か」だけ。any-of は使わない。禁止は構造マーカーか語の共起、要求は逐語。

### チェック 1: cli

数えるもの:

- `cargo build -p shin --bin shin` の終了コードが 0
- `test -x target/debug/shin` が真
- `$SHIN` の終了コードが 2、stdout バイト 0、stderr がちょうど `usage: shin check <file>\n`
- `$SHIN check` も同じ 3 点

4 つとも成立で OK。現状: バイナリ無しで build --bin は失敗。完了時: 4/4。出典: `docs/plan.0.1.md:19`（CLI 名 `shin`）、176 行（`shin check`）。

### チェック 2: accept-20

`tests/accept/*.shin` を列挙し、本文に行 `^-- needs: prelude` が無いファイルを配列に入れる。件数は 20。20 以外なら即 NG（ループしない）。

配列の各要素 `f` について:

- `$SHIN check "$f"` の終了コードが 0
- stdout バイト数が 0
- stderr バイト数が 0

ログに `m3_accept_ok: $f` を 1 行。`m3_accept_ok` の行数が 20 で OK。1 本でも欠ければ NG。現状 0/20。完了時 20/20。出典: `docs/plan.0.1.md:178`、147 行（`-- needs: prelude`）。

### チェック 3: reject-9

符号列を次の 9 個で固定する（この順）: `201 203 204 301 302 303 304 601 602`。

この列に `508` が出現する回数は 0（E508 除外、D4）。

各 `n` について `code=E$n` `f=tests/reject/$code.shin`:

- `$SHIN check "$f"` の終了コードが 1
- stdout バイト数が 0
- stderr がちょうど 1 行（末尾改行除く）
- その行が正規表現 `^tests/reject/$code\.shin:[0-9]+:[1-9][0-9]*: error\[$code\]: .+$` に一致

一致したら `m3_reject_ok: $f $code`。9 行そろい `m3_reject_ok_n: 9` が 1 行以上で OK。現状 0/9。完了時 9/9。

出典（ファイルと仕様）:

| ファイル | 欠陥行 | 仕様 |
|---|---|---|
| `tests/reject/E201.shin` | 3 | 136、1561 行 |
| `tests/reject/E203.shin` | 5 | 1506、1562 行 |
| `tests/reject/E204.shin` | 4 | 1506、1563 行 |
| `tests/reject/E301.shin` | 6 | 258、1564 行 |
| `tests/reject/E302.shin` | 4 | 312、1565 行 |
| `tests/reject/E303.shin` | 4 | 691、1566 行 |
| `tests/reject/E304.shin` | 7 | 373、1567 行 |
| `tests/reject/E601.shin` | 4 | 1052、1590 行 |
| `tests/reject/E602.shin` | 12 | 1072、1591 行 |

### チェック 4: swap

`tests/reject/E201.shin` を退避。先頭 2 行（`-- spec:` と `-- error: E201`）を残し、本文を `type Nest<T> = Leaf(T) | Node(Nest<List<T>>)\ndef main = 1\n` にする。`$SHIN check tests/reject/E201.shin` の stderr について:

- `error[E302]` の出現が 1 以上
- `error[E201]` の出現が 0

ファイルを復元。現状: CLI 無しで不可。完了時: 上記 2 述語。出典: チェック 3 と同じ E302（312 行）。形式の手本は `scripts/verify-m2.sh:67-78`。

### チェック 5: perturb

チェック 3 の 9 ファイルそれぞれ: `-- error:` で始まる行の直後に空行 1 つを挿入。`$SHIN check` の stderr 1 行目から `<line>` を取る。退避時の `<line>` + 1 と一致。9 本すべて一致で OK。復元。出典: チェック 3 の欠陥行、手本 `scripts/verify-m2.sh:139-151`。

### チェック 6: type-round-20（判定器側が生成し、判定器側が採点する）

**`scripts/verify-m3.sh` の中で入力を生成し、期待型も判定器が計算する。**
`crates/shin/tests/check_infer.rs` には依存しない（builder が書くものを judge に使わない）。

手順:

1. スクリプトが固定 seed の擬似乱数（`awk` か `python3` の線形合同法。seed は
   `date +%s` を混ぜず再現可能にする）で、**単相な式を 20 本**組み立てる。
   文法は Int リテラル・`+ - * `・比較 `< <= > >=`・`&& ||`・`not`・括弧。
   式ごとに期待型（`Int` か `Bool`）を**スクリプト側の規則**で計算する
   （算術は Int、比較と論理は Bool）
2. 各式を一時ファイル `$TMPD/round_<n>.shin` に `def main = <式>` として書き出す
3. `shin check --print-type $TMPD/round_<n>.shin` を実行し、出力の型が期待型と一致すること
4. **20 本すべて一致**。一時ファイルは `trap` で消す

述語: 一致した本数 == 20。

入力はスクリプト実行時に組み立てられ、リポジトリにも判定スクリプトにも固定で書かれていない。
期待型はスクリプトが計算する。内容キー辞書は、生成された式を事前に知り得ないので通らない。

### チェック 7: outside-corpus（ill-typed も実行時生成）

**型エラーになる式を実行時に生成する。** 固定で書かない。

1. スクリプトがチェック 6 と同じ生成器で、**型の合わない式を 10 本**組み立てる
   （Int の位置に Bool を、Bool の位置に Int を置く。例: `1 + (2 < 3)`、`not 1`、
   `(1 < 2) * 3`）。どの位置を壊すかも擬似乱数で選ぶ
2. `shin check` が **終了コード非 0** で落ち、診断に `error[E305]` が現れること
   （commander が plan-reviewer の指摘を受けて確定。仕様の §50 には型の食い違いを表す
   コードが無く、D2 の「符号を発明しない」と矛盾していた。**§50 に E205 unbound-name /
   E305 type-mismatch / E306 occurs-check を足した**ので、型の不一致は E305 を返す）
3. **10 本すべて**

述語: 落ちた本数 == 10。

### チェック 8: 構造 glue（`verify-m2.sh` のチェック 8 に相当）

型検査の経路が実際に呼ばれていることを構造で見る。要素ごとにループし、1 つでも欠けたら NG。

- `crates/shin/src/lib.rs` に `names::` の出現が 1 以上
- `crates/shin/src/lib.rs` に `types::` の出現が 1 以上
- `crates/shin/src/types.rs` に `fn unify` `fn generalize` `fn occurs` の 3 つが各 1 以上
- `crates/shin/src/names.rs`（または名前解決を担うファイル）が存在し、
  4 名前空間（型・module・constructor・値）を表す語がそれぞれ 1 以上
- **`crates/shin/src` 配下に、コーパスのファイル名（`E201.shin` 等 9 本）が
  文字列リテラルとして現れないこと**（パスキー辞書の禁止）
- **`crates/shin/src` 配下に、コーパスの本文から取った 20 文字以上の連続した断片が
  現れないこと**（内容キー辞書の禁止）。判定は、9 本の拒否コーパスと 20 本の受理コーパスの
  各行のうち長さ 20 以上のものを取り出し、`crates/shin/src` 全体に対して
  固定文字列検索でヒット 0 であること

### チェック 8b: unit-hm

`cargo test` の出力に、unify・generalize・出現検査・signature matching の 4 つを
名前に含むテストがそれぞれ 1 件以上あり、すべて ok であること
（`docs/plan.0.1.md:180`。要求なので逐語の語で照合してよい）。

さらに、**この 4 テストが判定力を持つ**ことを builder が確かめること:
それぞれ対応する実装を壊すと、そのテストが落ちる。

### チェック 9: gates

次を要素ごとに実行。1 つでも欠ければ NG。

- `cargo test` の終了コードが 0（現状 38 件通過。完了時も 0。件数は 38 以上でよい）
- `bash scripts/verify-m0.sh` 終了 0 かつ最終行がちょうど `PASSED: 7/7`
- `bash scripts/verify-m1-spec.sh` 終了 0 かつ最終行がちょうど `PASSED: 6/6`
- `bash scripts/verify-m1-corpus.sh` 終了 0 かつ最終行がちょうど `PASSED: 7/7`
- `bash scripts/verify-m2.sh` 終了 0 かつ最終行がちょうど `PASSED: 8/8`

現状: commander 実測どおり通過。完了時: 同じ 5 述語。出典: ユーザー要求。門番の末尾行は各スクリプトの最終 `echo "PASSED: ..."`（`verify-m2.sh:250`、`verify-m0.sh` は同型）。

### チェック 10: corpus-lock

`git diff --name-only HEAD -- tests/accept tests/reject crates/shin/tests/corpus.rs` の行数が 0。現状 0。完了時 0。出典: ユーザー「コーパスは既定でロック」。手本 `scripts/verify-m2.sh:222`。

### チェック 11: stdlib-types

次を要素ごとに。1 つでも欠ければ NG。

- ファイル `stdlib/option.shin` `stdlib/result.shin` `stdlib/list.shin` `stdlib/division_error.shin` がそれぞれ存在する（4）
- `grep -c 'type Option' stdlib/option.shin` が 1 以上
- `grep -c 'type Result' stdlib/result.shin` が 1 以上
- `grep -c 'type List' stdlib/list.shin` が 1 以上
- `grep -c 'type DivisionError' stdlib/division_error.shin` が 1 以上
- `grep -c '^def ' stdlib/*.shin` の合計が 0（関数を書いていない）

現状: ディレクトリ無しで 0。完了時: 上の 6 ブロックすべて。出典: `docs/spec.0.1.md:1520`、292–301、1095、`docs/plan.0.1.md:174`（関数は M7）。

### チェック 12: extra-def

`cargo test --lib -- --nocapture` のログに `lower_extra_def_ng: 1` がちょうど 1 行。そのテストはソース `def other = 1\ndef main = 1 + 2` について `lower_to_m0` が `None` かつ `compile_to_object` が `Err`。現状: `lower_to_m0` が `Some`（`parser.rs:1378-1381`）。完了時: 1 行。出典: `docs/ledger.0.1.md:385`。

---

## フェーズとチェックの対応

`verify-m3.sh` は常に 1–12 を最後まで実行する。フェーズの完了はログの部分だけを読む。

| フェーズ | ログで揃うもの |
|---|---|
| A | `OK: 1 cli`、reject_ok の E201/E203/E204/E601、`lower_extra_def_ng: 1`、`ns_ok_n: 4`、`typedef_params_ok: 1` |
| B | `type_round_ok:` ×20、`unify_ok` `generalize_ok` `occurs_ok`、reject_ok の E301/E304 |
| C | `PASSED: 12/12` |

---

## 審査強度

各チェックを壊す最小の変更。verify-m3 が NG になること。

| 注入 | 落ちるチェック |
|---|---|
| `main.rs` を消す | 1 |
| `shin check` が常に終了 0、出力空（素通し） | 3（9 本が exit 1 にならない）、7 の ill-typed |
| `shin check` が常に終了 1 | 2、7 の well-typed |
| 9 パスを辞書にして符号と行を返す（暗記） | 4（ヘッダ E201・本文 E302 なのに E201）、5（空行で行が動かない）、7 の TMP 上 E301 |
| 暗記に加え TMP の well-typed だけ通す | 6（生成木の型が合わない） |
| `check_infer.rs` が `check_source` を呼ばず `type_round_ok` を 20 回印字 | 6 は通過し得る → 残余 |
| `stdlib/` に `def length s => s` を足す | 11（`^def ` が 0 でない） |
| `tests/reject/E201.shin` を編集 | 10 |
| `lower_to_m0` の D5 を戻す | 12 |
| `types.rs` から `pub fn check` を消す | 9（verify-m2 条件 8） |
| E508 を拒否ループに入れる | 3（符号列に 508 が出現） |

素通しと 9 本暗記は、チェック 3+4+5+6+7 の連言で落ちる。どれか一つでは足りない（R4 の教訓）。

---

## 残余の限界

往復 20 本は、生成器が出す式の断片の型付けだけを強制する。ADT・module・`let?` は拒否コーパスと受理コーパスが担う。有限のテストは一般の検査器を強制できない。reviewer は `names.rs` と `types.rs` を読んで、マーカーだけを印字する結託が無いかを見る。

---

## 予測される停止（D3、推定）

受理 20 の完了条件は 20/20 のまま。次は閉じたプログラムになっていない候補。実測で未束縛または record 型 0 件なら、型検査を緩めず D3 の 1 行を出して止まる。除外リストに足さない（除外は E508 だけ、D4）。

| ファイル | 位置 | 識別子 | 節 |
|---|---|---|---|
| `tests/accept/classify.shin` | 7 | `Excellent` | §11、§39:1234 |
| `tests/accept/size.shin` | 5 | `Large` | §11、§39:1226 |
| `tests/accept/display.shin` | 5 | `Guest` | §11:339、§39:1209 |
| `tests/accept/name.shin` | 7 | `Guest` | §11:339 |
| `tests/accept/create_user.shin` | 5 | `parse` | §39:1187 |
| `tests/accept/delete_user.shin` | 5 | `find_user` | §39:1198 |
| `tests/accept/record.shin` | 8 | field 集合 `{name, age}` に対する型定義 0 件 | §12:373 |
| `tests/accept/display_name.shin` | 8 | `{ name = None }` の型定義 0 件 | §12:373 |
| `tests/accept/user_store.shin` | 8 | 型名 `Error` | §34:1025 |

---

## 完了条件

1. `shin check` が、`-- needs: prelude` を持たない受理 20 本全件を終了コード 0 で通す（確定、`docs/plan.0.1.md:178`）。D3 の停止が起きた場合は未完了。
2. 拒否 9 本が指定コードで落ちる（確定、同 179 行）。E508 は含めない（D4）。
3. unify、generalize、出現検査、signature matching の Rust 単体テストがある（推定、同 180 行）。チェック 8。
4. 既存門番 4 本と `cargo test` 全体が通り続ける（確定、チェック 9）。

全体の測定は `bash scripts/verify-m3.sh` が終了コード 0 かつ最終行 `PASSED: 12/12`。
