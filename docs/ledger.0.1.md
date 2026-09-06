# 実行台帳 v0.1

`docs/plan.0.1.md` を code-perfection-multiagent で実行した記録。ラウンド単位で追記する。

## 役割割当

| 役割 | エージェント | モデル | 実行方式 |
|---|---|---|---|
| commander | claude（本セッション） | claude-opus-5[1m] | — |
| planner | cursor-agent | cursor-grok-4.6-xhigh | headless（`-p --force --trust`）|
| plan-reviewer | opencode | opencode/muse-spark-1.3-contributor-free | headless（`run --dir`）|
| builder | opencode | opencode/muse-spark-1.3-contributor-free | headless（Herdr ペイン内で実行）|
| reviewer | cursor-agent | cursor-grok-4.6-xhigh | headless（`-p --force --trust`）|
| fixer | opencode | opencode/muse-spark-1.3-contributor-free | builder と同系 |

ユーザー明示指定に従う。スキル既定からの逸脱と理由:

- planner を自分ではなく cursor に割り当てた（スキルは「planner は常に自分」）。ループの指揮・戻り先の最終判定・ラウンド分割は commander が握る。
- muse spark は `-contributor-free` の版を使う（ユーザー指定・2026-09-06）。R1 は `opencode/muse-spark-1.3` で実施済み。
- **builder は常に muse spark**（ユーザー指定・2026-09-06）。planner の cursor は plan と spec を書いてよい。
- plan-reviewer を deepseek-v4-flash から muse spark へ、reviewer を deepseek-v4-flash から cursor-grok-4.6-xhigh へ変更（ユーザー指定・2026-09-06）。R1 は変更前の割当で実施済み。
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

対象: `docs/plan.0.1.md` M0（Cargo ワークスペース、C ランタイム、`def main = 1 + 2` の歩く骨格、手組み CLIF の 1 億深さ）

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
- `scripts/verify-r1.sh` を `scripts/verify-m0.sh` に改名。"R1" はセッション固有のラウンド ID でリポジトリには無意味なため、`docs/plan.0.1.md` の M0 を指す名前にした

**既知の債務（後続ラウンドで整理する。R1 の判定材料にはしない）**

- `walk.rs` / `tail.rs` が生成物を `std::env::temp_dir()`（`/tmp/shin-<name>-<pid>/`）に置き、消さない。実行のたびにゴミが溜まる。サンドボックス内の reviewer から触れない原因でもあった。`target/` 配下に置くのが正しい
- `scripts/verify-m0.sh` のチェック 5〜7 は M0 時点の構造（`emit.rs` にフラグ設定がある等）を grep で固定している。M6 で `repr` / `emit` を書き換えると誤検知になりうる

**機構的ロス（3 回。根は 1 つ）**

plan-reviewer 1 回・builder 1 回・reviewer 1 回が、いずれも作業ディレクトリの外（`/tmp/vr1_pristine`、`/tmp/r1bak`、`/tmp/shin-tail-*`）へ復元用コピーを作る／読むために停止した。根は 1 つ — **欠陥注入を課す役割に、サンドボックス内の復元点を渡していなかった**。

解決は `git add -A` を復元点にすること。`git checkout -- .` はインデックスから作業ツリーを戻すので、コミットを作らずに「注入 → テスト → 復元」が閉じる。以後すべての発注に定型句として入れる。

セッション途中でユーザーが opencode の `permission.external_directory` に `/tmp/**` を追加したため、この経路での停止は以後起きない。ただし `~/git/**` も許可されているので、**reviewer の隔離は構造ではなく契約による**ものになっている（使い捨てコピーの外に出ようと思えば出られる）。

### R2 — M1 前半: 仕様の穴埋め

対象: `docs/plan.0.1.md` M1 手順 1（仕様への書き込み）。コーパスと `corpus.rs` は R3。

**ユーザー確認（2026-09-06）**

- 計画 2.3 の推定既定 13 項目は**すべて採用**
- `ArithmeticError` → **`DivisionError`**。v0.1 の variant は `DivisionByZero` だけで、返すのも Int の `/` と `%` だけ。加減乗は wrap するので算術一般のエラーは存在せず、広い名前は嘘になる
- spec 本文は planner（cursor）が書き、コーパスは builder（muse spark）が書く

**commander が計画を書く前に対象を読んで見つけた、計画自体の誤り**

`docs/plan.0.1.md:131` は「§38 struct を除外一覧から外す」と指示するが、**§38 の除外一覧に `struct` は無い**（実在するのは functors / first-class modules など）。実際に直すべきは §35（`spec:994`）の「v0.1では `sig` を優先し、実装側の詳細なmodule syntaxは後回し」。読まずに写していれば、実装役が存在しない行を探して往復を消費していた。

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

- `docs/spec.0.1.md`: 1318 → 1591 行。§1〜§42 の見出しは無傷、新設は §43〜§50 の 8 節（リテラルとコメント / 演算子・優先順位・短絡評価 / 構造比較の対象型 / List リテラル / 名前空間 / 末尾呼び出しの保証 / prelude / エラーコード）
- `docs/spec-coverage.0.1.md`: 計画 2.2 の 22 項目と 2.3 の 13 項目、計 35 行の対応表。**35 項目のどれかが黙って落ちることを検出可能にする**のが役割で、仕様本文だけを見ても書き忘れは誰にも見えない
- `scripts/verify-m1-spec.sh`: 受け入れ条件 6 件の門番。エラーコード 39 件（E1xx:3 E2xx:3 E3xx:4 E4xx:6 E5xx:16 E6xx:2 E7xx:5）

**仕様レビューの指摘 4 件（判定は合格。commander が R3 を塞ぐと判断して修正させた）**

機械検証は「壊していない・書き漏らしていない」しか見ない。内容の整合は宣言どおり人が diff を読んで拾った。

1. **E602 の発火条件が本文に無い** — 表には `E602 opaque-type §35` があるが、§35 本文は不透明性を性質として述べるだけで「何をしたら E602 か」が無い。他 38 コードは本文にアンカーを持つ。R3 で拒否コーパスを 1 ファイル 1 コードで書くので、接地が無いと E602 のコーパスが書けない
2. **record の文法と例が食い違う** — §41 の `"{" (name "=" expression ";")+ "}"` は各 field の後に `;` を必須にするので、§12 の例 `{ name = "a"; age = 3 }` が自分の文法を通らない。`;` が区切りか終端子かが決まっていなかった。区切り形に直した（決定 `plan.0.1.md:46` の例と整合）
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

