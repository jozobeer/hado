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

`crates/hado/src/check.rs` の `returned_binding` が、E501 の本文を `body.span.line` と `body.span.col` で組む。この span は §50 の読む順で連結した後のソースの座標である。`crates/hado/src/project.rs` の `source_diagnostic` と `localize` は `Diagnostic` の `line` と `related.line` をファイルの座標に直すが、`message` の中の数字は直さない。同じ形の本文は `check.rs` に他にもある: E502（`identical_branches` の `identical branch results at A and B`。位置が 2 つ）、E507（`branches reconstruct the value at`）、E508（`absence bound at`）、E406（`check_conditions` の `condition is redundant after`）、E403 と E405（`pattern is covered by the arm at`。同じ行なら E405、違えば E403）。どの位置もアンカーと同じ最上位の定義の中にあり、アンカーと同じファイルにある。

`related` は使えない。§56 は関連位置のラベルをコードごとに 1 つ定め（E205・E305・E602）、§58.4 の JSON の `label` はそこから取る。`Diagnostic.related` は 1 つで E502 の 2 位置を持てない。`crates/hado/src/parser.rs` の `Display` は `; <label> at <path>:<line>:<col>` を本文の直後に書き、`project.rs` の `report` はその後に `; canonical form:` を足す。`crates/hado/src/lsp.rs` の `diagnostic_value` は `range`・`code`・`source`・`message` だけを出し、`related` を捨てる。LSP の診断は `Project::check_all` を通るので `source_diagnostic` を経る。

## 案

- 本文が指す位置を `Diagnostic` の構造化した欄（`mentions: Vec<(usize, usize)>` のような行と列の並び）に持ち、本文は位置の穴を持つ形にする。`source_diagnostic` がアンカーと同じ `localize` で `mentions` を直してから本文に埋める。位置はアンカーと同じファイルにあるので、ファイルの選び直しは要らない。既定の 1 行、`--format json` の `message`、LSP の `message` の綴りは今のまま（`at 3:3`）で、値だけがファイルの座標になる。§56 と §58.4 は変えず、`related` も足さない。E502 の 2 位置も並びに入る。推奨。
- 本文の位置を `related`（§56 の関連位置）に移す。§56 にコードごとのラベルを足し、E502 のために `related` を複数にするか anchor を腕へ動かし、`Display` と `report` の連結順（`; <label> at ...; canonical form: ...`）に合わせて README と `docs/getting-started.md` の引用を直し、`lsp.rs` に `relatedInformation` を足す必要がある。綴りと仕様の両方が変わる。退ける。
- `source_diagnostic` が本文の `at L:C` を正規表現で書き換える。綴りに依存し、`L:C` に見える別の数字を誤って書き換えうる。退ける。
- 本文から位置を消す。E502 の「もう一方の腕」のように `end_line` と `end_col` の範囲の外にある位置は失われる。退ける。

## 完了条件

- 上の 3 ファイルの根で `hado check hello.hd` が `bad.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: "hello"` を出し、入口の行数（1 行と 8 行）によらず同じ。`--format json` の `message` も `at 3:3`。単独ファイルの出力は変わらない。
- E502（2 位置）・E507・E508・E406・E403・E405 も、入口以外のファイルで本文の位置がファイルの座標になる。門番は入口の行数を変えて測り、ケース名と行番号を埋め込まない。
- LSP: `scripts/verify-lsp-minimal.sh` に、入口以外のファイルの E501 の `message` が `at 3:3` を含む検査を足す（今は本文を照合していない）。code action の判定は変えない。
- §56・§58.4・`README.md`・`docs/getting-started.md` は変えない（綴りが変わらないため）。拒否コーパスはコードだけを固定しており変えない。

## 由来

`docs/wants.md` に無い。§50（複数ファイルと読む順）、§56（診断のアンカーと関連位置）。PR #28 のレビュー対応で `hado run` を複数ファイルの根に対して走らせて見つけた（2026-09-22）。
