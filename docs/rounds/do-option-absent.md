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
2. **`as p? ->` と `let p? =` を廃止する。** `?` は文法から外し、`as p ->` の後ろと
   移行の入口（`print-postfix` が読む旧形の `let`）のどちらに `?` を書いても E100 と
   する（`let`・`do` と同じ「廃止した綴り」の扱い）。構文木には `?` の束縛を表す
   ノードを残さない。
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

```
主張:     `print-postfix` は、移行の入口で読めなかった項目を黙って落とし、空の
          ファイルを書く（`let p? =` は今回 E100 になり、この経路に入る）
再現:     `printf 'def f value => let x? = value -> x
' > /tmp/l/legacy.hd`
          `cargo run --offline -q -- print-postfix /tmp/l /tmp/out`
          → `skipped=0` と表示し、`/tmp/out/legacy.hd` は空になる
現行:     字句で読めないときだけ `skipped` に数え、構文で読めない項目は AST から
          落ちて空の出力になる（今回の変更の前から同じ）
変えると: 空ファイルは移行の結果として危ない。読めなかった項目を `skipped` に数えるか、
          診断を出して書かないようにすると、`print-postfix` の出力契約が変わる
推奨:     このラウンドでは直さず、`docs/issues/print-postfix-drops-unconvertible-items.md`
          へ切り出す（`?` 以外の壊れた入力でも同じことが起きるため、`?` の廃止とは
          別の欠陥）。審査で優先度を見てほしい
```

```
主張:     §35 の「`e ->` の値は効果を持つ式でなければならない」と「`Option` を `e ->`
          の値にはできない（E303）」が食い違い、**効果を持つ `Option` の式**が E303 に
          なる。さらに E506（`as _ ->`）は「`->` を書け」と E303 側へ案内し、2 つの
          診断が互いを指す
再現:     `def f = env "X"`（型は `String Option {Error}`）として
          `f as _ -> ()` は E506 `` a plain binding cannot discard its value; write `->` ``、
          `f -> ()` は E303 `` `e ->` requires an effect ``（下の「実測」の疑義の再現）
現行:     `infer::propagation` は効果の行より先に `Option` を弾く。§35 の 2 文目と
          E303 のアンカー「効果を持つ式でない式」は、効果を持つ `Option` の場合を
          含まない。E506 の案内は `->` だけを指す
変えると: 効果を持つ `Option` を `->` で捨てられるようにするなら、§35 の 2 文目と
          E303 のアンカーを「効果を持たない式」に絞り直し、E506 の案内も
          「`as p ->` で束縛するか、効果の値を `->` で捨てる」へ分ける必要がある。
          仕様本文とエラー表の書き換えになる
推奨:     §35 の 2 文目を「`Option` の値は `->` で捨てられない。`Absent` へ持ち上げるか
          `as p ->` で束縛する」と明示し、E506 の案内を分ける。仕様の解釈なので人が
          決めてほしい
```

```
主張:     凍結項目 `let p? =` の `// item:` タグをどの受理コーパスに残すか
再現:     `grep -rn 'item:' tests/accept` は
          `tests/accept/rail_forms/main.hd:3:// item: let p? =` を返す。このファイルの
          本体は `if` と `raise` で `Absent` を使っておらず、`// spec: §34` も
          内容と合わない（`?` の伝播は `tests/reject/E303` と
          `tests/postfix-close/reject/N12` が拒否側で固定している）
現行:     凍結した `docs/spec-coverage.md` の対応行が受理コーパス 1 本を要求するため、
          `rail_forms` にタグだけを残している。`docs/plan.md` と `docs/spec-coverage.md`
          は凍結なので動かしていない
変えると: タグと `// spec:` を `tests/accept/absent_effect`（効果版の本体）へ移せば、
          項目名が指す形と中身が揃う。`docs/spec-coverage.md` の対応行（`§34, §35`）と
          凍結した §11.2 の関係を人が確かめる必要がある
推奨:     タグと `// spec:` を `tests/accept/absent_effect` へ移し、`rail_forms` は
          §40 の受け皿として残す。凍結文書に触れずに済むかの判断を審査で見てほしい
```

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §34 | 不在は prelude の `Absent` 効果で表し、`Maybe` が `Option` へ戻す | 実行 | — | `tests/accept/absent_effect` |
| §34 | `as p? ->` と `let p? =` は廃止。`?` は E100 | 構文 | E100 | `tests/unit/parser.rs`・`tests/postfix-close/reject/N12` |
| §35 | `e ->` は効果を持つ式だけを受ける | 型 | E303 | `tests/reject/E303` |
| §40 | 破棄に意味があるのは効果を持つ式だけ | 冗長 | E506 | `tests/reject/E506` |
| §55 | prelude の効果は `Absent` `Error` `Env` `Io` | 名前解決 | — | `scripts/verify-do-option-absent.sh` |
| §54 | `Absent` が `main` に届いたら操作名を印字して exit 1 | 実行 | — | `crates/hado/tests/mono.rs`・`scripts/verify-do-option-absent.sh` |

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
OK: d an unhandled absent exits 1 and prints the operation name
PASSED: 4/4
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
$ printf '\ndef extra = Some(1) as { value = Box(n) }? -> n\n' >> tests/accept/rail_forms/main.hd   # 変異 a: 空白入りの pattern
$ bash scripts/verify-do-option-absent.sh
NG: a no accept corpus or stdlib file spells `as p? ->` (tests/accept/rail_forms/main.hd:13:def extra = Some(1) as { value = Box(n) }? -> n)
PASSED: 3/4
$ sed -i "s/def main = Some(1) as x? -> x/def main = Some(1) as x -> x/" scripts/verify-do-option-absent.sh   # 変異 b: 検査の入力を旧綴りに
$ bash scripts/verify-do-option-absent.sh
NG: b an Option discard is E303 and `?` is E100 (discard=E303 question=E501)
PASSED: 3/4
$ sed -i 's/0 => Expression::Constant(Constant::String(fallback.into())),/0 => Expression::Constant(Constant::String(String::new())),/' crates/hado/src/mono.rs   # 変異 d
$ bash scripts/verify-do-option-absent.sh
NG: d an unhandled absent exits 1 and prints the operation name (status=1 stderr=warning: unused variable: `fallback`)
PASSED: 3/4
```

検査 a の探し方は、行コメントと文字列を外してから `?` + `->` を見るので、`// item: let p? =`
の記録用の `?` と文字列の中の `?` は綴りと数えない。誤検出しないことを、受理コーパスへ
一時的に足して確かめる。

```
$ printf '// note: as x? -> y\ndef extra = String.concat "?" "as x? -> y"\n' >> tests/accept/rail_forms/main.hd
$ bash scripts/verify-do-option-absent.sh
OK: a no accept corpus or stdlib file spells `as p? ->`
PASSED: 4/4
```

§35 の疑義の再現。

```
$ cat target/p3/main.hd
def f = env "X"
def main =
  f as _ ->
  ()
$ cargo run --offline -q -- check target/p3/main.hd
main.hd:3:8: error[E506]: a plain binding cannot discard its value; write `->`; canonical form: （束縛を削除）
$ cat target/p2/main.hd
def f = env "X"
def main =
  f ->
  ()
$ cargo run --offline -q -- check target/p2/main.hd
main.hd:3:3: error[E303]: `e ->` requires an effect
```

層別の全走（`cargo test --offline` の後、木を止めて 1 回ずつ）。

```
$ bash scripts/run-gates.sh --tier quick target/gates/quick
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates/core
TALLY: 720/720 passed; red: none
$ bash scripts/run-gates.sh --tier full target/gates/full
TALLY: 3841/3849 passed; red: verify-r62.log verify-r63.log verify-r68.log verify-string-split-linear.log
```

full の赤 4 本は main でも赤の既知の赤（`verify-r62`・`verify-r63`・`verify-r68` は
E402 の期待ずれ、`verify-string-split-linear` は検査 i の性能判定）。このラウンドの
成果物で新しく赤になった門番は無い。`verify-postfix-binding` は、`?` を印字する枝の
削除で変異 `drop-option-question` の対象が消えたため、その変異を外して実測 4/4。

```
$ bash scripts/verify-postfix-binding.sh
J3:印字器の変異(cases=4): PASS
PASSED: 4/4
```

## rollback（restore-option-question）

M7 は決定 D2（不在を効果 `Absent` で表す）を入れたが、同時に人が認めていない `?` の
構文を消した。このラウンドはその削除だけを戻す。D2 の成果は残す。両者は同居する。
`as p? ->` はデータの `Option` の伝播、`Absent` は効果としての不在を表す。

### 戻した（人の差分 `git diff 8472497 eeb1704` の 1〜9）

| # | 戻したもの | 固定するもの |
|---|---|---|
| 1 | `as p? ->` の受理（`parser.rs` の `NodeKind::LetQ`、`spec/grammar.ebnf` の `rail_binding`、`check` / `infer` / `mono` / `fixes` の腕） | `tests/accept/option_propagation`・`tests/unit/parser.rs`・`tests/postfix-close/accept/P5` |
| 2 | E508（`spec/errors.toml` の行、§56 の表、`tests/reject/E508/`） | `tests/reject/E508`・`scripts/verify-r57.sh` |
| 3 | §34 の題を `` `as p? ->` `` に戻す（`Absent` と `Maybe` の本文は残す） | `docs/spec.md` §34 |
| 4 | §2・§10・§11・§28・付録A の `?` の文と例（`e as whole -> whole as Some(x)? ->` を含む） | `docs/spec.md`・`scripts/verify-spec-contracts.sh` |
| 5 | `scripts/verify-r57.sh` の 57-c / 57-d（旧 E508 の形が受理されることを測る） | `scripts/verify-r57.sh` |
| 6 | `scripts/verify-spec-contracts.sh` の針 `('r57','57','E508')` と `('r57','57','本体ちょうど')` | `scripts/verify-spec-contracts.sh` |
| 7 | `scripts/verify-postfix-binding.sh` の変異 `drop-option-question` | `scripts/verify-postfix-binding.sh` |
| 8 | `tests/postfix-close/accept/P5/main.hd` の `?` の行 | `tests/postfix-close/accept/P5` |
| 9 | `crates/hado/src/parser.rs` の `LetQ` と各段の腕 | `cargo test --offline` の 11 本の判定器 |

### 人が差分に挙げていないが、1〜9 を戻すために必要だったもの

- `stdlib/option.hd` の `Option.map` を `value as x? -> Some(f x)` に戻した。戻さないと
  prelude 自身が E508（`| None => None` の再伝播）を出す。
- `tests/accept/option_propagation` を足した。`drop-option-question` の変異は受理
  コーパスと stdlib を正準形として往復させる検査（`corpus_is_canonical_and_round_trips`）が
  捕まえるので、`tests/postfix-close/accept` の P5 だけでは足りない。
- `editors/tree-sitter-hado` の `rail_binding`（`grammar.js`・`tests/golden/rail-binding.hd`・
  `.tree`）と `scripts/treesitter-check.py` の `GOLDEN_NAMES` を戻した。受理コーパスを
  parse する core の門番 `verify-treesitter` の検査 2 が、`?` を受理コーパスへ戻した
  時点で赤になる（下の変異）。
- E303 のアンカーを `Option` の `e ->` と `as p? ->` の両方に広げた（陰の短い名前は
  `discard-type`、節は §35 のまま。`tests/reject/E303` の `// spec: §35` に合わせる）。
- §11・§33・§35・§36 の `Option` の伝播の散文を、戻した `?` と残した D2
  （`e ->` は効果専用、E303）の両方に合わせて直した。

### 戻さなかった（M7 の成果）

- 効果 `Absent`・handler `Maybe`・`abort absent`（`stdlib/absent.hd`）、`tests/accept/absent_effect`、
  `main` に残った `Absent` の操作名印字と exit 1。
- `e ->` の値が `Option` なら E303（`e ->` は効果専用）。
- `List.head`・`List.nth`・`Env.env` は `Option` を返す。
- `let p? =` は E100（移行の入口でも）。`?` を認めるのは `as p? ->` だけである。
- E504 と `tests/reject/E504`。`as _? ->` は受理され、書き換えを持たない（`verify-r71.sh`）。
- `tests/accept/rail_forms`・`tests/accept/user_store` の `?`（M7 の実験で 0 にした）。
- `README.md` の拒否例は E507 のまま（正確である）。
- `docs/plan.md` §17.4 の「E504 / E507 / E508 は Option 限定に縮小して存続する」は
  凍結文書なので触らない。D2 の決定と食い違う点は「仕様への疑義」に書く。

### 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §34 | `as p? ->` はデータの `Option` を伝播し、`e ->` は効果だけを実行する | 構文 / 型 | E303 | `tests/accept/option_propagation`・`tests/reject/E303` |
| §36 | `\| Some(p) => body` と `\| None => None` の 2 腕は `as p? ->` へ書き換わる | 冗長 | E508 | `tests/reject/E508`・`scripts/verify-r57.sh` |
| §34 | `as _? ->` の破棄は受理され、書き換えを持たない（E504 は戻さない） | 冗長 | — | `scripts/verify-r71.sh` |
| §34 | 不在は効果 `Absent` で表し、`Maybe` が `Option` へ戻す | 実行 | — | `tests/accept/absent_effect`・`scripts/verify-do-option-absent.sh` |
| §55 | prelude の効果は `Absent` `Error` `Env` `Io` | 名前解決 | — | `scripts/verify-do-option-absent.sh` |

### 実測

Red（M7 の木。`?` は E100、E508 は出ない）。

```
$ cargo run --offline -q -- check tests/accept/option_propagation/main.hd
main.hd:4:42: error[E100]: `as p? ->` is removed; absence is an effect (bind with `as p ->`)
$ cargo run --offline -q -- check tests/reject/E508/main.hd
（診断を出さない。終了コード 0）
```

Green（このラウンドの木）。

```
$ cargo run --offline -q -- check tests/accept/option_propagation/main.hd
$ cargo run --offline -q -- run tests/accept/option_propagation/main.hd
{ none = None; some = Some(42) }
$ cargo run --offline -q -- check tests/reject/E508/main.hd
main.hd:4:60: error[E508]: absence bound at 4:62 is propagated unchanged; use `as p? ->` or `->`; canonical form: find_user id as user? -> Some(user + 1)
```

`?` と D2 の同居（`as p? ->` は `Option` の伝播、`e ->` は効果専用、`let p? =` は E100）。

```
$ bash scripts/verify-do-option-absent.sh
OK: a the accept corpus or stdlib spells `as p? ->`
OK: b an Option discard is E303 and `?` on a non-Option value is E303
OK: c the prelude-only absent program runs as its expect tag says
OK: d an unhandled absent exits 1 and prints the operation name
PASSED: 4/4
$ bash scripts/verify-r71.sh
OK: 71-a the discarded Option binding is accepted
OK: 71-b check --fix leaves the canonical spelling alone
OK: 71-c fmt leaves the canonical spelling alone and is idempotent
OK: 71-d Some discards its payload and None propagates
PASSED: 4/4
```

文法の変異。`verify-treesitter` の検査 2 と同じ手順（`tree-sitter generate` / `build` /
`parse`）を、文法だけ M7 の `git show eeb1704:editors/tree-sitter-hado/grammar.js` に
戻して回す。`tests/accept/option_propagation/main.hd` の `?` が ERROR になる。

```
$ python3 target/roundlogs/probe-treesitter-mutation.py "$ts" target/roundlogs/ts-probe
restored: exit=0 ERROR/MISSING=0
without-rail_binding: exit=1 ERROR/MISSING=1
```

層別の全走（`cargo test --offline` の後、木を止めて 1 回ずつ）。

```
$ cargo test --offline
test result 73 本が ok、FAILED は 0（ログは target/roundlogs/cargo-test3.log）
$ bash scripts/run-gates.sh --tier quick target/gates/quick
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates/core
TALLY: 725/725 passed; red: none
$ bash scripts/run-gates.sh --tier full target/gates/full
TALLY: 3846/3854 passed; red: verify-r62.log verify-r63.log verify-r68.log verify-string-split-linear.log
```

full の赤 4 本は main でも赤の既知の赤（`verify-r62`・`verify-r63`・`verify-r68` は
E402 の期待ずれ、`verify-string-split-linear` は検査 i の性能判定）。このラウンドの
成果物で新しく赤になった門番は無い。M7 の実測（quick 41/41、core 720/720、
full 3841/3849）と比べて、赤の顔ぶれは同じで、通過数は 5 増えた（戻した
`verify-r57` と `verify-postfix-binding` の検査が増えた分）。減った門番は無い。

```
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
$ bash scripts/verify-treesitter.sh
PASSED: 5/5
```

### 着地前の再走（main `621002c` 取り込み後）

`main` はこのラウンドの間に dogfood-text-tool（PR #70）の分だけ進んでいた。取り込んだ
（衝突なし）。main は locked コーパス（`tests/accept`・`tests/reject`・`stdlib`）を
変えていないので、`corpus-lock.sh --check` は差分なしで通り、作り直しは要らなかった。
main の新しい `tools/dogfood-text-tool/main.hd` は効果を持つ prelude を `as p ->` で
束縛しており、戻した仕様でも `check` は 0 で通る。

```
$ bash scripts/corpus-lock.sh --check
（差分なし、終了コード 0）
$ cargo run --offline -q -- check tools/dogfood-text-tool/main.hd
（終了コード 0）
$ cargo test --offline
test result 73 本が ok、FAILED は 0、終了コード 0
$ bash scripts/verify-do-option-absent.sh
PASSED: 4/4
$ bash scripts/verify-r71.sh
PASSED: 4/4
$ bash scripts/verify-r57.sh
PASSED: 6/6
$ bash scripts/verify-postfix-binding.sh
PASSED: 4/4
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
$ bash scripts/verify-treesitter.sh
PASSED: 5/5
$ bash scripts/verify-dogfood-text-tool.sh
PASSED: 7/7
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/run-gates.sh --tier quick target/gates/quick-postmerge
TALLY: 41/41 passed; red: none
```


