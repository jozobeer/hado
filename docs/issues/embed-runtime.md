# cargo install した hado がビルド先の削除で build / run できなくなる（embed-runtime）
優先度: 中。

## 現象

`cargo install --offline --path crates/hado` で入れた `hado` は、`check` はどこに置いても動くが、`build` と `run` はビルド時の `target/` の中の静的ライブラリをリンクする。ビルド先を退避すると `cc` が落ちる（2026-09-21、worktree `readme-users` の `target/release/build/hado-24b1034e11a61b66/out` を退避して実測。退避後に戻した）。

```
$ cargo install --offline --path crates/hado --root /tmp/hado-install
   Installed package `hado v0.1.0 (/home/zyun/git/local/hado/.worktrees/readme-users/crates/hado)` (executable `hado`)
$ cd /tmp/elsewhere
$ /tmp/hado-install/bin/hado run app.hd
ok
$ mv /home/zyun/git/local/hado/.worktrees/readme-users/target/release/build/hado-24b1034e11a61b66/out /home/zyun/git/local/hado/.worktrees/readme-users/target/release/build/hado-24b1034e11a61b66/out.bak
$ /tmp/hado-install/bin/hado check --print-type app.hd
String {Error}
$ /tmp/hado-install/bin/hado run app.hd
/usr/bin/ld: cannot find /home/zyun/git/local/hado/.worktrees/readme-users/target/release/build/hado-24b1034e11a61b66/out/libhado_runtime.a: No such file or directory
/usr/bin/ld: cannot find /home/zyun/git/local/hado/.worktrees/readme-users/target/release/build/hado-24b1034e11a61b66/out/libhado_float_format.a: No such file or directory
collect2: error: ld returned 1 exit status
```

`app.hd` は `tests/accept/create_user/main.hd` のコピーである。`cargo clean` や checkout の削除でも同じ状態になる。

## 原因

`crates/hado/src/lib.rs` の `runtime_lib_path` と `link_executable` が、コンパイル時の `env!("OUT_DIR")` で作ったパスで `libhado_runtime.a` と `libhado_float_format.a` を `cc` に渡す。prelude は同じファイルの `with_prelude` が `include_str!` で埋め込むので `check` は動く。`cargo install --path` はそのパッケージの `target/` でビルドするので、焼き込まれるのは checkout の `target/release/build/hado-<hash>/out` である。

## 案

- 2 本の `.a` を `include_bytes!` で `hado` に埋め込み、`link_executable` がリンクの直前に object と同じディレクトリへ書き出して `cc` に渡し、リンク後に消す。呼び出し元（`crates/hado/src/build.rs` の `executable_with` と各テスト）は変えない。`hado` は release で約 7.5 MB 大きくなる（`libhado_float_format.a` が Rust の staticlib で std を含む）。推奨。
- 実行時に `hado` の隣（`../lib/hado/`）や環境変数からアーカイブを探す。`cargo install` は実行ファイルしか置かないので、利用者が手で置く手順が要る。退ける。
- `build` / `run` を `cargo run` 経由に限定し、install を非対応と文書に書く。現状の追認で、利用者の問いに答えない。退ける。

## 完了条件

- 一時の `--target-dir` でビルドして `--root` へ install し、その `--target-dir` を消した後、別のディレクトリにコピーした受理コーパスの各ケースで `hado run main.hd` の観測と `hado build` の生成物の観測が `// expect:` / `// expect-error:`（`docs/corpus.md`）と一致する（終了コード・stdout・stderr）。`scripts/verify-embed-runtime.sh`（full）が測る。ケース数は判定器がディスクから数え、0 件と全件の期待値が空の判定は赤。
- `cargo test --offline` が 0 failed（`link_executable` を通るテスト群）。
- `tests/` `stdlib/` と corpus lock は変えない。

## 由来

`docs/wants.md` に無い。§58（`build` と `run` の出力先）。README の `cargo run` 経由の前提を外せるかという問いから（2026-09-21）。
