# 最小の LSP（lsp-minimal）
優先度: 中。

## 現象

配管は既にある。`hado check --format json` は 1 行 1 診断で `canonical` を持ち（§58.4）、`--print-type=<名前>` は最上位の `def` の型を出す（§58.3）。

LSP にすると見える設計上の穴が 3 つある（2026-09-17）。

1. 診断は常に 1 件。E305・E513・E505 を 1 ファイルに置いても E305 だけが出る。
2. 位置は開始点だけ。JSON に `line` `col` はあるが終端が無く、LSP の range が作れない。
3. hover の型は、ファイルに誤りが無いときの最上位の `def` だけ。局所束縛を聞くと診断が返る。E505 で止まっている間は型が見えない。

## 原因

各段階は診断を `Vec` に集めてから `min_by_key(diagnostic_order)` で 1 件を選ぶ（`crates/hado/src/lib.rs:213-233`、`check.rs:27`）。E3xx は最初の致命的な型エラーで `Err` を返し（`infer.rs:258`）、E1xx は parser に回復が無い。型付け結果 `Checked` は E4xx・E5xx・E6xx で止まっているときも手元にあり、`lib.rs:231` が診断があれば捨てる。

対象エディタは Neovim（0.12.5、`vim.lsp.config` / `vim.lsp.enable`）。offline の cargo cache に `lsp-server` `lsp-types` `tower-lsp` は無く、`serde_json` はある。

## 決定（`docs/plan.md` 35 節 L1〜L4）

- `--show-all` を opt-in で足す。同じ段階の全件をアンカー順。E1xx と E3xx の致命は 1 件のまま。既定は変えない。
- 診断で止まっていても型を返す。ライブラリの入口を「診断の列と `Option<Checked>`」を返す形にする。E1xx・E2xx・E3xx の致命で止まったときは型が無い。
- `hado lsp` サブコマンドで JSON-RPC を `serde_json` で書く。

## 案

- token と構文要素の span は `SourceId` と、その source 内の UTF-8 byte offset の開始・終端を持つ。project loader は source ごとの URI と本文を source map に保持し、複数ファイルの AST を結合した後も span から元の source を一意に引けるようにする。`--format json` はそこから 1-based の開始・終端行列を出し（§58.4）、LSP は初期化時に交渉した UTF-8・UTF-16・UTF-32 の position encoding に変換して 0-based range を返す。未交渉なら LSP の既定どおり UTF-16 とする。現在の `lexer::Span` は開始の行列しか持たないので、そのまま終端には使わない。
- 機能は診断・hover・「正準形へ置換」の code action の 3 つ。code action は `check --fix` と同じ関数を呼ぶ。補完は入れない。
- サーバは URI ごとに version と本文を保持する。`didOpen` の全文で開き、`didChange` は `TextDocumentSyncKind::FULL` の全文変更を version の昇順で反映する。開いている文書はディスクを読み直さず、古い version の変更と解析結果を公開しない。`didClose` ではその URI の本文と version を捨て、その URI を参照する保留中の解析結果を無効化する。
- hover の第一段はカーソル下の最上位 `def` の型に限る。局所束縛と式の型は、infer が span から型への対応を保持しているか未確認なので、調べてから決める。
- Neovim 側は `vim.lsp.config("hado", { cmd = { "<checkout>/scripts/hado-lsp.sh" }, filetypes = { "hado" } })` と `vim.lsp.enable("hado")` の数行。`scripts/hado-lsp.sh` は自分の位置からリポジトリ根を求め、`cargo run --offline -q -- lsp` を実行する。`AGENTS.md` の環境に `hado` のインストールは無いので、PATH 上の `hado` は使わない。

## 完了条件

- `nvim --headless` で `.hd` を開き、保存せずに本文を変更してから、診断・hover・code action の 3 つが新しい version の本文を参照することを判定する。ディスク上の本文は変更前のままとし、ディスクの再読込だけでは通らない門番にする。
- 門番は `README` の設定の `cmd` を checkout の絶対パスに置き換えて `nvim --headless` を走らせる。`target` の実行ファイルを `PATH` に足さず、`hado` という名前の実行ファイルにも依存しない。
- 同じ非致命段階の診断を 2 件以上持つ 1 ファイルで、`check --show-all --format json` と LSP の diagnostics が同じ全件をアンカー順に返す。E1xx と E3xx の複数欠陥を置いた対照は 1 件だけを返し、`--show-all` の無い CLI も 1 件のまま。
- 入口ファイルと下位 source 2 つに同じ非致命段階の診断を、行番号が逆になるように置く。`check --show-all --format json` と LSP の diagnostics が §50 のソース順（入口ファイルが先、残りは根からの相対パスのバイト順、同じファイル内は行・列）で並ぶ。source 内の offset だけで並べる実装では通らない。
- ASCII、BMP の多バイト文字、BMP 外の文字を診断の前に置いた対照に加え、対象のトークン自身が BMP の多バイト文字と BMP 外の文字を含む対照（文字列リテラルと名前）を置く。UTF-8・UTF-16・UTF-32 それぞれで、診断と置換の range の開始と終端が対象構文を指し、終端が開始 + byte 長になっていない（UTF-16・UTF-32 の幅を数えている）ことを判定する。
- 入口ファイルと別の source の同じ byte offset に診断・hover・code action の対象を置き、各結果が正しい URI と source 内 range を返す。2 つの source を `didOpen` したまま片方に未保存の変更を入れ、その変更が他方を入口にした診断・hover・code action に現れる（問い合わせた URI 以外をディスクから読み直す実装では通らない）。未保存の変更を持つ側を `didClose` した後は、その source を入口にした診断・hover・code action がディスクの本文を使う（閉じた overlay を残す実装では通らない）。
- 型付け後に止まる E4xx・E5xx・E6xx の代表として E402・E505・E602 を用意する。E602 は早期に `Err` を返す経路（`crates/hado/src/infer.rs:613` の missing signature member で、`tests/reject/E602` の入力）を使い、`crates/hado/src/infer.rs:1849` の遅延される経路だけで通さない。それぞれで `check --print-type=<名前>` と hover が同じ診断コードと同じ推論型を返す。E1xx・E2xx・E3xx では型を返さない対照も置く。

## 由来

`docs/wants.md` 2、7（1 件報告）、14、15。§56、§58.3、§58.4。highlight-treesitter の後。
