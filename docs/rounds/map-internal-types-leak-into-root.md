# prelude の `implement` の内部型が利用者の根の型と E205 で衝突するのを直す（map-internal-types-leak-into-root）

## 手順書

### 一周目の計画

| 要素 | 内容 |
|---|---|
| 意図 | prelude を使う根を書く hado の利用者が、`Map` の実装が内部に置く型と同じ名前（`T`・`MapColor`・`MapStep`）を自分の根で宣言できるようにする（推定）。いまは prelude の実装の内部型が利用者の根の型と E205 になり、利用者は名前を変えざるを得ない |
| 仮説 | E205 を出す経路は 2 つある。(1) `namespaces::nested` が実装の内部型を「その項目のディレクトリ＋根＋prelude」の可視型と比べる。(2) `type_realizations` が「連結後の全トップレベル型」を外の型とする。どちらも prelude と利用者を同じ根の名前空間に混ぜている。`prelude_items` の境界を「prelude の項目の外の型は prelude だけ」として両方に通せば、契約の抽象型 `(K, V) T` と実装の具象 `(K, V) T` の実体化（`abstracts`）を変えずに直る（推定） |
| 判定器 | issue の完了条件をそのまま使う。現在値: 利用者の `type T`/`MapColor`/`MapStep` が E205（`cargo test --offline --test map_internal_types` が 1 passed; 1 failed）、`verify-r60.sh` が `PASSED: 6/8`、`verify-map-by-builtin-compare.sh` は 9/9 のまま。目標値: 3 つとも `check` を通り、`verify-r60.sh` が 8/8、`verify-map-by-builtin-compare.sh` が 9/9、`cargo test --offline` が 0 failed、`run-gates.sh --tier full` が red なし |
| スコープ | 触る: `crates/hado/src/namespaces.rs`・`crates/hado/src/type_realizations.rs`・`crates/hado/src/names.rs`・`crates/hado/src/lib.rs`（`prelude_items` を渡す）・`crates/hado/tests/map_internal_types.rs`（新設）・`tests/accept/map_internal_type_names/`（新設）・本ファイル・`docs/issues/map-internal-types-leak-into-root.md` の削除・`docs/issues/implement-local-adt-identity-is-name-only.md`（新設）。触らない: `docs/spec.md`・`stdlib/map.hd`（内部型 `T` は契約の抽象型の実体化なので改名できない）・`tests/corpus.sha256`（着地時に作り直す）・`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` |
| 未確認 | `has` を prelude の項目で prelude の表に限定すると、prelude 内の名前解決（`Some`/`None`/`List.reverse`/`__` 名）が変わらないか。`type_realizations` の外の型を「利用者の実装は prelude＋利用者、prelude の実装は prelude だけ」にして r16 の E205 が保たれるか。`stdlib/map.hd` を単独で check する経路（`is_stdlib_source`）で境界が正しく効くか |

### 背景

`stdlib/map.hd` の `implement Map as Map` は、契約 `module Map` が宣言する抽象型 `(K, V) T` を実体化する `type (K, V) T = | MapLeaf | MapNode(...)` と、契約に載らない内部型 `MapColor`・`MapStep` を持つ。§47・§49 はこれらを「その実装だけの局所的な空間」に置く。ところが prelude は `crates/hado/src/lib.rs` の `with_prelude` が利用者の項目の前に連結するので、実装の内部型を外の型と比べる 2 つの検査が、利用者の根の型まで「外」と見なしていた。

`crates/hado/tests/check_adts.rs` が `type Color` を使って `map-by-builtin-compare` の着地で赤くなり、そのときは内部型に `Map` を前置して避けた（`docs/rounds/map-by-builtin-compare.md`）。契約の抽象型と同じ名前の `T` は前置できず、`MapColor`・`MapStep` も前置しただけで漏れ自体は残っていた。

### 決定

- 決定 1: 境界は `prelude_items`（連結後の prelude の項目数）で表す。これは `ParsedSource` が既に持ち、`namespaces::resolve` と `infer` が使っている。新しい欄や綴りは足さない。
- 決定 2: `crates/hado/src/namespaces.rs` の `Resolver::has` を、prelude の項目を解決している間は prelude の表（`self.prelude`）に限定する。実装の内部型を外の型と比べる `nested` の検査も、名前を引く他の経路も、同じ 1 つの境界を通る。呼び出し側ごとに「prelude なら除く」を足す形にはしない。
- 決定 3: `type_realizations::check` に `prelude_items` を渡し、`implement` が外の型と見なすトップレベル型を、その実装が属する単位に閉じる。prelude の実装は prelude の型だけを、利用者の実装は prelude と利用者の型を見る（利用者の実装が prelude の公開型と衝突する検査は緩めない）。
- 決定 4: 契約の抽象型と実装の具象型の実体化（`abstracts`）は変えない。`type (K, V) T = | ...` は実体化であり、E205 の対象ではない。
- 決定 5: `stdlib/map.hd` の内部型名は変えない。`find_actual_type` が実体化を名前で突き合わせるので、`type (K, V) T` を `MapTree` に改名すると契約の抽象型 `T` が実体化されず E602 になる（issue の次点案は実現できない）。

### 期待値の根拠

issue の完了条件をそのまま検査にする。Red は `crates/hado/tests/map_internal_types.rs` の失敗で表す。受理コーパスは `type T`・`type MapColor`・`type MapStep` を宣言して 3 つとも使う 1 ファイルで、`check` と実行の両方を通す。既存の門番 `verify-r60.sh` が同じ欠陥を `type T` の fixture で測っている（60-d・60-g）。

### 成果物

- `crates/hado/src/namespaces.rs`: `Resolver.in_prelude` と、prelude の項目で prelude だけを見る `has`。
- `crates/hado/src/type_realizations.rs`: `check` の `prelude_items` と、実装が属する単位で外の型を求める `outer_types`。
- `crates/hado/src/names.rs`・`crates/hado/src/lib.rs`: `prelude_items` を `type_realizations` まで渡す。
- `crates/hado/tests/map_internal_types.rs`（新設）: 受理（3 つの名前）と、利用者の実装が外の型と衝突する E205 を測る。
- `tests/accept/map_internal_type_names/main.hd`（新設、`// needs: prelude`）。
- 本ファイル。`docs/issues/map-internal-types-leak-into-root.md` の削除と、見つけた別の欠陥の `docs/issues/implement-local-adt-identity-is-name-only.md`（新設）。

### 工程

1. Red: 3 つの名前を宣言して使うプログラムが `check` を通ることを測る失敗テストを書く。
2. Green: `namespaces::has` を prelude の項目で prelude 限定にし、`type_realizations` の外の型を実装の単位に閉じる。
3. 受理コーパスを足し、`print-postfix` の正準形にする。
4. ラウンドの最後に 1 回だけ `cargo test --offline`・`run-gates.sh --tier quick`・`--tier core`・`--tier full`・`lint-docs.sh`・`verify-spec-contracts.sh` を回す。
5. 着地の直前に main を取り込み、`corpus-lock.sh` を作り直し、そのラウンドの門番と quick 層を回す。
6. `docs/issues/map-internal-types-leak-into-root.md` を消し、push して PR にする（マージしない）。

### 判定

issue の完了条件ごとの検査。

1. 3 つの名前が E205 にならない → `cargo test --offline --test map_internal_types`、`tests/accept/map_internal_type_names`。
2. `verify-r60.sh` が 8/8。
3. `verify-map-by-builtin-compare.sh` が 9/9（検査 a の E204 と検査 g の §55 の型を弱めない）。
4. `cargo test --offline` が 0 failed、`run-gates.sh --tier full` が red なし。

### 許容される赤

なし。ただし判定 4 の `--tier full` は、main が既に持つ `verify-r62.sh`・`verify-r63.sh`・`verify-r68.sh`（このラウンドが見つけた別の欠陥）と `verify-string-split-linear.sh`（既知の揺らぎ）で赤いままである。3 本は main でも赤（E205）で、このラウンドの前後で緑から赤になった門番は無い。

### スコープ外

- `stdlib/map.hd` の内部型名の改名（決定 5。`find_actual_type` が名前で実体化を突き合わせるため E602 になる）。
- `implement` の局所型と根の同名 variant が型検査で混ざる欠陥（`docs/issues/implement-local-adt-identity-is-name-only.md` に切り出した。型の同一性を実装名で修飾する別のラウンド）。
- E205 の検査を緩める案（issue の退ける案）。
- §47・§49・§55 の本文の変更。仕様の解釈はこの 3 節の逐語から導いた。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- `has` の限定が prelude 内の名前解決を壊した。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §47 | 実装の中の ADT と record は実装ごとの局所名前空間にあり、外の型と同名なら E205 | 名前解決 | E205 | `crates/hado/tests/map_internal_types.rs`、`crates/hado/tests/r16_sections.rs` |
| §49 | `implement` の中の型はその実装だけの局所的な空間にあり、外の型と同名なら E205 | 名前解決 | E205 | 同上 |
| §55 | prelude は全プログラムに連結され、全ての検査を受ける | 名前解決 | — | `tests/accept/map_internal_type_names` |

## 実測

### Red（main の現象）

着手時の main は `bb3dbd0`。issue の再現は prelude を使う根で E205 になる。作業場所の作業ディレクトリ直下で `check` すると `editors/tree-sitter-hado` が §51 の名前空間ディレクトリとして不正で読む前に落ちるため、1 つ下の書き場所で測った（issue の行番号 235 は連結後の座標）。

```
$ printf 'type T = { a: Int }\ndef main = 0\n' > target/leak/main.hd
$ cargo run --offline -q -- check target/leak/main.hd
main.hd:239:13: error[E205]: local data type conflicts with an existing type
$ printf 'type MapColor = { a: Int }\ndef main = 0\n' > target/leak/main.hd
$ cargo run --offline -q -- check target/leak/main.hd
main.hd:238:6: error[E205]: local data type conflicts with an existing type
```

`bash scripts/verify-r60.sh` は 6/8。

```
NG: 60-d the field of a record literal
NG: 60-g one report per run
PASSED: 6/8
```

失敗するテストは 1 passed; 1 failed。

```
$ cargo test --offline --test map_internal_types
test a_user_implementation_type_still_conflicts_with_an_outer_type ... ok
test prelude_implementation_types_do_not_conflict_with_root_types ... FAILED
Some(Diagnostic { path: "main.hd", line: 242, col: 6, ..., code: Some("E205"), message: "local data type conflicts with an existing type", ... })
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

### Green（このラウンドの成果物）

```
$ cargo test --offline --test map_internal_types
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ bash scripts/verify-r60.sh
PASSED: 8/8
$ cargo run --offline -q -- run tests/accept/map_internal_type_names/main.hd
6
$ cargo test --offline --test corpus
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ cargo test --offline --test postfix_close
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ bash scripts/verify-map-by-builtin-compare.sh
OK: a surface: internal constructors and defs are unreachable (E204)
OK: g spec: §55 types the Map members and fold/to_list are ascending
PASSED: 9/9
```

`verify-map-by-builtin-compare.sh` の検査 a は内部の 6 constructor と 52 def が E204 のままであることを、検査 g は §55 の 6 member の型と昇順を測っている。どちらも弱めていない。

### 実装から分かったこと（仮説との突き合わせ）

- **当たり**: E205 は 2 経路から出ていた。`type_realizations` だけを直しても `namespaces::nested` が「local data type conflicts with a visible outer type」を出し続ける（`type_realizations::check` の呼び出しを一時的に外して確認）。観測される 1 行は同じアンカーを指す 2 件が診断の重複除去で畳まれた先頭だった。境界を 1 つ（`prelude_items`）にして両方に通す形で直った。
- **当たり**: `has` を prelude の表に限定しても prelude 内の名前解決は変わらなかった。受理コーパス全件（prelude を連結して実行する）と `postfix_close`、`cargo test` の全 suite が緑。
- **外れ**: issue の「`T` を `MapTree` に」はそのままでは実現できない。`crates/hado/src/infer.rs` の `find_actual_type` が signature の抽象型と実装の型を名前で突き合わせるので、改名すると契約の抽象型 `T` が実体化されず E602 になる。名前空間の直しが本命で、改名は不要だった。
- **外れ**: 「実装の内部型の名前が根へ出る」という向きではなく、実際は「利用者の根の型が prelude の実装の外の型として見える」向きだった。`namespaces` は実装の内部型を `symbols` に登録しておらず、`type_realizations` と `nested` が `symbols` の根の表（prelude と利用者が同居）を参照していた。

### 見つけた別の欠陥（このラウンドの外）

record の `type T` は通るが、variant の `type T = | A | B` は通らない。名前解決の漏れが消えた結果、その奥にあった型検査の欠陥へ届くようになった。利用者の constructor が prelude の `implement Map as Map` の `type (K, V) T` と同じ `Ty::Adt("T")` に束なり、prelude の網羅性が E402 になる。

```
$ printf 'type T = | A | B\ndef main =\n  if A | A => 1 | _ => 2\n' > target/probe-var-T/main.hd
$ cargo run --offline -q -- check target/probe-var-T/main.hd
main.hd:247:1: error[E402]: not all constructor values are covered
```

`MapColor` は `main.hd:252:1`、`MapStep` は `main.hd:281:16` を指す（いずれも `stdlib/map.hd` の実装の行）。名前が重ならない `type U = | A | B` は `E503` の判定へ進む。同じ族の門番 `verify-r62.sh` を、このラウンドの 4 ファイルを main のままにした木と、直した木の両方で回して切り分けた。

```
=== r62 WITH this round's fix ===
NG: 62-d the earlier rewrites still apply
PASSED: 1/4
=== r62 WITHOUT this round's fix (files at main) ===
main.hd:242:13: error[E205]: local data type conflicts with an existing type
NG: 62-d the earlier rewrites still apply
PASSED: 1/4
```

main でも赤（E205）で、このラウンドで緑から赤になった門番ではない。型の同一性を実装名で修飾する別のラウンドとして `docs/issues/implement-local-adt-identity-is-name-only.md` に切り出した。

### 決定に還元したこと

- 境界は既存の `ParsedSource::prelude_items` に一本化した。呼び出し側で「prelude なら除く」を足す形は、経路ごとに穴埋めが漏れるので採らなかった。
- prelude の項目の名前引きを `has` に閉じたので、今後 `nested` 以外の検査が根の表を引いても利用者の名前は prelude へ届かない。
- variant の E402 は型検査の別の欠陥なので、このラウンドでは直さず issue に切った。仕様（§47・§49）の解釈は変えていない。

### ラウンドの最後の全走

main（`dd63bc0`、cranelift-opt-level の着地後）を取り込み、corpus lock を作り直した木で回した。

```
$ cargo test --offline
CARGO_TEST_RC=0
$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates-core
TALLY: 716/716 passed; red: none
$ bash scripts/run-gates.sh --tier full target/gates-full
TALLY: 3837/3845 passed; red: verify-r62.log verify-r63.log verify-r68.log verify-string-split-linear.log
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
```

`--tier full` が回す core 層の門番も緑である（`verify-m0` 8/8（`cargo-test passed=791`）・`verify-m1-corpus` 7/7・`verify-m4` 173/173・`verify-m5` 118/118・`verify-m6` 144/144・`verify-clippy` 2/2・`verify-localize-message-positions` 3/3・`verify-lsp-minimal` 12/12）。

### main の赤

`--tier full` の赤は 4 本で、いずれもこのラウンドの成果物に由来しない。

- `verify-r62.sh`・`verify-r63.sh`・`verify-r68.sh`: fixture の `type T = | A | B` が、このラウンドの直した E205 を通り抜けた先で、上に切った型同一性の欠陥（E402）に当たる。main では同じ fixture が E205 で赤い。
- `verify-string-split-linear.sh`: 既知の揺らぎ（`docs/issues/flaky-string-split-timing-gate.md`）。検査 i の速さだけが落ちる。
