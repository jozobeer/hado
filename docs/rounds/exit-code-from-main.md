# `main` から終了コードを返す（exit-code-from-main）

## 手順書

### 背景

§54 は終了コードを 0 と 1 の二値に固定する。`raise` が届いても印字が失敗しても 1 で、プログラムが自分で決められない。stderr への書き込みも `Io` に無く、`write_file "/dev/stderr"` で代用できることを実測した（2026-09-17、`docs/issues/exit-code-from-main.md`）。

`runtime/hado_runtime.c` の `hado_abort_error` が `exit(1)` で `Failure is an effect` を OS の境界で 0/1 に潰し、`emit.rs` の `define_int_entry` が `main` の値を印字して 0 を返す。`Io` の操作は 5 つ（`read_file`・`write_file`・`log`・`read_dir`・`read_stdin`）で、網羅的な `handler Io` を持つ門番（`scripts/verify-r37.sh`・`scripts/verify-r43.sh`）が固定している。

### 決定

issue の「案」と「完了条件」をそのまま実装する。

- `main` の型が `Int` のときだけ、0 以上 255 以下の値を終了コードにする。それ以外の型は今までどおり印字して 0。
- `Int` が範囲外なら `hado: exit code out of range` を stderr に書いて 1 で終わる。i64 から ABI の i32 へ縮める前に判定する。
- `Io` に `log_error(contents: String): Void {Error}` を足す。`log` と同じ規則で改行を 1 つ書き、失敗は `message` を `"IoFailure"` とする `Error`。
- 退ける: `exit` 操作。

### 意図

hado を書く人と、hado を走らせる道具（CI・シェルスクリプト・`hado test`）が、プログラムの結果を終了コードで受け取れるようにする（推定）。失敗の報告先として stderr を `Io` の操作にする。

### 仮説

- 終了コードの判定は `emit.rs` の `define_int_entry` に閉じる。`main` の結果型は `program.entry.ty()` で既に見えているので、`Int` のときだけ「範囲内ならその値、範囲外なら stderr に 1 行書いて 1」を返し、それ以外の型は今までどおり印字して 0 を返す分岐を足せる。
- `log_error` は既存の `log` の写しで足りる。`Io` は `stdlib/io.hd` の `effect`・`module IoDefault`・`implement IoDefault as IoDefault` の 3 箇所と、`runtime_abi.rs` の `RuntimeFunction`・`runtime/hado_runtime.c` の `hado_io_log` の写しを足せば、mono と emitter は intrinsic 表から自動で追随する。
- コーパスの `// expect:` は `crates/hado/tests/corpus.rs` の `expected_output` が終了コードも照合するので、`main` が `Int` を返す受理ケースは `Int.to_string` で包まないと赤になる。包めば印字の期待はそのまま保てる。
- `scripts/verify-m1-corpus.sh` の項目被覆は `// item:` と `// spec:` のタグだけを見るので、`def main` の書き換えでは動かない。

### 判定器

issue の完了条件をそのまま使う。8 項目。

1. `def main = 0`・`1`・`3`・`255` がその終了コードで終わり、何も印字しない。`def main = raise ...` は 1 のまま。
2. `def main = -1`・`256`・`9223372036854775807`・`(-9223372036854775807 - 1)` が範囲外の診断を stderr に書いて 1 で終わる。生成した実行ファイルと `hado run` の両方で判定する。
3. `log_error "x"` を呼ぶ `def main` を build と `hado run` で走らせ、stdout は空、stderr は `x` と改行 1 つだけになる。`log` は stdout、`log_error` は stderr に書く対照を同じ門番に置く。
4. stderr の読み手が先に終わって書き込みが失敗する状態で `log_error` を呼ぶと、`handler` が `message` を `"IoFailure"` とする `Error` を捕まえる。捕まえなければ 1 で終わる。§55 の失敗の表に `log_error` の行を足す。
5. `tests/accept/` の入口 `main` が `Int` を返し `// expect:` が数値のケースを棚卸しし、`Int.to_string` で包むなどして移行する。移行の前後で `scripts/verify-m1-corpus.sh` の項目被覆が変わらない。`tests/corpus.sha256` を作り直す。
6. 実行する `main` が `Int` を印字に使う非コーパスの門番とテストも同じ棚卸しに含める（`crates/hado/tests/walk.rs`、`scripts/verify-r29.sh`、`scripts/verify-r43.sh` など）。`cargo test` と各 `verify-*.sh` が移行の前後で同じ被覆を測る。
7. `scripts/verify-r54.sh` の `a_test.hd`・`b_test.hd`・`c_test.hd` を、2 以上 255 以下のゲスト終了コード `n` を返す `*_test.hd` 1 本を含む形に置き換える。`hado test` の標準出力に §58.2 の `test: <相対パス> failed (exit <n>)` が同じ `n` で出ること、パス順でその後の `*_test.hd` が走って `ok` になること、集約の終了コードが 1 であることを直接測る。
8. `Io` に `log_error` を足すと既存の網羅的な `handler Io` は腕が 1 つ足りなくなり E605 になる。`scripts/verify-r37.sh` の `io_arms`（5 操作）と `scripts/verify-r43.sh` の固定 5 腕を `log_error` 込みに移行し、腕を欠かすと E605 になる検査と網羅の被覆を弱めずに保つ。§54 の表と「二値を保つ」の文を改める。

本ラウンドの門番は `scripts/verify-exit-code-from-main.sh`（新設、`# tier: core`・`# round: exit-code-from-main`）。上記 8 項目のうち 1・2・3・4 と 7 を直接測る。

### スコープ

触る: `runtime/hado_runtime.c`（`hado_print_error_string` の周辺と `hado_io_log_error`）、`crates/hado/src/emit.rs`（`define_int_entry`）、`crates/hado/src/runtime_abi.rs`（`IoLogError`）、`crates/hado/src/mono.rs`（要れば intrinsic の綴り）、`stdlib/io.hd`、`docs/spec.md`（§54・§55）、`tests/accept/`（数値 `// expect:` の棚卸し）・`tests/corpus.sha256`、非コーパスの門番・テスト（`scripts/verify-r29.sh`・`scripts/verify-r37.sh`・`scripts/verify-r43.sh`・`scripts/verify-r54.sh`、`crates/hado/tests/walk.rs`・`run_cli.rs` など）、`scripts/verify-exit-code-from-main.sh`（新設）、`docs/rounds/exit-code-from-main.md`。

触らない: `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`（`docs/rounds.md`）。他のレーンの worktree。`docs/plans/issue-backlog.md`。

### 未確認

- `Int` 以外の型の `main` が今までどおり印字して 0 で終わることを、既存のどの門番が測っているか（`verify-r29.sh` は String・Char の印字を見る）。
- `log_error` の intrinsic 名（`__io_log_error`）を `mono.rs` の intrinsic 解決がそのまま拾うか。
- 範囲外の判定を emitter に置いたとき、`verify-m5.sh` の `dump-mono` の期待（`print_Int` を含む）が動くか。
- `hado test` の `failed (exit <n>)` を直接測る形にしたとき、`verify-r54.sh` の他の検査（54-b・54-c・54-d・54-e）が `Int.to_string` を要するか。

### 成果物

- `runtime/hado_runtime.c`: `hado_io_log_error` と、範囲外の診断を書く entry 用の関数。
- `crates/hado/src/emit.rs`: `define_int_entry` の `Int` 分岐。
- `crates/hado/src/runtime_abi.rs`・`stdlib/io.hd`: `log_error`。
- `docs/spec.md`: §54 の表と文、§55 の操作と失敗の表。
- `tests/accept/`・`tests/corpus.sha256`: 棚卸し。
- `scripts/verify-r37.sh`・`scripts/verify-r43.sh`・`scripts/verify-r54.sh`・`scripts/verify-r29.sh`: 移行。
- `scripts/verify-exit-code-from-main.sh`: 本ラウンドの門番（`# tier: core`・`# round: exit-code-from-main`）。

### 工程

1. 失敗するテストを先に置く（`crates/hado/tests/exit_code.rs` と `scripts/verify-exit-code-from-main.sh` の枠）。赤を示す。
2. `emit.rs` の `define_int_entry` を `Int` のときだけ終了コードにする。`log_error` を `effect`・`implement`・ABI・C に足す。
3. 仕様（§54・§55）を追随させる。
4. コーパスと非コーパスの門番・テストを棚卸しする。
5. `git merge main`・corpus lock の作り直し・ラウンドの実測。

### 判定

主要コミットの単位ごとに触った門番 1 本を回す。最後に `cargo test --offline`・`bash scripts/run-gates.sh --tier quick target/gates-quick`・`bash scripts/run-gates.sh --tier core target/gates-core`・`bash scripts/lint-docs.sh`・`bash scripts/verify-spec-contracts.sh` を 1 回ずつ回す。

### 許容される赤

- なし。

### スコープ外

- `exit` 操作（issue が退ける）。
- `docs/plans/issue-backlog.md` の R レーンの後続（`cranelift-opt-level` 以降）。
- 仕様の節番号を動かすこと、`docs/plan.md` への追記。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- `Int` 以外の型の `main` の印字が `define_int_entry` から外れる（型ごとの entry が要る）と分かった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §54 | `main` が `Int` なら 0..255 を終了コード、範囲外は診断して 1 | 実行 | — | `tests/exit_code.rs`・`verify-exit-code-from-main.sh` |
| §54 | `Int` 以外は今までどおり印字して 0 | 実行 | — | `verify-r29.sh`・受理コーパス |
| §55 | `Io.log_error` は stderr に改行 1 つ、失敗は `Error` | 効果 | — | `verify-exit-code-from-main.sh`・`verify-r37.sh` |
| §55 | 失敗の `message` は `"IoFailure"` | 効果 | — | `verify-r37.sh`・`verify-r43.sh` |
| §55 | `Io` の操作は 6 つで、網羅的な `handler` は 6 腕 | 網羅性 | E605 | `verify-r37.sh`・`verify-r43.sh` |
| §58.2 | `test: <相対パス> failed (exit <n>)` はゲストの終了コード | 実行 | — | `verify-r54.sh` |

## 実測

完了条件ごとに測る。境界の値は仕様から選び、生成した実行ファイルと `hado run` の
両方で測る。

```
$ bash scripts/verify-exit-code-from-main.sh
OK: 1 Int 0 is the exit code and prints nothing
OK: 1 Int 1 is the exit code and prints nothing
OK: 1 Int 3 is the exit code and prints nothing
OK: 1 Int 42 is the exit code and prints nothing
OK: 1 Int 255 is the exit code and prints nothing
OK: 1 String main prints and exits 0
OK: 1 Bool main prints and exits 0
OK: 1 Void main prints nothing and exits 0
OK: 1 raise reaching main exits 1 with the message
OK: 2 -1 is diagnosed
OK: 2 256 is diagnosed
OK: 2 i64 max is diagnosed
OK: 2 i64 min is diagnosed
OK: 3 log_error writes stderr only and log writes stdout
OK: 4 a failed log_error is a catchable IoFailure
OK: 4 an uncaught log_error failure exits 1
OK: 4 an uncaught stderr EPIPE exits 1, not 141
OK: 4 a caught stderr EPIPE is an IoFailure
OK: 4 a failed print exits 1
OK: 4 an Int main never writes so a broken stdout does not matter
OK: 5 every numeric // expect: entry prints its number
OK: 7 hado test reports each guest exit code in path order
PASSED: 22/22
```

条件ごとの当て方。

- 条件 1・2: 門番の 1・2 が、0・1・3・42・255 の終了コードと、-1・256・i64 の
  両端の診断を、build と `hado run` の両方で測る。i64 の最小値はリテラルに
  書けない（E112）ので `(-9223372036854775807 - 1)` で作る。`raise` が届く経路は
  1 のまま。
- 条件 3: 門番の 3 が `log_error` の stdout 空・stderr `x`（改行 1 つ）と、`log` の
  stdout を同じ門番で対照する。`crates/hado/tests/exit_code.rs` の
  `log_goes_to_stdout_and_log_error_goes_to_stderr` が同じ 3 経路を持つ。
- 条件 4: 門番の 4 が、`/dev/full` へ落ちる `log_error` を `handler` が
  `IoFailure` として捕まえること（終了コード 0）と、捕まえない場合の終了コード 1 を
  測る。`crates/hado/tests/exit_code.rs` の
  `log_error_failure_is_a_catchable_io_failure` が同じ 2 経路を持つ。§55 の失敗の表に
  `log_error` の行を足した。
- 条件 5: 43 本の `tests/accept/*/main.hd` の `def main` を `Int.to_string` で包み、
  35 本に `// needs: prelude` を足した。`// item:` `// spec:` `// expect:` `// error:`
  `// short-circuit:` は 1 行も変えていない。

```
$ for f in $(git ls-tree -r --name-only main -- tests/accept); do
>   diff <(git show main:$f | grep -E '^// (item|spec|expect|error|short-circuit):' | sort) \
>        <(git show HEAD:$f | grep -E '^// (item|spec|expect|error|short-circuit):' | sort) \
>     || echo "DIFF $f"
> done
header-diffs=0

$ bash scripts/verify-m1-corpus.sh
OK: 7 protected-files-unchanged
PASSED: 7/7
```

- 条件 6: 実行する `main` が `Int` を印字に使っていた非コーパスの門番とテストを
  移した。`crates/hado/tests/` は `walk.rs`・`run_cli.rs`・`build_cli.rs`・
  `emit_mono.rs`・`prelude.rs`・`named_record_constructors.rs`・`r9_module_types.rs`・
  `r9_unused.rs`・`r35_array.rs`・`parse_corpus.rs` と生成器の検査、`scripts/` は
  `verify-r29.sh`・`verify-r43.sh`・`verify-r54.sh`・`verify-m0.sh`・
  `verify-char-and-float-conversions.sh`・`verify-prelude-tail-recursion.sh`・
  `verify-r38.sh`。`verify-m0.sh` の `walk.rs` の針は `"3"`（印字）から
  `"def main = 1 + 2", 3`（終了コード）へ変え、判別力を保った。`verify-r38.sh` の
  `Int.parse` の受理 probe は `Int.to_string` で包み、拒否 probe は raise のまま
  終了コード 1 を測る。prelude を付けない `compile_source_to_object` の経路は
  `Int.to_string` を使えないので、`crates/hado/tests/corpus.rs` の成功例は
  `def main = "<n>"` へ、`emit_mono.rs` と `r35_array.rs` の Int は
  `__int_to_string` へ寄せた。

```
$ cargo test --offline
（778 判定が全て ok。0 failed）

$ bash scripts/run-gates.sh --tier quick target/gates-quick2
TALLY: 41/41 passed; red: none

$ bash scripts/run-gates.sh --tier core target/gates-core2
TALLY: 681/681 passed; red: none
```

- 条件 7: `scripts/verify-r54.sh` の 54-a が、2 以上 255 以下のゲスト終了コードを
  乱数で 2 本作り（`code` と `other`）、`test: a_test.hd failed (exit code)`・
  `test: b_test.hd failed (exit other)`・`test: c_test.hd ok` と `1/3 passed`、
  集約の終了コード 1 を測る。54-b は `raise` と `def main = 1` の経路を
  `failed (exit 1)` のまま保つ。

```
$ bash scripts/verify-r54.sh
OK: 54-e only the root directory is selected
PASSED: 5/5
```

- 条件 8: `scripts/verify-r37.sh` の `io_arms` を 6 腕（`log_error` 込み）にし、
  腕を欠かすと E605 になる `37-d(4) missing-arm` を保つ。`scripts/verify-r43.sh` の
  固定腕も 6 にし、`io handler: 6 arms accepted, missing log is E605` を測る。腕は
  減らしていない。§54 の表の Int の行を「10 進。ただし `main` の型が `Int` のときは
  印字せず、その値を終了コードにする（上記）」へ、本文を新しい規則に書き直した。
  旧文の針 `終了コードは 0 と 1 の二値` は落ちた門番 r76 のもので、
  `scripts/verify-spec-contracts.sh` の針を新しい規範文
  `その値がそのまま終了コードである` へ移した。仕様の節番号は動かしていない。

```
$ bash scripts/verify-r37.sh
main.hd:2:8: error[E605]: handler operations do not match the effect
OK: 37-d(4) missing-arm
PASSED: 262/262

$ bash scripts/verify-r43.sh
io handler: 6 arms accepted, missing log is E605, r37 gates follow
OK: 43-h Io handler and r37 follow-up
PASSED: 10/10

$ bash scripts/verify-r29.sh
PASSED: 24/24
```

- 着地前の corpus lock: `git merge main`（`b2587d1`）の後、`--check` の差分 44 行は
  すべて `stdlib/io.hd` と受理コーパスの `Int.to_string` 包み・`// needs: prelude` で
  説明でき、main 側の変更の説明を要する行は無かった。作り直した後の照合は一致する。

```
$ bash scripts/corpus-lock.sh --check | grep -E '^[+-][0-9a-f]{64}' | awk '{print $2}' | sort -u | wc -l
44
$ bash scripts/corpus-lock.sh && bash scripts/corpus-lock.sh --check && echo LOCK_OK
LOCK_OK

$ bash scripts/lint-docs.sh
PASSED: 16/16

$ bash scripts/verify-spec-contracts.sh
OK: the spec body keeps the needles of the dropped gates
OK: the gate tier lock matches the membership derived from # tier:
PASSED: 2/2
```

### 実装から分かったこと

- 仮説どおり、終了コードの判定は `emit.rs` の entry に閉じた。`program.entry.ty()`
  を `define_entry` へ渡し、`Int` のときだけ範囲判定の分岐を選ぶ。非 `Int` は
  今までどおり `hado_print` を通る。`verify-m5.sh` の `dump-mono` への懸念は空振りで、
  m5 は core 層で緑のまま。
- 仮説どおり、`log_error` は `log` の写し（`stdlib/io.hd` の 3 箇所・
  `runtime_abi.rs`・`hado_runtime.c`）で足り、mono と emitter は intrinsic 表から
  追随した。intrinsic 名 `__io_log_error` もそのまま解決した。
- 仮説「項目被覆はタグだけを見るので動かない」は当たり。ただし `Int.to_string` を
  使う受理ファイルには `// needs: prelude` が要った。
- 想定外: 非コーパスの門番が issue の挙げた 3 例より広く、core 層に 5 本の赤
  （m0・char-and-float-conversions・prelude-tail-recursion・r38・spec-contracts）が
  出た。いずれも Int の印字か spec の針への依存で、条件 6 の棚卸しで閉じた。
- 想定外: `hado test` の `failed (exit <n>)` の `n` がゲストの値そのものになった。
  `verify-r54.sh` は乱数で作った 2 値でこれを測る。
- 想定外: `hado test` の Int のゲストは何も印字しないので、`test:` 行の
  `ok` / `failed (exit <n>)` だけが結果を伝える。§58.2 の報告はそのままで足りた。

### 審査対応（2026-09-27）

指摘 1（第 2 入口の棚卸し漏れ）と指摘 2（stderr の EPIPE 未測定）への対応。

- 入口の洗い直し: `grep -rl '^def main' tests` の 218 本をファイル名で絞らずに見て、
  `main.hd` 以外の入口は 10 本だった。数値の印字を期待していたのは
  `tests/accept/two_entries/store_test.hd`（`// expect: 110`）と
  `tests/postfix-close/run/Q2/case.hd`（`// run-expect: 7`）・
  `tests/postfix-close/run/Q3/case.hd`（`// run-expect: 11`）の 3 本で、いずれも
  `Int.to_string` で包み、印字の期待をそのまま保った。拒否フィクスチャの `def main`
  は `// expect:` を持たないので対象外。
- 棚卸しを門番にした: `scripts/verify-exit-code-from-main.sh` の検査 5 が、`tests/` と
  `stdlib/` の数値の `// expect:`・`// run-expect:` を持つファイルをファイル名で
  選ばずに全て build して走らせ、印字と終了コード 0 を測る。期待値はヘッダから読む。

```
$ for f in $(grep -rlE '^// (run-)?expect: [0-9]+' tests stdlib); do
>   while IFS= read -r want; do ... done < <(grep -oE '^// (run-)?expect: [0-9]+' "$f" | awk '{print $NF}')
> done
RED tests/accept/two_entries/store_test.hd want=110 got= rc=110
RED tests/postfix-close/run/Q3/case.hd want=11 got= rc=11
RED tests/postfix-close/run/Q2/case.hd want=7 got= rc=7
total=46 red=3

（3 本を Int.to_string で包んだ後）
total=46 red=0
```

- EPIPE: `scripts/exec-stderr-epipe.py` を足した（`exec-stdout-epipe.py` の fd 1 を
  fd 2 にしたもの。読み手の居ないパイプへ stderr を繋ぐ）。`/dev/full`（ENOSPC）の
  検査は置き換えずに残し、両方を測る。`runtime/hado_runtime.c` の
  `signal(SIGPIPE, SIG_IGN)` により、捕捉すれば `IoFailure`・捕まえなければ 1
  （141 ではない）。

```
$ bash scripts/verify-exit-code-from-main.sh
OK: 4 a failed log_error is a catchable IoFailure
OK: 4 an uncaught log_error failure exits 1
OK: 4 an uncaught stderr EPIPE exits 1, not 141
OK: 4 a caught stderr EPIPE is an IoFailure
OK: 5 every numeric // expect: entry prints its number
PASSED: 22/22
```

- 測り直し: `cargo test --offline` は 778 判定 / 0 failed、`bash scripts/run-gates.sh
  --tier quick target/gates-quick3` は 41/41、`--tier core target/gates-core3` は
  684/684（新しい 3 検査を含む）。`verify-r37.sh` 262/262・`verify-r43.sh` 10/10・
  `verify-m1-corpus.sh` 7/7。`corpus-lock.sh --check` の差分は `store_test.hd` の
  1 本だけで、作り直して一致。`gate-tiers-lock.sh --check` は差分なし。

### 仕様への疑義（人に上げる）

```
主張:     §54 の改訂後、`Int` の `main` に `raise` が届いたときの終了コード
再現:     `def main = if 1 == 1 => raise { message = "boom" } | 0`（型 `Int {Error}`）→ 終了コード 1・stderr `boom`
現行:     1903 行は `raise` を「`main` の型が `Int` 以外なら」の段落で書き、1909 行も「1 は `Int` 以外の `main` で `raise` が届いた場合」と書く
変えると: 挙動は変えない。本文を「`raise` が届いたら型に関係なく 1（stderr に message）」と改めれば、`def main = 1` との区別は stderr が空かどうかだけだと読める
推奨:     本文にその 1 文を足す。実装（`hado_abort_error` の `exit(1)`）は既にそうで、文の穴を閉じるだけ。仕様を触るかは人が決める
```
