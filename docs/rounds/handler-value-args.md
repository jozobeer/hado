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
- `with` の実引数の 1 回評価は `r26_effects.rs` のテストでも測る。同じ形の stdout を完全一致で照合し、`List.length` の値まで見る。

### 成果物

- `crates/hado/src/name_resolution.rs`: `handler` が `HandlerDecl::params` を腕のスコープに入れる。
- `crates/hado/src/file_scopes.rs`: `handler` が `HandlerDecl::params` を束縛として数え、腕の中の参照を file-local の名前へ付け替えない。
- `crates/hado/src/infer.rs`: `with` の実引数と宣言型の照合、腕の推論での値引数の束縛。
- `crates/hado/src/mono.rs`: `with` の位置での 1 回評価と `Let`、`convert` の追加引数、継続の capture、腕の lower 中の `scope` の差し替え。
- `crates/hado/tests/r26_effects.rs`: 1 回評価の判定器。
- `tests/accept/handler_value_arg*`: 完了条件 (1)(2) の判定器（5 本）。
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
$ for c in handler_value_arg handler_value_arg_take handler_value_arg_nested handler_value_arg_return handler_value_arg_effect; do printf '%s: ' $c; out=$(cargo run --offline -q -- run tests/accept/$c/main.hd 2>&1); printf 'exit=%s out=%s\n' "$?" "$(echo "$out" | tr '\n' '|')"; done
handler_value_arg: exit=0 out=0|
handler_value_arg_take: exit=0 out=30|
handler_value_arg_nested: exit=0 out=52|
handler_value_arg_return: exit=0 out=3|
handler_value_arg_effect: exit=0 out=arg|
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

