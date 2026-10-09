# ライブラリを読み込める REPL（repl）

開始 2026-10-10。方式は `docs/plans/repl.md` の推奨 A（JIT）を人が選んだ。

## 手順書

### 背景

`docs/issues/repl.md` の段階 1。hado の実行は AOT で、評価の状態を入力のあいだ持ち越す仕組みが無い。素直に「入力ごとに、ここまでの入力を全部含むプログラムをビルドして実行する」と、前の入力の効果が毎回やり直される（`log "a"` の次に `1 + 1` を打つと `a` が 2 回出る）。issue の完了条件は「対話の入口があること」と「前の入力の効果が再実行されないこと」の両方を要求する。

`docs/plan.md` は JIT を v0.2 へ保留しているが、`emit` は `ObjectModule` と `JITModule` が同じ `Module` trait を実装するので共有できる、と見積もっている（`docs/plans/repl.md`）。段階 1 の方式は人（審査）が A（JIT）と決めた。

### 決定

- **決定 1: セッションのプロセスを 1 つ保つ。** `hado repl` は入力ごとに `hado_step` という関数を合成し、`JITModule` へ足してその場で 1 回だけ呼ぶ。前の入力の効果はそのプロセスのメモリで済んでいるので、再実行されない。JIT したモジュールは閉包のコードを指しているあいだ落とさず、セッションの終わりにまとめて捨てる。
- **決定 2: 束縛はソースの型注釈つきの仮引数として次の入力へ渡す。** `e as x ->` の値は機械語のビット列のまま `Binder` に持ち、次の入力で `def hado_step (x: <型>) => <本体>` の仮引数にする。値のソースへの埋め戻しはしないので、Int でも Bytes でも閉包でも同じ経路で持ち越せる。仮引数にするのは本体に字句として現れる束縛だけで、残りは E505（未使用の束縛）になるので入れない。
- **決定 3: §10 の規則は合成した `main` にそのまま受ける。** 仮引数が要らない入力は `def main = <式>`、要る入力は `def hado_step (...) => <式>` と `def main = hado_step` にする。後者は必ず E702（`main` は関数を含む値を返せない）になるので、その 1 コードだけを落とす。効果（E706）と単相性（E701）はそのまま効かせる。単相性は repl 側でも値の型で見て、印字できない値（関数値）は印字の段で断る。
- **決定 4: 値は §54 の `main` の印字規則で見せるが、Int を終了コードにする規則は当てない。** 対話では値を見せる必要があるため。この 1 点は仕様の外の決定である（「仕様への疑義」を参照）。
- **決定 5: C ランタイムは共有ライブラリを `dlopen` して住所だけ渡す。** `libhado_repl_runtime.so` を `build.rs` で作り、`include_bytes!` で埋めて一時ファイルへ書き、`dlopen` したうえで `RuntimeFunction` ごとに `dlsym` した住所を `JITBuilder::symbol` へ渡す。ゲスト向けの `libhado_runtime.a` をそのまま `hado` 本体へ静的リンクすると、panic=abort の Rust staticlib である `libhado_float_format.a` が `__rustc::rust_begin_unwind` を std と二重に定義する。`--defsym` で 0 に束ねる 2 つの未定義シンボル（`rust_eh_personality`・`hado_entry`）は JIT から呼ばない。
- **決定 6: `emit` は総称化ではなく `&mut dyn Module` にする。** `define_mono_function`・`define_entry`・`emit_int_exit_code` の受け口を trait object にした。これらの道具は `Module` の非総称なメソッドしか使わず、呼び出し側の型を名指しする必要が無いため。`ObjectModule` の経路（`build`・`run`）は呼び出しのままで、生成物は変わらない。
- **決定 7: `<dir>` は §52 のライブラリでなければならない。** セッションの根を一時ディレクトリに作り、`<dir>` をその名前の下へ写して `name::実装.member` で見えるようにする（§51）。名前が名前空間の字句でないディレクトリと、`library` を宣言しないディレクトリは入口で断る。`stdlib/` は §52 のライブラリではなく、根の宣言として読むと prelude と衝突して入力ごとに大量の E205 が出るためである。
- **決定 8: 入力の完結は字句で決める。** 括弧（`(` `[` `{`）が釣り合い、`fn`・`module`・`implement`・`effect`・`handler` の開きと `/…` の閉じが釣り合い、最後の字句が `=>`・`|>`・`|`・`,`・`=` でなければ送る。`->` で終わる入力は束縛の印として送る（未完にしない）。字句が読めない入力は未完として続きを待つ。

### 期待値の根拠

- §11: 「`->` の後ろ全部が継続」をセッションの残りに当てる。1 入力が継続の 1 歩である。
- §54: 値の印字は `main` の値と同じ規則（Int は 10 進、String は引用符なし、Bool は `true`/`false`、Char は 1 字、Void は何も出さない）。Int を終了コードにする部分だけは実行ファイルの規則なので当てない。
- §10: `main` に残ってよい効果と型の単相性。E701・E705・E706 が同じ段階で当たる。
- §52・§51: `<dir>` を名前空間として根の下に置き、`library` の宣言を要求する。
- §14: 行頭に `|>` を置く書き方は、入力の先頭が `|>` になるのでセッションでは使えない（issue の段階 1 の但し書きどおり）。

### 成果物

- `crates/hado/src/repl.rs`: セッション（完結判定・束縛・合成・JIT・橋・印字）。
- `crates/hado/src/lib.rs`・`crates/hado/src/main.rs`・`crates/hado/src/project.rs`・`crates/hado/src/emit.rs`: `repl` コマンドの受け付け、`Project::lower_repl_step`、`define_mono_function` の共有。
- `crates/hado/build.rs`: `libhado_repl_runtime.so` の組立。
- `crates/hado/tests/repl.rs`: 完了条件を測るテスト。
- `scripts/verify-repl.sh`（quick・`# round: repl`）。
- `docs/rounds/repl.md`（本ファイル）、`docs/plans/repl.md`（方式の決定）。
- `docs/issues/repl.md` は着地で消す。段階 2（シェル向けライブラリ）の文面はそこに残っているので、切り出しは着地側で行う。

### 工程

1. 探針: JIT したコードから C ランタイムの関数を呼べるかを確かめる（ビルドの分離・シンボル解決）。`cranelift-jit` を依存に足す（`build:`）。
2. 探針: `emit` の関数定義を `JITModule` で共有できる形にする（`refactor:`）。`Module` trait で総称化する。
3. 実装: `repl` コマンドとセッション（`feat:`）。
4. テスト: `crates/hado/tests/repl.rs` と門番（`test:`）。
5. 記録: 本ファイルと方式の決定（`docs:`）。
6. 最後に 1 回だけ `bash scripts/run-gates.sh --tier quick target/.gates-repl` と `cargo test --offline`。

### 判定

- `bash scripts/verify-repl.sh` が `PASSED: 6/6`。
- `cargo test --offline --test repl` が 0 failed（完了条件の照合テストを含む）。
- `bash scripts/run-gates.sh --tier quick` が `red: none`。
- `cargo test --offline` が 0 failed。
- `bash scripts/verify-rustfmt.sh`、`bash scripts/verify-clippy.sh` が緑。
- `bash scripts/lint-docs.sh` が PASSED。`bash scripts/corpus-lock.sh --check` と `bash scripts/frozen-docs-lock.sh --check` が差分なし。

### 許容される赤

なし。`bash scripts/verify-cognitive-complexity.sh` は 2/3 で赤になるが、これは base（`4baca98`）から変わっていない `crates/hado/src/postfix_print.rs` と `crates/hado/src/mono.rs` の 3 関数を指しており、このラウンドの変更ではない（実測を参照）。自分の変更で緑にできる赤ではないので、門番は緩めない。

### スコープ外

- 定義（`def`・`type`・`module`・`implement`・`effect`・`handler`）を入力として受けること。段階 1 は式と `e as x ->` だけを受ける。定義の入力は 1 行の断りを出して次の入力へ進む。
- prelude の検査結果をセッションで使い回すこと。入力ごとに prelude を検査し直すので、1 入力あたり数秒かかる（実測を参照）。速さは段階 1 の条件ではない。
- JIT したコードのメモリを入力ごとに返すこと。セッションの間は落とさない（閉包のコードを指すため）。`JITModule::free_memory` は明示的で、入力ごとの解放は閉包の寿命の設計が要る。
- 段階 2（シェル向けライブラリ、`Process` 効果）。`docs/issues/repl.md` の段階 2 として切り出す。
- 対話の前口上・プロンプトの体裁。標準入力が端末のときだけ `> ` を標準エラーへ出す。

### 中断の条件

- JIT したコードから C ランタイムの関数を呼べないことが 2 回続けて確かめられたら、方式 A を止めて報告する（その測定自体を成果物にする）。
- セッションの根を作れない、または `dlopen` が失敗する環境なら止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §11 | `->` の後ろ全部が継続。セッションは `main` の本体の続きで、入力 1 つが継続の 1 歩 | 構文 | 受理 | `crates/hado/tests/repl.rs`、`scripts/verify-repl.sh` |
| §54 | 値の印字は `main` の値と同じ規則。Int を終了コードにする規則は当てない（決定 4） | 実行 | 受理 | `crates/hado/tests/repl.rs` |
| §10 | `main` に残ってよい効果と型の単相性。単相でない値は受けない | 型 | E701 / E705 / E706 | `scripts/verify-repl.sh` |
| §52 | `<dir>` のライブラリを読み込んだセッションを始める。`library` を宣言しないディレクトリはライブラリではない | 名前解決 | 受理 / 入力の誤り | `scripts/verify-repl.sh` |
| §51 | 読み込むディレクトリの名前は名前空間の字句でなければならない | 名前解決 | 入力の誤り | `scripts/verify-repl.sh` |

## 実測

探針 1・2。JIT したコードから C ランタイムの `hado_print_int` を呼べるか（方式 A のいちばん大きい風險）。探針の源は `target/probe/jit_probe.rs`（`target/` は追跡しない）に置き、`crates/hado/examples/jit_probe.rs` の薄い包みから走らせた（2026-10-10）。

```
$ cargo run --offline -q --example jit_probe
42
probe status=0
```

`dlopen` は最初 `rust_eh_personality` を解決できずに落ちた。`libhado_float_format.a` が panic=abort でも人格ルーチンの参照を持ち、`--gc-sections` では落ちきらないためである。`hado_entry` も runtime の `main` から参照される。どちらも JIT から呼ばないので、共有ライブラリのリンクで `--defsym` により 0 に束ねた。

```
$ nm -D --undefined-only target/debug/build/hado-f0f49c3c5eec3264/out/libhado_repl_runtime.so | grep -v '@GLIBC'
                 U GC_init
                 U GC_malloc
                 U GC_realloc
                 w __gmon_start__
                 w _ITM_deregisterTMCloneTable
                 w _ITM_registerTMCloneTable
```

工程 3・4。完了条件を測るテスト（2026-10-10）。`hado repl` を子プロセスで走らせ、標準入力の列に対する標準出力を照合する。

```
$ cargo test --offline -q --test repl
running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.44s
```

門番（`scripts/verify-repl.sh`）。完了条件の 2 つを外からそのまま測る。

```
$ bash scripts/verify-repl.sh
OK: 1 repl evaluates each complete input and prints its value
OK: 2 an earlier input effect is not re-executed
OK: 3 repl carries bindings and waits for a balanced input
OK: 4 hado repl <dir> loads that library and rejects a bad name
OK: 5 a non-monomorphic input is diagnosed and the session continues
OK: 6 repl takes at most one library directory
PASSED: 6/6
```

効果の再実行が無いことの直接の観測。`log` の入力の間に束縛と別の入力を挟んでも、前の `log` は繰り返さない。

```
$ printf 'log "a"\n1 as x ->\nx\nlog "b"\n' | target/debug/hado repl
a
1
b
```

`<dir>` の入口（決定 7）。issue の現象にあった `repl stdlib` は、`library` を宣言しないので入口で 1 行だけ断る。

```
$ printf '1\n' | target/debug/hado repl stdlib; echo "exit $?"
repl: stdlib is not a §52 library: no `library` declaration（宣言の無いディレクトリは根の宣言として prelude と衝突する）
exit 1
```

入力あたりの時間（決定 1 の費用）。同じ `1` を n 回与えたセッションの実時間。`hado check` は同じ木の自明な `main.hd` の検査で、prelude の検査だけで数秒かかる。

```
$ for n in 1 2 4; do printf '1\n%.0s' $(seq $n) > target/probe/n.txt; start=$(date +%s.%N); target/debug/hado repl < target/probe/n.txt > /dev/null 2>&1; end=$(date +%s.%N); echo "n=$n $(echo "$end - $start" | bc)s"; done
n=1  10.366965539s
n=2  16.344137063s
n=4  21.383992920s
$ target/debug/hado check target/probe/trivial/main.hd
check 9.391593487s
check 4.462177704s
```

工程 6。最後に 1 回だけ回した全走（2026-10-10）。機械は他のラウンドと CPU を分け合っており、load average は 20 前後だった。全走の出力は末尾 60 行だけを残した。最後の `real` の行が cargo test の正常終了を示し、`error: test failed` の行は無い。

```
$ bash scripts/run-gates.sh --tier quick target/.gates-repl 2>&1 | tail -6
RUN: verify-gate-cache -> .../target/.gates-repl/verify-gate-cache.log
RUN: verify-m1-spec -> .../target/.gates-repl/verify-m1-spec.log
RUN: verify-repl -> .../target/.gates-repl/verify-repl.log
RUN: verify-rustfmt -> .../target/.gates-repl/verify-rustfmt.log
RUN: verify-spec-model -> .../target/.gates-repl/verify-spec-model.log
TALLY: 47/47 passed; red: none
$ time cargo test --offline 2>&1 | tail -60
running 9 tests
test run_prints_the_main_value_and_leaves_no_binary ... ok
test build_reports_the_canonical_form ... ok
test build_writes_to_the_requested_output ... ok
test run_forwards_the_arguments_after_the_separator ... ok
test run_returns_the_program_exit_code ... ok
test run_reads_dunder_named_source_files ... ok
test run_skips_dunder_directories_that_are_not_namespaces ... ok
test run_passes_standard_input_through ... ok
test run_discovers_namespaces_beside_embedded_compiler_sources ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.52s
...
   Doc-tests hado

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

real	101m30.838s
```

この全走のあとに、決定 7 の入口の検査（`library` の宣言）を足した。足したあとで、影響する 2 本を回し直した（`cargo test --offline -q --test repl` が 9 passed、`bash scripts/verify-repl.sh` が 6/6）。全走そのものは入口の検査を足す前の木（`482c6eb`）で測った。

```
$ cargo test --offline -q --test repl
running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 26.95s
```

記録を足したあとの木でもう一度だけ quick 層を回した（2026-10-10）。この走りのあとに変えたのは本ファイルの記述だけである。

```
$ bash scripts/run-gates.sh --tier quick target/.gates-repl-final 2>&1 | tail -3
RUN: verify-rustfmt -> .../target/.gates-repl-final/verify-rustfmt.log
RUN: verify-spec-model -> .../target/.gates-repl-final/verify-spec-model.log
TALLY: 47/47 passed; red: none
```

形と lint の門番（2026-10-10）。

```
$ bash scripts/verify-rustfmt.sh 2>&1 | tail -2
OK: rustfmt --check crates/hado/float_format.rs reports no diff
PASSED: 2/2
$ bash scripts/verify-clippy.sh 2>&1 | tail -2
OK: clippy-driver --crate-type staticlib -D warnings on crates/hado/float_format.rs is clean
PASSED: 2/2
$ bash scripts/verify-cognitive-complexity.sh 2>&1 | tail -3
NG: cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity is clean
OK: no #[allow] / #![allow] for cognitive_complexity under crates/hado/src
PASSED: 2/3
$ bash scripts/corpus-lock.sh --check; echo exit=$?
exit=0
$ bash scripts/frozen-docs-lock.sh --check; echo exit=$?
exit=0
```

認知複雑度の赤は base から変わっていない 3 関数である（このラウンドの変更ではない）。`git diff --stat 4baca98` は空で、`clippy.toml` も `crates/hado/src/postfix_print.rs` も `crates/hado/src/mono.rs` も触っていない。認知複雑度は関数ごとの構文の性質なので、同じファイルと同じ閾値なら base でも同じ 3 件が赤になる。

```
$ cargo clippy --offline --lib --bins -- -F clippy::cognitive_complexity 2>&1 | grep -E '^error|-->' | head -6
error: the function has a cognitive complexity of (16/7)
   --> crates/hado/src/postfix_print.rs:165:4
error: the function has a cognitive complexity of (8/7)
   --> crates/hado/src/postfix_print.rs:774:4
error: the function has a cognitive complexity of (9/7)
   --> crates/hado/src/mono.rs:776:8
$ git diff 4baca98 -- clippy.toml crates/hado/src/postfix_print.rs crates/hado/src/mono.rs | wc -l
0
```
