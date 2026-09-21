# 最小の LSP（lsp-minimal）

開始 2026-09-21。着手時の main は `fb1c29d`。

## 手順書

### 背景

v0.5 の 3 本目（`docs/plan.md` 35 節の L1〜L4）。利用者は言語設計者本人で、エディタは
Neovim 0.12.5。`hado check` の配管（`--format json`、`--print-type`）は既にあり、`lsp` の
入口が無い。LSP にして初めて見える穴が 3 つある。診断が常に 1 件、位置が開始点だけ、
診断で止まると型が無い。根拠は着手時の issue（`docs/issues/lsp-minimal.md`、着地で削除。
履歴が持つ）の現象欄と、§56（段階順と
1 件報告）、§58（CLI）、§58.3（型の印字）、§58.4（機械可読な診断）。

### ゴール

- Neovim 0.12.5 が `.hd` と `.hdi` を開くと、`hado lsp` から診断・hover（最上位の `def` の
  型）・正準形への code action が届く。補完は無い。
- CLI 側の 3 つの穴が埋まっている。`check --show-all` が同じ段階の全件をアンカー順に出し
  （E1xx と E3xx の致命は 1 件のまま、既定と拒否コーパスの判定は不変）、`--format json` に
  終端の位置があり、網羅性・冗長・単相化で止まっていても `--print-type` と hover が型を
  返す。
- `scripts/verify-lsp-minimal.sh` が issue の完了条件（着地で削除。履歴が持つ）を測って緑で、
  下の「判定」の変異で赤になる。
- 仕様 §58 系と根の `README.md` に書かれ、main に着地し、CI の full 層が `red: none`。

### 決定済みの事項（`docs/plan.md` 35 節）

- L1: `--show-all` は opt-in。同じ段階の全件をアンカー順。E1xx と E3xx の致命は 1 件。
- L2: 冗長・網羅性・契約の検査で止まっていても型を返す。`--print-type` と hover は同じ
  意味。字句・構文・名前解決・型の致命で止まったときは型が無い。
- L3: `hado lsp` サブコマンド。JSON-RPC は `serde_json` で書く。機能は診断・hover・code
  action の 3 つ。
- L4: `--format json` に終端の位置を足す。
- 判断基準は 7 節と同じ。迷ったら拒否、新機能は最小形。

### 着手時の所見

コードの事実（2026-09-21、`fb1c29d`）。設計の判断は builder が持つ。

- `crates/hado/src/lexer.rs` の `Span` は `line` と `col` だけで終端が無い。`col` は
  `Lexer::bump` の Unicode スカラの数え（1-based）。`.span` の参照は src 全体で 555 件、
  `Span {` の構築は 11 件。`Checked.value_types` の鍵 `(usize, usize)` と `Ast` の anchor は
  `.line ==` `.col ==` で引き、`Span` を通らない。
- 1 件に絞る `min_by_key(diagnostic_order)` は 13 箇所あり、段階の全件を落としているのは
  `lib.rs` の `check_parsed_filtered`（`Checked` を捨てる場所でもある）、`check.rs` の
  `check_ast`、`names.rs` の名前解決の 2 箇所。`infer.rs` の `recover_inference_error` は
  段階 2 と 3 も `deferred` に回すので、E3xx が出ていても `Checked` は作られる。L2 の
  「型の致命で型が無い」は自動では成り立たない。`diagnostic_order` が E6xx を E3xx と
  同じ段にする。
- JSON の直列化は `project.rs` の `Project::report_json` の 1 箇所（手書き）。CLI の
  サブコマンドは `main.rs` の `parse_args` の白名簿で、`input_path` が `.hd` を強制する。
  `crates/hado/tests/check_cli.rs` が usage の文字列をバイト一致で見る。
- `fixes.rs` の `fix_at(source, code, line, col) -> Option<Fix>` は純関数で byte range と
  置換文字列を返す。disk 結合は `project.rs` の `canonical_hint` と `apply_canonical_fix`
  にあり、overlay はこれを迂回できる。`LineMap` は private。
- `Project::load(entry)` は disk からしか読めず、overlay の入口は無い。`SourceId` は無く、
  source の同一性は行 offset の連結（`read_file` と `localize`）で表す。prelude と functor
  の実体は合成行を持つ。`root_directory` は `entry.parent()` だけで、契約から根を遡る
  処理は無い。`source_paths` の順は入口が先、残りは相対パスのバイト順。
- `Checked.value_types` は名前の参照位置・`Fn`・field・適用結果の型を持つが、束縛の
  定義位置には無い。
- `serde_json` は依存に無いが、`cargo add serde_json --offline --package hado` は 1.0.151 に
  解決してビルドできる（下の実測）。`lsp-server` `lsp-types` `tower-lsp` は cache に無い。
- Neovim は `scripts/treesitter-setup.sh --cache DIR` で固定版が取れ、`vim.lsp.config` /
  `vim.lsp.enable` / `vim.lsp.start` と `positionEncodings` の `utf-8` `utf-16` `utf-32` を
  持つ（下の実測）。client は `didChange` を既定で 150 ms まとめる
  （`runtime/lua/vim/lsp/_changetracking.lua` の `client.flags.debounce_text_changes or 150`）。
  `.hd` `.hdi` を `hado` にする対応は Neovim に無く、`editors/tree-sitter-hado/README.md`
  の `vim.filetype.add({ extension = { hd = 'hado', hdi = 'hado' } })` が出所。
- Neovim を駆動する既存の門番は `verify-treesitter.sh`（`scripts/treesitter-check.py` が
  `nvim --headless` を pty の下で起こす）だけで、JSON-RPC を扱う既存コードは無い。
- 新しい門番は `# tier:` と `# round: lsp-minimal` を持ち、`docs/rounds/lsp-minimal.md` の
  `## 実測` にフェンスが要る（`lint-docs.sh` 検査 6）。末尾は `PASSED: n/N` で、NG のとき
  非 0 で終える（r70 / r71 で見つかった落とし穴）。
- 拒否コーパスは 1 ファイル 1 欠陥で、本数は §56 の行数に固定されている。多欠陥の入力
  や符号化の対照はコーパスに足せない。

前提のずれ。

- issue の完了条件が E602 の代表に指定する「早期に `Err` を返す経路（missing signature
  member）」は現在のコードに無い。`module_body` の E602 も `recover_inference_error` が
  遅延し、`infer_with_diagnostics` は `Ok((Checked, deferred))` を返す。
- issue の案と完了条件の `パス:行` は 2026-09-19 の移行（`17fe4da`）以降ずれている。
  対応: `lexer.rs:236-244` は `Lexer::bump`、`lexer.rs:430` と `:539` は `lex_word` の
  分岐、`project.rs:247` は `source_paths` と `collect_paths`、`project.rs:184` は
  `root_directory`、`check.rs:531` は `check_rewrap`（`:531` は E403）、`fixes.rs:990` は
  E507 の書き換え、`infer.rs:613` は missing signature member（`module_body` の中）、
  `lib.rs:243-250` は `diagnostic_order`、`spec.md:2079` は E602 の行、`spec.md:2015` は
  §56 の段階順の文。`infer.rs:1849` に E602 固有の経路は無い。
- 根の `README.md` に Neovim の設定は無い（見出しは 必要なもの / 使い方 / 検証 / 構成 /
  文書）。完了条件の「`README` の設定の `cmd`」の置き場は builder が決める。

計画時に考えた案（採否は builder。採らなくてよく、理由の記録も要らない）。

- `Span` に `end_line` `end_col` を既存の単位で足す。行 offset の連結が source の同一性を
  既に与えるので、`SourceId` と byte offset の導入は要らない可能性がある。
- 検査器の入口を「診断の列と `Option<Checked>`」にし、CLI が先頭 1 件か全件かを選ぶ。
- 解析を worker thread に置き、`(uri, version)` の付いた結果を最新 version だけ公開する。
  門番が完了順を決める手段として、環境変数で hold ファイルを待つ試験用の hook。
- `Project::load` の隣に、パスから本文を引く provider を受ける入口。
- 契約の解析根の選び方（issue の案）を純関数にして `cargo test` で測る。
- byte offset・行列・符号化の変換を 1 つのモジュールに寄せ、JSON と LSP で共有する。

### 判定

各行は「何を測って、どの値なら合格か」。現在値は着手時の実測。対照の詳細は
`docs/issues/lsp-minimal.md` の完了条件が正で、着地で issue を消した後は git 履歴が持つ。
以下では issue の完了条件を「J1〜J11」として参照する。

- J1. `bash scripts/verify-lsp-minimal.sh` が `PASSED: N/N` で終了コード 0。N は検査の
  数で、直書きせず一覧から導く。現在値: 門番が無い。
- J2. `check --show-all --format json` が同じ段階の全件をアンカー順（§50 のファイル順・
  行・列）に出し、既定の `check` は先頭の 1 件だけ。E1xx と E3xx は `--show-all` でも
  1 件。段階をまたぐ入力（E204 と E505、E602 と E402）では最初の段階だけ。現在値:
  `--show-all` は用法の誤り（終了コード 2）。
- J3. `--format json` の各行に `end_line` `end_col` があり、`end_col - col` が対象の
  スカラ数、`end_line - line` が対象の行数 - 1。期待値は判定器が本文から独立に数える。
  現在値: 7 フィールドのみ。
- J4. E402・E505・E602 で `check --print-type=<名前>` が診断と型の両方を出し、hover が
  同じ型を返す。E1xx・E2xx・E3xx では型が無い。現在値: E505 で止まると型が出ない。
- J5. `nvim --headless` から `.hd` を開き、保存せずに本文を変え、診断・hover・code action
  が新しい version を参照する。誤りを直す version で空の diagnostics が publish される。
- J6. version の追い越し。N と N+1 を続けて送り、完了順を N+1 → N と N → N+1 の両方に
  しても、publish された diagnostics は N+1 だけで、N の hover と code action は終端の
  応答を返す。N の解析を始めない実装も、公開が N+1 のままなら通す。
- J7. `shutdown` に `null` が返り、続く `exit` で子プロセスが終了コード 0 で自分で終わる
  （`jobwait` が timeout せず、kill を送らない）。`didClose` の後も `shutdown` まで解析が
  続く。
- J8. 契約（`.hdi`）の URI に診断と hover が出て code action は空配列。契約だけを開いた
  対照、入れ子の契約（`tests/accept/namespaces/store/store.hdi`）、`library` 宣言だけの
  `.hd` と契約の 2 ファイル、複数の `workspaceFolders`、folder 名が接頭辞になる対照
  （`<work>/w` と `<work>/w/app`、契約は `<work>/w/application/c.hdi`）、§47 の複数実装で
  E602 が実装の URI に出て関連位置が契約を指す対照。候補の `didOpen` の順を入れ替えても
  結果が同じ。
- J9. 符号化。`utf-8` `utf-16` `utf-32` それぞれ別の initialize で、
  `result.capabilities.positionEncoding` が 1 つの文字列で、診断と code action の range の
  両端がスカラ数から独立に数えた期待値に一致する。`positionEncodings` を省いた
  initialize では `utf-16`。client から送る hover と code action の位置は、
  `def decoy = "あ` と `😀` 15 個と `" def target (b: Bool) => if b | true => 1 | false => 0`
  を 1 行に置いた入力で、符号化ごとに独立に数えた列（hover は UTF-8 82・UTF-16 50・
  UTF-32 35、code action の range は UTF-8 102-131・UTF-16 70-99・UTF-32 55-84）に送り、
  `target` の型と `if b => 1 | 0` の書き換えが返る。
- J10. 2 つの source を開き、片方の未保存の変更が他方を入口にした結果に現れ、`didClose`
  の後は disk の本文に戻る。診断が付いた側の URI に非空と空の diagnostics が順に
  publish される。code action の edit を本文に適用した結果が、同じ本文への
  `check --fix` の結果とバイト一致する。
- J11. `cargo test --offline` が全緑、core 層が `red: none`、`bash scripts/lint-docs.sh` が
  全緑、`bash scripts/corpus-lock.sh --check` が差分なし。現在値: それぞれ緑・
  `red: none`・10/10・差分なし。
- 門番が赤になることを次の変異で確かめ、結果を実測に書く。
  1. 公開の前の version の照合を外す → J6 で N の診断が出て赤。
  2. 空の diagnostics を送らない → J5 と J10 で赤。
  3. `end_col` を `col + byte 長` にする → J3 と J9 で赤。
  4. `positionEncoding` の advertisement を消す → J9 で赤。
  5. `shutdown` に応答しない → J7 で赤（timeout ではなく判定として）。
  6. 契約の親だけを根にする → J8 の入れ子の契約で赤。

### 許容される赤

- 本ファイルの実測に門番の出力を書くまで、`lint-docs.sh` の検査 6 は赤でよい。
- **終了時に許容される赤は無い。**

### スコープ外

- 補完、局所束縛と式の hover、rename、go to definition。
- Neovim 以外のエディタの設定。
- `tests/accept`・`tests/reject`・`stdlib/`・`tests/corpus.sha256` の変更（門番の入力は
  作業ディレクトリに生成する）。
- `docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md` へのラウンドの節や行の追加。
- 性能（増分解析・キャッシュ）。

### 中断の条件

- 段階の全件を返す変更で、拒否コーパスの先頭の 1 件が変わる入力が見つかったとき。
  変えずに止め、その入力と両方の出力を報告に書く（§56 の解釈に触る）。
- 仕様の解釈を選ばないと進めない箇所に当たったとき。「仕様への疑義」に書いて止める。
- レビューの往復が 3 回に達したとき。残りは `docs/issues/<slug>.md` に切り出して PR
  本文に書く（AGENTS.md）。

### 不確実性

- `nvim --headless` を pty 無しの subprocess で起こしたとき、`vim.lsp` の client が動くか。
  tree-sitter の門番は pty を使った。
- J6 の「両方解析して N+1 を先に完了」を Neovim から決定的に起こせるか。起こせなければ
  N の解析を始めない形で通してよい。
- `Span` の終端を parser のどの層で埋めるか（`Node` の構築は 98 箇所）。
- 契約の解析根で、`library` 宣言だけの `.hd` を `Project::load` に渡したとき、既存の
  ライブラリ検査（§52）がそのまま使えるか。

### 決定

builder が設計を決めたら、ここに決定と理由を書く。

- **D1. 終端は `lexer::Span` が持ち、`Diagnostic` は `Range`（始点 + 終端）へ広げない。**
  `Span` に `end_line` `end_col` を足し、`Span::point` / `Span::until` / `Span::before` を
  置く。`Diagnostic` にも同じ 2 フィールドを足すが、`Diagnostic` は `Span` を持たず
  `line` `col` を持ち続ける（`format!` の `{:?}`・`Display`・門番の逐語が `line` `col` の
  綴りに依存しているため、フィールド名を変えない）。
- **D2. 全 `Span` 構築点（11 件）を一斉に直す。** `Span` は `Copy` な構造体で、1 箇所でも
  フィールドを足すと全構築点が型エラーになる。`Span::point` で意味が変わらない箇所を
  まとめ、`Span::until` で範囲が伸びる箇所を選ぶ。
- **D3. 位置の同一性は始点で比べる（`check::same_start`）。** 範囲が伸びると、
  「同じ要素か」を `span == span` で見ていた 4 箇所（`type_has_span`、`condition_is_direct`、
  `def_at`、`branch_entry`）が壊れる。いずれも要素の始まりだけを見る意味だったので、
  始点比較へ寄せる。これは §58.4 の範囲追加が既存の意味論を変えないための必須の追随で、
  `cargo test` の全緑が根拠（下の実測）。
- **D4. `Diagnostic` の終端は「その診断の書き換え範囲（§57）」から引く。**
  §58.4 の終端はアンカーの構文要素の終端だが、判定 J3 は `end_col - col` が対象の
  スカラ数であることを要求し、判定 J10 は code action の edit が `check --fix` とバイト
  一致することを要求する。両方を満たす自然な定義が「§57 の書き換え範囲」で、これは
  `fixes::fix_range` が既に持っている。書き換えの無い診断は始点と同じ位置で終わる。
  理由: これで `end_*` が CLI と LSP で同じ 1 つの真実になり、J3 と J10 が別々の実装を
  持たない。
- **D5. 連続する離れた診断の範囲は `Span` の伝播で決め、枝は parser の記録に合わせる。**
  式・型のノードは「最後に消費したトークンの終端」まで伸ばす（`Parser::to` /
  `TypeExpr::ending_at`）。枝（`PatternBranch` / `CondBranch`）は `branch_spans` に記録する
  終端と `node.span` の終端を必ず一致させる。`branch_entry` が枝を span で引くため、
  ここがずれると §57 の書き換えが丸ごと落ちる（下の実測の E507 がそれ）。
- **D6. `Project` は読み込んだ本文を保持する。** `Project::load` が読んだ各 source の本文を
  `Rc<String>` で持ち、`diagnostic_end` と LSP の `range_in_parse` がそこから範囲を引く。
  overlay（未保存の本文）を扱う LSP が、位置の裏のバイト列を disk から読み直せないため。
- **D7. 差し当たり `fixes::fixes` は自分の解析から範囲を引く経路（`range_in`）を持つ。**
  LSP は overlay を解析するので、「パスを読み直して `fixes()` を呼ぶ」経路は使えない。
- **D8. `has_type` と `type_of` は 1 つの探索（`Checked::scheme_of`）を共有する。**
  着手時の `has_type` は最上位の `def` しか見ず、実装メンバ（`lib::Util.helper`）の
  `--print-type` が `no type for` で落ちていた（`verify-r55.sh` の 55-e が赤）。同じ探索に
  乗せて直し、無い名前は §58.3 の `no definition named`、推論が失敗した名前（Q2）だけが
  `no type for` になる。
- **D9. 契約が宣言するメンバの型は、契約名で鍵付けした `Checked.contracts` に契約だけから
  作る。** `Checked.modules` は同じ宣言を実装ごとに実体化した形（抽象型は `Mem.T`、
  `shared` は解けた型）で持つので、実装が 0 の契約（ライブラリの公開面。§39・§52）と
  実装が 2 つ以上の契約には引ける鍵が無い。`contracts` は契約自身の抽象型を `Store.T` と
  綴り、他の契約の `X.T` は `contract_types` の束縛のまま解く（実装がちょうど 1 つでなければ
  `Error` で、hover は `null`）。探索順は最上位の `def` → 実装名 → 契約名。診断は 1 件も
  足さない（宣言が解けないメンバは表に無いだけで、その診断は既存の段階が持つ）。
  理由: §58.5 は契約の URI の結果が候補の順序に依らないことを求め、契約だけから作った
  型は実装の個数と名前に依らない。同名の実装（`implement Store as Store`）が `shared type
  T = Int` を持つときは実装名の表が先に当たり、契約の hover も解けた型（`Int Option`）を
  出す。CLI の `--print-type=Store.find` と hover が同じ 1 つの経路なので、J4 の一致は
  保たれる。

### 工程

builder が進めた順と、工程ごとに回した門番をここに書く。着地の作法は AGENTS.md の
「着地の直前に main を取り込み、lock を作り直す」に従い、着地後の full 層は CI（push to
main）の run を実測に書く。

1. `docs: lsp-minimal のラウンドを書く`（`4674532`、着手時の契約）。
2. `cargo fmt --all` / `cargo test --offline` を回しながら、`Span` の終端（D1〜D5）と
   `Project` の本文保持（D6）を入れる。
3. `--show-all`・`--format json` の終端・`--print-type` の型（J2〜J4）。
4. `hado lsp`（J5〜J10）。
5. 門番 `scripts/verify-lsp-minimal.sh` と仕様 §58 系・`README.md`。§58.4 は 7 フィールド
   から 9 フィールドへ改め、終端を §57 の書き換え範囲と定める（Q1 の決着）。

### 工程（実測）

実際に進めた順と、各コミットで回した門番。

1. `4674532 docs: lsp-minimal のラウンドを書く`（契約）。
2. `27a9b5a feat: 診断の範囲に終端を持たせ、§58.4 の end_line / end_col を出す`。
   `Span` に終端（D1・D2）を足し、位置の同一性を開始点で見る（D3）。回した門番:
   `cargo test --offline`、`verify-rustfmt`、`verify-clippy`。
3. `20ee32b`・`2ad7a10 docs: ...` — 決定 D1〜D7 と疑義 Q1・Q2 の決着。
4. `83d56f0 feat: --show-all と、止まっていても型を返す --print-type を入れる`（J2〜J4）。
   回した門番: `cargo test --offline`、`verify-m1-corpus`、`verify-r47`・`verify-r16`。
5. `b55de46 fix: 枝の書き換え範囲が最後のトークンを落とすのを直す`。J9 の
   code action の範囲を測る途中で見つけた既存の不具合。`branch_range` が枝の最後の
   トークンの開始を終端にしていたので、`check --fix` が `if b => 1 | 0 0` を作っていた。
   `LineMap::end_offset` を足して直し、腕の形（§31・§32）は `catch_all_range` から外す
   （外さないと E512 の書き換えが腕の本体を越えて `| f` を作る。HEAD では書き換えを
   出さず正しかったので、これは退行になる）。
6. `186435c feat: hado lsp で診断・hover・code action を返す`（J5〜J10）。
7. `ee671ff docs: §58.4 の終端と §58.5 の LSP、README の Neovim 設定を書く`。
8. `5b3cf14 docs: lsp-minimal の実測を書く`（J2〜J5・J7・J9・J10 の実測）。
9. `3ed7528`・`29a8a14`・`6e88512`・`44fd924`・`bd29c53` — 契約のメンバを位置から
   引けるようにし（J8）、契約が宣言する型を実装が無くても契約名で引けるようにする。
10. `b1d8165 test: lsp-minimal の門番を足す`、`404013d docs: 門番の実測と、6 つの変異の
    結果を書く`。回した門番: `verify-lsp-minimal`（9/9）、quick 層（38/38、red: none）。

### 仕様への疑義

builder が見つけた疑義をここに書く（上の「前提のずれ」は計画時のもの）。

- **Q1. §58.4 は終端の位置を定義していない。** 現行の §58.4（`docs/spec.md:2170`）は
  フィールドを「`path` `line` `col` `code` `message` `canonical` `related` の **7 つ**」と
  明記し、`end_line` `end_col` に触れない。一方 `docs/plan.md:1289` の L4 は「`--format json`
  に終端の位置を足す」と定め、issue の完了条件は `end_line` `end_col` の存在と、
  `end_col - col` が対象のスカラ数であること、`end_line - line` が対象の行数 - 1 で
  あることを要求する。実装は L4 に従って 9 フィールドを出している（実測 J3）。
  仕様本文の 7 つの欄は L4 の着地で書き換わるべきだが、**何を終端とするかの定義が
  仕様のどこにも無い**ので、書き換える文を builder が決められない。

  対立する 2 つの読み:

  1. **対象のトークンの終端**（issue の「案」34 行目）: `end_col` は字句の `bump` と同じ
     単位で、対象のスカラ数が `end_col - col`。`tests/reject/E505/main.hd` の E505 なら
     束縛名 `unused` の 6 スカラで `end_col - col = 6`。
  2. **§57 の書き換え範囲の終端**（issue の完了条件 45〜51 行目）: `tests/reject/E507/main.hd`
     の E507 で `end_line` が `def f` の行、終了行が `| None => None` の行になり、
     `end_col - col` は分岐全体のスカラ数になる。E402（書き換えなし）は始点と同じ位置で
     終わる。

  判定 J3 は「`end_col - col` が**対象**のスカラ数」と言い、判定 J10 は code action の
  edit が `check --fix` とバイト一致することを要求する。読み 1 は J3 の字面に合うが、
  E507 の「対象」が `if` なのか分岐全体なのかを決めないと値が定まらない。読み 2 は J10 と
  同じ 1 つの真実になり、issue の完了条件の逐語（`tests/reject/E507/main.hd:3` から
  `:5` まで）に合うが、J3 の「対象のスカラ数」を「書き換え範囲のスカラ数」と読み替える
  ことになる。

  **決着（2026-09-21、人の判断）**: 読み 2 を採る。§58.4 の終端は §57 の書き換え範囲の
  終端とする。理由は、J10 が code action の edit と `check --fix` のバイト一致を要求する
  以上 CLI と LSP が同じ 1 つの範囲を共有する必要があり、J3 の「対象」を書き換え範囲と
  読めば両方を 1 つの実装で満たせるため。決定 D4 の実装がそのまま正しく、E507 の実測は
  issue の完了条件の逐語と一致する（`3:1`〜`5:12`）。§58.4 の本文は着地で 9 フィールドに
  改め、`end_col - col` を「対象の書き換え範囲のスカラ数」と定める（下の工程 5）。

- **Q2. E602 は型を返し、E3xx は返さない。同じ段階 3 なので段階番号では分けられない。**
  `diagnostic_order`（`crates/hado/src/lib.rs:375`）は E3xx と E6xx を同じ stage 3 に置く
  （`docs/spec.md:2015` の段階順どおり）。しかし plan.md L2 は「型の致命で止まったときは
  型が無い」と定め、issue は E602 で型を返すことを要求する。段階番号だけでは両者を
  区別できない。

  **決着（2026-09-21、人の判断）**: **型推論が途中で失敗したかどうか**で分ける。E602 は
  推論が最後まで走って `Ok((Checked, deferred))` を返すので型がある。E3xx は
  `infer_with_diagnostics` が早期に `Err` を返すので型が無い。コードの採番に依存せず、
  §56 の段階順ではなく実装の経路と一致する。issue の「E602 は `infer.rs:613` の早期
  `Err` 経路を使う」という記述は前提がずれているが（上の「前提のずれ」）、経路で分ける
  方針はそのずれに影響されない。

- **Q3. §58.3 の「`<契約名>.<member>`」は実装名で引いており、実装 0 のときの抽象型の
  綴りが仕様に無い。** §58.3 は `lib::Util.helper` の `Util` を契約名と呼ぶが、値の参照は
  実装の名前で行い（§47）、`implement Store as Mem` なら源の綴りは `Mem.find` である。
  着手時の実装は実装名でしか引けず、契約名 `Store.find` は「無い名前」だった。§58.5 は
  契約の URI に hover を返すと定めるので、D9 で契約名でも引けるようにしたが、契約名で
  引いたときに抽象型を `Store.T` と綴ること、`shared` を解かないこと、同名の実装があれば
  実装名の表が先に当たることは、仕様のどこにも書かれていない。決着は人の判断待ちで、
  仕様本文の改稿は工程 5 に含める。

## 系譜

| 仕様の節 | 要求 | 種別 | 判定 | 根拠 |
|---|---|---|---|---|
| §56 | 段階順で最初の段階、同一段階はアンカー位置のソース順 | 検査 | 受理・拒否 | J2 の段階をまたぐ対照、`verify-m1-corpus.sh` |
| §56 | E1xx と E3xx の致命は 1 件 | 検査 | 拒否 | J2 |
| §50 | ファイルの順序は入口が先、残りは相対パスのバイト順 | 検査 | 受理 | J2 の行番号を逆にした対照 |
| §52 | `main` の無い `.hd` はライブラリとして検査し、独立した根になる | 解析根 | 受理 | J8 の `library` 宣言だけの対照 |
| §47 | 1 つの契約を複数の実装が満たせる | 解析根 | 受理 | J8 の複数実装の対照 |
| §57 | 局所的な書き換えは `--fix` と同じ | code action | 受理 | J10 のバイト一致 |
| §58 | `hado lsp` の命令 | CLI | 受理 | J7 |
| §58.3 | 網羅性・冗長・単相化で止まっていても型を返す | 型の印字 | 拒否 | J4 |
| §58.4 | `end_line` `end_col`、`--show-all` | 診断の出力 | 拒否 | J2、J3 |
| §58 の新しい小節 | 診断・hover・code action、lifecycle、position encoding、契約の URI | LSP | 受理・拒否 | J5〜J10 |
| §58.5 | 契約の URI の hover は、実装の有無と個数に依らず契約が宣言する型 | LSP | 受理 | J8、`crates/hado/tests/lsp_hover_contract.rs`、`print_type_members.rs` |

## 実測

着手時（2026-09-21、main `fb1c29d`）。

```
$ cargo run --offline -q -- check --format json tests/reject/E505/main.hd
{"path": "main.hd", "line": 4, "col": 7, "code": "E505", "message": "binding unused is unused", "canonical": null, "related": null}
$ cargo run --offline -q -- check --print-type tests/reject/E505/main.hd
main.hd:4:7: error[E505]: binding unused is unused
$ cargo run --offline -q -- check --show-all --format json tests/reject/E505/main.hd; echo rc=$?
usage: hado check [--print-type[=<name>]] [--fix] <file> | dump-mono <file> | build [-o <path>] [--fix] <file.hd> | run <file.hd> [-- <arg>...] | fmt [--check] <file.hd> | test <file.hd>
rc=2
$ cargo run --offline -q -- lsp </dev/null; echo rc=$?
usage: hado check [--print-type[=<name>]] [--fix] <file> | dump-mono <file> | build [-o <path>] [--fix] <file.hd> | run <file.hd> [-- <arg>...] | fmt [--check] <file.hd> | test <file.hd>
rc=2

# serde_json は offline で解決してビルドできる（試した後に戻した）
$ cargo add serde_json --offline --package hado
      Adding serde_json v1.0.151
$ cargo build --offline -q; echo rc=$?
rc=0

# 固定版の Neovim は LSP client の入口と 3 つの符号化を持つ
$ nvim --headless -u NONE -i NONE -c 'lua local c = vim.lsp.protocol.make_client_capabilities(); print(vim.fn.has("nvim-0.12"), vim.lsp.config ~= nil, vim.lsp.enable ~= nil, vim.lsp.start ~= nil, vim.inspect(c.general.positionEncodings))' -c 'qa!'
1 true true true { "utf-8", "utf-16", "utf-32" }
```

### 実装の実測（2026-09-21）

J2（`--show-all`）。同じ段階の E505 を 2 件持つ入力を門番と同じ形で生成し、
既定が 1 件、`--show-all` が全件をアンカー順に出すことを見る。

```
$ cargo run --offline -q -- check --format json <2 件の E505>/main.hd
{"path": "main.hd", "line": 2, "col": 7, ..., "code": "E505", "message": "binding unusedA is unused", ...}
$ cargo run --offline -q -- check --show-all --format json <同>/main.hd
{"path": "main.hd", "line": 2, "col": 7, ..., "code": "E505", "message": "binding unusedA is unused", ...}
{"path": "main.hd", "line": 5, "col": 7, ..., "code": "E505", "message": "binding unusedB is unused", ...}
```

段階をまたぐ対照。独立な E204（名前解決）と E505（冗長）を持つ 1 ファイルでは、
`--show-all` でも最初の段階の E204 だけが出る。E602（型）と E402（網羅性）でも
同じで、E602 だけが出る。

```
$ cargo run --offline -q -- check --show-all --format json <E602 と E402>/main.hd
{"path": "main.hd", "line": 4, "col": 1, ..., "code": "E602", "message": "missing signature member", "related": {"path": "main.hd", "line": 3, "col": 5, "label": "declared in the contract"}}
```

J3（終端）。E507 は `def f`（3 行目）から `| None => None`（5 行目）までを覆い、
issue の完了条件の逐語と一致する。書き換えの無い E402 は開始と同じ位置で終わる。

```
$ cargo run --offline -q -- check --format json tests/reject/E507/main.hd
{"path": "main.hd", "line": 3, "col": 1, "end_line": 5, "end_col": 12, "code": "E507", ...}
$ cargo run --offline -q -- check --format json tests/reject/E402/main.hd
{"path": "main.hd", "line": 4, "col": 3, "end_line": 4, "end_col": 3, "code": "E402", ...}
```

J4（止まっていても型を返す）。E4xx・E5xx・E6xx では診断と型の両方、E1xx・E2xx・
E3xx では診断だけを出す。

```
$ cargo run --offline -q -- check --print-type tests/reject/E402/main.hd
main.hd:4:3: error[E402]: not all constructor values are covered
Int
$ cargo run --offline -q -- check --print-type tests/reject/E505/main.hd
main.hd:4:7: error[E505]: binding unused is unused
Int
$ cargo run --offline -q -- check --print-type tests/reject/E602/main.hd
main.hd:8:1: error[E602]: missing signature member; declared in the contract at main.hd:7:5
Int
$ cargo run --offline -q -- check --print-type tests/reject/E301/main.hd
main.hd:6:12: error[E301]: cannot unify Fn(Int, Int => Int {A}) with Fn(Int, Int, Int => B {C})
```

J7・J9（lifecycle と符号化）。標準入出力に直接入力を送り、応答を読む。

```
$ printf 'Content-Length: 58\r\n\r\n{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run --offline -q -- lsp
{"id":1,"jsonrpc":"2.0","result":{"capabilities":{"codeActionProvider":true,"hoverProvider":true,"positionEncoding":"utf-16","textDocumentSync":1}}}
```

`positionEncodings` に `["utf-8","utf-16"]` を出す initialize では `utf-8` を広告し、
`shutdown` が `null` を返し、`exit` で終了コード 0 になる。

J5・J10（未保存の本文・hover・code action）。作業ディレクトリに `.hd` を生成し、
`didOpen` の後に hover と code action を送る。hover は `check --print-type=target`
と同じ型を返し、code action の編集を本文に適用した結果は `check --fix` とバイト
一致する。

```
{"method":"textDocument/publishDiagnostics","params":{"diagnostics":[{"code":"E520","range":{"end":{"character":53,"line":1},"start":{"character":24,"line":1}},"severity":1,"source":"hado"}],"uri":".../lsp_e2e.hd","version":1}}
{"id":10,"jsonrpc":"2.0","result":{"contents":{"kind":"plaintext","value":"Fn(Bool => Int {A})"}}}
{"id":11,"jsonrpc":"2.0","result":[{"edit":{"changes":{".../lsp_e2e.hd":[{"newText":"if b => 1 | 0","range":{"end":{"character":53,"line":1},"start":{"character":24,"line":1}}}]}},"kind":"quickfix","title":"if b => 1 | 0"}]}
$ cargo run --offline -q -- check --print-type=target <同>/lsp_e2e.hd
Fn(Bool => Int {A})
$ cmp <code action を適用した本文> <check --fix を適用した本文> && echo BYTE-IDENTICAL
BYTE-IDENTICAL
```

途中で見つけた既存の不具合。`branch_range` が枝の最後のトークンの開始を終端に
していたので、`check --fix` が `if b | true => 1 | false => 0` を `if b => 1 | 0 0`
にしていた（末尾の `0` が余る）。直した後は正しい `if b => 1 | 0` になる。

```
$ cargo run --offline -q -- check --fix <E520 の入力>/main.hd; sed -n 1p <同>/main.hd
def target (b: Bool) => if b => 1 | 0
```

### 実装メンバの `--print-type` の回帰（2026-09-21）

`has_type` が最上位の `def` しか見ず、実装メンバの `--print-type` が落ちていた（D8）。

```
$ bash scripts/verify-r55.sh | tail -2         # 直す前
NG: 55-e an implementation member is printed by name
PASSED: 6/7
$ bash scripts/verify-r55.sh | tail -1         # 直した後
PASSED: 7/7
```

### 契約のメンバの型の実測（2026-09-21）

J8（契約の hover）。契約だけを開き（`.hd` は開かない）、`rootUri` を作業ディレクトリに
した生の JSON-RPC で `good` の位置に hover を送る。返る型は `check --print-type=<契約名>.
<member>` と同じで、入れ子の契約は祖先の `.hd` を入口にして `api::M.good` で引ける。

```
$ python3 hover.py <flat> store.hdi 2 4        # type T = | A | B / module M / def good (x: Int): T
{"id": 2, "jsonrpc": "2.0", "result": {"contents": {"kind": "plaintext", "value": "Fn(Int => T {A})"}}}
$ python3 hover.py <nested> api/store.hdi 2 4
{"id": 2, "jsonrpc": "2.0", "result": {"contents": {"kind": "plaintext", "value": "Fn(Int => api::T {A})"}}}
$ cargo run --offline -q -- check --print-type=M.good <flat>/main.hd
Fn(Int => T {A})
$ cargo run --offline -q -- check --print-type=api::M.good <nested>/main.hd
Fn(Int => api::T {A})
```

抽象型を持つ契約 `Store`（`type T`、`type A Box`、`def A find(id: Int, x: A): T Option`）を
実装 0・1（`Mem`）・2（`Mem` と `Disk`）・同名 `shared` で測る。契約名の印字は実装の個数に
依らず、実装名の印字は実装ごとに変わる（D9）。

```
$ cargo run --offline -q -- check --print-type=Store.find <実装 0>/main.hd
Fn(Int, A => Store.T Option {B})
$ cargo run --offline -q -- check --print-type=Store.find <実装 1>/main.hd
Fn(Int, A => Store.T Option {B})
$ cargo run --offline -q -- check --print-type=Mem.find <実装 1>/main.hd
Fn(Int, A => Mem.T Option {B})
$ cargo run --offline -q -- check --print-type=Store.find <実装 2>/main.hd
Fn(Int, A => Store.T Option {B})
$ cargo run --offline -q -- check --print-type=Store.find <implement Store as Store, shared type T = Int>/main.hd
Fn(Int, A => Int Option {B})
```

J8 の `library` 宣言だけの `.hd` と契約の 2 ファイル。契約だけを開いた hover と、その
`.hd` を入口にした `--print-type` が同じ型を返す。値メンバ（§46 の `def A empty: A T`）は
§58.3 のとおり本体の型と効果行で印字される。

```
$ python3 hover.py <library 1 行の lib.hd と store.hdi> store.hdi 2 4
{"id": 2, "jsonrpc": "2.0", "result": {"contents": {"kind": "plaintext", "value": "Fn(Int => T {A})"}}}
$ cargo run --offline -q -- check --print-type=M.good <同>/lib.hd
Fn(Int => T {A})
$ cargo run --offline -q -- check --print-type=Stack.empty <module Stack / type A T / def A empty: A T、実装 0>/main.hd
A Stack.T {B}
```

### 門番の実測（2026-09-21）

`bash scripts/verify-lsp-minimal.sh` は 10 件を測り、3 回続けて緑で、実行時間は
6.4 秒・12.2 秒・6.4 秒だった（差は固定版の Neovim と tree-sitter の取得）。

```
$ bash scripts/verify-lsp-minimal.sh
OK: lifecycle-shutdown
OK: capabilities
OK: encoding-negotiation
OK: diagnostics-range-encoding
OK: diagnostics-unsaved-change
OK: hover-top-level-def
OK: code-action-byte-identical
OK: contract-hdi
OK: root-selection
OK: version-guard
PASSED: 10/10
```

「判定」の 6 つの変異を実際に当てて、赤になることを確かめた。

| 変異 | 結果 | 赤になった検査 |
| --- | --- | --- |
| 1. 公開の前の version の照合を外す | 赤にならない | （下に理由） |
| 2. 空の diagnostics を送らない | 赤 | `diagnostics-unsaved-change`（PASSED: 8/9） |
| 3. `end_col` を `col + byte 長` にする | 赤 | `encoding-negotiation` と `diagnostics-range-encoding`（7/9） |
| 4. `positionEncoding` の advertisement を消す | 赤 | `capabilities` と符号化の 2 件（6/9） |
| 5. `shutdown` に応答しない | 赤 | `lifecycle-shutdown`（8/9、timeout ではなく判定） |
| 6. 契約の親だけを根にする | 赤にならない（門番では） | 単体テストで赤（下に理由） |
| 7. `component_prefix` を文字列の接頭辞にする | 赤 | `root-selection`（9/10） |

変異 3 の実測。判定器が本文から数えた期待値は UTF-8 の 102-131 で、判定 J9 の
数値と一致した。

```
NG: encoding-negotiation utf-8: no diagnostic with range
  {'start': {'line': 0, 'character': 102}, 'end': {'line': 0, 'character': 131}};
  got [{'start': 0, 'end': 0}, {'start': 102, 'end': 102}]
```

変異 5 の実測。応答が来ないことを timeout として扱うのではなく、期待した応答が
来ないこととして落とす。

```
NG: lifecycle-shutdown probe error: timeout: shutdown response
```

変異 1 と 6 が門番で赤にならない理由。

変異 1（版の照合）は、解析が 1 本の筋で直列に走り、`didChange` がまとまって
届いたときは最新の本文だけで解析するので、古い版が新しい版より後に公開される
経路が無い。照合を外しても観測できる差が出ない（`crates/hado/src/lsp.rs` の
settle のループ）。門番は「ある版の後にそれより古い版が公開されない」ことを
全公開列で確かめる形に強めたが、この実装では照合が到達しないので緑のままになる。
判定 J6 の「完了順を入れ替えても」は、実装が直列化を選んだ以上この門番では
弁別できない。

変異 6（契約の根）は、根を渡さないと Neovim が client を起こさない（`clients=0`
を実測）ので、client 経由では測れない。根の決定は純粋な関数なので、単体テスト
`library_root_walks_up_to_the_directory_declaring_library`
（`crates/hado/tests/lsp_protocol.rs`）で祖先を辿ることを固定した。このテストは
変異 6 で赤になる。門番の `contract-hdi` は根を明示する 4 つの対照
（契約だけ・入れ子・実装と同時・根を渡さない入れ子）を持ち、うち 3 つが緑になる。

変異 7（`component_prefix` を文字列の接頭辞にする）は `root-selection` で赤になる。
`file:///work/w` と `file:///work/w/app` の 2 つの folder を渡し、`file:///work/w/application/c.hdi`
の契約を単独で開いたとき、根が `w` になって `w` の下の `.hd` が入口になることを測る。
文字列の接頭辞で選ぶ実装は `w/app` を選び、`w/application` の下を検索して候補 0 に
なるので通らない（§58.5、issue の「folder の名前が接頭辞になる対照」）。

### 着地前の全走（2026-09-21）

main は `fb1c29d` で、この枝はその上にある（`git merge-base --is-ancestor main HEAD`
が真）。取り込むものが無いので `git merge main` は不要で、lock に差分は無い。

```
$ bash scripts/corpus-lock.sh --check
（差分なし）

$ bash scripts/lint-docs.sh
PASSED: 10/10

$ cargo test --offline
test result: ok. ... （69 本すべて ok、rc=0）
```

層別。quick は 38/38、core は 601/601 で、どちらも `red: none`。

```
$ bash scripts/run-gates.sh --tier quick <out>
TALLY: 38/38 passed; red: none

$ bash scripts/run-gates.sh --tier core <out>
TALLY: 601/601 passed; red: none
```

core を最初に回したときは `verify-cognitive-complexity` が赤だった。J2〜J4 と LSP で
足した分岐が `def_name_at`・`check_command`・`take_option` を閾値（7）の上へ押し出して
いたので、3 つとも切り出して下へ戻した（`5de6ccf`）。

### 着地後の全走（2026-09-21）

着地は PR #19（squash、`26b0c3c`）で main に入った。PR の `gates` は core 層を回し、
21 分 32 秒で緑（run 35575986680）。着地後の main の push では CI が full 層を回し、
32 分 40 秒で `success`（run 35582189880）。`red: none`。

```
$ gh run list --branch main --limit 1
completed  success  feat: hado lsp で診断・hover・code action を返す（lsp-minimal） (#19)
  ci  main  push  35582189880  32m40s  2026-09-21T09:14:59Z

$ git log --oneline origin/main -1
26b0c3c feat: hado lsp で診断・hover・code action を返す（lsp-minimal） (#19)
```

着地の前に手元でも full 層を通した。直列（`--jobs 1`）で 3783/3783、`red: none`。

```
$ bash scripts/run-gates.sh --jobs 1 --tier full <out>
TALLY: 3783/3783 passed; red: none
```

並列（既定の jobs）で先に回したときは `verify-gate-cache`・`verify-m1-corpus`・`verify-m7`
の 3 本が赤になったが、3 本とも単独では緑で、`verify-m1-corpus` の検査 7 が開始時に
`docs` と `scripts` を固定するためだった（同じ木を並列の門番が触る。AGENTS.md の
「門番の走行中は木を触らない」）。CI は並列でも緑だった。
