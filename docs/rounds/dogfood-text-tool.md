# エラーコード表と拒否コーパスの差分を出すテキスト処理ツールを hado で書く（dogfood-text-tool）

## 手順書

### 背景

issue の現象を main（`eeb1704`）で再現し直した。受理コーパスの最大は 28 行、`tools/` の最大は `tools/diagnostics/main.hd` の 15 行、`tools/gates/main.hd` は 7 行である（issue 本文の 23 行・30 行は古い）。100 行のテキスト処理のプログラムは無い。

```
$ wc -l tools/*/main.hd
  15 tools/diagnostics/main.hd
   7 tools/gates/main.hd
  22 total
$ find tests/accept -name main.hd -exec wc -l {} + | sort -rn | grep -v total | head -2
   28 tests/accept/shared_functor/main.hd
   21 tests/accept/user_store/main.hd
```

前提（`string-split-linear`・`char-and-float-conversions`・`map-by-builtin-compare`・`postfix-binding`・`do-option-absent`・`exit-code-from-main`）は着地済み。`tools/wordfreq/` の 3 本（10・8・7 行）は後置のテキスト処理の実例である。

### 意図

- 使う人: hado の作者と、拒否コーパスやエラーコード表を足す builder（推定）。
- 場面: §56 の表に行を足したとき、`tests/reject/` にケースを足したとき、片方だけを直して着地させそうなとき。
- 理由: 表とコーパスの 1 対 1 は `scripts/verify-m1-corpus.sh` が Python で測っている。同じ突き合わせを hado で書けば、仕様の markdown とコーパスを読むテキスト処理の実例になり、門番の一部を置き換えられる（推定）。

### 仮説（一周目のモデル化）

- 境界: `tools/dogfood-text-tool/main.hd` 1 本。入力は引数で受ける（第1引数 = 仕様の markdown、第2引数 = 拒否コーパスの根）。既定は `docs/spec.md` と `tests/reject`。読み取り専用で木を書かない。出力は `IoDefault.log` の行と `main` の `Int`（§54）。
- 型: コードは `E` と数字 3 つの 4 文字。表とコーパスを `Map String Int`（§55 の `Map`）に数え、`Map.find` の有無で欠け・余り・重複を出す。`String.split`・`String.trim` は行とセルの切り出し、`Char` は字句の検めに使う。
- データフロー: `IoDefault.read_file` → `Bytes.to_string` → `String.split "\n"` → §56 の節だけを走査して表のセルを取る。`IoDefault.read_dir` → 各 `main.hd` を読んで `// error:` の行を取る。両者を `Map` に数えて差分を出し、差分の有無を終了コードにする。
- 未確認（着手時に決めていなかった点）:
  - 効果を持つ字句の述語を条件に置けない（§36 の E308）ので、`as` の束縛で効果を条件の外へ出す書き方になる。
  - `List.fold` に `Map` を累算する閉包を渡したとき、`Map.fold` と同じ E706 にならないか。
  - `hado run <file>` は同じディレクトリの `.hd` を連結するので、変異の写しは 1 本だけのディレクトリに置く必要がある。

### 判定器（期待値の根拠）

issue の完了条件をそのまま検査にする。件数は実ファイルと入力から導き、コーパスのファイル名や行を判定器へ書き込まない。

1. 100 行以上 → 門番 a（`find tools/dogfood-text-tool -name '*.hd' -exec cat {} + | wc -l`）。
2. `run-gates.sh` の中で走る → `# tier: core` と `scripts/gate-tiers.lock` の行。
3. split・trim・Char・Map を実行経路で使う → 門番 f（コメントを除いたソースに 4 つが現れる）と g（4 つを 1 箇所ずつ壊すと実木の走行が変わる）。
4. 差分を出して非 0 → 門番 b（一致で 0）、c（拒否ケースを隠すと欠け）、d（表に無い `// error:` を足すと余り）、e（同じタグを 2 つにすると重複）。
5. 足りなかった API が issue → `docs/issues/string-get-option.md`・`docs/issues/read-dir-entry-kind.md`。

### 成果物

- `tools/dogfood-text-tool/main.hd` — ツール本体（114 行）。
- `scripts/verify-dogfood-text-tool.sh` — 門番（`# tier: core`、`# round: dogfood-text-tool`）。
- `scripts/gate-tiers.lock` — 層の錠。
- `docs/issues/string-get-option.md`・`docs/issues/read-dir-entry-kind.md` — 書く途中で当たった不足。
- `docs/rounds/dogfood-text-tool.md`。

### 工程

1. 門番を先に書き、ツールを置く前（一時的に退避）に赤を測る。
2. ツールを後置で実装し、実木で差分なしを測る。
3. `print-postfix` の正準形へ揃え、入力の変異と道具の変異で赤を測る。
4. 層に載せ、`scripts/gate-tiers-lock.sh` で錠を作り直す。
5. main を取り込み、corpus lock を作り直し、層別を回す。

### 判定

- `bash scripts/verify-dogfood-text-tool.sh` が `PASSED: 7/7`。
- `cargo test --offline` が赤なし。
- `bash scripts/run-gates.sh --tier quick`・`--tier core`・`--tier full` が赤なし（既知の赤を除く）。
- `bash scripts/lint-docs.sh` と `bash scripts/verify-spec-contracts.sh` が赤なし。

### 許容される赤

無し。`verify-string-split-linear` の検査 i は main でも揺れる既知の赤（`docs/issues/flaky-string-split-timing-gate.md`）で、このラウンドの成果物ではない。

### スコープ外

- 処理系（`crates/hado`・`runtime/`）の変更。字句の API の不足は issue に切り出した。
- `tests/reject/` と `docs/spec.md` の変更。ツールは読むだけ。
- `scripts/verify-m1-corpus.sh` の置き換え。突き合わせが重なるだけで、置き換えは別ラウンドにする。

### 中断の条件

- 仕様の解釈に疑義が出た（仕様を変えず報告する）。
- 完了条件の判定が 2 回続けて赤になった。
- `List.fold` に `Map` を累算する閉包が E706 になり、回避できないと分かったとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §56 | 拒否コーパスは表のコードと 1 対 1、表の行数がコーパスの本数 | 名前解決 | — | `scripts/verify-dogfood-text-tool.sh` 検査 b〜e |
| §55 | コードは `E` と数字 3 つ（表の「コード」列）、`String.split`・`trim`・`Char`・`Map` の型 | 型 | — | 検査 f・g |
| §54 | `Int` の `main` は印字せず終了コードになる（0〜255） | 実行 | — | 検査 b〜e の終了コード |
| §19 | String は不変の UTF-8、索引はスカラ値の単位 | 実行 | — | 検査 f・g |
| §36 | 効果は条件に置けない（E308） | 型 | E308 | 検査 g の Char 変異 |

## 実測

### 赤（門番を先に書き、ツールを置く前）

`tools/dogfood-text-tool` を一時退避して門番を回した。行数も入力の走査も成立しないので 7 検査すべてが落ちる。

```
$ mv tools/dogfood-text-tool /tmp/dogfood-tool-hidden
$ bash scripts/verify-dogfood-text-tool.sh
NG: a lines (0 < 100)
NG: b clean (rc=1 out=tools/dogfood-text-tool:1:1: error: No such file or directory (os error 2) )
NG: c hidden (rc=1 want=missing:E100 out=tools/dogfood-text-tool:1:1: error: No such file or directory (os error 2) )
NG: d extra (rc=1 want=extra:E990 out=tools/dogfood-text-tool:1:1: error: No such file or directory (os error 2) )
NG: e duplicate (rc=1 want=duplicate:E100 2 out=tools/dogfood-text-tool:1:1: error: No such file or directory (os error 2) )
NG: f api (not found: String.split String.trim Char. Map.)
g split: not caught (rc=1)
g trim: not caught (rc=1)
g char: not caught (rc=1)
g map: not caught (rc=1)
NG: g mutations (0/4 caught)
PASSED: 0/7
$ mv /tmp/dogfood-tool-hidden tools/dogfood-text-tool
```

### 緑（このラウンドの成果物）

```
$ bash scripts/verify-dogfood-text-tool.sh
OK: a lines: the tool is 114 lines (>= 100)
OK: b clean: the code table and tests/reject agree (dogfood-text-tool: spec=68 reject=68 missing=0 extra=0 duplicate=0)
OK: c hidden: removing E100 is reported as missing
OK: d extra: adding E990 is reported as extra
OK: e duplicate: two cases with E100 are reported as a duplicate
OK: f api: split, trim, Char and Map appear in the tool source
g split: caught
g trim: caught
g char: caught
g map: caught
OK: g mutations: split, trim, Char and Map are on the execution path (cases=4)
PASSED: 7/7
```

### ツールが使った API

実行経路で使った prelude の member。

- `String.split`（`lines_of`・`cell_at`）と `String.trim`（`token_of`）— 仕様の markdown の行と `|` のセル、拒否コーパスの `// error:` の行を切り出す。
- `String.get` と `Char.to_int`（`char_at`・`is_digit_char`）— セルとタグの字句が `E` と数字 3 つかを検める。
- `Map.empty`・`Map.insert`・`Map.find`・`Map.to_list`（`bump_code`・`count_codes`・`map_codes`・`map_size`・`missing_codes`・`extra_codes`・`duplicates`）— 表とコーパスをコードごとに数え、`Map.find` の有無で欠けと余りを、値を 1 より大きいかで重複を出す。`Map.fold` は抽象型を含むので使わず、`Map.to_list` を `List.fold`/`List.filter` で畳む（`docs/issues/contract-member-effect-generalization.md` の既知の E706 を避ける）。
- `IoDefault.read_file`・`IoDefault.read_dir`・`IoDefault.log`・`args`・`Bytes.to_string`・`Option.unwrap_or`・`Option.is_some`・`List.fold`・`List.map`・`List.filter`・`List.nth`・`List.append`・`List.length`・`Int.to_string`・`String.length`・`String.starts_with`・`String.concat`。

### 書く途中で当たった不足（issue に切り出した）

- `String.get` が範囲外を `{Error}` で返すので、`Char` を見る述語が効果を持ち、条件に置くと E308 になる。`as` の束縛を積んで条件の外へ出した（`is_digit_char`・`has_code_chars`）。`List.nth` は同じ索引の不在を `Option` で返すので、規則が 2 つある。→ `docs/issues/string-get-option.md`。
- `IoDefault.read_dir` が名前だけを返すので、拒否コーパスの項目がディレクトリかを名前の綴り（`E` と数字 3 つ）で代用している。コードの綴りをした普通のファイルがあると `NotADirectory` で落ちる。→ `docs/issues/read-dir-entry-kind.md`。

```
$ rm -rf /tmp/dir-kind && mkdir -p /tmp/dir-kind/reject && cp docs/spec.md /tmp/dir-kind/spec.md && cp -r tests/reject/E100 /tmp/dir-kind/reject/E100
$ printf 'not a directory\n' > /tmp/dir-kind/reject/E999
$ cargo run --offline -q -- run tools/dogfood-text-tool/main.hd -- /tmp/dir-kind/spec.md /tmp/dir-kind/reject
NotADirectory
```

### 入る前に読んだ仕様と、実装から分かったこと

- 当たり: §56 の表は `| コード | 短い名前 | ...` の行形式なので、`String.split "|"` の第 2 セルと `String.trim` でコードが取れる。見出し行と区切り行は `E` と数字 3 つの字句で落ちる。表 68 行・拒否 68 件が一致した。
- 当たり: `List.fold` に `Map` を累算する閉包を渡しても E706 にならない（`Map.fold` と違い `List.fold` の署名は型変数だけ）。効果を持つ読み取りは `String List` の累算器に分け、`Map` の集計は純粋な fold にした。
- 外れ: `String.split text ""` は `EmptySeparator` なので文字単位の分解には使えない。`String.get` を字句の検めに使うことになり、E308 を避けるための `as` 束縛が要った。
- 実装から還元した点: 門番の変異の写しは、同じディレクトリに `.hd` が 1 本だけになるよう専用ディレクトリへ置く。`hado run` はディレクトリの `.hd` を連結するので、まとめて置くと E205 で落ちる。
