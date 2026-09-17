# テキストを読む自分用のツールを 1 本 hado で書く（dogfood-text-tool）
優先度: 高。

## 現象

受理コーパスの最大は 24 行、`tools/gates/main.hd` は 23 行である。20 行の CLI（stdin の単語頻度）を 1 本書いただけで、string-split-linear、prelude-tail-recursion、char-and-float-conversions、canonical-in-syntax-errors が出た。どれも `docs/wants.md` の一覧には無かったか、優先度が低かった。

## 原因

`docs/wants.md` 53 は「自分用のプログラムを常に 1 本」と言うが、いまある 2 本は入力が自前の構造化された出力で、任意のテキストを読まない。

## 案

- 候補: エラーコード表（§56）と `tests/reject/` のディレクトリの差分を出すツール。仕様の markdown を読むので split・trim・Char・Map を使う。門番 `verify-*.sh` の一部を置き換えられる。
- `string-split-linear`、`prelude-tail-recursion`、`char-and-float-conversions`、`map-by-builtin-compare`、`canonical-in-syntax-errors` の着地後に始める。前提の issue は各 API と診断の門番だけで閉じ、この統合ツールを完了条件に含めない。

## 完了条件

- `tools/` に 100 行以上のテキスト処理のプログラムがあり、`scripts/run-gates.sh` の中で走る。
- split・trim・Char・Map と正準形診断をこのツールの実行経路で使う。
- 足りなかった API と診断が issue になっている。

## 由来

`docs/wants.md` 53。
