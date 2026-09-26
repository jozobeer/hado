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

正準形の往復と閉じの正例・負例、受理コーパスの実行を測る。

```
$ cargo test --offline --test postfix_close
running 3 tests
test close_shapes_parse ... ok
test close_defects_are_e100 ... ok
test corpus_is_canonical_and_round_trips ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --offline --test corpus
running 4 tests
test launch_tests::isolates_work_and_stdin ... ok
test library_corpus ... ok
test generated_output_headers_enforce_stream_status_and_empty_output ... ok
test corpus ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --offline --lib
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

条件が `(` で始まる `if` は、裸・`/if`・`if(...)` の 3 つを受理コーパス
（`tests/accept/if_condition_paren_*`）が、閉じの不足・余分・食い違いを `tests/postfix-close/reject`
が固定する。受理コーパスの `if` 全体への継続と最後の腕への継続は
`tests/accept/if_close_whole`・`tests/accept/if_close_last_arm` が実行結果まで固定する。
