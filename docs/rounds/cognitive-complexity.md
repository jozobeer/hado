# clippy の認知複雑度を 7 に絞る（cognitive-complexity）

## 手順書

### 背景

- Rust の lint は `scripts/verify-clippy.sh` が守るが（`docs/rounds/rust-lint.md`）、
  `cargo clippy --offline --all-targets -- -D warnings` は既定の lint 集合を見るだけで、
  認知複雑度（`clippy::cognitive_complexity`）は allow のままである。rust-lint は
  `clippy.toml` の新設を明示的にスコープ外としていた。
- 実測（origin/main `6a797ad` の worktree、`clippy.toml` に
  `cognitive-complexity-threshold = 7` を置いた状態）: `cargo clippy --offline --lib --
  -W clippy::cognitive_complexity` は **64 件**の警告を出す。テストコード
  （`#[cfg(test)]` の内側）を除いた本番の関数だけを数えている。
  `--all-targets` で見るとさらに 38 件（`tests/` の 14 と `src` 内のテストの 3、
  および `tests/unit/parser.rs` の 1、残りは重複）が加わる。
- 最上位は `fixes.rs` の `walk_node`（35）、`infer.rs` の 171 行の式（28）、
  `names.rs` の `declaration_types`（27）、`namespaces.rs` の `item`（25）、
  `infer.rs` の `ordinary_module`（23）、`emit.rs` の `build_mono_expr`（22）。
- 閾値を 9 にすると本番の対象は 46 件、7 では 64 件である。

### 決定

1. `clippy.toml` を新設し `cognitive-complexity-threshold = 7` を置く。
2. 対象は**本番のコード（`src/` のうち `#[cfg(test)]` の外側）に限る**。テストは
   対象外とする。`tests/` と `src` 内のテストモジュールは現状の複雑度のままでよい。
   したがって門番は `--lib` に認知複雑度を掛け、`--all-targets` は現行どおり
   既定の lint 集合に掛ける（テスト側の複雑度は判定しない）。
3. リファクタは**振る舞いを変えない**。`cargo test` の判定が前後で同じであることを測る。
   テストとプロダクションは同時に書き換えない。
4. 7 で green にできない関数が出たら、閾値を 9 に落とすのではなく、まず分割で解く。
   どうしても解けない場合だけ報告して相談する（閾値を黙って緩めない）。

### 期待値の根拠

| 形 | 期待 | 根拠 |
|---|---|---|
| `cargo clippy --offline --lib -- -D clippy::cognitive_complexity` | 警告 0 で exit 0 | 決定 1・2 |
| `cargo clippy --offline --all-targets -- -D warnings` | 警告 0 で exit 0 | 既存の core 門番を緩めない |
| リファクタ前後の `cargo test` | 同じ判定 | 決定 3 |
| `cargo fmt --all --check` | 差分なし | 既存の quick 門番 |

### 成果物

`clippy.toml`（新設）、`scripts/verify-cognitive-complexity.sh`（新設）、
`docs/rounds/cognitive-complexity.md`（本ファイル）、`crates/hado/src/` の 64 関数の分割。

### 工程

- 工程 1: `clippy.toml` を置き、作業リストを固定する（済）。
- 工程 2: 複雑度 16 以上の関数を分割する。
- 工程 3: 複雑度 10〜15 の関数を分割する。
- 工程 4: 複雑度 8〜9 の関数を分割する。
- 工程 5: 門番を足し、`cargo test`・quick・core を回して実測を書く。

### 判定

- cc-a. `cargo clippy --offline --lib -- -D clippy::cognitive_complexity` が警告 0 で終わる。
- cc-b. `cargo clippy --offline --all-targets -- -D warnings` が警告 0 で終わる。
- cc-c. `bash scripts/verify-cognitive-complexity.sh` が `PASSED: n/n`。
- cc-d. `cargo test --offline` が 0 failed（リファクタ前と同じ）。
- cc-e. `cargo fmt --all --check` が差分なし。
- cc-f. quick 層と core 層が緑。
- cc-g. 否定的対照: 複雑度 8 以上の関数を 1 つ戻すと cc-c が赤になる。

### 許容される赤

工程 5 で本ファイルの `## 実測` を書くまで `scripts/lint-docs.sh` 検査 6 は赤でよい。
**終了時に許容される赤は無い。**

### スコープ外

言語機能、`tests/`・`stdlib/`・`runtime/` の変更、テストコードの複雑度、
`--all-targets` の既定 lint 集合、`docs/plan.md` と `docs/ledger.md` の改訂。

### 中断の条件

分割が `cargo test` の判定を変えるなら、allow や `#[allow]` で隠さずに止めて報告せよ。
同じ関数を 2 度分割しても 7 を割らないなら、閾値の判断を人に上げて止まる。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 本番のコードは認知複雑度 7 以下に保つ | 道具 | 成功 | `clippy.toml`・`scripts/verify-cognitive-complexity.sh` |

## 実測

先行分（`walk_node`・`lexer`・`repr`・`dependencies`・`emit`・`infer`・`mono`
ほか）を差分でレビューし、振る舞いが変わりうる 4 件を見つけて直した。
`source_print` の末尾 `Nil` 判定、`infer` の `current_definition` の戻し忘れ、
`fixes` の畳み込み失敗時のフォールバック、`dependencies` の `M.field` の
子走査と handler 値引数の型走査。いずれも原典の制御フローに戻した。

```
# 変更前: origin/main 6a797ad に clippy.toml を置いた一時 worktree。
# `--lib` には `#[cfg(test)]` の内側は現れない。
$ cargo clippy --offline --lib --message-format short -- -W clippy::cognitive_complexity 2>&1 \
    | grep -c 'cognitive complexity of'
64

$ cargo clippy --offline --lib --message-format short -- -W clippy::cognitive_complexity 2>&1 \
    | grep -oE 'cognitive complexity of \([0-9]+' | grep -oE '[0-9]+' | sort -n | uniq -c
      7 8
     14 9
      4 10
      7 11
      6 12
      5 13
      4 14
      3 15
      2 16
      1 17
      1 18
      1 19
      2 20
      1 21
      1 22
      1 23
      1 25
      1 27
      1 28
      1 35
```

```
$ cargo clippy --offline --lib --message-format short -- -W clippy::cognitive_complexity 2>&1 \
    | grep -c 'cognitive complexity of'
0

$ bash scripts/verify-cognitive-complexity.sh
OK: cargo clippy --offline --lib -- -D clippy::cognitive_complexity is clean
OK: no #[allow(clippy::cognitive_complexity)] under crates/hado/src
PASSED: 2/2
```

```
$ cargo test --offline > /tmp/t.log 2>&1; echo exit=$?
exit=0
$ grep -c 'test result: ok' /tmp/t.log
66
$ grep -c 'test result: FAILED' /tmp/t.log
0
$ awk '/^test result/ {p+=$4; f+=$6} END {print "passed="p" failed="f}' /tmp/t.log
passed=818 failed=0
$ tail -3 /tmp/t.log
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```
$ bash scripts/verify-clippy.sh
OK: cargo clippy --offline --all-targets -- -D warnings is clean
OK: clippy-driver --crate-type staticlib -D warnings on crates/hado/float_format.rs is clean
PASSED: 2/2

$ bash scripts/verify-rustfmt.sh
OK: cargo fmt --all --check reports no diff
OK: rustfmt --check crates/hado/float_format.rs reports no diff
PASSED: 2/2
```

```
$ bash scripts/run-gates.sh --tier quick /tmp/cc-quick
RUN: build -> /tmp/cc-quick/build.log
RUN: verify-ci -> /tmp/cc-quick/verify-ci.log
RUN: verify-gate-cache -> /tmp/cc-quick/verify-gate-cache.log
RUN: verify-m1-spec -> /tmp/cc-quick/verify-m1-spec.log
RUN: verify-rustfmt -> /tmp/cc-quick/verify-rustfmt.log
TALLY: 38/38 passed; red: none
# exit 0

$ bash scripts/run-gates.sh --tier core /tmp/cc-core
RUN: verify-ci -> /tmp/cc-core/verify-ci.log
RUN: verify-gate-cache -> /tmp/cc-core/verify-gate-cache.log
RUN: verify-m1-spec -> /tmp/cc-core/verify-m1-spec.log
RUN: verify-rustfmt -> /tmp/cc-core/verify-rustfmt.log
RUN: verify-clippy -> /tmp/cc-core/verify-clippy.log
RUN: verify-cognitive-complexity -> /tmp/cc-core/verify-cognitive-complexity.log
RUN: verify-m0 -> /tmp/cc-core/verify-m0.log
RUN: verify-m1-corpus -> /tmp/cc-core/verify-m1-corpus.log
RUN: verify-m2 -> /tmp/cc-core/verify-m2.log
RUN: verify-m3 -> /tmp/cc-core/verify-m3.log
RUN: verify-m4 -> /tmp/cc-core/verify-m4.log
RUN: verify-m5 -> /tmp/cc-core/verify-m5.log
RUN: verify-m6 -> /tmp/cc-core/verify-m6.log
RUN: verify-m7 -> /tmp/cc-core/verify-m7.log
RUN: verify-r38 -> /tmp/cc-core/verify-r38.log
TALLY: 575/575 passed; red: none
# exit 0
```

```
# 否定的対照 1: ambiguous_declarations.rs を origin/main の collect（複雑度 10）に戻す
$ bash scripts/verify-cognitive-complexity.sh
error: the function has a cognitive complexity of (10/7)
NG: cargo clippy --offline --lib -- -D clippy::cognitive_complexity is clean
OK: no #[allow(clippy::cognitive_complexity)] under crates/hado/src
PASSED: 1/2
# exit 1

# 否定的対照 2: contract_types.rs に #[allow(clippy::cognitive_complexity)] を注入する
$ bash scripts/verify-cognitive-complexity.sh
OK: cargo clippy --offline --lib -- -D clippy::cognitive_complexity is clean
NG: no #[allow(clippy::cognitive_complexity)] under crates/hado/src
PASSED: 1/2
# exit 1
```

### code-complete の一巡（事前分割ぶん 57 関数）

`code-complete` を当てる前に分割していた 57 関数を、同じループ（探索→モデル化・仮設計→Red-Green-Refactor→学習）でやり直した。実装は既存の分割を出発点にし、差分の再導出はしていない。origin/main `6a797ad` で 7 を超えていた 64 関数のうち、このラウンドで `code-complete` を通した 7 関数（`contract_types::collect`・`ambiguous_declarations::collect`・`type_realizations::check_module`・`normalize::rename`・`dependencies::expression`・`dependencies::type_expression`・`infer/functors::instantiate_template`）を除く 57 関数が対象である。

- 意図: 本番コードの認知複雑度を 7 以下に保ち、分割で振る舞いを変えない。
- 仮説: 関数ごとに「分けた境界」の列へ 1 行で書く。
- 判定器: `bash scripts/verify-cognitive-complexity.sh` が `PASSED: 2/2`、`cargo test --offline` が 0 failed。どちらも達成済み（上の実測）。
- スコープ: `crates/hado/src/` の本番コード。`tests/`・`stdlib/`・`runtime/`・テストコードは触らない。
- 未確認: 分割が評価順・短絡・早期 return・状態の戻し・引数の対応を変えていないか。origin の該当範囲と現在のコードを突き合わせた結果を「学習」の列に置く。
- Red: skip。振る舞いを変えない分割なので新しい失敗テストは置かない。現状の振る舞いは既存の `cargo test --offline`（818 passed / 0 failed）が固定する。
- Green: 下表の分割（既存の実装）。
- Refactor: ヘルパー構成は下表のとおり。点検で見つけた内部構造の問題は「学習」に書く。レビューで見つけて直したのは `infer` の `current_definition` の戻し、`fixes` の畳み込み失敗時のフォールバック、`source_print` の末尾 `Nil` の述語の 3 件（ほかに `dependencies` の handler 頭と子走査の分離）。`walk_node` の子の二重走査は origin 由来で、分割では変わっていない。

#### `crates/hado/src/check.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `check_effect_rows` | 9 | row の重複診断を `duplicate_effect`、型の形ごとの子の列挙を `type_children`（Apply/Group は `args`、Record は `field.ty`、Adt は `payload.types()`）に分けた。 | row 内は「`effect_key` で重複判定 → 再帰」の順、`type_children` は variant→payload の順で origin と同じ。`TypeKind::Apply` の第 1 フィールド（名前）を子に混ぜず `args` だけを降りる点、`duplicate_effect` が `ty` でなく row 内の `effect` の span を使う点を確認。未確認: clippy の複雑度実測。 |
| `check_node` | 14 | `fn` 引数の検査を `check_fn_node`、束縛の使用検査を `check_bindings`、ノード単位の 7 検査を `NODE_CHECKS`（`NodeCheck` 型の配列）、子の直接条件の判定を `condition_is_direct` に分けた。 | 検査順（boolean form → `check_fn_node` → `check_bindings` → `NODE_CHECKS` の 7 つ → 子の再帰）と `NODE_CHECKS` の並びが origin と同一で、診断が出る順が変わらないことを確認。子へ渡す `direct_condition` は `arm_condition \|\| (Fn かつ子の span が親と同一)` で origin の `... \|\| arm_conditions` と同値。危険は見つからず。 |
| `check_local_redundancy` | 9 | E502 を `identical_branches`、腕の catch-all を `catch_all_rows`、`let _?` の E504 を `discarded_binding`、`let _` の E506 と素通しの E501 を `returned_binding` に分け、`?` で最初のエラーだけ返す。 | origin は段ごとに早期 return していたので、`?` の連鎖でも「後段が走らない」ことは同じ。`returned_binding` が E506 を E501 より先に判定する順、`catch_all_rows` が `catch_all_row` の Err で残りの腕を打ち切る点も一致。危険は見つからず。 |

#### `crates/hado/src/dependencies.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `diagnostics` | 13 | `Inspection` ごとの根の列挙を `roots`、handler の効果と腕の閉包を `close_over_handlers`、E513 の宣言走査を `unused_declarations`、module ごとの local data と E521 を `module_diagnostics` に分けた。 | `roots`→`close_over_handlers`→`unused_declarations`→`module_diagnostics` の順に `used` が育ち、診断順が origin と同じ「E513 → 各 module の local data → E521」であることを確認。`skip(prelude_items)` が `unused_declarations` と `module_diagnostics` の両方に残る点も一致。未確認: clippy の複雑度実測（cargo 実行は禁止）。 |
| `item` | 15 | 7 つの腕を `type_definition` / `signature_definition` / `module_implementation` / `effect_definition` / `handler_definition` に振り分け、handler は頭（`extend_types` での型引数、`Key::Effect`、値引数の型）と腕の走査 `handler` に分けた。module 本体の scope は `extend_signature_scope`。 | 型引数の取り違え（`def.params` と `def.type_params`、`effect.type_params`、`handler.type_params`）が origin と一致。型引数を足した scope で値引数の型を歩き、`Key::Signature` を本体より先に入れる順も同一。handler の腕が head の scope をそのまま受け取る（腕から型引数が見える）点も保たれている。危険は見つからず。 |

#### `crates/hado/src/emit.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `build_mono_expr` | 22 | CallRuntime を `build_runtime_call`、Switch を `build_switch`、Construct を `build_construct`、Record を `build_record`、MakeClosure を `build_make_closure`、Unary を `build_unary`、Binary を `build_binary` に移し、本体は match dispatch だけにした（Project/RecordGet/Let/Branch は残置）。 | `build_binary` は `left.ty()` から `float`/`signed`/`string` を left/right の build 前に読む（origin と同じ）。`build_unary` の `float`、`build_switch` の `value.ty()` とタグ分岐も build 前。`build_record` の `record_field_index`→build、`build_construct` の field 数検査→確保→タグ→offset 格納、`build_make_closure` の capture 型→layout→値→func_addr→格納の命令順も同一。`prepare_direct_call`／`prepare_closure_call` に差分なし。危険なし。 |

#### `crates/hado/src/fixes.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `catch_all_variable_fix` | 19 | 最後の腕の選択を `catch_all_arm`、scrutinee 名の確認を `catch_all_scrutinee`、書換範囲と `let`・コメント検査を `catch_all_range`、`x =>` と scrutinee の綴りを `catch_all_binding_value`、本体のトークン置換を `substitute_binding`、§41 の畳み込み判定を `arms_reconstruct` / `arm_reconstructs`、畳み込みの Fix 生成を `catch_all_folded_fix` に分けた。 | 順序は `start` → `catch_all_range` → `catch_all_binding_value` → `substitute_binding` で origin と一致。畳み込みが失敗したとき素の E512 に落ちる fall-through も保持（ここを `?` に変えると壊れる）。`arms_reconstruct` は綴り不一致を先に弾き、腕数不一致と位置取得失敗（`arm_reconstructs` の `None`）を false に畳む点が origin の `reconstructs` と同じ。 |
| `literal_pattern_fix` | 11 | 腕ごとの `x ==` 走査と共通左辺の確定を `literal_arms`、`head` から名前を落とす処理を `literal_prefix`、`cuts` を使う再構築を `literal_replacement` に分けた。 | `literal_arms` が `name?` を返してから `cuts[0]` を使う順と、`literal_replacement` の `cursor` 前進・`cut_end > end` 検査・`prefix.len() + 1 >= head.len()` の位置が origin と一致。`literal_prefix` は `head` の部分スライスを返すライフタイム `'a` で、所有権の取り違えはない。 |
| `catch_all_fallback_fix` | 10 | 入れ子外側の `=>` を探す深さ走査を `top_level_arrow`、`=>` 後の空白飛ばしを `skip_blanks` に切り出した。 | 深さ 0 の `Arrow` で止まり、`Eof` とトークン尽きで `None` になる点が origin の `break` → `end?` と同じ。`=>` 長の `+2` を足してから `skip_blanks` を掛ける順も同じ。 |
| `condition_arm_fix` | 13 | 同じ `top_level_arrow` を共有し、`=>` 前の空白落としを `trim_blanks_back`、`if` 後の空白飛ばしを `skip_blanks` に分けた。 | `trim_blanks_back` / `skip_blanks` はスペースとタブだけを対象にし、改行も落とす `trim_back` と混同していないことを確認（`repropagate` 側は `trim_back`）。`tokens.get(at + 1)` の `KwIf` 検査と、`Eof` で `None` になる点も origin と一致。 |
| `walk_item` | 12 | 不変文脈を `Walk`、集積先を `Found` にまとめ、`Item` の分岐を `walk_item_in`、`def` 関数の腕を `walk_def_function` に切り、`Found::push` で `if let Some(fix)` を畳んだ。 | `ModuleImpl` の再帰が `walk_item` から `walk_item_in` に変わり `Walk` / `Found` を作り直さないだけで、押下順（`sole_parameter_fix` → `rewrap_arm_fix` → `repropagate_fix` → `walk_node`）は origin と同じ。`Found::push` は `None` を捨てるだけで順序を変えない。 |
| `let_identity_fix` | 13 | `let <bound> =` の綴り検査を `let_identity_equals`（下位の `let_identity_head` を含む）、本体直前の `->` を `body_dash_arrow` に分けた。 | 検査順（`KwLet` → 小文字 → `Name == bound` → `Eq`）と `equals` が `at + 2` を指すこと、`body_dash_arrow` の戻り値 `body_at - 1` を `value_end` に使うことが origin と一致。`let_identity_equals` が通る時点で `body_at >= 3` なので `body_at - 1` は破綻しない。 |
| `repropagate_fix` | 12 | §36 の形の取り出しを `repropagate_parts`（`Repropagate` 型に `scrutinee` / `body` / `pattern` / `absence` を詰める）、範囲と `pipe` を `repropagate_range`、末尾形ごとのテキストを `repropagate_replacement` に分けた。 | `references(body, "value")` と `rewrap_branch` の早期 return が形の検査より後・範囲計算より前という順を維持。`repropagate_range` の `checked_sub(1)` と `TokKind::Pipe` 検査、`repropagate_replacement` の `Tail::Def` で `def?` が `None` なら全体 `None` になる点、`pattern_text(inner)` が `pattern: inner` と同じ綴りである点を確認。 |
| `walk_node` | 35 | 各腕を `walk_written_nil` / `walk_written_cons` / `walk_let` / `walk_let_q` / `walk_bool_branch` / `walk_cond_branch` / `walk_pattern_branch` / `walk_fn` に分け、全ノード共通の子走査を `walk_children`、集積を `Found::push` に残した。 | 危険（origin 由来・分割では未導入）: `walk_bool_branch` と `walk_cond_branch` が子を明示的に辿った後、`walk_children` が `check::children` で同じ子を二度辿る。重複 Fix は `fixes()` の重なり除去で落ちるが、片方だけ消す・順を変えると修正の順序と本数が変わる。`walk_written_cons` は綴り判定に `callee`、`list_expression` に `node` を使い取り違えていない。各ヘルパー内の `return` は `walk_children` を止めない。clippy の実測値は未確認（cargo 実行禁止）。 |
| `walk_pattern` | 9 | `Nil` 綴りを `walk_written_nil_pattern`、`Cons` 2 引数を `walk_written_cons_pattern`、`Constructor` の引数を `walk_pattern_args`、`Record` / `NamedConstructor` を `walk_pattern_fields` に分けた。 | 式側 `walk_written_nil` / `walk_written_cons` が E510、パターン側 `walk_written_nil_pattern` / `walk_written_cons_pattern` が E516 で、取り違えていないことを確認。`Cons` 2 引数の腕がサブパターンを辿らない（一般 `Constructor` 腕だけが `walk_pattern_args` に入る）ことも origin と同じ。 |
| `list_expression` | 11 | `Cons` の連なりを辿る `loop` を `list_parts(tail, &mut parts)` に切り、`list_expression` は `parts` の初期化と `format!` だけにした。 | `Nil` と非 `Cons` の `break` が `return` になり、後続の `Some(format!([...]))` は `list_parts` の外で必ず通る（origin の `break` → `Some` と同じ）。`Cons` のたび `args[1]` を新しい `tail` にして前進する走査順も同一。 |

#### `crates/hado/src/functor_instances.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `max_line` | 9 | 本体 item 以外の位置の走査を `offset_auxiliary_spans` に集め、そのうち組で持つ集まりを `offset_branch_spans`（`branch_anchors`/`branch_spans`/`equality_anchors`）と `offset_named_sites`（`qualified_type_anchors`/`name_sites`）に分けた。 | origin が触る 8 つの集まり（3 つの単独 span 列＋組 5 つ）を全部写していること、`maximum` が Cell の max で各 span を 1 回しか触らない（offset の二重加算が無い）ことを確認した。`library`/`span` は origin も対象外。危険なし。 |
| `item` | 8 | `Item::TypeDef` の本体を `type_definition`、`Item::SigDef` の本体を `signature_definition` に切り出し、`item` は `Def`/`ModuleSig`/`ModuleImpl`/`Effect`・`Handler` の振り分けに絞った。 | `TypeDef` と `SigDef` はどちらも `params`/`type_params` を持ち取り違えやすいが、`type_definition` は `params` を、`signature_definition` は `type_params` と `params` を origin と同じフィールドに割り当てていることを確認した。危険なし。 |
| `ty` | 11 | `TypeKind::Adt` 腕の constructor 走査を `adt` に切り出し、`ty` は `Name`/`Apply`/`Group`/`Record` の各腕と末尾の effects 走査に絞った。 | `adt` が origin と同じ順（constructor 名 → named payload の field 名 → `payload.types_mut()`）で、`types_mut` が positional と named field の型を連結して返すため named payload の型が二重に走査されないことを確認した。危険なし。 |

#### `crates/hado/src/infer.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `check_ast のクロージャ`（origin 171 行） | 28 | クロージャ本体を `infer_program` へ移し、前処理を `install_compare`・`register_type_arities`・`collect_signatures`・`register_signature_abstract_parameters`・`register_types`・`collect_definitions`・`infer_definition_groups`・`check_comparison_constraints`・`check_row_constraints`、main の型規則を `finish_main`→`check_main_type` に分けた（`prepare_ambiguous_declarations`・`collect_shared_realizations`・`register_effects`・`check_operation_defaults`・`check_ambiguous_definitions`・`check_ambiguous_modules`・`resolve_projections`・`check_realization_cycles` は既存工程を順に呼ぶ骨格に残す）。 | 文の順は origin と一致（`collect_shared_realizations` が型・効果の登録より前、Library の早期 `Ok(Void)` は最後尾）。`definitions` は外の HashMap を `&mut` で渡して同一物を使う。`register_types` の `?` で violations の deferred 追加が飛ぶ短絡、`check_comparison_constraints`/`check_row_constraints` が `&mut self` 内で `self.deferred` へ push する形も origin と同じ。危険なし。 |
| `prepare_ambiguous_declarations` | 8 | `declared_constructors` を `$outer$` 名へ退避する for を `hide_declared_constructors` に抽出（`declared_types` の除去、`extend`、Error 値挿入は本体に残す）。 | 渡すのは `&ambiguous.declared_constructors` で `declared_types` との取り違え無し。`ambiguous_constructors` の remove → constructors / constructor_tags / named_constructors の退避順、および `ambiguous.constructors` の `extend` より前という位置も origin と一致。危険なし。 |
| `ordinary_module` | 23 | 成功後の修飾ブロックを `qualify_module_exports` に移し、`qualify_module_representations`（abstract_representations / shared_realizations）・`qualify_module_public_types`（exports / module_definitions）・`qualify_module_value_types`（value_types / composite_types）・`qualify_module_type_definitions`→`qualify_module_definition`→`qualify_module_constructors`（records / constructors）に分けた。 | `type_names` は 1 回だけ作って同じ参照を渡す。`saved_value_spans`↔`value_types`、`saved_composite_spans`↔`composite_types` の対応は取り違え無し。`module_path.push` 前にスパン集合を取得する点、record のとき constructors を走査しない早期 return も origin と同じ。危険なし。 |
| `infer_module_items` | 15 | 入れ子 module の推論を `infer_nested_modules`、ローカル型名の登録を `register_local_type_names`→`register_local_type_name`、item の登録を `register_module_items`、定義グループの推論を `infer_module_definition_groups` に分けた。 | `register_local_type_name` へ渡すのは `ast.span`（`def.span` ではない）で origin の `aliases.insert(..., ast.span)` と一致——取り違えやすい箇所。`ambiguous_types` の skip、`type_arities.insert`→`aliases.remove`→`aliases.insert` の順、`check_ambiguous_definitions`→`module_definitions` 充填→`check_ambiguous_modules` の順も同じ。危険なし。 |
| `module_body` | 11 | 署名型の実体化登録を `register_module_realizations`→`register_module_realization`（探索は自由関数 `find_actual_type`）、公開型の生成を `collect_module_exports`→`module_export`、多相性検査を `check_signature_polymorphism` に分けた。 | `module_export` の順序（存在確認→Value 形照合→instantiate_constrained→has_error 時 freshen→record_comparisons/row_constraints→type_params fresh→`signature_type(false)`→unify→多相性→public_parameters→`signature_type(true)`→unify→generalize_constrained）は origin と一致。`check_signature_polymorphism` は `&self` だが `Inference::resolve` も `&self` で副作用が無く差は無い。危険なし。 |
| `definition_group` | 9 | プレースホルダ設置を `install_group_placeholders`、定義ごとの検査を `check_group_definitions`→`check_group_definition`、無効時の巻き戻しを `invalidate_group`、一般化を `generalize_group` に分けた。 | 危険: `current_definition = None` の戻しは分割で落としやすく、実際にこの分割で抜けていた。レビューで見つけ、グループ走査のループ直後に 1 箇所戻した（origin と同位置。早期 `?` で None に戻らない経路も origin と同じ）。 |
| `register_effects` | 16 | effect 本体を `register_effect`→`register_effect_operation`、prelude の default 解決を `register_prelude_defaults`→`register_prelude_default`、handler 検査を `register_handlers`→`register_handler` に分けた。 | `template` は参照で渡し op ごとに `clone()` する origin の意味を保持。op の `?` 失敗時に `self.effects.insert` へ達しない点、`operation_result_rows`/`values`/`operations` を op 単位で先に書く点も同じ。`filter(op.kind == OpKind::Once)` は origin の `continue` と同値。危険なし。 |

#### `crates/hado/src/infer/functors.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `collect_shared_realizations` | 13 | 署名表を `module_signatures`、functor 雛形を `functor_template`、適用ごとの等式生成を `application_realizations`（契約名の照合は `contracts_match`、名前表は `instantiation_names`）に分けた。2 走査目の骨格と輪の除去は本体に残す。 | 非 Application・ambiguous・雛形無し・契約不一致をすべて空 map で返すので origin の `continue` 列と同値。`instantiation_names` は `(functor_name, module)` の順で受け、`format!("{}${}", functor_name, parameter.0)` と `module.name` の組み立てが origin と一致（functor 名と module 名の取り違え無し）。`additions.extend` 後に輪を除いて `shared_realizations` へ入れる順も同じ。危険なし。 |
| `shared_body_type` | 13 | `Fn` 腕を `shared_fn_type`、名前解決を `shared_named_type`（parameters→functor_abstracts→local→abstract_types→type_arities→builtin_type）、局所型の展開を `shared_local_type` に分け、引き回していた module/local/parameters/functor_abstracts/visiting を `SharedBody` にまとめた。 | `visiting` は `&mut Vec<String>` の push/pop から不変スライスの複製＋push に変わったが、各再帰点で見える訪問列は同じで輪の検出は変わらない。`definition.params.len() != args.len()` の早期 None、Record/Adt の `{module}.{name}` 修飾、仮引数置換後の再帰、`Fn` の `arguments[0]`/`[1]` と `.ok()?` も origin と同じ。危険なし。 |

#### `crates/hado/src/lexer.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `lex_token` | 20 | 2文字目で変わらない記号を `single_char_token` の表へ、残りを `lex_compound_token` の字種・記号の振り分けへ出した。そこから `(` を `lex_open_paren`、記号綴りを `lex_operator` とし、`=`/パイプ/`!`/`&` を `lex_eq_family`・`lex_pipe_family`・`lex_bang`・`lex_ampersand`、2文字目の消費を `two_char` に集めた。 | 表に `(` を入れていないこと（入れると `()` が Void にならない）、`lex_operator` で `...` と `--` のガードが各1文字腕より先であること、`two_char` の (second, paired, single) 対応（`>`→Ge/Gt、`<`→Le/Lt、`:`→DoubleColon/Colon、`-`→DashArrow/Minus）が origin と一致することを確認した。分割で不要になった EOF の `expect` を消し、`lex_operator` が peek 済みの文字を受け取る呼びに揃えた。危険なし。 |
| `lex_number` | 11 | 数字の連続消費を `take_while_digit`、小数点なしの確定を `finish_int`、小数点ありの確定を `finish_float` に分け、本体は `if peek != '.' { return finish_int }` の早期 return と bump 後の数字チェック（E103）だけにした。 | `start` を `take_while_digit` の前に取って両確定関数へ同じ値を渡していること、`is_ascii_digit` の述語、E103 の判定位置（bump 後）と E112 / code None の割り当てが origin と一致することを確認した。危険なし。 |

#### `crates/hado/src/mono.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `contains_handled_call` | 12 | 除算/剰余の raise 判定を `division_needs_raise_handler`、Name 腕の 2 判定を `name_carries_effects`、Apply/Call 腕を `call_carries_effects`（内部で `called_scheme_carries_effects` と `is_effectful_callee`）に分け、子走査だけ残した。 | 早期 return の順（除算→Name→abort→Apply/Call→子）と `&&` の短絡は不変。`default_operation_carries_effects` は name が Some のときだけ呼ばれ、`stack_needs_outcome` の再評価位置も維持。危険なし。 |
| `expression` | 20 | if 連鎖の各 NodeKind 腕を `pipe_expression`／`field_expression`／`record_expression`／`list_expression`（→`list_elements`・`derived_element_type`・`cons_list`・`element_type_mismatch`）／`name_expression`（→`upper_reference`）／`do_expression`（→`propagate_do_outcome`・`bind_again_do`・`is_option_expression`）／`let_expression`／`pattern_branch`／`cond_branch`／`bool_branch`／`unary_expression`／`literal_expression` に移し、本体は match dispatch だけにした。 | 腕は排他的 NodeKind なので if 連鎖→match の並べ替えは安全。`lower_literal` は Bin/Neg/Not に None を返すため末尾 `_ => literal_expression` でも順序不変。`list_elements` が items を昇順で lower してから `cons_list` が逆順に Cons を組む順、Name 腕の operation→upper→definition→local の順も維持。DoQ の Option 判定が is_outcome の後ろへ移ったが OUTCOME="$Outcome" と "Option" は排他。危険なし。 |
| `column_kind` | 9 | 行ごとの分類を `classify_head`、集めたフラグを `ColumnHeads` に持たせ、優先順位付けを `ColumnHeads::kind` に移した。 | `record_head`→`constructor_head`→`literal_head`→`catch_all` の else-if 優先順位と、未対応パターンで後続行を見ずに `Err` を返す点は不変。`kind` の record > Bool+constructor > constructor > literal > Any も同じ。危険なし。 |
| `structural_equality` | 9 | 前段（closure/abstract 拒否・キャッシュ・配列）の後に、id 確保と登録・本体構築を行う `define_structural_equality`、本体を組む `equality_body` に分けた。 | `id`／`next_function` と `structural_equalities` への push、`lhs`/`rhs` の `next += 2` は本体構築より前で、再帰がキャッシュ済み id を見る順は不変。`fields.iter().flatten()` と `fields.unwrap_or_default()`＋`fields.is_none()` は等価。`signature` の clone 位置と `call_direct` への受け渡しも同じ。危険なし。 |
| `collect_modules` | 8 | 外側の ModuleImpl/Ordinary 判定を `ordinary_module_impl`、1 モジュールの path 決定・definitions 登録・子への再帰を `collect_module` に分けた。 | 非 ModuleImpl と非 Ordinary をどちらも読み飛ばす点、path の組み立てと再帰順は不変。危険なし。 |
| `specialize_function` | 10 | compare 特例を `comparison_specialization`、定義/スキーム取得を `function_parts`、効果の答えを `outcome_answer`、署名を `outcome_signature`、キャッシュ/再帰探索を `cached_specialization`／`recursive_specialization`、降ろしを `lower_specialization`（`bind_parameters`・`reconcile_outcome`、文脈は `SpecializationPlan`）に分けた。 | 最も壊れやすいのは `key = specialized_name(name)` の計算位置で、origin は effect 判定前、現在は後。`specialized_name` は `handler_stack`/`arm_handlers` を読むが、間に挟まる `callback_answer`・`specialization_answer`・`outcome_signature` はいずれも `&self` で両者を変えないため値は同一（危険なし）。saved の復帰順（specialization/scope/context_answer/active）と `mem::take` 後に `bind_parameters` で scope を組む順も維持。 |
| `display_value` | 9 | `text` クロージャを `display_text` に、record 腕を `display_record_value`、一般 data 腕を `display_constructor_value`（→ named は `constructor_display`、位置は `positional_constructor_display`）に分けた。 | 腕順（List→Array→record→data→Err）と `parameters.len()==1` guard は不変。named が positional に勝つ分岐、fields 空なら名前だけを返す点、ラベル→lower→連結の順も同じ。`context.checked` のコピーと `context.format` の可変借用の交差は型検査が担保。危険なし。 |

#### `crates/hado/src/name_resolution.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
| --- | --- | --- | --- |
| `nested` | 14 | 宣言収集ループを `collect_declarations`（Adt の各 variant は `add_constructor`）、モジュール収集ループを `collect_modules` に分けた。 | `collect_modules` が `self.signatures` を読むため、末尾で signatures を extend する `collect_declarations` が先に走る順序を origin と一致確認。`add_constructor` の `named_fields.remove`→初回のみ insert と `local_constructors` の役割も不変。順序を崩すと壊れる最要注意点。 |
| `items` | 9 | Def 腕を `definition` に、Handler 腕を `handler` に切り出し（パラメータ束縛は `bind_params`）、ModuleImpl 腕とシグネチャ存在検査は `items` に残した。 | `definition` の `!def.rec` での `values.remove` と `current=Some(def.name)`、`handler` が腕ごとに `scope.clone()` すること、`bind_params` の `_` 除外を origin と一致確認。ModuleImpl の E204 検査が `implementation_scope` より先の順も不変。危険なし。 |
| `expression` | 21 | Name 腕を `name_expression`（`__` の E203/E202/E204 順は `intrinsic_error`）に、NamedConstructor を `named_constructor_expression`、Field の修飾解決を `resolve_qualified`（bool で再帰抑止）、PatternBranch を `pattern_branch_expression` に分けた。Apply/Call・Pipe・Let・Fn は `expression` に残した。 | `intrinsic_error` の判定順（!called で E203、!trusted で E202、非組み込みで E204）と、`resolve_qualified` が true のとき `expression(base)` を呼ばない極性を origin と一致確認。`pattern_branch_expression` の pat 束縛前に pattern 検査、腕ごとの local 生成、fallback が外側 scope を使う点も同じ。危険なし。 |
| `pattern` | 12 | NamedConstructor 腕（コンストラクタ検査＋E205 の `seen` 追跡＋フィールド再帰）を `named_pattern_fields`、Record 腕の入れ子再帰を `record_pattern` に分けた。 | `named_pattern_fields` の順（コンストラクタ検査→各フィールドで `named_field`→E205 を `report_related`（初出 span）か `seen` 追加→入れ子再帰）を origin と一致確認。Record が `named_field` を呼ばない差、Name upper 腕が先頭という match 順も不変。危険なし。 |

#### `crates/hado/src/names.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
| --- | --- | --- | --- |
| `free_names` | 14 | 反復箇所を `free_names_all`、CondBranch を `free_cond_branch`、PatternBranch（腕ごとの `bound.clone()` と `bind`）を `free_pattern_branch` に分けた。Let/LetQ/Fn の束縛追加と BoolBranch のループは `free_names` に残した。 | `free_pattern_branch` の腕ごとの `bound.clone()` が腕間・fallback に漏れず、fallback が外側 `bound` を使うことを origin と確認。List の values→tail、Apply の callee→args、Record の値走査順も不変。危険なし。 |
| `declaration_types` | 27 | 型集合を `visible_types`、効果を `visible_effects`、シグネチャを `visible_signatures`、impl による隠蔽を `hide_shadowed_module_types`、シグネチャ型の輸出を `export_signature_types` に分け、項目ループを `declaration_item` 経由で `declaration_module_sig` / `declaration_module_impl` / `declaration_sig_def` / `declaration_type_def` / `declaration_def` / `declaration_effect` / `declaration_handler` に割った。 | 順序（可視集合の構築→hide→export→項目処理）と、`declaration_types(items, inherited, inherited_signatures, inherited_effects, …)` の引数並び（signatures と effects の取り違え）を origin と一致確認。`declaration_module_sig` が継承込み `signatures` から contract_types を作る点、`declaration_def` だけが `types` を直接使う点も同じ。危険なし。 |
| `resolve_type_names` | 11 | ノード自身の未知型検査を `resolve_type_names` に残し、子型走査（Apply/Group/Record/Adt）を `resolve_type_children`、effects 行のラベル検査を `resolve_effect_label` へ分けた。 | origin では Apply 腕が検査と引数再帰を同時に行っていたが、相対順（自身の E204→子→effect label）が現在も保たれるのを確認。Adt の `payload.types()` 走査、Group/Record の再帰も不変。危険なし。 |
| `check_cycles` | 12 | E207/E208 判定を `cycle_violation`、群ごとの Def 走査を `collect_cycle_violations`、ModuleImpl の再帰を `collect_module_cycle_violations` に分け、`check_cycles` は群フィルタと最小診断選択だけにした。 | `cycle_violation` の判定順（value なら E207、次に `!rec` で E208）を origin と一致確認。群外 Def のスキップ、合成 `Ast`（items 以外を空ベクタ）を作る再帰、`min_by_key` の位置も不変。危険なし。 |

#### `crates/hado/src/namespaces.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
| --- | --- | --- | --- |
| `resolve` | 14 | 位置・root 計算を残し、組み込み登録を `register_builtin_types`、宣言走査を `register_symbols`（項目ごとに `register_item_declarations`＋`register_item_effects`）、解決走査を `resolve_items`（競合は `check_declaration_conflicts`→`check_prelude_operation_declarations`＋`check_declaration_conflict`）へ分けた。`qualified_type_anchors` の改名ループは `resolve` に残した。 | 呼び出し順（組み込み→登録→解決）と prelude 判定 `index < prelude_items`→`in_prelude` の受け渡しを origin と一致確認。`conflicts_with_root` は origin がループ変数 `root`、現在は `self.root` を参照するが直前に代入済みで同値。`or_insert` の先勝ちも不変。危険なし。 |
| `item` | 25 | Item の match 腕ごとに `item_type_def` / `item_module_sig` / `item_module_impl` / `item_sig_def` / `item_def` / `item_effect` / `item_handler` へ分け、Handler 内の腕 match を `bind_handler_arm` に切り出した。 | `item_module_impl` は sig 参照→`nested`→form（Functor の `scope.implementations` 追加）→items→名前修飾の順、`item_handler` は effect 参照→`prelude_effect`→annotations→arms→修飾の順で origin と一致。`bind_handler_arm` が引数型を `scope`、本体を `local` に渡す取り違えやすい形をそのまま保存しているのを確認（危険ではないが要注意点）。 |
| `ty` | 11 | 先頭の Name/Apply 名解決（`.` 付き `prefix.member`、`contract_types` による Space 選択、`renamed` 記録）を `resolve_type_name`、子型走査（Apply/Group/Record/Adt）を `visit_type_children` に分け、effects 行の `effect_label` ループは `ty` に残した。 | 「名解決→子走査→effect label」の順を origin と一致確認。`renamed` 挿入と `*name = format!` の前後、Adt の `payload.types_mut()` 走査、Name/Apply 以外で何もしない短絡も不変。危険なし。 |
| `pattern` | 8 | NamedConstructor と Record で重複していたフィールド反復を `bind_field_patterns` に集約し、Name/Constructor/Literal の腕は `pattern` に残した。 | NamedConstructor は先にコンストラクタ名を `reference`（`Space::Constructor`）してから `bind_field_patterns`、Record は参照なしで直入る差を origin と確認。フィールド順と `None` スキップ、upper の Name 腕が Constructor より前という match 順も不変。危険なし。 |
| `node` | 18 | Field 腕の upper 名特殊処理を `node_field`、NamedConstructor/Record のフィールド反復を `node_fields`、Apply/Call を `node_arguments`、List を `node_list`、Fn を `node_fn`、CondBranch を `node_cond_branch`、PatternBranch を `node_pattern_branch`、With を `node_with` に切り出した。 | `node_field` の早期 return（upper 名を `Space::Implementation` で解決したら `node(base)` に落とさない）と、`node_with` のハンドラ参照が args/body より先、`node_pattern_branch` の scrutinees→各腕 pattern→body→fallback の順を origin と一致確認。危険なし。 |

#### `crates/hado/src/parser.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `parse_items` | 12 | 1 宣言の読み分けを `parse_item`（`None` は library 見出し）、見出し判定を `section_keyword`、回復の目印を `declaration_keyword` に分けた。 | `section_keyword` は KwModule/KwImplement/KwEffect/KwHandler のみ、`declaration_keyword` は Def/KwType/KwShared を足した集合で、origin の outer 更新と `recover_declaration` の matches に一致。library 見出しで `outer` を false にせず回復も起こさない（origin の `continue` と同じ）ことを確認。未確認: clippy の複雑度実測。 |
| `parse_type_body` | 8 | `\|` で始まる ADT 本体のループを `parse_adt_type(span)` に切り出した。 | `parse_adt_type` へ渡す `span` が match 前に取った外側の span で、origin が `TypeExpr::of(..., span)` に使ったものと同一（constructor ごとの span ではない）。`shared_body_anchor` による事前の E100 と、`Name`+`{` の腕を `Implementation` の alias 腕より先に見る順も不変。危険は見つからず。 |
| `parse_atom` | 8 | `(` の枝（bump → `parse_expr` → E105 のコンマ検査 → `)` の検査 → 括弧を消して inner を返す）を `parse_group` に切り出した。 | `lp = self.span()` を bump の前に取る順、コンマ検査が `expect(RParen)` より先という短絡、inner をそのまま返してノードを作らない点が origin と同一。他の腕の `return self.parse_fun()` / `parse_name_or_call()` の早期 return も不変。危険は見つからず。 |
| `canonical` | 16 | Float を `canonical_float`、Apply/Call/Field を `canonical_apply` / `canonical_call` / `canonical_field`、Record と NamedConstructor の共通表記を `canonical_fields`、List を `canonical_list`、PatternBranch を `canonical_pattern_branch`、With を `canonical_with` に分けた。 | 子の走査が origin と一致：apply は子が Apply か大文字 `Name` のときだけ括弧、list は `...tail` を末尾に追加、pattern_branch は `comma_nodes` / `comma_patterns` / `pattern_arm_body` / fallback。Apply と Call、Field の `l`/`r` の取り違えはなし。空フィールド時の `Name {  }` のような空白の出方も同一。 |

#### `crates/hado/src/repr.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `machine_type` | 10 | Outcome/Array/record の 3 つの Pointer 腕を `is_boxed_data` に、payload 有無を走査する一般 data 腕を `data_type` に分けた。 | 3 腕は同じ Pointer を返すので `\|\|` への統合で値は不変。短絡順（OUTCOME→ARRAY→`records.contains_key`）は origin の腕順と同じで `contains_key` に副作用なし。`data_type` の `found`/`has_payload` と未知名エラーは本文そのまま。危険なし。 |

#### `crates/hado/src/source_print.rs`

| 関数 | 前 | 仮説（分けた境界） | 学習（壊れやすかった点） |
|---|---|---|---|
| `list_pattern` | 9 | `Nil` の判定を `is_nil_pattern`、`Cons`/2引数の判定を `cons_parts`、Cons 並びの走査を `cons_elements`、`[...]` の組み立てを `list_text` に分け、本体は `cons_parts(pattern)?` を門番にした。 | 分割時に `list_text` の末尾判定が origin の「裸の `Name("Nil")` のときだけ Nil」という述語からずれ、引数なし `Constructor("Nil", ..)` まで Nil 扱いにする差が出ていた。レビューで見つけ、origin の述語に戻した。`cons_elements` が `(head, tail)` を args[0]/args[1] の順で使う点も確認。 |
| `item` | 8 | `Item::Def` の本体を `definition`、`Item::Effect` を `effect_declaration`、`Item::Handler` を `handler_declaration` に切り出し、`item` は腕の振り分けだけにした。`TypeDef`/`SigDef`/`ModuleSig`/`ModuleImpl` はその場に残した。 | 各ヘルパーが読むフィールドと順序（`def.rec`/`def.kind`/`def.name`、`effect.ops`/`type_params`/`name`、`handler.params`/`arms`/`type_params`/`effect`/`name`）が origin と同じであることを確認した。危険なし。 |
