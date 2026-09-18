# 最小の LSP（lsp-minimal）
優先度: 中。

## 現象

配管は既にある。`hado check --format json` は 1 行 1 診断で `canonical` を持ち（§58.4）、`--print-type=<名前>` は最上位の `def` の型を出す（§58.3）。`lsp` の入口は無い。

```
$ cargo run --offline -q -- --help
usage: hado check [--print-type[=<name>]] [--fix] <file> | dump-mono <file> | build [-o <path>] [--fix] <file.hd> | run <file.hd> [-- <arg>...] | fmt [--check] <file.hd> | test <file.hd>
```

LSP にすると見える設計上の穴が 3 つある（2026-09-17）。

1. 診断は常に 1 件。E305・E513・E505 を 1 ファイルに置いても E305 だけが出る。
2. 位置は開始点だけ。JSON に `line` `col` はあるが終端が無く、LSP の range が作れない。
3. hover の型は、ファイルに誤りが無いときの最上位の `def` だけ。局所束縛を聞くと診断が返る。E505 で止まっている間は型が見えない。

## 原因

各段階は診断を `Vec` に集めてから `min_by_key(diagnostic_order)` で 1 件を選ぶ（`crates/hado/src/lib.rs:213-233`、`crates/hado/src/check.rs:27`）。E3xx は最初の致命的な型エラーで `Err` を返し（`crates/hado/src/infer.rs:258`）、E1xx は parser に回復が無い。型付け結果 `Checked` は E4xx・E5xx・E6xx で止まっているときも手元にあり、`crates/hado/src/lib.rs:231` が診断があれば捨てる。

対象エディタは Neovim（0.12.5、`vim.lsp.config` / `vim.lsp.enable`）。offline の cargo cache に `lsp-server` `lsp-types` `tower-lsp` は無く、`serde_json` はある。

## 決定（`docs/plan.md` 35 節 L1〜L4）

- `--show-all` を opt-in で足す。同じ段階の全件をアンカー順。E1xx と E3xx の致命は 1 件のまま。既定は変えない。
- 診断で止まっていても型を返す。ライブラリの入口を「診断の列と `Option<Checked>`」を返す形にする。E1xx・E2xx・E3xx の致命で止まったときは型が無い。
- `hado lsp` サブコマンドで JSON-RPC を `serde_json` で書く。

## 案

- token と構文要素の span は `SourceId` と、その source 内の UTF-8 byte offset の開始・終端を持つ。project loader は source ごとの URI と本文を source map に保持し、複数ファイルの AST を結合した後も span から元の source を一意に引けるようにする。`--format json` はそこから 1-based の開始・終端の行列を出し（§58.4）、`line` と `col` の単位は既存のまま変えない。`col` は `crates/hado/src/lexer.rs:236-244` と同じく Unicode スカラ値の数（1-based）で、UTF-8 の byte 数でも UTF-16 の符号単位でもない。`end_line` と `end_col` も同じ単位にする。`end_col` は字句の `bump` と同じく終端の次のスカラの列で、対象のスカラ数は `end_col - col` である。LSP は初期化時に交渉した UTF-8・UTF-16・UTF-32 の position encoding に変換して 0-based range を返す。client は `params.capabilities.general.positionEncodings` で候補を出す。初期化の応答は選んだ符号化を `result.capabilities.positionEncoding` に 1 つの文字列（`utf-8` / `utf-16` / `utf-32`）として advertisement し、client から送る位置も同じ符号化で復号する。未交渉なら LSP の既定どおり UTF-16 とする。現在の `lexer::Span` は開始の行列しか持たないので、そのまま終端には使わない。
- 機能は診断・hover・「正準形へ置換」の code action の 3 つ。code action は `check --fix` と同じ関数を呼ぶ。補完は入れない。
- サーバは URI ごとに version と本文を保持する。`didOpen` の全文で開き、`didChange` は `TextDocumentSyncKind::FULL` の全文変更を version の昇順で反映する。開いている文書はディスクを読み直さず、古い version の変更と解析結果を公開しない。`didClose` ではその URI の本文と version を捨て、その URI を参照する保留中の解析結果を無効化する。
- `.hdi` も `hado` として開かれるので、開いた契約の URI でも診断・hover・code action を返す。入口の候補は、開いている `.hd` だけにしない。契約を単独で開いたとき（`.hd` が 1 つも開かれていないとき）は、その契約の URI を最も長く接頭辞として含む `workspaceFolders` の `uri` を根とする（複数の workspace folder を持つ 1 つの LSP でも、契約が属する folder を選ぶ）。接頭辞になる folder が無いときは `initialize` の `rootUri`、それも無ければ `workspaceFolders` の最初の `uri` を根とし、その下の `.hd` をディスクから列挙して、相対パスのバイト順（`crates/hado/src/project.rs:247` の `source_paths` と同じ順）で入口の候補にする。`rootUri` も `workspaceFolders` も無いときは、契約の祖先を上へ辿り、`library` 宣言を持つ最も外側のディレクトリ（無ければ契約の親ディレクトリ。`crates/hado/src/project.rs:184` の `root_directory`）を根にする。根を契約の親に固定すると、`tests/accept/namespaces/store/store.hdi` のような入れ子の契約が、`tests/accept/namespaces/main.hd` を入口にする project（隣の `id` namespace を読む）を取り落とすので、根は契約の親より上を取りうる。`library` 宣言を持つ `.hd` は実行の入口（`build`・`run`）にはできないが、解析の根にはできる。§52（`docs/spec.md:1797-1799`）は `main` の無い `.hd` を `hado check` がライブラリとして検査し、宣言したディレクトリが独立した根になると定めるので、候補から外さず解析根として `Project::load` する。1 つの契約は §47 により複数の実装が満たせるので、契約の URI の diagnostics は、その契約を読み込む候補の解析の合併を、URI・開始行・開始列・コードで重複を除き、その順に並べて返す（候補の順序と個数で結果が変わらない）。hover と code action は、その契約を読み込む候補のうち相対パスのバイト順で最初のものを選び、その結果を返す。候補が 1 つもその契約を読み込まないときだけ、その URI に空の diagnostics を返す。
- hover の第一段はカーソル下の最上位 `def` の型に限る。局所束縛と式の型は、infer が span から型への対応を保持しているか未確認なので、調べてから決める。
- Neovim 側は `vim.lsp.config("hado", { cmd = { "<checkout>/scripts/hado-lsp.sh" }, filetypes = { "hado" } })` と `vim.lsp.enable("hado")` の数行。`scripts/hado-lsp.sh` は自分の位置からリポジトリ根を求め、`cargo run --offline -q -- lsp` を実行する。`AGENTS.md` の環境に `hado` のインストールは無いので、PATH 上の `hado` は使わない。

## 完了条件

- `nvim --headless` で `.hd` を開き、保存せずに本文を変更してから、診断・hover・code action の 3 つが新しい version の本文を参照することを判定する。ディスク上の本文は変更前のままとし、ディスクの再読込だけでは通らない門番にする。誤りを含む未保存の本文から最後の診断を消す version の編集を送り、その URI に空の diagnostics が publish されることも測る（空を送らない実装では、直した後も Neovim に診断が残る）。
- version の追い越しで古い解析結果を公開しないことも測る。version N に多くの欠陥を、version N+1 に少ない欠陥を置き、`didChange` を N → N+1 と続けて送る。N+1 の解析を先に完了させ、そのあと N の解析を完了させる（解析の完了順を門番が決める。公開の前に version を見ない実装では N の診断が後から出る）。そのあと Neovim が受け取った diagnostics が N+1 のものであり、N の診断が 1 件も publish されず、遅れて返った N の hover と code action も返らないことを判定する。最後に publish された diagnostics の `version` が N+1 であることも測る。完了順を入れ替えても同じ結果になること（N+1 を先に、N を後に完了させる 1 通りと、N を先に完了させてから N+1 を完了させる 1 通り）を両方測る。最新の version でだけ解析を起動し、遅れて返った結果を捨てる実装では通らない。
- 門番は `README` の設定の `cmd` を checkout の絶対パスに置き換えて `nvim --headless` を走らせる。`target` の実行ファイルを `PATH` に足さず、`hado` という名前の実行ファイルにも依存しない。
- Neovim がサーバを止めるときの `shutdown` 要求と `exit` 通知も門番に通す。`shutdown` に `null` の応答が返り、そのあとの `exit` 通知で子プロセスが自分で終了する（終了コード 0）ことを、`jobwait` が timeout せず、kill を送らずに判定する。`shutdown` に応答しない実装、`exit` で終了しない実装、`shutdown` の前に `exit` で終了する実装では通らない。`didClose` のあとも `shutdown` まで解析を続ける対照を置き、終了の順序が §58 の lifecycle と一致することを測る。
- 契約（`.hdi`）を開いた対照でも診断・hover・code action が働く。契約を含む根を選び、契約の URI に診断が出ることを測る。その契約を編集した未保存の本文（member の型や名前を変える編集）が、その契約を使う `.hd` を入口にした診断に現れ、契約の URI の診断も新しい version に従うことを測る。契約だけを開いた対照（`.hd` のバッファを 1 つも開かない。Neovim が `.hdi` を `hado` にする既定の経路そのもの）でも、workspace 根の下の `.hd` を入口の候補として、その契約の URI に診断が出ることを測る。入れ子の契約だけを開く対照も置く。`tests/accept/namespaces/store/store.hdi` を単独で開き、祖先の `tests/accept/namespaces/main.hd` を入口にする project（隣の `id` namespace を読む）が選ばれ、その契約に触る member の診断・hover・code action がその契約の URI に返ることを測る。契約の親だけを根にすると、この入口は `store/` の下に無いので選べず、`store/` の中の `.hd` だけでは `id` namespace を解決できないので通らない。`library` 宣言だけを持つ `.hd` と契約の `.hdi` の 2 ファイルからなるライブラリの対照も置く。契約を単独で開いたときは、その `.hd` を解析根（§52 の独立した根）にして契約の URI に診断・hover・code action が出ることを測る。`library` 宣言を持つファイルを候補から外す実装、`library` 根を `build` の入口として扱って E206 にする実装では通らない。複数の `workspaceFolders` を持つ 1 つの LSP の対照も置く。2 つの folder を `initialize` で渡し、2 つ目の folder の契約を単独で開いたとき、2 つ目の folder の下の `.hd` が入口の候補になり、契約の URI に診断・hover・code action が出ることを測る。最初の folder（または `rootUri`）だけを検索する実装は候補 0 で空を返すので通らない。§47 が許す複数の実装（同じ契約を満たす 2 つの `implement` と、その間で型が食い違う member）と、同じディレクトリに複数の入口候補を置いた対照でも測る。契約の URI の diagnostics が、その契約を読む候補の合併を重複除去して並べたものと一致し、候補の `didOpen` の順序を入れ替えても、片方だけを開いても、契約だけを開いても同じ結果になることを判定する。hover と code action は相対パスのバイト順で最初の候補の結果になる。契約の URI を無視する実装、契約を入口ファイルとして CLI に渡そうとする実装、`.hdi` の診断をディスクの本文のままにする実装、開いた順で入口を選ぶ実装、最初に見つけた実装だけを読む実装、候補を開いている `.hd` だけに限る実装では通らない。
- 同じ非致命段階の診断を 2 件以上持つ 1 ファイルで、`check --show-all --format json` と LSP の diagnostics が同じ全件をアンカー順に返す。E1xx と E3xx の複数欠陥を置いた対照は 1 件だけを返し、`--show-all` の無い CLI も 1 件のまま。
- 入口ファイルと下位 source 2 つに同じ非致命段階の診断を、行番号が逆になるように置く。`check --show-all --format json` と LSP の diagnostics が §50 のソース順（入口ファイルが先、残りは根からの相対パスのバイト順、同じファイル内は行・列）で並ぶ。source 内の offset だけで並べる実装では通らない。
- ASCII、BMP の多バイト文字、BMP 外の文字を診断の前に置いた対照に加え、対象のトークン自身が BMP の多バイト文字と BMP 外の文字を含む対照（文字列リテラルと文字リテラル）を置く。名前は `crates/hado/src/lexer.rs:430` と `:539` により ASCII だけなので使わない。UTF-8・UTF-16・UTF-32 それぞれで、診断と置換の range の開始と終端が対象構文を指し、終端が開始 + byte 長になっていない（UTF-16・UTF-32 の幅を数えている）ことを判定する。同じ対照を `check --format json` でも測る。`col` と `end_col` の期待値は判定器側で、対象行を `crates/hado/src/lexer.rs:236-244` と同じく Unicode スカラごとに 1 進めて独立に数える（1-based）。`col` は対象トークン先頭のスカラ列、`end_col` は次のスカラの列で、`end_col - col` は対象トークンのスカラ数である。BMP の多バイト文字（`あ` はスカラ 1・UTF-8 で 3 バイト）と BMP 外の文字（`😀` / 128512 はスカラ 1・UTF-8 で 4 バイト・UTF-16 で 2 単位）の両端をこの数と照合する。JSON の列を byte 数や UTF-16 の符号単位にする実装、終端を含む列（`end_col - col` がスカラ数より 1 小さい）にする実装、実装の出力から期待値を読む実装では通らない。加えて、行をまたぐ range の期待値は判定器側で、ソース本文の行番号から独立に数える（1-based）。入力は 3 行以上の 1 構文とし、`tests/reject/E507/main.hd:3` の `def f` から `:5` の `| None => None` までを置く（`crates/hado/src/check.rs:531` の診断は `node.span` の分岐全体、`crates/hado/src/fixes.rs:990` の code action も同じ範囲を `def f value => value` に書き換える）。診断と code action の両方で、開始行が `def f`、終了行が `| None => None` であり、`end_line - line` が対象の行数より 1 小さいことを、`check --format json` と LSP の両方の経路で照合する。E508 の診断アンカーは再伝播する腕の `|`（`crates/hado/src/check.rs:34` の `arm_diagnostic`、1 行）なので、E508 の診断や `end_line > line` だけでは足りない。`end_line` に `line` や `line + 1` を写す実装、終了を次の行にする実装、開始行だけを返す code action、実装の出力から終了行を読む門番、片方の経路だけでは通らない。非既定の encoding（`utf-8` と `utf-32`）で range を変換する対照は、それぞれ別の initialize で `params.capabilities.general.positionEncodings` にその符号化だけを出す。判定器側の期待値は、initialize の `result.capabilities.positionEncoding` が同じ文字列であること（1 つの文字列。配列でも `general.positionEncoding` でもない）。range の変換もその同じ符号化で、既存の独立な幅の数えに従う。advertisement を欠く実装、`general` の下や配列に置く実装、応答と変換で別の encoding を使う実装、実装の advertisement を読んで期待値にする門番では通らない。client から送る位置の期待値は判定器側で、対象行の先頭から対象トークン直前までを交渉した encoding の符号単位で独立に数える（0-based）。同じ行に `def decoy = "あ` と `😀` 15 個と `" def target (b: Bool) => if b | true => 1 | false => 0` を置く。接頭 `あ` と `😀` 15 個は UTF-8 で 63、UTF-16 で 31、UTF-32 で 16 である。`target` の名前の列は `def decoy = "`（13）と接頭と `" def `（6）の和（UTF-8 82、UTF-16 50、UTF-32 35）であり、hover はその列に送り、返る型は `check --print-type=target` と同じで `decoy` の型ではない。code action の request range は `if b | true => 1 | false => 0` だけを同じ数えで覆い（開始は UTF-8 102、UTF-16 70、UTF-32 55、終了は UTF-8 131、UTF-16 99、UTF-32 84）、返る書き換えは `if b => 1 | 0` である。`あ😀` 1 組だけ（UTF-8 の `target` 列を UTF-16 として読んでも名前 `target` に当たる）、行頭からの広い request range（誤復号でも `if` と交わる）、実装の診断 range から送る列を読む門番、送る位置を常に UTF-16 として復号する実装では通らない。
- 入口ファイルと別の source の同じ byte offset に診断・hover・code action の対象を置き、各結果が正しい URI と source 内 range を返す。code action は返した edit の range と newText を現在の version の本文に適用し、その結果を、同じ version の本文に対する `hado check --fix` の書き換え後のソースとバイト一致させる。照合する本文は局所的な書き換えを 1 つだけ持ち、`--fix` の適用後が適用前と異なる（正準形の入力では空の edit が通り、ディスク上の本文との照合では overlay 向けでない古い置換も通る）。2 つの source を `didOpen` したまま片方に未保存の変更を入れ、その変更が他方を入口にした診断・hover・code action に現れる（問い合わせた URI 以外をディスクから読み直す実装では通らない）。未保存の変更を持つ側を `didClose` した後は、他方を入口にした診断・hover・code action が閉じた source のディスク本文を使う（閉じた overlay を残す実装では通らない）。解析のたびに、以前に非空の diagnostics を publish した開いている URI のうち新しい結果に現れないものへ空の diagnostics を publish する。2 つの source を `didOpen` したまま、入口ではない側に未保存の誤りを置く。判定器は誤りの本文を持つ source の URI を入力から特定し、その誤りの側の URI へ非空の diagnostics が一度 publish されることを測る。誤りの側だけを直す編集か誤りの側の `didClose` を送り、入口側は編集せず開いたままにする。その同じ誤りの側の URI へ空の diagnostics が publish されることを測る（サーバが最初に非空を出した URI を記録するだけでは、入口側へ非空と空を送る実装が通る。診断が付いた側を特定せず入口側だけに空を送る実装では通らない）。
- 型付け後に止まる E4xx・E5xx・E6xx の代表として E402・E505・E602 を用意する。E602 は早期に `Err` を返す経路（`crates/hado/src/infer.rs:613` の missing signature member で、`tests/reject/E602` の入力）を使い、`crates/hado/src/infer.rs:1849` の遅延される経路だけで通さない。それぞれで `check --print-type=<名前>` と hover が同じ診断コードと同じ推論型を返す。E1xx・E2xx・E3xx では型を返さない対照も置く。

## 由来

`docs/wants.md` 2、14、15（1 件報告は `docs/plan.md` 35 の L1）。§56、§58.3、§58.4。highlight-treesitter の後。
