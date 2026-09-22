# 門番の重複検査を削る（prune-gates）

開始 2026-09-22。着手時の main は `f2a3c81`（PR #30 着地後）。

## 手順書

### 背景

門番 83 本・検査約 3,800 件を、PR #30（test-prune）着地後の木で精査した。判定の基準は
`docs/plan.md` 33 節と同じ「処理系の検出力を落とさないか」で、門番の本数や検査数は目的に
しない。削除は「赤を隠すため」ではなく「処理系を守らない検査だから」に限る。

精査で見つかった重複は 7 群である。すべて、同じ内容を同じ走行でもう一度測っているか、
`docs/` の文書管理だけを測っている。

- **A. `cargo test` のラッパー。** m0 の検査 2 が同じ走行で `cargo test --offline --no-fail-fast`
  を実走し、対象のテスト二進をすべて含む（`docs/plan.md` 33 節の棚卸しと同じ根拠）。
  門番ごと消せるものは r9・r13・r14・r16〜r22・r23・r24・r25・r26・r28・record-constructors。
  一部の検査だけのものは r33-j・r45-e・r47-e・r48-e・r49-b・r44-f。
- **B. §56 の表と拒否ディレクトリの突き合わせ。** `crates/hado/tests/parse_corpus.rs` の
  `corpus_counts` が「拒否の `// error:` の集合 == spec の表のコード集合」「1 コード 1 本」を
  同じ走行で測る（m0）。r23-b・r24-f・r25-b・r26-d・r44-c 前半はこれと重複する。
  r25-e（accept に `Ok(`/`Error(` が無い）だけが r9〜r28 で唯一の固有検査なので、m1-corpus へ移す。
- **C. 門番ごとの corpus lock 照合。** `sha256sum -c tests/corpus.sha256` を m3 の検査 10（core）と
  parallel-rounds-d が持ち、run-gates の安定性ガードが走行中の改変を見る。r43-g・r45〜r65 の
  `corpus lock`・r68〜r81 の混合検査は重複する。
- **D. 文書の逐語固定と層の自己確認。** 「`spec §NN` に断片がある」「wants が §NN を指す」
  「spec-trace に行がある」「`rN` in the full tier」を測る検査は、処理系ではなく文書を固定する。
  `docs/plan.md` 33 節「文章固定検査の棚卸し」が削除・移動を認めている。層の自己確認は
  gate-cache の層分割検査（各門番の `# tier:` が一意で 3 層が全門番を覆う）が持つ。
  文書の構造は `scripts/lint-docs.sh` の検査 1・2・5・6 が持つ。r49-b（bug.md）と verify-r42 全体も
  ここに含め、r42 のうち lint-docs が持たない検査は lint-docs へ移す。
- **E. 受理コーパスの全件実走の重なり。** m6（core）は m7 の `--with-prelude` と重なり、r67 は
  `crates/hado/tests/corpus.rs` より緩い照合で、`build -o` は run_cli.rs と r48-e が測る。
  r44-c 後半・r25-a の `cargo test --test corpus` も m0 と重なる。
- **F. テストや stdlib の原文を grep する検査。** r44-e（r9_grammar.rs の綴り）・r44-d（accept の
  綴りを戻す）・r44-a の 2 ケース（`def once` 等）・r81-d・r43-a。保証は r9_grammar 自身、
  44-a の残り、r23_syntax の予約語テスト、81-a が持つ。r44 の残る 6 ケースは r23_syntax.rs へ
  移して門番を消す。r43-a は §55 の `def` 行を m7 が表として読む形に統合する。
- **G. 他の門番の結果を読み返す検査。** m2-5・m3-9・r36-h・r37-k・m1-corpus-7 の m0/walk/tail
  呼び直し。同じ走行の m0・m1・m3 が既に測っている。m2-6 と m1-corpus-7 の protected-files は
  同じ検査なので m1-corpus 側に 1 つ残す。

### 決定

- **決定 1: 削除の根拠は「同じ内容を同じ走行で測っている」に限る。** 検査の内容そのものは
  変えない。統合するときは畳む先に足してから元を消す（r25-e は m1-corpus へ、r43-a は m7 へ、
  r44 の 6 ケースは r23_syntax.rs へ、r42 の 3 検査は lint-docs へ）。
- **決定 2: verify-r11 と verify-r12 は残す。** verify-r38.sh の 38-a が「実在する 1/1 の門番 2 本」
  として `verify-r11`・`verify-r12` を探針に使う（`scripts/verify-r38.sh:23-33`）。消すなら r38 を
  別の 1/1 門番に付け替える必要があり、それは本ラウンドの範囲外にする。
- **決定 3: verify-m6.sh のファイルは残す。** verify-r37.sh:552 が `scripts/verify-m6.sh` を ast で
  読み、`verify` 関数を取り出して起動条件を測る。中身の検査は m7 へ統合するが、ファイルは残す。
- **決定 4: 門番を消すときはファイルごと消す。** gate-cache の層分割検査は `scripts/verify-*.sh`
  全件に一意な `# tier:` を要求するので、検査を空にした門番を残すと `PASSED: 0/0` の空振りになる。
- **決定 5: 文書の対応付けは外さず、今守るものへ付け替える。** `docs/spec-trace.md` の
  「固定するもの」欄が消える門番を指す場合は、m0 の `cargo test`・`parse_corpus::corpus_counts`・
  m7・lint-docs へ向け直す。
- **決定 6: コーパスと `tests/corpus.sha256` は変えない。** `tests/`・`stdlib/`・`crates/` の製品
  コードは変えない。`crates/hado/tests/r23_syntax.rs` だけは r44 の 6 ケースの移設で足す。
- **決定 7: `docs/plan.md` は触らない。** 33 節の「検査は 1 つも消していない」は 2026-09-15 の
  時点の記録で、本ラウンドの結果は本ファイルが持つ。

### 成果物

- 19 本の門番の削除: r9・r13・r14・r16〜r22・r23・r24・r25・r26・r28・record-constructors・r42・
  r44・r67（83 → 64 本）。
- 残る門番からの、corpus lock・文書固定・層自己確認・他門番の読み返し・`cargo test` ラッパーの
  検査の削除。
- r25-e の m1-corpus への移設、r43-a の m7 への統合、r42 の 3 検査の lint-docs への移設、
  r44 の 6 ケースの `crates/hado/tests/r23_syntax.rs` への移設。
- `docs/spec-trace.md` の参照の付け替え。
- `docs/rounds/prune-gates.md`: 本ファイル。

### 工程

1. 群 A〜G の削除・移設（担当群ごとに分けて並行。門番の実走は統括が直列に回す）。
2. `bash scripts/lint-docs.sh`。
3. `cargo test --offline`（m0 の全走）。
4. 触った門番、quick 層、core 層、full 層。
5. 本ファイルの実測を書いてコミットし、PR にする。

### 判定

- `cargo test --offline` が緑（テストの本数は r44 の 6 ケースを足す分だけ増える）。
- `bash scripts/run-gates.sh --tier quick|core|full` が `red: none`。
- `bash scripts/lint-docs.sh` が緑。
- `bash scripts/corpus-lock.sh --check` が緑（`tests/corpus.sha256` を更新していない）。
- 門番 64 本・検査数は削除前より減る。減った内訳は「適用した判定」に列挙する。

### 許容される赤

なし。削除・移設は振る舞いを変えないので、全層が緑であるべきである。赤が出た場合は、その
検査が実際には他で覆われていなかったということなので、1 本戻して報告する。

### スコープ外

- 残した門番の書き換え。
- verify-r11・verify-r12 を r38 の探針から外す改修。
- verify-m6.sh を m7 へ完全に畳んでファイルごと消す改修。
- `crates/`・`runtime/`・`stdlib/`・`tests/accept`・`tests/reject`・`tests/corpus.sha256` の変更。

### 中断の条件

- 削除した検査の重複先が実際には覆っておらず、門番が赤になった場合は、その検査を戻し、
  他の群を続けて報告する。
- 移設先（m1-corpus・m7・lint-docs・r23_syntax.rs）が元の検査と同じ入力で赤にならないことを
  確かめられない場合は、移設せずに残す。

### 適用した判定

門番ごと消した 19 本と、残した門番から落とした検査を列挙する。

- 門番ごと削除（19 本）: `verify-r9.sh`・`verify-r13.sh`・`verify-r14.sh`・`verify-r16.sh`〜
  `verify-r22.sh`・`verify-r23.sh`・`verify-r24.sh`・`verify-r25.sh`・`verify-r26.sh`・
  `verify-r28.sh`・`verify-record-constructors.sh`・`verify-r42.sh`・`verify-r44.sh`・
  `verify-r67.sh`。
- `scripts/verify-m1-corpus.sh`: 検査 7 の `walk`・`tail`・`m0` の呼び直しを落とし、
  開始時の protected-files の照合だけを残した。検査 6 に r25-e（受理コーパスが `Ok(`/`Error(` を
  名指ししない）を足した。
- `scripts/verify-m2.sh`: 検査 5（m0・m1-spec・m1-corpus の呼び直し）と検査 6（protected-files の
  重複）を落とし、7・8 を 5・6 に詰めた（8 → 6 検査）。`protected.sha` の作成も落とした。
- `scripts/verify-m3.sh`: 検査 9（m0・m1-spec・m1-corpus・m2 の呼び直し）を落とし、
  `cargo test` の終了コードを検査 8 に含めた。10〜12 を 9〜11 に詰めた（12 → 11 検査）。
- `scripts/verify-m6.sh`: 既定を受理コーパス全件の実走にし、`--with-prelude` を互換のため受ける。
- `scripts/verify-m7.sh`: m6 の呼び出しを落とした（3 → 2 検査）。
- `scripts/verify-r33.sh`: 33-j（`r9_grammar` と `r23_syntax` の cargo test）。
- `scripts/verify-r36.sh`: 36-h（m7 のログの cargo test 集計と m4・m5・m6 の呼び直し）。
- `scripts/verify-r37.sh`: 37-k（m7 の呼び直しと m7 のログの cargo test 集計）。
- `scripts/verify-r43.sh`: 43-a（§55 と stdlib の member 一致）と 43-g の corpus lock。
- `scripts/verify-r45.sh`: 45-e（§30・付録C・E125 行・§26.1 の逐語と `r9_grammar` の cargo test）と
  45-f の corpus lock。
- `scripts/verify-r46.sh`: 46-f（§57・wants・full tier）と corpus lock。
- `scripts/verify-r47.sh`: 47-e（§57・wants・full tier）、47-e（`r47_fixes` の cargo test）、
  corpus lock。
- `scripts/verify-r48.sh`: 48-e（§58・wants・full tier）、48-e（`run_cli` の cargo test）、
  corpus lock。
- `scripts/verify-r49.sh`: 49-b（`docs/bug.md` の節）、49-b（`build_cli` の cargo test）、
  49-d（full tier）、corpus lock。
- `scripts/verify-r50.sh`〜`verify-r65.sh`: 各 `rN_docs`（仕様の断片・系譜・wants・full tier）と
  各 corpus lock。
- `scripts/verify-r68.sh`〜`verify-r76.sh`: 各 `rN_docs`（仕様の断片・系譜・full tier と
  corpus lock）。
- `scripts/verify-r70.sh`・`verify-r71.sh`: 70-e・71-e（tier・系譜・手順書・corpus lock）。
- `scripts/verify-r77.sh`: 77-e のうち docs と corpus lock を落とし、print-type だけを残した。
- `scripts/verify-r81.sh`: 81-b（spec の断片）、81-d（accept の写しの `Void` → `Unit` 変異）、
  81-e（系譜・coverage・corpus lock・台帳）。
- `scripts/lint-docs.sh`: verify-r42 の 42-a(2)・42-a(3)・42-a(4) を検査 11〜13 として移した
  （10 → 13 検査）。
- `crates/hado/tests/r23_syntax.rs`: r44-a の残る 6 ケースを `once_and_again_are_the_operation_kinds`
  として移した（r44 の `once-is-reserved`・`again-is-reserved` の 2 ケースは
  `keywords_cannot_be_names` が持つ）。
- `docs/spec-trace.md`: 5 行目の `verify-r9.sh` の参照を、R9〜R28 の判定が
  `crates/hado/tests/` にあり m0 の全走が回すという記述に替えた。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番運用） | 門番は製品の欠陥を見逃さない範囲で最小に保つ | 工程 | 成功 | 本ファイルの実測 |
| （門番運用） | 同じ内容を同じ走行で 2 度測らない | 工程 | 成功 | `scripts/lib/gate-cache.sh`・`verify-gate-cache.sh` |
| （門番運用） | 層の所属は各門番の `# tier:` が唯一の正で、3 層が全門番を覆う | 工程 | 成功 | `verify-gate-cache.sh` の層分割検査 |
| （門番運用） | コーパスの lock は m3 の検査 9 と parallel-rounds-d が照合する | 工程 | 成功 | `verify-m3.sh`・`verify-parallel-rounds.sh` |
| （門番運用） | 文書の構造は lint-docs が持ち、逐語の断片は門番に置かない | 工程 | 成功 | `scripts/lint-docs.sh` |

## 実測

