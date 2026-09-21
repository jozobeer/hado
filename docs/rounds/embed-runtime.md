# cargo install した hado をビルド先に依存させない（embed-runtime）

開始 2026-09-22。

## 手順書

### 背景

`cargo install --offline --path crates/hado` で入れた `hado` は、`check` はどこに置いても動くが、`build` と `run` はビルド時の `target/release/build/hado-<hash>/out` の `libhado_runtime.a` と `libhado_float_format.a` を `cc` に渡す。`crates/hado/src/lib.rs` の `runtime_lib_path` と `link_executable` がコンパイル時の `env!("OUT_DIR")` をパスとして焼き込むためで、ビルド先を退避すると `ld: cannot find ...libhado_runtime.a` で落ちる（2026-09-21 実測）。prelude は `with_prelude` が `include_str!` で埋め込むので `check` は影響を受けない。

### 決定

- **決定 1: アーカイブを埋め込む。** 2 本の `.a` を `include_bytes!` で `hado` に埋め込み、`link_executable` がリンクの直前に object と同じディレクトリへ `<object の stem>.<アーカイブ名>` の名で書き出して `cc` に渡し、リンクの後に消す。呼び出し元の契約 `link_executable(obj, exe)` は変えない。書き出し先を object の隣にするのは、呼び出し元がその場所を既に私有している（`build.rs` の `Scratch`、`run` の `RunDirectory`、各テストの作業ディレクトリ）からで、名前に object の stem を付けるのは同じディレクトリで並行するリンクが衝突しないためである。
- **決定 2: 期待値はコーパスのヘッダから読む。** 門番はケース名を埋め込まず、`tests/accept/` をディスクから数え、`// expect:` / `// expect-error:` を `crates/hado/tests/corpus.rs` の `expected_output` と同じ読み方で判定する。0 件と、全件の期待値が空の判定は赤にする。
- **決定 3: 文書はこのラウンドで触らない。** README の `cargo run` 経由の案内は PR #24 が飛行中なので変えない。install の案内は着地後に別の `docs:` で足す。

### 期待値の根拠

`docs/corpus.md` のヘッダタグと `corpus.rs` の `expected_output`（`expect:` は終了コード 0・stdout 一致・stderr 空、`expect-error:` は終了コード 1・stdout 空・stderr 一致）。§58 の `build` と `run` の出力先。

### 成果物

- `crates/hado/src/lib.rs`: `runtime_lib_path` を消し、埋め込んだアーカイブと書き出しを足す。
- `scripts/verify-embed-runtime.sh`（full）。
- `docs/rounds/embed-runtime.md`（本ファイル）。`docs/issues/embed-runtime.md` は着地で消す。

### 工程

1. issue と本ファイルの手順書（`docs:`）。
2. 門番を書き、埋め込む前の木で赤を取る（`test:`）。
3. 埋め込み（`feat:`）。`cargo test`、rustfmt、clippy、認知的複雑度、門番。
4. 実測を足し、issue を消す（`docs:`）。

### 判定

- `bash scripts/verify-embed-runtime.sh` が `PASSED: 4/4`。
- `cargo test --offline` が 0 failed。
- `bash scripts/verify-rustfmt.sh`、`bash scripts/verify-clippy.sh`、`bash scripts/verify-cognitive-complexity.sh` が緑。
- `bash scripts/run-gates.sh --tier quick` が `red: none`。

### 許容される赤

なし。

### スコープ外

- 埋め込みで増えるバイナリ（release で約 7.5 MB。`libhado_float_format.a` が Rust の staticlib で std を含む）を減らすこと。
- README と AGENTS.md への install の案内（決定 3）。
- `cc` と `libgc-dev` の同梱。install 先でも両方は要る。

### 中断の条件

- `include_bytes!` で `cargo build` の時間か `cargo test` の時間が受け入れ難く増えるとき。基準値は着手前に測る。
- テスト群の並行実行で書き出しが衝突するとき（`cargo test` が非決定的に赤）。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §58 | `build` は入力の隣に、`run` は一時ディレクトリに実行ファイルを作る。実行ファイルの作成は `hado` のビルド先の存在に依存しない（推定。§58 は出力先だけを定める） | 実行 | 受理 | `scripts/verify-embed-runtime.sh` |

## 実測

工程 2。埋め込む前の木（`1657838`）で門番を走らせ、赤を取った（2026-09-22）。

```
$ time bash scripts/verify-embed-runtime.sh
OK: cargo install --path crates/hado into a private root
OK: the --target-dir of the install is removed
AssertionError: 111 of 111 failed: abort_arm_effectful: status=1 stdout=b'' stderr=b'/usr/bin/ld: cannot find /tmp/tmp.MPLlOg2NkS/target/debug/build/hado-f0f49c3c5eec3264/out/libhado_runtime.a: No such file or directory
NG: hado run of every accept case matches its header outside the checkout
NG: hado build of every accept case links and the executable matches
PASSED: 2/4
real	0m55.234s
```
