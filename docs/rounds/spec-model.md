# 仕様の機械可読な部分を spec/ に出す（spec-model）

## 手順書

### 背景

`docs/spec.md` は 2568 行・本文 71 節で、規範内容を散文と表で持つ。うち機械が読む部分（§56 のエラーコード表、付録C の文法）は Markdown の中にしか無く、読み手ごとに正規表現で読み直していた。

同じ事実が 4 か所に写されている。

| 事実 | 置き場 | 個数 |
|---|---|---|
| エラーコード | `docs/spec.md` §56 の表 / `crates/hado/src/*.rs` の `"Exxx"` / corpus の `// spec:` / `docs/spec-trace.md` の行 | 70 行 / 69 個 / 182 / — |
| 文法 | `docs/spec.md` 付録C の散文 EBNF | 85 生成規則（10 フェンス・126 行） |

正本が無いので、「仕様は X と言っている」を針（文字列の実在）でしか表せない。`docs/plan.md` 33 節が 2026-09-14 に「spec.md の文章固定検査を棚卸しする」と決めていたのがこの問題である。`scripts/lint-docs.sh` は 1000 行・検査ごとに専用の正規表現で Markdown を再パースしており、PR #31 の審査で検査 1・2・5・11・14（2 回）・15・16 に指摘が付いた。

### 決定

エラーコード表と文法を `spec/` に出す。`docs/spec.md` の §56 の表と付録C のフェンスは**生成物**にし、`scripts/spec-lock.sh --check` がバイト一致で照合する（`scripts/corpus-lock.sh` と同じ形）。散文（節の導入・フェンスの間の説明・見出し）は `docs/spec.md` に残し、生成器は触らない。

これは `AGENTS.md` の「真偽が要る実測は手で書かせず、走らせた側に生成させる」を、実測ではなく仕様の機械可読部分へ広げたものである。

### 成果物

- `spec/errors.toml` — §56 の表の正本。`code` / `name` / `section` / `stage` / `anchor` の 5 列 × 70 件。
- `spec/grammar.ebnf` — 付録C の文法の正本。`### <ラベル>` ごとに 1 フェンス、10 群・126 行。
- `scripts/spec-lock.sh [--check]` — 生成と照合。
- `scripts/verify-spec-model.sh` — 門番（`# tier: quick`、`# round: spec-model`）。検査 2 本。

### 工程

1. §56 の表を解析し、`spec/errors.toml` を作る。
2. 付録C の 10 フェンスを解析し、`spec/grammar.ebnf` を作る。
3. `scripts/spec-lock.sh` を書き、往復がバイト一致することを確かめる。
4. `scripts/verify-spec-model.sh` を書く。
5. 変異で赤を確かめる。
6. 層別を回し、実測を書く。

### 判定

- `bash scripts/spec-lock.sh --check` が一致を出す。
- `bash scripts/verify-spec-model.sh` が `PASSED: 2/2`。
- `bash scripts/lint-docs.sh` が赤なし。
- `bash scripts/run-gates.sh --tier quick` が red なし。

### 期待値の根拠

- 表の往復は、生成前の `docs/spec.md` と生成後を比べて**差分が正規化の 5 行だけ**であることで確かめる。付録C のフェンスに差分が出なければ、文法の抽出がバイト一致だったことになる。
- 門番の赤は変異で測る。表を手で編集する・モデルのコードを書き換える・節を実在しないものにする・コードを重複させる・文法のラベルを変える。

### 許容される赤

無し。

### スコープ外

- 追跡表（`docs/spec-trace.md`・`docs/spec-coverage.md`）をデータにする第 2 段。消費者は人間だけでなく `scripts/lint-docs.sh`（検査 3・12・14）と `scripts/verify-m1-corpus.sh` なので、同じ形で移せる。本ラウンドでは扱わない。
- 字句（キーワード・`op_kind`・型名）のモデル化。
- `crates/hado/src/*.rs` の `"Exxx"` リテラルをモデルへ寄せること。エラー表とコードの対応は本ラウンドの後に別途測る。
- `scripts/lint-docs.sh` の分割（第 3 段）。

### 中断の条件

- §56 の表の往復で、正規化の 5 行以外の差分が出たとき。
- 付録C のフェンスの往復で差分が出たとき（文法の抽出がバイト一致でない）。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 仕様のエラーコード表と文法は `spec/` が正本で、`docs/spec.md` の表とフェンスは生成物である | 道具 | 成功 | `scripts/verify-spec-model.sh`（a・b）、`scripts/spec-lock.sh --check` |

## 実測

### 表の往復（正規化の 5 行だけが変わる）

```
$ bash scripts/spec-lock.sh --check 2>&1 | tail -4
-| E100 | syntax-error | 付録C | 構文 | 文法に合わない最初のトークン。付録Cの腕 0 本の列は先頭の `|` |
+| E100 | syntax-error | 付録C | 構文 | 文法に合わない最初のトークン。付録Cの腕 0 本の列は先頭の `\|` |
spec-lock --check: docs/spec.md が spec/ と食い違う
$ bash scripts/spec-lock.sh
spec-lock: 70 エラーコード、10 文法群
$ git diff --stat docs/spec.md
 docs/spec.md | 10 +++++-----
 1 file changed, 5 insertions(+), 5 deletions(-)
$ bash scripts/spec-lock.sh --check
spec-lock --check: 一致（70 エラーコード、10 文法群）
```

差分は 5 行で、いずれも表のセルの中の `` `|` `` を `` `\|` `` に逃がすものである。GFM の表はセルの `|` を（コードスパンの中でも）逃がすことを求めるので、元の表は E100・E403・E404・E405・E508 の 5 行が壊れていた。付録C のフェンスには差分が出ず、文法の抽出がバイト一致だったことを示す。

### 門番

```
$ bash scripts/verify-spec-model.sh
spec-lock --check: 一致（70 エラーコード、10 文法群）
OK: docs/spec.md matches spec/ (spec-lock --check)
70 エラーコード、10 文法群、7 段階の族、見出し 69
OK: the model is well formed and its sections exist
PASSED: 2/2
```

### 変異

表を手で編集する（モデルと食い違う）、モデルのコードを書き換える、節を実在しない `§99` にする、コードを重複させる、文法のラベルを変える、の 5 種で赤を確かめた。

```
$ sed -i 's/| E100 | syntax-error |/| E100 | syntax-err |/' docs/spec.md; bash scripts/verify-spec-model.sh | tail -1
PASSED: 1/2
$ sed -i 's/^section = "§26"/section = "§99"/' spec/errors.toml; bash scripts/spec-lock.sh >/dev/null; bash scripts/verify-spec-model.sh | tail -1
PASSED: 1/2
$ sed -i 's/### パターン:/### パターンX:/' spec/grammar.ebnf; bash scripts/verify-spec-model.sh | tail -1
PASSED: 1/2
```

節と重複の変異は表も作り直してから測った（`lock` が緑のまま `model` だけが赤になる）。2 本の検査が独立に効いていることを示す。

### 層別

```
$ bash scripts/run-gates.sh --tier quick --jobs 4 target/quick-gates | tail -1
TALLY: 40/40 passed; red: none
$ bash scripts/lint-docs.sh | tail -1
PASSED: 10/10
```

```
$ bash scripts/run-gates.sh --tier core --jobs 4 target/core-gates | tail -1
TALLY: 604/604 passed; red: none
```

```
$ bash scripts/run-gates.sh --tier full --jobs 4 target/full-gates | tail -1
TALLY: 3790/3790 passed; red: none
```

quick 層は 38 → 40、core 層は 602 → 604、全走は 3788 → 3790（新しい門番の 2 検査）。
門番は 83 → 84 本。

### 審査の指摘と対応（3 回目）

Codex 審査で 8 件（P2）が付いた。いずれも自分が入れた検査と生成器の穴なので直した。

| 指摘 | 直す前 | 直した後 |
|---|---|---|
| 短い名前の一意性を見ていない | 別コードで名前を重複させると緑 | `duplicate-name` で赤 |
| 空行だけの文法群を本文在りと数える | 群を空にしても緑 | 非空行を要る |
| 段階を族と突き合わせていない | E100 を `型` にしても緑 | `spec/stages.toml` の族ごとの段階と照合 |
| 空白だけのアンカーを通す | `anchor = "   "` で緑 | `field()` が空で落ちる |
| 改行を含むフィールドが表を壊す | 生成が表を壊し、以後 `§56 の表が連続していない` で復旧不能 | 改行を拒む |
| 本文の `#` 行を黙って捨てる | 正本の行が spec.md に出ない | 本文として保つ（注記は先頭の `### ` より前だけ） |
| 閉じフェンスを前方一致で見る | `\`\`\`text` の閉じを閉じと見なす | 開きと閉じを厳密に区別 |
| `tomllib` は 3.11 以上 | Ubuntu 22.04 の python3.10 で全層が止まる | `tomli` へ退避し、無ければ理由を出す。`AGENTS.md` の環境に 3.11 以上（または `tomli`）と書く |

段階の検査は当初「7 種の一覧を門番に書くと暗記になる」として見送っていた。族ごとの対応を
`spec/stages.toml` にデータとして置けば暗記にならないので、モデル側に足して検査する。

```
$ bash scripts/verify-spec-model.sh | tail -1
PASSED: 2/2
$ bash scripts/run-gates.sh --tier core --jobs 3 target/core-gates | tail -1
TALLY: 641/641 passed; red: none
```

変異は 8 件とも赤（`#` 行の保持だけは、保たれることを生成物で確かめた）。`tomllib` の退避は
`tomllib` を失敗させる module を `PYTHONPATH` に置いて、両方の道具が理由を出して落ちることを
確かめた。

### 審査の指摘と対応（4 回目）

Codex 審査で 2 件（P2）が付いた。どちらも直した。

- **`spec/stages.toml` の族の重複（`scripts/verify-spec-model.sh`）**: 辞書に畳む前に
  重複を弾くようにした。同じ族を 2 度書くと辞書が黙って後を残すので、写し間違いが
  見えなかった。変異（E1 の記録を 2 つにする）で `PASSED: 1/2` を確かめた。
- **実測ブロックの出力が古い（`docs/rounds/spec-model.md`）**: 3 回目で段階の検査を足した
  ので、門番の出力に `7 段階の族` が入る。記録を実際の出力に直した。

### 審査の指摘と対応（5 回目）

Codex 審査で 3 件（P2）が付いた。いずれも自分が入れた検査の穴なので直した。

- **文法の本文のフェンス行（`scripts/spec-lock.sh`）**: 本文にフェンス行があると、生成した
  `docs/spec.md` を次の走行が構造として読み、正本を直しても復旧できなくなる。組む前に
  弾く。変異で `spec-lock: spec/grammar.ebnf の 字句: の本文にフェンス行 '\`\`\`'` を確かめた。
- **`spec/stages.toml` の `stages` の形（`scripts/verify-spec-model.sh`）**: 文字列にすると
  `in` が部分文字列の照合になり、族の取り違えが素通りする。空でない文字列の並びであることを
  見る。変異（`stages = "字句構文"`）で赤を確かめた。
- **`-O` で `assert` が消える（`scripts/verify-spec-model.sh`）**: `PYTHONOPTIMIZE=1` で
  `assert` が消えると、検査が黙って通る。先頭で `if not __debug__` を見て落とす。§99 を
  入れて表も作り直した変異で、`-O` なしは `PASSED: 1/2`、`-O` ありも
  `verify-spec-model: assert が無効（PYTHONOPTIMIZE か -O）` を出して `PASSED: 1/2` を確かめた。

3 件目と同じ穴が `scripts/lint-docs.sh` にもあることを実測した（`doc_type_spelling` が
消え、別の検査だけが赤になる）。本ラウンドの成果物ではないので
`docs/issues/gate-optimize-flag.md` に切った。往復は 5 回目で `AGENTS.md` の予算（3 回）を
超えている。以後の指摘は同じ issue へ切る。

### main の取り込み（#31 の着地後）

PR #31 が着地した（`2372f95`）ので main を取り込んだ。`scripts/corpus-lock.sh --check` は
差分なし（exit 0）。門番は 65 本、quick 層は 40/40・red なし。

```
$ git merge main --no-edit
$ bash scripts/corpus-lock.sh --check; echo "exit=$?"
exit=0
$ bash scripts/spec-lock.sh --check
spec-lock --check: 一致（70 エラーコード、10 文法群）
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/run-gates.sh --tier quick --jobs 4 target/quick-gates | tail -1
TALLY: 40/40 passed; red: none
```

`#31` 以前は main のワークフローが `runs-on: ubuntu-latest` で、org の Free 枠の支払いに
より job が始まらなかった（steps 0 で 2 秒）。取り込みで self-hosted へ切り替わった。
