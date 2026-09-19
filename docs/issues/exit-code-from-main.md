# `main` から終了コードを返す（exit-code-from-main）
優先度: 中。

## 現象

§54 は終了コードを 0 と 1 の二値に固定する。`raise` が届いても印字が失敗しても 1 で、プログラムが自分で決められない。stderr への書き込みも `log` に無く、`write_file "/dev/stderr"` で代用できることを実測した（2026-09-17）。

```
$ cat main.hd
def main = 1
$ cargo run --offline -q -- run main.hd; echo exit=$?
1
exit=0
$ cat main.hd
def main = write_file "/dev/stderr" (Bytes.of_string "x\n")
$ cargo run --offline -q -- run main.hd 2>err.txt; echo exit=$?; cat err.txt
exit=0
x
```

## 原因

§54 の決定。`docs/spec.md` が終了コードを 0 と 1 の二値に保つと定め、`runtime/hado_runtime.c` の `hado_abort_error` が `exit(1)` で `Failure is an effect` を OS の境界で 0/1 に潰す。

## 案

- `main` の型が `Int` のときだけ、0 以上 255 以下の値を終了コードにする。それ以外の型は今までどおり印字して 0。`Int` の印字は失われるので、印字したい Int は `Int.to_string` で String にする。
- `Int` が範囲外なら `hado: exit code out of range` を stderr に書いて 1 で終わる。i64 から ABI の i32 へ縮める前に判定し、負数や 256 を OS 任せに正規化しない。
- stderr は `Io` に `log_error(contents: String): Void {Error}` を足す。`log` と同じ規則で改行を 1 つ書き、失敗は `log` と同じく `message` を `"IoFailure"` とする `Error` にする。
- 退ける: `exit` 操作。`abort` 種別の効果として書けるが、`main` の値で表せるものを操作にしない。

## 完了条件

- `def main = 0`、`1`、`3`、`255` がその終了コードで終わり、何も印字しない。`def main = raise ...` は 1 のまま。
- `def main = -1`、`256`、`9223372036854775807`、`(-9223372036854775807 - 1)` が範囲外の診断を stderr に書いて 1 で終わる。i64 の最小値はリテラルに書けない（E112）ので引き算で作る。生成した実行ファイルと `hado run` の両方で判定する。
- `log_error "x"` を呼ぶ `def main` を build と `hado run` で走らせ、stdout は空、stderr は `x` と改行 1 つだけになる。`log` は stdout、`log_error` は stderr に書く対照を同じ門番に置く。
- stderr の読み手が先に終わって書き込みが失敗する状態で `log_error` を呼ぶと、`handler` が `message` を `"IoFailure"` とする `Error` を捕まえる。捕まえなければ 1 で終わる。§55 の失敗の表に `log_error` の行を足す。
- `tests/accept/` の入口 `main` が `Int` を返し `// expect:` が数値のケース（`tests/accept/once_loop/main.hd:2` の `10000000` など）を棚卸しする。印字の期待を保つため `Int.to_string` で包むなどして移行し、移行の前後で `scripts/verify-m1-corpus.sh` の項目被覆が変わらないことを確かめる。`tests/corpus.sha256` はこのラウンドの成果物として作り直す。
- 実行する `main` が `Int` を印字に使う非コーパスの門番とテストも同じ棚卸しに含める（`crates/hado/tests/walk.rs:15`、`scripts/verify-r29.sh:108`、`scripts/verify-r43.sh:245` など）。`Int.to_string` で包むなどして移行し、`cargo test` と各 `verify-*.sh` が移行の前後で同じ被覆を測ることを確かめる。
- `scripts/verify-r54.sh:58` の `a_test.hd`・`b_test.hd`・`c_test.hd`（`def main = 11`・`22`・`33`）は、§54 の変更後はゲストの終了コード 11・22・33 になるので、そのままでは `hado test` が失敗する。3 本とも `Int.to_string` に包んで緑にするのではなく、`2` 以上 `255` 以下のゲスト終了コード `n` を返す `*_test.hd` を 1 本置く（例: `def main = 11`）。`hado test` の標準出力に §58.2 の `test: <相対パス> failed (exit <n>)` が同じ `n` で出ること、パス順でその後の `*_test.hd` が走って `ok` になること、集約の終了コードが 1 であることを直接測る。`raise` や `def main = 1` で `failed (exit 1)` になる既存の経路（`scripts/verify-r54.sh:90`）や、非零をすべて `failed (exit 1)` に潰す実装では通らない。
- `Io` に `log_error` を足すと既存の網羅的な `handler Io` は腕が 1 つ足りなくなり E605 になる。`scripts/verify-r37.sh:205` の `io_arms`（5 操作）と `scripts/verify-r43.sh:380` の固定 5 腕を `log_error` 込みに移行し、腕を欠かすと E605 になる検査（`scripts/verify-r37.sh:211`）と網羅の被覆を弱めずに保つ。腕を減らして緑にしない。
- §54 の表と「二値を保つ」の文を改める。

## 由来

`docs/wants.md` 25。§54、§55。
