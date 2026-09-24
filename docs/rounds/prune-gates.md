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
- **C. 門番ごとの corpus lock 照合。** `sha256sum -c tests/corpus.sha256` を m3 の検査 9（core）、
  r36-g・r37-i・r38、parallel-rounds-d が持ち、run-gates の安定性ガードが走行中の改変を見る。
  r43-g・r45〜r65 の `corpus lock`・r68〜r81 の混合検査は重複する（審査の 2 回目で
  「門番 1 本と直列の全走の後段が薄くなる」と指摘されたが、lock の持ち主は `docs/corpus.md` の
  m3 で、残る照合と `corpus-lock.sh --check` で足りると判断した。下の「審査の指摘と対応」）。
- **D. 文書の逐語固定と層の自己確認。** 「`spec §NN` に断片がある」「wants が §NN を指す」
  「spec-trace に行がある」（検査 14 が一般化して持つ）「`rN` in the full tier」を測る検査は、
  処理系ではなく文書を固定する。
  `docs/plan.md` 33 節「文章固定検査の棚卸し」が削除・移動を認めている。層の自己確認は
  gate-cache の層分割検査（各門番の `# tier:` が一意で 3 層が全門番を覆う）が持つ。
  文書の構造は `scripts/lint-docs.sh` の検査 1・2・5・6 が持つ。r49-b（bug.md）と verify-r42 全体も
  ここに含め、r42 のうち lint-docs が持たない検査は lint-docs へ移す。spec-trace の門番と検査名、
  `verify-rN.sh` とラウンドの記録の関係は lint-docs の検査 14 が持つ（審査の 2 回目で追加）。
  **審査の 6 回目でこの群の判断の 2 つが誤りと分かった**（`docs/issues/dropped-spec-contracts.md`）。
  仕様本文の断片を消しても緑のまま（`scripts/verify-r50.sh` の `r50_overflow` は実行時しか
  見ない）で、層の自己確認も字種と一意性しか見ないので門番の層が黙って変わっても赤にならない
  （`scripts/verify-r52.sh` の `# tier: full` を `quick` にすると `--print-tier full` から r52 が
  消えるのに `scripts/verify-gate-cache.sh` は `PASSED: 26/26`）。「wants が §NN を指す」のほうは、
  `docs/wants.md` が昇格で消えるメモなので契約ではない（審査の 6 回目で人の指摘）。
- **E. 受理コーパスの全件実走の重なり。** m6（core）は m7 の `--with-prelude` と重なり、r67 は
  `crates/hado/tests/corpus.rs` より緩い照合で、`build -o` は run_cli.rs と r48-e が測る。
  r44-c 後半・r25-a の `cargo test --test corpus` も m0 と重なる。
- **F. テストや stdlib の原文を grep する検査。** r44-e（r9_grammar.rs の綴り）・r44-d（accept の
  綴りを戻す）・r44-a の 2 ケース（`def once` 等）・r81-d・r43-a。保証は r9_grammar 自身、
  44-a の残り、r23_syntax の予約語テスト、81-a が持つ。r44 の残る 6 ケースは r23_syntax.rs へ
  移して門番を消す。r43-a の署名照合は m7 の prelude inventory へ移す。
- **G. 他の門番の結果を読み返す検査。** m2-5・m3-9・r36-h・r37-k・m1-corpus-7 の m0/walk/tail
  呼び直し。同じ走行の m0・m1・m3 が既に測っている。m2-6 と m1-corpus-7 の protected-files は
  見る区間が違う（m1-corpus は自分の走行の中、m2 は M2 の前後）ので、審査の 1 回目で m2 側も
  戻した（下の「審査の指摘と対応」）。

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
  r44 の 6 ケースの `crates/hado/tests/r23_syntax.rs` への移設。審査の 2 回目で lint-docs に
  検査 14（門番と文書の関係）を足した。
- `docs/spec-trace.md` の参照の付け替え（削った検査を指していた検査名を含む）。
- 本ラウンドの着地に要る基盤の変更として、CI の runner を自前の Linux x86_64 へ切り替えた
  （`.github/workflows/ci.yml`・`scripts/verify-ci.sh`・`docs/rounds/ci.md` の決定 9。private の
  org が Free 枠で job を起動できなくなったため）。
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
  詰めた。protected-files の検査は審査の指摘で検査 7 として戻し、`cargo test` 全体
  （`docs/corpus.md` が名指しする結合性の探針 `parser::tests::left_assoc_add_and_mul` を含む）も
  検査 8 として戻した（8 → 8 検査）。検査 5 の呼び直しの側だけが冗長で、全走では
  `cargo_cached` が m0 の同じ実走に畳まれる。
- `scripts/verify-m3.sh`: 検査 9（m0・m1-spec・m1-corpus・m2 の呼び直し）を落とし、
  `cargo test` の終了コードを検査 8 に含めた。10〜12 を 9〜11 に詰めた（12 → 11 検査）。
- `scripts/verify-m6.sh`: 既定を受理コーパス全件の実走にし、`--with-prelude` を互換のため受ける。
- `scripts/verify-m7.sh`: m6 の呼び出しを落とし、43-a の署名照合（§55 と stdlib の `def` 行の
  一致）を prelude inventory に足した。門番の `PASSED` は 2 のままで、inventory は 48 → 67 件
  （うち 9 件は審査の指摘で戻した 43-a(1) の関数集合の member）。
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
- `scripts/lint-docs.sh`: verify-r42 の 42-a(2)・42-a(3)・42-a(4) を検査 11〜13 として移した。
  審査の指摘で、spec-trace の門番と検査名、`verify-rN.sh` とラウンドの記録の関係を検査 14 に
  足した（10 → 14 検査）。
- `crates/hado/tests/r23_syntax.rs`: r44-a の残る 6 ケースを `once_and_again_are_the_operation_kinds`
  として移した（r44 の `once-is-reserved`・`again-is-reserved` の 2 ケースは
  `keywords_cannot_be_names` が持つ）。審査の指摘で、受理ケースは `parse_source` だけでなく
  `hado::check_source` も通す。
- `docs/spec-trace.md`: 5 行目の `verify-r9.sh` の参照を、R9〜R28 の判定が
  `crates/hado/tests/` にあり m0 の全走が回すという記述に替えた。あわせて、削った検査を
  指したままだった門番の検査名（45-e、68-e〜76-e、81-e）を現行に直した。

### 審査の指摘と対応（1 回目）

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

### 審査の指摘と対応（2 回目）

`0194fe1` への Codex 審査で 4 件（P2）が付いた。3 件を直し、1 件は記録して対応しない。

- **r23_syntax への移設（`crates/hado/tests/r23_syntax.rs`）**: 直した。旧 verify-r44 の受理ケースは
  `hado check` の実走だったので、`parse_source` だけでなく `hado::check_source` も通す。移設した
  ケースの `reply + capture` に型の食い違いを混ぜると E305 で赤になる（`check_source` が
  字句と構文の先まで見ている）。
- **m7 の 43-a(1)（`scripts/verify-m7.sh`）**: 直した。§55 の関数集合のブロックに、R43 で足した
  List・String・Int の 9 つの member が載っていることを名指しで固定した。ブロックから `head` を
  落とす変異で `NG: List.head in the §55 function set`（在庫 65/66）を確かめた。
- **spec-trace とラウンドの記録（`docs/spec-trace.md`、`scripts/lint-docs.sh`）**: 直した。削った検査を
  指したままだった spec-trace の検査名（45-e、68-e〜76-e、81-e）を現行に直し、lint-docs に検査 14 を
  足して「spec-trace が名指しする門番と検査名が実在する」「`verify-rN.sh` がラウンドの記録を持つ
  （`r10-r13` のような範囲の記録でもよい）」を機械で判定する。変異（`68-e` を戻す・実在しない門番を
  書く・`docs/rounds/r68.md` を外す）の 3 つで赤を確かめた。
- **門番ごとの corpus lock（`scripts/verify-r70.sh` など）**: 対応しない（記録）。lock の持ち主は
  `docs/corpus.md` の「`verify-m3.sh` チェック 9 がそれと照合する」で、全走では r36-g・r37-i・r38 と
  parallel-rounds-d も照合する。門番 1 本を直に回すときの照合は記録された `corpus-lock.sh --check` が
  持つ。r45 以降だけが持っていた写しを落としたのは、r11〜r43 と同じ形に揃えたもの。直列の全走の
  後段と門番 1 本に残る隙間は、実行全体の指紋を直列にも広げる別の変更の領分。

### 審査の指摘と対応（3 回目）

`93d0b4e` への Codex 審査で 6 件（P2 が 5、P1 の security が 1）が付いた。5 件は門番に取り込み、
1 件は人が決める判断として上げた。

機械で決まる整合の 5 件は、いずれも「削った検査が持っていた関係を、置き換えた検査が
前向きにしか見ていない」という同じ形だった。削除前の門番が守っていた組を名指しで固定する。

- **spec-trace の行の削除（`scripts/lint-docs.sh` 検査 14）**: 直した。前向きの照合（名指しされた
  門番と検査名が実在する）だけでは、表の行ごと消すのが素通りしていた。表が `verify-rN.sh` の形で
  門番を名指ししている節は、行を消したら赤にする。表に行を持たない門番（R29〜R31・R38・R49・R51。
  spec-trace は「R38 の判定器」の形で指す）は対象外。変異（r68・r81・r45 の行を消す）で赤を確かめた。
- **付録の部の見出し（`scripts/lint-docs.sh` 検査 1・2）**: 直した。`doc_parts` の `seen` は最初の部で
  1 になったきりなので、`# 付録` の部の見出しが消えても素通りしていた。部の見出しの実在と、文法の
  概形が付録C にあることを固定する。変異（`# 付録` を消す・付録C の見出しを変える）で赤を確かめた。
- **版の固定（`scripts/lint-docs.sh` 検査 5）**: 直した。旧 42-d の `spec_version` は `v0.4` を名指しで
  要求していたが、置き換えた検査は文書から導いた版を比べるだけだった。前付けと見出しを揃えて
  書き換えると素通りする。`v0.4` を名指しで固定し、前付けの中の版がその 1 つだけであることも見る。
  変異（全部 `v0.5` にする）で赤を確かめた。
- **専用テストターゲットの実在（`scripts/verify-m0.sh` 検査 8）**: 直した。`cargo test` は対象ごと
  消えても残りを走らせて緑になる（`r13_conditions.rs` を退避しても exit 0 を実測）。削除したラッパー
  門番が守っていた 16 の組（r9_grammar・r13_conditions・r14_rails・r16_sections・r17_files・
  r18_spelling・r19_contracts・r20_entries・r21_functors・r22_namespaces・r23_syntax・r24_effects・
  r25_effects・r26_effects・r31_effects・r37_world）を固定し、実在と 1 件以上の実行を要求する。
  名前をディスクから導くと、消えた対象が期待からも消えて素通りするので固定が要る。変異（対象を
  消す・中身を空にする）で `absent-` と `empty-` の赤を確かめた。
- **仕様の型名（`scripts/lint-docs.sh` 検査 15）**: 直した。旧 verify-r81 の `r81_spec` を移した。
  仕様本文に `Unit` が無いことと `Void` の綴り 8 か所を固定する。振る舞いの検査（81-a / 81-c）だけでは、
  仕様に `Unit` が戻っても緑になる。変異（`Unit` を戻す・綴りを変える）で赤を確かめた。
- **PR を sudo の通る自前 runner で走らせること（`.github/workflows/ci.yml`）**: 人が決める。PR を
  出せる人は `scripts/ci-run.sh` を書き換えられるので、門番が走る前に runner の root を取れる。
  `contents: read` は GitHub の token しか絞らない。いま登録した runner は 1 台で、PR を出せるのは
  共同作業者だけなので実害は出ていないが、public にする時点で `ubuntu-latest` に戻すか `sudo` を
  外すかが要る。4 案（PR だけ standard に戻す / PR では `apt` の段を回さない / Environment の承認を
  挟む / いまのまま）を PR のスレッドに上げ、推奨は「いまのまま着地させ、public の時点で切り替える」。

### 審査の指摘と対応（4 回目）

`badac66` への Codex 審査で 3 件（P2）が付いた。いずれも 3 回目で入れた検査の詰めが甘かった箇所で、
3 件とも直した。

- **削除した門番の対象の数え漏れ（`scripts/verify-m0.sh` 検査 8）**: 直した。3 回目で固定した 16 本は
  各ラウンドの専用ラッパー（`verify-r13.sh` など）だけを見ており、`verify-record-constructors.sh` が
  守っていた `named_record_constructors`・`named_record_order` と、`verify-r9.sh` が守っていた
  `r9_*` の 10 本を数えていなかった。16 → 26 本にした。変異（`r9_order.rs` を消す）で
  `absent-r9_order` の赤を確かめた。
- **節のコメントを検査名と数える（`scripts/lint-docs.sh` 検査 14）**: 直した。本文のどこかに
  `45-b` があればよいとしていたので、`check '45-b ...'` を消しても `# --- 45-b:` のコメントが
  残れば素通りしていた。ラベルを引数として渡している箇所だけを見る。`70-$([ ... ] && echo b ||
  echo c)` のように動的に組むラベルは、字句の `NN-$(` があるときだけ例外にする。変異（呼び出しを
  消してコメントを残す）で `verify-r45:45-b` の赤を確かめた。
- **一覧の門番の実在（`scripts/lint-docs.sh` 検査 14）**: 直した。`is_file()` で飛ばしていたので、
  門番と spec-trace の行を同時に消すと、前向きの照合（参照が無い）もここも往復の記録を数える
  ところも素通りしていた。一覧を「その門番と行が実在する」の契約にし、欠けているときは
  `absent` / `unlisted` として赤にする。変異（`verify-r68.sh` と r68 の行を同時に消す）で
  `verify-r68.sh:absent` の赤を確かめた。

3 回目と 4 回目の指摘は 8 件で、往復の予算（3 往復）を超えている。5 回目が来たときは、
残った指摘を `docs/issues/` に切り出して打ち切る（打ち切りの判断は人がする）。

### 審査の指摘と対応（5 回目）

`60223ac` への Codex 審査で 2 件（P2）が付いた。1 件は直し、1 件は切り出して打ち切る。

- **仕様の効果操作の綴り（`scripts/lint-docs.sh` 検査 16）**: 直した。削除した `verify-r44.sh` の
  `r44_spec`（44-b）は仕様本文の綴りを見ていたが、移設した `r23_syntax.rs` は綴りをコンパイラの
  入力としてしか見ていない。仕様だけを書き換える退行（`once` をキーワードの表から落とす、
  `reply` を戻す）が緑になっていた。検査 16 として仕様本文の綴りを固定した。変異（`once` を
  キーワードの表から落とす・`reply` を戻す）で赤を確かめた。
- **凍結した台帳の書き換え（`docs/issues/frozen-ledger-guard.md` に切り出し）**: 打ち切る。
  指摘の前提（削除した `r81_spec` が台帳の綴りを守っていた）は成り立たない。`r81_spec` は
  `docs/spec.md` だけを読んでいた（削除前の `scripts/verify-r81.sh`）。ただし「凍結した台帳を
  書き換えても文書層が緑のまま」という現象そのものは実在する（`docs/ledger.md` の `Unit` を
  `Void` に置換しても `PASSED: 16/16`）。これは門番の穴ではあるが、この PR の退行ではなく、
  凍結した文書の不変性を門番で持つかという別の決定なので、`docs/issues/` に切って PR 本文に
  切り出し先を書く（往復の予算 3 に対する 5 回目）。

### 審査の指摘と対応（6 回目）

`78f451e` への Codex 審査で 2 件（P2）が付いた。1 件は自分の検査の欠陥なので直し、1 件は削りの
見落としなので切り出す。

- **動的ラベルの例外が広すぎる（`scripts/lint-docs.sh` 検査 14）**: 直した。4 回目で入れた例外は
  番号だけで判定していたので、`verify-r70.sh` に `70-$(...)` が 1 つあると 70-a〜70-d の全部が
  登録済みと見なされ、70-a や 70-d の実行を消しても緑のままだった。式が作る文字（`echo b` と
  `echo c`）だけを候補にした。変異（70-a を消す・70-d を消す）で赤、70-b/70-c の動的な式を
  残したままでは緑を確かめた。
- **削った門番が持っていた仕様本文の契約（`docs/issues/dropped-spec-contracts.md` に切り出し）**:
  切り出す。指摘のとおりで、これはこの PR の見落としである。落ちた `NN-x spec, trace, tier,
  wants` の 4 つの対象を 1 つずつ実測で振り分けた（推測で「引き受け手がある」と判断しない）。
  - **仕様本文の綴り**: 引き受け手が無い。§53.1 の `hado: stack overflow` を消して `lint-docs`
    16/16・`verify-r50` 4/4 のまま。落ちた検査のうち仕様本文の針を持っていたのは 25 本。
  - **門番ごとの層の所属**: 引き受け手が無い。`verify-r52.sh` の `# tier: full` を `quick` に
    すると `--print-tier full` から r52 が消えるのに `verify-gate-cache.sh` は `PASSED: 26/26`。
    `tier_list_case` は字種と一意性しか見ない。
  - **`docs/spec-trace.md` の門番名と § 行**: 検査 14 が引き受ける。§58.1 の行を消すと
    `PASSED: 15/16`。
  - **`docs/wants.md` の参照**: 契約ではない（人の指摘）。`docs/wants.md` は
    `# やりたいことメモ` で「以下は未決定」と書いており、`docs/issues.md` が「根拠と完了条件が
    揃った項目を `docs/issues/` へ移し、`docs/wants.md` からは消す」と定める。`'R52' in wants`
    は昇格のたびに赤になるので、落としたままでよい。**下の「適用した判定」の群 D が「wants が
    §NN を指す」を文書の逐語固定として扱っていたが、これは契約ではなかった。**

  規範文の針と層の所属を 1 つの検査にまとめる案を推奨として issue に切り出し、PR 本文に
  切り出し先を書く。

6 回目の 1 件目は自分の検査の欠陥なので直した。2 件目は削りの効果そのものに関わるので、
**着地の前に人が決める**（仕様本文の契約を戻すか、削りの主張を狭めるか）。

### 審査の指摘と対応（8 回目）

`7c1b8b0` への Codex 審査で 2 件（P2）が付いた。どちらも自分が入れた検査の欠陥なので直した。

- **旧い型名の照合が語の全体でない（`scripts/lint-docs.sh` 検査 15）**: 3 回目で入れた検査は
  `'Unit' not in spec` で見ていたので、`UnitPrice` のような現行の識別子を巻き込んでいた。
  旧 `r81_spec` と同じ `\bUnit\b` に戻した。変異で `UnitPrice` を足すと緑のまま、`Unit` を
  戻すと赤（`PASSED: 15/16`）を確かめた。
- **保護対象の照合が `cargo test` より前にある（`scripts/verify-m2.sh`）**: 7 回目で検査 8 として
  `cargo test` を戻したが、保護対象の照合は検査 7 のまま前段にあった。単体で m2 を回す手順では
  `cargo test` が `docs` `runtime` `scripts` を書き換えても緑のままになる。`cargo test` を検査 7、
  照合を検査 8 に入れ替えた。書き換えるスタブで、直す前は `PASSED: 8/8`、直した後は
  `NG: 8 protected-files-changed`（`PASSED: 7/8`）を確かめた。

### 審査の指摘と対応（9 回目）

`b56cfc5` への Codex 審査で 2 件（P2）が付いた。どちらも自分が入れた検査と記録の欠陥なので直した。

- **コーパスの照合も `cargo test` より前にある（`scripts/verify-m2.sh`）**: 8 回目で保護対象の
  照合を最後に移したが、検査 5 のコーパスの照合は前段のままだった。片付けが戻すのは
  `tests/accept` と `tests/reject` だけなので、`tests/library` や `crates/hado/tests/corpus.rs` の
  書き換えは残る。検査 8 で両方を照合するようにした（`tree-unchanged`）。検査 7 が
  `crates/hado/tests/corpus.rs` を書き換えるスタブで、直す前は `PASSED: 8/8`、直した後は
  `NG: 8 tree-unchanged: corpus-changed`（`PASSED: 7/8`）を確かめた。
  最初の変異は `tests/library/main.hd` に追記したが、このファイルは開始時に存在しない
  （未追跡で、テストが作る）ので `sha256sum -c` は新しいファイルを見ない。変異が無効だった
  ので、開始時に確実に在る追跡済みのファイルに変えて測り直した。
- **層の所属の探針が測れていなかった（`docs/issues/dropped-spec-contracts.md`）**: 記録した
  探針は `--print-tier` の名前を `verify-r52` で照合していたが、実際は `verify-` と `.sh` を
  落とした `r52` を出す。加えて `hado_tier_gates` は累積なので `full` は `quick` と `core` を
  含み、`full` へ動かしても full の走行からは消えない。層を**後ろへ**動かしたときだけ検出力が
  落ちる（`quick` → `full` で PR を回す `core` から消える）。探針を `m1-spec` で測り直し、
  変異前 1・変異後 0・`verify-gate-cache` は `PASSED: 26/26` を確かめた。

### 仕様本文の契約の引き受け手（人の判断。2026-09-23）

落ちた仕様本文の針（25 本の門番）と門番ごとの層の所属は、**戻さずに着地する**と決めた。理由は、
針が症状であって原因ではないためである。正本が無いので「仕様は X と言っている」を文字列の
実在でしか表せず、それが 25 本の針を生んだ。25 本を戻せば、原因を直すラウンドがまた消す。

原因の側は `docs/plan.md` 36 節に段取りを記録し、第 1 段（`spec/errors.toml`・
`spec/grammar.ebnf` を正本にし、§56 の表と付録C を生成物にする）を PR #34 で進めた。以後の段で
エラーコードのアンカーとラベル、文法、追跡表がモデルに入り、針の代わりにフィールドの照合で
測れるようになる。

層の所属（`verify-gate-cache.sh` が字種と一意性しか見ないこと）は、本 PR の見落としとして
`docs/issues/dropped-spec-contracts.md` に残す。

**この PR で検出力が落ちる範囲を明示する。** 仕様本文の規範文を消しても赤にならない状態が、
第 2 段以降が入るまで続く。エラー表と文法は第 1 段でモデルに入ったが、§30・§53.1・§57・§58 系の
規範文の針は戻っていない。

### 審査の指摘と対応（7 回目）

`a1db857` への Codex 審査で 1 件（P2）が付いた。直した。

- **m2 の結合性の探針（`scripts/verify-m2.sh` 検査 8）**: 直した。m2 は `cargo test --test
  parse_corpus` しか回さなくなっており、`parser::tests::left_assoc_add_and_mul` を含む全体を
  見ていなかった。`docs/corpus.md` は「式木の往復は結合性を見ない（描画側が括弧を必ず付ける）
  ので、`parser::tests::left_assoc_add_and_mul` が固定し、`verify-m2.sh` が `cargo test` 全体を
  見ることで捕まえる」と書いており、**この PR がその記述を偽にしていた**（`docs/corpus.md` の
  当該の文はこの PR で変えていない）。全走では m0 が同じ `cargo test` を回すので気づきにくいが、
  AGENTS.md の「門番は小さく回す」に従って m2 を単体で回す手順では誰も見ていない。検査 8 として
  全体の実走と、`parser::tests::left_assoc_add_and_mul ... ok` の行を戻した。全走では
  `cargo_cached` が m0 の実走に畳むので費用は増えない。変異（テスト名を変える・期待値を壊す）で
  `associativity-probe-not-run` と `cargo-test-exit!=0` の赤を確かめた。

m2 は 8 → 8 検査に戻り、core は 639/639。削りの正味は −68 → −63 になった（m2 の −1 が消え、
文書層が +6 になったため）。削りの判断そのものは、呼び直しの側（検査 5）だけを冗長と見る形で
保っている。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番運用） | 門番は製品の欠陥を見逃さない範囲で最小に保つ | 工程 | 成功 | 本ファイルの実測 |
| （門番運用） | 同じ内容を同じ走行で 2 度測らない | 工程 | 成功 | `scripts/lib/gate-cache.sh`・`verify-gate-cache.sh` |
| （門番運用） | 層の所属は各門番の `# tier:` が唯一の正で、3 層が全門番を覆う | 工程 | 成功 | `verify-gate-cache.sh` の層分割検査 |
| （門番運用） | コーパスの lock は m3 の検査 9、r36-g、r37-i、r38、parallel-rounds-d が照合する | 工程 | 成功 | `verify-m3.sh`・`verify-r36.sh`・`verify-r37.sh`・`verify-r38.sh`・`verify-parallel-rounds.sh` |
| （門番運用） | 文書の構造は lint-docs が持ち、逐語の断片は門番に置かない | 工程 | 成功 | `scripts/lint-docs.sh` |

## 実測

着手時の main は `f2a3c81`。すべてこの worktree（`CARGO_TARGET_DIR` は既定の `./target`）で
回した。層別は `--jobs 1`（直列）と `--jobs 3` の両方で回し、下は審査の 3 回目まで直した後の
`--jobs 3` の結果。

```
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 38/38 passed; red: none
$ bash scripts/run-gates.sh --tier core --jobs 3 target/gates-core 2>&1 | tail -1
TALLY: 639/639 passed; red: none
$ HADO_GATE_TIMES=target/gate-times.tsv bash scripts/run-gates.sh --tier full --jobs 3 target/gates-full 2>&1 | tail -1
TALLY: 3720/3720 passed; red: none
$ bash scripts/verify-m0.sh 2>&1 | tail -1
PASSED: 8/8
$ bash scripts/verify-m2.sh 2>&1 | tail -1
PASSED: 8/8
$ bash scripts/verify-m3.sh 2>&1 | tail -1
PASSED: 11/11
$ bash scripts/verify-m7.sh 2>&1 | grep 'prelude inventory'
prelude inventory: 67/67
$ bash scripts/lint-docs.sh 2>&1 | tail -1
PASSED: 16/16
$ bash scripts/corpus-lock.sh --check; echo exit=$?
exit=0
$ cargo fmt --all --check; echo exit=$?
exit=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1; echo exit=${PIPESTATUS[0]}
clippy exit=0
$ cargo test --offline --test r23_syntax 2>&1 | tail -3
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

m0 の全走は `OK: 2 cargo-test passed=746`（8/8。検査 8 は審査の 3 回目で足し、4 回目で
対象を 26 本に広げた）。文書層の 16 は、審査の 5 回目で足した検査 16 を含む。r23_syntax の
新しい 1 本を足した分だけ、test-prune の 745 から増えた。受理コーパスの全件実走は m6 の
`PASSED: 111/111` の 1 回と m0 の `cargo test`（`corpus.rs`）の 1 回で、m7 は `PASSED: 2/2`
（inventory と cargo test）。m7 の inventory は門番 1 件の中身なので、層の合計は増えない
（48 → 67 は内訳）。full は m7 の編集前（`--jobs 1`）と、審査の 2 回目まで直した後（`--jobs 3`）に
回し、後者が `3720/3720`・`red: none`。core は 602 → 639（m6 の +38 と、審査で戻した m0 の検査 8・m2 の検査 8）。文書層は 14 → 16
（検査 15・16）。

門番は 83 → 64 本。全走の検査の合計は 3788 → 3720（test-prune の実測が 3788）。落とした
検査は 107 件、m6 が既定で回すようになった受理コーパス 38 件と lint-docs の検査 6 件が
増えて、正味 −63。core 層は 602 → 639（m6 の +38 と、審査で戻した m0 の検査 8・m2 の検査 8）。
quick 層は 38 のまま。

主な門番の検査数（前 → 後）: m0 7 → 8、m2 8 → 8（審査で戻した）、m3 12 → 11、m6 73 → 111、
m7 3 → 2、r33 73 → 72、r36 388 → 384、r37 262 → 260、r43 12 → 10、r45 8 → 5、r46 7 → 5、
r47 7 → 4、r48 8 → 5、r49 6 → 2、r50〜r65 は各 −2、r68〜r76 は各 −1、r77 は 5 のまま
（中身を print-type だけに替えた）、r81 5 → 2。文書層は 10 → 16。

門番ごとの秒（編集前の `--jobs 1` の全走、`target/gate-times.tsv` の上位）: r34 214、m0 186、
r35 183、r36 76、m1-corpus 67、embed-runtime 67、m2 65、r37 54、r43 42。合計 1239 秒で、
wall は約 20 分。critical path は r34・m0・r35 で変わらず、削減は wall にはほとんど出ない
（編集の費用が実益である）。

静的照合: `tests/accept`・`tests/reject`・`stdlib/`・`tests/corpus.sha256` の差分は空。
`crates/hado/tests/r23_syntax.rs` だけを足した。門番の名指しの参照で消した門番を指すものは
残っていない（`docs/ledger.md` と `docs/plan.md` の履歴の記述は凍結なので触らない）。
`docs/spec-trace.md` の 5 行目だけを、R9〜R28 の判定が `crates/hado/tests/` にあり m0 が回す
という記述に替えた。

