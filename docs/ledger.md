# 実行台帳 v0.1 → v0.1.2

`docs/plan.md` を code-perfection-multiagent で実行した記録。ラウンド単位で追記する。

## 役割割当

| 役割 | エージェント | モデル | 実行方式 |
|---|---|---|---|
| commander | claude（本セッション） | claude-opus-5[1m] | — |
| planner | cursor-agent | cursor-grok-4.6-xhigh | headless（`-p --force --trust`）|
| plan-reviewer | opencode | opencode/deepseek-v4-flash | headless（`run --dir`）|
| builder | opencode | opencode/deepseek-v4-flash | headless（Herdr ペイン内で実行）|
| reviewer | cursor-agent | cursor-grok-4.6-xhigh | headless（`-p --force --trust`）|
| fixer | opencode | opencode/deepseek-v4-flash | builder と同系 |

ユーザー明示指定に従う。スキル既定からの逸脱と理由:

- planner を自分ではなく cursor に割り当てた（スキルは「planner は常に自分」）。ループの指揮・戻り先の最終判定・ラウンド分割は commander が握る。
- muse spark は `-contributor-free` の版を使う（ユーザー指定・2026-09-06）。R1 は `opencode/muse-spark-1.3` で実施済み。
- **builder は常に muse spark**（ユーザー指定・2026-09-06）。planner の cursor は plan と spec を書いてよい。
- plan-reviewer を deepseek-v4-flash から muse spark へ、reviewer を deepseek-v4-flash から cursor-grok-4.6-xhigh へ変更（ユーザー指定・2026-09-06）。R1 は変更前の割当で実施済み。
- **R4 の途中で muse spark がレート制限に達したため、builder / fixer / plan-reviewer を deepseek-v4-flash へ戻した**（ユーザー指定・2026-09-06）。R2・R3 と R4 の計画審査は muse spark で実施済み。独立性は保たれる（builder=deepseek、reviewer=cursor）。
- **審査は成果物の種類で分ける**: plan-reviewer（muse spark）が計画側の成果物（PLAN.md と spec 本文）を、reviewer（cursor）が実装側の成果物（コード・コーパス）を見る。こう切ると、planner の cursor が plan と spec の両方を書いても、審査は常に別ベンダーになる。
- R1 時点では builder と reviewer が同じ CLI（opencode）だった。「別ベンダー」の文面には反するが、意図（盲点の非重複）はモデル系列が別（muse-spark 1.3 / deepseek v4 flash）で満たすと判断した。R2 以降は reviewer が cursor になり、CLI も分かれる。

## 環境の実測（2026-09-06）

| 項目 | 結果 |
|---|---|
| rustc / cargo | 1.98.0 |
| cc | gcc 13.3.0 |
| libgc-dev | 1:8.2.6-1build1 導入済み（`/usr/include/gc.h`）|
| cursor-agent | 2026.09.02。`-p`（非対話）と `--trust` があり Trust ゲートを回避できる |
| opencode | 1.18.29。scratchpad への書込を実測、判定行も stdout に出た |
| 計画 §4 の probe コード | リポジトリにも `~/git` にも無い。Cranelift 0.135 の API 差は PLAN.md に注記して builder へ渡す |

## ラウンド記録

### R1 — M0 環境と歩く骨格

対象: `docs/plan.md` M0（Cargo ワークスペース、C ランタイム、`def main = 1 + 2` の歩く骨格、手組み CLIF の 1 億深さ）

| 段階 | 担当 | 結果 | 所要 |
|---|---|---|---|
| 計画 1 回目 | planner (cursor-grok-4.6-xhigh) | PLAN.md 初版 | 約 9 分 |
| 計画審査 1 回目 | plan-reviewer (deepseek-v4-flash) | **不合格** | 約 8 分 |
| 計画 2 回目 | planner | 根で修正 | 約 7 分 |
| 計画審査 2 回目 | plan-reviewer | 機構的ロス（判定行なし） | — |
| 計画審査 3 回目 | plan-reviewer | 注入 7/7 検出を実測。残穴 1 件を発見後に停止 | — |
| 計画の最終確定 | commander | 残穴を PLAN.md に反映して上書き承認 | — |

**plan-review 1 回目の不合格（実質 1 回。予算 4 のうち 1 消費）**

指摘 3 件、根は 1 つ — 判定コマンドが語の存在しか見ず、条件を満たさない実装でも終了コード 0 を返す。plan-reviewer は実際にプローブを作って 3 件とも実証した:

- 条件 6: 全フラグを逆の値にした `emit.rs` でも `rg` が exit 0
- 条件 7: `shin_alloc` と `GC_malloc` が無い `runtime/main.c` でも `grep -E 'A|B|C|D'` が exit 0（any-of）
- 条件 5: `cranelift-codegen = "0.134"` だけ・`cc` 無しの `Cargo.toml` でも exit 0

個別に継ぎ足さず根で直させた: 判定を `scripts/verify-r1.sh` 1 本に集約し、any-of 禁止・値の照合・要素ごとのループ・`OK:`/`NG:`/`PASSED: n/7` を仕様として明記。条件ごとの「これを壊す最小の変更」7 つを PLAN.md に列挙し、その注入で対応チェックが NG になることを builder の義務にした。

**機構的ロス（審査 2 回目・3 回目）**

opencode が `--dir` の外（`/tmp/vr1_pristine`）に復元用コピーを作ろうとして自動拒否され、判定行を出さずに停止した。原因は発注側 — 「`git checkout -- .` で戻せ」と書いたが、reviewer が作る足場は untracked なので戻らず、逃げ場を外に求めて詰まった。3 回目は「足場を作った直後に `git add -A && git commit` して基準点を作る」を手順に書いて解消。契約で禁じるのではなく、コピーの中に復元点を作る構造にした。

3 回目は注入 7/7 がすべて対応チェックで `NG` になることを実測し、そのあと残穴を 1 件見つけて停止した:

- 条件 4 のチェックは深さ定数を grep するだけなので、**定数が死んだまま実際の深さが `1`** でも `OK` になる（reviewer 自身の分類は「同種の深掘り」）

**指揮者の上書き承認**

残穴は同種の深掘りであり、修正は計画（ドキュメント）の範囲。4 回目の往復を回さず、commander が PLAN.md を直して承認した。直した内容: 手組み CLIF の相互再帰にカウンタを持たせ、**実際に降りた段数を印字**し、`tail.rs` が stdout と深さ定数の一致を assert する。これで死んだ定数はテストを通せない。注入 4 と審査強度の注入例 3 も同じ形に書き直した。

**commander が一次資料で裏を取った項目**

plan-reviewer は `--dir` の外を読めないため Cranelift API の主張を確かめられない。commander が `~/.cargo/registry/.../cranelift-*-0.135.1` と照合し、9 項目すべて一致を確認した:

| 主張 | 照合結果 |
|---|---|
| `FunctionBuilder::finalize(mut self, frontend_config: TargetFrontendConfig)` | 一致（`cranelift-frontend/src/frontend.rs:704`）。docs.rs/latest は引数なしなので見ると型エラーになる |
| `Module::target_config -> isa.frontend_config()` | 一致（`cranelift-module/src/module.rs:839`）|
| `cranelift_native::builder() -> Result<isa::Builder, &'static str>` | 一致（`cranelift-native/src/lib.rs:16`）|
| `isa::Builder::finish(&self, Flags) -> CodegenResult<OwnedTargetIsa>` | 一致。`IsaBuilder<T>::finish -> T` と `type Builder = IsaBuilder<CodegenResult<OwnedTargetIsa>>`（`isa/mod.rs:225, 166`）|
| `OwnedTargetIsa = Arc<dyn TargetIsa>` | 一致（`isa/mod.rs:163`）|
| `ObjectBuilder::new(OwnedTargetIsa, name, libcall_names) -> ModuleResult<Self>` | 一致（`cranelift-object/src/backend.rs:50`）|
| `CallConv::Tail`。`supports_tail_calls` が true なのは Tail だけ | 一致（`isa/call_conv.rs`）|
| verifier: callee の規約が caller と一致かつ tail 対応、戻り型も一致 | 一致（`verifier/mod.rs` の `typecheck_tail_call`）|
| 既定 `opt_level=none` `enable_verifier=true` `is_pic=false` `preserve_frame_pointers=false`、`iadd_imm`/`icmp_imm` は deprecated | 一致（`settings.rs:491-510`、`codegen-meta/src/gen_inst.rs:1349-1461`）|

**実装フェーズ**

| 段階 | 担当 | 結果 | 所要 |
|---|---|---|---|
| 実装 | builder (muse-spark-1.3) | `PASSED: 7/7` 到達。注入検証の直前で停止 | 約 15 分 |
| 実装（継続） | builder | 注入 7/7 すべて対応チェックが NG になることを実測 | 約 8 分 |
| レビュー | reviewer (deepseek-v4-flash) | 必須注入 3 件を検出。独自注入で穴 2 件を発見後に停止 | 約 20 分 |
| レビュー（継続） | reviewer | **合格** | 約 3 分 |
| 判定力の修正 | fixer (muse-spark-1.3) | walk を 3 ケース化。穴 2 件を塞いだ | 約 2 分 |

**reviewer の指摘（合格だが最も価値のある観測）**

独自注入 3 件のうち 2 件が**検出されなかった**:

- 注入 A: `emit.rs` の `Expr::Add` が AST を無視して定数 `3` を返す → walk が通ってしまう
- 注入 B: `runtime` の `shin_print_int` が引数を無視して常に `3` を印字 → walk が通ってしまう

原因は walk テストが単一入力・単一期待値（`def main = 1 + 2` → `"3"`）しか持たないこと。値 `3` をどこかにハードコードすれば通る。「実装は正しいがテストが何も固定していない」型で、受け入れ条件の充足だけを見るレビューでは原理的に検出できない。

合格判定は維持したまま、判定力の穴だけを fixer に閉じさせた（M1 以降の全ラウンドがこの骨格に載るため）。fixer は walk を 3 ケース（`1 + 2`→3、`10 + 20`→30、`4 + 5 + 6`→15）にし、「どんな単一定数でも全ケースを同時に満たせない」性質を持たせた。3 件目は `+` の連鎖も通す。

**commander が自分の環境で実測した結果（役割の主張を根拠にしない）**

| 検証 | 結果 |
|---|---|
| `bash scripts/verify-m0.sh` | `PASSED: 7/7`、終了コード 0 |
| `cargo test` | walk・tail の 2 件通過 |
| 生成された tail 実行ファイルを直接実行 | stdout `100000000`、0.162 秒 |
| **`ulimit -s 1024`（スタック 1 MB）で同バイナリを実行** | 完走。**フレームを積んでいないことの決定的な証拠**。既定 8 MB での完走は「たまたま足りた」可能性を残すが、1 MB で通るなら残らない |
| 注入 A（emit が定数 3）を自分で注入 | walk が落ちる（`left: "3" right: "30"`）|
| 注入 B（印字が引数無視）を自分で注入 | walk が落ちる（同上）|

**結果: 合格**

**後始末**

- `PLAN.md` は削除。ただし**コードから読めない設計知識**である Cranelift 0.135 の呼び出し形を `docs/cranelift-0.135.md` へ切り出した。M6 の `emit` で必ず要り、docs.rs の latest を見ると間違える種類の知識のため。意図・スコープ・受け入れ条件はコミットログと git 履歴が持つので移さない
- `scripts/verify-r1.sh` を `scripts/verify-m0.sh` に改名。"R1" はセッション固有のラウンド ID でリポジトリには無意味なため、`docs/plan.md` の M0 を指す名前にした

**既知の債務（後続ラウンドで整理する。R1 の判定材料にはしない）**

- `walk.rs` / `tail.rs` が生成物を `std::env::temp_dir()`（`/tmp/shin-<name>-<pid>/`）に置き、消さない。実行のたびにゴミが溜まる。サンドボックス内の reviewer から触れない原因でもあった。`target/` 配下に置くのが正しい
- `scripts/verify-m0.sh` のチェック 5〜7 は M0 時点の構造（`emit.rs` にフラグ設定がある等）を grep で固定している。M6 で `repr` / `emit` を書き換えると誤検知になりうる

**機構的ロス（3 回。根は 1 つ）**

plan-reviewer 1 回・builder 1 回・reviewer 1 回が、いずれも作業ディレクトリの外（`/tmp/vr1_pristine`、`/tmp/r1bak`、`/tmp/shin-tail-*`）へ復元用コピーを作る／読むために停止した。根は 1 つ — **欠陥注入を課す役割に、サンドボックス内の復元点を渡していなかった**。

解決は `git add -A` を復元点にすること。`git checkout -- .` はインデックスから作業ツリーを戻すので、コミットを作らずに「注入 → テスト → 復元」が閉じる。以後すべての発注に定型句として入れる。

セッション途中でユーザーが opencode の `permission.external_directory` に `/tmp/**` を追加したため、この経路での停止は以後起きない。ただし `~/git/**` も許可されているので、**reviewer の隔離は構造ではなく契約による**ものになっている（使い捨てコピーの外に出ようと思えば出られる）。

### R2 — M1 前半: 仕様の穴埋め

対象: `docs/plan.md` M1 手順 1（仕様への書き込み）。コーパスと `corpus.rs` は R3。

**ユーザー確認（2026-09-06）**

- 計画 2.3 の推定既定 13 項目は**すべて採用**
- `ArithmeticError` → **`DivisionError`**。v0.1 の variant は `DivisionByZero` だけで、返すのも Int の `/` と `%` だけ。加減乗は wrap するので算術一般のエラーは存在せず、広い名前は嘘になる
- spec 本文は planner（cursor）が書き、コーパスは builder（muse spark）が書く

**commander が計画を書く前に対象を読んで見つけた、計画自体の誤り**

`docs/plan.md:131` は「§38 struct を除外一覧から外す」と指示するが、**§38 の除外一覧に `struct` は無い**（実在するのは functors / first-class modules など）。実際に直すべきは §35（`spec:994`）の「v0.1では `sig` を優先し、実装側の詳細なmodule syntaxは後回し」。読まずに写していれば、実装役が存在しない行を探して往復を消費していた。

同じ確認で、2.2 が引く `spec:1289`（条件列の改行要求）と `spec:965`（`Error` はユーザー定義型）はどちらも正確だった。加えて `spec:1287` の箇条書きも改行に言及しており、**1289 だけを直しても足りない**ことが分かった。

**plan-review（不合格 2 回。予算 4 のうち 2 消費）**

| 回 | 審査者 | 結果 |
|---|---|---|
| 1 | deepseek-v4-flash | 注入 1〜5 の検出を実測。**条件 6 の穴**を実証して停止 |
| 2 | deepseek-v4-flash | 直した条件 6 に**2 つ目の穴**を実証して停止 |
| 3 | muse-spark-1.3-contributor-free | **合格** |

2 回の不合格は根が 1 つ:

1. 1 回目 — 禁止すべき文言を**手書きのリテラル**（`改行された \`| condition\``）で照合していたため、実際の本文（`改行された \`| condition =>\``）と 1 文字ずれて検出できず、改行要求を残したまま `OK` になった
2. 2 回目 — 条件文が「`パーサは改行を見ない` に**相当する**記述」という含みのある表現だったため、実装者が文言を推測してスクリプトを書き、判定がその推測に依存した

根での修正: **禁止の判定と要求の判定は係留先が逆**という規則を検証スクリプトの仕様に置いた。禁止は構造マーカーか語の共起で見る（書き手が言い換えると文言照合は空振りする）。要求は逐語で指定する（「相当する記述」と書くと実装者が推測する）。あわせて各チェックを「何を数え、いくつなら OK か」の 1 行の述語として書くことを義務にした。

**3 回目の審査者が示した、共起述語の原理的な限界**

`条件列` と `改行` の共起は、**同義語**（「`|` は行頭になければならない」）と**行またぎ**（要求を 2 行に分割）ですり抜けられる。どちらも実測ですり抜けが再現した。述語を同義語の列挙で固めるのは同じ根の繰り返しなので採らず、宣言済みの人間 diff レビューで受けることにした。diff を読む側が `行頭` `先頭` `ライン` も見る、と計画に追記した。

**機構的ロス**

使い捨てコピーを `git add -A` で更新したとき、**前任の審査者が書き捨てたスクリプトを一緒にコミット**してしまい、次の審査者がそれを成果物と誤認して指摘した（指摘としては正しい）。コピーは更新のたびに作り直すべきだった。

**実装フェーズ**

| 段階 | 担当 | 結果 | 所要 |
|---|---|---|---|
| 仕様執筆 | planner/implementer (cursor-grok-4.6-xhigh) | `PASSED: 6/6`。新設 §43〜§50、エラーコード 39 件 | 約 8 分 |
| 仕様レビュー | plan-reviewer (muse-spark-1.3-contributor-free) | **合格**。指摘 4 件 | 約 2 分 |
| 修正 | cursor | 指摘 4 件を修正 | 約 5 分 |

**成果物**

- `docs/spec.md`: 1318 → 1591 行。§1〜§42 の見出しは無傷、新設は §43〜§50 の 8 節（リテラルとコメント / 演算子・優先順位・短絡評価 / 構造比較の対象型 / List リテラル / 名前空間 / 末尾呼び出しの保証 / prelude / エラーコード）
- `docs/spec-coverage.md`: 計画 2.2 の 22 項目と 2.3 の 13 項目、計 35 行の対応表。**35 項目のどれかが黙って落ちることを検出可能にする**のが役割で、仕様本文だけを見ても書き忘れは誰にも見えない
- `scripts/verify-m1-spec.sh`: 受け入れ条件 6 件の門番。エラーコード 39 件（E1xx:3 E2xx:3 E3xx:4 E4xx:6 E5xx:16 E6xx:2 E7xx:5）

**仕様レビューの指摘 4 件（判定は合格。commander が R3 を塞ぐと判断して修正させた）**

機械検証は「壊していない・書き漏らしていない」しか見ない。内容の整合は宣言どおり人が diff を読んで拾った。

1. **E602 の発火条件が本文に無い** — 表には `E602 opaque-type §35` があるが、§35 本文は不透明性を性質として述べるだけで「何をしたら E602 か」が無い。他 38 コードは本文にアンカーを持つ。R3 で拒否コーパスを 1 ファイル 1 コードで書くので、接地が無いと E602 のコーパスが書けない
2. **record の文法と例が食い違う** — §41 の `"{" (name "=" expression ";")+ "}"` は各 field の後に `;` を必須にするので、§12 の例 `{ name = "a"; age = 3 }` が自分の文法を通らない。`;` が区切りか終端子かが決まっていなかった。区切り形に直した（決定 `docs/plan.md:46` の例と整合）
3. **module_impl が型定義を許さない** — `struct definition* end` の `definition` は `def` の 2 形式だけで `type` を含まず、§35 の例の `type T = User` が自分の文法を通らない
4. **`pattern` 非終端子の規則が無い** — `pattern_arm` が `pattern` を参照しているのに規則が無く、`record_pattern` と `literal_pattern` がどこからも参照されていない。M2 のパーサがリテラル／record パターンの出現位置を文法から決められない

指摘 2 は commander が独立に diff を読んで見つけたものと一致した。

**commander が自分の環境で実測した結果**

| 検証 | 結果 |
|---|---|
| `bash scripts/verify-m1-spec.sh` | `PASSED: 6/6`、終了コード 0 |
| 注入「改行要求を別の言い回しで書き戻す」 | チェック 6 が NG（`cooccur-条件列-改行=1`）|
| 注入「`# 20.` の見出しを書き換える」 | チェック 1 が NG |
| 注入「対応表から 1 行消す」 | チェック 5 が NG（`rows=34`）|
| 修正後の `E602` の出現 | 2 件（本文 1072 行と表 1587 行）|
| `bash scripts/verify-m0.sh` | `PASSED: 7/7`（R2 で壊していない）|

**結果: 合格**

### R3 — M1 後半: コーパスと corpus.rs

対象: `docs/plan.md` M1 手順 2・3。

| 段階 | 担当 | 結果 | 所要 |
|---|---|---|---|
| 計画 | planner (cursor-grok-4.6-xhigh) | 述語形式の受け入れ条件 7 件 | 約 12 分 |
| 計画審査 | plan-reviewer (muse-spark) | **合格**。すり抜け 2 件を実測して報告（深刻度低）| 約 5 分 |
| 実装 | builder (muse-spark) | 受理 22 / 拒否 39、`corpus.rs`、`verify-m1-corpus.sh`。`PASSED: 7/7` | 約 9 分 |
| レビュー 1 | reviewer (cursor-grok-4.6-xhigh) | **不合格:内部構造**。中身とタグの不一致 11 件 | 約 15 分 |
| 修正 1 | fixer (muse-spark) | 11 件 + 自ら見つけた 7 件 | 約 5 分 |
| 要求側の修正 | commander | §50 に 4 コード追加、計画の条件 1・3 を実態に合わせる | — |
| 修正 2 | fixer | 新設 4 本、タグ付け替え | 約 6 分 |
| レビュー 2 | reviewer | **不合格:内部構造**。被覆のために寄せたタグ 4 件 | 約 10 分 |
| 修正 3 | fixer | タグを動かさず、項目を体現する内容に書き換え | 約 2 分 |
| レビュー 3 | reviewer | **不合格:内部構造**。残 1 件 | 約 12 分 |
| 最終判断 | commander | **上書き承認**（既知の欠陥 1 件つき） | — |

**根（3 回のレビューを通じて 1 つ）**

判定器はタグの**形式と集合**しか見ない。`-- error: E501` のファイルの中身が identity function でも `PASSED: 7/7` が出るが、M4 でそのコードは永遠に発火しない。さらに **1 ファイルに欠陥が 2 つある**と、検査の順序（字句 → 構文 → 型 → 網羅性 → 冗長）で先に別の禁止に当たり、タグのコードまで到達しない。

指摘は 11 → 4 → 1 と収束した。

**要求側にあった根（commander が 2 回目のレビュー後に特定）**

タグの不一致が繰り返し出たのは、**受け入れ条件そのものが正直には満たせなかった**ため。計画 `docs/plan.md:154`（**（推定）**印つき）の「2.2 の各項目に受理と拒否のペア」について:

- `冗長の対象外`（eta は冗長で「ない」という許可）と `末尾呼び出し`（スタックを消費しないという保証）は、**拒否例が原理的に存在しない**
- `リテラル` `コメント` `tuple` `呼び出し構文` は禁止が実在するのに、**§50 にコードが無かった**（E1xx は 3 つだけ）

数を満たすにはタグを近くのファイルへ寄せるしかなく、そのたびにその項目の例が嘘になっていた。commander が要求側を直した:

- §50 に E103 float-literal-form / E104 comment-form / E105 no-tuple / E106 constructor-call-form を追加し、本文にアンカーを付けた（拒否コーパス 39 → 43 本）
- 計画の条件 3 の拒否側を 22 項目 → 20 項目にし、除外した 2 項目と理由を明記した

**成果物**

受理 23 本 / 拒否 43 本 / `crates/shin/tests/corpus.rs` / `scripts/verify-m1-corpus.sh`。

**commander が自分の環境で実測した結果**

| 検証 | 結果 |
|---|---|
| `bash scripts/verify-m1-corpus.sh` | `PASSED: 7/7`、終了コード 0 |
| `verify-m0.sh` / `verify-m1-spec.sh` | `PASSED: 7/7` / `PASSED: 6/6` |
| **列挙が本物か（probe）** | `tests/accept/__probe__.shin` を置くと `corpus_enumerated:` が 61 → 62、消すと 61 に戻る。probe は実際にコンパイル・実行され `corpus_mismatch: expect="probe" got="3"` を報告した |
| 注入「拒否を 1 本消す」 | チェック 1 が NG（`reject-count=38`）|
| 注入「`-- error:` を既出の別コードに書き換える」 | チェック 1 が NG（本数は 39 のまま、集合から欠ける）|
| 注入「`emit.rs` を 1 行変える」 | チェック 7 が NG（`forbidden-diff-lines=1`）|

**結果: 上書き承認（fixer 上限到達）**

fixer の上限 3 回に達した時点で、reviewer の指摘は 1 件に収束していた。スキルの既定は打ち切りだが、次の理由で上書き承認した:

- 指摘は 11 → 4 → 1 と単調に減っており、往復の発散ではなく収束
- 残る欠陥は 66 本中 1 本で、所在も原因も特定済み
- その欠陥は M3（型検査）が動いた瞬間に露見する種類で、静かに腐らない

**既知の欠陥（M3 着手時に直す）**

`tests/reject/E508.shin` は、タグの E508 がそのファイルの最初の欠陥ではない。

```
def f id =>
  if find_user id
  | Ok(user) => user
  | Error(error) => Error(error)
```

`if` は 1 つの型の式なので両枝の型が揃わなければならないが、成功枝は中身の型 `T`、失敗枝は `Result` のまま。検査順（字句 → 構文 → 型 → 網羅性 → 冗長）では**型の不一致が冗長（E508）より先に当たる**ため、このコーパスは M4 で E508 を検証できない。仕様 §24 の例は成功枝が `...`（Result 側の継続）で、コーパスがそこを `user` で埋めたことが原因。

対比: `tests/reject/E507.shin` は両枝とも `Option` で、再包装だけが欠陥になっている。

### R4 — M2 構文: パーサと M2 コーパス

対象: `docs/plan.md` M2。受け入れ条件 1〜8（`docs/plan.md:162-166`）。

| 段階 | 担当 | 結果 | 所要 |
|---|---|---|---|
| 実装 | builder (deepseek-v4-flash) | `PASSED: 8/8` 到達。parser.rs 約 1700 行、`parse_corpus.rs`、`verify-m2.sh` | — |
| レビュー | reviewer (cursor-grok-4.6-xhigh) | **合格**。ただし「壊してもゲートが落ちない」穴を 5 件観測 | — |
| 修正 | fixer (deepseek-v4-flash) | 穴 4 件を単体テストで固定 | — |

**reviewer の観測（合格だが判定力の穴。根は 1 つ）**

コーパスがその構文を使っていないため、実装を消してもどの判定器も落ちない。根は「**実装の存在を固定するゲートが無い**」こと。`tests/**` はスコープ外なので、コーパスを増やすのではなく `parser.rs` の単体テストで固定した。`scripts/verify-m2.sh` の条件 5 は `cargo test` 全体を見るため、以後の回帰は単体テストが捕まえる。

直した 4 件（すべて単体テストで固定、実装を消すと落ちることを実測済み）:

1. §10・§11 の先頭 `|` 付き ADT が parse できない — `parse_type` が最初の variant を name 前提で読んでいた。先頭 `|` の有無どちらも受理するようにした
2. 単項 `not` を消しても通る — `parse_unary` の `KwNot` 分岐。§44 の演算子集合として固定
3. `Fn<(Int, Int), Int>` の型グループを消しても通る — `parse_type` の `LParen` 枝。§37 の関数型の書き方として固定
4. `let (a, b) = ...` のタプルパターンが E105 にならない — `parse_pattern` が `(` で素のエラーを返していた。§37 どおり E105 を返すようにした

**既知の債務（M3 で扱う。M2 の完了条件に含まれず、fixer では直さない）**

- `glue` の `lower_to_m0` が余剰トップレベル `def` を弾かない。`def main = 1 + 2` に別の `def` が並んでも M0 に落ちる。名前解決・型を扱う M3 の領分であり、M2 の完了条件には入っていない

### R4 — M2 構文（lexer と parser）

| 段階 | 担当 | 結果 | 所要 |
|---|---|---|---|
| 計画 | planner (cursor-grok-4.6-xhigh) | 述語形式の受け入れ条件 8 件 | 約 3 分 |
| 計画審査 1〜4 | plan-reviewer (muse-spark) | **不合格 ×3**、4 回目で予算到達 | 約 20 分 |
| 計画の最終確定 | commander | 指摘を反映して上書き承認 | — |
| 実装 A（lexer と式）| builder (deepseek-v4-flash) | 式木の往復 20/20 | 約 8 分 |
| 実装 B（全文法）| builder | `PASSED: 8/8`、3 カウント 23/7/36 | 約 25 分 |
| レビュー | reviewer (cursor-grok-4.6-xhigh) | **合格**。落ちない欠陥 5 件 | 約 15 分 |
| 修正 | fixer (deepseek-v4-flash) | 4 件を単体テストで固定 | 約 8 分 |

**plan-review が 3 回不合格になった根（1 つ）**

> **判定器が「文法を実装すること」を一度も強制していない。**

審査者は毎回、実装を伴わない結託ハーネスを作って実測した:

1. **ヘッダ結託**: parse を常に成功させ、ヘッダのコードに対応する欠陥マーカーが本文にあれば診断行を印字する → 8/8 通過。commander は条件 2 のプローブを**内容入替え**（ヘッダは据置き、本文だけ別コードの欠陥に差し替え）にした
2. **符号キーの行・列テーブル**: 符号は本文から拾うが位置は暗記 → 全チェック素通り。commander は**摂動プローブ**（`-- error:` 行の直後に空行を入れると `<line>` が 1 増える、空白を 2 つ足すと `<col>` が 12 → 14 になる）を足した
3. **本文走査で符号と位置を両方計算**: マーカー走査だけで摂動にも追随 → 素通り。commander は判定を**コーパスの外**に出した

3 回目への対処として最初に置いた「実行時に生成した算術式を本番経路に通す」は、4 回目の審査で**中核の主張が偽**と実証された — 生成式が `<int> + <int>` で M0 の部分集合にすぎず、§41 を一行も実装しない実装が素通りする。

**主判定器の最終形: 式木の往復**

固定 seed の乱数で式木を 20 本作り、**冗長な括弧・空白・行コメントを混ぜて**ソースに描画し、parse して**完全括弧付きの正規形**を印字させ、木から直接作った正規形と照合する。

これが通るには優先順位・単項マイナス・冗長括弧の除去・コメントと空白の読み飛ばしをすべて正しく実装している必要がある。実行を伴わないので `emit` に手を入れる必要もない（M6 への越境がない）。

**残余の限界（合否ゲートにしない、と計画で宣言した）**

式木の往復まで通す実装は実質 §41 の式文法を実装しているが、「テストが生成する範囲だけを通す最小実装」は理論上ありうる。**有限のテストで一般性は強制できない。** reviewer と commander が `parser.rs` を読む監査で受ける、と宣言した。

**式木の往復が見ていないもの（commander が実測で発見）**

結合性を壊しても式木の往復は落ちない。描画側が `parens_needed` で必要な括弧を必ず付けるため、`a * b * c` のような無括弧の連鎖がソースに現れないから。**往復テストの落とし穴** — 描画側が構造を括弧で明示すると、parser は構造を復元する必要がなくなる。

結合性は `parser::tests::left_assoc_add_and_mul`（lib 側の単体テスト）が固定しており、条件 5 が `cargo test` 全体を見るので捕まる。

**reviewer が見つけた「壊してもゲートが落ちない」5 件**

コーパスがその構文を使っていないため、実装を消してもどの判定器も落ちない:

1. **§10 の先頭 `|` 付き ADT が parse できない** — 仕様自身が使う形。M3 で即座に当たる
2. 単項 `not` を消しても通る
3. `Fn<(Int, Int), Int>` の型グループを消しても通る
4. `let (a, b) = x -> a` が E105 にならない（式の `(1, 2)` はなる）
5. glue の `lower_to_m0` が余剰トップレベル `def` を弾かない

1〜4 を fixer が直し、**`parser.rs` の単体テストで固定**した（コーパスは既定ロックなので増やさない。条件 5 が `cargo test` 全体を見るので回帰は捕まる）。

**commander が自分の環境で実測した結果**

| 検証 | 結果 |
|---|---|
| `bash scripts/verify-m2.sh` | `PASSED: 8/8`、終了コード 0 |
| `verify-m0.sh` / `verify-m1-spec.sh` / `verify-m1-corpus.sh` | 7/7 / 6/6 / 7/7 |
| `cargo test` | 38 件通過 |
| 注入「`parse_mul` の右辺を `parse_mul` にして右結合にする」 | NG: 5（`cargo test` が落ちる）|
| 注入「診断の `line` を常に 1 にする」 | NG: 4（期待行と摂動プローブの両方で検出）|
| 注入「先頭 `|` の受理を消す」 | `adt_accepts_leading_pipe` が FAILED、NG: 5 |

**結果: 合格**

**既知の債務（M3 で扱う）**

- glue の `lower_to_m0` が余剰トップレベル `def` を弾かない。`def other = 1` と `def main = 1 + 2` が並ぶと `compile_to_object` が成功する。名前解決と未使用 `def` の検査（E513）は M3・M4 の担当なので、そこで扱う
- `tests/reject/E508.shin`（R3 から持ち越し）は両枝の型が違うため、型検査が冗長検査より先に当たり E508 に到達しない

### R5 — M3 型（計画まで完了。実装は未着手）

| 段階 | 担当 | 結果 |
|---|---|---|
| 計画 | planner (cursor-grok-4.6-xhigh) | 12 チェック、フェーズ A/B/C に分割 |
| 計画審査 1 | plan-reviewer (deepseek-v4-flash) | **不合格**。型検査ゼロの結託が 12/12 を通した |
| 計画の修正 | commander | 判定器の設計原則を立て、チェック 6/7/8 を作り直した |
| 計画審査 2 | plan-reviewer | **計画の内部矛盾**を実測で確定（下記）。判定行は未出力 |
| 仕様の修正 | commander | §50 に E205 / E305 / E306 を追加 |

**plan-review 1 回目の指摘（根は R4 と同じ）**

> **判定器の入力がリポジトリに固定されている限り、暗記で満たせる。**

審査者は、型検査を一切せず**コーパスの内容そのものをキーにした辞書**を持つ実装を作り、12 チェックすべてを通した。swap・perturb・ill-typed の各プローブは入力がスクリプトに固定されていたため、内容キー辞書は変更後の内容も暗記できた。

**commander が立てた判定器の設計原則**

効く判定器は次の 2 条件をともに満たす:

1. **入力を実行時に生成する**（リポジトリにも判定スクリプトにも固定で書かれていない）
2. **期待値の oracle を判定器側に置く**（被験側 `crates/shin/**` のコードに計算させない）

当初案は「型推論の往復」を builder が書く `check_infer.rs` に任せていた。これは**被験者が自分の答案を採点する**形なので判定として成立しない。judge が擬似乱数で式を 20 本組み立て、期待型も judge 側の規則で計算し、`shin check --print-type` の出力と照合する形に直した。ill-typed も実行時生成にし、`crates/shin/src` にコーパスのファイル名や本文断片が現れないことを構造で見るチェックを足した。

**plan-review 2 回目が実測で確定させた、計画の内部矛盾**

正直な最小実装を書いて走らせたところ、チェック 6 は 20/20 で通るのにチェック 7 が落ちた。

- D2（API 境界）は「仕様の表に無い失敗には符号を発明しない」と定めている
- チェック 7 は ill-typed な式に `error[E3` を要求している
- **§50 には「型が食い違う」を表すコードが無い**

**仕様側の欠落**（commander が §50 に追加）

型検査を書こうとして分かったのは、仕様が最も頻出する 2 つのエラーに符号を与えていなかったこと。コードが無ければ拒否コーパスが書けず、その失敗は M3 以降で永遠に検証されない。

| コード | 短い名前 | 節 |
|---|---|---|
| E205 | unbound-name | §47 |
| E305 | type-mismatch | §9 |
| E306 | occurs-check | §9 |

拒否コーパスは 43 本から 46 本になる。R5 のフェーズ A で 3 本を新設する（この 3 本に限りコーパスのロックを解除する）。

R3 で構文の禁止 4 件にコードが無かったのと同じ形で、**判定器を作る作業が仕様のレビューになっている**。「コーパスをコンパイラより先に書く」という計画の狙い（`docs/plan.md:269`）が実際に効いている。

**次のセッションが再開する地点**

- `PLAN.md` はリポジトリ直下に在り、上記の修正を反映済み
- plan-review の予算は 4 回のうち 2 回消費
- 次の一手: `PLAN.md` を plan-reviewer へ 3 回目に出すか、commander が上書き承認して builder のフェーズ A（名前解決 + CLI + 拒否コーパス 3 本）へ進む


### R6 — 完了報告の検証と、仕様の未決事項の決定（2026-09-07）

M3〜M7 は `9600c2f` `8b5a2d9`（別セッション、builder = Claude Sonnet 5）の 2 コミットで入り、planner / plan-reviewer / builder / reviewer の審査ループを経ていない。本ラウンドは commander がその成果物を実測で検証し、見つかった仕様の未決事項をユーザーに grilling 形式で問うた。

**役割の逸脱:** R7（仕様改訂）は commander が仕様の差分を書き、plan-reviewer が審査する。builder は R8 から。

**commander の実測（門番は全通過。それでも完了と言えない根拠）**

| 検証 | 結果 |
|---|---|
| `cargo test`、`verify-m0`〜`m7` | 全通過 |
| `module type S = sig def f(x: Int): Bogus end` + `def main = 1` | exit 0。実装されない sig の型名を解決していない。`tests/accept/user_store.shin` の未定義型 `Error` がこれで通っていた |
| `def f id => if find_user id \| Ok(u) => u \| Error(e) => 0` | E205。だが `tests/reject/E508.shin` は E508 で止まる |
| `let y = x + "a" -> y` | E501（E305 であるべき）。`check_source` が `check_ast`（E5xx）を `infer`（E205/E305）より先に走らせている |
| `if \| x == 1 => one \| 2`（`one` は未束縛） | E511（E205 であるべき）。`E511.shin` はこれで通っていた |
| `def f => 1` | 通る。呼び出す構文が無い 0 引数関数値ができる |
| §12 の record 型定義の例（終端子形 `;`） | `expected a field name, found }`。仕様の例が自分のパーサを通らない |
| `def main = [1, 2]` | `Cons(1, Cons(2, Nil))` と E510 が拒否する形で印字 |
| `def main = { name = "a" }` | `{ name = a }`。入れ子の String に引用符が無い |
| M6「Error-main と 1 億深さの受理コーパス」 | `tests/accept` に無く、`verify-m6.sh` の生成プローブで代替 |
| `verify-m3.sh` チェック 10（corpus-lock） | スクリプト実行の前後比較になっており、直後の `8b5a2d9` で受理 10 本が書き換えられた |

**決定（ユーザー、grilling 36 問）**

決定の一覧は `docs/plan.md` 7 節。前半 19 問（Q1〜Q19）は M3〜M7 の欠陥から出た未決事項と、引数のパターン分岐の追加。後半 16 問（Q20〜Q35）は、符号無しの診断と「通ってしまう入力」を実測で洗い出して決めたもの（Float の `%`、`do?` の対象、sig の不一致、`main` の欠落、同名宣言、refutable な `let`、負のリテラルパターン、パイプの右辺の constructor、Float の印字、番号と段階の一致、アンカー位置、無名 record、型の別名、sig の型）。

**R7（仕様改訂）の実施**

commander が本セッションで `docs/spec.md` に反映した。`verify-m1-spec.sh` は `PASSED: 6/6`（チェック 2 の新設節の範囲を 43..52 に広げた）。§50 は 56 行で全行に段階とアンカーがある。§41 は完全な EBNF に置き換えた。付随して `docs/plan.md` 6 節と 7 節、`docs/corpus.md`、`docs/spec-trace.md`（新設）、`PLAN.md` を更新した。commander のレビューで §41「空白は意味を持たない」と E106 コーパス `inc(1)` の矛盾が見つかり、Q36 として追加で問うた（A: 空白は意味を持たず、E106 は constructor の側だけ）。仕様の審査は `PLAN.md` の手順 0 として実行エージェントが行う（ユーザー指定・2026-09-07: PLAN.md を役割の語彙に依存させない）。

仕様を先に変えたため、R7 終了時点で次の門番が赤になっている（想定どおり。R8 と R9 で green に戻す）: `verify-m1-corpus.sh` チェック 1（§50 が 56 行、拒否コーパスが 46 本で 1 対 1 でない）とチェック 2（§39 の例が新形式でコーパスが旧形式）、それらに依存する `verify-m2.sh` チェック 5 と `verify-m3.sh` チェック 9。他の門番と `cargo test` は全通過。

**成果物:** `docs/spec.md`（R7 反映済み）、`PLAN.md`（R8〜R9）。

**次の一手:** `PLAN.md` をエージェントに渡す。手順 0（仕様の審査）から R9 まで一続きで進む。


### 0.1.1 の計画 — grilling と文書再編（2026-09-08）

v0.1 は R9 で完了し、全門番と `cargo test` が通過していた。本ラウンドは commander（Claude Fable 5.1、本セッション）が `docs/wants.md` の 6 項目をユーザーに grilling 形式で問い、0.1.1 の範囲と仕様を決めた。決定の一覧は `docs/plan.md` 9 節。

**役割:** R7 と同じく、仕様の改訂と手順書は commander が書いた。R10 以降の実行と審査は 10 節の手順書でエージェントに渡す。

**決めたこと:** `docs/plan.md` 9.1 の 14 問。途中でユーザーがコードサンプルの OK / NG を確認し、式側の `Cons(x, xs)` を許す案を「NG にして `[x, ...xs]` で書けるようにしたい」と改めた（Q9 の改訂と Q14）。

**実施したこと**

| 手順 | 内容 | コミット |
|---|---|---|
| 1 | `PLAN.md` を廃止して 8 節へ移し、`AGENTS.md` `README.md` から進行中の開発への依存を消す | `21132db` |
| 2 | `git tag v0.1`（手順 1 の後） | — |
| 3 | 版付き 6 文書を `docs/0.1.1/` へ `git mv`。パス参照を更新 | `4ec9b56` |
| 3' | tag が版を持つのでディレクトリも外し、`docs/spec.md` などに戻す（ユーザー指摘） | `ece5762` |
| 4 | 仕様の改訂、9 節と 10 節、コーパス規約、追跡表、対応表、`wants.md` の整理 | `35582be` |
| 5 | 門番の項目数を計画の表から導く形にし、9.2 と §53 を対象に含める | `db895f0` |

手順 1 の後の門番は全通過。1 回だけ `verify-m3.sh` が 11/12 になったが、これは 2 系統の門番を同時に走らせて cargo の target を取り合ったときで、単独で回すと 12/12 だった。

**手順 5 の時点で赤の門番（実測）**

| 門番 | 赤のチェック | 原因 | 戻すラウンド |
|---|---|---|---|
| `cargo test` | `parse_corpus::corpus_counts`、`r9_grammar::grammar_generated_programs_preserve_every_declaration`（402 passed / 2 failed） | §50 が 58 行で拒否が 56 本。§41 の EBNF から生成した `//` `[x, ...xs]` `def f x \| ...` を現行パーサが読めない | R11〜R13 |
| `verify-m0.sh` | 2 cargo-test、3 walk、4 tail（4/7） | `cargo test` の終了コードを見る | 同上 |
| `verify-m1-corpus.sh` | 1 reject-1:1（E516 E517 が無い）、2 sec39（`def classify score` が受理に無い）、3 pairs（9.2 の 2 項目に受理と拒否が無い）、7 no-regressions（m0 が赤）（3/7） | 仕様先行 | R12, R13 |
| `verify-m2.sh` | 1 accept-27（`cargo` が exit 101）、3 non-e1xx-44（非 E1xx が 44 → 46）、5 gates（5/8） | 同上 | R12, R13 |
| `verify-m3.sh` | 9 gates（11/12） | m0 m1-corpus m2 が赤 | R13 |
| `verify-r9.sh` | R9-1 字句・全文法の往復（4/5） | §41 の新形式 | R13 |
| `verify-m1-spec.sh` `verify-m4`〜`m7` | 無し | — | — |

`verify-m4`〜`m7` は 0.1 の実装だけを見るので、本ラウンドの後も green のはずである（未実測。R10 の 10-b で改名前後の同数通過として実測する）。

R11〜R13 で green に戻す。手順書 10 節の「着手時点で赤の門番」がこれを指す。

**次の一手:** `docs/plan.md` 10 節をエージェントに渡す。R10（改名）から R13 まで一続きで進む。

### R10 — hado への改名（2026-09-08）

起点は `413e651`。CLI・crate・ランタイム・拡張子・文書参照を改名した。コーパスと prelude の内容は保存し、lock はパスだけを更新した。

**環境:** Rust 1.98.0、`CARGO_HOME=/tmp/hado-011-cargo`、`CARGO_TARGET_DIR=/tmp/shin-target`、`CARGO_NET_OFFLINE=true`。PATH に Cargo が無く、既定の Rust 1.84.0 も依存の要件を満たさなかったため、既設の 1.98.0 を指定した。Git メタデータと OpenCode の保存領域は書き込み可能な実行環境を使った。

**判定:** 改名前後で次の結果が一致した。各行は `bash scripts/verify-<名前>.sh` の末尾 `PASSED: n/N`。ログは `/tmp/hado-011-evidence/baseline/` と `r10-fixed/`、C 型名修正後の前半再検査は `r10-final/`。

| 門番 | 改名前 | 改名後 | 終了コード |
|---|---|---|---|
| m0 | 4/7 | 4/7 | 1 |
| m1-spec | 6/6 | 6/6 | 0 |
| m1-corpus | 3/7 | 3/7 | 1 |
| m2 | 5/8 | 5/8 | 1 |
| m3 | 11/12 | 11/12 | 1 |
| m4 | 50/50 | 50/50 | 0 |
| m5 | 85/85 | 85/85 | 0 |
| m6 | 24/24 | 24/24 | 0 |
| m7 | 2/3 | 2/3 | 1 |
| r9 | 4/5 | 4/5 | 1 |

`cargo test --offline --no-fail-fast` は前後とも 402 passed / 2 failed、終了コード 101（m7 のログ）。失敗は `parse_corpus::corpus_counts` と `r9_grammar::grammar_generated_programs_preserve_every_declaration`。E516/E517 と新構文の未実装が原因で、m0・m1-corpus・m2・m3・m7・r9 に伝播する。計画時点の「m4〜m7 は green のはず」は、m7 が全テストを見るため成立しなかった。

10-a の検索は空。大文字小文字を無視する追加検索も空。10-c のハッシュ列は完全一致。10-d は `cargo run --offline -q -- build` で旧拡張子が exit 1（`build input must have the .hd extension`）、新拡張子が exit 0、生成物の stdout が `3`、exit 0。

**審査と対応:** 最終判定は 10-a〜10-d 全て PASS（セッション `ses_f8060cd42ffeN5V4a2XNufTnzT`、`review-r10-final.jsonl`）。判定者は指定の `opencode/muse-spark-1.3-contributor-free`。初回審査で R10 の仕様上の阻害要因なし。実装の自己検査でランタイム実ファイル名の改名漏れを検出して修正した。独立審査でも C 型名とヘッダガードの旧名を検出し、`HadoString` / `HADO_RUNTIME_H` に統一した。

**仕様への疑義:** §41 の型付き引数はカンマ区切りの括弧 1 群だが、§53 の `sign_of` 例は括弧 2 群。指定モデルも矛盾を確認した。ユーザーへ決定を求め、依存しない R10〜R12 を進める。仕様は未変更。

### R11 — `//` コメント（2026-09-08）

`0fd29f2` で審査済みのコーパス・lock・門番を先にコミットし、lexer を変更した。`//` は行末まで読み飛ばし、`--` と `#` は先頭文字で E104、`/` は除算となる。

- **11-a:** R10 と同じ環境で m0〜m7・r9 を実行。末尾は順に m0 `4/7`、m1-spec `6/6`、m1-corpus `3/7`、m2 `5/8`、m3 `11/12`、m4 `50/50`、m5 `85/85`、m6 `24/24`、m7 `2/3`、r9 `4/5`。`cargo test --offline --no-fail-fast` は 404 passed / 2 failed、exit 101。ログ: `/tmp/hado-011-evidence/r11/`。
- **除外した R12・R13 依存:** `corpus_counts`（E516/E517 が未追加）、`r9_grammar`（新文法）。m1-corpus の 1（コード表との対応）、2（classify 短縮形）、3（新項目のペア）、7（m0 の伝播）、m0 の 2〜4、m2 の 1・3・5、m3 の 9、m7 の全テスト判定、r9 の 1。これ以外の赤は無い。
- **11-b:** `bash scripts/verify-r11.sh` は実装前 `PASSED: 0/1`（2 failed）、実装後 `PASSED: 1/1`（2 passed）。48 回の乱数生成で行頭・式中・行末の E104 と列を照合し、同位置の `//` を受理。CLI build と生成物実行で `6 / 2` → `3`、`1 // 2` → `1`、文字列内の記号保存を確認した。
- **11-c:** 指定モデルのコーパス審査 PASS 後に lock を更新。実装後も `sha256sum --status -c tests/corpus.sha256` は exit 0。

判定者は `opencode/muse-spark-1.3-contributor-free`。コーパス審査 `ses_f80593028ffe8FwJW6nNqvbOXu`、実装審査 `ses_f804ffff7ffePdXgATEFJTvztJ`。コメント変更で必要となる sed の区切りと awk の `/` エスケープ、位置摂動、門番を弱めていないことも審査した。

準備コピーに E105 の余分な空行が入り、初回コーパス審査で不合格となった。実行中の門番が挿入した一時的な摂動を拾ったため、門番終了後のコミット済み状態から再作成して再審査 PASS。以後、次工程の準備は `git show` から作る。

仕様の文言残存: §50 のヘッダ説明が `-- spec:` のまま。R11 の変更範囲に仕様本文は含まれないため未変更。§41/§53 の型付き引数の疑義も決定待ち。

最終審査は 11-a（上記の依存例外付き）・11-b・11-c 全て PASS。審査文の比較先 `r10/` は失敗した途中試行なので、完了状態 `r10-fixed/` と `r10-final/` を採用した。テスト件数もログを機械集計し、R10 は 402/2、R11 は 404/2 と確認した（`r11-count-audit.txt`）。審査文の 402/2 という R11 集計は採用していない。

### R12 — List の式とパターン（2026-09-08）

コーパス審査 PASS 後、`71d5627` でコーパス・lock・6 本の新テストを固定した。実装前は `PASSED: 0/1`（6 failed）。`b1f9b49` で既存テストの受理例を新表記に移し、IR 内の constructor と直接表記の拒否テストは残した。

式は要素列と任意の末尾式を持ち、型推論で末尾を List 型と単一化する。パターンは parse 時に既存の Cons/Nil 構造へ展開し、利用者が書いた名前の位置を AST に保存して冗長段階で E516 にする。`coverage.rs` は変更していない。正規形印字・自由変数の参照・分岐検出・単相化にも末尾式を渡す。再包装検査には List 構文を加えた。

- **12-a:** `/tmp/hado-011-evidence/r12/` に全門番のログ。m0 `4/7`、m1-spec `6/6`、m1-corpus `3/7`、m2 `5/8`、m3 `11/12`、m4 `52/52`、m5 `87/87`、m6 `25/25`、m7 `2/3`、r9 `4/5`。`cargo test --offline --no-fail-fast` は 410 passed / 2 failed、exit 101。
- **除外した R13 依存:** `corpus_counts`（E517 のみ未追加）、`r9_grammar`（`condition_arm` のみ未到達）。m1-corpus の 1・2・3・7（E517、classify、新項目、m0 の伝播）、m0 の 2〜4、m2 の 1・3・5、m3 の 9、m7 の全テスト判定、r9 の 1。これ以外の赤は無い。
- **12-b:** `bash scripts/verify-r12.sh` は `PASSED: 1/1`（6 passed）。生成した入れ子と末尾展開を parse→印字→parse し全定義の本体 AST を照合。長さに関する網羅性、E510/E516 と名前の位置、不正展開の E100、生成物の stdout、再包装の E507 を検査した。`bash scripts/verify-r11.sh` も `PASSED: 1/1`。
- **12-c/d:** `stdlib/` の Nil/Cons は型宣言だけ。審査後の lock は実装後も `sha256sum --status -c tests/corpus.sha256` が exit 0。
- **12-e:** `check_cli::every_reject_preserves_code_and_column_after_header_line_insertion` は全拒否を列挙し、E510/E516 を含めて通過した（m7 のログ）。

判定者は `opencode/muse-spark-1.3-contributor-free`。コーパスは `review-r12-corpus.jsonl` で PASS、実装は `ses_f803ea316ffexPNnrnNImR35Oi`。往復生成器に展開式が不足するとの指摘を受け、`cf4aaf6` で生成と元 AST の保存検査を追加した。隔離コピーで印字時に末尾を落とす誤実装を入れると往復テストが失敗し、通常実装では通過した（`r12-mutation-baseline.log`、`r12-mutation-dropped-tail.log`、`r12-final-green.log`）。

仕様への新たな疑義は無い。R13 の型付き引数の表記は引き続きユーザーの決定待ち。

最終審査は 12-a（R13 依存例外付き）から 12-e まで PASS（`review-r12-final.jsonl`）。R12 の実装差分を固定し、R13 は型付き引数の決定を受けてからコーパスを完成させる。

### R13 — 型付き引数の表記の決定（2026-09-08）

ユーザーが §41 のカンマ区切り 1 群に統一する修正案を承認した。§53 の例を `(a: Int, b: Int)` に訂正した。R10 から保留していた疑義は解消し、R13 を再開する。指定モデル `opencode/muse-spark-1.3-contributor-free` も §41 との一致を確認した（`review-r13-corpus.jsonl`、`ses_f8022dccaffeXbqD5cIgTpke52`）。

R13 コーパス審査は新設・classify 移行と、既存拒否 E401/E404/E406/E511 の短縮形移行の両方で PASS（同セッション、`review-r13-corpus-followup.jsonl`）。既存拒否の移行は E517 との二重欠陥を避けるため。腕と期待するコードは維持した。審査後に lock を更新した。

実装前の `bash scripts/verify-r13.sh` は `PASSED: 0/1`。旧表記を受理すること、短縮形を parse できないこと、条件腕ゼロの条件列を受理することで失敗した（`r13-red.log`）。文法生成器も短縮形を生成して失敗を確認した（`r13-grammar-red.log`）。既存の冗長検査用入力は短縮形へ移し、元の検査対象を維持する。

### R13 — 引数の条件列（2026-09-08）

引数列の直後の `|` を条件列として解析する。元の `=> if | ...` は AST に関数の位置を残し、E517 を `def` / `fn` に出す。正規形は短縮形で印字する。引数名のない pattern 腕、E515、継続を挟む条件列は従来の判定を維持した。

- **13-a:** `cargo test --offline --no-fail-fast` は 417 passed / 0 failed、exit 0。全門番も exit 0。ログは `/tmp/hado-011-evidence/r13-final/`。追加修正後に全テストと m3 を再実行し、m3 内部の m0・m1-spec・m1-corpus・m2 も全て通過した。

| コマンド（`bash scripts/verify-*.sh`） | 出力の末尾 |
|---|---|
| m0 | `PASSED: 7/7` |
| m1-spec | `PASSED: 6/6` |
| m1-corpus | `PASSED: 7/7` |
| m2 | `PASSED: 8/8` |
| m3 | `PASSED: 12/12` |
| m4 | `PASSED: 54/54` |
| m5 | `PASSED: 89/89` |
| m6 | `PASSED: 26/26` |
| m7 | `PASSED: 3/3` |
| r9 | `PASSED: 5/5` |
| r11 | `PASSED: 1/1`（2 tests） |
| r12 | `PASSED: 1/1`（6 tests） |
| r13 | `PASSED: 1/1`（5 tests） |

- **13-b:** arity 1〜3 × 注釈あり/なしの全組み合わせを生成し、旧表記と括弧で囲んだ旧表記を E517・関数先頭位置で拒否。同じ腕の短縮形 72 本を CLI で build し、実行値を独立計算した和と閾値から照合した。継続の許可、引数なし pattern 形の E206、名前付き pattern の誤用の E204、正規形往復、モジュール内の E517 も通過した。
- **13-c:** 審査後の `b34154b` で lock を更新し、実装では変更していない。`sha256sum --strict --status -c tests/corpus.sha256` は exit 0。
- **13-d:** `every_reject_preserves_code_and_column_after_header_line_insertion` は E517 を含む全拒否を列挙して通過。生成検査でも E517 の行・列を照合した。

**審査と対応:** 指定モデル `opencode/muse-spark-1.3-contributor-free` の実装審査と追審は PASS（`ses_f801be035ffeEMJb7M9EwdjY4C`、`review-r13-implementation.jsonl`、`review-r13-refinements.jsonl`）。自己検査で引数ゼロの型注釈付き短縮形を parse が受理する問題を再現し、回帰テストと拒否処理を追加した。審査で挙がった短縮形 E511 と条件腕不足 E100 の位置検査も追加し、後者を関数先頭から文法に合わないトークンへ修正した。いずれも red→green を保存した（`r13-zero-red.log`、`r13-empty-anchor-red.log`、`r13-refined-green.log`）。

赤の門番は無い。型付き引数の疑義はユーザー承認で解消済み。既知の文書上の残存は §50 導入文の `-- spec:` 表記で、現行の字句規則（§41/§43）とコーパス規約は `// spec:` に統一されている。仕様本文の変更は承認された §53 の例に限定した。最終削りでは、この工程に不要な追加成果物・コメントは見つからなかった。

最終受入審査も 13-a〜13-d と 0.1.1 の完了条件を PASS と判定した（`review-r13-final.jsonl`、同セッション）。R10〜R13 の全工程を完了し、この実装・検証記録のコミットを `v0.1.1` とする。

### 0.1.2 の計画 — grilling（2026-09-08）

v0.1.1 は R13 で完了し、`v0.1.1` を tag した。本ラウンドは commander（Claude Fable 5.1、本セッション）が `docs/plan.md` 9.3 の 4 項目をユーザーに grilling 形式で問い、0.1.2 の範囲と仕様を決めた。決定の一覧は `docs/plan.md` 11 節。

**役割:** 0.1.1 と同じく、仕様の改訂と手順書は commander が書いた。R14 以降の実行と審査は 12 節の手順書でエージェントに渡す。

**決めたこと:** `docs/plan.md` 11.1 の 30 問。module 構文は 3 度方向を変えた。

1. 探索規則とファイル名からの導出（Q2 Q3）で始め、契約の書き方を「`.hd` 冒頭の無名 `sig ... end`」と提案した。ユーザーは `.hdi` を想定しており、「それだけだと同じ契約を複数の実装で満たせない」を懸念に挙げた（Q5）
2. `.hdi` に `include Store.STORE` を書く案（Q6）は「契約ファイルが実装を知るのは逆」で退けられ、`.hd` 冒頭の `: Store` 注釈（Q7）は「ぱっと見で明確でない」で退けられた。`.hdi` を廃止する案は却下
3. 「1 ファイル 1 module（定義と実装は別）、ただし 1 ファイルに全部書く option は残す、OCaml の綴りを写しすぎない」の 3 条件で、キーワードを `sig` / `impl` → `interface` / `implement` → `module` / `implement` と改め（Q10 Q12）、`end` を持たない節形式（Q14）に落ち着いた。`as` は必須（Q17）、`.hdi` にも見出しを書く（Q19）

契約の型を抽象型に限る決定（7 節 Q35）は、透明な型を提案したときに「前にも却下している」と再確認された（Q15）。中間展開は「先頭マッチ以外は走査と割り当てが照合に隠れる」の説明で「やらない。やるなら最適化の後」となった（Q24）。

advisor の指摘で、前提に埋めていた 3 点（`implement` 節の本体付きの型、`.hdi` の外の型、入口ファイルの節）を Q28〜Q30 として問い直した。ファイルをまたぐ順序、`?` のトークン化、E113 の欠番、E514 の改名は commander が置いた（11.4）。

**実施したこと**

| 手順 | 内容 |
|---|---|
| 1 | 仕様の改訂。§22 §23 §26 §40 §41 §49 §50 の `let?` `do?`、§34 §35 の全面書き換え、§5 §27 §39 §46 §47 の追随、§54 の新設 |
| 2 | `docs/plan.md` 11 節と 12 節、`docs/spec-trace.md` `docs/spec-coverage.md` `docs/corpus.md` の追随、`docs/wants.md` の整理 |
| 3 | `verify-m1-spec.sh` と `verify-m1-corpus.sh` の項目導出に 11.2 と §54 を含め、全門番を実走（下表） |
| 4 | 判定者レビュー。muse spark 1.3（contributor-free）が 3 回で 13、11、7 件を指摘し、全件を反映した（主なもの: 14-a の除外、`item:` 名のバッククォート、`Y.T` の可視性、E513 の条件、`.hdi` と契約の名前解決、診断形式、E1xx の件数、m3-11 と m7-2 の門番の置換、局所名前空間、修飾参照と E100 のアンカー）。4 回目は muse spark がリミットに当たったため deepseek v4 flash に切り替え、指摘 0 件（PASS）。3 回目の反映後に別セッションが `d89262d` としてコミットした |

**手順 3 の時点で赤の門番（実測）**

| 門番 | 赤のチェック | 原因 | 戻すラウンド |
|---|---|---|---|
| `cargo test` | `parse_corpus::corpus_counts`、`r9_grammar::grammar_generated_programs_preserve_every_declaration`（2 failed） | §50 が 57 行で拒否が 58 本（E113）。§41 の EBNF から生成した `let p? =` と節形式を現行パーサが読めない | R14（E113、`let p? =`）、R16（節形式） |
| `verify-m0.sh` | 2 cargo-test、3 walk、4 tail（4/7） | `cargo test` の終了コードを見る | 同上 |
| `verify-m1-spec.sh` | 1 headings（5/6） | 見出しの改名（表題 §22 §23 §34 §35）を `git diff` が削除と数える。コミット `d89262d` の後は 6/6（実測） | — |
| `verify-m1-corpus.sh` | 1 reject-1:1（E113 が §50 に無い）、2 sec39（`module UserStore` が受理に無い）、3 pairs（11.2 の 2 項目に受理と拒否が無い）、7 no-regressions（m0 が赤）（3/7） | 仕様先行 | R14（1、3 の let p? =）、R16（2、3 の節構文、7） |
| `verify-m2.sh` | 1 accept-29（`cargo` が exit 101）、2 e1xx-13（E1xx が 14 → 13）、5 gates（5/8） | 同上。初回は 6 protected-files も赤だったが、実走中に文書を編集したためで、編集を止めて回すと 5/8 | R14, R16 |
| `verify-m3.sh` | 9 gates（11/12） | m0 m1-spec m1-corpus m2 が赤 | R16 |
| `verify-r9.sh` | R9-1 字句・全文法の往復（4/5） | §41 の新形式 | R16 |
| `verify-m7.sh` | 3（2/3） | `cargo test` の終了コード | R16 |
| `verify-m4`〜`m6`、`r11`〜`r13` | 無し | — | — |

R14〜R17 で green に戻す。

**次の一手:** `docs/plan.md` 12 節をエージェントに渡す。R14 から R17 まで一続きで進む。

### R14 — `let p? =` と `do`（2026-09-08）

`?` を独立トークンとし、成功値への束縛はパターン末尾、破棄は `do` で表す。既存の `LetQ` / `DoQ` AST を保つ方針で、lexer・parser・正規形・診断案内を変更した。生成テストは実装前に 0 passed / 5 failed、実装後に 5 passed / 0 failed（`/tmp/hado-v012/r14-red.log`、`r14-green.log`）。

**14-a の実測:** `PATH=/home/zyun/.cargo/bin:$PATH RUSTUP_TOOLCHAIN=1.98.0 CARGO_TARGET_DIR=/tmp/hado-v012/target` を設定して実行。既定の PATH に cargo が無く、rustup の既定 1.84 では固定依存の edition 2024 を読めなかったため、インストール済み 1.98 と書き込み可能な target を使った。依存を `cargo +1.98.0 fetch --locked` で補充した後は offline で検証した。

`cargo test --offline --no-fail-fast` は **421 passed / 1 failed、exit 101**。失敗は `r9_grammar::grammar_generated_programs_preserve_every_declaration` の節構文6規則の未被覆だけで、14-a の許容する R16 依存である。全門番を個別に実行した末尾は次のとおり（ログ: `/tmp/hado-v012/r14/`）。

| コマンド（`bash scripts/verify-*.sh`） | 末尾 | exit |
|---|---|---|
| `verify-m0.sh` | `PASSED: 4/7` | 1 |
| `verify-m1-corpus.sh` | `PASSED: 4/7` | 1 |
| `verify-m1-spec.sh` | `PASSED: 6/6` | 0 |
| `verify-m2.sh` | `PASSED: 7/8` | 1 |
| `verify-m3.sh` | `PASSED: 11/12` | 1 |
| `verify-m4.sh` | `PASSED: 55/55` | 0 |
| `verify-m5.sh` | `PASSED: 89/89` | 0 |
| `verify-m6.sh` | `PASSED: 27/27` | 0 |
| `verify-m7.sh` | `PASSED: 2/3` | 1 |
| `verify-r11.sh` | `PASSED: 1/1` | 0 |
| `verify-r12.sh` | `PASSED: 1/1` | 0 |
| `verify-r13.sh` | `PASSED: 1/1` | 0 |
| `verify-r14.sh` | `PASSED: 1/1` | 0 |
| `verify-r9.sh` | `PASSED: 4/5` | 1 |

赤は m0 の 2〜4、m1-corpus の 2・3・7、m2 の 5、m3 の 9、m7 の 3、r9 の R9-1。全て節構文の未被覆と、それを参照する終了コードに由来する。R14 に固有の赤は無い。

**14-b:** 名前・record・constructor・入れ子の生成パターンの正規形往復と空白摂動、旧形の E100 と `?` アンカー、純粋値の E303、Option/Result を捨てる E504、非網羅パターンの E402 を検証。Option/Result それぞれの成功・各段階の失敗経路を CLI build → 実行し、判定器が計算した値と照合した。

**14-c:** コーパス審査後の `5c57d11` で lock を固定。実装 `eb9b2eb` では変更せず、`sha256sum --strict --status -c tests/corpus.sha256` は exit 0。拒否コード表とディスクはともに57、E1xxは13（表から導出）。

**14-d:** `every_reject_preserves_code_and_column_after_header_line_insertion` が E303・E504 を含む全拒否について通過。生成テストも両コードの行・列を摂動して照合した。

**審査:** `opencode/deepseek-v4-flash` のコーパス審査は、非網羅パターンのテストに型エラーがあるとの指摘1件。成功値を取り出してからパターン照合する §22 の規則と、旧表記の同じ式を HEAD の独立コピーで検査した E402 の実測を示し、レビュアーが撤回して PASS（セッション `ses_f7eb441c8ffe3zRn761Yq9EGEx`、`review-r14-followup.jsonl`）。実装審査も PASS（`/tmp/hado-v012/review-r14-implementation.jsonl`）。実測と判定を分ける `evidence-discipline` に従い、誤った指摘に合わせて正しいテストを変更しなかった。

仕様への疑義は無い。最終削りで追加削除すべき成果物は無かった。工程コミットは `5c57d11`（コーパス・判定器）と `eb9b2eb`（実装）。この記録の直前の `git status --short` は空。次は R15 の機械的なディレクトリ移行。

### R15 — コーパスのディレクトリ化（2026-09-08）

`tests/accept/X.hd` と `tests/reject/EXXX.hd` を `git mv` でそれぞれ `X/main.hd`、`EXXX/main.hd` に移した。列挙、門番のプローブ、ドキュメントのパスを同じ単位へ移し、コーパスの内容は変更していない。

**15-a の実測:** R14 と同じ環境変数で `cargo test --offline --no-fail-fast` と全 `bash scripts/verify-*.sh` を実行。全テスト対象の passed / failed 件数が R14 と一致（合計421 passed / 1 failed）、全門番の exit と `PASSED: n/N` も R14 の表と完全一致した。比較ログは `/tmp/hado-v012/r14/summary.txt` と `/tmp/hado-v012/r15/summary.txt`、各テストのログはそれぞれの `cargo-test.log`。節構文に由来する R14 の赤だけが残る。

初回の m3 は10/12で、追加の赤は check 8（暗記検出）だった。ケース固有だった `p.name` が全件で `main.hd` になり、正しい列挙処理の文字列を暗記と誤認していた。検査対象を `ケース名/main.hd` へ移し、完全パスと20文字以上のコーパス行の検出は維持した。全ケースの相対パス・完全パスを注入すると拒否し、正しいソースは受理することを直接確認。修正後の `bash scripts/verify-m3.sh` は11/12、check 8 は OK、check 9 だけが既知の赤。初回ログも `/tmp/hado-v012/r15-initial-m3.log` に保存した。

**15-b:** 移行前の `/tmp/hado-v012/r14-lock.sha256` と現行lockのハッシュ列は順序も含めて一致。`sha256sum --strict --status -c tests/corpus.sha256` は exit 0。

**15-c:** 受理・拒否ディレクトリ直下に `.hd` は無く、各ケースに `main.hd` がちょうど1本。ケースの追加削除は無い。

**審査:** 指定の `opencode/deepseek-v4-flash` は独立コピーの移行差分を PASS（セッション `ses_f7ea48a9bffeyjdwwLCYI6gvr1`、`review-r15-preview.jsonl`）。実測で見つけた m3 の移行漏れを追加審査し、修正も PASS（`review-r15-predicate.jsonl`）。審査済みコピーと本体の変更ファイルはバイト単位で一致する。差分の審査だけでは門番の実測を代替できないことが今回の移行で確認できた。

仕様への疑義は無い。最終削りで追加の削除対象は無かった。

最終の実測審査も **PASS — 15-a, 15-b, 15-c all satisfied**（`/tmp/hado-v012/review-r15-final.jsonl`、同セッション）。次は R16 の節構文。

## R16 節構文（2026-09-09）

`module X` / `implement X as Y` を次の見出しかEOFまでの節として解析する。契約名を必須にし、`sig`・`struct`・`end` を通常の名前に戻した。契約内の実装修飾型、未知の抽象型の実体化、内側型の衝突を仕様の段階とトークン位置で拒否する。局所ADT・recordの使用は、契約の公開定義と抽象型から依存関係をたどって判定する。

既存テストは外の宣言を見出しより前に置き、旧構文の診断アンカーを新構文に移した。抽象型のrecord・ADT表現は実装内に保った。未知の実体化はE204なので構文エラーが先になる。外の同名宣言が実装内の式より先になるケースは、そのソース順を明示して検査する。全文法の生成器も `outer_item* section*` に移した。

E601の最初の移行案では表現recordを外へ出し、fieldから具体型が推論されてE305になった。元の局所性を保つ形へコーパス工程を戻し、再審査PASS後にlockを固定し直した（`eff63c7`）。実装内の未使用ADT・recordを受理する欠陥は生成テストの赤で再現し、使用されるrecord・constructor・抽象型の表現と区別して修正した。

実行環境はR14と同じRust 1.98.0、`CARGO_TARGET_DIR=/tmp/hado-v012/target`。`cargo test --offline --no-fail-fast` は **429 passed / 0 failed**。全門番の実測は以下の通り（すべてexit 0）。ログは `/tmp/hado-v012/r16/`。

| コマンド | 出力末尾 |
|---|---|
| `bash scripts/verify-m0.sh` | `PASSED: 7/7` |
| `bash scripts/verify-m1-spec.sh` | `PASSED: 6/6` |
| `bash scripts/verify-m1-corpus.sh` | `PASSED: 7/7` |
| `bash scripts/verify-m2.sh` | `PASSED: 8/8` |
| `bash scripts/verify-m3.sh` | `PASSED: 12/12` |
| `bash scripts/verify-m4.sh` | `PASSED: 56/56` |
| `bash scripts/verify-m5.sh` | `PASSED: 90/90` |
| `bash scripts/verify-m6.sh` | `PASSED: 28/28` |
| `bash scripts/verify-m7.sh` | `PASSED: 3/3` |
| `bash scripts/verify-r9.sh` | `PASSED: 5/5` |
| `bash scripts/verify-r11.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r12.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r13.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r14.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r16.sh` | `PASSED: 1/1` |

16-a: 全件通過、R17へ持ち越す赤なし。16-b: 契約・実装の数と順序、CLI実行、旧構文の拒否、名前空間・不透明性・局所型を生成判定器で確認。16-c: stdlibの旧キーワード検索は0件。16-d: コーパス再審査後のlockとディスクが一致（`sha256sum --status -c tests/corpus.sha256` exit 0）。16-e: R9の拒否コーパス位置摂動とR16のE602・E514位置検査が通過。

m3のstdlib述語は外のdefを拒否し、m7は同名の契約・実装の見出しを照合する。契約だけを改名する変異を拒否することを確認し、変更後のm7も単独再実行で3/3だった。審査側の重複門番はコーパスの一時変更が競合しないよう停止した。本体側の全門番が完走し、lockの一致を再確認した。

審査は `opencode/deepseek-v4-flash`。コーパスのセッション `ses_f7e9e945effeCOscKQkGcAudfQ`、実装のセッション `ses_f7e7cbcc3ffemASNBTOxvYimAm` ともPASS。実装審査は既存テストの意図保全とE601・E204・E205・E602の独立プローブも確認した。新たな仕様への疑義、未対応の指摘、赤の門番はない。最終削りでは余剰成果物を残さず、一時プローブとログは `/tmp` に置いた。

工程コミット: `dffeac9`（コーパス・判定器）、`2e72208`（修飾型アンカー）、`eff63c7`（E601局所表現の保持）、`3478e35`（既存判定の移行・局所型の未使用）。実装は `3bb755a`。本記録以外の作業ツリー差分はない。


## R17 複数ファイル（2026-09-09）

入口と同じディレクトリの `.hd`・`.hdi` を読み、入口を先、残りをパスのバイト順にする。各ファイルを個別に解析してから構文木を結合する。内部ではファイルごとに重ならない行範囲を使い、検査後に診断を元のパス・行へ戻す。全ファイルの字句解析を構文解析より先に行い、入口以外の外の `def` と `.hdi` の `implement` をE100にする。CLIの入口は `.hd` に限る。

コーパスの `multi_file/` は入口のrecord、`.hdi` の外の型と契約、2つの実装ファイルからなり、実行結果は20+22=42。事前審査PASS後、全 `.hd`・`.hdi` をlockに固定した。コーパスの実行とM6のコピーもケース全体を扱う。既存のCLIプローブはケースごとにディレクトリを分け、単一ファイルの全拒否診断が従来のソースAPIと一致することを確認する。

追加判定器は実装前に5件すべて赤だった。後続ファイルの字句エラーより先行ファイルの構文エラーを先に返す欠陥も赤で再現し、全ファイルを先に字句解析する形へ修正した。最終的な8テストは、乱数による契約・実装・配置と実行結果、重複の順序、ファイル役割、未使用と壊れた無関係ファイル、診断位置、単一ファイル互換性を検査する。

17-a: `cargo test --offline --no-fail-fast` は **437 passed / 0 failed**。全16門番がexit 0。Rustとtarget directoryはR14と同じで、最終実行は `CARGO_NET_OFFLINE=true`。ログは `/tmp/hado-v012/r17/`。

| コマンド | 出力末尾 |
|---|---|
| `bash scripts/verify-m0.sh` | `PASSED: 7/7` |
| `bash scripts/verify-m1-corpus.sh` | `PASSED: 7/7` |
| `bash scripts/verify-m1-spec.sh` | `PASSED: 6/6` |
| `bash scripts/verify-m2.sh` | `PASSED: 8/8` |
| `bash scripts/verify-m3.sh` | `PASSED: 12/12` |
| `bash scripts/verify-m4.sh` | `PASSED: 57/57` |
| `bash scripts/verify-m5.sh` | `PASSED: 91/91` |
| `bash scripts/verify-m6.sh` | `PASSED: 29/29` |
| `bash scripts/verify-m7.sh` | `PASSED: 3/3` |
| `bash scripts/verify-r11.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r12.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r13.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r14.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r16.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r17.sh` | `PASSED: 1/1` |
| `bash scripts/verify-r9.sh` | `PASSED: 5/5` |

17-b: `r17_files` 8/8。契約・実装の生成配置、CLIの実行結果、ファイル順で2番目のE205、無参照実装のE513、非入口の外のdefとhdiのimplementのE100、入口の切り替え、無関係な壊れたファイル、hdi内の実装修飾型のE204が通過。`.hdi` を入口にしたCLI拡張子エラーも確認した。

17-c: lockは事前審査後に更新し、実装工程では変更していない。lockの99行とディスク上の `.hd`・`.hdi` 99ファイルが一致し、`sha256sum --status -c tests/corpus.sha256` はexit 0。m3のlock・暗記排除も補助ファイルを含める。

17-d: 非入口ファイルのE100・E204・E205・E602・E514・E402と字句エラーについて、改行・空白の摂動後も元ファイルのパス・行・列を確認した。単一ファイルの拒否診断はソースAPIとの全文一致でも確認した。

審査は `opencode/deepseek-v4-flash`。コーパスは `ses_f7e78a670ffe36jLzic6HBZeK4`、実装は `ses_f7e611663ffedWvr8ceGzdzu9u` でPASS。実装審査は隔離コピーを読み、診断段階・ファイル順・prelude・出力・テスト移行を確認した。実行証拠は本体側のログを使った。未対応の指摘、赤の門番、仕様への疑義はない。最終削りでは一時ログと審査用コピーを `/tmp` に置き、リポジトリには判定に必要な成果物だけを残した。

工程コミットは次の通り。共通処理の抽出をファイル収集・結合の振る舞いから分けた。

```
0a8dc3e test: 複数ファイルのコーパスと生成判定器を固定する
71c114c test: ファイル単位の実行と診断順序を検証する
d3c5afd feat: 構文木の検査と実行ファイルの出力を共有する
e09b52e feat: 同じディレクトリのhdとhdiを結合する
```

全判定後の `git status --short` は空。本記録をコミットし、R17終了のタグ `v0.1.2` を付ける。R14〜R17の増分は `git log --oneline 5c57d11^..v0.1.2` で確認できる。

## 0.1.3 の計画（2026-09-09）

`v0.1.2` の tag の後、次の版の範囲を決めた。入口は「言語仕様の致命的欠陥を探してふさぐ」と「ディレクトリを名前空間にする module システム」の 2 つ。

**手順 1. 欠陥の探索。** 仕様を全文読み、独立レビュアー 1 体（general-purpose、依頼文は逐語でファイル渡し）に「書けない正当なプログラム」と「矛盾する節の対」だけを探させた。20 項目（致命 0、重大 5、軽微 15）。測れるものは 0.1.1 のコンパイラ（`/tmp/hado-v012/target`、当時 codex が R15〜R16 を実行中だったため同じ target を共有した）と R16 途中のバイナリで実測した。

| 実測 | 結果 |
|---|---|
| 契約が固定した引数を使わない実装（`def save user => Error(NotFound)`） | E505 |
| 契約に `def pi(): Float` | E100（引数 1 個以上が必須） |
| 契約に無い `def hidden = 42` を外から `M.hidden` | E204（module member is not exported） |
| 契約の型に `Id.T`（0.1.1 文法） | 受理。0.1.2 の §34 は E204 にする（R16 で実装済みを確認） |
| `main` が `M.T`（不透明型） | build 成功、`{ name = "a" }` を印字 |
| `f Red (g 1)` | `Red(g 1)` と読まれ E301 |
| 腕の中の `fn \| A => 1 \| B => 2` | E101 |
| `def f \| true => 1 \| false => 0` と `if b => 1 \| 0` | 両方受理 |
| `1 \|> (fn y => y + 1)` | 受理 |
| `def div x y => x / y` に Float | E305（一般化で Int に固定） |
| 2 実装が同名 constructor `Idle` | 受理（実装ごとに局所） |
| `compare` の修飾なし参照 | 受理（組み込み） |

重大 5 件は全て「冗長を拒否する規則」と「契約が形を固定する規則」の継ぎ目にあった: 使わない引数、契約の値 member、型引数付き抽象型、契約間の抽象型参照、同一ディレクトリの 2 実装。

**手順 2. grilling（25 問）。** `docs/plan.md` 13.1 の表。途中でユーザーが方向を広げた点: 型引数の順番を OCaml 式の後置にする（Q6〜Q9。`<User>List` の案は「空白区切りが美しい」で退けた）、`Fn(引数列 => 戻り)` と `fn(...)` を括弧で必ず閉じる（Q10〜Q11。`fn(A): B` は「`:` が同じ場所で 2 つの意味を持つ」で退けた）、Functor を入れる（Q13〜Q17。同じ引数列の再適用は「1 回までしか適用できない」で E604）、名前空間の区切りを `::` にする（Q18）。契約の中の `X.T` は、当初「実装修飾を契約でも解決する」と提案したが、「契約は実装に依存しないのでは」との指摘で契約修飾に改めた（Q12）。版は 0.1.3 のまま（Q25）。

**手順 3. 文書。** 仕様の改訂（§3 §5 §6 §7 §8 §10 §14 §16 §20 §21 §25 §26 §27 §34 §35 §36 §37 §38 §39 §40 §41 §44 §45 §46 §47 §49 §50 §52 §53 §54 の改訂、§55 §56 の新設、E518〜E521 E603 E604 E705 の追加）、`docs/plan.md` 13 節と 14 節、`docs/spec-coverage.md` `docs/spec-trace.md` `docs/corpus.md` `docs/wants.md` の追随、`verify-m1-spec.sh` と `verify-m1-corpus.sh` の項目導出に 13.2 を追加。advisor の指摘（E603 の 0 実装、functor 定義を実装に数えない、`_` の場所、`main` 無しの根のファイル、適用見出しの本文、型の印字、E105 の範囲）を 13.4 の細則と仕様本文に入れた。

**手順 4. 全門番の実走。** `docs/plan.md` 14 節末尾「着手時点の実測」。赤は全て仕様先行に由来し、0.1.2 の着手時点と同じ形（`cargo test` 2 failed、m0 m1-corpus m2 m3 m7 r9 が赤、m1-spec は表題の改版がコミットで消える）。

**手順 5. 判定者レビュー。** 第 1 回は `opencode/deepseek-v4-flash`（ユーザー指定。muse spark で始めた分は停止）に 5 観点（決定の反映、EBNF からの導出、§50 の一致、節の対の矛盾、手順書の実行可能性）で依頼し、FAIL・指摘 8 件（`0656778` に対して）。所要 18 分のうち 11 分は、5 文書を読み終えた直後の 1 ステップが 5 万トークンを生成した時間で、依頼文が「全部読んで 5 観点を一度に」だったことが原因。主な指摘: §55 の根との同名衝突が小文字のディレクトリ名と大文字の宣言名の比較で空文、§41 の EBNF が fallback 必須で E401/E404 に届かない、§5 の `def main = Nil` は E510 が先、19-a〜21-a に残る赤の但し書きが無い、13.2 の 3 項目に拒否コーパスの割り当てが無い。全件を `bbb90ca` で反映。第 2 回は依頼文を「指摘 8 件の反映確認と、直した箇所の新しい矛盾」に絞ったが、opencode が 20 分と 8 分の 2 回とも無出力で止まった（1 回目は `--dir` の外の `/tmp` を読ませたための権限待ちと推測、2 回目は原因不明。ログは init の後に何も無い）。以後の判定はこの会話とは別文脈のサブエージェント（general-purpose）に同じ依頼文で行わせた。第 2 回: 8 件全て「直っている」、新しい矛盾 3 件（`=>` を持つ腕が 0 本の列が診断未定義、21-a の赤の列挙の自己矛盾、E205 のアンカーのずれ）→ `9dfe42d`。第 3 回: 3 件全て直り、新しい矛盾 2 件（21-a に m2 の 5 が漏れ、E100 のアンカーに腕 0 本の列が無い）→ `1ffa354`。第 4 回: 2 件とも直り、新しい矛盾は無く PASS。判定結果は `/tmp/hado-v013/review/` と scratchpad に残した。

**次の一手:** 14 節をエージェントに渡す。R18 から R22 まで一続きで進む。

## R18 型の綴りと匿名関数（2026-09-09）

後置型、宣言前の型変数、`Fn(...)`、3形の `fn(...)` と E520 を実装した。構文と印字で既存の型付きIRを保つ方針を採った。型注釈混在と腕0本のアンカーを追加テストで再現してから修正した。

コーパスは `cursor-agent --model cursor-grok-4.6-high` の独立審査でPASS（`/tmp/hado-v013/review/r18-corpus.log`）。既存期待値を維持し、審査後にlockを更新した（`6d81fb1`）。`8e85dc8` は E520 と型印字の失敗再現テスト。最終削りでは生成された `__pycache__` と、型構文の旧綴りを示すコメント2行を除去した。

判定器の第1回審査はFAIL（`r18-tests.log`）。`main = probe` の関数値は E702 で拒否されるため、関数型・抽象型を含む構文往復と、印字可能な型のCLI検証を分けた。修正版は8件通過。実装審査はPASS（`r18-implementation.log`）。レビュアーも新規テスト、字句・構文単体、CLIの旧綴り拒否位置・新しいfn・E520と例外・診断段階順を実走した。新規テストは `d906ce7`、実装は `bd46550`。既存テスト・門番移行の初回審査はFAIL（`r18-migration-tests.log`）。旧構文の拒否テストが、R19で合法となる抽象型宣言に書き換わっていたため、拒否対象を旧綴り `type T<A>` に戻した。`cargo test --offline --test r9_syntax` は24 passed。再審査はPASS（`r18-migration-tests-followup.log`）で、レビュアーも `<` のE100をCLIで確認した。コーパス・実装・テストの審査は終了し、18-aだけを後続ラウンド完了時の再判定に残す。

実測環境は `PATH=/home/zyun/.cargo/bin:$PATH RUSTUP_TOOLCHAIN=1.98.0 CARGO_TARGET_DIR=/tmp/hado-v013/target`。既定の Cargo 1.84 では依存キャッシュを使えず、1.98でビルドできた。

| コマンド | 出力の末尾・結果 |
|---|---|
| `cargo test --offline --no-fail-fast` | 合計446 passed / 2 failed、exit 101 |
| `cargo test --offline --test r18_checks` | 3 passed / 0 failed（変更前は1 passed / 2 failed） |
| `bash scripts/verify-r18.sh` | 8テスト通過、`PASSED: 1/1` |
| `bash scripts/verify-m1-spec.sh` | `PASSED: 6/6`、exit 0 |
| `verify-m4.sh` / `verify-m5.sh` / `verify-m6.sh` | `PASSED: 60/60` / `93/93` / `30/30`、exit 0 |
| `verify-r11.sh`〜`r14.sh`、`r16.sh`、`r17.sh` | 各 `PASSED: 1/1`、exit 0 |
| `sha256sum --strict --status -c tests/corpus.sha256` | exit 0 |

全門番のログは `/tmp/hado-v013/r18-gates/`。初回の赤から、門番に残った旧形式のstdlib宣言検索・多相member検索と、E101の固定行番号を修正した。m2・m3・m7を再実行したログは `/tmp/hado-v013/r18-gates-recheck/`。

残る赤:

| 門番 | 結果 | 理由 |
|---|---|---|
| `cargo test` | 2 failed | `corpus_counts` は E518 E519 E521 E603 E604 E705 が未追加。`r9_grammar` は1200往復後にR21/R22の5規則未訪問を検出 |
| m0 | `PASSED: 4/7` | 全テストの非ゼロ終了をチェック2〜4が見る |
| m1-corpus | `PASSED: 3/7` | 後続の拒否コード、§39のFunctor例、項目ペアが不足。チェック7はm0の赤 |
| m2 | `PASSED: 5/8` | チェック1はcorpus_counts、3は非E1xxの不足、5は依存門番の赤 |
| m3 | `PASSED: 11/12` | チェック9の依存門番。型往復・stdlib型・lock検査は通過 |
| m7 | `PASSED: 2/3` | チェック3の全テスト。実行34件とmember一覧24件は通過 |
| r9 | `PASSED: 4/5` | チェック1の未訪問文法規則 |

計画との食い違いは次の2点。仕様本文は変更していない。

- R18のE520例は、注釈が無いと E515 の `def` アンカーが先になる。仕様§50・§52に従って引数注釈を足し、単一欠陥にした。Cursorも旧コンパイラで両形を実測して確認した。
- 18-a は後続依存の赤が無いとするが、仕様§50の全コードと§41の全規則を要求する門番は、R19〜R22の成果物が無ければ赤になる。18-aは未達として残し、全ラウンド終了時に再判定する。門番を緩めていない。

## R19 契約の拡張（2026-09-09）

値member、型引数付き抽象型と実体化、契約member実装・fnの `_`、契約内の `X.T`、E518・E519・E603・E705を実装した。実体化は形式型変数と本体を保持し、公開型の不透明性を残したままコード生成時に実引数を置換する。値memberはthunkとして評価し、Fn値なら結果のclosureを呼ぶ。

コーパス8件はCursorの独立審査でPASS（`/tmp/hado-v013/review/r19-corpus.log`）、その後lockを更新した（`7b48581`）。判定器の初回は1 passed / 9 failed（`521283f`）、型引数付き契約参照の位置を追加した（`4137c2f`）。判定器11件の審査はPASS（`r19-tests.log`）。既存テスト移行も78件通過と審査PASS（`r19-existing.log`、`14d5aae`）。内部schemaの検証は具体値へ戻す呼出しを加え、元のassertionを維持した。

実装審査はPASS（`r19-implementation.log`）。レビュアーは独立したCLI入力で値member・Fn値・異種型引数・公開型の不透明性・自己修飾・各診断と位置・段階順を実走した。その後の追加調査で `type T = M.T` がstack overflowになることを再現し、失敗テストを固定した（`f9f0ba3`）。局所の修飾別名を循環検査へ戻し、実装をまたぐ実体化の依存も検査した。自己参照・多相自己参照・相互参照がE602となり、12件通過。追加修正の独立審査もPASS（`r19-cycle.log`）。レビュアーは3実装の閉路とcheck/build/dump-monoを実走し、正常な型引数付き実体化・名前付き再帰ADT/record・非循環の実装間参照を過剰拒否しないことも確認した。

全門番で `contract_type` の§34タグが§39例の共通見出しだけで拒否された。原条件は「§39の例が全て受理コーパスにある」であり、同じ見出しを含む別節の例にタグを強制する条件ではない。各例が§39タグ付きファイルに存在することを要求する修正は、独立審査で「判定条件を保つ誤検出修正」とPASS（`r19-gate-proposal.log`）。欠落・タグ付け替え・Guest枝削除・preludeタグ削除は引き続き赤になることをレビュアーが実測した。コーパスを変更せず門番を修正した（`d5b600b`）。

最終実測環境はR18と同じ。ログは `/tmp/hado-v013/r19-final-gates/`。

| コマンド | 出力の末尾・結果 |
|---|---|
| `cargo test --offline --no-fail-fast` | 合計458 passed / 2 failed、exit 101 |
| `bash scripts/verify-r19.sh` | 12件通過、`PASSED: 1/1`、exit 0 |
| `verify-m1-spec.sh` | `PASSED: 6/6`、exit 0 |
| `verify-m4.sh` / `verify-m5.sh` / `verify-m6.sh` | `PASSED: 65/65` / `98/98` / `31/31`、exit 0 |
| `verify-r11.sh`〜`r14.sh`、`r16.sh`〜`r18.sh` | 各 `PASSED: 1/1`、exit 0 |
| `sha256sum --strict --status -c tests/corpus.sha256` | exit 0。実装中はlockを変更していない |

残る赤は、`corpus_counts` のE521・E604不足と、`r9_grammar` のR21/R22の5規則未訪問、それらへの依存である。m0は4/7、m1-corpusは3/7（Functor例・複数入口/Functorのペア不足も検出）、m2は5/8、m3は11/12、m7は2/3（実行37/37は通過）、r9は4/5。19-aの列挙にはm2の3とr9の文法未訪問が漏れるが、いずれも後続ラウンドの未追加を検出している。R22後に全件を再判定する。

仕様への疑義・計画との食い違い:

- 計画の `user_store` の「契約を `def save(_: T)`」は、仕様§41が契約引数を名前に限定するため、§27・§39に従って実装側だけ `_` にした。コーパス審査も確認した。
- §53の説明文はBool分岐 `=> if c => a | b` をE517とするが、§14・§52と既存コーパスは許す。ユーザーへ確認中で、この振る舞いは変更していない。
- 型適用そのものの引数不足（例: `type A T` に対する `def empty: T`）はコード無し診断になる。実装審査で§50のコード表の穴として挙がった。13.4が現行の扱いの維持を指定しており、新しいコードや仕様解釈は追加していない。実体化左辺と契約の個数差はE602で検証済み。

仮説は「公開の抽象型と内部の実体化を分ければ、型引数と値memberを既存IRで扱える」。生成実行で確認した一方、修飾名を経由する別名循環が既存の局所alias検査から漏れることが分かった。`design-principles` の境界の判断に従い、置換を実体化に閉じ、公開型を直接具象化する変更は採らなかった。次はR20のファイルごとの値の可視性へ進む。

## R20 複数の入口（2026-09-09）

外のdefとその自由参照をファイルごとの内部名に置き換え、局所束縛と節内のdefを保持した。選択mainを実行の起点とし、非選択mainからの依存も使用判定に含める。全ファイルの本体は従来の検査を通す。サブディレクトリの外defをE100にするR20のプローブに合わせ、再帰探索を先行した。

コーパスは独立審査PASS（r20-corpus-preflight.log）後にlockした（331907a）。E206の旧helperはファイル局所化後に未使用という別の欠陥を生むため除き、main無しを保持した。判定器を2 passed / 14 failedで固定（43c82a8）、実装後は16 passed / 0 failed。候補審査もPASS（r20-tests-preflight.log）。レビューでは独立算術期待値、各入口実行、値の可視性、非選択mainの検査と依存、E206位置、診断順を仕様から確認し、現行実装でも16件を実走した。

実測ログは /tmp/hado-v013/r20-gates/。cargo test --offline --no-fail-fastは474 passed / 2 failed、exit101。残る2件はcorpus_countsのE521/E604不足とr9_grammarのR21/R22構文規則未訪問。全門番は終了した。実装審査もPASS（`/tmp/hado-v013/review/r20-implementation.log`）。レビュアーは両入口の42/110、値漏れ・局所束縛・compare・再帰・非選択本体のE305・選択mainだけのE701・E206位置を独立CLIで確認した。

仕様§5のmain制約は実行ファイルの規則なので選択mainに適用し、非選択mainの通常の型・名前・冗長性検査は維持した。既知の§53 Bool分岐疑義は引き続きユーザー確認中、挙動を変更していない。

| コマンド | 出力の末尾・結果 |
|---|---|
| `cargo test --offline --no-fail-fast` | 合計474 passed / 2 failed、exit 101 |
| `bash scripts/verify-r20.sh` | 16件通過、`PASSED: 1/1`、exit 0 |
| `verify-m1-spec.sh` | `PASSED: 6/6`、exit 0 |
| `verify-m4.sh` / `verify-m5.sh` / `verify-m6.sh` | `PASSED: 66/66` / `99/99` / `32/32`、exit 0 |
| `verify-r11.sh`〜`r14.sh`、`r16.sh`〜`r19.sh` | 各 `PASSED: 1/1`、exit 0 |
| `sha256sum --strict --status -c tests/corpus.sha256` | exit 0。実装中はlockを変更していない |

残る赤はm0 4/7、m1-corpus 3/7、m2 5/8、m3 11/12、m7 2/3、r9 4/5。m1-corpusの複数入口ペアの不足は解消し、Functorペア・§39例とE521/E604不足が残った。r9はR21/R22の5規則未訪問。19-aに列挙のない依存赤はR19と同じで、門番を緩めていない。

既存r17の役割テストは根の外defをE513、空の選択ファイルをE206、サブディレクトリの外defをE100へ移した（2580342）。禁止hdiの例から未使用契約などの余分な欠陥を除き、元ファイルの診断位置と全ファイル検査を保持した。

仮説は「値の名前だけをファイル単位に分け、使用判定の起点を全入口にすれば、既存の型・単相化経路を保てる」。16件の生成・位置検証と独立実走で確認した。`less-is-more`による余剰成果物・コメントの削除対象は無かった。次はR21のFunctorへ進む。

## R21 Functor基礎実装とconstructor追加への移行（2026-09-09）

基礎実装を `2a7b1ef` に保存した。本文を引数の契約型で一度検査し、適用ごとに型・比較制約・コード生成用ASTを置換する。引数と同名の大域実装の型を分離し、元ソースの診断位置を保持する。コーパスは `a8bd12d` で独立審査後にlock、判定器と追加再現は `c7ccb53`〜`0fe8f74` に先行して保存した。

Cursorの初回実装審査は診断文に内部型名が漏れる点でFAIL。CLIと単一ソースAPIの再現テストを固定し、型の同一性を保持して表示だけを元の名前へ戻した。追審は既知record疑義を除く範囲でPASS（`/tmp/hado-v013/review/r21-implementation-followup.log`）。レビュアーは不透明性、異種/generic適用、局所ADT、比較制約、定義元の値の可視性、依存順、E521/E604/E603と各CLI診断を実走した。文法生成の独立審査もPASS（`r21-grammar.log`）。

ソース固定後の実測は `/tmp/hado-v013/r21-gates-after-review/`。実行環境はR18と同じ。

| コマンド | 結果・出力末尾 |
|---|---|
| `cargo test --offline --no-fail-fast` | 合計491 passed / 16 failed、exit101 |
| `verify-m1-spec.sh` / `verify-m5.sh` / `verify-m6.sh` | 6/6、101/101、32/32、全てexit0 |
| `verify-r11.sh`〜`r14.sh`、`r16.sh`〜`r20.sh` | 各 `PASSED: 1/1`、exit0 |
| `verify-r21.sh` | `PASSED: 0/1`、exit1。正式14 passed / 14 failed |
| `sha256sum --check --strict tests/corpus.sha256` | exit0。実装中のlock変更なし |
| 別名形式の補助判定器 | 28 passed / 0 failed。正式判定器の合格ではない |

赤はm0 4/7、m1-corpus 5/7、m2 7/8、m3 10/12、m4 67/68、m7 1/3、r9 3/5、r21 0/1。受理Functorの旧 `type T = {...}` がE205となる1件、同じ形式を生成するR21判定器14件、R22のpath未訪問1件とそれらに依存する門番である。21-aは未達。門番を緩めていない。

§35/47の同名record禁止と§39/56例の矛盾について、ユーザーは局所record＋型別名の案ではなく、`type T = Payload {...}` を承認した。Tは型名、Payloadはconstructor名であり、名前付きfieldを直接宣言し、生成とパターンにも使う。計画§15の追加工程として仕様・コーパス・実装を改訂し、その後R21を再判定する。§53のBool分岐とE517の別件疑義は未解決。

R22のコーパス候補と判定器候補はCursor審査PASS（`r22-corpus.log`、`r22-tests.log`）。候補は `/tmp/hado-v013/` のみで、未採用。名前空間未実装の現行CLIで判定器17件は全て赤。R21追加工程を終えてから採用へ進む。

constructor宣言の構造整理は `b4c02a7`。型参照から名前とpayloadを分離し、新構文は加えていない。Cursorが基準と候補のCLI・診断・Functor実行を比較してPASS（`constructor-ast.log`）、隔離コピーの全cargoは同じ491/16、採用後のcheck_adtsは9/9。新構文のコーパスlockに依存しない振る舞い不変の整理として、追加実装とは別コミットにした。


## R21 名前付きfieldのconstructor（2026-09-09）

承認された `type T = Payload { id: Id.T; name: String }` を宣言・生成・パターンに実装した。単一constructorのfield参照、generic、名前による型の選択、抽象型の不透明性とFunctor適用ごとの型の分離を、既存ADTの型・表現へ接続した。生成は記述順に評価して宣言順のpayloadへ格納し、印字も宣言順にする。

仕様は `793b99b`、独立審査済みコーパスは `aec149a` でlockした。判定器のredは `fdf1394`、審査・追加調査で得た反例のredは `10afa08`。仕様/コーパス追審はPASS（`named-record-spec-v2.log`）、判定器追審もPASS（`named-record-tests-v2.log`）。

独立実装審査の初回FAILは、外と実装内の同名constructorが衝突する点だった。局所宣言のscheme・tag・named metadataを検査中の環境で切り替え、退出後は外を復元した。追加調査では、入れ子の未知field E204が先行するfield値のE305に隠れる反例を再現し、型推論に先立つ名前検査で未知fieldを採集した。Cursor追審はPASS（`/tmp/hado-v013/review/named-record-implementation-followup.log`）。外/内の位置/namedの4組合せ、Functor内同名、形式違い、同scope重複、複数fileの段階順を独立CLIで確認した。

ソース固定後の実測は `/tmp/hado-v013/named-record-gates-after-fixes/`。実行環境はR18と同じ。

| コマンド | 結果・出力末尾 |
|---|---|
| `cargo test --offline --no-fail-fast` | 合計544 passed / 1 failed、exit101。失敗は `r9_grammar` の `unvisited rules: ["path"]` |
| `bash scripts/verify-record-constructors.sh` | CLI36件・IR評価順2件通過、`PASSED: 1/1`、exit0 |
| `bash scripts/verify-r21.sh` | 28件通過、`PASSED: 1/1`、exit0 |
| `verify-m1-spec.sh` / `verify-m4.sh` / `verify-m5.sh` / `verify-m6.sh` | 6/6、70/70、103/103、34/34、全てexit0 |
| `verify-r11.sh`〜`r14.sh`、`r16.sh`〜`r20.sh` | 各 `PASSED: 1/1`、exit0 |
| `sha256sum --strict --status -c tests/corpus.sha256` | exit0。実装中のlock変更なし |

全門番は終了した。赤はm0 4/7、m1-corpus 6/7、m2 7/8、m3 11/12、m7 2/3、r9 4/5。いずれもR22のpath生成未訪問に依存する。21-b/c/dは満たしたが、21-aの全cargo通過は未達である。21-aの残存赤の列挙にはこの文法生成と依存赤が含まれていない。pathはR22の成果物なので、R22で全条件を再判定する。門番は緩めていない。

評価順の判定器は実際のmono IRを記号実行する。emitのLetが値から本体へ命令を生成する経路も審査で確認したが、ネイティブ命令列上の副作用順を直接測った証拠ではない。§53のBool分岐とE517の既知疑義は変更していない。

仮説は「名前付きfieldを既存ADTのpayloadへ対応させれば、型の名義性と実行表現を保てる」。CLI生成・実行と独立追審で確認した。型推論の早期returnだけでは名前段階の診断を集めきれないことを学び、名前の検査を先行させた。less-is-moreの確認で追加のコメント・余剰成果物の削除対象は無かった。

## R22 名前空間とv0.1.3の最終検証（2026-09-09）

入口の親を根として再帰探索し、型・契約・実装・constructorの4空間をディレクトリごとに解決する。無修飾参照は現在のディレクトリと根を探し、絶対パスは根から辿る。局所束縛とファイルごとの値の可視性は保持した。全ファイルの字句検査を構文検査より先に行い、診断は根からの相対パスと元の行・列で返す。

独立審査済みコーパスを0970610でlock、元の判定器17件を2a802c6で全件失敗の状態から固定した。位置プローブ追加と既存判定器の相対パス・再帰探索への移行は50f67dc。移行初回審査のFAILは、旧診断prefix、拒否ケースのhelper欠落、入れ子パスの暗記検出漏れだった。ケース全体の複製と元診断の比較、実際の欠陥ファイルへの摂動、入れ子相対パスの検出を加え、追審PASS。既存の段階・位置・lock条件は維持した。

R22初回統合で未知契約より先に公開memberの派生E204を返した。未解決契約の実装だけ派生member診断を抑え、既知契約の非公開member拒否は維持した。名前空間実装のCursor独立審査はPASS（r22-implementation.log）。4空間、親・兄弟可視性、自己絶対パス、根との衝突位置、named/generic/Functor、不透明性、字句先行、入口変更、空ディレクトリと不正名を独立CLIで確認した。

親の追加探索で、局所同名constructorを登録すると外側の型のfield情報と網羅性が失われる反例を得た。追加3件を36 passed / 3 failedで固定（c232089）。名前を隠しても型のmetadataを保持し、契約の制約を反映したprojectionを局所環境の退出前に解決する。コード生成へ渡すschemeは量化変数を保持してfree変数を解決する。

最終実測環境はR18と同じ（Rust1.98.0、offline、CARGO_TARGET_DIR=/tmp/hado-v013/target）。ログは `/tmp/hado-v013/r22-final-gates/`。全Rustテストは569 passed / 0 failed。名前空間の判定器19件、名前付きconstructor39件、IR評価順2件を含む。R21で残っていたR22 path文法未訪問も解消した。

仕様への疑義: §53のBool分岐とE517の既知矛盾は今回変更していない。型適用そのものの引数不足のコード無し診断も、計画13.4の現行維持に従う。

constructor追加修正のCursor追審はPASS（constructor-shadow-final.log）。修正前と後を別バイナリで比較し、独立20入力で異なるfield型、String/Intの多相実体化、Functor、比較、入れ子実装、不透明性、内外の網羅性を確認した。hidden keyがdump-monoへ漏れないことも実測した。


最終バッチのM3だけが11/12だった原因は、内部M2の180秒打ち切り。単独M2は8/8で、隔離計測の `m3-investigation.log` は `timed out after 180 seconds` を記録した。複合門番だけ完走を待ち、個別コマンドの180秒とexit0・PASSED全件の条件は保つ修正を87d0964へ保存した。Cursor審査PASS（m3-timeout.log）。修正後の実走は `/tmp/hado-v013/r22-final-m3/verify-m3.sh.log`、末尾 `PASSED: 12/12`、exit0。

最終判定（M3は上記の修正後ログ、他は最終バッチ）:

| コマンド（bash scripts/ 以下） | 出力末尾 | exit |
|---|---|---|
| `verify-m0.sh` | PASSED: 7/7 | 0 |
| `verify-m1-corpus.sh` | PASSED: 7/7 | 0 |
| `verify-m1-spec.sh` | PASSED: 6/6 | 0 |
| `verify-m2.sh` | PASSED: 8/8 | 0 |
| `verify-m3.sh` | PASSED: 12/12 | 0 |
| `verify-m4.sh` | PASSED: 71/71 | 0 |
| `verify-m5.sh` | PASSED: 104/104 | 0 |
| `verify-m6.sh` | PASSED: 35/35 | 0 |
| `verify-m7.sh` | PASSED: 3/3 | 0 |
| `verify-r11.sh` | PASSED: 1/1 | 0 |
| `verify-r12.sh` | PASSED: 1/1 | 0 |
| `verify-r13.sh` | PASSED: 1/1 | 0 |
| `verify-r14.sh` | PASSED: 1/1 | 0 |
| `verify-r16.sh` | PASSED: 1/1 | 0 |
| `verify-r17.sh` | PASSED: 1/1 | 0 |
| `verify-r18.sh` | PASSED: 1/1 | 0 |
| `verify-r19.sh` | PASSED: 1/1 | 0 |
| `verify-r20.sh` | PASSED: 1/1 | 0 |
| `verify-r21.sh` | PASSED: 1/1 | 0 |
| `verify-r22.sh` | PASSED: 1/1 | 0 |
| `verify-r9.sh` | PASSED: 5/5 | 0 |
| `verify-record-constructors.sh` | PASSED: 1/1 | 0 |

22-a: 全Rustテスト569/0と全22門番exit0。22-b/d: 名前空間判定器19件は生成実行・4空間・位置摂動を含め通過。22-c: lock 135行とディスク上の対象135ファイルが集合として一致し、`sha256sum --strict --status -c tests/corpus.sha256` はexit0。実装中のlock変更なし。残る赤の門番は無い。R18〜R22の後続依存で保留していた判定もこれで満たした。

実装コミットは1ead80e、追加の失敗固定はc232089、門番修正は87d0964。記録追加直前の `git status --short` と `git diff --check` は出力なし。v0.1.3のタグ対象はこの記録を含む最終コミットとする。コード・コメントの最終削りでは追加の削除対象は無く、生成されたPythonキャッシュだけを作業ツリーから除いた。

## 0.2 の計画 — 代数エフェクト（2026-09-09）

`v0.1.3` の tag と docs の追随の後、ユーザーが「Result 型を廃止し koka のような代数エフェクトを導入したい」と提起した。前日にコミットした 16 節「v0.2 の I/O と `do`」が v0.2 の I/O を Result 前提で設計していたため、同じ日に方針を反転させることになる。grilling で 20 問を確定した。決定は `docs/plan.md` 17 節。

**手順 1. 一次資料。** Koka の実装戦略を記憶ではなく原典で確かめた。Xie と Leijen の *Generalized Evidence Passing for Effect Handlers*（ICFP 2021）。分かったのは、Koka が multi-prompt monad への翻訳で「特別なランタイム機構を要求せず」C に落ちること（p.71:2）、monadic 翻訳後は「全ての効果を持つ関数が Pure か Yield を返す」こと（p.71:12）、そして §48 に直撃する一文「any direct tail-recursive calls are no longer directly tail-recursive as they occur under a lambda now!」（p.71:11）。回復条件は bind-inlining と join-point sharing による Pure 分岐の fast path で、full yield が起きると slow path に落ちる。

**手順 2. コードベースの実測。** 影響範囲は事前の見立てより小さかった。

| 対象 | 実測 |
|---|---|
| 受理コーパス 54 本のうち Result 使用 | 6 本 |
| 同 Option 使用 | 2 本 |
| 同 `?` / `do` 使用 | 3 本 |
| 拒否コーパス | E303 E504 E507 E508 E509 の 5 ディレクトリ |
| 仕様 2,039 行のうち改訂対象（§1 §21-24 §37 §38 §42 §44 §47-51） | 466 行（22%） |
| 外の `def` 139 本のうち戻り型を書いているもの | 24 本（17%） |
| 契約内の `def` | 18 本 |
| `let p? =` と `do` の lowering | `mono.rs:660` の `propagation`。成功 tag の projection と失敗 tag の再包装への branch |

`propagation` が tag 検査と分岐に落ちていることは、monadic 翻訳の Pure / Yield と同形であることの根拠になった。書き換えの主戦場はコードではなく仕様と `infer.rs` の型推論である。

**手順 3. grilling（20 問）。** ユーザーが方向を変えた点。

- Q3 効果の深さ。推奨は tail-resumptive と中断だけの最小形だったが、ユーザーは一般 `ctl`（1 回再開）を選んだ。
- Q6 §48 の運命。一度「`resume` を末尾位置に限る」で確定したが、ユーザーが「代数エフェクトの本来の目的を考えるとやはり中間 `resume` はできるべき。パフォーマンスは書き方を分離して、末尾保証されたものとコスト許容するものに分ける」と撤回した。Koka の `fun` / `final ctl` / `ctl` に対応する 3 種別へ改めた。Q5 で種別を効果宣言に固定していたため、どちらの保証が当たるかは呼び出し位置ごとに静的に決まる。
- Q12 効果行の綴り。ユーザーは後置と `{}` を希望した。`{}` 単独は R21 の名前付き field constructor（`constructor := upper_name [record_type]`）と衝突し、大文字名の直後の `{` を中を見ずに判別できないため LL(1) を失う。`!{...}` と `![...]` と `[...]` を出し、後置・記号なしの `Int [Raise]` に決まった。`<...>` は 13.2 が `<T>` を E100 に予約済みで使えない。
- Q13 と Q14 省略の規則。ユーザーの「シグネチャを書くときはデータ型は書くけど副作用は意識しなくていいのが理想」を型システムの言葉に翻訳すると、推論ではなく暗黙の効果変数（効果多相）になる。契約には本体が無いので推論できないが、効果変数なら意味が定まる。純粋の主張には `Pure` を書く。

Q8 でユーザーが「do だらけになるか」と迷ったので実測した。`do` は値を捨てる位置にだけ現れ、これは `?` を要求するかどうかと独立なので、`do` の個数はどちらの案でも同じである。差は `?` 1 文字だけと分かり、印を付けない案に決まった。

**手順 4. 自分の誤りと訂正。**

- EPT の制限を「一般 `ctl` では静的 evidence が使えない」と過大に述べた。実際の制限は scoped resumption であり（p.71:12）、`resume` を handler 節内限定のキーワードにすれば全ての再開が scoped になるので、単相化による静的 evidence は Q3 を一般 `ctl` にしても使える。advisor の指摘で訂正した。
- 効果行の綴りで §10 の後置に 2 度反した。`Fn(A => E B)` は `type_expr := type_atom type_ref*` により「`B` に `E` を与えた型」に読まれるので専用の区切りが要る（advisor の指摘）。`Raise[ParseError]` は後置なら `ParseError Raise` である（実例を書く過程で気づいた）。
- 「効果的な呼び出しにも `?` を要求する案は §42 が禁じる」と述べたが、印の無い形を E コードで拒否すれば canonical form は 1 つになり成立する。§42 が禁じるのは両方の綴りが通る場合である。

**手順 5. ユーザーの指摘から出た欠陥。** 最終確認の段で、ユーザーが「効果の順序ってなんだろう？順序は関係ない気がします」と Q1 の枠組みを疑った。仕様に評価順を定めた箇所は record の field（§12「式の評価順は書かれた順である」）と条件列（§15「上から順に評価し」）だけで、`->` が構文で並びを決める正格評価なので、順序は hado の問題ではない。16 節が実際に訴えていたのは効果の消失（E502 が条件の評価ごと落とす）と純粋性が型に無いことの 2 つで、順序ではなかった。Q1 の記述を直した。

この訂正の過程で欠陥が 1 つ出た。今 I/O は Result を返すので条件位置に置けないが、効果になると `is_ready ()` が `Bool [Io]` を返せるので条件位置に書けてしまう。すると §30 の E502 が `if is_ready () => x | x` に対して効果を持つ呼び出しの削除を要求する。条件列では E403 と E406 も条件を見るため、`if | read_flag () => a | read_flag () => b` という別々の 2 つの呼び出しが重複条件として拒否される。16.2 が「I/O が Result を返す限り §14 の『条件は Bool』がこの経路を塞ぐ」と書いていた守りを、Result の廃止が外すことになる。Q20 として、効果を持つ式を `let` / `do` の右辺・末尾位置・`|>` の連鎖に限る決定を追加した。引数位置の評価順を定める必要も同時に消え、効果を持つ呼び出しが必ず行頭に現れるので Q8 で印を外した損失も補う。

**手順 6. advisor の指摘。** 4 件を反映した。Q4 の「値化・保存・返却は禁止」が Q6 の改訂後も末尾位置限定と読めるので「節内なら位置は問わない」を追記。Q10（`main` は組み込み効果のみ）と Q14（省略は暗黙の効果変数）が衝突するので、`main` の効果行だけは一般化せず本体の効果で閉じることを 17.4 に明記。E303 は `?` 側のみ存続で `do` 側は新しい規則になることを明記。prelude の契約が Q14 で効果多相になる点が未決だったので Q19 として問い、純粋な 9 本に `Pure` を書くことに決まった。

**次の一手:** R23 の手順書を書く。17.6 の未確認 2 件をプロトタイプで潰してから、効果を足し `Result` を最後に撤去する。仕様への反映は手順書と一緒に決める。

## 0.2 の構文（2026-09-09）

17 節の決定を仕様本文へ落とす段で、構文が決まっていないことが分かった。grilling で 19 問を確定した。決定は `docs/plan.md` 18 節。

**入口の誤り。** 17.4 に「`/` の戻り型は `Int [DivisionError Raise]` へ」「`Option.ok_or` は `A [E Raise]` へ」を「機械的に決まること」として書いていたが、`Raise` という名前も型引数を取る形も grilling で問うていない。Q12 の綴りの例を書くときに具体名が要って置いた仮の名前が、そのまま導出の顔をして残っていた。ユーザーの「そもそも Raise ってなんだっけ？」で発覚し、17.4 の 2 行を撤回した。

**一次資料。** 効果の宣言と handler の綴りは、記憶で書かず Koka の実物を取って写した。`lib/std/core/exn.kk`:

```koka
pub effect exn
  final ctl throw-exn( exn : exception ) : a
pub value struct exception( message :string, info :exception-info )
```

`samples/handlers/basic.kk`:

```koka
effect<a> ctl yield( item : a ) : ()
effect state<s>
  fun get() : s
  fun put( x : s ) : ()
effect fun ask() : string
val amb = handler
  return(x)  [x]
  ctl flip() resume(False) ++ resume(True)
with final ctl raise(s) Nothing
```

ここから 2 つ分かった。`exn` は型引数を持たず payload を具体型 1 つに固定していること（ユーザーの「E ERROR じゃなくて ERROR でいい」と一致）、および効果一般は型引数を取れること（`effect<a> ctl yield`）。後者により、18.1 Q11 の射程を `Error` に限った。

**実測で決めた 3 件。**

| 論点 | 実測 | 結論 |
|---|---|---|
| 効果行に `{}` を使えるか | `parser.rs:914-915` が `type X = ...` の右辺で「大文字名 + `{`」を名前付き field constructor として 1 トークン先読みで確定する | 行を矢印の結果位置に限れば競合しない（Q8）。`{}` を採用（Q9） |
| 開いた行（`{Io, ...}`）が要るか | `types.rs:267` は `Ty::Var(id) => write!(f, "t{id}")`。型変数は既にソースに書けない形で印字されている | 効果変数も同様でよく、round-trip のためには不要。省略で足りる |
| エラー型の命名規約 | 仕様に規約は無い。存在するのは `DivisionError` と §34 の例の `Error` の 2 つだけ | payload 型の `-Error` 接尾辞は規約ではない。`Config Error` のような綴りは自由 |

**ユーザーが方向を変えた点。** 失敗の表現で 3 度動いた。契約（`module`）として提供して開発者に実装させる案 → 「いや、違うか、モジュールじゃなくてエフェクトだ」→ `effect Error`。契約案のときに E705（`main` に抽象型を含めない）と E603（`X.T` は実装がちょうど 1 つ）に当たることを示していたが、効果に切り替えたことで両方消えた。効果行の綴りでも、明示の効果変数 `E` を「不要」→「なかったら不便」→「やっぱり全部型推論」と 3 度動いた。最後の形（書く義務は無く、`Pure` と具体行の両方が任意）で落ち着いた。

種別の綴りは `fun` / `ctl` / `final ctl` を「`fun` が `fn` と 1 文字違い、`ctl` は hado に無い略語」として退け、`reply` / `abort` / `capture` にした。一度「短くしたい」と言われて `pass` / `stop` / `hold` を出したが、「まだ一般的とは言いづらい概念だし」で長い方に戻した。

**進め方の修正。** 抽象的な A/B/C を問うと回答が揺れ、実物のコードを並べると 1 往復で決まる、という傾向が途中で見えた。「koka と同じ例じゃないから分かりにくい」という指摘を受けて、以降は Koka の原典を引いてから hado の形を並べる進め方に変えた。最後は個別の問いを止め、全行に出所（決定 / Koka の原典 / 私の仮置き）を付けた草案を `scratchpad/v02-draft.md` に書いて一度に確認した。仮置き 6 件はそのまま承認され、種別の綴りだけが差し戻された。

草案を説明する過程で、例に置いた `effect Parse` がどこからも使われず E513 になることが分かり、`Ask` を使う例に差し替えた。

**次の一手:** `docs/spec.md` の改訂。§5、§21〜24、§36、§38、§40、§41、§44、§47〜51 の改訂と、`effect`・handler・`with`・`resume` の新節。17 節の未確認 2 件はプロトタイプで潰す（R23 の前）。

## 0.2 のプロトタイプ — `capture` のスタック消費（2026-09-09）

17.6 の未確認 1 を実測した。仕様の §48 の文言がこれに依存するので、仕様改訂の前に置いた。結果は `docs/plan.md` 17.8。

**環境。** Koka は未インストールで、docker hub の `kokalang/koka` は 2021 年の v2.0.16 しかない（論文と同時期で、short-cut resumption の最終実装を含むか不明）。ubuntu 22.04 のコンテナに公式インストーラで Koka 3.2.3 を入れて測った。ユーザーの環境は変えていない。インストーラは `-b` が引数を取るので `-f` だけを渡す。成功しても非ゼロで終わるため、Dockerfile の検証は `koka --version` に置き換えた。

**測ったプログラム。**

```koka
effect yielder
  ctl emit( i : int ) : ()

fun walk( i : int ) : <div,yielder> ()
  if i > 0 then
    emit(i)
    walk(i - 1)          // 末尾位置

fun modea( n : int ) : div int
  with handler
    return(_x) 0
    ctl emit(_i) resume(())
  walk(n)
```

handler は 3 形（末尾 resume・resume の後ろに仕事・生成器）、対照は 2 つ（効果なし、`fun tell` による tail-resumptive）。

**診断。** 数値と結論は 17.8 に置いた。束縛条件がスタックだと確定できたのは、`--ulimit stack=-1:-1` を付けた瞬間に全ケースが完走したからである。論文 p.71:11 の「any direct tail-recursive calls are no longer directly tail-recursive as they occur under a lambda now!」がそのまま観測された形で、`ctl` 操作は定義上必ず yield するため bind-inlining が作る Pure 分岐の fast path を決して通らない。

**17 節の 3 箇所が誤っていた。** 17.2 の表が `capture` のコード生成を「ヒープのフレーム列」とし、Q6 が §48 の書き換えを「消費先がヒープへ移る」とし、Q15 が short-cut resumption を「実装義務として引き受ける」としていた。いずれも私が論文の前半だけから推測した内容で、Koka は short-cut resumption を実装済み（p.71:3）でありながらこの結果である。3 箇所を実測に合わせて直し、6 節のリスクに約 3 万反復の上限を足した。

**ユーザーの判断。** 受け入れる案を選んだ。§48 は「`capture` を経由しない末尾呼び出しはスタックを消費しない」に限定し、上限をリスクとして文書化する。退けた案は、一般 `ctl` を諦める、`capture` を含むループを静的に拒否する、segmented stacks を実装する、の 3 つ。

**次の一手:** `docs/spec.md` の改訂。17.6 の未確認 2（`under` フレームの復元が hado の設定でも要るか）は残るが、これは実装方式の問題で仕様の文言には効かないので、仕様改訂を止めない。

## 0.2 の仕様改訂（2026-09-10）

17 節と 18 節の決定を `docs/spec.md` に反映した。0.1.2 と 0.1.3 と同じく仕様先行で、門番は赤のまま R23 以降のラウンドへ渡す。

**変えた節。** §1 の思想を 3 行に差し替え、§21 を「Option と失敗」、§24 を「伝播と分岐」に改題した。§22 の `?` を Option 限定にし、§23 の `do` を「値を捨てて継続する」に移した。§36 §37 §38 §40 §41 §44 §47 §48 §49 §50 §51 を追随させ、§57「効果」と §58「handler と `with`」を新設した。`Result` は仕様から 0 件になった。2 型引数の綴りの例は Result が消えたので `(K, V) Pair` に置き換えた。

**内部整合の実測。** 節参照（`第N節` と `§N`）は 58 見出しに対して全て解決する。キーワードの本文の列挙は 23 個で 18 節の宣言と一致する。旧綴り（`final ctl` `ctl` `fun` `Raise` `[Io`）の残存は無い。

**着手時点の赤（実測）。**

| 門番 | 結果 | 理由 |
|---|---|---|
| `verify-m0.sh` | 4/7 | `cargo test` が 2 件落ちる（下記） |
| `verify-m1-spec.sh` | 5/6 | 3 は `DivisionError` の廃止で恒久的に成り立たなくなった。改題を検出した検査 1 は `git diff` を見るので、コミットした時点で解消した |
| `verify-m1-corpus.sh` | 4/7 | 拒否コーパスが 64 本で、§50 から導かれる 69 本に足りない。E123 E209 E308 E522 E605 E706 が無く、E509 が余る |
| `verify-m2.sh` | 4/8 | 上記の波及と、E1xx の本数が 14 に合わない |
| `verify-m3.sh` | 11/12 | 内部で走らせる門番が赤 |
| `verify-m7.sh` | 2/3 | 同上 |
| `verify-r9.sh` | 4/5 | §41 の EBNF から生成したプログラムの往復 |
| 他 15 本 | 満点 | — |

`cargo test` の 2 件は `parse_corpus.rs:144` の `corpus_counts` と `r9_grammar.rs` の `grammar_generated_programs_preserve_every_declaration` である。前者は §50 の表から拒否コードの集合を導出してディスクと照合しており、後者は §41 の EBNF からプログラムを生成して往復させる。どちらも仕様を正として期待値を計算する作りなので、仕様を変えた時点で赤くなるのが正しい。AGENTS.md の「判定器に暗記を持ち込まない」がそのまま効いている。

`verify-m1-spec.sh` の検査 3 だけは性質が違う。`ArithmeticError=0` かつ `DivisionError>=1` を要求する v0.1 期の改名確認で、`DivisionError` を廃止した以上もう緑にならない。ラウンドの成果物として判定器を書き換える必要がある。緩めるのではなく、役目を終えた検査を落とす形になる。

**次の一手:** R23 以降の手順書を 19 節として書く。ラウンドの分割、成果物、判定条件、そして上の赤をどのラウンドで緑に戻すかを決める。

## 0.2 の仕様の独立審査（2026-09-10）

仕様改訂と手順書を deepseek-v4-flash に審査させた。重大 5 件・軽微 9 件を受け、全件を反映した。

**opencode が返らない原因を特定した。** 台帳の 0.1.3 の記録は「依頼文が『全部読んで 5 観点を一度に』だったこと」「`--dir` の外の `/tmp` を読ませたための権限待ちと推測」としていたが、どちらも違った。**opencode は疑似端末（TTY）を要求する。** Bash ツールは端末を与えないので、`init` の直後で止まり、CPU をほとんど使わないまま無出力で終わる。実測では 25 分で出力 0 バイト・CPU 10 秒だった。`--auto` を付けても、依頼を 1 観点に絞っても、空ディレクトリで `--pure` を付けても変わらない。`script -qec "..." /dev/null` で包むと 3 秒で返る。ユーザーが端末から実行すると返るのはこのためである。以後 `script -qec` と `--auto` を付け、出力はパイプを通さずファイルへ直接書かせる。パイプは EOF まで全部バッファするので、生きていても 1 バイトも見えない。

4 日間動いていた対話セッション（PID 1450427、CPU 1 時間 39 分、RSS 847 MB）はユーザーの許可を得て落とした。停止とは無関係だった。

**審査は 2 本に割った。** A は `docs/spec.md` だけを読ませ「仕様内部の矛盾」と「EBNF の導出可能性」、B は計画と周辺文書を読ませ「更新漏れ」「書けない正当なプログラム」「手順書の実行可能性」。所要は A が 7 分、B が 5 分。

**重大 5 件。**

| 指摘 | 実測 | 対応 |
|---|---|---|
| §58 の `def main => with ...` が書けない形 | §41 の `definition` は `parameters` を 1 個以上要求し（E109）、受理コーパスも `def main = create_user []` である | 2 例を `def main =` に直した |
| §58 の Collect の腕の `Cons(value, rest)` と `Nil` が E510 | §46 は式の `Cons` `Nil` を E510 とする | `[value, ...rest]` と `[]` に直した |
| handler の腕の `_` が §27 の列挙に無い | §27 は `_` 引数の場所を 2 つに限り、それ以外は E519 とする | 18.1 Q14 で決めていたのに §27 を直し忘れていた。handler の腕を第 3 の場所として足した |
| 拒否コーパスが 68 本にしかならず §50 の 69 行に届かない | 64 − 1（E509）+ 5（E123 E124 E308 E522 E605）= 68 | E706 の拒否コーパスが計画から抜けていた。R26 に足し、判定 26-d で本数を確かめる |
| §24 の効果の位置の規則が自己矛盾 | 「`let`/`do` の右辺、末尾位置、`|>` の 3 つである」（ホワイトリスト）と「条件位置、引数位置、field には書けず」（ブラックリスト）を両方書いていた。`if` の腕で解釈が割れる | ブラックリストに一本化し、分岐の腕は末尾位置なので書けると明示した。演算子の被演算子と scrutinee も禁止側に足した |

**軽微 9 件のうち反映したもの。** §34 と §41 の見出しの列挙に `effect` と `handler` を足した。§27 の E513 の列挙に効果と handler を足した。§50 の「効果の検査は型の段階に属する」は E123（構文）E522（冗長）E706（単相化）と食い違うので、段階が分かれることを書いた。§47 の「値の空間はファイルごと」と効果の操作の可視性の関係を §57 に書いた。§20 が handler の腕に効かないことを §58 に明示した。19 節に「ラウンド名は主題であって検査段階ではない」を足した。`docs/bug.md` の「§51 は `Ok(x)` を…」を現行の文言に直した。

**審査より先に自分で見つけたもの。** E308 の「節」列に 4 節を書いていた（§50 は「主となる節を 1 つ」と定める）。E209 を名前解決段階に置いていたが、`resume` の可否は構文的な入れ子だけで決まるので構文段階が正しい。E124 に移して R23 で完結するようにした。審査 A も E308 の節列を軽微として挙げており、独立に一致した。

**次の一手:** R23 に着手する。仕様は審査を 1 巡した状態である。

## 0.2 の仕様の独立審査 2 巡目（2026-09-10）

1 巡目の指摘を反映した状態を、同じ 2 本立てで審査させた。依頼は「探索」から「確認」へ絞り、直した箇所の反映と、修正が生んだ新しい矛盾を見させた。重大 4 件・軽微 8 件。所要は A が 7 分、B が 21 分。

**A（仕様）は 10 箇所の反映を全て確認し、新しい矛盾を 1 件出した。** 私が 1 巡目の軽微「`read_a () + read_b ()` も禁止では」に応えて「演算子の被演算子」を禁止側に足したが、`/` と `%` は `Int {Error}` を返す（18 節 Q16）ので、`x / y + 1` が E308 になり除算を含む算術が書けなくなる。

これを機に、禁止していた位置が性質の違う 2 つの理由で並んでいたことが分かった。条件と scrutinee は E502・E403・E406 の前提（条件が副作用を持たない）を守るためで、禁止しなければ直せない。引数と被演算子は評価順が未規定だったためで、順序を定めれば済む。ユーザーの判断で前者だけを禁止に残し、後者は §44 に「二項演算子の被演算子は左から右、適用の引数も書かれた順」を書いて解いた。17 節 Q20 と 17.3 を追随させた。失うのは「効果を持つ呼び出しが必ず行頭に現れる」性質だが、これは Q8 で印を外したことを後から補う副産物で、Q20 の目的ではなかった。

A の残る重大は、契約 member の戻り型が `type_expr` のままで `def save(user: T): Unit {Error}` が文法から導けないこと。`module_item` の 2 行を `result` に直した。

**B（計画）は本数を独立に検算し、64 − 1 + 6 = 69 が §50 の行数と一致することを確かめた。** 重大 2 件はどちらも連鎖の見落としだった。

| 指摘 | 実測 | 対応 |
|---|---|---|
| `E508/` のコーパスが Result を使う | `tests/reject/E508/main.hd` は `(Int, String) Result` と `Ok(` `Error(` を持つ。Result が消えると型名が解決できず E204 で落ち、E508 として機能しない | 17.4 に「E508 は Option 限定に縮小」と書きながら、コーパスの書き直しをどのラウンドにも割り当てていなかった。R25 に足した |
| 18.4 の 6 項目に `// item:` タグの割り当てが無い | `verify-m1-corpus.sh` の検査 3 が項目ごとに受理と拒否を要求する。R27 で判定器に 18.4 を足すと 6 項目分が必要になる。R18〜R22 の表は item を明記していた | 19 節に割り当て表を足した |

**軽微で効いたもの。** 除算の `raise` が運ぶ `message` を仕様が定めていなかった。`tests/accept/division_error/main.hd` は `expect-error: DivisionByZero` を期待するが、v0.2 の仕様に `DivisionByZero` の文字列は 1 件も無い。§36 に定めた。E303 の Option 限定への狭めを R24 に置くと `stdlib/result.hd:7` の `let x? = value`（右辺が Result）が落ちて prelude が検査不能になるので、R25 へ移した。r9_grammar の生成器が EBNF から `abort x: Int` や裸の `resume` を作りうるので、生成対象から外すことを 23-a に書いた。新設 6 コードの位置摂動プローブの判定が無かったので 23-c・24-d・26-e に足した。各ラウンドで許容される赤の内訳が無かったので表を足した。

**手順の反省。** 審査 A が仕様を読んでいる最中に §48 へ末尾位置の定義を足した。審査対象を動かしながら審査させたので、A の §48 関連の指摘は現物と突き合わせ直す必要があった。次巡からは審査中に対象を編集しない。

**自分で見つけたもの。** 「末尾位置」が §24 と §48 で使われているのに定義が無かった。v0.1 では `def rec` が唯一のループで自明だったが、§24 が「分岐の腕は末尾位置なので書ける」と書いた時点で未定義語に依存する。§48 に定義を置いた。除外リスト（適用の引数、演算子の被演算子、record 式の field、分岐の条件と scrutinee）は §24 の禁止リストとは別物で、§48 はスタックの保証、§24 は冗長規則の前提と、目的が違うことを明示した。

**次の一手:** R23 に着手する。仕様は審査を 2 巡した。

## 0.2 の仕様の独立審査 3 巡目（2026-09-10）

依頼に「読んだファイルの内容を貼らない」「調査は手短にし結論を必ず書く」を足し、§24 を緩めた影響を名指しで確認させた。重大 3 件・軽微 8 件。所要は A が 6 分、B が 6 分。2 巡目の B が 21 分・43KB だったのに対し、A 3.3KB・B 10.5KB に収まった。出力の制約が効いた。

反映の確認は A が 7 件、B が 6 件、いずれも全て「反映済み」で指摘なし。3 巡続けて反映の質は安定している。

**A の重大 1: scrutinee の禁止に根拠が無い。** §24 は禁止の理由を「E502・E403・E406 の前提を守る」としていたが、これは条件にしか及ばない。実測すると、§30 の E502 は「**Boolean branch** の両方の結果が同一なら」と対象を限っており、§33 の E507 は `if x | Some(v) => Some(v) | None => None` を `x` に畳むが scrutinee はそのまま残る。E403 と E405 は腕を落とすだけである。**scrutinee を落としたり混同したりする冗長規則が 1 つも無い。** ユーザーの判断で禁止を分岐の条件だけに絞った。

禁止は 3 巡かけて 3 段階に縮んだ。当初は「`let`/`do` の右辺・末尾位置・パイプ以外すべて」、2 巡目で引数と被演算子を外し、3 巡目で scrutinee を外した。残るのは E502 と E406 が条件の純粋性に依存する 1 点だけである。§24 に scrutinee を禁じない根拠も書いた。

**A の重大 2: 別ディレクトリから操作を呼ぶ綴りが文法に無い。** 2 巡目の軽微（§47 の「値はファイルごと」と §57 の「操作は値空間」の衝突）に応えて「操作は効果の宣言があるディレクトリ全体から見える」と書いたが、文法を確かめていなかった。値の修飾は `path? upper_name "." name` の形しか無く（§41）、小文字の操作名を別ディレクトリから綴る手段が無い。ユーザーの判断でこの一文を撤回し、操作は効果を宣言したディレクトリの中だけで書けるとした。別のディレクトリの効果は、行に現れても操作を起こせず、その効果を持つ関数を呼ぶ形になる。

**B の重大: 18.5 の E コード表が緩和前のまま。** §24 を緩めたとき 17.1 Q20 と 17.3 は直したが、18.5 の E308 の誘因が「条件位置・引数位置・record field 式」のまま残っていた。R24 の実装者がこれを読めば、許可された位置を拒否する実装を書く。緩和の 6 点修正のうち 1 箇所だけの漏れだった。

**B の軽微で効いたもの。** 緩和後の `E308/` に「引数位置に効果」を書くと受理されて E308 として機能しない。19 節に内訳を明記し、受理側の `effect_position/` には許した位置の例を並べて狭めの回帰を検出できるようにした。判定 25-c が要求する失敗系は `tests/accept/division_error/main.hd` の `expect-error: DivisionByZero` が持つので、`abort` の実行経路が R25 に要る。R26 に置いた割り当てと衝突していたので、最小の経路を R25、handler 経由と `return` の腕を R26 に分けた。

**A の軽微。** §48 の末尾位置の定義で「分岐（第14節から第17節）の腕」と「条件列の腕と fallback」が範囲として重複していたので統合した。`abort` の呼び出し位置の式の型が未規定だったので、「その位置が要求する型に合わせる。返らないので、どの型とも矛盾しない」と定めた。

**手順。** 2 巡目の反省どおり、審査中は仕様を編集しなかった。監視スクリプトに欠陥があり、`pgrep -c` が 0 のとき改行を含む値を返して終了条件が効かず、完了後も回り続けた。

**次の一手:** R23 に着手する。仕様は審査を 3 巡した。

## 0.2 の仕様の独立審査 4 巡目（2026-09-10）

重大 3 件・軽微 4 件。所要は A が 5 分、B が 6 分。反映の確認は A が 5 件、B が 4 件、いずれも全て「反映済み」。

**疑わせた副作用のうち 3 点は「矛盾なし」と確認が取れた。** scrutinee を許したことと §16・§18・§31・§32・§33 の関係（E507 は scrutinee を残し、E403 と E405 は腕だけ落とす）、prelude の `raise` がどこからでも書けることと §55 の関係（prelude は根に連結され、根の名前はどこからも無修飾で参照できる）、`abort` の型と §5 の E701 の関係（ボトム型として文脈の型に確定し型変数を残さない）。

**A の重大: handler の置き場が未規定。** 3 巡目で操作を宣言ディレクトリ内に限ったが、§58 は handler が「効果の全ての操作を過不足なく実装」することを要求し、腕は操作名を綴る。効果と別ディレクトリの handler は腕を書けないのに、§58 に置き場の規定が無かった。「handler は処理する効果と同じディレクトリに置く。別のディレクトリに置くと E204」と定めた。3 巡目の修正の波及を見落としていた。

**A の軽微で効いたもの。** §47 が `prelude の constructor Some None Ok Error` と列挙したままだった。`Ok` と `Error` は Result の constructor で v0.2 には存在せず、`Error` は効果の名前で契約の空間にある。仕様から Result を消したときの取りこぼしで、1〜3 巡目も拾えていなかった。4 巡目で初めて出た過去の残骸である。同一ディレクトリの別ファイルから操作を書けるかも未確定だったので、「値の空間でファイルごとの例外を持つのは操作だけ」と明示した。

**B の重大 1: 18.4 の「効果の位置」が 3 巡連続で取りこぼされていた。** 「`let` と `do` の右辺、末尾位置、`|>` の連鎖に限る」という当初の記述のまま残っており、2 巡目の緩和で既に古く、3 巡目の scrutinee 緩和でさらに乖離していた。17.1 Q20・17.3・18.5・§50 は毎回直したのに 18.4 だけ 3 回とも漏れた。同じ内容が 5 箇所に散っていることが原因である。

**B の重大 2: R25 が R26 に前方依存しうる。** 「受理コーパス 54 本を新しい綴りへ移す」としか書いておらず、移行先の形を定めていなかった。効果と handler を使う形へ移すと `abort` と `return` の handler コード生成が要るが、それは R26 の成果物である。移行先を 2 つに限った。値の不在は Option と `?` へ、理由のある失敗は `raise` を `main` まで通す形へ移し、handler を使う形へは移さない。handler を使う受理コーパスは R26 が新設する。

**B の軽微。** ディレクトリ限定の規則（操作は宣言ディレクトリの中だけ、handler は効果と同じディレクトリ）を検証する判定がどのラウンドにも無かったので、判定 24-e に足し、R24 の成果物に `names.rs` と `namespaces.rs` を加えた。

**次の一手:** R23 に着手する。仕様は審査を 4 巡した。

## R23 構文（2026-09-10）

代数エフェクトの字句と構文を入れた。キーワード 9 個（`effect` `handler` `with` `resume` `return` `reply` `abort` `capture` `Pure`）、`effect_section` `handler_section` `with_form`、primary の `resume`、`result := type_expr effect_row?` を足した。拒否コーパスに `E123/`（`abort` の操作に戻り型）と `E124/`（`capture` の外の `resume`）を新設し、66 本になった。

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）を herdr 経由で回した。工程順（コーパス → lock → 実装）はコミット 3 本に分かれている。

**計画の内部矛盾を 2 件、指揮者が裁定した。**

19 節の成果物表は R23 に `tests/accept` を許さないのに、18.4 の割り当て表は受理側 `effect_forms/` `handler_forms/` を R23 に置く。R23 は型を見ないので受理コーパスは `check` を通らない。受理側を R24（`tests/accept` を成果物に持つ）へ送った。

判定 23-a は `verify-r9.sh` の緑を求めるが、その検査 R9-1 は `parse_corpus::corpus_counts` を同梱しており、判定 25-b と「各ラウンドで許容される赤」の R26 行は corpus_counts を R26 まで赤と定める。**門番は分割せず 4/5 のまま残した。** 23-a の本文が要求するのは「§41 の EBNF から生成したプログラムが往復する」ことなので、`verify-r23.sh` が `r9_grammar` を直接判定する形にした。

**審査は 2 件を指摘し、1 件が妥当、1 件が誤検出だった。**

妥当: `verify-r23.sh` が `r23_syntax` だけを走らせ、判定 23-d の「上記を判定する」に対して 23-a と 23-b を判定していなかった。3 検査の形に直した（`cbec92a`）。E1xx の集合は §50 の表から `sed` で導出し、ディスクの実数と照合する。件数は直書きしていない。

誤検出: 「`effect E reply ask: String def main = 1` が 1 行で受理されるのは §57 の節境界の違反」。`docs/spec.md:1754` が「パーサは改行を見ない。」と定め、§41 が空白は意味を持たないとする。改行で分けた形と 1 行に詰めた形の両方を実測し、どちらも受理されることを確かめて棄却した。`parser.rs` は変更していない。既存の `module` 節が後続の `def` を飲んで E100 になるのは、契約 member も `def` で始まり区別できないという別の理由である。

**着手前に §50 の表から期待値を独立に導出した。** E コード 69 個、うち E1xx が 15 個。ディスクの拒否コーパスは 64 本で、不足が E123 E124 E308 E522 E605 E706 の 6 件、余剰が E509 の 1 件。64 − 1 + 6 = 69 が計画の 26-d と一致する。

**実測（R23 終了時）。** 赤は着手時点と同じ 7 本（m0、m1-corpus、m1-spec、m2、m3、m7、r9）で、新しい赤は無い。`cargo test --offline --no-fail-fast` の失敗は `parse_corpus` の 1 ターゲットだけになった（着手時は `parse_corpus` と `r9_grammar` の 2 本）。

| 判定 | 実測 |
|---|---|
| 23-a | `cargo test --test r9_grammar` 緑。`verify-r9.sh` は 4/5 で、NG は R9-1 が同梱する `corpus_counts` のみ |
| 23-b | `verify-m2.sh` の検査 2 `OK: 2 e1xx-15+swap` |
| 23-c | `r23_syntax` 9 tests ok。§57/§58 の例が全て parse、E123 は `:`、E124 は `resume` にアンカー |
| 23-d | `verify-r23.sh` `PASSED: 3/3`、exit 0 |

仕様への疑義（builder が挙げ、指揮者が本文優先で処理）: 計画 18.3 の handler は `functor_param`・引数なしの `with`・型引数なしだが、§41 本文は `typed_param`・`with_args`・`type_vars` を持つ。本文に従った。

**次の一手:** R24 型。効果行の推論と単一化、E308 E522 E605、受理側 `effect_forms/` `handler_forms/` `effect_row/` `effect_position/`。
