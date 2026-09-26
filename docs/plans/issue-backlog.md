# 残った issue を片付ける計画（issue-backlog）

2026-09-27 時点の `docs/issues/` の全 issue を、どの順に、どこまで並行して片付けるかを決める。各 issue の完了条件は issue 自身が持ち、ここでは写さない。

## 意図

- 使う人: hado の作者と、ラウンドを委譲される builder（Claude・codex・cursor）（推定）。
- 場面: 次に着手するラウンドを選ぶとき、並行して走らせてよい組を選ぶとき（明示: 「全部対応するための 1 つの計画」）。
- 理由: 優先度の行だけでは、同じファイルを触る issue の衝突と、issue どうしの前提が見えない（推定）。

## 方針

原因欄で触るファイルが重なる issue を 1 本のレーンにまとめ、レーンの中は直列、レーンどうしは並行にする。同じファイルを 2 本のラウンドが同時に書くと、着地の直前の `git merge main` で衝突し、`tests/corpus.sha256` の作り直しも重なるため。レーンの中の順は、優先度・issue に書かれた前提・直すと後ろの issue が消える見込みの順である。

同時に走らせるのは 4 レーンまでにする。委譲先を 5 体以上同時に起こすと沈黙する体が出た実績から置いた上限である（推定）。

## レーン

| slug | レーン | 順 | 優先度 | 主に触る場所 | 前提 |
|---|---|---|---|---|---|
| `effectful-closure-through-prelude` | M 単相化 | 1 | 高 | `mono.rs` | なし |
| `effectful-closure-in-list-literal` | M 単相化 | 2 | 中 | `mono.rs` | M1 の着地後に再現し直す |
| `pipe-in-tail-argument` | M 単相化 | 3 | 中 | `mono.rs`・`stdlib/io.hd` | M1 の着地後に再現し直す |
| `map-by-builtin-compare` | M 単相化 | 4 | 中 | `mono.rs`・prelude | 速さの比較は `string-split-linear` の後 |
| `handler-answer-types` | M 単相化 | 5 | 中 | `mono.rs` | なし |
| `handler-value-args` | M 単相化 | 6 | 中 | `name_resolution.rs` | なし |
| `do-option-absent` | M 単相化 | 7 | 中 | 仕様 §33〜§36・`check.rs`・`fixes.rs` | M5・M6、決定 D2 |
| `string-split-linear` | R 実行系 | 1 | 高 | `runtime/hado_runtime.c`・`stdlib/string.hd` | なし（`prelude-tail-recursion` は着地済み） |
| `char-and-float-conversions` | R 実行系 | 2 | 中 | `stdlib/float.hd`・仕様 §55 | なし |
| `exit-code-from-main` | R 実行系 | 3 | 中 | `runtime/hado_runtime.c`・仕様 §54 | R1（同じ C ファイル） |
| `cranelift-opt-level` | R 実行系 | 4 | 低 | `emit.rs` | R1（その入力で測る） |
| `dogfood-text-tool` | R 実行系 | 5 | 高 | `tools/` | R1・R2・M4 |
| `dropped-spec-contracts` | G 門番 | 1 | 高 | `scripts/lint-docs.sh` | 決定 D3 |
| `spec-lock-input-hardening` | G 門番 | 2 | 中 | `scripts/spec-lock.sh`・`scripts/verify-spec-model.sh` | なし |
| `gate-optimize-flag` | G 門番 | 3 | 中 | `scripts/lint-docs.sh`・`scripts/verify-spec-model.sh` | G1・G2（同じファイル） |
| `frozen-ledger-guard` | G 門番 | 4 | 中 | `scripts/lint-docs.sh` | 決定 D4 |
| `lint-docs-hardening` | G 門番 | 5 | 中 | `scripts/lint-docs.sh`・`docs/issues.md` | 決定 D5 |
| `localize-message-positions` | F 前段 | 1 | 中 | `check.rs`・`parser.rs`・`project.rs`・`lsp.rs` | なし |
| `build-json-diagnostics` | F 前段 | 2 | 中 | `main.rs`・`project.rs` | F1（同じファイル） |
| `postfix-binding` | F 前段 | 3 | 中 | `parser.rs`・仕様・全コーパス | `postfix-binding-probe` の報告、決定 D1 |
| `e101-suggestion-scope` | F 前段 | 4 | 低 | `parser.rs` | F3（`if` の文法が変わる） |
| `fmt-layout` | F 前段 | 5 | 中 | `source_print.rs`・`fixes.rs`・`lexer.rs` | F3（issue に明記） |
| `repl` | F 前段 | 6 | 低 | `main.rs`・`lexer.rs` | F3（`e as x ->` を使う） |

以下、`M1` は M レーンの 1 番目を指す。

最初に並行で起こすのは M1・R1・G1 の 3 本である。F レーンは `postfix-binding-probe` が進行中なので、F1 を 4 本目として足せる。

## 着手前に人が決めること

| ID | 論点 | 推奨 | 止めるもの |
|---|---|---|---|
| D0 | `docs/wants.md` の低に残る「`as` パターン」（`docs/plan.md` 7 節 Q17' の保留）を消すか | 消した（2026-09-27）。`postfix-binding` で `as` を名付け専用にした。照合対象全体は `e as whole ->` で束縛できる | なし（記録の整合だけ） |
| D1 | `postfix-binding` を採るか、`if` の閉じの規則を有力案で確定するか | `postfix-binding-probe` の報告で J1〜J6 が PASS なら採る | F3〜F6 |
| D2 | `do-option-absent` の推奨（不在を効果にする）と次善のどちらか | M5・M6 の着地後、`absent-effect-draft` の実測を見て決める | M7 |
| D3 | `dropped-spec-contracts` の案 A・B・C | 決めた（2026-09-27）。C を改めた形（針は `verify-spec-contracts.sh` に隔離、層の所属は生成物） | G1 |
| D4 | `frozen-ledger-guard` の案 A・B・C | 決めた（2026-09-27）。A。`docs/ledger.md` と `docs/plan.md` を丸ごと凍結する | G4 |
| D5 | `lint-docs-hardening` の案 A・B・C | 決めた（2026-09-27）。issue ごと退けた | G5 |
| D6 | `e101-suggestion-scope` の読み | F3 の後に、新しい `if` の文法で issue を書き直してから決める | F4 |

## 1 ラウンドの手順

どの slug も `AGENTS.md` の「守ること」に従う。レーンの中で次の slug に移るのは、前の slug が main に着地してからにする。

1. `git -C <主チェックアウト> pull --ff-only` の後、`git worktree add .worktrees/<slug> -b <slug> main`。
2. 前提の欄に「再現し直す」とある slug は、issue の現象のコマンドを main で走らせる。再現しなければ、issue を消すコミット（本文に消えた理由と、直したコミット）で終える。
3. `docs/rounds/<slug>.md` に手順書を書く。判定には issue の完了条件をそのまま使う。
4. 実装し、触ったところの門番を 1 本だけ回す。ラウンドの最後に `bash scripts/run-gates.sh`（層別）と `cargo test --offline` を 1 回回す。
5. `git merge main` の後、`bash scripts/corpus-lock.sh --check` の差分を読み、main 側の変更とそのラウンドの成果物だけで説明できることを確かめてから作り直す。
6. 同じコミットで `docs/issues/<slug>.md` を消し、push して PR を作る。着地後の全走の実測は、同じ `docs/rounds/<slug>.md` に `docs:` の PR で足す。

レーンの注意:

- M: 5 本とも `crates/hado/src/mono.rs` を触る。M1 の推奨（本体を実際の型で下ろす）が M2・M3 の原因も消す見込みなので、M2・M3 は手順 2 の再現から始める。
- R: R1 と R3 は `runtime/hado_runtime.c` の別の関数を触る。R3 は `Io` に `log_error` を足すので、網羅的な `handler Io` を持つ門番（`scripts/verify-r37.sh` など、issue に列挙）を同じラウンドで直す。
- G: `docs/plan.md` の門番基盤の節と同じく、並走させない（`AGENTS.md`）。G レーンの走行中に、他のレーンが `scripts/lint-docs.sh` を触らない。
- F: F3 は全コーパスの `let` と `do` を書き換えるので、着地の時点で走っている他のレーンは、main を取り込むときに `print-postfix`（`postfix-binding-probe` で試作）で自分の追加分を写す。F3 の着地を他のレーンに先に知らせる。

## 受け入れ条件

- 網羅: `docs/issues/` の全 slug がレーンの表にある。次のコマンドの出力が空なら合格。今は空。
  `comm -23 <(ls docs/issues | sed 's/\.md$//' | sort) <(grep -o '^| `[a-z0-9-]*`' docs/plans/issue-backlog.md | tr -d '|` ' | sort -u)`
- 完了: レーンの表の slug が、`docs/issues/` に 1 つも残っていない。次のコマンドの出力が空なら合格。今は表の全 slug（23 行）が出る。
  `grep -o '^| `[a-z0-9-]*`' docs/plans/issue-backlog.md | tr -d '|` ' | while read s; do [ -e docs/issues/$s.md ] && echo $s; done`
- 着地の記録: 消えた slug ごとに、`docs/rounds/<slug>.md` があるか、`git log --diff-filter=D --format=%B -- docs/issues/<slug>.md` の本文に消した理由がある。
- 門番: 最後の slug の着地後、main で `bash scripts/run-gates.sh --tier full` が red なし、`cargo test --offline` が 0 failed。

計画の途中で生まれた issue（`dogfood-text-tool` は足りない API を issue にすることを完了条件に持つ）は、この表に足さない。別の計画に置く。

## 不確実性

- M1 が M2・M3 の原因も消すかは未確認。消えなければ M2・M3 はそれぞれ単独のラウンドになり、M レーンが長くなる。
- F3 が全コーパスを書き換えた後、他のレーンの追加分を `print-postfix` で写せるかは、`postfix-binding-probe` の J1（往復）の結果に依存する。
- 中・低の issue どうしの依存は、互いの slug への言及と原因欄のファイルからしか見ていない。仕様の同じ節（§55 を R1・R2・M4 が触る）の衝突は、節が違えば `git merge` が解けると見ているが未確認。
