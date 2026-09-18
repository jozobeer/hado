# 原因の参照を行番号から識別子にする（stable-anchors）

開始 2026-09-18。

## 手順書

### 背景

PR #5 の間に、原因の `file:line` が `crates/hado/src/mono.rs` の 3845 → 3962、4407 → 4532、`crates/hado/src/lib.rs` の 231 → 227-229 と 4 回ずれた。検査 8 はパスの実在と行の範囲しか見ないので、ずれは赤にならず、レビューで見つけるしかなかった。パスを省いた `:391` の形も混ざっていた。

### 決定（判断シートで決めた）

- **決定 1: 参照は `` `パス` の `識別子` `` にする。** 行番号は `path:line` も `:line` も書かない。仕様は `§N`（`§26.1` も可）と `付録X`、エラーは `` `E602` `` で指す。新しい記法（タグ）は発明せず、そのファイルで場所を絞れる既存の名前（`fn`・`def`・`const` の名前、`re.compile` の変数名）を使う。
- **決定 2: 門番は 5 つを見る。** 原因に (1) 行番号が無い、(2) バッククォートのパスが実在する、(3) `` `パス` の `識別子` `` の識別子がそのファイルに出現する、(4) `§N`・`付録X` が仕様の見出しに実在する、(5) `` `Exxx` `` が仕様のエラー表に実在する。`List.length` のような member はパスとして読まない（`/` を含むものだけをパスとする）。

### 成果物

- `scripts/lint-docs.sh`: 検査 8 を `doc_issue_causes` のまま書き換え（`file:line` の解析と範囲の検査を削除し、行番号の検出・パスと識別子・節とエラーコードに）
- `docs/issues.md`: 「参照の書き方（2026-09-18）」を追記
- `docs/issues/*.md`: 11 ファイル 22 箇所の原因の参照を移行
- `docs/issues/lint-docs-hardening.md`: 案 B（安定 ID）を閉じ、残件を実測の生成と前方領域のスキーマに絞る

### 判定

- 原因の 22 箇所が行番号を持たず、識別子で場所を指す。赤の探針（`path:line`・`:392`・存在しない節・存在しないコード）が赤になる。
- 門番が実測の誤りを 1 件見つけた。`prelude-tail-recursion.md` の「`stdlib/env.hd` `stdlib/io.hd` の `to_list` 系」は誤りで、`to_list` は `stdlib/array.hd` と `stdlib/bytes.hd` にしか無い（env は `argv_elements`、io は `directory_elements`）。
- `bash scripts/lint-docs.sh` が `PASSED: 10/10`、`git diff --check` が 0、`cargo test --offline` が 0 failed、`bash scripts/run-gates.sh --tier full --jobs 4` が red なし。

### スコープ外

- 実測の生成（`scripts/record-round.sh`）と前方領域のスキーマ（`docs/issues/lint-docs-hardening.md`）。
- 他の文書（`docs/plan.md`・ラウンドファイル）の行番号の参照。検査 8 は issue の原因だけを見る。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （文書運用） | 原因は場所を安定な識別子で指し、行番号を書かない | 文書 | 成功 | `lint-docs.sh` 検査 8 |
| （文書運用） | 仕様は `§N`・`付録X`、エラーは `Exxx` で指す | 文書 | 成功 | `docs/issues.md`（2026-09-18） |

## 実測

```
git diff --check; echo diff-check=$?
diff-check=0
bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 10/10

# 移行の前に門番が見つけた形（赤の探針ではなく実ファイル）
AssertionError: lint-docs-hardening.md: 原因 writes a line number: scripts/lint-docs.sh:420（`パス` の `識別子` で指す）
AssertionError: prelude-tail-recursion.md: 原因 identifier is not in stdlib/io.hd: to_list

# その誤りの実測（to_list は array と bytes にだけある）
grep -c to_list stdlib/array.hd stdlib/bytes.hd stdlib/env.hd stdlib/io.hd
stdlib/array.hd:3
stdlib/bytes.hd:2
stdlib/env.hd:0
stdlib/io.hd:0

# 最終の木での全走
cargo test --offline -> cargo-test-exit=0（0 failed）
bash scripts/run-gates.sh --tier full --jobs 4 -> TALLY: 3751/3751 passed; red: none

# 一次レビュー（Codex）の指摘 4 件を門番と規約に取り込んだ（補修）
# (1) 参照が 1 つも無い原因が通っていた（前の門番の assert found に当たる）
# (2) エラーコードがバッククォート無しだと読まれていなかった
# (3) 識別子を部分文字列で照合していた（`make_is` が `make_isa` に一致）
# (4) 別のパスが続く並びで、識別子が前のパスに結び付いていた
bash scripts/lint-docs.sh -> PASSED: 10/10

# (1) の探針（参照の無い原因を一時ファイルで作る）
AssertionError: zz-probe.md: 原因 points at nothing（`パス` の `識別子`、§N、Exxx のどれかを書く）
# (2)(3)(4) の探針（doc_issue_causes の python ブロックを exec して呼ぶ）
error_code.findall('E602 と `E100` と E3xx と E1000') -> ['E602', 'E100']
mentions('fn make_isa() -> Isa {', 'make_is') -> False
pairs('`crates/hado/src/lib.rs` の `check_ast`、`crates/hado/src/check.rs` の `check_ast`') ->
  [('crates/hado/src/lib.rs', 'check_ast'), ('crates/hado/src/check.rs', 'check_ast')]
```

```
# 二次レビュー（Codex）の指摘 1 件（`§55x`・`付録Cfoo`・`E100foo` が実在の参照として通る）
# 直す前: 誤った参照だけを書いた原因が通る
$ sed -n '/^## 原因/,/^## /p' docs/issues/zz-probe.md
## 原因
§55x。
bash scripts/lint-docs.sh -> PASSED: 10/10
# 直した後: 同じ探針が赤になる
AssertionError: zz-probe.md: 原因 points at nothing（`パス` の `識別子`、§N、Exxx のどれかを書く）
NG: each issue cause points at a path and an identifier
PASSED: 9/10
$ rm docs/issues/zz-probe.md
bash scripts/lint-docs.sh -> PASSED: 10/10
# 単体の探針（doc_issue_causes の python ブロックを exec して呼ぶ）
section.findall('§55x と 付録Cfoo と §55.1') -> ['§55.1']
error_code.findall('E100foo と E602') -> ['E602']
```
