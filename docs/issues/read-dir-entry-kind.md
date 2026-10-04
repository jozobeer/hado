# `read_dir` が項目の種別を返す（read-dir-entry-kind）
優先度: 低。

## 現象

`read_dir` は項目の名前だけを返すので、名前から「ディレクトリか」を決められない。`tools/dogfood-text-tool/main.hd` は拒否コーパスの項目名がコードの綴り（`E` と数字 3 つ）であることで代用している。コードの綴りをした普通のファイルがあると、その下の `main.hd` を読みにいって実行時エラーで落ちる。

```
$ rm -rf /tmp/dir-kind && mkdir -p /tmp/dir-kind/reject && cp docs/spec.md /tmp/dir-kind/spec.md && cp -r tests/reject/E100 /tmp/dir-kind/reject/E100
$ printf 'not a directory\n' > /tmp/dir-kind/reject/E999
$ cargo run --offline -q -- run tools/dogfood-text-tool/main.hd -- /tmp/dir-kind/spec.md /tmp/dir-kind/reject
NotADirectory
```

`E999` はコード表に無いので、本来は「余り」として差分に出るべき項目である。

## 原因

`runtime/hado_runtime.c` の `hado_io_read_dir` は `readdir` の `d_name` だけを集める。§55 は `read_dir` の戻りを `String List` と定め、種別の欄を持たない。`tools/dogfood-text-tool/main.hd` の `read_code` は名前の綴りで項目を選ぶしかなく、種別を確かめる手段が無い。`Input` の `read_file` はディレクトリでない項目に対して `NotADirectory` か `NotFound` を返すので、ツールはそこで止まる。

## 案

- 推奨 A: `read_dir` の要素に種別を足す（例: `DirEntry` の record で名前と `FileType` を返す）。呼ぶ側は `Directory` の項目だけを再帰できる。
- 退ける B: `Io.is_dir(path: String): Bool {Error}` を足す。名前ごとに `stat` を呼ぶので、深い木では syscall が項目数だけ増える。
- 退ける C: 現行のまま、名前の綴りと `Error` の捕捉で代用する。コーパスの命名規約が変わると静かに壊れる。

## 完了条件

- 採る案を決め、`docs/spec.md` §55 の `Io` の操作と `read_dir` の規定を追随させる。
- 通常のファイルが混ざったディレクトリを歩く探針を門番に足し、落ちずにその項目を飛ばすか種別として報告することを測る。
- `tests/reject/` を読むツールが、名前の綴りに依存せずに項目を選べることを確かめる。

## 由来

`docs/wants.md` 53。`docs/rounds/dogfood-text-tool.md`。§55。
