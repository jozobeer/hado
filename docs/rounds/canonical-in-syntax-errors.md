# 構文と分岐の拒否に正準の綴りを出す（canonical-in-syntax-errors）

## 手順書

### 背景

- 引数を括弧ごとに分けた `def` は `expected =>, found (`（E100）、Boolean branch を
  pattern の腕に置いた形は `branches cannot be nested inside another branch`（E101）で、
  どちらも正しい綴りを示さない（`docs/issues/canonical-in-syntax-errors.md`）。
- この 2 つは読み込みの段階で止まり、`Project` を作らずに印字されるので、
  `--format json` を付けても既定の 1 行で出る。

### 決定

1. `Diagnostic` に `suggestion`（形の提示）を足す。値は 1 バイトの列挙 `Suggestion`
   （`&'static str` にすると `Diagnostic` が 128 バイトに達し、clippy の
   `result_large_err` に当たる）。
2. parser が形を知っている箇所で埋める。`parse_params` は閉じ括弧の直後の `(` に、
   `parse_pattern_arm_body` は腕の `if` が Boolean branch のときに添える。Boolean branch
   かどうかは、括弧の外で `=>` と `|` のどちらが先に来るかで決める
   （`boolean_branch_ahead`）。メッセージは変えない。
3. 既定の 1 行は末尾に `; suggestion: ...`、JSON は `suggestion` フィールド。
   `fmt` と `--fix` は適用しない。
4. 読み込みの失敗は `load_error_report` で描き、check・build・`check --fix`・run・test の
   どれでも `--format json` に従わせる。書き換えは引けないので `canonical` は `null`、
   終端はアンカーと同じ位置。
5. §58.4 のフィールドを 10 にし、`suggestion` の意味を書く。
6. E101 の suggestion（`| p => if | c => a | b`）の適用範囲は、§24 と §26.1 の貪欲な
   読み（現行のまま、決定 A）とする。腕の本体の Boolean branch の else が続く腕の綴りを
   食う形（`if true => 1` の次に `| B => 2`）には添え、else を欠く形
   （`| 0 => if true => 1 |`）には添えない（`docs/issues/e101-suggestion-scope.md`）。
   退ける案（先読みの終端が `=>` のときは添えない）は、§58.4 の「Boolean branch を
   pattern の腕に置いた形（E101）には `| p => if | c => a | b` を添え、他の形には
   添えない」が定める範囲を実装で狭め、仕様の書き換えを要する。合法な条件列の扱いは
   両案で変わらない。

### code-complete の一周目

- 意図: 仕様を引かずに、診断だけで正しい綴りへ直せるようにする。
- 仮説: 形は parser が知っているので、組み立てたときに `Diagnostic` へ載せれば、
  既定の 1 行と JSON の両方に 1 つの値から出せる。
- 判定器: 下の「判定」。
- スコープ: `crates/hado/src/parser.rs`・`project.rs`・`main.rs`（と `Diagnostic` を
  組み立てる各所）、§58.4、門番、本ファイル。
- 未確認: `Diagnostic` を組み立てる箇所の数と、フィールドを足したときの大きさ。

### code-complete の一周目（e101-suggestion-scope）

決定 A の記録が成果物で、実装は変えない。Red は「検体が赤になりうること」を欠陥の
注入で示す（既存の判定と同じ形）。

- 意図: E101 の suggestion をどの ill-formed な入力に添えるかを決定 A として記録し、
  門番で固定する。使う人: hado の作者と builder。場面: 適用範囲を変える変更を入れる
  とき、退ける案 B を門番が赤にする。
- 仮説: 現行実装は決定 A のとおりに動く。`boolean_branch_ahead` は先読みの
  `parse_branch` が `BoolBranch` になるときだけ真になるので、`| B` が else の式として
  読める形には綴りが付き、else を欠く形には付かない。したがって `parser.rs` を変えず、
  門番に「添える側」の検体を足すだけで決定を固定できる。
- 判定器: `bash scripts/verify-canonical-in-syntax-errors.sh` が全件緑。両側の検体が
  `suggestion` の値そのものを見る（else を欠く `incomplete-boolean-branch` は
  `suggestion is None`、添える側の `arm-if-else-next-arm` は
  `| p => if | c => a | b`）。変異（`boolean_branch_ahead` を常に真、先読みの終端が
  `=>` なら偽）で赤。
- スコープ: `scripts/verify-canonical-in-syntax-errors.sh`・本ファイル・
  `docs/issues/e101-suggestion-scope.md`（消す）。`crates/hado/src/parser.rs`、
  `docs/spec.md` の §24・§26.1・§58.4、`docs/plan.md`、`scripts/gate-tiers.lock` は
  触らない。
- 未確認: 退ける案 B に変えたとき、既存の `nested`・`paren-in-arm` が suggestion を
  失うか（失うなら検体の置き場が悪い）。`incomplete-boolean-branch` が suggestion の
  不在そのものを見ているか。

実装から分かったこと。仮説は当たり、`parser.rs` は変えずに済んだ。
`incomplete-boolean-branch` は既に `no_suggestion` が `suggestion is None` を見ている。
`boolean_branch_ahead` を常に真にすると、この検体を先頭に置いたとき
`incomplete-boolean-branch` 自身が `suggestion: "| p => if | c => a | b"` を返して赤に
なる。追加した `arm-if-else-next-arm` は、先読みの終端が `=>` なら偽にする変異
（退ける案 B）で 5 件が赤になる。既存の `nested`・`paren-in-arm` は同じ変異で赤に
ならない（else が同じ行にあるため）ので、検体の置き場は TARGETS で正しい。

追加で分かったのは、続く `|` の右が `_` の形（`| 0 => if true => 1` の次に `| _ => 2`）
には suggestion が付かないこと。`_` は式として読めないので `if true => 1 | _` が
`BoolBranch` にならない。決定 A の「続く `| B` が else として読める形」の「読める」は、
`_` を除く式・リテラル・変数・constructor を指す。

### 期待値の根拠

綴りは §8（`def f(a: T, b: U): R =>`）と §26.1（`| p => if | c => a | b`）の本文。
他の E101 の形（`fn(| ... )`、腕の中の pattern branch、条件列の腕の中の分岐）と
他の E100 には、当たらない提示を出さない。

### 成果物

`scripts/verify-canonical-in-syntax-errors.sh`、上のソース、§58.4、本ファイル。

### 工程

門番（赤）→ `report_json` から `diagnostic_json` を分ける → 提示と読み込みの経路 →
欠陥の注入 → §58.4。

### 判定

- `bash scripts/verify-canonical-in-syntax-errors.sh` が、修正前は 1/9（通るのは
  「提示を適用しない」の対照）、修正後は 9/9。
- 欠陥を注入すると赤。`boolean_branch_ahead` を常に真にすると腕の中の pattern branch に
  提示が出て赤、`execute` の読み込みの失敗を既定の 1 行に戻すと run と test が赤。
- `cargo test --offline` が 0 failed、clippy が赤なし、`bash scripts/verify-r65.sh` が
  赤なし。

### 許容される赤

無し。

### スコープ外

- 読み込みの後の検査で止まる診断が、`build`・`run`・`test` で `--format json` を
  無視する（`docs/issues/build-json-diagnostics.md`）。
- LSP の診断のメッセージ。LSP は `message` だけを使い、提示は載せない。

### 中断の条件

`Diagnostic` にフィールドを足すと clippy か LSP の門番が赤になり、1 バイトの値で
収まらないときは、提示を `Diagnostic` の外（ソースから後で計算する `canonical_hint` の形）
に移す。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §58.4 | E100・E101 の 2 つの形に正準の綴りを `suggestion` として添え、`fmt` と `--fix` は適用しない | 構文 | E100, E101 | `scripts/verify-canonical-in-syntax-errors.sh` |
| §24・§26.1・§58.4 | E101 の `suggestion` の適用範囲は貪欲な読み（else を欠く形には添えず、続く `\|` が else として読める形には添える） | 構文 | E101 | `scripts/verify-canonical-in-syntax-errors.sh` の `arm-if-else-next-arm`・`incomplete-boolean-branch` |

## 実測

修正前（df4f2ac）。

```
$ cargo run --offline -q -- check --format json e100/main.hd
main.hd:1:36: error[E100]: expected =>, found (
$ cargo run --offline -q -- check --format json e101/main.hd
main.hd:4:12: error[E101]: branches cannot be nested inside another branch
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 1/9
```

修正後。

```
$ cargo run --offline -q -- check e100/main.hd
main.hd:1:36: error[E100]: expected =>, found (; suggestion: def f(a: T, b: U): R =>
$ cargo run --offline -q -- check e101/main.hd
main.hd:4:12: error[E101]: branches cannot be nested inside another branch; suggestion: | p => if | c => a | b
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 9/9
$ cargo test --offline -q 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 750 failed 0
$ bash scripts/verify-r65.sh | tail -1
PASSED: 5/5
```

欠陥の注入。

| 注入 | 門番 |
|---|---|
| `boolean_branch_ahead` を常に真にする | `PASSED: 8/9`（other E101 shapes が NG） |
| `execute` の読み込みの失敗を `error.to_string()` に戻す | `PASSED: 7/9`（run と test が NG） |

一次レビュー（Codex）の指摘 1 件を修正と門番に取り込んだ（補修）。`test` は根の
列挙の失敗を、`build --fix` は整形の前段の失敗を、既定の 1 行で出していた。どちらも
`LoadError` なので `load_error_report` に通した。指摘は `test` の 1 か所だが、
`source_paths` の失敗が同じ形で残っていたので揃えた。

```
# 直す前（親ディレクトリが無い入力を `test --format json` に与える）
$ target/debug/hado test --format json /tmp/nope/main.hd
/tmp/nope:1:1: error: No such file or directory (os error 2)
# 直した後は 1 行の JSON（`check` と同じ形）
$ target/debug/hado test --format json /tmp/nope/main.hd
{"path": "/tmp/nope", "line": 1, "col": 1, "end_line": 1, "end_col": 1, "code": null, ...}
$ target/debug/hado build --fix --format json /tmp/nope/main.hd
{"path": "/tmp/nope", "line": 1, "col": 1, "end_line": 1, "end_col": 1, "code": null, ...}
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 10/10
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
# 探針（`run_tests` と `format_sources` を既定の 1 行に戻す）
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 9/10（a load error is JSON for every command that takes --format が NG）
```

二次レビュー（Codex）の指摘 4 件（うち 1 件は同文が 2 スレッド）。4 件とも直した。

1 件目は `boolean_branch_ahead` が宣言を越えて `=>` を探していたこと。腕の本体が
不完全な `if broken` のとき、続く宣言の `=>` を Boolean branch の印と読み、
pattern branch の形にも ArmConditions の綴りを添えていた。

```
$ cat leak/main.hd
def f x =>
  if x
    | 0 => 0
    | _ => if broken
def g y => y
$ cargo run --offline -q -- check --format json leak/main.hd
{"path": "main.hd", ..., "code": "E101", "suggestion": "| p => if | c => a | b", ...}
# 宣言を始めるキーワード（`declaration_keyword`）で探索を止めた後
$ cargo run --offline -q -- check --format json leak/main.hd
{"path": "main.hd", ..., "code": "E101", "suggestion": null, ...}
```

2 件目は 2 個目の `(` がパラメータ群を始めるときだけ Parameters の綴りを添える
こと。`def f(a: Int) (1)` は引数を括弧ごとに分けた形ではない（§8、§58.4）。指摘に
あった `def f(a: Int) () => a` は直す前から綴りを添えていない（別の経路で
`expected =>, found ()` になる）。

```
$ cargo run --offline -q -- check --format json group/main.hd   # def f(a: Int) (1)
{"path": "main.hd", ..., "code": "E100", "suggestion": "def f(a: T, b: U): R =>", ...}
# 直した後
{"path": "main.hd", ..., "code": "E100", "suggestion": null, ...}
# 対照（分割した形には添えたまま）
$ cargo run --offline -q -- check --format json other/main.hd   # def f(a: Int) (b: Int): Int => a
{"path": "main.hd", ..., "code": "E100", "suggestion": "def f(a: T, b: U): R =>", ...}
```

3 件目と 4 件目は門番の側。対照の検体（E101 の fn の括弧、E100 の型の綴り、
E520 の書き換え）をスクリプト内で作るようにして、コーパスのパスを読まなくした。
`hado` は `cargo run --offline -q --` で起こし、`target/` の配置を決め打ちしない。
コーパス側の同じ形は `cargo test` のコーパス判定が見ている。

```
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 10/10
# 注入（宣言の境界を外す / 2 個目の括弧を確かめない）
NG: other E101 shapes have no suggestion: ('incomplete-before-declaration', ...)
NG: other E100 shapes have no suggestion: ('literal-group', ...)
$ cargo test --offline 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 753 failed 0
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/run-gates.sh --jobs 4 --tier quick gate-out 2>&1 | tail -1
TALLY: 40/40 passed; red: none
```

コーパスのパスを読む門番は他にも残っている（`verify-r69.sh` の `E502`、
`verify-r71.sh` の `E504`、`verify-prelude-tail-recursion.sh` の
`list_map_head_order` など）。この門番は検体を自分で作る形にしたが、既存の門番は
そのままにした。

三次レビュー（Codex）の指摘 1 件。2 個目の括弧の中身を最後まで見ていなかった。

`def f(a: Int) (b)` の `(b)` は型を持たないので、引数を括弧ごとに分けた形ではない
のに Parameters の綴りを添えていた。

```
$ cargo run --offline -q -- check --format json group/main.hd   # def f(a: Int) (b)
{"path": "main.hd", ..., "code": "E100", "suggestion": "def f(a: T, b: U): R =>", ...}
# 直した後
{"path": "main.hd", ..., "code": "E100", "suggestion": null, ...}
```

`name: Type` の列と閉じ括弧まで先読みする。型の中の括弧は数えない。

```
$ cargo run --offline -q -- check --format json other/main.hd   # def f(a: Int) (b: Option(Int)): Int => a
{"path": "main.hd", ..., "code": "E100", "suggestion": "def f(a: T, b: U): R =>", ...}
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 10/10
# 注入（名前が小文字なら true）
NG: other E100 shapes have no suggestion: ('name-only-group', ...)
PASSED: 9/10
$ cargo test --offline 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 753 failed 0
$ bash scripts/run-gates.sh --jobs 4 --tier quick gate-out 2>&1 | tail -1
TALLY: 40/40 passed; red: none
```

四次レビュー（Codex）の指摘 2 件。

1 件目は 2 個目の括弧の中身を字面で見ていたこと。`(b:)` も
`(b: Int c: String)` も `name: Type` の列に見えるので Parameters の綴りを添えて
いた。先読みを字面の走査から型の文法の読み取りに置き換え、1 組を最後まで読めて
かつ空でないときだけ群と数える（§8 により引数は 1 個以上なので `()` は群では
ない）。門番に `name-only-with-colon` と `mixed-group` を足した。

```
$ cargo run --offline -q -- check --format json a1/main.hd   # def f(a: Int) (b:)
{"path": "main.hd", ..., "code": "E100", "suggestion": "def f(a: T, b: U): R =>", ...}
$ cargo run --offline -q -- check --format json a2/main.hd   # def f(a: Int) (b: Int c: String)
{"path": "main.hd", ..., "code": "E100", "suggestion": "def f(a: T, b: U): R =>", ...}
# 直した後は 2 つとも suggestion が null。分割した形には添えたまま
$ cargo run --offline -q -- check --format json c1/main.hd   # def rec bump (entries: Entry List) (word: String)
{"path": "main.hd", ..., "code": "E100", "suggestion": "def f(a: T, b: U): R =>", ...}
```

2 件目は腕の本体に括弧で包んで置いた Boolean branch。`| p => (if c => a | b)` にも
腕の条件列の綴りを添える（§26.1）。本体の最上位が Boolean branch のときだけ
添え、腕の外の括弧（`def f c d => if c => (if d => 1 | 2) | 3`）には添えない。

```
$ cargo run --offline -q -- check b1/main.hd   # def f x => if x | 0 => (if true => 1 | 2) | _ => 0
main.hd:3:13: error[E101]: branches cannot be nested inside another branch
# 直した後
main.hd:3:13: error[E101]: branches cannot be nested inside another branch; suggestion: | p => if | c => a | b
$ cargo run --offline -q -- check b2/main.hd   # 腕の外の括弧
main.hd:1:23: error[E101]: branches cannot be nested inside another branch
```

検体を足した時点で門番は赤（`other E100 shapes` と、`paren-in-arm` の既定の 1 行・
`check`/`build`/`run`/`test` の JSON の 5 件、`PASSED: 4/10`）。直した後は緑。

```
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 10/10
$ cargo fmt --all --check && cargo clippy --offline --all-targets -- -D warnings
（出力なし）
$ cargo test --offline 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 753 failed 0
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/run-gates.sh --jobs 4 --tier quick gate-out 2>&1 | tail -1
TALLY: 40/40 passed; red: none
```

五次レビュー（Codex）の指摘 3 件。3 件とも再現した。

1 件目は 1 組目が空の分けた形。指摘の `def f() (b: Int) => b` の `()` は Void の
字句なので、先読みの経路には入らず `function requires a parameter` で止まる。
群として読まれる空の組は空白を入れた `( )` で、こちらは `expected =>, found (`
に Parameters の綴りが付いていた。1 組目にも引数があることを条件に足した。

2 件目は途中で終わる Boolean branch。`| 0 => if true => 1 |` は Boolean branch
として成っていない（閉じの式が無い）のに、区切りの `|` を見ただけで腕の条件列の
綴りを添えていた。字面の走査をやめ、分岐として先読みして `BoolBranch` になると
きだけ添える。腕の本体が不完全な `if` の後ろに宣言が続く形
（`incomplete-before-declaration`）と、腕の外の括弧には添えないままである。

3 件目は名前空間ディレクトリの誤り（§51）の `--format json`。`LoadError::Input`
は `_` の腕に落ちて人間向けの 1 行になっていた。位置を持たない診断に組み、
`check`・`build`・`run`・`test` のどの経路でも §58.4 の 10 フィールドを出す。

```
$ cargo run --offline -q -- check --format json e1/main.hd   # def f( ) (b: Int) => b
{"path": "main.hd", ..., "code": "E100", "message": "expected =>, found (", "suggestion": "def f(a: T, b: U): R =>", ...}
$ cargo run --offline -q -- check --format json e2/main.hd   # def f x =>\n  if x\n    | 0 => if true => 1 |
{"path": "main.hd", ..., "code": "E101", "suggestion": "| p => if | c => a | b", ...}
$ cargo run --offline -q -- check --format json root/main.hd   # root/Bad/ がある根
invalid namespace directory: Bad
```

直した後は 1 件目・2 件目とも `suggestion` が null、3 件目は同じメッセージの
JSON になる。指摘の 1 件目は `def f() (b: Int)` の形で、これは直す前も後も
`function requires a parameter`（`suggestion` は null）である。

```
$ bash scripts/verify-canonical-in-syntax-errors.sh 2>&1 | tail -1
PASSED: 10/10
$ cargo fmt --all --check && cargo clippy --offline --all-targets -- -D warnings
（出力なし）
$ cargo test --offline 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 753 failed 0
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ bash scripts/run-gates.sh --jobs 4 --tier quick gate-out 2>&1 | tail -1
TALLY: 40/40 passed; red: none
```

検体を足した時点の門番は赤（`other E100 shapes`・`other E101 shapes`・
`a load error is JSON for every command that takes --format` の 3 件、
`PASSED: 7/10`）。直した後は緑。

六次レビュー（Codex）の指摘 1 件。再現したが、spec の読み方の判断なので現行の
ままとした。切り出し先は `docs/issues/e101-suggestion-scope.md`。

指摘は、腕の本体の `if` の else が続く腕の綴りを食う形
（`if x | A => if true => 1 | B => 2`）にも腕の条件列の綴りが付く、というもの。

```
$ cat e/main.hd
def f x =>
  if x
    | A => if true => 1
    | B => 2
$ cargo run --offline -q -- check --format json e/main.hd
{"path": "main.hd", "line": 3, "col": 12, "end_line": 3, "end_col": 12, "code": "E101", "message": "branches cannot be nested inside another branch", "canonical": null, "suggestion": "| p => if | c => a | b", "related": null}
```

§24 は `|` の右を必ず else の式と定めるので、腕の本体は `if true => 1 | B`
（else が `B`）の Boolean branch として成り、直後の `=>` が外側の腕の綴りとして
余る。§26.1 も、fallback を持たない腕の条件列が続く `|` の腕を自分の条件として
読むと明記し、この形の曖昧さは貪欲に読む。先読みの終端が `=>` のときは添えない
形にすると 1 か所（`boolean_branch_ahead`）で変わることも確かめたが、ill-formed
な入力でだけ差が出る（valid な入力では両案一致）ので、spec の明文に従い現行の
ままとした。人に確認して切り出しを選んでいる（2026-09-26）。切り出した issue は
2026-09-27 に決定 A（現行のまま）として片付けた（下の「e101-suggestion-scope」）。

着地後（main を checkout した側、2026-09-26）。PR #44 は `fca01a1` で入った。

```
$ git log --oneline -1
fca01a1 Merge pull request #44 from jozobeer/canonical-in-syntax-errors
$ cargo test --offline 2>&1 | grep '^test result' | awk '{p+=$4; f+=$6} END {print "passed",p,"failed",f}'
passed 756 failed 0
$ cargo fmt --all --check && cargo clippy --offline --all-targets -- -D warnings
（出力なし）
$ bash scripts/run-gates.sh --jobs 4 --tier full gate-out 2>&1 | tail -1
TALLY: 3736/3736 passed; red: none
$ bash scripts/corpus-lock.sh --check; echo $?
0
```

e101-suggestion-scope（決定 A、2026-09-27）。決定は現行のままなので `parser.rs` は
変えず、門番に「添える側」の検体（`arm-if-else-next-arm`）を足した。issue の再現
コードは旧構文なので、後置の綴りで測り直した。

```
$ cat no-else/main.hd
def f x =>
  if x
    | A => if true => 1 |
$ cargo run --offline -q -- check --format json no-else/main.hd
{"path": "main.hd", "line": 3, "col": 12, "end_line": 3, "end_col": 12, "code": "E101", "message": "branches cannot be nested inside another branch", "canonical": null, "suggestion": null, "related": null}
$ cat else-next-arm/main.hd
def f x =>
  if x
    | A => if true => 1
    | B => 2
$ cargo run --offline -q -- check --format json else-next-arm/main.hd
{"path": "main.hd", "line": 3, "col": 12, "end_line": 3, "end_col": 12, "code": "E101", "message": "branches cannot be nested inside another branch", "canonical": null, "suggestion": "| p => if | c => a | b", "related": null}
$ bash scripts/verify-canonical-in-syntax-errors.sh | tail -1
PASSED: 10/10
```

続く `|` の右が式として読めない形は、決定 A でも綴りを添えない。`_` は式ではない
ので（`def f = if true => 1 | _` は E100）、`if true => 1 | _` が `BoolBranch` に
ならない。

```
$ cat underscore/main.hd
def f x =>
  if x
    | A => if true => 1
    | _ => 2
$ cargo run --offline -q -- check --format json underscore/main.hd
{"path": "main.hd", "line": 3, "col": 12, "end_line": 3, "end_col": 12, "code": "E101", "message": "branches cannot be nested inside another branch", "canonical": null, "suggestion": null, "related": null}
```

欠陥の注入（`parser.rs` は注入のたびに戻す）。

| 注入 | 門番 |
|---|---|
| 先読みの終端が `=>` なら偽にする（退ける案 B） | `PASSED: 5/10`（`arm-if-else-next-arm` が既定の 1 行と check/build/run/test の JSON で NG） |
| `boolean_branch_ahead` を常に真にする | `PASSED: 9/10`（other E101 shapes が NG。`incomplete-boolean-branch` を先頭に置くとこの検体が `suggestion: "| p => if \| c => a \| b"` を返して NG） |
