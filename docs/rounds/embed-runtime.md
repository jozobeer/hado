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

工程 3。埋め込んだ木（`94e232b`）で測った（2026-09-22）。`cargo test` の基準値は main と同じコードの worktree `readme-users` で同時刻に測り、どちらも他の走行と CPU を分け合っている。

```
$ time bash scripts/verify-embed-runtime.sh
OK: cargo install --path crates/hado into a private root
OK: the --target-dir of the install is removed
run: 111 accept cases match their headers (109 expect output)
OK: hado run of every accept case matches its header outside the checkout
build: 111 accept cases match their headers (109 expect output)
OK: hado build of every accept case links and the executable matches
PASSED: 4/4
real	1m18.343s
$ time cargo test --offline 2>&1 | grep -E "^test result" | grep -vc "0 failed"
0
real	8m58.740s
$ time cargo test --offline 2>&1 | grep -E "^test result" | grep -vc "0 failed"    # 基準。main の木、worktree readme-users
0
real	7m50.872s
$ cargo fmt --all --check; echo fmt=$?
fmt=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.99s
$ bash scripts/verify-cognitive-complexity.sh 2>&1 | tail -1
PASSED: 3/3
$ cargo build --offline --release 2>&1 | tail -1; ls -la target/release/hado
    Finished `release` profile [optimized] target(s) in 1m 01s
-rwxrwxr-x 2 zyun zyun 18394232 Sep 22 00:17 target/release/hado
$ ls -la /tmp/claude-1000/-home-zyun-git-local-hado/9e879231-0b7a-468b-bd3a-244a2e3465ca/scratchpad/install/bin/hado    # 埋め込む前の木（1657838）から cargo install --path crates/hado --root した release
-rwxrwxr-x 1 zyun zyun 10869808 Sep 21 23:52 /tmp/claude-1000/-home-zyun-git-local-hado/9e879231-0b7a-468b-bd3a-244a2e3465ca/scratchpad/install/bin/hado
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 10/10
$ bash scripts/verify-r42.sh 2>&1 | tail -1
PASSED: 5/5
$ bash scripts/corpus-lock.sh --check; echo exit=$?
exit=0
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 38/38 passed; red: none
```

並行リンクの衝突（中断の条件の 2 つ目）。`link_executable` を呼ぶ 15 本のテストと `build.rs` の作業ディレクトリを読んだ。各テストは目的名と pid（ループでは添字も）で別々の一時ディレクトリを作り、同じ stem `test.o` を繰り返すループ（`crates/hado/tests/r35_array.rs` の `polymorphic_allocation_specializes_and_slots_execute`）は直列で、書き出しはリンクごとに書いて消す。同じディレクトリで同時に走るリンクは無い。スレッド数を上げた再走でも赤は出なかった。

```
$ time cargo test --offline -- --test-threads=32 2>&1 | grep -E "^test result|FAILED|panicked" | grep -v "0 failed"
real	5m58.583s
```

### 着地後の全走（2026-09-22）

着地は PR #27（squash、`a3a2a25`）で main に入った。PR の `gates` は core 層で緑（head `b3239bb`、run 35659078033）。着地後の main の push では CI が full 層を回し、30 分 34 秒で `success`（run 35660081049）。`red: none`。決定 3 の README への `cargo install` の案内は、この記録と同じ PR で足した。

```
$ gh run list --branch main --limit 1
completed	success	feat: ランタイムの静的ライブラリを hado に埋め込む（embed-runtime） (#27)	ci	main	push	35660081049	30m34s	2026-09-21T21:57:18Z
$ gh run view 35660081049 --log | grep TALLY
TALLY: 3788/3788 passed; red: none
```

着地後の main（`a3a2a25`）を新規 clone し、README の手順で build と install をしてから checkout を消し、別のディレクトリで走らせた。`hello.hd` は `docs/getting-started.md` の「失敗のレール」、`hello_test.hd` は `def main = if 1 + 1 == 2 => () | raise { message = "arithmetic" }`。

```
$ git clone git@github.com:jozobeer/hado.git src && cd src
$ cargo fetch --locked
$ cargo build --offline
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.85s
$ cargo run --offline -q -- run tests/accept/create_user/main.hd
ok
$ cargo install --offline --path crates/hado --root ../root
   Installed package `hado v0.1.0 (/tmp/claude-1000/-home-zyun-git-local-hado/9e879231-0b7a-468b-bd3a-244a2e3465ca/scratchpad/e2e/src/crates/hado)` (executable `hado`)
$ ls -la ../root/bin/hado
-rwxrwxr-x 1 zyun zyun 18464800 Sep 22 07:01 ../root/bin/hado
$ cd .. && rm -rf src && mkdir app && cd app
$ ../root/bin/hado check --print-type hello.hd
String {Error}
$ ../root/bin/hado run hello.hd
ok
$ ../root/bin/hado build hello.hd && ./hello
ok
$ ../root/bin/hado fmt --check hello.hd; echo exit=$?
exit=0
$ ../root/bin/hado test hello.hd
test: hello_test.hd ok
test: 1/1 passed
$ ../root/bin/hado check --format json bad.hd
{"path": "bad.hd", "line": 2, "col": 3, "end_line": 3, "end_col": 4, "code": "E501", "message": "binding x is returned unchanged at 3:3", "canonical": "\"hello\"", "related": null}
```
