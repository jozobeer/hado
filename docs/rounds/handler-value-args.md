# handler の値引数を腕から参照できるようにする（handler-value-args）

## 手順書

### 背景

§38 は handler が値の引数を取れるとし、`handler A Yield as Take(limit: Int)` を例に挙げる。腕の中でその引数を参照すると E204 で止まる（`docs/issues/handler-value-args.md`、2026-09-26）。

```
$ cat target/issue/value-arg/main.hd
// needs: prelude
handler Error as OrValue(fallback: Int)
abort raise _ => fallback
def main = with OrValue(0) -> Int.parse "x"
$ cargo run --offline -q -- check target/issue/value-arg/main.hd
main.hd:3:18: error[E204]: unbound name fallback
```

issue の「原因」は名前解決だけを名指しし、型検査と単相化が値引数を腕へ渡すかは未確認としていた。確かめた結果、値引数を腕へ渡していない段は 4 つあった。issue の見立ては 1 つ足りない。

- 名前解決: `name_resolution.rs` の `handler` は腕ごとに操作の引数と `return` の引数だけをスコープに入れ、`HandlerDecl::params` を入れない。
- ファイルの隔離: `file_scopes.rs` の `handler` も `HandlerDecl::params` を束縛として数えない。この段が腕の中の参照を `file0$fallback` へ付け替えるので、名前解決を直しても参照は解決しない。issue にも計画にも無い段である。
- 型検査: `infer.rs` の `handler_answer` → `infer_handler_arms` は `self.values` を複製して腕を推論するだけで、`HandlerDecl::params` を束縛しない。`with` の実引数の型を宣言型と照合もしない。
- 単相化: `mono.rs` の `with_handler` は `for arg in args { let _ = self.expression(arg)?; }` と実引数を lower して捨てる。腕を lower する `interpret_outcome` は `handler.params` を見ない。

### 決定

issue の「案」の推奨をそのまま実装する。

1. 名前解決・ファイルの隔離・型検査・単相化の 4 段で、handler の値引数を全ての腕のスコープに入れる。
2. `with` の実引数の評価は `with` の位置で 1 回とする。腕ごとにも単相化の再入時にも評価し直さない。
3. 実引数の型は、`with` の位置で handler の宣言型と単一化する。個数が合わなければ型段階の誤りとする。
4. 退ける案（§38 から値引数を消す）は採らない。

### code-complete の一周目

- 意図: hado で効果の handler を書く人が、状態を持つ handler の初期値や既定値（`with OrValue(0) -> …`、`handler A Yield as Take(limit: Int)`）を腕から参照できるようにする。§38 は値引数を認めており、`do-option-absent`（M7）が前提にする既定値付き handler が今は書けない。
- 仮説: 3 段とも値引数を腕のスコープに入れれば参照できる。単相化では、`with` の位置で評価した実引数を、腕を lower する `convert` 関数の追加の引数として渡し、腕の lower 中だけ `scope` の名前をその引数 id に向ければ、腕は `with` の位置の変数を参照できる。`with` の位置で `Let` を 1 重かぶせれば評価は 1 回になる。
- 判定器: 下の「判定」。issue の完了条件をそのまま使う。
- スコープ: `crates/hado/src/name_resolution.rs`・`crates/hado/src/file_scopes.rs`・`crates/hado/src/infer.rs`・`crates/hado/src/mono.rs`・`crates/hado/tests/r26_effects.rs`・`tests/accept/handler_value_arg*`・本ファイル。`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` は触らない。
- 未確認: `with` の実引数が効果を持つとき、`Let` を 1 重かぶせて 1 回にできるか。入れ子の `with` で内側と外側の値引数が混ざらないか（M5 の `WithTypes` と同じ族の罠）。`again` の腕で継続が `convert` を呼び直すとき、実引数を継続の capture に載せる必要があるか。実引数の個数・型が合わないときの報告段階とコード。issue の名指す 3 段の外に段が残っていないか。

### 期待値の根拠

issue の完了条件のとおり。

- `handler_value_arg` は issue の `value-arg` をその後置の綴りのまま置く。`with OrValue(0) -> Int.parse "x"` は `Error` を `OrValue` が捕まえ、`fallback = 0` を返す。
- `handler_value_arg_take` は §38 の `Take(limit: Int)` の形。複数の操作の腕が同じ値引数を参照する。
- `handler_value_arg_nested` は入れ子の `with`。内側の値引数と外側の値引数が入れ替わらないことを測る。`abort ask => limit * 10` と `return value => value + limit` を `with Take(2) -> (with Take(5) -> ask)` に当て、正しい対応なら `50 + 2 = 52`、入れ替わっていれば `20 + 5 = 25` になる。
- `handler_value_arg_return` は `return` の腕から値引数を参照する。
- `handler_value_arg_effect` は実引数が効果を持つ形。実引数に `Io.log` を置き、`again` の腕が `resume` を繰り返して `convert` が複数回呼ばれる形で、`log` の出力が 1 回であることを `// expect: arg` で見る。
- `handler_value_arg_once` は `once` の腕。値引数を参照する `once` の腕を関数の中から呼び、その関数をさらに関数から呼ぶ（capture が 2 段伝わること）。`return` の腕も値引数を使う（`6 + 37 + 6 = 49`）。
- `handler_value_arg_shadow` は `with` の値引数と呼んだ関数の引数が同じ名前の形。`use 100` の引数が腕の `n` と同名でも、腕の `n` は値引数（6）、本体の `n` は利用者の束縛（100）で、`6 + 100 = 106` になる。
- `with` の実引数の 1 回評価は `r26_effects.rs` のテストでも測る。同じ形の stdout を完全一致で照合し、`List.length` の値まで見る。

### 成果物

- `crates/hado/src/name_resolution.rs`: `handler` が `HandlerDecl::params` を腕のスコープに入れる。
- `crates/hado/src/file_scopes.rs`: `handler` が `HandlerDecl::params` を束縛として数え、腕の中の参照を file-local の名前へ付け替えない。
- `crates/hado/src/infer.rs`: `with` の実引数と宣言型の照合、腕の推論での値引数の束縛。
- `crates/hado/src/mono.rs`: `with` の位置での 1 回評価と `Let`、値引数の表 `frame_value_args`、腕を lower する間の名前の差し替え、単相化した `def`・lambda・継続への capture の引き継ぎ、`convert` の追加引数。
- `crates/hado/tests/r26_effects.rs`: 1 回評価の判定器。
- `tests/accept/handler_value_arg*`: 完了条件 (1)(2) の判定器（7 本）。
- 本ファイル。

### 工程

main で再現（後置の綴り）→ 赤のテスト（受理コーパス 5 本と 1 回評価のテスト）→ 名前解決 → ファイルの隔離 → 型検査 → 単相化 → 周辺の形（`return` の腕・効果を持つ実引数・入れ子・個数と型の食い違い）の実測 → 着地の直前に `git merge main` と corpus lock の作り直し。

### 判定

- `handler_value_arg` が `0` を印字して終了コード 0。
- §38 の `Take(limit: Int)` の形を受理コーパスが固定する（複数の腕・入れ子の `with`・`return` の腕・効果を持つ実引数）。
- `with` の実引数が 1 回だけ評価される。
- `cargo test --offline` が 0 failed。
- `bash scripts/run-gates.sh --tier quick`・`--tier core`・`bash scripts/lint-docs.sh`・`bash scripts/verify-spec-contracts.sh` が赤なし。

### 許容される赤

無し。

### スコープ外

- `do-option-absent`（M7）本体と、計画の決定 D2。
- handler の型引数（`handler A Yield as Take(limit: A)`）を値引数の型に使う形。`type_params` を型検査の環境に束縛する仕組みは今の checker に無く、本ラウンドの完了条件ではない。
- `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` への追記。
- `cargo test --offline` と層別（quick / core）はラウンドの最後に 1 回だけ回す。

### 中断の条件

- 判定が 2 回続けて赤になったら止める。
- 仕様の解釈に疑義が出たら仕様を変えず、依存しない作業だけ進める。

### 仕様への疑義

- 主張: `with` の実引数の個数・型が handler の宣言と合わないとき、どのコードで報告するか。
- 再現: `printf 'handler Error as OrValue(fallback: Int)\nabort raise _ => fallback\ndef main = with OrValue("x") -> Int.parse "y"\n' > main.hd; cargo run --offline -q -- check main.hd`
- 現行: 直す前は実引数を型検査しない（捨てる）ので何も報告しない。§56 に `with` の引数のための行は無い。
- 変えると: 個数は E301（arity-mismatch、アンカーは適用の先頭）、型は E305（type-mismatch、アンカーは食い違った式の先頭）で報告する。§38 は「引数があれば呼び出しの形で渡す」と書くので、呼び出しと同じ段階・コードに寄せた。§56 の表に行は足さない。
- 推奨: E301 / E305 を使い、§56 の表は変えない。審査で見てほしい点に挙げる。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §38 | handler は値の引数を取れ、`with` は呼び出しの形で渡す | 名前解決 | — | `tests/accept/handler_value_arg*` |
| §38 | 値引数は handler の全ての腕から見える | 名前解決 | — | `crates/hado/src/file_scopes.rs` の束縛 |
| §38 | 値引数は handler の全ての腕から見える | 型 | E301・E305 | `tests/accept/handler_value_arg*` |
| §38 | 値引数は handler の全ての腕から見える（`once` の腕も、呼んだ関数の中でも） | 単相化 | — | `tests/accept/handler_value_arg_once` |
| §38 | 値引数の名前は腕からだけ見え、本体の利用者の束縛を覆わない | 単相化 | — | `tests/accept/handler_value_arg_shadow` |
| §38 | `with` は `rest` を `H` の下で評価する | 単相化 | — | `crates/hado/tests/r26_effects.rs` の 1 回評価 |

## 実測

### 再現（main、直す前）

issue の入力を後置の綴りのまま `target/issue/value-arg/main.hd` に置いて走らせる。

```
$ cargo run --offline -q -- check target/issue/value-arg/main.hd; echo "check exit=$?"
main.hd:3:18: error[E204]: unbound name fallback
check exit=1
```

### 赤（直す前の新しいテスト）

```
$ for c in handler_value_arg handler_value_arg_take handler_value_arg_nested handler_value_arg_return; do printf '%s: ' $c; cargo run --offline -q -- check tests/accept/$c/main.hd 2>&1 | head -1; done
handler_value_arg: main.hd:5:18: error[E204]: unbound name fallback
handler_value_arg_take: main.hd:7:14: error[E204]: unbound name limit
handler_value_arg_nested: main.hd:6:14: error[E204]: unbound name limit
handler_value_arg_return: main.hd:7:25: error[E204]: unbound name limit
$ cargo test --offline --test r26_effects -- handler_value_argument_is_evaluated_once_at_the_with
main.hd:4:44: error[E204]: unbound name limit
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 67 filtered out; finished in 1.05s
```

### 緑（このラウンドの成果物）

```
$ for c in handler_value_arg handler_value_arg_take handler_value_arg_nested handler_value_arg_return handler_value_arg_effect handler_value_arg_once handler_value_arg_shadow; do printf '%s: ' $c; out=$(cargo run --offline -q -- run tests/accept/$c/main.hd 2>&1); printf 'exit=%s out=%s\n' "$?" "$(echo "$out" | tr '\n' '|')"; done
handler_value_arg: exit=0 out=0|
handler_value_arg_take: exit=0 out=30|
handler_value_arg_nested: exit=0 out=52|
handler_value_arg_return: exit=0 out=3|
handler_value_arg_effect: exit=0 out=arg|
handler_value_arg_once: exit=0 out=49|
handler_value_arg_shadow: exit=0 out=106|
$ cargo test --offline --test r26_effects -- handler_value_argument_is_evaluated_once_at_the_with
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 67 filtered out; finished in 1.09s
```

### 実装から分かったこと

- 仮説の当たり: `with` の位置で評価した実引数を `convert` の追加引数にし、腕の lower 中だけ `scope` の名前をその引数へ向け直せば、腕は値引数を参照できた。`with_params` が結果に `Let` をかぶせるので、`Io.log` を置いた実引数は 1 回だけ走る（`handler_value_arg_effect` の stdout が `arg` 1 行）。
- 仮説の外れ 1: 直す段は 3 ではなく 4 だった。`file_scopes.rs` が腕の中の参照を `file0$fallback` へ付け替えるので、名前解決と型検査を直しても `unbound name fallback` が残る。`file_scopes.rs` の `handler` にも同じ束縛が要る。issue の「原因」も、観測した E204 の文言 `unbound name` は `infer.rs` のもので、`name_resolution.rs` の `invalid name reference` ではない。
- 仮説の外れ 2: 入れ子の対応は最初の判定器では測れていなかった。`abort ask => limit` と `return value => value + limit` の対称な形は、内側と外側を入れ替えても同じ 7 になる。`abort ask => limit * 10` に変えて 52 と 25 を分けた。
- 仮説の外れ 3: `again` の腕で継続が `convert` を呼び直す経路は、実引数を継続の capture に載せる必要があった（`continuation_calling_convert`）。載せないと継続関数の中で convert の引数 id が解決しない。
- 設計に還元したこと: `with` の実引数の評価は `with_params` に寄せた。`mono.rs` に新しい束縛の仕組みを足さず、腕の引数と同じ経路で 1 回評価と `Let` を手に入れている。`interpret_outcome` は `scope` から値引数を読み、腕の lower の間だけ convert の引数へ向け直す。
- 周辺: 実引数の個数違いは E301（アンカーは handler 名）、型違いは E305（アンカーは食い違った実引数）で止まる。値引数を使わない handler（`HAsk(unused: Int)`）も通る。`file_scopes` を直す前は、腕の参照が file-local 名へ付け替えられていた。

### ラウンド最後の全走

```
$ cargo test --offline
test result: ok. ... 0 failed （70 本の test result 行、777 passed、FAILED 0）
$ bash scripts/run-gates.sh --tier quick target/gates-quick2
TALLY: 41/41 passed; red: none
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
```

`--tier core` の 1 回目は 716/721 で、赤は `verify-m0`・`verify-m2`・`verify-m3`・`verify-m7` の 4 本だった。いずれも (a) 受理コーパスの正準形（`handler_value_arg_effect` などの腕を改行していなかった）と (b) `tests/corpus.sha256` が新しい 5 本をまだ持たないこと、の 2 つに還元できた。両方を直した後、その 4 本だけを回し直して緑を確かめた。

```
$ bash scripts/run-gates.sh target/gates-red verify-m0 verify-m2 verify-m3 verify-m7
TALLY: 29/29 passed; red: none
```

corpus lock の `--check` 差分は、追加した 5 本の `+` 行だけで、main 側の変更は無かった（`origin/main` は基点の `bb44419` のまま）。

### 着地前（main 取り込み後）の全走

審査の指摘への対応の途中で `origin/main` が `59907ec`（exit-code-from-main、§54 の終了コード）へ進んだ。main を取り込み、受理コーパスの `main` が `Int` を返す棚卸し（`Int.to_string` で包む）に本ラウンドの 7 本を追随させた。lock の `--check` 差分は、main 側の 44 本の `M` と本ラウンドの 7 本の `+` だけで説明できた。

```
$ cargo test --offline
test result: ok. ... 0 failed （71 本の test result 行、783 passed、FAILED 0）
$ bash scripts/run-gates.sh --tier quick target/gates-quick-r2
TALLY: 41/41 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates-core-r2
TALLY: 710/710 passed; red: none
$ bash scripts/verify-m4.sh
PASSED: 172/172
$ bash scripts/verify-r37.sh
PASSED: 262/262
$ bash scripts/verify-r43.sh
PASSED: 10/10
$ bash scripts/lint-docs.sh
PASSED: 16/16
$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2
$ bash scripts/corpus-lock.sh --check
lock ok
```

赤は `--tier quick`・`--tier core` のいずれにも無い。

### 審査の指摘への対応

審査が 2 件の fatal を出した。1 件目（値引数の名前が `with` の本体へ漏れる）は親が再現して確定した。2 件目（`once` の腕に値引数が届かない）は自分で再現して直した。

#### 指摘 1: 値引数の名前が本体の利用者の束縛とすり替わる

審査の形は `abort` の腕だった。`abort` は継続を捨てる（`ask + k` の `+ k` は走らない）ので、値そのものは食い違わない。`once` に置き換えた最小形で再現した。

```
$ cat target/v6-samename/main.hd
effect Ask
once ask: Int
handler Ask as Fixed(n: Int)
once ask => n
def use (n: Int): Int {Ask} => ask + n
def main = with Fixed(6) -> use 100
$ cargo run --offline -q -- run target/v6-samename/main.hd   # 直す前
200
```

正しい値は `6 + 100 = 106`。腕の `n` が、呼んだ関数の引数 `n`（100）を拾っていた。main では E204 で止まっていた形なので、黙って違う値を返す退行である。

原因は `with_params` に値引数の名前を `self.scope` へ入れさせ、`with` の本体の lower の間ずっと見せていたこと。腕の本体は呼ばれた関数のフレームで lower されるので、そのフレームの引数と衝突する。

直し方は、名前解決と infer の扱いに揃えること。値は `frame_value_args`（`handler_stack` と同じ並びの表）に持ち、`self.scope` には入れない。腕を lower する間だけ、その handler の値を `scope` に被せる（`install_handler_frame_values`）。本体の式は利用者の束縛をそのまま引く。

```
$ cargo run --offline -q -- run target/v6-samename/main.hd   # 直した後
106
$ cat target/v8-paramshadow/main.hd
effect Ask
once ask(value: Int): Int
handler Ask as Fixed(value: Int)
once ask value => value * 10
def main = with Fixed(6) -> ask 2
$ cargo run --offline -q -- run target/v8-paramshadow/main.hd   # 操作の引数が勝つ
20
```

#### 指摘 2: `once` の腕に値引数が届かない

```
$ cat target/v6-val/main.hd
effect Ask
once ask: Int
handler Ask as Fixed(n: Int)
once ask => n
def use (x: Int): Int {Ask} => ask + x
def main = with Fixed(6) -> use 1
$ cargo run --offline -q -- run target/v6-val/main.hd   # 直す前
function definition requires a function type   (exit=1)
```

`once` の腕は操作の位置で inline に lower される。単相化は `std::mem::take(&mut self.scope)` でフレームを作り直すので、呼ばれた `use` の中に値引数が無い。トップレベルの `def n` があれば黙ってそちらを使う。

対処は、`with` の本体から単相化した `def`・lambda・継続へ値引数を **capture** として引き継ぐこと。capture の id は呼び出し側のフレームのまま使う（閉包は環境の値を同じ id へ読み戻すので、lambda が `self.scope` の束縛を capture にするのと同型である）。値引数があるときの直接呼び出しは閉包にして呼ぶ。載せる組は handler の並びだけで決まるので、同じ単相化を共有する呼び出し位置で capture の数が食い違わない。

```
$ cargo run --offline -q -- run target/v6-val/main.hd   # 直した後
7
$ cat target/v7-nested/main.hd
effect Ask
once ask: Int
handler Ask as Inner(n: Int)
once ask => n
handler Ask as Outer(m: Int)
once ask => m
def use (x: Int): Int {Ask} => ask + x
def main = with Outer(10) -> (with Inner(3) -> use 100)
$ cargo run --offline -q -- run target/v7-nested/main.hd   # 内側が勝つ
103
```

#### 足した検査（赤 → 緑）

`handler_value_arg_once`（`once` の腕を 2 段先の関数から呼ぶ。`return` の腕も値引数を使う）と `handler_value_arg_shadow`（値引数と呼んだ関数の引数が同名）を受理コーパスに足した。赤は直す前の等価な入力で測った。

```
$ cargo run --offline -q -- run target/v6-val/main.hd            # = once の形。直す前
function definition requires a function type   (exit=1)
$ cargo run --offline -q -- run target/v6-samename/main.hd       # = shadow の形。直す前
200
$ cargo run --offline -q -- run tests/accept/handler_value_arg_once/main.hd     # 直した後
49
$ cargo run --offline -q -- run tests/accept/handler_value_arg_shadow/main.hd   # 直した後
106
```

`with` の実引数の 1 回評価（`handler_value_arg_effect` の `arg` 1 行と `r26_effects` のテスト）は直した後も変わらない。


### 仕様への疑義（人が決める）

```
主張:     §38 の「引数の個数と型」の誤りが §56 のどの行か。§56 は handler の値引数に行を持たず、§38 は個数・型の照合を明文化していない
再現:     handler H as Fixed(n: Int) / with H(1, 2) -> … と with H("s") -> …
現行:     E301（個数、アンカーは handler 名）と E305（型、アンカーは食い違った実引数）を適用の類推で選んだ。§56 は変更していない
変えると: §56 に行を足せば拒否コーパスが 1 本増え、門番の件数が追随する。§38 に 1 文足せば適用と同じ規則であることが読める
推奨:     §38 に「値引数は適用と同じく個数と型が合わなければならない」を 1 文足し、コードは E301・E305 のままにする（新しい行を足すと §56 の表と既存の 2 コードの意味が重なる）
```
