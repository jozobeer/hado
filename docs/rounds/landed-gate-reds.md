# 着地済みの門番に残った赤を消す（landed-gate-reds）

## 手順書

### 背景

main（`4baca98`）で 5 本の門番が赤い（`verify-r62` 1/4・`verify-r68` 1/4・
`verify-r63` 2/4・`verify-r46` 4/5・`verify-r47` 3/4）。このラウンドが直すのは
r62 と r68 で、r63 の 63-c も同じ根から直る。**r46・r47 と r63 の 63-d は F5 の
印字し直しで期待値が古くなった別の原因**で、r46 と r47 は
`check-fix-reprint-expectations`（PR #85、着地済み）が、63-d は PR #81 が担当する。
このラウンドでは直さない。

- `scripts/verify-r62.sh`（1/4）: 62-a と 62-b が `E503` を期待する探針に
  `main.hd:254:1: error[E402]` が先に立つ。62-d は `check --fix` が残す
  `def main = 1` ではなく `def main =\n  1` を期待している。
- `scripts/verify-r68.sh`（1/4）: 68-a と 68-b が `check --fix` / `fmt` の
  残したソースを `E402` で拒む。
- `scripts/verify-string-split-linear.sh`（10/11）: 検査 i（join）が
  `small median 0.182s is below the 0.200s floor` で赤。

前のラウンド `map-internal-types-leak-into-root` は E205 の半分（`implement`
の内部型が根の名前空間へ漏れる）を直し、残る赤を「許容される赤」に挙げた。
issue `implement-local-adt-identity-is-name-only` が、残った型同一性の欠陥を
記録している。F5（`350a575`）は `check --fix` に `fmt` と同じ印字し直しを
足した（§57）ので、F5 より前の期待値を持つ門番は印字し直しの後の正準形を
期待していない。

### 決定

- r62/r68 の赤は実装の欠陥である。根の `type T = | A | B` と prelude の
  `implement Map as Map` の中の `type (K, V) T = | MapLeaf | MapNode(...)` は、
  どちらも `Ty::Adt("T")` になる。網羅性の family を `Ty::Adt` の名前でまとめると、
  map の `def red` / `def left` の腕が `MapLeaf` / `MapNode` を覆っていないと
  判定され、`E402` が E503 / E512 より先に立つ。期待値を書き換えず、
  `crates/hado/src/infer.rs` を直す。
- family は constructor の宣言元の型でまとめる。`implement` の本文では §49 の
  局所名前空間の綴り（`Map.T`）を使う。`Ty::Adt` の名前は変えない。単一化と
  抽象型の開示に触れないためである。
- 名前で引く group（名前付き constructor の field と projection の schema）は
  `Ty::Adt` の名前のまま残す。family と schema の引き先を同じ関数にすると、
  `implement` の中の名前付き record の field が引けなくなる
  （`tests/accept/functor/main.hd` が `E305` になる。実測を参照）。
- 62-b は family を直しただけでは緑にならない。
  `def f (t: T): Int => if t | A => 1 | _ => // why\n  0` の `// why` が、
  印字し直しで定義の外（`def main` の直前）へ出る。§58.1 は「本体の中に
  コメントを持つ定義は、本体を次の行へ送る。これによりコメントは同じ定義の
  中に残る」と定めるので、`crates/hado/src/postfix_print.rs` の
  `body_break_line` を直す。見出しと同じ行の `=>` の後ろのコメントを本体の
  中として数え、本体の終わりの後ろ（同じ行の行末）のコメントは外として数える。
- 62-a・62-b・62-d と 63-c の期待値は F5 より前のもので、印字し直しの後の
  正準形ではない。§58.1 から導いて写す（`f339df3` が r52・r57・r64 にしたのと
  同じ扱い）。とくに `g A` → `g (A)` のずれは F5 由来である。`bf1310c` の
  `format` は行末の空白と改行だけを整える `layout` を通していたので `g A` を
  残し、印字し直しを `fmt` に入れた `350a575`（F5）から `g (A)` になった。
- string-split-linear の床は門番の設計の問題である。計測の分解能を守るという
  意図は、目標時間の絶対値ではなく引いた差に対して測るべきもので、0.182s は
  分解能に埋もれる大きさではない。門番は緩めず、実測と判断を報告する
  （`docs/issues/flaky-string-split-timing-gate.md` が扱う）。

### 期待値の根拠

- §49: 「`implement` 節の中の ADT と record とその constructor は、その実装
  だけの局所的な空間にあり」。根の同名型と別の constructor の集合を持つ。
- §28 と E402（`docs/spec.md` のエラー表）: 網羅性はそのパターンの型の
  constructor を覆うことを見る。
- §57: 「1 つでも書き換えを当てたら、そのあと `fmt` と同じ印字し直し
  （§58.1）まで行う」。`check --fix` は書き換えを当てたソースを正準形で残す。
- §58.1: 「改行は式の形だけで決める。腕を持つ分岐、束縛、`fn … /fn`、
  `module`・`implement`・`effect`・`handler` が改行を持つ」。裸のリテラル `1`
  の本体は改行を持たないので `def main = 1` が正準形である。
- §58.1: 「本体の中にコメントを持つ定義は、本体を次の行へ送る。これにより
  コメントは同じ定義の中に残る」。
- 並置の引数の constructor を括弧で囲む綴り（`f (A)`）は、印字器
  `postfix_print` の作る正準形である。受理コーパスの `name (Guest)` と同じで、
  `verify-fmt-layout.sh` の検査 b が 188 ファイルの往復で固定する。§58.1 の
  「印字し直した形が正準形である」が根拠である。

### 成果物

- `crates/hado/src/infer.rs` — `constructor_owners`、`data_type_owner`、
  `group_constructors` / `constructor_groups` / `constructor_families`。
- `crates/hado/src/postfix_print.rs` — `body_break_line` と `after`。
- `crates/hado/tests/map_internal_types.rs` — 2 本追加。
- `crates/hado/tests/fmt_layout.rs` — 1 本追加。
- `scripts/verify-r62.sh` — 62-a・62-b・62-d の期待値を §58.1 の正準形へ。
- `scripts/verify-r63.sh` — 63-c の期待値を §58.1 の正準形へ。
- `docs/issues/implement-local-adt-identity-is-name-only.md` — 削除（型同一性の
  欠陥はこのラウンドで直り、回帰テストが固定する）。**issue の完了条件には
  「`verify-r63.sh` が赤なし」もあったが、残る 63-d は印字器の別の欠陥で
  PR #81 が担当するため、形式上は満たしていない。** 63-d は base でも同じ
  assert（`id if(b => 1 | 2)` の綴り）で落ちており、型同一性とは無関係である。
- `docs/issues/flaky-string-split-timing-gate.md` — 検査 i の床の原因と案を
  実測つきで追記。
- `docs/rounds/landed-gate-reds.md`。

### 工程

1. main で 3 本を再現し、`E402` の位置（`main.hd:253/254`。prelude の
   `stdlib/map.hd:21/22` に対応する）を確定する。
2. `type T` を `type U` に替えた対照で、E503 / E512 の書き換えと印字し直しが
   正しく動くことを確かめ、欠陥が型同一性にあると切り分ける。
3. `map_internal_types.rs` に赤いテストを足す。
4. `infer.rs` を直して緑にする。
5. `fmt_layout.rs` に赤いテストを足し、`postfix_print.rs` を直して緑にする。
6. r62・r63 の期待値を §58.1 から導いて写す。
7. string-split-linear の床を測り、判断を書く。
8. `cargo test --offline` と quick 層を回す。

### 判定

- `bash scripts/verify-r62.sh` が `PASSED: 4/4`。
- `bash scripts/verify-r68.sh` が `PASSED: 4/4`。
- `bash scripts/verify-r60.sh` が `PASSED: 8/8`、
  `bash scripts/verify-map-by-builtin-compare.sh` が `PASSED: 9/9`、
  `bash scripts/verify-fmt-layout.sh` が `PASSED: 6/6`。
- `cargo test --offline` が 0 failed。
- `bash scripts/run-gates.sh --tier quick target/.gates-lgr` が赤なし。
- `bash scripts/verify-string-split-linear.sh` は検査 e〜h と j〜k が緑。
  検査 i は床の判定だけが環境依存で赤になりうる。

### 許容される赤

- `scripts/verify-r63.sh` の 63-d。引数の位置の `if` に括弧を付ける綴り
  （`id (if b => 1 | 2)`）が別の欠陥で、PR #81 が扱う。r63 は 3/4 である。
- `scripts/verify-string-split-linear.sh` の検査 i。床（0.200s）を割ったときの
  赤で、既知の計時揺れ（`docs/issues/flaky-string-split-timing-gate.md`）。

### スコープ外

- PR #81 の印字器の綴り（引数の位置の括弧。63-d）。
- `verify-string-split-linear.sh` の床の作り直し。

### 中断の条件

- 期待値を §58.1 から導けないとき。
- 実装の変更が E205 か単一化の意味を変えるとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §49 | `implement` の ADT と constructor はその実装だけの局所空間 | 型 | `infer.rs` | `map_internal_types.rs`、`verify-r62.sh` |
| §28 | 網羅性はその型の constructor を覆うことを見る | 型 | `infer.rs` | `verify-r62.sh`、`verify-map-by-builtin-compare.sh` |
| §57 | `check --fix` は書き換えのあと `fmt` と同じ印字し直しをする | 実行 | `fixes.rs`・`postfix_print.rs` | `verify-r62.sh`、`verify-r63.sh` |
| §58.1 | 本体の中のコメントは同じ定義の中に残す | 実行 | `postfix_print.rs` | `fmt_layout.rs`、`verify-fmt-layout.sh` |
| §58.1 | 改行は式の形だけで決める | 実行 | `postfix_print.rs` | `verify-r62.sh` |

## 実測

`main`（`4baca98`）での赤の再現。

```
$ bash scripts/verify-r62.sh
AssertionError: ('a/hint', 'E503', 'main.hd:254:1: error[E402]: not all constructor values are covered\n')
NG: 62-a the catch-all underscore becomes the fallback
AssertionError: ('b/hint', 'E503', 'main.hd:255:1: error[E402]: not all constructor values are covered\n')
NG: 62-b a comment after the arrow survives
OK: 62-c the exclusions of §24 are accepted
AssertionError: ('d/e501', "'def main = 1\\n'", "'def main =\\n  1\\n'")
NG: 62-d the earlier rewrites still apply
PASSED: 1/4

$ bash scripts/verify-r68.sh
NG: 68-a check --fix makes E512 accepted (main.hd:254:1: error[E402]: not all constructor values are covered
NG: 68-b fmt does the same and is idempotent (the formatted file is not accepted)
NG: 68-c the value is unchanged ()
OK: 68-d the four shapes are left alone
PASSED: 1/4
```

62-d の赤は期待値だけの問題である。`check --fix` は base でも `def main = 1`
を残す（F5 の印字し直しが効いている）。門番の期待値 `def main =\n  1` が F5 前の
ままである。

`E402` の位置は、利用者のソースが 3 行のときの prelude の行に写る。254 行は
`stdlib/map.hd` の 22 行（`def left`）である。

```
$ bash scripts/verify-string-split-linear.sh
AssertionError: join: small median 0.182s is below the 0.200s floor
NG: i perf: join is linear in the element count
PASSED: 10/11
```

この赤は記録にある値である（`docs/rounds/exit-code-from-main.md` に 0.182s、
`docs/rounds/postfix-binding.md` に 0.179s〜0.198s）。このセッションの全走では
再現せず、検査 i は緑だった。実装も門番の入力もこのラウンドで変えていない。

原因の切り分け。`type T` を `type U` に替えると E503 の書き換えと印字し直しが
最後まで動く。欠陥は書き換えの表ではなく型同一性にある。

```
$ cat target/lgr-probe/control/main.hd
type U = | A | B
def f (t: U): Int => if t | A => 1 | _ => 0
def main = f A
$ target/debug/hado check target/lgr-probe/control/main.hd
main.hd:2:38: error[E503]: use a fallback instead of a catch-all underscore pattern; canonical form: | 0
$ target/debug/hado check --fix target/lgr-probe/control/main.hd
fmt: main.hd (1 rewrites)
$ cat target/lgr-probe/control/main.hd
type U = | A | B
def f (t: U): Int => if t | A => 1 | 0
def main = f (A)
$ target/debug/hado check target/lgr-probe/control/main.hd
（診断なし、終了コード 0）
```

`implement` の本文の名前付き record の field は、family の綴りではなく
`Ty::Adt` の名前で引く。family の綴りを schema の引き先へ流用すると E305 に
なる（`constructor_groups` を一時的に owner で引く形にして測った）。

```
$ cat target/lgr-probe/f/main.hd
module Store
type T
def create (name: String): T
def describe (user: T): String
/module
implement Store as StoreImpl
type T = | Payload { name: String }
def create name => Payload { name = name }
def describe user => user.name
/implement
def main = StoreImpl.describe (StoreImpl.create "x")
$ target/debug/hado check target/lgr-probe/f/main.hd   # constructor_groups を owner で引く版
main.hd:9:22: error[E305]: field access requires a single record constructor
$ target/debug/hado check target/lgr-probe/f/main.hd   # 直した版
（診断なし、終了コード 0）
```

62-b のコメントの位置。`infer.rs` だけを直した時点では、62-b の `// why` が
定義の外（`def main` の直前）へ出る。

```
AssertionError: ('b/comment',
  "'type T = | A | B\\ndef f (t: T): Int => if t | A => 1 | 0\\n  // why\\ndef main = f (A)\\n'",
  "'type T = | A | B\\ndef f (t: T): Int =>\\n  // why\\n  if t | A => 1 | 0\\ndef main = f (A)\\n'")
```

`postfix_print.rs` を直すと §58.1 の「本体を次の行へ送る」形になる。

```
$ printf 'def f x => // 腕の中のコメント\n  x + 1\ndef main = f 1\n' > main.hd
$ target/debug/hado fmt main.hd && cat main.hd
fmt: main.hd (1 rewrites)
def f x =>
  // 腕の中のコメント
  x + 1
def main = f 1
$ target/debug/hado check main.hd
（診断なし、終了コード 0）
```

書き換えた期待値は 4 か所である。どれも §58.1 の「印字し直した形が正準形」
から出る。

- 62-a: `def main = f A` → `def main = f (A)`（並置の引数の constructor を
  括弧で囲む）。
- 62-b: `... | // why\n  0` → `... =>\n  // why\n  if ... | 0`（本体の中の
  コメントは本体を次の行へ送る）。
- 62-d: `def main =\n  1` → `def main = 1`（裸のリテラルの本体は改行を
  持たない）。base でも `check --fix` は `def main = 1` を残す。
- 63-c: `f true + g A` → `f true + g (A)`（62-a と同じ）。

`g A` → `g (A)` は F5 由来である。`bf1310c` の `format` は行末の空白と改行
だけを整える `layout` を通していたので `g A` を残し、印字し直しを `fmt` に
入れた F5 から `g (A)` になった。

```
$ git show bf1310c:crates/hado/src/fixes.rs | sed -n '146,152p'
    // §58.1: the bulk form lays the text out as well. `check --fix` applies the
    // diagnostic-anchored rewrites only, so it never comes through here.
    let (laid_out, layout_applied) = layout(&text);
$ git show bf1310c:scripts/verify-r63.sh | sed -n '135p'
FIXED = '...def main = f true + g A\n'
$ git log --oneline -1 350a575
350a575 Merge pull request #76 from jozobeer/fmt-layout
```

直した後の門番。

```
$ bash scripts/verify-r62.sh
PASSED: 4/4
$ bash scripts/verify-r68.sh
PASSED: 4/4
$ bash scripts/verify-r60.sh
PASSED: 8/8
$ bash scripts/verify-map-by-builtin-compare.sh
PASSED: 9/9
$ bash scripts/verify-fmt-layout.sh
PASSED: 6/6
$ bash scripts/verify-r63.sh
OK: 63-a two Boolean arms become a Boolean branch
OK: 63-b an arm and a fallback become a Boolean branch
OK: 63-c E503 and E520 settle together
NG: 63-d the exclusions and the argument position
PASSED: 3/4
```

join の床。門番を単独で走らせると、検査 i の小さい方は床を超える。引いた差
（比の分母）は 0.29s で、CPU 時間の分解能（マイクロ秒）に埋もれていない。

```
$ python3 target/lgr-probe/probe_join.py
small trial0: control=0.0299 target=0.3245 diff=0.2948 ratio_vs_floor=above
small trial1: control=0.0301 target=0.3239 diff=0.2940 ratio_vs_floor=above
small trial2: control=0.0302 target=0.3164 diff=0.2862 ratio_vs_floor=above
large trial0: control=0.0569 target=0.6403 diff=0.5843 ratio_vs_floor=above
large trial1: control=0.0552 target=0.6322 diff=0.5753 ratio_vs_floor=above
large trial2: control=0.0562 target=0.6526 diff=0.5971 ratio_vs_floor=above
```

床は目標時間の絶対値（`small_total >= 0.200`）に置かれている。機械が速いと
正しい実装でも下回る。0.182s のときの引いた差は 0.15s 程度で、分解能の話では
ない。したがって「速すぎて測れない」を赤にするのはこの大きさでは正しくない。
床の設計（絶対値ではなく引いた差か、その対照に対する比で測る）が誤りである。
門番は緩めず、issue に委ねる。

`cargo test --offline` と quick 層。全走の直前に `cargo fmt --all --check` と
`cargo clippy --offline --all-targets -- -D warnings` を通した。

```
$ cargo test --offline
（74 スイートすべて `test result: ok`、`0 failed`。終了コード 0）

$ bash scripts/run-gates.sh --tier quick target/.gates-lgr
TALLY: 41/41 passed; red: none
```

`bash scripts/verify-string-split-linear.sh` の全走。この走行では検査 i の
小さい方が 0.308s で床を超え、11/11 だった（床を割る赤は環境依存である）。

```
$ bash scripts/verify-string-split-linear.sh
split ascii lines: small=1.004s large=2.019s subtracted 0.434->0.849 ratio 1.96
split multibyte lines: small=0.724s large=1.536s subtracted 0.232->0.492 ratio 2.12
join: small=0.308s large=0.600s subtracted 0.282->0.549 ratio 1.95
OK: i perf: join is linear in the element count
PASSED: 11/11
```

修正の範囲の門番は前節までにすべて緑である（r62 4/4、r68 4/4、r60 8/8、
map 9/9、fmt-layout 6/6）。r63 だけが 63-d の別原因で 3/4 である。

### 着地後の全走（2026-10-13、main `b6f7126`）

main を checkout した側（`.worktrees/postlanding-main`）で回した。full 層（quick・
core・full の累積に文書層を足した 84 本）は `TALLY: 3894/3894 passed; red: none` で
赤は 1 本も無い。`cargo test --offline` は full 層の門番 `verify-m0` の検査 2 が
`cargo-test passed=850` で緑である（終了コード 0、`FAILED` は 0）。quick 層の門番
6 本は full 層の走行に含まれ、すべて緑である。

```
$ bash scripts/run-gates.sh --tier full target/.gates-full | tail -1
TALLY: 3894/3894 passed; red: none
$ ./target/debug/hado run tools/gates/main.hd -- target/.gates-full
3894/3894 passed; red: none
$ grep cargo-test target/.gates-full/verify-m0.log
OK: 2 cargo-test passed=850
$ for f in verify-ci verify-gate-cache verify-m1-spec verify-repl verify-rustfmt verify-spec-model; do tail -1 target/.gates-full/$f.log; done
PASSED: 8/8
PASSED: 26/26
PASSED: 1/1
PASSED: 10/10
PASSED: 2/2
PASSED: 3/3
$ tail -1 target/.gates-full/verify-r62.log
PASSED: 4/4
$ tail -1 target/.gates-full/verify-r68.log
PASSED: 4/4
$ tail -1 target/.gates-full/verify-map-by-builtin-compare.log
PASSED: 9/9
$ tail -1 target/.gates-full/verify-r63.log
PASSED: 4/4
```

着地時は `verify-r63` が 63-d の別原因で 3/4 だった。PR #81 が 63-d を直し、4/4 になった。
