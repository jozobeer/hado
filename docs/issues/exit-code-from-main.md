# `main` から終了コードを返す（exit-code-from-main）
優先度: 中。

## 現象

§54 は終了コードを 0 と 1 の二値に固定する。`raise` が届いても印字が失敗しても 1 で、プログラムが自分で決められない。stderr への書き込みも `log` に無く、`write_file "/dev/stderr"` で代用できることを実測した（2026-09-17）。

## 原因

§54 の決定。`Failure is an effect` が OS の境界で 0/1 に潰れる。

## 案

- `main` の型が `Int` のときだけ、0 以上 255 以下の値を終了コードにする。それ以外の型は今までどおり印字して 0。`Int` の印字は失われるので、印字したい Int は `Int.to_string` で String にする。
- `Int` が範囲外なら `hado: exit code out of range` を stderr に書いて 1 で終わる。i64 から ABI の i32 へ縮める前に判定し、負数や 256 を OS 任せに正規化しない。
- stderr は `Io` に `log_error(contents: String): Void {Error}` を足す。`log` と同じ規則で改行を 1 つ書く。
- 退ける: `exit` 操作。`abort` 種別の効果として書けるが、`main` の値で表せるものを操作にしない。

## 完了条件

- `def main = 0`、`1`、`3`、`255` がその終了コードで終わり、何も印字しない。`def main = raise ...` は 1 のまま。
- `def main = -1`、`256`、`Int` の最小値・最大値が範囲外の診断を stderr に書いて 1 で終わる。生成した実行ファイルと `hado run` の両方で判定する。
- §54 の表と「二値を保つ」の文を改める。

## 由来

`docs/wants.md` 25。§54、§55。
