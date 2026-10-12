# full 全走で赤になった embed-runtime と型変数閉包の門番を直す（full-tier-reds）

## 手順書

### 背景

`docs/plans/issue-backlog.md` の受け入れ条件「main で `bash scripts/run-gates.sh --tier full` が red なし」を満たす。main `b5b52d4` の full 全走（2026-10-10、`target/.gates-full`）は `TALLY: 3878/3884 passed; red:` で 4 本が赤かった。

- `verify-embed-runtime.log`: `cargo install` が一時の `--target-dir`（`/tmp/tmp.oWI5CQdMXI`、tmpfs）へ deps を書けず（`error writing dependencies ...: No such file or directory`）、`root/bin/hado` が作られない。検査 3/4 が `FileNotFoundError` になり `PASSED: 1/4`。
- `verify-pure-closure-in-type-variable-argument-fails-specialization.log`: 検査 6 が `mutation site not found` で `PASSED: 5/6`。
- `verify-r63.log`: PR #81（`045b3c9`）が直した。この枝では緑。
- `verify-string-split-linear.log`: 既知の床（`docs/issues/flaky-string-split-timing-gate.md`）。このラウンドの担当外。

2 本の層を確かめた。`verify-embed-runtime.sh` は `# tier: full`、`verify-pure-closure-in-type-variable-argument-fails-specialization.sh` は `# tier: core` である。後者は core 層の全走にも入る（`bash scripts/run-gates.sh --print-tier core` に出る）。

### 決定

- **決定 1（embed-runtime）: 門番の測り方を直す。実装は直さない。** 門番の測る対象「ビルド先を消した後も `cargo install` した `hado` が build / run できる」は本物で、実装は既に満たしている。門番を単独で回すと 4/4、`cargo install` は `bin/hado` を作る。赤は一時のビルド先を共有の `/tmp`（tmpfs）に置いていたための書き込み失敗で、同じ失敗はこのリポジトリのログ全体で 1 回しか無い（他の full 全走 7 回は 4/4）。検査 1〜4 は変えず、途中成果だけを worktree の `./target/embed-runtime.XXXXXX` に置く。受理ケースを走らせる場所は、チェックアウトの外であることを測るため `/tmp` のままにする。
- **決定 2（pure-closure）: 門番のアンカーを直す。実装は直さない。** 赤は `e666fec`（PR #84、2026-10-10 着地）の refactor が、本体を宣言側の outcome に包む分岐を `lambda_result` へ移し、`141f170` が入れたアンカー（`body = self.done(body, &declared_result)?; declared_result`）を消したためである。分岐は今もあり、欠陥（純粋な閉包を型変数の引数に置くと `specialization failed: Mismatch`）は今も防げている。アンカーを現在の 2 行へ更新する。検査の意味（変異で検査 2 が赤になること）は変えない。

### 期待値の根拠

`docs/corpus.md` のヘッダタグと `crates/hado/tests/corpus.rs` の `expected_output`。pure-closure は issue の完了条件（型変数の位置の純粋/効果 × `Map.insert` の V / `Option.unwrap_or` の A の 4 形が期待値ちょうどを返すこと）をそのまま検査にしている。

### 成果物

- `scripts/verify-embed-runtime.sh`: 一時のビルド先を `./target/embed-runtime.XXXXXX` へ移す。
- `scripts/verify-pure-closure-in-type-variable-argument-fails-specialization.sh`: 検査 6 の変異アンカーを現在のコードへ更新する。
- 本ファイル。

### 工程

1. main `045b3c9` から worktree `full-tier-reds` を切る。
2. embed-runtime: `cargo install` を単体で回して再現しないこと、過去の全走ログが 4/4 であることを確かめ、ビルド先を移す（`test:`）。単独 4/4。
3. pure-closure: `git log` でアンカーを消した commit（`e666fec`）を特定し、アンカーを更新する（`test:`）。変異で検査 2 が赤になることを確かめ、単独 6/6。
4. `cargo fmt --all --check`、`cargo clippy --offline --all-targets -- -D warnings`、`bash scripts/lint-docs.sh`、`bash scripts/corpus-lock.sh --check`。
5. quick / core / full の層別を回す（full は 1 回）。string-split は単独でも回す。
6. 実測を本ファイルに書く（`docs:`）。

### 判定

1. `bash scripts/verify-embed-runtime.sh` が 4/4。
2. `bash scripts/verify-pure-closure-in-type-variable-argument-fails-specialization.sh` が 6/6（変異で検査 2 が赤）。
3. quick / core が red なし。
4. full の赤が `verify-string-split-linear`（既知）だけ。
5. `git status --short` が空。

### 許容される赤

`verify-string-split-linear`（検査 i の絶対値の床。`docs/issues/flaky-string-split-timing-gate.md` が扱う）。

### スコープ外

- `verify-string-split-linear.sh` の床の修正。
- `verify-r63.sh`（PR #81 が着地済みで、この枝では緑）。

### 中断の条件

- 仕様の解釈に疑義が出た。
- 単独の門番が 2 回続けて赤になった。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §58 | `build` は入力の隣に、`run` は一時ディレクトリに実行ファイルを作る。実行ファイルの作成は `hado` のビルド先の存在に依存しない（推定。§58 は出力先だけを定める） | 実行 | 受理 | `scripts/verify-embed-runtime.sh` |
| §20 | 効果行は関数の型に載り、`with` が捕まえる | 単相化 | — | `crates/hado/tests/r26_effects.rs` |
| §38 | `with` の腕が効果操作を捕まえる | 単相化 | — | 同上 |
| §41 | 型の正準形 | 単相化 | — | 同上 |
| §46 | 契約の型引数 | 単相化 | — | 同上 |
| §47 | 実装の局所型 | 単相化 | — | 同上 |

## 実測

### b5b52d4 の full 全走の赤

担当する 2 本のログの末尾。

```
$ tail -6 .worktrees/postlanding-main/target/.gates-full/verify-embed-runtime.log
error: could not compile `log` (lib) due to 1 previous error
error: failed to compile `hado v0.1.0 (...)`, intermediate artifacts can be found at `/tmp/tmp.oWI5CQdMXI/target`.
NG: cargo install --path crates/hado into a private root
OK: the --target-dir of the install is removed
FileNotFoundError: [Errno 2] No such file or directory: PosixPath('/tmp/tmp.oWI5CQdMXI/root/bin/hado')
NG: hado build of every accept case links and the executable matches
PASSED: 1/4
```

```
$ cat .worktrees/postlanding-main/target/.gates-full/verify-pure-closure-in-type-variable-argument-fails-specialization.log
OK: 1 cargo-build
OK: 2 pure-closure-in-map-value
OK: 3 pure-closure-in-unwrap-or
OK: 4 effectful-closure-in-map-value
OK: 5 effectful-closure-in-unwrap-or
mutation site not found
NG: 6 mutation-site-missing
PASSED: 5/6
```

### embed-runtime の切り分け

失敗の先頭は、ビルド先への書き込みの失敗である（`cargo install` が `bin/hado` を作らないのは結果）。

```
$ sed -n '1p' .worktrees/postlanding-main/target/.gates-full/verify-embed-runtime.log
error: error writing dependencies to `/tmp/tmp.oWI5CQdMXI/target/debug/deps/log-0374e6dcb710939f.d`: No such file or directory (os error 2)
```

同じコマンドをこの枝で回すと通り、`bin/hado` を作る。

```
$ W=$(mktemp -d /tmp/embed-repro.XXXX); cargo install --offline --debug --path crates/hado --root "$W/root" --target-dir "$W/target" >"$W/install.log" 2>&1; echo "install_exit=$?"
  Installing /tmp/embed-repro.qjNr/root/bin/hado
   Installed package `hado v0.1.0 (...)` (executable `hado`)
install_exit=0
$ ls -la /tmp/embed-repro.qjNr/root/bin
-rwxrwxr-x 1 zyun zyun 83863424 hado
```

過去の full 全走の embed-runtime は 4/4 で、`1/4` はリポジトリのログ全体で 1 回だけである。

```
$ for f in $(find . -name verify-embed-runtime.log -not -path './.git/*'); do tail -1 "$f"; done | sort | uniq -c
      7 PASSED: 4/4
      1 PASSED: 1/4
$ grep -rl 'error writing dependencies' --include='*.log' . | wc -l
1
```

単独で 4/4（ビルド先を移した後）。

```
$ time bash scripts/verify-embed-runtime.sh 2>&1 | tail -7
OK: cargo install --path crates/hado into a private root
OK: the --target-dir of the install is removed
run: 146 accept cases match their headers (144 expect output)
OK: hado run of every accept case matches its header outside the checkout
build: 146 accept cases match their headers (144 expect output)
OK: hado build of every accept case links and the executable matches
PASSED: 4/4
real	9m37.869s
```

### pure-closure の切り分け

アンカーを消したのは `e666fec`（PR #84）である。`e666fec^` にはアンカーがあり、`e666fec` が分岐を `lambda_result` へ移した。`b5b52d4` は `e666fec` を含む。

```
$ git merge-base --is-ancestor e666fec b5b52d4 && echo yes
yes
$ git log -1 --format='%h %ci %s' 6249222
6249222 2026-10-10 02:14:32 +0900 Merge pull request #84 from jozobeer/printer-cognitive-complexity
$ git show e666fec^:scripts/verify-pure-closure-in-type-variable-argument-fails-specialization.sh | grep -c 'body = self.done(body, &declared_result)'
1
$ grep -n 'let body = self.done(body, declared_result)' crates/hado/src/mono.rs
5179:            let body = self.done(body, declared_result)?;
```

単独で 6/6。変異は検査 2 を赤にする（`specialization failed: Mismatch`）。

```
$ time bash scripts/verify-pure-closure-in-type-variable-argument-fails-specialization.sh 2>&1 | tail -9
OK: 1 cargo-build
OK: 2 pure-closure-in-map-value
OK: 3 pure-closure-in-unwrap-or
OK: 4 effectful-closure-in-map-value
OK: 5 effectful-closure-in-unwrap-or
exit=1 expected=102 stderr=[specialization failed: Mismatch]
OK: 6 mutation-turns-check-2-red
PASSED: 6/6
real	0m48.603s
```

### 静的検査

```
$ cargo fmt --all --check; echo fmt_exit=$?
fmt_exit=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1; echo clippy_exit=${PIPESTATUS[0]}
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.67s
clippy_exit=0
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/corpus-lock.sh --check; echo corpus_exit=$?
corpus_exit=0
```

### 層別の全走

quick。

```
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 41/41 passed; red: none
```

core。

```
$ bash scripts/run-gates.sh --tier core target/gates-core
TALLY: 755/755 passed; red: none
$ tail -1 target/gates-core/verify-cognitive-complexity.log
PASSED: 3/3
$ tail -1 target/gates-core/verify-pure-closure-in-type-variable-argument-fails-specialization.log
PASSED: 6/6
```

full（1 回）。

```
$ bash scripts/run-gates.sh --tier full target/gates-full
TALLY: 3883/3884 passed; red: verify-string-split-linear.log
$ tail -1 target/gates-full/verify-embed-runtime.log
PASSED: 4/4
$ tail -1 target/gates-full/verify-pure-closure-in-type-variable-argument-fails-specialization.log
PASSED: 6/6
$ tail -1 target/gates-full/verify-r63.log
PASSED: 4/4
$ tail -1 target/gates-full/lint-docs.log
PASSED: 16/16
```

### verify-string-split-linear（既知の床）

単独 2 回と full の 1 回とも、検査 i の絶対値の床（0.200s）を割る。

```
$ bash scripts/verify-string-split-linear.sh
split ascii lines: small=0.543s large=1.018s subtracted 0.230->0.397 ratio 1.72
split multibyte lines: small=0.391s large=0.763s subtracted 0.120->0.230 ratio 1.91
g: small=0.396s large=0.785s subtracted 0.239->0.486 ratio 2.03
h: small=0.368s large=0.724s subtracted 0.180->0.364 ratio 2.03
AssertionError: join: small median 0.187s is below the 0.200s floor
NG: i perf: join is linear in the element count
OK: j control: the subtracted ratio judge rejects the issue example
PASSED: 10/11
```

```
$ bash scripts/verify-string-split-linear.sh
split ascii lines: small=0.520s large=1.009s subtracted 0.205->0.391 ratio 1.91
split multibyte lines: small=0.388s large=0.747s subtracted 0.114->0.219 ratio 1.91
g: small=0.402s large=0.768s subtracted 0.248->0.466 ratio 1.88
h: small=0.375s large=0.737s subtracted 0.188->0.358 ratio 1.90
AssertionError: join: small median 0.190s is below the 0.200s floor
NG: i perf: join is linear in the element count
OK: j control: the subtracted ratio judge rejects the issue example
PASSED: 10/11
```

full の 1 回。

```
$ sed -n '/split ascii lines/,$p' target/gates-full/verify-string-split-linear.log
split ascii lines: small=0.492s large=0.974s subtracted 0.192->0.396 ratio 2.06
OK: e perf: split on ASCII lines is linear
split multibyte lines: small=0.363s large=0.721s subtracted 0.109->0.215 ratio 1.97
OK: f perf: split on Japanese lines is linear
g: small=0.381s large=0.739s subtracted 0.234->0.455 ratio 1.95
OK: g perf: split with a growing ASCII separator is linear
h: small=0.348s large=0.699s subtracted 0.176->0.350 ratio 1.99
OK: h perf: split with a growing Japanese separator is linear
AssertionError: join: small median 0.182s is below the 0.200s floor
NG: i perf: join is linear in the element count
OK: j control: the subtracted ratio judge rejects the issue example
PASSED: 10/11
```

### 木

全走のあと、追跡ファイルに変異は残っていない。

```
$ git status --short
$ git log --oneline -3
cb23ace test: 型変数の位置の閉包の変異アンカーを lambda_result に直す
4a85a3a test: embed-runtime のビルド先を worktree の target に置く
045b3c9 Merge pull request #81 from jozobeer/cursor/critical-bug-management-13c6
```

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
$ tail -1 target/.gates-full/verify-embed-runtime.log
PASSED: 4/4
$ tail -1 target/.gates-full/verify-pure-closure-in-type-variable-argument-fails-specialization.log
PASSED: 6/6
$ tail -1 target/.gates-full/verify-string-split-linear.log
PASSED: 11/11
```

このラウンドが直した `verify-embed-runtime` は 4/4、`verify-pure-closure-in-type-variable-argument-fails-specialization` は 6/6。`verify-string-split-linear` の床は `timing-floor`（#89）が直し、11/11 になった。
