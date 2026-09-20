# 最小の LSP（lsp-minimal）

開始 2026-09-21。着手時の main は `fb1c29d`。

## 手順書

### 背景

v0.5 の 3 本目（`docs/plan.md` 35 節の L1〜L4）。利用者は言語設計者本人で、エディタは
Neovim 0.12.5。`hado check` の配管（`--format json`、`--print-type`）は既にあり、`lsp` の
入口が無い。LSP にして初めて見える穴が 3 つある。診断が常に 1 件、位置が開始点だけ、
診断で止まると型が無い。根拠は `docs/issues/lsp-minimal.md` の現象欄と、§56（段階順と
1 件報告）、§58（CLI）、§58.3（型の印字）、§58.4（機械可読な診断）。

### ゴール

- Neovim 0.12.5 が `.hd` と `.hdi` を開くと、`hado lsp` から診断・hover（最上位の `def` の
  型）・正準形への code action が届く。補完は無い。
- CLI 側の 3 つの穴が埋まっている。`check --show-all` が同じ段階の全件をアンカー順に出し
  （E1xx と E3xx の致命は 1 件のまま、既定と拒否コーパスの判定は不変）、`--format json` に
  終端の位置があり、網羅性・冗長・単相化で止まっていても `--print-type` と hover が型を
  返す。
- `scripts/verify-lsp-minimal.sh` が `docs/issues/lsp-minimal.md` の完了条件を測って緑で、
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

### 工程

builder が進めた順と、工程ごとに回した門番をここに書く。着地の作法は AGENTS.md の
「着地の直前に main を取り込み、lock を作り直す」に従い、着地後の full 層は CI（push to
main）の run を実測に書く。

1. `docs: lsp-minimal のラウンドを書く`（`4674532`、着手時の契約）。
2. `cargo fmt --all` / `cargo test --offline` を回しながら、`Span` の終端（D1〜D5）と
   `Project` の本文保持（D6）を入れる。
3. `--show-all`・`--format json` の終端・`--print-type` の型（J2〜J4）。
4. `hado lsp`（J5〜J10）。
5. 門番 `scripts/verify-lsp-minimal.sh` と仕様 §58 系・`README.md`。

### 仕様への疑義

builder が見つけた疑義をここに書く（上の「前提のずれ」は計画時のもの）。

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
