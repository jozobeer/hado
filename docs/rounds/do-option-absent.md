# Option の不在を効果 `Absent` に一本化する（do-option-absent）

## 手順書

### 背景

`e ->` の値が `Option` なら `None` をそのまま伝播する。`?` の印は付かない。§34 は
「`?` の有無が `Option` と効果を見分ける」とするので、`Option` に対する `->` だけが
その原則の例外になる（再現は下の「実測」）。

決定 D2（`docs/plans/issue-backlog.md`、人が決定）は推奨案を採る。**不在も効果にする。**
prelude に `Absent` 効果と `None` へ戻す handler `Maybe` を置き、`as p? ->` と
`Option` に対する `->` を廃止する。`Option` はデータとしてだけ残す。伝播はどれも
効果として関数の型に現れ、`->` は「効果を持つ式の値を捨てる」の 1 つの意味になる。

### 決定

1. **prelude に `Absent` と `Maybe` を置く。** 新しい `stdlib/absent.hd` に
   `effect Absent` / `abort absent` と `handler Absent as Maybe`（`abort absent => None`、
   `return value => Some(value)`）を置き、`lib.rs` の prelude 連結に足す。効果名
   `Absent` は予約になり、利用者の同名宣言は E205 とする（§49、§55）。
2. **`as p? ->` を廃止する。** `?` は文法から外し、`as p ->` の後ろに `?` を書くと
   E100 とする（`let`・`do` と同じ「廃止した綴り」の扱い）。構文木の `LetQ` は
   移行入力（`print-postfix`）だけが読む。
3. **`e ->` は効果を持つ式だけを受ける。** `Option` は効果ではないので E303 とする。
   E303 の短い名前を `rail-type` から `discard-type` へ改める（「rail」は
   `Option` の伝播路を指す古い語で、その路が消えるため）。
4. **E504 と E508 を §56 から外す。** E504 の禁止（`as _? ->` は `e ->` が正準形）は
   `?` の廃止で消える。E508 の正準形（`<scrutinee> as p? -> <body>`）も消え、データの
   `Option` を写す分岐（`if o | Some(x) => Some(f x) | None => None`）は書き換え先が
   無いので冗長としない。E507 はデータの `Option` の恒等な再包装として残す
   （`if o | Some(x) => Some(x) | None => None` は `o` が正準形）。
5. **`Option` を返す stdlib の 3 関数は Option のままにする。**
   - `List.head`・`List.nth`: 「空のリストに先頭が無い」「添字が範囲外」は**探索の結果と
     してのデータの不在**で、呼び出し側は値として検査する（`Option.unwrap_or`・分岐）。
     §33 の既存文「`List` の探索は `Option` を返し」をそのまま保つ。効果に寄せると
     探索のたびに `with Maybe` が要り、`Map.find` と同じ形の関数が揃わなくなる。
   - `Env.env`: 環境変数が未設定なのは環境という写像の**値の不在**で、失敗は非 UTF-8 の
     `{Error}` が運ぶ。§55 の既存の型をそのまま保つ。
   - 一本化するのは**伝播の仕組み**である。関数の結果が得られないことを呼び出し側へ
     伝える路は効果（`Absent`）に一本化し、`Option` は伝播しない。データの `Option` を
     伝播させたい関数は、`Absent` 効果で書き、必要なら `with Maybe` で `Option` に戻す。
6. **`main` に `Absent` が残ったら、操作名 `absent` を stderr に印字して終了コード 1。**
   `Absent` は prelude の効果なので E706 の対象外である（§37 は「prelude の `abort` は
   handler が無ければ `main` の入口で stderr と終了コード 1」とする）。`raise` が届いた
   ときと同じ扱いで、運ぶ値が無いので印字するのは操作名にする。`Int` の `main` でも
   exit 1（R3 の `raise` の前例と同じ）。
7. **§33 の境界を「理由の有無」から「`Absent` と `Error`」へ書き直す。** 理由を持たない
   不在は `Absent`、理由を運ぶ失敗は `Error` とする。`Option` は値としての不在をデータで
   表し、伝播しない。

### code-complete の一周目

- 意図: hado を書く人が、値が得られない計算の不在を 1 つの仕組み（効果）で書き、
  `?` と `->` の 2 つの印を使い分けずに済むようにする（推定。D2 は「不在も効果にする」）。
- 仮説: 不在を `Absent` 効果にすれば、`as p ->`（効果の bind）と `e ->`（効果の破棄）と
  `with Maybe`（`Option` への run）で、今 `?` と `Option` の `->` が担っている伝播を
  すべて書ける。`Option` の `->` を型段階で E303 にし、`?` を構文段階で E100 にすれば、
  例外は消える。`Option` を写す分岐は書き換え先が無くなるので E508 は不要になる。
- 判定器: issue の完了条件をそのまま使う（下の「判定」）。加えて、門番の赤は
  「変異で落ちること」で確かめる。
- スコープ: `stdlib/absent.hd`（新設）・`stdlib/option.hd`・`crates/hado/src/lib.rs`・
  `parser.rs`・`infer.rs`・`check.rs`・`fixes.rs`・`mono.rs`・`spec/errors.toml`・
  `spec/grammar.ebnf`・`docs/spec.md`・`docs/corpus.md`・`README.md`・
  `tests/accept`・`tests/reject`・`tests/postfix-close`・`tests/corpus.sha256`・
  追随する `crates/hado/tests/*.rs` と `scripts/verify-*.sh`・
  `scripts/verify-do-option-absent.sh`（新設）・本ファイル。
  触らない: `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`（凍結）。
- 未確認: `?` を E100 にしたとき、`docs/plan.md` §11.2 の項目 `let p? =` の受理コーパスが
  原理的に書けなくなる（下の「仕様への疑義」）。`Absent` が `main` に残る経路の
  mono の印字。`handler Absent as Maybe` を prelude に置いたとき、`Maybe` の答え型が
  複数の型で使えるか（M5 の `WithTypes` が効くか）。`Option` を使う stdlib の関数のうち
  `?` に依存するのは `Option.map` だけか。

### 仕様の書き換え（節番号は動かさない）

本文の `## N.` はそのままに、次を書き換える。§56 の表と付録C は `spec/errors.toml` と
`spec/grammar.ebnf` が正本で、`scripts/spec-lock.sh` で作り直す。

| 節 | いまの文 | 書き換え |
|---|---|---|
| §2 | eta 展開の 1 文だけ | 不在の表し方の規則を 1 段落足す（計算が値を作れないことは効果、`Option` はデータ、`e ->` は効果だけ） |
| §6 | `find_user id as user? ->` の例 | `find_user id as user ->` の効果版に差し替える |
| §11 | `as p? ->` も同じ継続構文を使う／`e as whole -> whole as Some(x)? ->` | `?` の文と例を消し、`e ->` の 1 文にする |
| §10 | `?` を付けた `as p? ->` は Option の伝播を行う | 文を消す |
| §28 | `?` の有無を問わない | 「`as` の束縛のパターンにも同じ規則を当てる」に |
| §29 | E505 は `as` の束縛（`?` の有無によらず） | `?` を消す |
| §33 | Option と失敗。境界は理由の有無。`List` の探索は Option | `Absent` を足し、境界を `Absent` と `Error` に。`Option` はデータで伝播しないと明記 |
| §34 | `as p? ->` | 題を「不在の効果 `Absent` と `Maybe`」にし、`Absent`・`Maybe`・効果版の例・`?` の E100 を書く |
| §35 | `e ->` は効果か Option。Option の伝播。`as _? ->` は E504 | 効果だけを受ける。Option の段と E504 を消す |
| §36 | Option の不在は `?` と `e ->` で伝わる。E508 | `?` と E508 の段を消し、責務表を効果と `if` と handler にする |
| §40 | 効果と Option の破棄 | 効果だけの破棄にする |
| §45 | 不要な再包装（Option） | 変えない（E507 は残る） |
| §54 | `raise` が届いたとき | `Absent` が届いたら操作名を印字して exit 1 を足す |
| §55 | 効果は `Error` `Env` `Io`。prelude の効果 | `Absent` を足し、`Maybe` を prelude の handler として書く |
| §56 | E303・E504・E507・E508 の行 | E303 を効果限定に。E504・E508 の行を削る。E507 は残す |
| §57 | E508 の書き換え | E508 の項を消す |
| §58.1 | 書き換えを持たない違反（E503 / E504） | E504 の綴りを消す |
| 目次 | §34 の題 | 追随させる |
| 付録A | `as p? ->` の使用 | `as p ->` の効果版に |

### 期待値の根拠

- 効果版（issue の `target/issue/absent/main.hd`）は、`effect` と `handler` の宣言を
  自分のファイルから消しても prelude だけで `check`・`build`・`run` が通り、
  `{ found = Some("hi alice"); missing = None }` を印字する。
- `Option` に対する `->` は E303。`?` は E100。
- `tests/accept`・`stdlib` の `?` は 0 件。`stdlib/option.hd` の `Option.map` は
  `if value | Some(x) => Some(f x) | None => None` にする。
- 既存の受理コーパスと stdlib の `->` はすべて効果の呼び出しなので、意味を変えずに
  通る。`as p ->` のデータ `Option` 束縛は E402（パターンが網羅しない）で拒否され、
  データ `Option` の検査は `if` の分岐で書く。

### 成果物

- `stdlib/absent.hd`（新設）・`crates/hado/src/lib.rs`: prelude の `Absent`・`Maybe`。
- `crates/hado/src/parser.rs`: `?` を E100 に。`LetQ` を作らない。
- `crates/hado/src/infer.rs`: `e ->` は効果だけ。E303 の文。
- `crates/hado/src/check.rs`・`fixes.rs`: E504・E508 の検査と書き換えを消す。
- `crates/hado/src/mono.rs`: 0 引数の abort の印字を操作名にする。`Option` の `->` の
  分岐を消す。
- `spec/errors.toml`・`spec/grammar.ebnf`・`docs/spec.md`・`docs/corpus.md`・`README.md`。
- `tests/accept/absent_effect`（新設）・`tests/accept/rail_forms`・`tests/accept/user_store`・
  `tests/reject/E303`・`tests/reject/E504`（削除）・`tests/reject/E508`（削除）・
  `tests/corpus.sha256`。
- 追随する `crates/hado/tests/*.rs`・`scripts/verify-*.sh`・`editors/tree-sitter-hado`。
- `scripts/verify-do-option-absent.sh`（新設、`# tier: core`・`# round: do-option-absent`）。
- 本ファイル。

### 工程

1. 赤を置く: `tests/accept/absent_effect`（prelude だけで通る効果版）と、門番の枠。
   main では `absent` が E204 で落ちることを示す。
2. prelude に `Absent`・`Maybe` を足し、赤を緑にする。
3. `Option` の `->` を E303、`?` を E100 にする。拒否コーパスを置き、E504・E508 の
   検査・書き換え・コーパス・§56 の行を消す。
4. 受理コーパス・stdlib を効果版へ書き直す（`?` を 0 に）。
5. `main` に `Absent` が残る経路を実装する。
6. 仕様・`spec/`・`README.md`・`docs/corpus.md` を追随させ、`spec-lock.sh` で表を作り直す。
7. 追随する門番・テスト・tree-sitter を直す。
8. 上記を工程ごとにコミットする（`docs:` は仕様の書き直しだけ）。
9. ラウンド最後に全走。

### 判定

- 効果版が prelude だけで `check`・`run` を通り、`{ found = Some("hi alice"); missing = None }`。
- `Option` に対する `->` が E303、`?` が E100。`cargo test --offline` が 0 failed。
- `tests/accept`・`stdlib` の `?` が 0。`bash scripts/lint-docs.sh` が PASSED。
- `bash scripts/run-gates.sh --tier quick`・`--tier core`・`--tier full` に、この
  ラウンドに由来する赤が無い。`bash scripts/verify-spec-contracts.sh` が赤なし。
- 新設の門番の各検査が、対応する変異で赤になる。

### 許容される赤

- full 層の既知の赤（`verify-r62`・`verify-r63`・`verify-r68`・
  `verify-string-split-linear` の検査 i）は main でも赤。

### スコープ外

- `List.head`・`List.nth`・`Map.find`・`Env.env` の効果化（決定 5）。
- `Option` を効果へ持ち上げる専用の prelude 関数（`Option.ok_or` で `Error` へは行ける）。
- `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` への追記。
- `handler Absent as Maybe` 以外の `Absent` の回復形（既定値付き handler などは
  `handler-value-args` の範囲）。

### 中断の条件

- 判定が 2 回続けて赤になったら止める。
- 仕様の解釈に疑義が出たら仕様を変えず、依存しない作業だけ進める。
- §33 の書き直しが §33 から §36・§40 の外へ波及しすぎて、このラウンドで閉じられない
  と分かったら止める。

### 仕様への疑義（人が見る）

```
主張:     `?` の廃止で、`docs/plan.md` §11.2 の項目 `let p? =` の受理コーパスが
          原理的に書けなくなる。計画は凍結なので、項目名と `docs/spec-coverage.md` の
          対応行はそのまま残る
再現:     `bash scripts/verify-m1-corpus.sh` は `docs/spec-coverage.md` の各項目に
          `// item: <項目名>` を持つ受理コーパスが 1 本以上あることを要求する
現行:     受理の `let p? =` は `tests/accept/rail_forms` の 1 本だけ。これを効果版へ
          書き直すと `?` を使う受理コーパスが 0 本になる
変えると: 計画と門番のどちらかを動かす必要がある。門番の免除リストに `let p? =` を
          足せば門番は緩む。項目タグを効果版の受理コーパスへ残せば、項目名が
          「今は効果で書く形」を指すことになる
推奨:     項目タグを効果版の受理コーパス（`tests/accept/rail_forms`）へ残す。受理の
          形は §34 の効果版に置き換わっており、`docs/spec-coverage.md` の対応行
          （§34・§35）もそのまま使える。審査で見てほしい
```

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §34 | 不在は prelude の `Absent` 効果で表し、`Maybe` が `Option` へ戻す | 実行 | — | `tests/accept/absent_effect` |
| §34 | `as p? ->` は廃止。`?` は E100 | 構文 | E100 | `tests/unit/parser.rs`・門番の変異 |
| §35 | `e ->` は効果を持つ式だけを受ける | 型 | E303 | `tests/reject/E303` |
| §40 | 破棄に意味があるのは効果を持つ式だけ | 冗長 | E506 | `tests/reject/E506` |
| §55 | prelude の効果は `Absent` `Error` `Env` `Io` | 名前解決 | — | `scripts/verify-do-option-absent.sh` |
| §54 | `Absent` が `main` に届いたら操作名を印字して exit 1 | 実行 | — | `scripts/verify-do-option-absent.sh` |

## 実測

### Red（main の現象）

効果版の受理コーパスを main の prelude で検査すると、`Absent` が名前解決で落ちる。
`?` と `Option` に対する `->` は受理され、伝播していた。

```
$ cargo run --offline -q -- check tests/accept/absent_effect/main.hd
main.hd:6:32: error[E204]: unknown effect Absent
$ cargo run --offline -q -- check tests/reject/E303/main.hd
（受理。診断を出さない。`find 0 ->` が None を伝播していた）
```

同じ赤は、実装後の木で `stdlib/absent.hd` の `effect Absent` を外す変異でも再現する
（下の変異 c）。

### Green

```
$ grep -c 'test result: ok' target/gates/cargo-test.log
73
$ grep -c 'test result: FAILED' target/gates/cargo-test.log
0
$ cargo test --offline; echo EXIT=$?
EXIT=0
```

```
$ bash scripts/verify-do-option-absent.sh
OK: a no accept corpus or stdlib file spells `as p? ->`
OK: b an Option discard is E303 and `?` is E100
OK: c the prelude-only absent program runs as its expect tag says
PASSED: 3/3
```

```
$ bash scripts/verify-spec-contracts.sh
OK: the spec body keeps the needles of the dropped gates
OK: the gate tier lock matches the membership derived from # tier:
PASSED: 2/2
$ bash scripts/verify-spec-model.sh
OK: docs/spec.md matches spec/ (spec-lock --check)
OK: the model is well formed and its sections exist
OK: the docs layer refuses to run when assert is disabled
PASSED: 3/3
$ bash scripts/verify-m1-spec.sh
OK: 3 error-codes
PASSED: 1/1
$ bash scripts/verify-m1-corpus.sh
OK: 7 protected-files-unchanged
PASSED: 7/7
```

新設の門番の各検査は、対応する変異で赤になる（変異は一時的に当てて戻す）。

```
$ sed -i '1s/^effect Absent/\/\/ effect Absent/' stdlib/absent.hd   # 変異 c: prelude から外す
$ bash scripts/verify-do-option-absent.sh
NG: b an Option discard is E303 and `?` is E100 (discard=E100 question=E100)
NG: c the prelude-only absent program runs as its expect tag says (seen=1 output:tests/accept/absent_effect/main.hd)
PASSED: 1/3
$ printf '\ndef extra = Some(1) as x? -> x\n' >> tests/accept/rail_forms/main.hd   # 変異 a
$ bash scripts/verify-do-option-absent.sh
NG: a no accept corpus or stdlib file spells `as p? ->` (tests/accept/rail_forms/main.hd:14:def extra = Some(1) as x? -> x)
PASSED: 2/3
$ sed -i "s/def main = Some(1) as x? -> x/def main = Some(1) as x -> x/" scripts/verify-do-option-absent.sh   # 変異 b: 検査の入力を旧綴りに
$ bash scripts/verify-do-option-absent.sh
NG: b an Option discard is E303 and `?` is E100 (discard=E303 question=E501)
PASSED: 2/3
```


