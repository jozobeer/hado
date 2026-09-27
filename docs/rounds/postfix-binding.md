# 後置の束縛と `if` の閉じを既定にする（postfix-binding）

## 手順書

### 背景

束縛（`let p =`）と、値を使わない一歩（`do`）は行頭のキーワードで始まり、書く側が計算より先に名前か破棄を決める。`if` は閉じる印を持たず、最後の腕が右端まで伸びるので、`do` を `if` の前へ置くか腕の中へ置くかで `->` の付き先が変わる。現象は「束縛と破棄が行頭のキーワードで始まり、`if` の腕の終わりが字下げでしか決まらない」こと。

決定 D1（2026-09-27、`docs/plans/issue-backlog.md`）でこの構文を採ることが決まった。土台は試作ブランチ `postfix-binding-probe`（`git log --oneline main..postfix-binding-probe` の 11 コミット）である。本ラウンドは試作を既定の構文へ格上げし、旧構文の経路を消し、全コーパス・stdlib・tools を写し直す。

### 決定

決定（2026-09-27、試作 `postfix-binding-probe` で測った）をそのまま実装する。

- 束縛は `e as p ->`、Option は `e as p? ->`。値を使わない一歩は `e ->`。`let` と `do` は E100。
- `with H ->` は前置のまま残す。`as` は名付け専用で最も弱く結合する。
- `if` は本体に `->` があれば `/if` で閉じる。無ければ続きがあるときだけ `if(...)`、無ければ閉じない。
- `if` の直後の `(` は、対応する `)` までの最上位に `=>` があるときだけ閉じの開きと読む。空白に依らない。
- `,`・`;`・`)`・`]`・`}` は中の構文も閉じる。区切りの前の `if` は印を持たない。
- `fn` は本体に `->` が無ければ `fn(...)`、あれば `fn … /fn`。複数引数は `fn a, b => … /fn`。
- `def` は閉じない。`module`・`implement` は `/module`・`/implement` で閉じる。
- 閉じの不足・余分・食い違いは E100。`fmt` は閉じを書き換えない（下の「仕様への疑義」を参照）。

### 意図

hado を書く人と読む人が、計算を先に書き、名前と閉じを後ろに置けるようにする（推定）。新しい構文を書く側（`tests/`・`stdlib/`・`tools/`）と、処理系の構文解析・正準形の印字が同じ 1 通りの綴りを使う。

### 仮説

- 表層は既存の `NodeKind` へ写せる（試作で変種が要らなかった）。`mono.rs` の `make_continuation` は構文木しか見ないので変わらない。
- 後置の枝を既定にし、旧構文（`Syntax::Infix`）の経路と `--syntax` を消せる。
- `source_print.rs`（`fmt`・`--fix` の正準形）を後置の綴りへ替えれば、印字は冪等になり、`parse_corpus.rs` の往復も通る。
- 閉じの不足・余分・食い違いは、構文木が決まるので意味を変えずに診断できる。

### 判定器

issue の完了条件をそのまま使う。

- 選んだ構文と `if` の後ろの規則が仕様に書かれ、旧形（`let` と `do`）が E100 になる。
- `if` 全体に続ける形と、最後の腕に続ける形の両方を受理コーパスが固定し、実行結果が規則どおりになる。
- 閉じの不足・余分・食い違いと、条件が `(` で始まる `if` の 3 つの形を、受理・拒否のコーパスが固定する。
- `cargo test --offline` が 0 failed、`bash scripts/lint-docs.sh` が PASSED。

### スコープ

触る: `crates/hado/src/parser.rs`・`source_print.rs`・`fixes.rs`・`main.rs`・`project.rs`・`lexer.rs`（要れば）、`docs/spec.md`・`spec/grammar.ebnf`・`spec/errors.toml`、`tests/`（コーパス・判定器）・`stdlib/`・`tools/`、`editors/`（tree-sitter 文法）、`scripts/verify-postfix-binding.sh`（新設）。

触らない: `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`（`docs/rounds.md`）。他のレーンの worktree。

### 未確認

- 旧構文の経路を消したとき、`fixes.rs` の各書き換え（`let` を前提にする E501・E504・E508・E517）が後置の綴りで成り立つか。
- `if (` で条件が始まり本体に `->` を持つ `if` の 3 つの綴りが、字句の曖昧さなく受理されるか。
- 閉じの正準形違反を `fmt` が直せる粒度で書き換えを書けるか（§57 の範囲とコメントの規則）。実装は E100 にして書き換えない選択をした（「仕様への疑義」を参照）。
- tree-sitter の文法とハイライトが後置の綴りへ追随できるか。

### 成果物

- `crates/hado/src/parser.rs`: 後置を唯一の構文にする。`let`・`do` を E100。閉じの読み。
- `crates/hado/src/source_print.rs`・`postfix_print.rs`: 正準形の印字を後置へ（型とパターンの綴りは `source_print.rs`、項目と式は `postfix_print.rs`）。
- `crates/hado/src/fixes.rs`: 後置の綴りでの書き換え。
- `crates/hado/src/main.rs`・`project.rs`: `--syntax` と試作コマンドを消す。
- `docs/spec.md`・`spec/grammar.ebnf`・`spec/errors.toml`: 構文と閉じの規則。
- `tests/accept`・`tests/reject`・`tests/library`・`stdlib/`・`tools/`: 写し直しと新しい判定器。
- `scripts/verify-postfix-binding.sh`: 本実装の門番（`# tier: full`・`# round: postfix-binding`）。
- `editors/`: tree-sitter の文法。

### 工程

1. 試作ブランチを取り込み、`print-postfix` で全コーパス・stdlib・tools を写す（欠陥が消える E404・E516・E517 と本文コメントは手で保つ）。失敗するテスト（旧構文が E100、閉じの受理・拒否）を赤で示す。
2. 後置を既定にし、旧構文の経路と試作コマンドを消す。正準形の印字を後置へ替える。
3. 仕様・文法・tree-sitter を追随させる。
4. 試作の門番・探針を本実装の門番へ置き換える。
5. `git merge main`・corpus lock の作り直し・ラウンドの実測。

### 判定

主要コミットの単位ごとに触った門番 1 本を回す。最後に `cargo test --offline`・`bash scripts/run-gates.sh --tier quick target/gates-quick`・`bash scripts/lint-docs.sh` を 1 回ずつ回し、新設の門番は単独で緑を確かめる。

### 許容される赤

- なし。試作の `--syntax` と探針は同じラウンドで消した。

### スコープ外

- `docs/plans/issue-backlog.md` の F レーンの後続（`localize-message-positions` 以降）。
- `do-option-absent`（M7）と `fmt-layout`（F5）。
- 仕様の節番号を動かすこと、`docs/plan.md` への追記。

### 仕様への疑義（issue と実装の食い違い）

- 主張: 閉じの不足・余分・食い違いのうち、どれを `fmt` の書き換えにし、どれを E100 にするか。
- 再現: `printf 'def f c => if c => 1 | log "a" -> 2\n' > main.hd; cargo run --offline -q -- check main.hd`
- 現行: `main.hd:1:1: error[E100]`（`/if` を補う suggestion 付き）。issue の案は「足りない閉じ・余分な閉じ…は正準形の違反になり、`fmt` が直せる」と書く。
- 変えると: `fixes.rs` に閉じ専用の書き換えを足し、`fmt` が `/if`・`if(...)` を補う／外すことになる。構文木は変わらないが、`fmt` の書き換えと `fixes` のテストが増える。
- 推奨: 実装は E100 と suggestion に留めた。閉じは構文の一部で、`fmt` の担当（冗長の除去・整形）とは性質が違うため。`fmt` が直す形にするなら別ラウンドで `fixes.rs` を後置へ広げる。
- 決定（2026-09-27、人）: issue の「足りない閉じ・余分な閉じは正準形の違反で `fmt` が直せる」という文は §57 に譲る。§57 の書き換えの表に閉じの印は無いので、閉じの不足・余分・食い違いは E100 のままにし、`fmt`・`--fix` は閉じを足さず直さない。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- 表層を既存の `NodeKind` へ写せない（新しい変種が要る）と分かった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §6・§11 | 束縛は `e as p ->`、破棄は `e ->`、`let`/`do` は E100 | 構文 | E100 | `tests/reject/E100`・受理の実行 |
| §12 | `fn` は `fn(...)` か `fn … /fn`。`/` の閉じの読み | 構文 | E100 | 受理・拒否コーパス |
| §12・§23〜§26.1 | `if` は本体に `->` があれば `/if`、続きがあるときだけ `if(...)` | 構文 | E100 | 受理・拒否コーパス |
| §12・§30 | 閉じ違い（`if … /fn`）は E100 | 構文 | E100 | 拒否コーパス |
| §10 | `module`・`implement` は `/`+名前で閉じる | 構文 | E100 | 受理・拒否コーパス |
| §30・E101 | 継続の定義を後置の綴りへ | 構文 | E101 | `tests/reject/E101` |
| §40・§41・§53 | E501・E504・E505・E506 の文言と書き換え | 冗長 | E5xx | 拒否コーパス・fmt |
| §57・§58.1 | 正準形の印字と fmt | 冗長 | — | `cargo fmt` の冪等 |

## 実測

完了条件ごとに測る。

```
$ cargo test --offline --test postfix_close
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --offline --no-fail-fast
（70 本の判定が全て ok。0 failed。レビューの指摘を閉じたあとに測り直した）
```

条件ごとの当て方。

- 選んだ構文と旧形の E100: `tests/unit/parser.rs` の `removed_binding_keywords_are_e100`
  が `let`・`do`・`let?`・`do?` の E100 を、`tests/postfix-close/reject` が閉じの
  不足・余分・食い違いを固定する。仕様は §6・§11・§12.1・§56 と `spec/errors.toml`。
- `if` 全体への継続と最後の腕への継続: `tests/accept/if_close_whole`（`if(b => 1 | 2) + 1` が
  `2`）と `tests/accept/if_close_last_arm`（`log "a"` が `a`）が実行結果まで固定する。
- 条件が `(` で始まる `if` の 3 つの形: `tests/accept/if_condition_paren_bare`
  （裸・`big`）・`if_condition_paren_group`（`if(...)`・`11`）・`if_condition_paren_rail`
  （`/if`・`7`）が実行結果まで固定する。
- 正準形の往復: `cargo test --offline --test postfix_close` が `tests/accept`・`stdlib`・
  `tools` の全ファイルで、印字が入力と一致し読み直しても変わらないことを見る。
- 門番: `bash scripts/verify-postfix-binding.sh` は 4/4。`J3` は印字器と
  `if(` の読み分けの 5 つの変異が全て赤になることを見る。

```
$ bash scripts/verify-postfix-binding.sh
J1:往復と閉じ(ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.50s): PASS
J2:コーパス(ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.22s): PASS
J3 drop-rail-if: caught
J3 drop-option-question: caught
J3 drop-if-parens: caught
J3 add-if-parens: caught
J3 always-close-if-paren: caught
J3:印字器の変異(cases=5): PASS
J4:lint-docs(PASSED: 16/16): PASS
PASSED: 4/4

$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 41/41 passed; red: none

$ bash scripts/run-gates.sh --tier core target/gates-core
TALLY: 661/661 passed; red: none

$ bash scripts/verify-treesitter.sh
PASSED: 5/5
corpus files: 149 / goldens: 47 / extmarks: 39

$ bash scripts/verify-m1-corpus.sh
PASSED: 7/7

$ bash scripts/verify-m7.sh
PASSED: 2/2

$ bash scripts/lint-docs.sh
PASSED: 16/16

$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2

$ bash scripts/corpus-lock.sh --check
（差分なし、rc=0。作り直しの差分は受理コーパス 6 本の `// item:` を外した
ハッシュだけで、いずれもこのラウンドの成果物）

$ bash scripts/gate-tiers-lock.sh --check
（差分なし、rc=0）
```

### 指摘 1（局所修正が旧構文を書く）の実測

`crates/hado/src/fixes.rs` の E504・E506・E520 の置換と `parser::canonical` を後置へ
写し、`crates/hado/tests/postfix_close.rs` の `canonical_rewrites_write_parseable_postfix`
が `E504`・`E506`・`E507`・`E508`・`E520` の `check --fix` と `fmt` の出力を
**読み直して検査**する（`--fix` と `fmt` の往復）。`canonical_rewrites_keep_parenthesized_branch_close`
が `if(...) as v ->` の閉じ `)` を保つことを固定する。

```
$ cargo test --offline --test postfix_close
test result: ok. 6 passed; 0 failed
```

診断の `canonical form:` も同じ綴りを使う。実測:

```
$ cargo run --offline -q -- check tests/reject/E504/main.hd
main.hd:3:23: error[E504]: use `->` to discard a success value; canonical form: Some(x) -> Some(1)

$ cargo run --offline -q -- check tests/reject/E506/main.hd
main.hd:3:17: error[E506]: a plain binding cannot discard its value; write `->`; canonical form: （束縛を削除）
```

E303 の案内も `e ->`・`e as p? ->` へ写した。`fmt` は閉じの印を足さない
（上の「仕様への疑義」の決定）。

### 指摘 2（門番が旧構文を埋め込む）の実測

触った full の門番は全て 1 本ずつ回し、緑を確かめた。

```
prelude-tail-recursion 8/8   r30 16/16   r31 25/25   r32 45/45   r34 1360/1360
r35 714/714   r37 260/260   r43 10/10   r46 5/5   r47 4/4   r48 5/5
r49 2/2   r50 4/4   r52 5/5   r53 5/5   r62 4/4   r63 4/4   r71 4/4
r72 4/4   r73 4/4   r75 4/4   r76 4/4   r77 5/5
```

`r37` は入れ子で `verify-m1-corpus.sh` を回す。旧構文を埋め込む門番のほかに、
`verify-m7.sh`（core）と `scripts/lsp-check.py` の入力・契約も後置へ写した
（`E504`・`E506` の書き換えの比較対象、`module` の `/module`、未保存テキストの
`1 as unused ->`）。`verify-lsp-minimal.sh`（core）はこの 3 件で 8/11 だったのが
11/11 になった。

core 層を回すと、このラウンドが書いた `postfix_print.rs` の `expr_prec` と
`parser.rs` の `parse_fun`、`main.rs` の `print_postfix_command`・`is_comment_line`
が `verify-cognitive-complexity.sh` の閾値 7 を超えていた（8/7・14/7・8/7・9/7）。
入れ子を補助関数へ出して 3/3。往復（J1・J2）と `cargo test` は緑のまま。

`string-split-linear` の `i perf: join` は、機械が速いと小さい方の中央値が
較正の下限 `0.200s` を割って赤になる（`0.193s`〜`0.198s`）。同じ木で
10/10 が 3 回、9/10 が 3 回で、`String.join` の実装も門番の入力もこのラウンドで
変えていない。門番は緩めず、環境依存の揺れとして報告する。

### 指摘 3（tree-sitter が追随しない）の実測

`editors/tree-sitter-hado/grammar.js` に `e as p ->`・`e as p? ->`・`e ->` と
`/if`・`/fn`・`/module`・`/implement` の閉じを足し、golden とハイライトを追随させた。

```
$ bash scripts/verify-treesitter.sh
PASSED: 5/5
corpus files: 149 / goldens: 47 / extmarks: 39
```

### 仕様と実装の食い違い（記録）

正準形の印字器は `def` の本体の最初の一歩を見出し行へ連結し、括弧付き引数の
`(` の前に空白を置く。付録B と §55 の例はこの綴りへ写した（`m1-corpus` の
突き合わせと `verify-m7` の署名）。付録B の `def display`・`def active_names users =>`
は本体と一体の 1 行になったので、`verify-m1-corpus.sh` の検査 2 の二次の見張り
（`def display` を含むファイルに `| Guest => "guest"` と `// expect: guest` を、
`def active_names users =>` を含むファイルに `// needs: prelude` を要求する）の
引き金を、完全一致から行頭一致 `^def display($|[ (])`・`^def active_names users =>`
へ合わせた。見張りは狭めていない（当たるファイルは増える）。

### main 取り込み後

`origin/main`（`a19f6cd`、M3 `pipe-in-tail-argument`・R2 `char-and-float-conversions`・
G4 `frozen-ledger-guard`）を取り込み、5 本の衝突を解いた。`docs/spec.md` は §55 の
`Char`・`Float` の追記を残し、署名を正準形の綴り（`def to_int (value: Char)`）へ
揃えた。`stdlib/env.hd`・`io.hd` は後置の綴りで M3 の `|>` を保ち、`float.hd`・
`char.hd` は R2 の追加を正準形にした。`tests/corpus.sha256` は `--check` の差分
140 行が全て「このラウンドが写したファイル」か「main が変えた 4 本（char・env・
float・io）」で説明できたので作り直した。

main が足した探針も後置へ写した。`verify-r33.sh` は module/implement の閉じと
`gates_case` の `let` を直して 72/72、`crates/hado/tests/r26_effects.rs` の効果
閉包の探針は `let` を `as` に写した。`verify-char-and-float-conversions.sh` の
署名の期待値は正準形へ揃えた（5 つの署名を要求する強さは変えない）。

full 層の門番は 49 本ある（`# tier: full`）。このラウンドで中身を写したのは
r33・r36・r54・r55・r58・r69・r74 の 7 本。r33・r54・r55・r58 は埋め込み hado の
module/implement の閉じと `let` を直し、r36 は §55 の署名が括弧前に空白を置く
正準形になったので member の切り出しと変異の正規表現を合わせ、r69・r74 は
E502・E511 の拒否コーパスが 1 行になったので期待値の導出を `if` / `|` から腕を
取る形へ直した。r74 の赤は E511 の書き換えが 1 行の多節関数で見出しを二重に
する印字器の穴でもあったので、`literal_replacement` が見出しの後ろだけを前置きに
残すよう直した（`fix:`）。full 層は 49 本全てを `--tier full` の 1 回で回した。

```
$ cargo test --offline
（70 本の判定が全て ok。0 failed）
$ bash scripts/run-gates.sh --tier quick .
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core .
TALLY: 697/697 passed; red: none
$ bash scripts/run-gates.sh --tier full .        # full 49 本
TALLY: 3806/3807 passed; red: verify-string-split-linear.log
$ bash scripts/verify-r33.sh                      → PASSED: 72/72
$ bash scripts/verify-r36.sh                      → PASSED: 384/384
$ bash scripts/verify-r54.sh / r55.sh / r58.sh    → PASSED: 5/5, 5/5, 8/8
$ bash scripts/verify-r69.sh / r74.sh             → PASSED: 4/4, 4/4
$ bash scripts/verify-char-and-float-conversions.sh → PASSED: 9/9
$ bash scripts/corpus-lock.sh --check / gate-tiers-lock.sh --check → 差分なし rc=0
```

full 層の唯一の赤は `verify-string-split-linear` の検査 i（join）で
`small median 0.179s is below the 0.200s floor`。既知の計時揺れ
（`docs/issues/flaky-string-split-timing-gate.md` が、検査 i は下限を割ると赤に
なり単独再測で緑に戻ると記録する）。門番は緩めていない。

### 写しの途中で見つけた印字器の穴

`print-postfix` の出力を読み直すと、正準形が自分の構文解析を通らない綴りが
3 つあった。いずれも直し、`tests/accept` の往復が固定する。

1. `- - x` の正準形が `--x` になり、字句が `--` をコメントの印と読んで E104。
   入れ子の `-` の被演算子を括弧で包む（`tests/accept/neg_nested`）。
2. pattern の腕の本体が `->` を含むとき、包まずに綴ると後ろの `| 次の腕` が
   本体の内側の腕として読まれる。`tests/accept/user_store` の
   `| Some(user) => do … -> true | false` は旧構文では `| false` が独立の腕
   だったが、後置の綴りでは `true` の後ろに付いていた。包んで旧構文と同じ木に戻す。
3. 見出しの中の項目が読めないとき、`recover_declaration` が `/module`・
   `/implement` を飛ばして木の外まで走り、「閉じていない」という別の E100 が
   本当の誤りを位置の順で覆っていた。閉じの手前で止める。
