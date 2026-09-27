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
