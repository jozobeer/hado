# テキストを読む自分用のツールを 1 本 hado で書く（dogfood-text-tool）
優先度: 高。

## 現象

受理コーパスの最大は 24 行、`tools/gates/main.hd` は 23 行である。20 行の CLI（stdin の単語頻度）を 1 本書いただけで、string-split-linear、prelude-tail-recursion、char-and-float-conversions、canonical-in-syntax-errors が出た。どれも `docs/wants.md` の一覧には無かったか、優先度が低かった。

## 原因

`docs/wants.md` 53 は「自分用のプログラムを常に 1 本」と言うが、いまある `tools/gates/main.hd:1` と `tools/diagnostics/main.hd:1` は入力が自前の構造化された出力で、任意のテキストを読まない。

## 案

- 候補: エラーコード表（§56）と `tests/reject/` のディレクトリの差分を出すツール。仕様の markdown を読むので split・trim・Char・Map を使う。門番 `verify-*.sh` の一部を置き換えられる。
- `string-split-linear`、`prelude-tail-recursion`、`char-and-float-conversions`、`map-by-builtin-compare`、`canonical-in-syntax-errors` の着地後に始める。前提の issue は各 API と診断の門番だけで閉じ、この統合ツールを完了条件に含めない。

## 完了条件

- `tools/` に 100 行以上のテキスト処理のプログラムがあり、`scripts/run-gates.sh` の中で走る。
- split・trim・Char・Map をこのツールの実行経路で使う。
- 正準形の診断はツールの実行経路では出せない（E100/E101 はコンパイルを止め、ツールに別入力を `hado` へ渡す操作は無い）。`canonical-in-syntax-errors` の入力は、そのラウンドの門番が `hado check --format json` に通して測る。
- ツールはエラーコード表（§56）と `tests/reject/` のディレクトリの差分（欠けと余り）を出し、差分があれば非 0 で終わる。一時ディレクトリで拒否ケースを 1 つ隠したときと、コード表に無い `// error:` を 1 つ足したときに、その差分を印字して非 0 になることを門番で測る。常に成功を返す実装では通らない。
- 足りなかった API と診断が issue になっている。

## 由来

`docs/wants.md` 53。
