# 複数ファイルの根で診断本文の位置が連結後の行番号になる（localize-message-positions）
優先度: 中。

## 現象

入口以外のファイルに出た E501 の本文の位置 `at N:3` が、入口ファイルの行数に応じてずれる。先頭のアンカー `bad.hd:2:3` は正しい（2026-09-22、PR #27 着地後の main `a3a2a25` を `cargo install --locked --offline --path crates/hado` した `hado` で実測）。`hello.hd` は 8 行の入口（`docs/getting-started.md` の「失敗のレール」）、`bad.hd` は 3 行。

```
$ wc -l hello.hd bad.hd
  8 hello.hd
  3 bad.hd
$ cat bad.hd
def main =
  let x = "hello" ->
  x
$ hado check hello.hd
bad.hd:2:3: error[E501]: binding x is returned unchanged at 13:3; canonical form: "hello"
$ hado check --format json hello.hd
{"path": "bad.hd", "line": 2, "col": 3, "end_line": 3, "end_col": 4, "code": "E501", "message": "binding x is returned unchanged at 13:3", "canonical": "\"hello\"", "related": null}
$ printf 'def main = "hi"\n' > hello.hd
$ hado check hello.hd
bad.hd:2:3: error[E501]: binding x is returned unchanged at 6:3; canonical form: "hello"
$ mkdir solo && mv bad.hd solo/ && cd solo && hado check bad.hd
bad.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: "hello"
```

入口が 8 行なら 13、1 行なら 6 で、差は入口の行数と区切りの 2 行。単独では 3:3 で正しい。

## 原因

`crates/hado/src/check.rs` の `returned_binding` が、E501 の本文を `body.span.line` と `body.span.col` で組む。この span は §50 の読む順で連結した後のソースの座標である。`crates/hado/src/project.rs` の `source_diagnostic` と `localize` は `Diagnostic` の `line` と `related.line` をファイルの座標に直すが、`message` の中の数字は直さない。同じ形の本文は `check.rs` に他にもある: E502（`identical branch results at`）、E508（`absence bound at`）、E507（`branches reconstruct the value at`）、網羅性の E403 と E405（`pattern is covered by the arm at`。同じ行なら E405、違えば E403）。`crates/hado/src/parser.rs` の `Diagnostic` の `Display` は `related` を `; <label> at <path>:<line>:<col>` で印字でき、そちらは `localize` を通る。

## 案

- 本文の位置を `related`（§56 の関連位置）に移し、ラベル（`returned`、`bound`、`reconstructed`、`covered by`）を `related_label` に足す。`localize` が `related` を直すので、既定の 1 行も `--format json` の `related` もファイルの座標になる。既定の 1 行の綴りは `binding x is returned unchanged; canonical form: "hello"; returned at bad.hd:3:3` のように変わるので、本文を引用する `README.md` の拒否の例と `docs/getting-started.md` の冗長の例を同じ変更で直す。推奨。
- `source_diagnostic` が本文の `at L:C` を正規表現で書き換える。綴りに依存し、`L:C` に見える別の数字を誤って書き換えうる。退ける。
- 本文から位置を消す。`--fix` の書き換え範囲は `end_line` と `end_col` が持つが、E502 の「もう一方の腕」のように範囲の外の位置は失われる。退ける。

## 完了条件

- 上の 3 ファイルの根で、E501 の本文または `related` が `bad.hd` の 3 行 3 列を指し、入口の行数（1 行と 8 行）によらず同じ。`--format json` の `related` は `null` でない。
- E502・E507・E508・E403・E405 も、入口以外のファイルで同じくファイルの座標になる。門番は入口の行数を変えて測り、ケース名と行番号を埋め込まない。
- 単独ファイルの出力は位置の値を変えない。拒否コーパス（コードだけを固定）と `scripts/verify-lsp-minimal.sh` の code action の判定は変えない。本文の綴りを固定している判定器があれば同じ変更で直す（`scripts/lsp-check.py` が本文を照合するかは未確認）。
- `README.md` の拒否の例と `docs/getting-started.md` の冗長の例が新しい綴りと一致する。

## 由来

`docs/wants.md` に無い。§50（複数ファイルと読む順）、§56（診断のアンカーと関連位置）。PR #28 のレビュー対応で `hado run` を複数ファイルの根に対して走らせて見つけた（2026-09-22）。
