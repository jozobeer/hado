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
- **C. 門番ごとの corpus lock 照合。** `sha256sum -c tests/corpus.sha256` を m3 の検査 9（core）と
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
  移して門番を消す。r43-a の署名照合は m7 の prelude inventory へ移す。
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
- `scripts/verify-m2.sh`: 検査 5（m0・m1-spec・m1-corpus の呼び直し）を落とし、7・8 を 5・6 に
  詰めた。protected-files の検査は審査の指摘で検査 7 として戻した（8 → 7 検査）。
- `scripts/verify-m3.sh`: 検査 9（m0・m1-spec・m1-corpus・m2 の呼び直し）を落とし、
  `cargo test` の終了コードを検査 8 に含めた。10〜12 を 9〜11 に詰めた（12 → 11 検査）。
- `scripts/verify-m6.sh`: 既定を受理コーパス全件の実走にし、`--with-prelude` を互換のため受ける。
- `scripts/verify-m7.sh`: m6 の呼び出しを落とし、43-a の署名照合（§55 と stdlib の `def` 行の
  一致）を prelude inventory に足した。門番の `PASSED` は 2 のままで、inventory は 48 → 58 件。
- `scripts/verify-r33.sh`: 33-j（`r9_grammar` と `r23_syntax` の cargo test）。
- `scripts/verify-r36.sh`: 36-h（m7 のログの cargo test 集計と m4・m5・m6 の呼び直し）。
- `scripts/verify-r37.sh`: 37-k（m7 の呼び直しと m7 のログの cargo test 集計）。
- `scripts/verify-r43.sh`: 43-g の corpus lock。43-a（§55 と stdlib の署名一致）は m7 へ移した。
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
- `docs/corpus.md`: `// needs: prelude` の行を、M6 が prelude を連結して全件を実走する形に直した
  （審査の指摘。下の「審査の指摘と対応」）。
- `scripts/lint-docs.sh`: verify-r42 の 42-a(2)・42-a(3)・42-a(4) を検査 11〜13 として移した
  （10 → 13 検査）。
- `crates/hado/tests/r23_syntax.rs`: r44-a の残る 6 ケースを `once_and_again_are_the_operation_kinds`
  として移した（r44 の `once-is-reserved`・`again-is-reserved` の 2 ケースは
  `keywords_cannot_be_names` が持つ）。
- `docs/spec-trace.md`: 5 行目の `verify-r9.sh` の参照を、R9〜R28 の判定が
  `crates/hado/tests/` にあり m0 の全走が回すという記述に替えた。

### 審査の指摘と対応

PR #31 の Codex 審査で 4 件（P2）が付き、4 件とも直した。

- **M2 の protected-files の検査（2 件、`scripts/verify-m2.sh`）**: 落とす根拠を「m1-corpus 検査 7 と
  重複」としていたが、m1-corpus は自分の走行の間しか見ず、run-gates の指紋照合は `--jobs > 1` の
  並列の後段だけ、門番 1 本を直に回す手順では誰も見ない。指摘のとおりなので検査 7 として戻した。
  戻した検査が赤を出せることは、走行中に `docs/corpus.md` を書き換えて
  `NG: 7 protected-files-changed`（`PASSED: 6/7`）を確かめた。
- **`// needs: prelude` の契約（`scripts/verify-m6.sh`）**: m6 の既定を全件にしたことで
  `docs/corpus.md` の「M3〜M6 の判定から除外される」と食い違った。文書を「prelude を連結しない
  M5 は付いた件を飛ばし、M6 は prelude を連結して全件を実走する」に直した。タグの付け忘れは
  m1-corpus 検査 6 が赤にし、m6 は `tests/accept/*/main.hd` を実数から取るので、文書の文と実装は
  どちらも門番が決める。
- **移した 42-a(2) の版の一意性（`scripts/lint-docs.sh`）**: 移設で `versions == {'v0.4'}` の検査を
  落としていた。見出しの版を正として、前付けの版がそれ 1 つだけである検査を戻した（`v0.4` の
  直書きはしない）。前付けに `v0.3` を足す変異で `NG: the current spec is the only snapshot`
  （`PASSED: 12/13`）を確かめた。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番運用） | 門番は製品の欠陥を見逃さない範囲で最小に保つ | 工程 | 成功 | 本ファイルの実測 |
| （門番運用） | 同じ内容を同じ走行で 2 度測らない | 工程 | 成功 | `scripts/lib/gate-cache.sh`・`verify-gate-cache.sh` |
| （門番運用） | 層の所属は各門番の `# tier:` が唯一の正で、3 層が全門番を覆う | 工程 | 成功 | `verify-gate-cache.sh` の層分割検査 |
| （門番運用） | コーパスの lock は m3 の検査 9 と parallel-rounds-d が照合する | 工程 | 成功 | `verify-m3.sh`・`verify-parallel-rounds.sh` |
| （門番運用） | 文書の構造は lint-docs が持ち、逐語の断片は門番に置かない | 工程 | 成功 | `scripts/lint-docs.sh` |

## 実測

着手時の main は `f2a3c81`。すべてこの worktree（`CARGO_TARGET_DIR` は既定の `./target`）で
回した。層別は `--jobs 1`（直列）と `--jobs 3` の両方で回し、下は審査の指摘を直した後の
`--jobs 3` の結果。

```
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 38/38 passed; red: none
$ bash scripts/run-gates.sh --tier core --jobs 3 target/gates-core 2>&1 | tail -1
TALLY: 637/637 passed; red: none
$ HADO_GATE_TIMES=target/gate-times.tsv bash scripts/run-gates.sh --tier full --jobs 3 target/gates-full 2>&1 | tail -1
TALLY: 3719/3719 passed; red: none
$ bash scripts/verify-m2.sh 2>&1 | tail -1
PASSED: 7/7
$ bash scripts/verify-m7.sh 2>&1 | grep 'prelude inventory'
prelude inventory: 58/58
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 13/13
$ bash scripts/corpus-lock.sh --check; echo exit=$?
exit=0
$ cargo fmt --all --check; echo exit=$?
exit=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
clippy exit=0
$ cargo test --offline --test r23_syntax 2>&1 | tail -3
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

m0 の全走は `OK: 2 cargo-test passed=746`（7/7）。r23_syntax の新しい 1 本を足した分だけ、
test-prune の 745 から増えた。受理コーパスの全件実走は m6 の `PASSED: 111/111` の 1 回と
m0 の `cargo test`（`corpus.rs`）の 1 回で、m7 は `PASSED: 2/2`（inventory と cargo test）。
full は m7 の編集前（`--jobs 1`）と、審査の指摘を直した後（`--jobs 3`）に回し、後者が
`3719/3719`・`red: none`。

門番は 83 → 64 本。全走の検査の合計は 3788 → 3719（test-prune の実測が 3788）。落とした
検査は 107 件、m6 が既定で回すようになった受理コーパス 38 件が増えて、正味 −69。core 層は
602 → 637（m6 の +38 と、m2 −1・m3 −1・m7 −1）。quick 層は 38 のまま。

主な門番の検査数（前 → 後）: m2 8 → 7、m3 12 → 11、m6 73 → 111、m7 3 → 2、r33 73 → 72、
r36 388 → 384、r37 262 → 260、r43 12 → 10、r45 8 → 5、r46 7 → 5、r47 7 → 4、r48 8 → 5、
r49 6 → 2、r50〜r65 は各 −2、r68〜r76 は各 −1、r77 は 5 のまま（中身を print-type だけに
替えた）、r81 5 → 2。

門番ごとの秒（編集前の `--jobs 1` の全走、`target/gate-times.tsv` の上位）: r34 214、m0 186、
r35 183、r36 76、m1-corpus 67、embed-runtime 67、m2 65、r37 54、r43 42。合計 1239 秒で、
wall は約 20 分。critical path は r34・m0・r35 で変わらず、削減は wall にはほとんど出ない
（編集の費用が実益である）。

静的照合: `tests/accept`・`tests/reject`・`stdlib/`・`tests/corpus.sha256` の差分は空。
`crates/hado/tests/r23_syntax.rs` だけを足した。門番の名指しの参照で消した門番を指すものは
残っていない（`docs/ledger.md` と `docs/plan.md` の履歴の記述は凍結なので触らない）。
`docs/spec-trace.md` の 5 行目だけを、R9〜R28 の判定が `crates/hado/tests/` にあり m0 が回す
という記述に替えた。

