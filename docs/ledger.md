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

## R24 型（2026-09-10）

効果行の推論と単一化を入れた。行は集合として扱い順序を持たない。効果変数は宣言せず一般化のときに暗黙に量化し、`Pure` は空の行、書いた行は上界として閉じる。E305 を効果の不一致に当て、E308（効果を持つ式を分岐の条件に置く）、E522（行の重複）、E605（handler の過不足）を実装した。拒否コーパスは 69 本になった（集合としての一致は E509 の削除と E706 の追加を待つ）。

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）。18.4 の受理側 `effect_forms/` `handler_forms/` `effect_row/` `effect_position/` は R23 から送られた分を置いた。`error_effect/` は `stdlib/error.hd` が R25 の成果物なので R25 へ送った。

**門番が緑のまま欠陥が 5 件通った。** 初回実装は `verify-r24.sh` が緑（2 検査）で、赤の門番も着手時の 7 本から増えていなかった。それでも審査が 5 件を出し、全件が実測で再現した。初版の判定器は「E308 E522 E605 が §50 の表と一致する」という**存在**しか見ておらず、判定 24-a〜24-e が名指しする**振る舞い**を判定していなかった。判定条件がコーパスではなく `verify-r24.sh` に置かれる設計（判定 24-b）は、判定器がそれを実装しなければ空手形になる。

**fixer を 3 回回した。** 規律は 1 回だが、各巡が同じ指摘の再発ではなく新しい欠陥を露わにしたので継続した。逸脱の記録として残す。

| 巡 | 指摘 | 実測（前 → 後） |
|---|---|---|
| 1 | 値定義 `def cached = ask` の効果行が破棄される | `String` → `String {Ask}` |
| 1 | 未確定の条件の効果がすり抜ける | 受理 → `E308` |
| 1 | 効果多相が引数関数の行を固定する | `E305` → `Int {Ask, Io}` |
| 1 | 操作の型引数が単一化前に文字列化される | `Unit {t387 Yield}` → `Unit {Int Yield}` |
| 1 | 別ディレクトリの handler が E605 になる | `E605` → `E204` |
| 2 | 宣言行の下で無注釈の引数関数の行が空に潰れる | `E305` → 受理 `Int {Io}` |
| 2 | 契約の 0 引数 member が効果行を落とす | 受理 → `E602` |
| 3 | 上界検査が効果名だけを比べ型引数を照合しない | 受理 → `E305` |

**打ち切り線を引き、前提を実測して越えなかった。** 2 巡目の時点で「3 巡目に `types.rs` / `infer.rs` の行の扱いでまた回帰が出たら fix を止めて設計に戻す」と決めた。3 巡目の指摘は `row_within`（types.rs:467）で、これは `&self` の純粋な述語なので構造的に単一化ができず、名前の部分集合しか見られない。10 行上の兄弟 `unify_open_closed` は名前検査のあと `unify_matching_labels` で型引数まで単一化しており、**正しい形が同じファイルに既にある**。1 巡目と 2 巡目が行の解法そのもの（尾を閉じる → 上界＋遅延検査）を変えたのに対し、3 巡目は現行設計の内側の欠落だった。修正の範囲を `row_within` と呼び出し元 1 箇所に限定し、`Row` の表現・`bind_row`・`unify_open_closed`・一般化・制約の生成に手が伸びたら中断せよと発注した。実際の差分は `types.rs` 14 行と回帰テスト 34 行に収まった。

**回帰は 14 セルで固定した。** 上の 8 件に加えて、健全性の対照（宣言行を超える効果は E305）と過剰拒否の対照（型引数が一致すれば受理）を両側に置いた。片側だけのテストは、健全性と引き換えに緑にする修正も、表現力と引き換えに健全にする修正も検出できない。

| セル | 実測 |
|---|---|
| 無注釈引数 × 型引数一致 | 受理 `Int {Int Yield, Io}` |
| 無注釈引数 × 型引数不一致 | `E305: effect row mismatch` |
| 注釈あり引数 × 型引数一致 | 受理 `Int {Int Yield}` |
| 注釈あり引数 × 型引数不一致 | `E305` |
| 宣言行 `{Io}` に `{Ask}` を渡す | `E305` |
| 呼び出し位置ごとの一般化（`g peek 1 + g tickit 1`） | 受理 `Int {Ask, Io}` |

審査は 4 巡（不合格 3・合格 1）。誤検出は 1 件で、2 巡目の「宣言行の下で `apply2` が壊れる」は指摘の書いた形（無注釈引数）では再現し、指揮者が最初に試した形（注釈あり引数）では再現しなかった。**構文を変えて再現しなかったことを反証にしないこと。**

**実測（R24 終了時）。** 赤は着手時と同じ 7 本で、新しい赤は無い。`cargo test --offline --no-fail-fast` の失敗は `parse_corpus` の 1 ターゲットだけ。`verify-r24.sh` は `PASSED: 2/2`、`verify-r23.sh` は `PASSED: 3/3`、`verify-r9.sh` は `PASSED: 4/5` を保った。

仕様への疑義。§37 の規範文は「`{Io}` と書いた関数が**他の効果も持てば** E305 とする」で、上界（本体 ⊆ 注釈）を意味する。同じ段落の要約句「具体行はちょうどそれだけ」を厳密一致と読むと食い違うが、規範文を正として実装した。行が推論で開かれていないことを指す句と読める。

**R25 への申し送り。** `do yield "s" ->` は現在 `error[E303]: propagation requires Option or Result` になる。E303 の狭め（`do` の右辺を「空でない効果行を持つ式」にする）は R25 の成果物なので、これは回帰ではない。

**次の一手:** R25 失敗の効果。`stdlib/error.hd` の新設と `Result` の撤去、受理コーパス 54 本の移行、`abort` の最小の実行経路。

## R25 失敗の効果（2026-09-10）

`Result` 型を撤去し、失敗を `Error` 効果へ移した。`stdlib/error.hd` を新設して `result.hd` と `division_error.hd` を削除し、`Option.ok_or` を `A {Error}` に、純粋な 9 本に `Pure` を書いた。`/` と `%` は `Int {Error}` を返す。`mono.rs` の `propagation` を Option 限定にし、拒否コーパスから `E509/` を消して `E508/` を Option の再伝播に書き直し、`E303/` を Option 限定に狭めた。`main` まで届いた `raise` を stderr と終了コード 1 にする `abort` の最小の実行経路を置いた。

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）。

**着手時の赤が 7 本から 6 本になった。** `verify-m1-spec.sh` が `PASSED: 5/5` で緑に戻った（判定 25-d）。検査 3（`DivisionError` の存在確認）の削除は 19 節が明示的に許した、役目を終えた v0.1 期の改名確認の除去である。他の検査は触っていない。

**計画の内部矛盾を 1 件、緩めずに赤のまま残した。** 「各ラウンドで許容される赤」の R25 行は `verify-m1-corpus` の検査 1 を緑に戻すと書くが、判定 25-b は「本数が表の行数に揃うのは E706 を足す R26」と書く。検査 1 は拒否とコード表の 1 対 1 を見るので、E706 の拒否コーパスが無い R25 では緑にならない。検査器を緩めず赤のまま R26 へ渡した。

**審査は 2 件を指摘し、1 件が妥当、1 件を棄却した。**

妥当: `do` の効果検査が行の確定より先に走る。`def f x => do (x / x) -> 1` が `E303` になっていた。`/` の `{Error}` は数値の既定化で後から確定するが、`do` の検査がその前に走る。注釈を付けた `def f (x: Int): Int => do (x / x) -> 1` は通り、行が即座に分かる `do ask` も無注釈で通ることから、欠陥が「あとで確定する経路」に限られると特定した。制約として積んで解決フェーズで確かめる形に直した（`deb1fbf`）。

**これは R24 で直した欠陥の裏返しである。** R24 の 2 件目は「条件の効果が未確定なら純粋性の制約を残さず検査を終了する」（緩すぎて通す）、R25 は「未確定のうちに拒否する」（厳しすぎて弾く）。どちらも**検査を行の解決に対してどの時点で走らせるか**の問題で、単一化ベースの検査で型の確定を待つ判定は、制約として積んで解決後に検証しないと必ずどちらかへ倒れる。回帰は 4 形（無注釈の除算・注釈ありの除算・純粋な式・即確定の効果）で両側から固定した。

棄却: 「`tests/corpus.sha256` が `5154869` と `4e140c8` の 2 コミットで動いている」。AGENTS.md の規則は「**実装の工程で** 更新しない」であり、実装コミット `af05fd2`（`feat:`）は lock を触っていない。`4e140c8` は `test:` のコーパス工程（`stdlib/option.hd` と `tests/accept/rail_forms/main.hd` と lock）である。規則の意図は満たされており、発注文の言い回しの方が規則より厳しかった。

**実測（R25 終了時）。**

| 判定 | 実測 |
|---|---|
| 25-a | `tests/accept` と `stdlib` に `Ok(` `Error(` が 0 件。`stdlib/` は 6 本 |
| 25-b | 拒否コーパス 68 本（69 − E509）。`corpus_counts` は計画どおり赤 |
| 25-c | 既存の `expect:` タグの変更は 0 件。追加された 1 件は新設 `error_effect/` のもの |
| 25-d | `verify-m1-spec.sh` `PASSED: 5/5`、exit 0 |
| 25-e | `verify-r25.sh` 緑 |

赤は 6 本（m0、m1-corpus、m2、m3、m7、r9）。`cargo test --offline --no-fail-fast` の失敗は `parse_corpus` の 1 ターゲットだけ。

**前方依存を作らなかった。** 受理コーパスの `handler` / `with` の出現は `tests/accept/handler_forms/`（R24 が 18.4 の割り当てで作ったもの）だけで、移行先の 54 本には無い。19 節の「handler を使う形へは移さない」を守っている。

仕様への疑義（builder が挙げた）。§22 の例 `let name? = user.name -> name` は、現行の Option レール（継続も Option）だと E305 になる。0.1.3 も同じ形だった。仕様は変えていない。

**次の一手:** R26 種別ごとのコード生成。`reply` / `abort` / `capture`、E706、handler の実行。最大の未知。

## R26 種別ごとのコード生成（2026-09-10）

3 つの操作種別をコード生成に落とした。`reply` は呼び出し位置でインライン展開し、`abort` は tag 検査と分岐（0.1.3 の Result と同形）、`capture` は実行時の evidence とヒープの継続フレーム列で下げる。E706（`main` にユーザー定義効果）を実装し、拒否コーパスは 69 本になって §50 の表と集合として一致した。

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）。ラウンドが大きいので前半（`reply` / handler 経由の `abort` / `return` の腕 / E706 / コーパス）と後半（`capture`）の 2 発注に割った。

**着手時の赤 7 本のうち 5 本が緑に戻った。** R26 終了時の赤は m1-corpus・m2・m3・m7 の 4 本で、**いずれも原因が R27 の作業範囲にある**（§39 の典型コードに `handler Error as ToOption` が無い、判定器が 18.4 の 6 項目を読まない、`verify-m7.sh` が廃止した `Result` の member を直書きしている、m2 と m3 の複合検査がそれらを内包する）。`cargo test --offline --no-fail-fast` は 0 failed になった。

| 判定 | 実測 |
|---|---|
| 26-a | `reply_loop` の出力 `10000000`、最大 RSS 2048 KB。門番は 2 点（小 N と大 N）の RSS 比で定数性を判定する |
| 26-b | `division_error` は stderr に `DivisionByZero`、終了コード 1。0.1.3 の Result 版と同じ |
| 26-c | `capture_loop` の出力 `10000`、最大 RSS 2688 KB |
| 26-d | 拒否コーパス 69 本。`verify-m1-corpus.sh` の検査 1 が `OK: 1 reject-1:1`、`parse_corpus::corpus_counts` が緑 |
| 26-e | E706 のアンカーを位置摂動プローブで確認 |
| 26-f | `verify-r26.sh` 緑 |

**審査 1 巡目: 5 件（うち 4 件を実測で再現）+ 指揮者が 1 件を発見。**

| 欠陥 | 前 → 後 |
|---|---|
| 同じ関数を別 handler の下で呼ぶと最初の腕が使われる | `32` → `33` |
| 分岐の中の `abort` に `return` の腕が当たる | `1` → `0` |
| `capture` の無い handler で `return` の型変換が推論されない（§58 の `ToOption`） | `Int` / ビルド失敗 → `Int Option` / `Some(5)` |
| **handler の腕からトップレベルの定義が見えない**（指揮者が発見） | `E513` / `E204` → `7` |
| `capture` handler 内の純粋な補助関数の戻り型が書き換わる | 検証不能 → `3` |
| 門番が 26-a のメモリを判定していない | 出力のみ → 2 点の RSS 比 |

4 件目は指揮者が 5 件目を検証する過程で見つけた。`def double` を handler の腕から呼ぶと E204 になり、`def double` 自体が E513（未使用）と診断される。腕が名前解決と使用追跡の対象に入っていなかった。既存の受理コーパス（`capture_loop/` `handler_forms/`）は腕の中で局所束縛と `resume` しか使っていないため、この穴を踏んでいなかった。3 件目も同じ形で、`handler_forms/` の `return value => value` は型を変えないので、§58 の目玉の例（`ToOption`）が通らないことに気づけなかった。**指摘を検証する行為がコーパスの被覆の穴を暴いた。**

**審査 2 巡目: 3 件。設計と判定して方向を指示した。**

レビュアが実行して再現し、指揮者も確認した。3 件は別々の欠陥ではなく、**`return` の腕を「`with` の境界で 1 回」ではなく「`completion()` を分岐の下げ処理に再帰させて関数本体を書き換える」方式の 3 通りの現れ**だった。`grep -c "self.completion(" mono.rs` は 14 を返していた。

| 欠陥 | 前 → 後 |
|---|---|
| 入出力の型が同じだと `return` の腕を省略する（`return _ => 9`） | `3` → `9` |
| 非末尾の分岐にも変換が当たり二重適用される（`let x = (if … \| bail) -> x + 10`） | `15` → `14` |
| 純粋な補助関数の本体に変換が当たる | ビルド失敗 → `Some(6)` |

**R24 の 3 巡目との対比が判断の型になる。** R24 は `row_within` が `&self` で構造的に単一化できない**葉の欠落**で、正しい形が同じファイルの `unify_open_closed` に既にあったので、範囲を `row_within` と呼び出し元 1 箇所に厳密に限定した。R26 は方式そのものだったので、方向（`return` は `with` の境界で 1 回、特殊化される関数は自分の戻り型を保つ）を指示し、**構造的な検査**（`self.completion(` の呼び出しが `with` の下げ処理と `resume` の継続構築に収まること）を発注書に書いた。修正後は 14 → **2** になった。

同じ「3 巡目」でも前提を実測で確かめると対処が正反対になる。次代はこの 2 例を持ち出して判断すること。

**審査の質が契約で変わった。** R23〜R25 の指摘は「静的追跡、未実行」だったが、審査契約に「実行して再現できたかを明記すること」を入れた 2 巡目以降は、レビュア自身が再現入力を `/tmp` に置いて実行し、期待値と実測値を添えるようになった。誤検出が減り、指揮者の検証コストも下がった。

**回帰は 11 行で固定した。** `return value => value`（恒等）だけの `handler_forms/` は変換の適用回数を測れない。恒等関数はどんな適用回数でも同じ答えを出すからである。`return _ => 9`（同型だが非恒等）と `let x = (if … | bail) -> x + 10`（非末尾の分岐）を対照に入れて初めて「1 回だけ当たる」が判定可能になる。

**審査 3 巡目で打ち切り線に到達した。** 2 巡目の時点で「この修正後の審査で `mono.rs` の handler 下げに何か出たら、真偽を問わず R26 の修正を止める」と決めていた。3 巡目は 3 件を出し、全て実測で再現した。**いずれも入れ子に関わる。**

| ケース | 実測 | 期待 |
|---|---|---|
| `with Outer -> with Inner -> f 0`（別効果の handler を入れ子。`f` は `A` を raise、`Inner` は `B` の handler） | `12` | `7` |
| handler の腕の中に別の `with` を置く | `3` | `5` |
| `abort` の腕から効果付き関数を呼ぶ | ビルド失敗（`handler arms disagree on the answer type`） | 受理 |

1 件目は内側の handler が外側の効果の `abort` を捕まえており、**合法なプログラムに誤った答えを返す**。単なる未実装ではない。

これらは 19 節の R26 の箇条書きに無い。17.5「やらないこと」は「継続を別の handler の下へ運ぶ使い方」を範囲外とするが、これは `capture` 固有である。**`cross_abort` は継続を伴わない素の `abort` が非対応 handler を通り抜ける話なので、17.5 では覆えない。`abort` の入れ子について計画も仕様も沈黙している。**

判定 26-a〜26-f は全て満たされ、コーパスと門番も通る。3 件は `docs/bug.md` に記録した。修正には `$Fail` の tag が効果の同一性を運ぶ設計変更が要り、葉の修正では届かない。R27 は 4 本の赤がいずれも入れ子と無関係なので、依存しない作業として進める。`v0.2` を tag するかはユーザーの判断に委ねる。

## R27 仕上げ（2026-09-10）

判定器と周辺文書を 0.2 の語彙へ追随させ、**着手時の赤 7 本を全て緑に戻した。** 全 26 門番が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed（判定 27-a）。

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）。

**赤 4 本の手当ては、いずれも「暗記を導出に置き換える」形だった。**

| 門番 | 原因 | 手当て |
|---|---|---|
| m1-corpus 検査 2 | §39 の典型コードに §58 の handler の例が無い | `ToOption` の例を受理コーパスに足した |
| m1-corpus 検査 3 | 判定器が 18.4 の 6 項目を読まない | 対応表に 18.4 を足した（19 節は「検査 5」と書くが、現行スクリプトでは検査 4 が対応表） |
| m7 | `scripts/verify-m7.sh:16` が `"Result": "map map_error is_ok"` を**直書き**していた | §49 prelude からの導出に置き換えた |
| m2 検査 5 | `verify-m1-spec.sh:6/6` を**直書き**していた（R25 で検査を落として 5/5 になった） | 各スクリプトの `total=` から分母を導く形にした |
| m3 検査 6・11 | `type-round-20` と `stdlib-types` が Result 期の暗記 | §49 からの導出に置き換えた |

m7 と m2 と m3 は、AGENTS.md の「判定器に暗記を持ち込まない」に判定器自身が違反していた例である。R25 で `verify-m1-spec.sh` の検査 3（`DivisionError` の存在確認）を落としたのと同じ性質だが、**あちらは役目を終えた検査の削除、こちらは導出への置き換え**である。

**審査 1 巡目: 3 件。全て「判定器が検出できない嘘」だった。**

| 欠陥 | 前 → 後 |
|---|---|
| `tests/reject/E605/` が `// item: 失敗の効果` を名乗るが `reply Ask` の操作不足 | `Error` 効果の handler が `raise` を実装しない形へ |
| `tests/reject/E706/` が `// item: 末尾呼び出しの限定` を名乗るが再帰も `capture` も無い | `capture yield` を使う末尾再帰を handler 無しで呼ぶ形へ |
| `docs/spec-coverage.md:19` の `Unit payload` → §21 が §21 に規定が無い | 変えず、疑義として記録（下記） |

**全門番が緑でもタグの嘘は検出できない。** 判定器はタグの形式と集合しか見ないので、`// item:` が指す項目とファイルの中身が食い違っても緑になる。`docs/corpus.md` の「`item:` は被覆のために寄せない」が R6 の実例（`short_and.hado` の `-- item: 末尾呼び出し` は再帰が一度も走らない）を挙げて警告している型で、0.2 でも同じことが起きた。**18.4 の割り当てに合わせて中身を書くのが順序であって、タグを貼って数を満たすのは逆である。**

**仕様への疑義: `docs/plan.md:50` の `Unit payload` に 0.2 の対応規定が無い。** 項目の定義は「`Ok()` が canonical。`Ok(())` は拒否」で、`Result` を廃止したときに §21 と §50 から消えた。`docs/spec-coverage.md` は §21 に対応付けたままだが、実測で §21 に `Unit`/`payload` の記述は 0 件である。判定器は節が**存在する**ことしか見ないので緑になる。対応付けを捏造せず、そのまま残して疑義として記録した。plan.md 2.2 の項目自体をどうするかは計画の改訂なので、指揮者は判断しない。

**審査 2 巡目の 1 件は指揮者が棄却した。** 「コーパス変更（`32048ac`）と lock 更新（`701574e`）が別コミット」という指摘だが、AGENTS.md の規則は「**実装の工程で** `tests/corpus.sha256` を更新しない」であり、両方とも `test:`（コーパス工程）で、実装コミットは lock を触っていない。`sha256sum --strict -c tests/corpus.sha256` は HEAD で通る。**指揮者が審査契約に「同じ `test:` コミットで」と規範より強い条件を書いたのが原因で、R25 でも同じ型の誤りをした。** 審査契約は規範を引き写すか、強めるなら「これは規範より強い」と明示すること。

**最終実測（R27 終了時）。**

| 門番 | 出力末尾 | exit |
|---|---|---|
| `verify-m0.sh` | PASSED: 7/7 | 0 |
| `verify-m1-corpus.sh` | PASSED: 7/7 | 0 |
| `verify-m1-spec.sh` | PASSED: 5/5 | 0 |
| `verify-m2.sh` | PASSED: 8/8 | 0 |
| `verify-m3.sh` | PASSED: 12/12 | 0 |
| `verify-m4.sh` | PASSED: 83/83 | 0 |
| `verify-m5.sh` | PASSED: 118/118 | 0 |
| `verify-m6.sh` | PASSED: 44/44 | 0 |
| `verify-m7.sh` | PASSED: 3/3 | 0 |
| `verify-r9.sh` | PASSED: 5/5 | 0 |
| `verify-r11`〜`r14`, `r16`〜`r22` | PASSED: 1/1 | 0 |
| `verify-r23.sh` | PASSED: 3/3 | 0 |
| `verify-r24.sh` | PASSED: 2/2 | 0 |
| `verify-r25.sh` | PASSED: 4/4 | 0 |
| `verify-r26.sh` | PASSED: 2/2 | 0 |
| `verify-record-constructors.sh` | PASSED: 1/1 | 0 |

`cargo test --offline --no-fail-fast` は 0 failed。`git status --short` は空。

**判定 27-c（`v0.2` の tag）は保留した。** `docs/bug.md` の B4（入れ子の handler）が未修正で、うち 1 件は合法なプログラムに誤った値を返す。版が何を約束するかはユーザーの判断なので、指揮者は tag を打たずにユーザーへ渡す。

## R23〜R27 の指揮の記録（2026-09-10）

5 ラウンドを claude（opus-5）が指揮し、cursor（cursor-grok-4.6-high）が実装、codex（gpt-6-astra low）が審査した。herdr 経由で操縦し、実装は herdr ペイン、審査は `codex exec` の非対話で回した。

**審査 12 巡で 21 件の欠陥を検出し、うち 18 件を修正した。** 誤検出は 3 件（R23 の節境界、R25 と R27 の lock コミット）で、うち 2 件は指揮者が審査契約に規範より強い条件を書いたことが原因である。

**指揮者が実測で検証しなければ、誤検出と本物を区別できなかった。** R23 の「1 行に詰めた `effect` 節が受理される」は `docs/spec.md:1754`「パーサは改行を見ない。」で棄却したが、R24 の 5 件は全て「静的追跡、未実行」と自己申告されながら全件が再現した。**自己申告の確信度は判定基準にならない。**

**審査契約に「実行して再現できたかを明記すること」を入れると審査の質が変わった。** R23〜R25 の指摘は差分の静的追跡だったが、R26 の 2 巡目以降はレビュア自身が再現入力を `/tmp` に置いて実行し、期待値と実測値を添えるようになった。誤検出が減り、指揮者の検証コストも下がった。

**「fixer は 1 回だけ」の規律から 2 度逸脱した。** R24 で 3 回、R26 で 2 回回した。判断基準は「同じ指摘の再発か、新しい欠陥か」で、各巡が新しい欠陥を露わにする限り収束とみなした。逸脱にあたっては打ち切り線を先に引き、**その前提を実測で確かめた。**

**同じ「3 巡目」でも前提が違えば対処は正反対になる。**

| | R24 3 巡目 | R26 2 巡目 |
|---|---|---|
| 症状 | 1 件 | 3 件 |
| 原因 | `row_within` が `&self` で構造的に単一化不能 | `completion()` が分岐の下げ処理に 14 箇所散っている |
| 正しい形 | 同じファイルの `unify_open_closed` に既にある | 存在しない |
| 判定 | 葉の欠落 → 範囲を厳密に限定 | 設計 → 方向を指示 |
| 結果 | 差分 14 行に収束 | `completion()` が 14 → 2 箇所 |

**R26 の 3 巡目で打ち切り線に到達し、守った。** 入れ子の handler の 3 件を `docs/bug.md` の B4 に記録し、修正せずに R27 へ進んだ。R27 の 4 本の赤はいずれも入れ子と無関係だったので、AGENTS.md の「依存しない作業は進め、依存する作業は止める」がそのまま適用できた。

**回帰は両側の対照で固定した。** 健全性だけを見るテストは表現力と引き換えに健全にする修正を通し、表現力だけを見るテストは健全性と引き換えに緑にする修正を通す。R24 は 14 セル（引数の注釈の有無 × 型引数の一致／不一致 ほか）、R26 は 11 行（`return _ => 9` の同型・非恒等、`let x = (if … | bail) -> x + 10` の非末尾分岐 ほか）。**`return value => value` のような恒等の腕は適用回数を測れない。**

**門番が緑でも欠陥は通る。** R24 は初版の `verify-r24.sh` が緑のまま 5 件を通し、R26 は初版が 6 件を通し、R27 は全門番が緑のまま 3 件の `item:` タグの嘘を通した。緑は「判定した範囲で正しい」以上を意味しない。判定条件がコーパスではなく門番に置かれる設計（判定 24-b など）では、判定器がそれを実装しなければ判定条件は空手形になる。

**計画の内部矛盾を 3 件裁定した。** (1) 18.4 の受理側 2 ディレクトリを R23 から R24 へ送った。(2) 判定 23-a の `verify-r9` は R9-1 が `corpus_counts` を同梱するため分割せず、`verify-r23.sh` 側で `r9_grammar` を直接判定する形にした。(3) R25 の「`verify-m1-corpus` の検査 1 を緑に戻す」は E706 が R26 の成果物である限り成り立たないので、緩めずに赤のまま渡した。**いずれも門番を緩めない側に倒した。**

## R28 入れ子の handler（2026-09-10）

R26 が打ち切った `docs/bug.md` の B4 を直した。`$Fail` の実行時 tag に効果の同一性を持たせ、handler の腕の中の `with` の `return` の抑制を外し、`abort` の腕から効果付き関数を呼べるようにした。**全 27 門番が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）。

| 判定 | 実測 |
|---|---|
| 28-a | 別効果の handler の入れ子で `abort` が正しい handler に届く。`7`（旧 `12`） |
| 28-b | 腕の中の `with` の `return` が当たる。`5`（旧 `3`） |
| 28-c | `abort` の腕から効果付き関数を呼べる。ビルド可・`101`（旧 ビルド失敗） |
| 28-d/e | R24 の 14 セル、R25 の 4 形、R26 の 11 行が緑のまま |
| 28-f | 全門番 exit 0、`cargo test` 0 failed |
| 28-g | `verify-r28.sh` `PASSED: 2/2` |
| 28-h | `docs/bug.md` から B4 を削除 |

**審査は「起きにくくした」を「直した」と取り違えた修正を捕まえた。** 初版は tag に効果の同一性を持たせたが、識別子が FNV-1a の 32bit ハッシュだった。レビュアが衝突する名前の組を**探索して** `E4418` と `E272902` がどちらも tag `810750057` になることを見つけ、B4 の症状がそのまま再現する入力を作った。実測 `12`、期待 `7`。

修正は**単相化の時点で実際に現れる効果の集合から密な連番を割り当てる**形にした。集合が有限でコンパイル時に確定するので衝突が原理的に起こらない。ハッシュを 64bit に広げるのは確率を下げるだけなので採らなかった。回帰は衝突する名前の組をハードコードするのではなく、割り当ての単射性を確かめる形に置いた。

**正の対照が効いた。** B4 の再現 1（内側の handler が外側の効果を捕まえる）は、内側を全部素通りさせれば `7` になるが、それは入れ子そのものを壊す修正である。発注書に「内側の handler が**同じ効果**を扱う入れ子では内側が捕まえる」を対照として置かせた。実測は `12` で、内側が捕まえた `2` に外側の `return x => x + 10` が当たった形である。

**指揮者が期待値を間違えた。** この対照の期待値を当初 `112` と書いたが、`abort` の腕の結果は handler の結果そのもので、`return` の腕は正常完了時にしか当たらない。判定 26-b でまさに直した意味論を、指揮者自身が繰り返していた。実装が正しく、予想が誤っていた。

**条件付きの 2 件目は指示どおり中断させた。** `capture` を挟む 3 段の入れ子が `continuation result type mismatch` でビルドできない。発注書に「継続の表現そのものを変えないと直らないと判断したら直さずに中断せよ」と書いておいた。builder の分析は「answer 型を対応する handler に結び付けても、入れ子では Outcome の success がその handler の答えではなく内側の計算の成功型のまま残る。型を揃える修正と、残効果のプロトコルは別の層」で、17.6 の未確認 2（`under` フレームの復元）に届いていた。`docs/bug.md` の B5 として記録した。

**B4 と B5 の違いを記録した。** B4 の 1 件目は合法なプログラムに**誤った値**を返したが、B5 は**ビルドが失敗する**。誤った答えを黙って返すことはない。この差が `v0.2` を tag してよいかの分かれ目である。

**中断の質は「何が足りなかったか」を次代が読める形で言えるかで決まる。** builder は forwarding と Outcome の形まで降りて説明し、範囲を超える理由を具体的に示した。R26 の打ち切りと同じく、止めることを正常な終了経路として記録する。

**`v0.2` を tag した（判定 27-c / 28-i）。** 最終確認は全 27 門番が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed、`git status --short` が空、`sha256sum --strict -c tests/corpus.sha256` が整合。

**次の一手:** `docs/bug.md` の B1（印字の失敗を握り潰す）と B5（`capture` を挟む 3 段の入れ子）。B5 は 17.6 の未確認 2（`under` フレームの復元）に属し、継続の表現に踏み込む。

## バグ修正方針（2026-09-10）

`docs/bug.md` の B1 と B5 の扱いを grilling（10 問）で確定した。

**B1 は R29 として直す（plan の 21 節）。** 失敗時は終了コード 1（raise と同じ二値を保つ）、stderr への理由は best-effort、SIGPIPE は runtime で無視して EPIPE を通常の書き込み失敗に統一する。検証は EBADF・ENOSPC・EPIPE の 3 経路を門番化し、bug.md が未確認と残した 2 経路も実測に変える。完了で `v0.2.1` を tag する。実装はマルチエージェント発注。

**B5 は合法として直す。17.6 の未確認 2 の門を開けた。** 17.5 の「継続を別の handler の下へ運ぶ使い方」は継続を第一級の値として持ち出す使い方を指し、B5 の例は `resume` を腕の中で 1 回呼ぶだけなので当たらない、と解釈を確定した。拒否で逃げると capture handler を他の handler と合成する形が全般に不能になり、handler 合成という言語の核が欠ける。under フレームの保留は「不要と結論」ではなく「必要になれば扱う」の先送りだった（plan の 19 節・20 節のスコープ外）ので、B5 で必要になったと判断した。

**B5 の進め方はプロトタイプ先行。** mono.rs と切り離した最小モデル（フレーム列と Outcome の小さな解釈器）で ICFP'21（17.7）の prompt/under の意味論を再現し、B5 の 3 段入れ子が畳めることを確認してから移植する。実測は plan の節（17.8 と同型）として直接作業で行い、結果を見て R30 の手順書と版を切る。実測が失敗したときの撤退先（診断付き拒否に切り替えるか）はその時点で再度相談する。

**次の一手:** R29 の発注書を書いて builder に出す。B5 のプロトタイプ実測がそれに続く。

## B5 の最小モデル — prompt / under フレームの復元（2026-09-10）

17.6 の未確認 2 を実測した。B5 の修正は継続の表現に踏み込むので、mono.rs を触る前に置いた。
結果は `docs/plan.md` 17.9。

**環境。** python3 のみ。外部依存なし。計算を生成器で表し、中断点をそのまま継続として使う。
生成器は 1 回しか再開できないので、§58 の「`resume` は捕まえた継続を 1 回だけ再開する」と
構造が一致する。フレーム列は「prompt を境にした生成器の列」で表した。

**測ったもの。** 争点は capture のときにフレーム列をどこで切り、何を復元するかである。
畳み方を 2 つ用意して同じ 10 形に通した。

```python
if mode == "under":
    # prompt 自身を含めて切る。内側の prompt も継続に入り、resume で再設置される
    segment = stack[index:]
else:
    # 現行 hado。継続は最内 handler で閉じ、prompt を含めない
    segment = stack[-1:]
below = stack[:index]

def resume(x, _captured=list(segment), _h=h):
    value, ty = run(list(_captured), x, trace, mode)
    if ty != _h.answer_ty:
        raise TypeMismatch("continuation result type mismatch")
    return value

answer = arm(payload, resume)
stack = below
pending = (answer, deliver(h.answer_ty, stack, mode, trace, h.name))
```

`deliver` は現行 hado の forwarding を写したもので、その handler より下に handler が残っていれば
答えを外側の Outcome の `$Done` に包む（`interpret_outcome` の `forwarding = handler_stack.len() > 1`）。
success 欄は yield の払い出し型のままなので、下の handler の本体の型と食い違う。

**診断。** 数値と表は 17.9 に置いた。`under` は 10 形のうち 9 形で正しい値と型を出し、
残る 1 形（`resume` の中から外側の handler の `capture` を起こす）だけを畳めなかった。
`resume` が答えの型そのものを返す形では、継続の中でさらに中断する経路を表せない。

**モデルの限界を先に書いておく。** モデルは型検査を持たず、forwarding を実行時に決める。
本体は単相化の時点で型を決める。そのため `hado` の側は 12 形のうち 5 形しか症状を再現しない。
**負の側の根拠は本体の実測（17.9 の 1 つ目の表）であって、モデルではない。**
モデルが担ったのは `under` の意味論が畳めることの確認だけで、`$Outcome` の型付けは
確かめていない。移植では型の設計を別に立てる必要がある。

**指揮者がモデルを 2 度直した。** 最初は `run` を非生成器で書いて `reply` の再開点を
表せず、次は forwarding の条件を「capture の下に handler が残っているか」と読み違えて
v5 と v10 を再現できなかった。現物の条件は `handler_stack.len() > 1` で、
**その handler より下に handler が残っているか**である。3 度目に本体と 5 形で一致した。
それ以上モデルを本体に寄せるのはやめ、負の側は本体の実測に任せた。

**範囲が広がった。** `docs/bug.md` の B5 は 3 段の入れ子だけを記録していたが、実測では
2 段で既に壊れ、`capture` handler が内側か外側かで症状が変わる。通るのは単独のときだけである。
`hado check` は 12 形すべてを通すので、仕様も型システムもこの形を認めており、
追いついていないのは単相化の畳み方だけだと分かった。

## R29 印字の失敗（2026-09-10）

`docs/bug.md` の B1 を直した。§51 に印字の失敗の規範を足し、runtime の 4 つの出力経路の
書き込み失敗を終了コードへ反映し、SIGPIPE を無視して EPIPE を通常の書き込み失敗に統一した。
**全 28 門番が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）。
計画側（§51 の規範文と発注書）と実装側（runtime と判定器）で審査を分けた。planner が仕様も
書く体制でも、成果物の種類で役割を切れば独立性が保たれる。

| 判定 | 実測 |
|---|---|
| 29-a | fd を閉じた実行（`>&-`）が終了コード 1（旧 0） |
| 29-b | `/dev/full` が終了コード 1（ENOSPC）。bug.md が未確認としていた経路 |
| 29-c | 読み側を閉じたパイプが終了コード 1。141 でないことも別に判定（旧 141） |
| 29-d | stdout と stderr の両方を閉じても終了コード 1 |
| 29-e | 正常な実行は生成した値を印字して終了コード 0 |
| 29-f | `verify-r29.sh` `PASSED: 24/24` |
| 29-g | `cargo test --offline --no-fail-fast` 0 failed |
| 29-h | `docs/bug.md` から B1 を削除 |

**計画の審査が、判定器を暗記で通せることを実測で示した。** 初版の発注書は印字する値を固定に
していた。plan-reviewer は「その値のときだけ終了コードを 1 にする」偽実装を実際に書き、
現行 runtime + 新判定器 = 0/5、偽実装 + 同じ判定器 = 5/5、偽実装で全門番 28 本が満点、
通常の `hello` は 4 種の出力失敗すべてで終了コード 0（B1 が残っている）まで測って見せた。
**「暗記で通せるか考えよ」ではなく「通せるなら実際に書いて全門番で示せ」と契約したことが効いた。**
指摘が主張ではなく事実になる。

対処は 2 つを揃えた。**入力を実行時に生成する**（印字する整数・文字列・文字を乱数で作り、
値がソースにも `.hd` にも現れない）と、**期待値の oracle を判定器側に置く**（正常実行の
出力を判定器が突き合わせる）。あわせて出力経路を Int・String・Char の 3 つに増やした。
`hado_print_int` だけ直す実装を通さないためで、`hado_print_float` は `mono.rs:2675` が
Float を `CanonicalFloat` 経由で `hado_print_string` に落とすため生成コードから呼ばれない。

**実装の審査は判定器の穴を 2 段階で深掘りした。** 1 周目は `fstat(1)` で「通常ファイルなら成功、
それ以外なら失敗」とするだけの偽実装で 18/18 を通した。29-e の正常対照が通常ファイルだけ
だったためである。読み手がいるパイプと `/dev/null` の正常対照を足して 24 判定にすると、
同じ偽実装は 18/24・終了コード 1 で落ちた（足した 6 本だけが落ちる）。2 周目は `fstat` に
`poll` を足して出力先を識別する偽実装で 24/24 を通した。

**2 周目は上書き承認して先へ進めた。** 理由は 3 つ。reviewer 自身が「同種の深掘り」と
分類している。2 周を通じて**実装の欠陥は 1 つも出ていない**（読み取り専用 stdout でも
`hado: print failed: Bad file descriptor`・終了コード 1 を実測して正しいと確認している）。
そして残る指摘は暗記ではなく**別の oracle で答えを予測する実装**で、これは出力の判定では
原理的に排除できず、差分審査が担う領分である（R30 の plan-reviewer が同じ原理を
「この限界を不合格理由とはしない」と確定させている）。差分審査は 2 周とも実施済みで、
そのような分岐は入っていない。

**契約の側に欠陥があった。** 「B1 を直さずに通せる実装が書けるか試せ」には終端が無い。
fstat → poll → fcntl と、いくらでも深掘りできる。次回は「判定器の固定入力に依存する実装」
（判定器で塞ぐ）と「別の oracle を持つ実装」（差分審査で塞ぐ）を契約で分ける。

**stderr の接頭辞は §51 が要求していた。** 初版は `strerror(errno)` を裸で出しており、
実測すると `Bad file descriptor` とだけ出た。§51 は「終了コード 1 は `raise` が届いた場合と
印字の失敗の両方に当たり、区別は stderr の内容がつける」と定めているので、裸の errno 文字列では
`raise` の `message` と区別できない。`hado: print failed: ` を付けた。**好みではなく、
§51 が stderr に負わせた役割を果たすための修正である。**

**判定器が固定できていない点を 1 つ記録する。** `hado_print_int` や `hado_print_string` の
個別の検査を外しても 24/24 のままである。`main` の最後の `check_stdout(1)` が同じ失敗を
拾うためで、v0.2 には印字が 1 回しか無いので外から区別できない。個別の検査は将来の
print 効果のためにあり、そのとき初めて判定できるようになる。

**指揮者が門番の全走を builder にさせた。** `verify-m3.sh` は他の門番と `cargo test` を
中から `--test-threads=1` で再実行するので、単独でも十数分かかる。門番の全走は指揮者が
自分の環境で確定すると決めておけば、実装の 1 周は大幅に短くなる。審査者への契約には
これを書いてあり、実装審査の codex は 2 周とも 2 分前後で終わった（セッション記録で
2.3 分と 1.8 分）。結託を全門番で示させた計画審査は約 31 分かかっている。

**共有ビルド先が偽の赤を出した。** `verify-m2.sh` が 6/8・終了コード 1 になり、単独で
測り直すと 8/8・終了コード 0。`CARGO_TARGET_DIR` が `~/.cargo/target` を指すので、
複数のエージェントが同時に cargo を回すとロック待ちとリンク失敗（`undefined reference to main`）
が出る。**赤は単独で測り直してから報告する。**

**`v0.2.1` を tag した（判定 29-i）。** 最終確認は全 28 門番が満点・終了コード 0、
`cargo test --offline --no-fail-fast` が 0 failed、`git status --short` が空、
`sha256sum --strict -c tests/corpus.sha256` が整合。

**次の一手:** R30（`docs/bug.md` の B5）。17.9 の実測と 22 節の手順書がある。

## R30 `capture` handler の合成（2026-09-10）

`docs/bug.md` の B5 を直した。`$Yield` に効果の同一性を持たせ、handler ごとに `convert` 関数を
生成して forwarding が継続を包み直すようにし、継続の答え型をその操作を扱う handler の答えにした。
**全 29 門番が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**

指揮は claude（opus-5）、実装は cursor（cursor-grok-4.6-high）、審査は codex（gpt-6-astra low）。

| 判定 | 実測 |
|---|---|
| 30-a(1) | 再開した継続の中で内側の `reply` を呼ぶ。`[1, 2]` |
| 30-a(2) | 再開した継続の中で内側の `capture` を呼ぶ。`[1, 102]` |
| 30-b | 内側の `return` の変換が最終結果に残る。`[1, 42]` |
| 30-c/d | `Collect` ⊃ `Inner(B)` が `[2]`、`Collect` ⊃ `Inner(A)` がビルド可 |
| 30-e/f/g | `Outer` ⊃ `Collect` が `[1]`、capture 同士が `[3]`、`Collect` ⊃ `Trap` が `[7]` |
| 30-h | B5 の 3 段が `[3]`（旧 `continuation result type mismatch`） |
| 30-i | 同じ効果の入れ子で内側が捕まえる。`[103]` |
| 30-j | 単独 handler が `[3]` と `[1, 2]`（負の対照） |
| 30-k | 外側 `capture` を `resume` から起こす形を単相化で拒否。2 段と 3 段の両方 |
| 30-l | `verify-r30.sh` `PASSED: 16/16` |
| 30-m | 全門番 exit 0、`cargo test` 0 failed |
| 30-n | `docs/bug.md` から B5 を削除し、拒否した形を B6 として記録 |

**プロトタイプの予測が本体の実測と全形で一致した。** 17.9 の最小モデルの `under` が出した
期待値（`[1]` `[2]` `[3]` `[1, 2]` `[2, 5]` `[1, 42]` `[7]` `[103]`）は、実装後の本体の実測と
1 つも食い違わなかった。**計画に書いた期待値が仮説のまま実装へ渡り、そのまま当たった。**

**症状で工程を切ったが、根は 1 つだった。** `done payload mismatch` と
`continuation result type mismatch` は「継続が prompt を失う」という同じ根の 2 つの現れ方で、
工程 1（`$Yield#e` のタグ付けと `convert` による包み直し）を入れた時点で工程 2 の側も
副作用で通った。工程 2 の実体は判定器・受理コーパス・拒否になった。

**工程 1 が、スコープ外の形を「ビルド失敗」から「受理してクラッシュ」に変えた。**
`resume` した継続の中からその handler より外側の handler が扱う `capture` を起こす形が、
`hado check` も `hado build` も通ったうえで実行時に SIGILL で落ちるようになった
（終了コード 132、印字なし）。工程 1 の前は `done payload mismatch` でビルドが失敗していた。
**受理してクラッシュするのは後退である。** `v0.2` を tag できた根拠は
「B5 は誤った答えを黙って返さない」だったので、判定 30-k を新設して単相化で拒否させた。
診断は `resume cannot yield a capture handled outside this handler`。
新しいエラーコードは作っていないので §50 の表は変わらない。

**スコープ外の定義が実測で 2 度直った。** 1 度目は計画審査が捕まえた。「`H` と同じかそれより
外側へ yield したら `A_H` は存在しない」と書いたが、`H` 自身への再 yield は閉じる
（判定 30-a の 2 回目の `y`、単独 handler の yield 2 回が反例）。2 度目は実装後の実測で、
外側の **`reply`** は正しく `[9, 1]` を返した。`reply` は tail-resumptive で単相化の時点で
腕がその場に展開され `$Yield` を作らないためである。閉じないのは外側の `capture` だけだった。

**審査が「判定が欠陥を検出できない」を捕まえた。** 判定 30-a の内側の操作を `reply` にして
いたため、継続の包み直しを外しても通ってしまった。同じ理由で、`reply` は単相化で展開されて
`$Yield` を作らない。**包み直しを固定できるのは `$Yield` を実際に作る操作、つまり内側の
`capture` だけである。** 判定 30-a を 2 つに分け、(2) を内側の `capture` にした。
注入すると (2) だけが落ちる（実行時 Illegal instruction、`PASSED: 14/16`）ことを実測した。

**指揮者は「内側の操作を呼ぶ」とだけ書いて、どの種別なら固定できるかを考えていなかった。**
R28 の「起きにくくした」を「直した」と取り違えた修正を審査が捕まえた件と同じ構図で、
今回は判定の側に同じ穴があった。

**最小モデルを 3 度書き直した。** 非生成器で `reply` の再開点を表せず、forwarding の条件を
「capture の下に handler が残っているか」と読み違え（現物は `handler_stack.len() > 1`、
すなわち**その handler より下に handler が残っているか**）、最後は本体に寄せるのをやめて
負の側を本体の実測に任せた。**モデルが担ったのは `under` の意味論が畳めることの確認だけで、
`$Outcome` の型付けは確かめていない。**

**R28 の門番が、ラウンド固有の約束を焼き込んでいたために落ちた。** `verify-r28.sh` は
`grep -c "self.completion(" mono.rs` が **2 であること**を検査していた。R30 の後は 1 になる。
減ったのは `make_continuation` の中の `completion()` で、**それを消すこと自体が R30 の
修正の core である**（継続を最内 handler で閉じないようにする）。振る舞いの保証
（`r28_effects` の 6 件 = 判定 28-a〜28-c とタグ衝突の回帰）は全て通っていた。

**永続する門番が守るのはマイルストーンの完了条件だけで、そのラウンドの実装の形ではない。**
呼び出し箇所の本数は R28 の実装の形であって、R28 が約束した振る舞いではなかった。
指揮者が裁定して検査を外し、理由をスクリプトにコメントで残した。**門番を緩めて green に
したのではない** — 28-a〜28-c の保証は `r28_effects` が独立に持っており、外したのは
保証を測っていない検査である。

**共有ビルド先が門番に偽の赤を出した（R29 に続き 2 度目）。** `verify-m3.sh` が 10/12 に
なったが、fixer が同時にビルドしていた。fixer の居ない再走では満点だった。同じ回の
`verify-r28.sh` の 1/2 は単独でも落ちる本物の赤で、上の R28 の門番の件である。
**赤は単独で測り直してから報告する。**

**`v0.2.2` を tag した（判定 30-o）。** 最終確認は全 29 門番が満点・終了コード 0、
`cargo test --offline --no-fail-fast` が 0 failed、`git status --short` が空、
`sha256sum --strict -c tests/corpus.sha256` が整合。

**次の一手:** `docs/bug.md` の B6（`resume` の中から外側の `capture`）。直すには継続が
Outcome を返す形への一般化が要り、腕の本体が Outcome を扱う形を要求する。

## B6 の対応方針（2026-09-11）

`docs/bug.md` の B6 の扱いを grilling（8 問）で確定した。決定は `docs/plan.md` 23 節。
途中で B7〜B9 が見つかり、同じ節に入れた。

**B6 だけでなく共通の仕組みを直す。** 事実の収集で 2 件が新たに出た。`with Collect -> [y 1, y 2]`
が `hado check` も `hado build` も通って `[]` を印字する（B7。§58 の意味論では `[1, 2]`）。
`capture ask(seed: Int): Int` がビルドできない（B8）。B7 は「受理して誤答を黙って返す」形で、
`v0.2` を tag した根拠を破っており、どの門番にも見えていなかった。B6 と B7 の根は同じで、
yield 地点の後ろを継続に切り出す仕組みが `do <capture> -> rest` の形にしか無い。「B6 だけ」の案は、
受理コーパスの `capture_resume_inner_capture` が `resume` を展開位置に置いているため、
`let rest = resume () ->` の形だけ直すと展開位置に旧経路と拒否を残す部分修正になり、実際には
小さくならなかった。

**期待値は仕様から導き、Koka で独立に確かめた。** §58 と 17.5 で意味論は深い handler に一つに
決まる。判定 30-f の導出と同じ流儀で 9 形を導き、17.8 の `koka-probe` コンテナ（Koka 3.2.3）で
走らせたところ、9 形すべて一致した（表は plan の 23 節）。先に抽象的に問うより、コードを並べて
「Koka でも同じか」を測る方が 1 往復で決まった（0.2 の構文の grilling と同じ傾向）。

**機構は mono の中に収める。** A 正規化（yield しうる呼び出しを `let` に持ち上げる）と、
`resume` の結果の bind（`$Done` なら腕の残りへ、外側の `$Yield#e` なら継続を付け直して再 yield）。
継続は閉包のままで、§22 の中断条件（ヒープのフレーム列が要るなら止める）をそのまま持ち越す。

**payload の型が異なる handler の合成もビルドできない（B9）。** 機構の境界を決めた後、
`$Outcome` が payload の型を 1 つしか持たないことから測った。`String` と `Int` の capture 効果を
重ねると、`resume` を経なくても `continuation result type mismatch`。R30 の合成は payload が
すべて `Int` だったので通っていた。根は継続と型の 2 本と分かり、同じラウンドで 3 工程に切った。
工程 0 は B7 を初日に拒否へ変える暫定措置で、30-k と同じ扱い。

**30-k の門番は値の一致に置き換える。** 直せば `verify-r30.sh` の 30-k の 2 行が設計上赤くなる。
30-k が約束したのは「受理してクラッシュしない、誤答を黙って返さない」で、導出した値の印字は
それより強い。同じスクリプトに残し、理由をコメントに書く。31-0 も工程 1 で同じ運命なので、
手順書に先に書いた。

**版は `v0.2.3`。** 仕様は 1 文字も変わらず、9 形すべてを `hado check` が既に通す。

**Koka のソース。**

`common.kk`

```koka
module common

pub effect ey
  ctl y(value: int): ()

pub effect ez
  ctl z(value: int): ()

pub fun show-ints(xs: list<int>): string
  "[" ++ xs.map(fn(i) i.show).join(", ") ++ "]"

// Collect: return _ => []
pub fun collect(action: () -> <ey|e> a): e list<int>
  with handler
    return(_) Nil
    ctl y(value)
      val rest = resume(())
      Cons(value, rest)
  action()

// Outer: return _ => []
pub fun outer-drop(action: () -> <ez|e> a): e list<int>
  with handler
    return(_) Nil
    ctl z(value)
      val rest = resume(())
      Cons(value, rest)
  action()

// Outer: return x => x
pub fun outer-keep(action: () -> <ez|e> list<int>): e list<int>
  with handler
    return(x) x
    ctl z(value)
      val rest = resume(())
      Cons(value, rest)
  action()
```

`form1.kk`

```koka
import common
fun main()
  val r = outer-drop { collect { y(1); z(2) } }
  println(r.show-ints)
```

`form2.kk`

```koka
import common
fun main()
  val r = outer-keep { collect { y(1); z(2) } }
  println(r.show-ints)
```

`form3.kk`

```koka
import common
fun main()
  val r = outer-keep { collect { y(1); z(2); y(3) } }
  println(r.show-ints)
```

`form4.kk`

```koka
import common
fun main()
  val r = collect { [y(1), y(2)] }
  println(r.show-ints)
```

`form5.kk`

```koka
import common
effect ask
  ctl ask(seed: int): int

fun twice(action: () -> <ask|e> list<int>): e list<int>
  with handler
    return(x) x
    ctl ask(seed) resume(seed * 2)
  action()

fun main()
  val r = twice { val v = ask(21); [v] }
  println(r.show-ints)
```

`k3.kk`

```koka
import common
effect eb
  fun b(): int
fun inner(action: () -> <eb|e> a): e a
  with fun b() 2
  action()
fun main()
  val r = outer-drop { collect { inner { y(b()); z(5) } } }
  println(r.show-ints)
```

`spread.kk`

```koka
import common
effect ew
  ctl w(value: int): ()
fun inner(action: () -> <ew|e> a): e list<int>
  with handler
    return(_) Nil
    ctl w(value) Cons(value + 100, resume(()))
  action()
fun main()
  val r = outer-keep { inner { w(2); z(3) } }
  println(r.show-ints)
```

`mixpay.kk`

```koka
import common
effect es
  ctl s(value: string): ()
fun names(action: () -> <es|e> list<string>): e list<string>
  with handler
    return(x) x
    ctl s(value)
      val rest = resume(())
      Cons(value, rest)
  action()
fun collect-str(action: () -> <ey|e> a): e list<string>
  with handler
    return(_) Nil
    ctl y(value)
      val rest = resume(())
      Cons("n" ++ value.show, rest)
  action()
fun main()
  val r = names { collect-str { s("a"); y(1) } }
  println("[" ++ r.map(fn(x) "\"" ++ x ++ "\"").join(", ") ++ "]")
```

`mixres.kk`

```koka
effect ezs
  ctl zs(value: string): ()
effect ey
  ctl y(value: int): ()
fun outer-s(action: () -> <ezs|e> list<string>): e list<string>
  with handler
    return(x) x
    ctl zs(value)
      val rest = resume(())
      Cons(value, rest)
  action()
fun collect-s(action: () -> <ey|e> a): e list<string>
  with handler
    return(_) Nil
    ctl y(value)
      val rest = resume(())
      Cons(value.show, rest)
  action()
fun main()
  val r = outer-s { collect-s { y(1); zs("b") } }
  println("[" ++ r.map(fn(x) "\"" ++ x ++ "\"").join(", ") ++ "]")
```

実行は `docker run --rm -v <dir>:/work -w /work koka-probe:latest koka -e --buildtag=<name> <name>.kk`。

**次の一手:** R31 の発注。工程 0 を先に出し、工程 1 の緑を実測してから工程 2 を出す。

## R31 yield 地点の継続と `$Outcome` の型（2026-09-11）

`docs/bug.md` の B6〜B9 を直した。yield 地点の後ろを常に継続にし（構文木の A 正規化と Outcome の bind）、
`resume` の結果を bind して外側の handler への yield を通し、`$Outcome` の payload 欄を消して
効果ごとの payload 型と継続の引数型をタグの表に持たせた。**全 30 門番が満点・終了コード 0、
`cargo test --offline --no-fail-fast` が 0 failed。**

指揮・計画・審査は claude（fable-5.1）、実装と修正は codex（gpt-6-astra medium、`codex exec` の非対話、
本リポジトリで直接作業）。審査は指揮者が本リポジトリで直列に行い、欠陥注入は `git add -A` を復元点にした。

| 判定 | 実測 |
|---|---|
| 31-0 | `[y 1, y 2]`・`let` の右辺・引数位置を単相化で拒否。対照 3 本は生成 payload を印字。工程 1a で 31-d に置き換えた |
| 31-a/b/c | B6 の再現例が `[2]`、Outer `x => x` で `[2, 1]`、外へ渡った後の `y 3` で `[2, 1, 3]` |
| 31-d | `[y 1, y 2]` が `[1, 2]`。`let` / 引数 / record / pipe / 被演算子の位置も `Keep`（`return x => x`）で残りの値まで固定。負の対照: 分岐の腕 `[1]`、`&&` の右辺 `[3]`、内側の `with` の本体 `[2]` |
| 31-e/f | k3 が `[5]`、展開位置の `resume` が外へ yield して `[3, 102]` |
| 31-g | 30-k の対照 `[4]` と `[9, 1]` |
| 31-h/i/j | 非 Unit の `ask` が `[42]`、payload 混在が `["a", "n1"]`、混在で外へ yield が `["b", "1"]` |
| 31-k | `verify-r31.sh` `PASSED: 25/25`。31-a・c・d・f・h と被演算子の形は payload を実行時に生成し、期待値を判定器側で計算。`r31_effects.rs` 8 本 |
| 31-l | 全 30 門番 exit 0、`cargo test` 0 failed |
| 31-m | 受理コーパスに 9 形を追加、`tests/corpus.sha256` を更新 |
| 31-n | `docs/bug.md` から B6〜B9 を削除 |
| 31-o | `v0.2.3` を tag |

**期待値は 9 形すべて仮説のまま当たった。** 23 節の表（§58 からの導出、Koka 3.2.3 で確認）と本体の実測が
1 つも食い違わなかった。探針で足した形（非 Unit の `resume` が外へ yield して `[42, 43]`、
`[ask 1 + ask 2, ask 3]` が `[6, 6]`、Float の payload が `[1.5, 2.25]`）も導出どおり。

**builder が指揮者の期待値の誤りを捕まえた。** 工程 1a の 1 回目の発注は、`return _ => []` の `Collect` の下で
`let u = y 1 -> [7]` を `[1, 7]` と書いていた。§58 では `return` が正常完了値を捨てるので `[1]` になる。
builder は契約の「食い違ったら止める」に従って中断し、指揮者が `return x => x` の `Keep` を導入して再発注した。
残りの計算が走ったことを値で固定するには、正常完了値を捨てない handler が要る。

**工程 1 は 1a（A 正規化）と 1b（`resume` の bind）に割り、1a の `bind_N` が 1b でそのまま使えた。**
1a は構文木で子式を `let $anfN = … ->` に持ち上げ、`capture` を含む複合式が Outcome になったときに
`bind_capture_outcome`（`$Done` → 残り、`$Yield#e` → 継続を付け直して再 yield、`$Fail#e` → そのまま）で繋ぐ。
1b は forwarding 中の `resume`（閉包が Outcome を返す）を「Outcome を返す式」と認識するだけで同じ経路に乗り、
差分は mono.rs で 47 行の削除が中心になった。B6 の根は `continuation_calling_convert` の `take_done` が
`$Done` 以外を潰す（SIGILL）ことで、これを止めて `yield_arm` の `finish_handler` を外した。

**工程 2 は 1 回の fixer で通った。** builder は指揮者が提案した「payload 欄を消してタグ → 型の表を持つ」設計で
31-h〜31-j を通したが、表への登録が操作の呼び出し時だけなのに変換の腕が全タグを引くため、呼ばれない操作の
タグで `operation type missing` が出て既存の門番を後退させ、契約どおり未コミットで中断した。分類は
「不合格:内部構造」。fixer は本体と bind 入力が先に単相化されることを根拠に未登録タグの腕を飛ばし、
pipe が左辺と span を共有して戻り型を取り違える件を操作名の型から取る形で直した。`align_outcomes` は触っていない。
23 節の中断の条件（型引数に効果ごとの列が要り型合わせが壊れる）には当たらなかった。`repr.rs` は
`$Yield#` の payload を 1 語（I64）に固定し、実際の型は `Construct` / `Project` が持つ。

**欠陥注入で判定器の判定力を実測した。** 工程 0 の拒否を外す → 3/6。`&&` の右辺も持ち上げる → 短絡の対照だけ NG
（Rust 側の短絡テストは右辺が評価される形しか無く通ったので、1b で右辺が評価されない形を足させた）。
`resume` の検出を外す → 31-b/c/f と生成 c/f が NG（31-a は Outer が答えを捨てるので単独では弱い、23 節の注意どおり）。
bind の yield 腕で残りを付け直さない → 31-d の record/operand と 31-b/c/f が NG。戻り型を Unit に固定 → 31-h が NG。

**codex のサンドボックスで `git add` が `.git/index.lock: Read-only file system` になった**（工程 0 で 2 回に 1 回）。
工程 0 の実装は指揮者が代理コミットし、以後は `--add-dir <repo>/.git` を足して再発しなかった。ログインシェルの
`RUSTC_WRAPPER=sccache` はサンドボックスで動かず、`CARGO_TARGET_DIR` も無いので、契約で前置させた。

**所要（codex のセッション記録の首尾）:** 工程 0 が 6.6 分、工程 1a が 3.9 分（中断）＋ 15.3 分、
工程 1b が 6.0 分、工程 2 が 6.4 分（中断）＋ 6.6 分（fixer）、工程 3 が 4.2 分。
工程の合間（指揮者の審査と次の発注書の作成。前の記録の終わりから次の記録の始まりまで）は 2.0〜4.5 分。

**未確認。** 型引数付き効果（`effect A Yield`）を異なる型で重ねる形。タグが効果名ごとなので同じ `$Yield#k` になり
表も上書きされるが、探針は E701（`main` が多相）で書けなかった。R30 のタグ付けから続く性質で、R31 の判定には無い。
`fn` の本体で `capture` を起こす形は R31 以前から `lambda return type mismatch` で拒否される（誤答は返さない）。
handler の値引数（§58 の `Take(limit: Int)`）は型検査が腕で引数を束縛せず E204 になり到達しない。

**`v0.2.3` を tag した（判定 31-o）。** 最終確認は全 30 門番が満点・終了コード 0、`cargo test --offline --no-fail-fast` が
0 failed、`git status --short` が空、`sha256sum --strict -c tests/corpus.sha256` が整合。

---

## 0.3 の計画（2026-09-12）

**範囲。** `docs/wants.md` の最優先（核がライブラリの下で閉じること）を v0.3 とした。grilling は wants の順で 25 問
（A ライブラリ単位 6、B 型の共有 5、C データの機械的表現 6、D 世界との境界 5、E 版と受け入れ 3）。決定は `docs/plan.md` 24 節にある。

**独立レビュー。** 決定一覧を別文脈の claude-fable-5-1 にレビューさせた（依頼文は `/tmp/opencode/fable-review-v03.md`、
出力は同 `-out.md`）。致命 1・重大 8・軽微 6。致命は「A3 × A5: ライブラリの外の `def` とサブディレクトリが、
利用側の根の下に置いた瞬間に壊れる」で、`library` 宣言と最内根のパス解決（ポータブル化）で解いた。
主な変更は、`shared` の一様化（B1 の非対称をやめ、等式を公開する唯一の印にする）、E2 のソースを R32 で先に書く、
D2 の 2 層化（生操作 + 包み）、Bytes / Array の印字、`tests/library` の新設、操作名、非 UTF-8 の `Error`。
未確認 7 件は 24.9 に置いた。

**文書。** 仕様を改訂（§5 §21 §27 §34〜§36 §41 §45 §47 §49〜§51 §54〜§56 と §59 ライブラリの新設）。
対応表・追跡表・`corpus.md`・門番 2 本（表番号 24.2）・`wants.md` を追随。計画 24 節を決定表・仕様の項目・
やらないこと・細則・ラウンド骨格の形に書き直した。

**門番（改訂直後の実測）。** 30 本中 23 本が満点・終了コード 0。赤 7 本はすべて仕様先行の期待どおり（24.8）。
独立の原因は 4 つ。① `r23_syntax::keywords_cannot_be_names` が仕様のキーワード列を読むため `library` `shared` で赤
（R32 / R33）。② `verify-m1-spec` の見出し検査が表題 v0.3 と §59 を削除と数える（本改訂のコミットで解消）。
③ 24.2 の 7 項目にコーパスが無い（R32〜R37）。④ stdlib に String の 5 操作と Bytes の宣言が無い（R34）。
m0 の赤が m2 の 5・m3 の 9・m7 の 3・r23 の 23-c に連鎖した。

**レビューの効き。** 独立モデルは、決定の整合（A3 × A5）と実装の現物（`dependencies.rs` の根、
`infer/functors.rs` の `abstract_types`、`mono.rs` の `handled_operation`、`runtime_abi.rs` の単相 intrinsic）を
根拠に指摘を出した。特に「E2 が R38 まで書けず API の穴が最後に出る」は手順そのものを変えた（R32 で E2 を起票）。
1 巡で全件に対応が付き、持ち越しの論点は無い。

**文書レビュー（claude-fable-5-1、2026-09-12）。** 改訂した仕様・計画を 5 観点（決定の反映・EBNF からの導出・
§50 の一致・節の対の矛盾・手順書の実行可能性）でレビューし、致命 1・重大 13・軽微 15。致命は「§49 の包み
`def read_file` の本体が同名の操作を指せず E201 になる」で、1 層にして `module`・包み・`Attempt` を消して解いた。
公開面は functor と実装の無い契約も根にする形に改め、他の指摘は §49 の member の型と失敗の `message` の表、
`String List Attempt` の括弧の解消、§47 の効果の空間、§58 の prelude 効果の handler の位置、E206 の item の
付け替え、門番の免除表として反映した。再走で `verify-m1-spec` は 4/5（見出しはコミットで解消）、
`verify-m1-corpus` は 5/7（受け入れコーパスの不足は R32〜R37）。再確認（同モデル）で致命 1 は解消、残り 11・新規 14（致命なし）も
反映した。新規の重大は E206 コーパスの形が計画・追跡表・手順書で食い違っていた件で、`library` のディレクトリの `main` に一本化した。

## R32 ライブラリ単位（2026-09-12）

24.1 の A（A1〜A8）を実装した。`library` と `shared` を予約語にし、`main` の無い `.hd` の `check` をライブラリ検査（公開面を E513 の根にする）にし、`build` と `dump-mono` は entry 向けの E206 のままにした。`library` を宣言したディレクトリはライブラリ根で、中のパスは最内のライブラリ根から解き、利用側の根と兄弟は見えない。`tests/library/` と `tools/gates/`（E2 の起票）を新設した。**全門番のうち許容される赤（m1-corpus の pairs、m7、その連鎖）以外が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**

指揮・計画は claude（fable-5.1）、実装と修正は opencode（`opencode-go/deepseek-v4.1-flash`、`opencode run` の非対話、本リポジトリで直接作業）、計画審査（2 巡）と実装審査（工程ごと + 修正の再審査）は codex（gpt-6-astra medium、`codex exec`、使い捨てコピーで欠陥注入つき）。フォールバック用に cursor（grok 4.6 xhigh、`-p`）の smoke を通しておいたが、使わなかった。

| 判定 | 実測 |
|---|---|
| 32-a | `main` 無しの `check` が受理（stderr 0 バイト）、`--print-type` は stdout も 0 バイト、`build` / `dump-mono` は E206 1:1（`build requires an entry file with main; check treats this directory as a library`）で実行ファイルを作らない |
| 32-b | `library` のディレクトリの `main` は、値定義でも引数付きでも pattern 腕でも、どのファイルにあっても E206 でその `def`。行:列は探針側で乱数の空白と空行から計算 |
| 32-c | 同一ファイルの 2 つ目・別ファイルの 2 つ目が E205（2 つ目の位置）、`.hdi` の `library` が E100、見出しの後の `library` が E100、E205 と E206 が同時に当たるときはソース順 |
| 32-d | 未参照の実装・functor・契約・外の型・効果・handler を置いたライブラリが受理。E514、実装局所 ADT の E513、外の `def` の E513、ライブラリ根でもないファイルの外の `def` の E100 はそのまま。handler の参照は推移的に閉じる |
| 32-e | 宣言の無いディレクトリを `check` すると他ファイルの `main` は普通の `def`（E513） |
| 32-f | 埋め込み（`app/lib/` を `lib::store::Memory.find` で参照、中は `id::IntId` をライブラリ根から解く）が実行時生成の整数で一致。利用側の根の型 → E204、兄弟 → E204、未参照の実装 → E513、埋め込みの `main` → E206、ライブラリ根自身と prelude の衝突 → E205、ライブラリ根と配下の衝突 → E205 |
| 32-g | `def library` `def shared` `type shared` が E100。`keywords_cannot_be_names` が緑 |
| 32-h | `r9_grammar` が `library` を生成し（生成の有無を集計）、往復が通る |
| 32-i | `tests/library/` 2 ケース（`store` は `library` あり、`plain` は無し）を代表ファイルで `check`。代表ファイルの選択は字句（コメント除去）で runner と一致 |
| 32-j | `tools/gates/report/report.hd` 単体 `check` 受理。実物の `main.hd`（3 つ組のリストの fold）が `33/35 passed; red: m7`。判定器は写しに実行時生成の 3 つ組を書いて一致、全緑と空も一致 |
| 32-k | `cargo test --offline --no-fail-fast` 0 failed（全走の `cargo test --offline --no-fail-fast` で `test result: FAILED` が 0 行） |
| 32-l | 全 32 門番（`verify-*.sh` 31 本 + `cargo test`）を直列に全走（12:39〜12:59、20 分）。赤は `verify-m1-corpus.sh` 6/7（3 pairs: 型の共有・Bytes・配列・String 操作・Int.parse・Env と Io の受理が無い）、`verify-m2.sh` 7/8（5 gates: m1-corpus の連鎖）、`verify-m3.sh` 11/12（9 gates: 同）、`verify-m7.sh` 2/3（stdlib の String の 5 操作・Bytes・Array・`Int.parse`）の 4 本で、すべて 25 節の「許容される赤」。全走では m1-corpus の検査 1 が `tests/reject/E206/main.hd` の `// spec: §59` と §50 の表の §5 の不一致で赤だったので、タグを §5 に直して lock を更新し（`7c95f25`）、単独で測り直して 6/7 を確認した |
| 32-m | `tests/reject/E206` を `library` のディレクトリの `main` に差し替え、`tests/accept/library_embed`（`// item: ライブラリ単位`）、`tests/library/` 2 本、lock 186 行（`sha256sum --strict -c` 全件 OK） |
| `verify-r32.sh` | `PASSED: 45/45` |

**builder が手順書の漏れを 2 つ捕まえた。** 工程 0 では `parse_corpus.rs:121-129` が拒否コーパスの `main.hd` を単一ファイルとしてコンパイルしており、E206 の根拠が `lib.hd` にある新形と両立しなかった。builder は契約の「食い違ったら止める」に従って差し替えを未コミットで残して中断し、fixer が拒否の検査をディレクトリ単位（`Project::load`）に変えた。工程 3 では `verify-m3.sh:110` と `:141` の lock の期待に `tests/library` が無いことを報告し、指揮者が直した。

**審査は判定器の穴を毎回突いた。** 実装審査 4 回はすべて 1 巡目が不合格（致命 2 / 4 / 1 / 2）で、fixer 1 回で全て合格した。指摘の半分は「判定器がこの欠陥を検出しない」で、行:列を定数 1 で書いていた、コマンド置換が末尾改行を落とす、必ず赤を含む入力しか無い、代表ファイルの選択が字句と食い違う、の 4 種。残りは仕様との食い違いで、見出しの後の `library` の受理、重複 E205 がソース順を破る、別ファイルの引数付き `main` の受理、handler の参照が推移的でない、引数付き `main` の `build` が位置を失う、埋め込み時だけ prelude との衝突を見逃す。欠陥注入は延べ 38 本で 30 本を検出した（審査の表から集計）。

**検査の種類の判定は 3 箇所に散っていた。** `with_prelude` / `names::resolve` / `infer::check_ast_with_prelude` がそれぞれ `main` の有無を見ており、fixer が `lib.rs` の `has_main_definition` に寄せた。宣言表（`namespaces.rs`）は利用側の根からの絶対パスで索引されるので、ライブラリ根からの修飾パスは検証ではなく絶対パスへの書き換えで解いた。prelude は別表にし、どこからも無修飾で見える。

**A6（prelude の根）は構造を変えなかった。** `.skip(prelude_items)` のままで振る舞いが同じなので、コメントを §49 に合わせただけである。

**副作用。** 利用側の根で `type String` のような組み込み型名を宣言すると E205 になる（従来は素通り。§47 の名前空間の規則に沿う）。`dump-mono` の E206 メッセージは `build` と同文。

**所要（opencode のログの首尾）:** 工程 0 が 10.5 分（中断）+ 5.4 分（fixer）+ 9.7 分（審査後の fixer）、工程 1 が 25.0 分 + 16.6 分、工程 2 が 12.3 分 + 8.1 分、工程 3 が 13.5 分 + 7.8 分。codex の計画審査は 1.2 分と 0.9 分、実装審査は 3.4〜7.1 分。指揮者の合間（審査の読み取りと次の発注書）は 2〜5 分。

**仕様への疑義。** (1) 効果は `dependencies.rs` の宣言に数えられておらず、未使用の効果は E513 にならない。§50 は「効果（第57節）と handler（第58節）にも当てる」とする。R32 以前からの穴で、R32 では触っていない。R37 の D の設計で当たるので、そこで `docs/bug.md` に置くか決める。(2) stdlib のファイルを単体で `check` すると E205（prelude の型が重複）と E202（相対パスで `stdlib` の判定が落ちる）になる。A1 以前は E206 が隠していた潜在不整合で、門番は stdlib の単体 `check` をしていない。§49 は prelude を「全プログラムに連結され」と定めるので、単体検査の経路は仕様に無い。

**24.9 の未確認のうち R32 で確定したもの。** `trusted_start` は行の閾値で、prelude を prelude 無しで検査する経路は `stdlib/` を渡したときだけ存在し、上の (2) のとおり通らない。外の `def` の E100 は単一ファイルの parse では決められず、`Project::load` の `role_error` で決めている（既にそうだった）。`library` を `.hdi` に置くと `role_error` の `interface` の枝で E100。ユーザーの `type Bytes` / `type Array` は今は受理される（R34 / R35 で E205）。不透明な抽象型どうしの `==` は受理して `true`。`types.rs:515-519` の `Abstract` の単一化は名前比較（R33 の前提）。

**次の一手:** R33（型の共有）。手順書は 26 節。

## R33 型の共有（2026-09-12）

24.1 の B（B1〜B5）を実装した。`implement` の中でだけ `shared type T = <本体>` を受理し、契約の抽象型に等式を与える。等式は「解けた型への置き換え」で作り、不透明の機構（`types.rs:515-519` の名前比較による単一化）は触っていない。`shared` の型は単一化・E601・E705・印字のすべてで解けた型として振る舞い、印なしの `type T = ...` は今までどおり不透明のままである。**全門番のうち許容される赤（m1-corpus の pairs、m7、その連鎖）以外が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**

指揮・計画は claude（fable-5.1 / opus-5）、実装と修正は opencode（`opencode-go/deepseek-v4.1-flash`、`opencode run` の非対話、本リポジトリで直接作業）、計画審査（2 巡）と実装審査（工程ごと + 修正の再審査）は codex（gpt-6-astra medium、`codex exec`、使い捨てコピーで欠陥注入つき）。

| 判定 | 実測 |
|---|---|
| 33-0 | `shared` を書かないライブラリの構文が受理（stdout / stderr 0 バイト）。`shared` に意味の無い工程では、印なしの `T` を `main` に残すと E705 になる |
| 33-a | `shared type T = String` の `combine` が String に解けて実行値が一致。functor の `shared type T = M.T` は、引数が `shared` なら解け（`fold` が `String List` を取る）、印なしなら不透明のまま（公開関数で String に戻して実行値を測る）。適用結果の `StringFold.T` も引数の等式で解ける |
| 33-b | 印なしは E305 で `StringMonoid.T` を出す。印なし functor の 2 適用は別の抽象型（`r21_functors.rs:109` の固定を保つ）。印なし functor を `shared` の引数に適用しても `T` は不透明 |
| 33-c | `shared` + ADT 本体は E100（`\|` の位置）、record 本体は E100（`{` の位置）、契約に無い型は E204（型名の位置）、外・契約の中・`shared` の後の `def` は E100 |
| 33-d | `shared` で E705 が緩和されて整数を印字。緩和なしは E705 で `def main` |
| 33-e | `shared` で E601 が緩和されて field が読める。緩和なしは E601 で field の `.` |
| 33-f | `--print-type` が `String` を出す。E305 の診断は `String` と `Int` で `.T` を含まない。functor の適用結果も `String` と印字 |
| 33-g | 契約修飾型 `Id.T` が `shared` で `Int` に解ける。実装が 2 つなら E603。印なしの実装では `Int` を `Id.T` に渡せない |
| 33-h | 型引数付き `shared type A T = A List` の `push` と注釈が通る |
| 33-i | `shared type T = T`、`shared` の相互参照、functor 適用の輪がすべて E602 で止まる（core dump しない）。印なしの `type T = T` も E602 |
| 33-j | `r9_grammar` が `shared` を生成し、`r23_syntax` と往復が緑 |
| 33-k | `tools/gates/report/report.hd` が単体 `check` で受理。実物の `main.hd` が `Report.T` を fold して `render`。実行時生成の 3 つ組・全緑・空の 3 形が一致。`shared` を消した写しは E601 か E705 |
| 33-l | 受理コーパス 4 本（`shared_string_monoid` `shared_functor` `shared_store_int` `shared_user_record`、`// item: 型の共有`）、lock 190 行が `sha256sum --strict -c` で全件 OK |
| 前処理（設計変更） | 未注釈の関数が record の field を読む / record パターンの束縛 / 効果の操作の payload / handler の腕 / 関数型 / 効果行付きの関数型の 6 形が受理され、`shared` を消した対照はすべて型検査で落ちる |
| arity と契約（panic 防止） | 左辺の個数不一致（多い・少ない）、右辺の参照先の arity 不一致、型引数過剰な右辺、functor 本文の仮引数の arity 不一致、functor 適用の結果契約・引数契約の不一致の 7 形が、**終了コード 101 でなく**対照と同じ診断で終了 1。正の対照（二段適用・複数抽象型・二型引数）は受理して実行値が一致 |
| `verify-r33.sh` | `PASSED: 73/73` |
| 単体 | `cargo test --offline --no-fail-fast` が 0 failed（`r33_shared` は 44 passed） |


**設計を工程 1 の途中で変えた。これが R33 の最大の出来事である。** 当初の決定（26 節）は「`reveal_shared` を通す場所は 4 つ。`records` や効果の操作の型など 5 箇所目が見つかったら、中断せず報告して通す」だった。実際には 5 箇所目が出続けた。修正 1 で constructor の適用・record 構築・局所型名の修飾（審査の致命 3）、修正 2 でパターンの payload と field 射影（審査の致命 2）、修正 3 で named constructor の構築とパターン（builder 自身の棚卸し）。ここまでで消費点 12（`instantiate(` の命中: `infer.rs` 9 + `infer/named.rs` 2 + `infer/functors.rs` 1）のうち 5 箇所に展開を撒いた。それでもまとめ審査（3 巡目）が新規 4 件を出した。推論で型が決まる record、record パターン、効果の操作、handler の腕である。4 件とも指揮者が本物のリポジトリで再現した。

**原因は消費点の網羅ではなく順序だった。** `shared_realizations` が埋まるのは `infer.rs:579`（`checker.module()` の中、`inference_groups` のループ内）と `functors.rs:155` だが、`register_type` と `register_effects`（`:176`）はそのループより前に走る。したがって `type Box = { value: Store.T }` の record schema と `capture send(value: Store.T)` の操作型は、`annotation_inner`（`:1672`）が `reveal_shared` を通っても、その時点で等式が空なので `Ty::Abstract` のまま登録される。実装より前のグループで推論される未注釈の定義も、`resolve_projections(false)`（`:632` `:714`）の時点で等式が空である。指揮者の判別実験: 同じ record・同じ field 射影でも、`main` に畳んで `Store` に構文的に依存させた形は `check` が終了 0 で通る。順序が効いていることの実測である。

対応として 26 節の決定を訂正し（`1877907`）、AST から `shared type T = body` を集める前処理を型と効果の登録の前に置く設計変更に切り替えた（`40a7e50` `f451918` `d2f486f`）。消費点の展開は refactor コミットで 6 関数・10 呼び出し分を外し、判定器が緑のままであることを測った。`reveal_shared` として残るのは入口（`:229` `checked.main`、`:663` `:667` `signature_type` の公開面、`:1666` `annotation_inner`）と E705 / E702 の検査（`:1799` `:1821`）の 6 箇所だけである。判定器は 46/54 から 54/54 に増え、探針 10 本（審査 1 巡目の 3 本、2 巡目の 2 本、棚卸しの 1 本、3 巡目の 4 本）がすべて期待どおりになった。前処理の 1 行をコメントアウトする注入で 3 巡目の 4 探針が 4 本とも落ちることも実測している。

**連鎖の固定点は反復せずに済んだ。** `A.T = B.T`、`B.T = String` のような連鎖は、前処理が等式を格納した後に `Ty::reveal` が再帰置換するので、前処理側でループを回さなくても実効的に固定点になる。前処理が保証するのは「輪を作らない」ことだけで、それは `functors.rs` の `introduces_cycle` に任せた。既存の挿入（`infer.rs:582` と functor 適用）は同じ鍵で後から上書きするので、前処理と矛盾しない。

**前処理が本体を解けない等式は挿入しない。** 解ける条件は組み込み型、top-level 型の arity、実装内の局所型（`Impl.Name` に修飾し、局所別名は展開）、別モジュールの抽象型、functor の仮引数である。実装本体の推論は行わないので、中断の条件（本体の推論が要る）には当たらなかった。

**設計変更の後、工程 1 の審査は 2 回とも致命 1 件で、指摘の質が変わった。** 1 回目は前処理の穴（下記）、2 回目は判定器の判定力（効果行付き関数型の正例で `parse` が `if input == "" => raise` の形になっており、乱数の入力が常に非空なので `raise` が実行されていなかった）。修正 6 は判定器だけを直し、`crates/hado/src/` に差分を出していない。**設計変更の前は毎回「別の消費点」が出ていたのに対し、後は 1 件ずつで、しかも実装の欠陥は 1 つだけだった。**

**設計変更の後、審査は前処理の穴を 1 つ見つけた。** `shared_body_type` が `TypeKind` の `Name` と `Apply` だけを取り、関数型（`Fn` は `Apply("Fn", [Group([...]), 戻り])` の形）を落としていた。`shared type T = Fn(String => String)` は等式が入らず、`Store` に構文的に依存しない定義から呼ぶと E305 になった。修正 5 で `annotation_inner` と同じ形の枝を足し、`TypeKind` の全 variant の棚卸しを報告させた。**設計変更前と違い、指摘は 1 件で、消費点ではなく前処理の内部に閉じている。**

**教訓（24.9 へ）:** 不透明型に等式を公開するときは、等式の収集を型と効果の登録より前に置く。等式を消費点に散らす方式は、消費点を数え上げても閉じない。原因が消費点の外（登録の順序）にあるからである。

**builder が暴走の形を 1 つ捕まえた。** 工程 2 で `shared` の適用が循環する形（`shared type T = M.T` の相互参照）で `check` が core dump した。builder は E602 に固定した（`59b938a`）。v0.2.3 には無い形なので `docs/bug.md` 行きではない。

**指揮者の期待値の誤りが 2 件、いずれも同じ罠だった。** 工程 0 の「33-0 は受理」と工程 2 の「33-a(1b) は受理して実行」で、どちらも「印なしの `T` は `main` に残せない」（§5、E705）を見落としていた。前者は `shared` に意味の無い工程だったので E705 が正しく、後者は公開関数 `show` で `String` に戻して測る形に変えた。R34 の手順書ではこの罠を計画審査の観点に明記した。

**仕様への疑義:** 無し。単一化への unfold は要らず、中断の条件には当たらなかった。

**R33 で見つけた R33 以前の穴 3 つ（`docs/bug.md` の候補。R38 で判断する）。** どれも `shared` の有無で振る舞いが変わらず、R33 のスコープ外として触っていない。(1) `type T = (Int, Bool)`（`TypeKind::Group` を単体で書いた形）が、コードを持たない `record definitions are not implemented` で終了 1 になる。(2) 関数を返す handler の `build` が `callee is not a closure` で失敗する。(3) module の member が効果行付きの関数値を返すと `specialization failed: Mismatch` になる。(3) は工程 1 の修正 5 で判定器の探針の形を変える原因になった。

**運用の記録 2 つ。** builder が 4 コミットで `Claude-Session` の ID を写し間違えた（`...WVvAF` を `...WVVAF`）。履歴は書き換えず、契約書に「契約書からコピーして一字一句そのまま貼る」を足して再発を防いだ。もう 1 つ、`raise` を消す注入に対する判定器の検出が、実行値の比較ではなく `build` の失敗になっている（最終審査の指摘。重大・軽微には入っていない。R33 では直さない）。

**所要（opencode / codex のログの首尾）:** 工程 0 が 12.8 分 + 修正 5.0 分、工程 1 が 13.8 分 + 修正 7 回（11.7 / 8.1 / 6.0 / 14.8 / 9.4 / 12.9 / 13.0 分）、工程 2 が 17.5 分 + 修正 12.9 分、工程 3 が 15.6 分。計画審査は 1.7 分と 0.9 分、実装審査は 4.4〜9.3 分（工程 1 の完成形の審査は 5 回）。**工程 1 だけで修正 7 回・審査 6 回を要し、R33 の時間の 7 割を占めた。**

**審査モデルが 1 回止まった。** 工程 1 の最終審査で codex が `This content was flagged for possible cybersecurity risk` を返して 42 秒で終了し、判定行を出さなかった（判定は途中まで進んでおり、`verify-r33.sh 73/73` と 33-k の確認は出力済みだった）。フォールバックとして cursor（grok 4.6 xhigh、`-p`）に同じ依頼書で審査させ、合格を得た。cursor は完了時に一括出力するので、ログの静止を停止の根拠にしない。

**最終審査（cursor）の軽微 3 点。** いずれも実装の欠陥ではなく、記述と診断コードの粒度である。(1) 表 A の `Ty::Adt` の 2 点は照合を外しても panic しない（assert は `Ty::Abstract` 専用なので、Adt の照合は panic 防止ではない）。(2) 表 B の書き込み `:147` の arity は、表が言う `shared_body_type` ではなく左辺の照合が実効である。(3) 右辺の型引数が過剰な形は E602 ではなくコード無しの `type annotation is not implemented`（`annotation_inner`）に落ちる。`shared` を外した対照も同じ診断なので、判定器は対照との本文一致で測っている。R33 では直さない。

**101 の発火点は前処理の外だった。** cursor が stack を追い、`types.rs:697`（assert）← `types.rs:721`（`reveal`）← `infer.rs:671`（`reveal_shared`）← `infer.rs:1666`（`annotation_inner`）← `register_type` である。等式を書いたのは前処理だが、落ちるのは登録時の展開である。前処理の検査は「不正な等式を入れない」ことでしか panic を防げない。

**24.9 の未確認のうち R33 で確定したもの。** `types.rs:515-519` の単一化（名前比較）は触らずに済み、中断の条件（単一化への unfold）には当たらなかった。`shared` の適用が循環する形は v0.2.3 に無い形で、E602 に固定した（`docs/bug.md` 行きではない）。前処理の等式は `Ty::reveal` の再帰置換で実効的に固定点になるので、収集側で反復する必要は無い。functor の契約の arity は `abstract_types` からは取れず、署名（`ModuleSig`）から取る。

**24.9 へ足す教訓。** (1) 不透明型に等式を公開するときは、等式の収集を型と効果の登録より前に置く。等式を消費点に散らす方式は、消費点を数え上げても閉じない。(2) **同じ意味の型解決を 2 本持つと、片方の検査の写し落としが panic になる。** 前処理 `shared_body_type` は `annotation_inner` の別実装で、食い違いが審査 3 巡で 3 種（`Fn` の枝、arity の照合、契約名の照合）出た。次に同種の前処理を書くなら、`annotation_inner` を段階（登録前 / 後）で切り替えられる形にするか、構築点を 1 つの helper に寄せて assert の前で必ず照合する。(3) `TypeRealization::apply` の assert が、この 3 種の写し落としを 8 探針すべてで捕まえた。**panic する門番は、黙って誤った型を通す実装より安全である。**

**副作用。** `tools/gates/report/report.hd` の `Tally` が契約の抽象型 `T` になり、`main.hd` が `Report.T` の field を 1 箇所読む形になった（`shared` が無ければ E601 / E705 で書けない）。出力は `33/35 passed; red: m7` のまま変えていない。

**次の一手:** R34（Bytes と String の 5 操作）。手順書は 27 節で、計画審査 2 巡（1 巡目は重大 3 件で不合格、`fields` の効果行・lock 更新の工程・String と Bytes の変換 intrinsic を直して 2 巡目で合格）を通してある。

## R34 Bytes と String の操作（2026-09-13）

24.1 の C2（Bytes）・C4（String の 5 操作）・C7（印字）・C8（関数集合）と C1（失敗の分類）を実装した。組み込み型 `Bytes` は `HadoString` を String と共有する不変のバイト列で、`String` の索引はスカラ値の単位である。失敗はすべて prelude の包み（hado 側）で `raise` に持ち上げ、Cranelift 直書きの経路は使っていないので handler で捕まえられる。**全門番のうち許容される赤以外が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**

指揮・計画は claude（fable-5.1 / opus-5）。**ラウンドの途中で実装と審査の割当を入れ替えた**（ユーザーの指示、工程 0 の審査 2 巡目から）。前半は実装 opencode（deepseek-v4.1-flash）・審査 codex（gpt-6-astra medium）、後半は実装 codex（gpt-6-astra high）・審査 opencode（deepseek-v4.1-flash）。計画審査（2 巡）は codex。

| 判定 | 実測 |
|---|---|
| 34-a | `String.slice` `get` `split` `starts_with` `contains` を、文字列の分類（空 / ASCII のみ / 多バイトのみ / 混在 / `0x00` を含む。多バイトは 1〜4 バイトの全長と各長さの最大コードポイントを決定的に含む）× 索引の分類（`-1` / `0` / 中間 / `length - 1` / `length` / `length + 1`）で回し、期待値を python3 の同名操作で計算して一致。`split` はセパレータの位置（先頭 / 末尾 / 連続 / 中間 / 無い / 全体一致）× 種類（1 スカラ値の ASCII / 多バイト / 2 スカラ値以上）、`starts_with` は接頭辞の長さ（0 / 1 / 全体 / 全体 + 1 / `s` より長い / `s` 内の先頭でない部分列）、`contains` は部分列の位置 |
| 34-b | 失敗: `slice` の無効な索引の組み合わせすべて、`get` の `-1` と `length` と `length + 1`、`split` の空セパレータが `IndexOutOfRange` / `EmptySeparator`。負の対照は各失敗形のすぐ隣の有効な形。handler で捕まえる形は §58 に従い body と腕の型を揃える。member の型は §49 と照合（引数型・戻り型・効果行） |
| 34-c | `Bytes` の `of_string` → `length` がバイト数で `String.length` と異なる。`of_list` → `to_list` の往復（3 万要素の先頭・中間・末尾と要素数を含む）、`get` の各要素、`to_string` → `of_string` の往復。3 万要素の構築と `to_list` が build と実行を合わせて 2 秒以内（実測 76ms と 83ms）。失敗は `ByteOutOfRange`（範囲外の要素が先頭・中間・末尾のどこにあっても）、`InvalidUtf8`、`IndexOutOfRange` |
| 34-c(4b) | UTF-8 の判定を**分類から生成**。受理は各長さの最短形・最長形・`0x00`・`0x7f` とその連結、拒否は overlong 2/3/4 バイト・surrogate・U+10FFFF 超過・先頭バイト不正・途切れ・継続バイトの位置に非継続バイト。各分類を単独 / 受理列の後ろ / 受理列の前の 3 形で測る。期待値は python3 の `bytes(seq).decode('utf-8')` |
| 34-d | 印字。最上位の `Bytes.of_list [a, b, ...]`、入れ子（リストと record の field）、空の `Bytes.of_list []` |
| 34-e | 比較。`==` `!=` `compare` が memcmp と同じ規則。同内容の別インスタンス、空列どうし、大小両方向、接頭辞の対（長さが違う）、`0x00` を含む共通接頭辞の後の差 |
| 34-f | `type Bytes = \| A` が E205 で 2 回目の宣言の名前。`def f (b: Bytes) => b` の注釈は受理 |
| 34-f2 | `shared type T = Bytes` の等式が型の登録より前に収集される（`Store` に構文的に依存しない未注釈の関数が record の field を読める）。`shared` を消した対照は E305 |
| 34-g | `verify-m7.sh` の inventory が 94/94。残る赤は Array と `Int.parse` だけ |
| 34-h | `verify-m1-corpus.sh` の検査 6 の member 一覧が §49 のブロックから導かれる（直書きが消えた）。`docs/spec.md` の写しに 1 行足すと一覧が増える |
| 34-i | `Report.fields` を入力の分類から測る。受理は名前が ASCII / 多バイト / 空、数字が 1 桁 / 複数桁 / `0`。拒否は区切りが無い / 2 つ以上 / 空行 / 名前だけ / 数字だけで `InvalidLine` |
| 34-j | 受理コーパス 9 本（`// item: Bytes` と `// item: String 操作`）、lock 200 行が `sha256sum --strict -c` で全件 OK |
| prelude の操作 | サブディレクトリと 2 階層ネストとライブラリ根の実装が `raise` を使え、親から参照して実行できる。サブディレクトリで宣言した効果の操作は親から無修飾では見えない（E204） |
| `verify-r34.sh` | `PASSED: 1360/1360`（3 回連続で同数） |
| 全走 | 許容される赤（`verify-m1-corpus.sh` 6/7 の 3 pairs = 配列・`Int.parse`・Env と Io、`verify-m2.sh` 7/8 と `verify-m3.sh` 11/12 はその連鎖、`verify-m7.sh` 2/3 は Array と `Int.parse`）以外が満点。`verify-m4.sh` 123/123、`verify-m5.sh` 145/145、`verify-m6.sh` 71/71、`cargo test --offline --no-fail-fast` 0 failed |


**この版の最大の特徴は、実装に致命が 1 度も出なかったことである。** 工程 0 は審査 9 巡、工程 1 は 5 巡、いずれも致命 0 で、指摘 26 件はすべて判定器の穴だった。判定器 `verify-r34.sh` は 23 本から 1360 本に増えた。**測っていない入力の組み合わせが毎回 1〜4 件見つかる**状態が 8 巡続いたので、方針を変えた。

**個別の探針をやめ、分類からの生成にした（27 節に明記、`8936122`）。** UTF-8 の不正列を例に取ると、受理側は各長さの最短形・最長形・`0x00`・`0x7f` とその連結、拒否側は overlong 2/3/4 バイト・surrogate・U+10FFFF 超過・先頭バイト不正・途切れ・継続バイトの位置に非継続バイトで、それぞれを単独・受理列の後ろ・受理列の前の 3 形で測る。期待値は python3 の `bytes(seq).decode('utf-8')` から導く（実装の写しではない独立した oracle）。これで判定器は 62 から 165 本になり、その回の審査は 17 種類の注入をすべて検出して合格した。

**分類にも穴があった。** 工程 1 では (1) 文字列の分類に 2 バイト文字が 1 つも無く（3 バイトの `あ` と 4 バイトの `😀` だけ）、`hado_string_get` の 2 バイト用マスクを壊す注入が満点で通った。(2) 各長さの**最大コードポイント**が無く、マスクを**1 ビットだけ**狭める注入が通った。1 ビットの差はそのビットが立つコードポイントでしか観測できない。どちらも「境界値は乱数任せにせず決定的に含める」で塞ぎ、27 節に書いた（`eefd9fb` `10e0b83`）。

**R32 由来の実装欠陥を 1 つ直した。** 工程 2 の builder が、`tools/gates/report/report.hd` の `fields` が `raise` を使うと親から参照したときに `unbound name raise`（E204）になることを見つけて中断した（単独の `check` は通る）。指揮者が独立に再現し、`namespaces.rs` が効果の操作をディレクトリごとに登録する一方、prelude をどこからでも見せる仕組み（`prelude` / `has_prelude`）が操作には無いことを特定した。§47・§57 の「prelude の操作はどこからでも書ける」に反する。E2 の `fields` は §49 で `{Error}` なので回避できないため、工程 2 のスコープを広げて `prelude_operations` を足した（`694c9ae`、8 行）。prelude 以外の効果の操作はディレクトリ単位のままである。

**全走で門番の上限に当たった。** `verify-m3.sh` が 11/12 から 9/12 に落ち、チェック 8（暗記排除と HM のテスト）と 12（`extra-def`）が赤になった。判定の失敗ではなく、3 つのチェックが共有する `cargo test --no-fail-fast -- --nocapture --test-threads=1` の実測が 250.59 秒で、`run` の上限 180 秒を超えていた。`TimeoutExpired` が静かに `returncode 127`・stdout 空を返すので、時間切れと「条件を満たさなかった」が区別できなかった。R34 で受理コーパスが 9 本と `crates/hado/tests/r34_bytes.rs` が増えたためである。上限を 600 秒にし、**時間切れのときに `NG: timeout (<n>s): <コマンド>` を出す**ようにした（門番を強くする変更で、判定の条件は 1 文字も変えていない）。


**builder の中断は 2 回とも正しかった。** 工程 1 では `crates/hado/tests/prelude.rs` が String・Int・Float の 3 ファイルしか連結せず、新しい `String` の契約が参照する `Error` が無い（E204）ことを見つけた。工程 2 では上の `raise` の件である。どちらも「触ってよい場所の外の欠陥に当たったら止める」という契約に従った判断で、指揮者がスコープを広げて引き継がせた。

**所要（ログの首尾）:** builder / fixer が 18 本で合計 264 分（最長 48 分）、審査が 16 本で合計 361 分（最長 62 分）、計画審査が 1.5 分と 1.1 分。**審査の時間が実装を上回った版である。**

**24.9 へ足す教訓。** (1) **入力の形が列挙できる判定は、手順書の段階で生成規則として書く。** 個別の値を審査に足させると 1 巡 1〜4 件で 9 巡かかった。(2) **境界値は乱数任せにせず決定的に含める。** 各長さの最大コードポイントを落とすと、マスクを 1 ビット狭める欠陥が偶然回避される。(3) **期待値は独立した oracle から導く。** UTF-8 は python3 の decoder、文字列操作は python3 の同名操作を使い、実装の写しを判定器に書かない。(4) **門番の時間切れは判定の失敗と区別する。** 静かに false になると、テストが増えただけで判定が消える。

**副作用。** `namespaces.rs` に `prelude_operations` が入り、prelude の効果の操作が全階層から無修飾で見えるようになった（8 行）。`crates/hado/tests/prelude.rs` が `stdlib/error.hd` を連結するようになった。`scripts/verify-m3.sh` の `cargo test` の上限が 600 秒になり、時間切れが `NG: timeout` として出るようになった。

**次の一手:** R35（配列）。手順書は未着手。R34 の教訓（分類からの生成、境界値の決定的な包含、独立した oracle）は配列の索引にそのまま効く。

## R35 配列（2026-09-13）

24.1 の C3（配列）と C7（印字）・C8（関数集合）・C1（失敗の分類）を実装した。`Array` は要素型を 1 つ取る不変の列で、`{ int64 count; slot[count] }` の 1 つの GC ブロックに stride 8 バイトで詰める。`get` は emit が出す直接の load なので O(1)、`set` は新しい Array を返す。失敗は prelude の包み（hado 側）で `raise` に持ち上げる。**全門番のうち許容される赤以外が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。**

指揮・計画は claude（opus-5）。実装は codex（gpt-6-astra high）、審査は opencode（deepseek-v4.1-flash）。計画審査（2 巡）も opencode。

| 判定 | 実測 |
|---|---|
| 35-a | 5 member の正常形を、要素型（`Int` / `Float` / `Bool` / `String` / `Char` / record / `Int Array`）× 長さ（`0` / `1` / 乱数 / `3` / `30000`）× 有効な索引（`0` / 中間 / `length - 1`）の積で回し、期待値を python3 のリスト操作で計算して一致。要素の機械型 I64 / F64 / I8 / Pointer / I32 を全部通す |
| 35-b | 失敗: `get` と `set` の `-1` / `length` / `length + 1` が `IndexOutOfRange`。負の対照は隣の有効な形。handler で捕まえる形は §58 に従い body と腕の型を揃える |
| 35-c | 3 万要素の `of_list` → `length` と `get 29999` が build と実行を合わせて 2 秒以内（実測 0.15 秒）。`set` も 3 万要素で 0.16 秒、終了コード 0 |
| 35-d | 印字。最上位の `Array.of_list [...]`、空の `Array.of_list []`、入れ子（List の要素、record の field、Array の Array） |
| 35-e | 比較。`==` `!=` `compare` を要素型 ×（同内容の別インスタンス / 先頭が違う / 途中が違う / 末尾が違う / 接頭辞 / 空どうし / 空と非空）で回し、期待値を python3 のリスト比較から導く。NaN の等値と順序の違いも測る |
| 35-e2 | 要素型が Fn を含む Array の `==` と `compare` が E703 / E704（入れ子も）。Fn を含まない同じ形は受理 |
| 35-f | 利用者の `type A Array = ...` が E205 で 2 回目の宣言の名前。`def f (xs: Int Array) => xs` の注釈は受理 |
| 35-f2 | `shared type T = Int Array` の等式が型の登録より前に収集される。`shared` を消した対照は E305 |
| 35-g | `verify-m7.sh` の `prelude inventory` が 32/39 から 38/39。残る赤は `Int.parse` だけ |
| 35-h | `verify-m1-corpus.sh` の pairs から「配列」が消え、残りは `Int.parse` と Env と Io の 2 項目。受理コーパス 5 本（`// item: 配列`）、lock 206 行が `sha256sum --strict -c` で全件 OK |
| `verify-r35.sh` | `PASSED: 714/714` |
| 全走 | 許容される赤（`verify-m1-corpus.sh` 6/7 の 2 pairs、`verify-m2.sh` 7/8 と `verify-m3.sh` 11/12 はその連鎖、`verify-m7.sh` 2/3 は `Int.parse`）以外が満点。`verify-m4.sh` 128/128、`verify-m5.sh` 145/145、`verify-m6.sh` 71/71、`verify-r34.sh` 1360/1360、`cargo test --offline --no-fail-fast` 0 failed |

**設計は手順書を書く前の実測で決めた。** Explore の地図は「型多相 intrinsic は今の器に入らない」として (a) infer の scheme、(b) mono の `CallRuntime` の結果型の上書き、(c) 要素の ABI の 3 つが要ると報告し、C を 8 バイトスロット固定にする案を挙げていた。指揮者が 5 つ測って設計を変えた。

- 3 万要素の List の O(n) 走査は build 0.13 秒・実行 0.01 秒（hado の record で Array を作る案が性能で不可能ではないことの確認）
- `type A Arr = A Box` の別名は parser が拒否する（`expected a record or a leading | for an ADT`）。**`module Array` の `shared type A T` を §49 の `A Array` と綴らせる道は無い**
- トップに `type A Array = { items: A List; count: Int }` と書けば綴りは合うが、`a.items` が利用者に漏れて不正な `count` を持つ値を組める
- **emit は cranelift である。** 要素の load 型を `repr::machine_type` から決められるので、C の ABI の幅問題は存在しない。これが「C を通す案は 1 桁重い」という見立てを覆した

結果として、Array は `Ty::Adt("Array", [A])` / `mono::Type::Data("Array", [T])` の名前キーの `Data` にした（`mono::OUTCOME` と同じ形。新しい `Ty` / `Type` の variant は作っていない）。intrinsic 4 本（`__array_length` `__array_get` `__array_alloc` `__array_fill_unchecked`）は `value_signature` に載せず、`RuntimeFunction::is_polymorphic()` で 4 箇所を分岐させる。C が持つのは `hado_array_alloc` と `hado_array_length` の 2 本だけで、slot の読み書きは emit が `base + index * 8` の load / store を出す。`of_list` `to_list` `set` は hado で書いた（R34 の `Bytes.of_list` と同じ「構築中の未公開の値への書き込み」の形）。

**計画審査が致命 2 件を捕まえた。** どちらも「今のコードでは通らない」という指摘で、指揮者がコードを開いて確かめてから手順書を直した。

1. **`name_resolution.rs:167` は `__` で始まる名前の `value_signature()` が `None` なら E204 にする。** 多相 intrinsic を `value_signature` に載せない決定のままだと `stdlib/array.hd` の中の 4 本すべてが E204 になり、prelude が壊れて工程 0 の最初の `check` から一切通らない。分岐は 3 箇所ではなく 4 箇所だった
2. **`infer.rs` の `contains_function` は `Ty::Adt` の型引数を再帰しない。** 名前を結果に持つ constructor / record の payload だけを辿るので、`List` は `Cons` の payload で拾えるが、**constructor を持たない Array では `Adt("Array", [Fn])` が常に false になる**。§45 の E703 / E704 が出ず、`build` で `derive_comparison` が Fn に達して内部エラーになる。同じファイルの `contains_abstract` は型引数を再帰しているので、その腕を写した

**実装に致命が 1 度も出なかった。R34 に続いて 2 版連続である。** 3 工程とも審査 1 巡で合格し、fixer は 1 度も要らなかった。R34 が 14 巡（工程 0 が 9 巡、工程 1 が 5 巡）だったのと対照的で、違いは判定器の作り方にある。**R34 の教訓（入力の形が列挙できる判定は手順書の段階で生成規則として書く / 境界値は乱数任せにせず決定的に含める / 期待値は独立した oracle から導く）を 28 節に最初から書いた結果、判定器が 1 巡目から穴を持たなかった。** 審査は工程 0 で 14 件、工程 1 で 13 件、工程 2 で 8 件の欠陥を注入し、**合計 35 件すべてを判定器が検出した**（落ちない判定は 1 つも無かった）。

**手順書の予想が 1 つ外れた。** `infer/functors.rs` の `shared_body_type` に Array を足す決定を書いたが、既存の arity 参照でそのまま通り、変更は不要だった。R34 で Bytes を通したときに写し落として R33 の順序の欠陥が再発した箇所なので、判定 35-f2 は残してある。

**手順書に無い変更が 1 つ出た。** builder が工程 0 で、handler の中の `Array.length (Array.set ...)` が単相化で `Mismatch` になることを見つけ、`mono.rs` に `contains_handled_call` を足して既存の継続処理へ接続した。全 effect 呼び出しに及ぶ広い変更だが、撤去すると判定 35-b が build できないことを審査が注入で確かめ、`cargo test` 0 failed と `verify-r32/r33/r34` 満点で既存の handler 経路の後退は観測されなかった。

**指揮者が門番を 1 つ強くした**（`83edabb`）。`verify-r35.sh` に `FILTER` 環境変数によるラベルの部分一致での絞り込みがあった。未設定なら全件を回すので既定の実行に影響は無いが、値を与えれば判定を静かに減らせる。判定の条件は変えずに消した。

**未確認のまま残したもの:** `%` を使う未注釈の再帰生成器が E306 になる（builder が工程 0 で踏み、Pure な比較式で回避した。原因は未調査）。R38 で判断する。

**仕様への疑義:** §45 は Array の `compare` を「要素型から派生する」としか定めず、**長さが違う Array の順序を定めていない**。28 節で python3 のリスト比較・`List` の辞書順に合わせて「共通部分がすべて等しいときだけ短い方を先」と決めた。仕様は変えていない。R38 で §45 に 1 文足すかどうかを判断する。

**所要（ログの首尾）:** builder が 3 本で 73 分（28 / 22 / 23）、審査が 3 本、計画審査が 8 分と 10 分。全走は 34 分（入口）と 50 分（出口）。R34 の builder 264 分・審査 361 分に対し、**1 巡で通ったぶん全体が大きく縮んだ**。

## R36 Int.parse（2026-09-13）

24.1 の C5（`Int.parse`）と C1（失敗の分類）を実装し、E2（`tools/gates/`）の集計部に数への変換を足した。`Int.parse` は `-?[0-9]+` かつ i64 の範囲だけを受理し、それ以外を `"InvalidInt"` の `Error` にする。**`verify-m7.sh` が満点になり、prelude の関数集合が §49 の全 39 項目を満たした。** 全門番のうち許容される赤以外が満点・終了コード 0、`cargo test --offline --no-fail-fast` が 0 failed。

指揮・計画は claude（opus-5）。実装は codex（gpt-6-astra high）、審査は opencode（deepseek-v4.1-flash）。計画審査は 3 巡。

| 判定 | 実測 |
|---|---|
| 36-a | 受理を分類の積（符号 5 種 × 本体 12 種 × 境界の決定的な値）で回し、oracle と一致。**往復**（乱数の i64 を `Int.to_string` → `Int.parse`）が両端と `0` を含めて一致 |
| 36-b | 拒否。`""` `"-"` `"+42"` `" 42"` `"42 "` `"1_000"` `"4.2"` `"４２"` `"--5"` `"42a"` と、`"9223372036854775808"`（2^63）`"-9223372036854775809"` 2^64 10^19 10^20 が `InvalidInt` で終了コード 1。負の対照は各拒否形のすぐ隣の有効な形 |
| 36-c | handler で捕まえる形（§58 に従い body と腕の型を揃える）で `message` が `InvalidInt` |
| 36-d | `stdlib/int.hd` の 2 member の型が §49 の宣言と一致（`to_string` は `Pure`、`parse` は `{Error}`） |
| 36-e | `Report.parse_line "<名前> PASSED: <n>/<N>"` を名前（ASCII / 多バイト / 空）× 数（1 桁 / 複数桁 / `0` / 先行ゼロ付き）の積で回し、`render` の出力と一致。**`"m0 PASSED: +7/7"` は `InvalidInt`、`"m0 7/7"` は `InvalidLine`** で、2 つの失敗が入れ替わらない |
| 36-f | `verify-m7.sh` の `prelude inventory` が 38/39 から **39/39**。門番そのものも 3/3 で満点 |
| 36-g | 受理コーパス 3 本（`// item: Int.parse`。`// needs: prelude` 付き）、lock 209 行が全件 OK。`verify-m1-corpus.sh` の pairs から `Int.parse` が消えた |
| `verify-r36.sh` | `PASSED: 388/388` |
| 全走 | 許容される赤（`verify-m1-corpus.sh` 6/7 の 1 pair = Env と Io、`verify-m2.sh` 7/8 と `verify-m3.sh` 11/12 はその連鎖）以外が満点。**`verify-m7.sh` が 3/3 で満点に戻った。** `verify-m4.sh` 131/131、`verify-m5.sh` 145/145、`verify-m6.sh` 71/71、`verify-r34.sh` 1360/1360、`verify-r35.sh` 714/714、`cargo test --offline --no-fail-fast` 0 failed |

**受理する文法を仕様が定めていないので、往復から導いた。** §49（`docs/spec.md:1985`）は失敗を「不正（桁溢れを含む）」としか書かない。手順書は 1 つの導出と 2 つの決定に分けた。**導出**は「`Int.parse (Int.to_string n) == n` が全 i64 で成立しなければならない」から来る受理の下限で、`Int.to_string` が `snprintf("%lld")`（`runtime/hado_runtime.c:177-186`）で `-?[0-9]+` の形の i64 全域を出すことを実測して確かめた。**決定 1** は「使う字は `Int.to_string` が出す字に限る」（`+`・空白・`_`・小数点・全角数字を拒否）。**決定 2** は「数字列の形は往復では決まらないので定める」で、先行ゼロを長さに上限なく受理する。仕様は変えず、R38 の判断に回す。

**計画審査が 3 巡かかり、2 巡目で致命を 1 件捕まえた。** 内容は「負の側に累積する形は `"9223372036854775808"` を `-9223372036854775808` として受理する」で、reviewer は手順書の算法をそのまま C に書いて実測して示した。**指揮者は同じ穴を、審査コピーを作った後に `(INT64_MIN + digit) / 10` の式を展開していて独立に見つけ、既に直していた**（`02561d2`）。直し方も同じ（符号を戻す前に `acc == INT64_MIN && 符号が正` を拒否する）だったので、そのままにした。**式を自分で展開することと、審査に出すことは、どちらか一方では足りない。** 1 巡目は「先行ゼロは無制限に受理」と「21 バイトを超えたら範囲外」の矛盾を捕まえた。3 巡目は算法を C に書いて `digit` 0〜9 の全境界と乱数 200 万件の往復を実測して合格にした。

**C は `strtoll` を使わず、一時バッファも作らない。** `HadoString` は NUL 終端でない（`runtime/hado_runtime.h:6-9`）ので `bytes` を `strtoll` に渡せない。先行ゼロを無制限に受理する決定があるため、固定長のバッファに写すには先頭ゼロを外す前処理が要る。それなら直接走査の方が単純である。実装（`runtime/hado_runtime.c:193-214`）は先頭ゼロを読み飛ばしてから桁ごとに負の側へ積み、各段で `acc < (INT64_MIN + digit) / 10` を検査する。C の整数除算は 0 方向に切り捨てるので、負の値では `ceil` になり、この比較がちょうど境界を表す。

**実装に致命が 1 度も出なかった。R34・R35 に続いて 3 版連続である。** 2 工程とも審査 1 巡で合格し、fixer は 1 度も要らなかった。審査の欠陥注入は工程 0 で 10 件、工程 1 で 9 件で、**時間判定に当たる 1 件（R36 には時間の要件が無いので判定器に無い）を除いてすべて検出された**。

**単相の intrinsic は R35 の仕組みを使わずに済んだ。** `__int_parse_is_valid` と `__int_parse_unchecked` は `runtime_abi.rs` の 5 箇所（enum / `intrinsic_name` / `value_signature` / `ALL` / `declaration`）に足すだけで通り、R35 で入れた `is_polymorphic()` の 4 分岐は触っていない。`Int` は既存の型なので `Ty` / `Type` / `repr` / `emit` / `display_value` / `derive_comparison` はすべて無傷である。

**未確認のまま残したもの:** builder が工程 1 の注入の試作で、サブディレクトリの中の `handler Error` が E204 になることに当たった（原因未調査。注入自体は補助関数で代替した）。R35 から持ち越している「`%` を使う未注釈の再帰生成器が E306」と合わせて R38 で判断する。

**仕様への疑義:** §49 は `Int.parse` が何を受理するかを定めていない。本ラウンドは往復から下限を導き、字種の上限と先行ゼロの扱いを決定した。R38 で §49 に 1 文足すかどうかを判断する。R35 の「長さが違う Array の `compare` の順序」と同じ扱いである。

**所要（ログの首尾）:** builder が 2 本で 46 分（20 / 26）、審査が 2 本で 96 分（21 / 75）、計画審査が 3 本で 15 分。全走は 54 分。

## R37 世界との境界（2026-09-14）

24.1 の D 表（D1〜D6）を実装した。prelude が効果 `Env` と `Io` を持ち、操作は `reply`、handler が無ければ prelude の既定の `def` が応える。argv / env / cwd / stdin / ファイル / ディレクトリが hado から読めるようになり、E2（`tools/gates/`）が argv でディレクトリを受けて集計を印字する形で完成した。**全門番が満点になり、v0.3 の入口の赤が消えた。**

指揮・計画は claude（opus-5）。実装は codex（gpt-6-astra high）、審査は opencode（deepseek-v4.1-flash）と、落ちた後の再送が cursor（grok-4.6-xhigh）。**計画審査は 6 巡かかり、致命を 4 件捕まえた。**

| 判定 | 実測 |
|---|---|
| 37-a | 効果の宣言（`reply` の結果の効果行 `{Error}`）が parser を通り、呼び出し位置の行が `{Error, Foo}` になる（印字は整列順） |
| 37-b | 既定の `def` の効果行が操作の宣言より広ければ推論の後に診断。ローカル束縛が prelude の操作名を影にできる（`Int` 型と `Fn` 型の両方で） |
| 37-c | argv / env / cwd の往復。非 UTF-8 は `"InvalidUtf8"` |
| 37-d | (1) handler 無しで既定の `def` が応える。(2) 利用者の `handler Env as MyEnv` が優先する。(3) `handler Error` が非 UTF-8 の `message` を受け取る。(4) 1 操作を落とすと E605。(5) prelude の `Reply` 操作すべてに既定の対応がある（`r37_world.rs` が `Checked` を直接見る） |
| 37-e | 利用者の `effect Env` / `module Env` / 同名の宣言が E205。サブディレクトリでも同じ。**`module EnvDefault` / `IoDefault` も E205**（既定実装の契約名） |
| 37-f | ファイルの読み書き、`read_dir`（整列・`.` と `..` を除く）、`read_stdin`（閉じた stdin は空、2 回目も空） |
| 37-g | I/O の失敗の分類（`ENOENT` → 1、`EACCES`/`EPERM` → 2、`ENOTDIR` → 3、その他 → 4）が `message` に写る |
| 37-h | E2 が実行時に生成したディレクトリを argv で受け、`PASSED: ` に一致する**最後の**行を選んで集計する。空 argv は `MissingDirectory` |
| 37-i | 受理コーパス `env_io_roundtrip`、lock、`verify-m1-corpus.sh` の pairs から `Env と Io` が消えた |
| `verify-r37.sh` | `PASSED: 254/254` |
| 全走 | **赤ゼロ**（04:23:56 → 05:56:51、93 分）。m0 7/7、m1-corpus 7/7、m1-spec 5/5、m2 8/8、**m3 12/12**、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r9〜r37 の 24 本すべて満点（r34 1360/1360、r35 714/714、r36 388/388、r37 254/254）、`cargo test --offline --no-fail-fast` 0 failed |

**既定実装は `handler` ではなく普通の `def` にした。計画審査 3 巡目の致命がそれを決めた。** 当初の設計は「prelude に `handler Env as EnvDefault` を置き、mono が handler の無い `Reply` でそれを被せる」だった。審査は **handler の腕が `with` で被せたときしか推論されない**ことを実測で示した（`infer.rs:1969` の `handler_answer` は `with_handler` からしか呼ばれず、`infer.rs:175` `:186` と `dependencies.rs:17` は `Item::Handler` を無視する）。`handler Foo as H` の `reply pick => "not an int"` は `reply pick: Int` の宣言と食い違っても `check` が終了 0 になる。腕が推論されないと mono は span の型（`composite_types`）を引けず、既定実装に必ず入る argv の分類・`Bytes.to_string`・`raise` で `missing list type` などに落ちる。**E605（`infer.rs:1884-1891`）は腕名しか見ないので型の保証にはならない。**

**同名の影の欠陥は mono と infer の両方にあり、直す層を 2 度間違えた。** 24.9 に「ローカル束縛が prelude 操作名を shadow すると昇格が崩れる」として R38 に回していた穴である。計画審査 3 巡目が `def f (raise: Int): Int => raise` が `check` を通って build 後に落ちることを実測し、4 巡目が**直す場所は `namespaces.rs` ではなく mono である**ことを実測で特定した（`namespaces.rs` の `operation` は bare 名に対して恒等変換なので、束縛の集合を渡しても効かない。実際に操作として扱っているのは `mono.rs:482-485` と `:1424-1427` が `operations` を `scope` より先に見ているところ）。5 巡目が**infer 側にも影がある**ことを示し（`unwrap_operation` が `Fn` 型のローカルを操作として解釈して E305）、6 巡目が「外側の `Fn` の結果が登録済みの効果 ADT であることを確かめる」4 行の直しをコピーで実測して合格にした。**判定 37-b2 に `Fn` 型のローカルを必ず含めることになったのは、`Int` 型のローカルでは `unwrap_operation` が素通しするからである。**

**builder が工程 3 で 2 度「中断」を返し、2 度とも正しかった。** 1 度目は「`verify-r33.sh:997`（33-k(2)）が E2 を引数なしで起動して固定文字列を期待している」「`:1044`（33-k(4)）が集計の join の移動で空虚になっている」「`crates/hado/tests/corpus.rs:68` も受理ケースの実行ファイルを cwd と stdin を固定せずに起動する」の 3 点。**指揮者の「受理ケースを起動するのは `verify-m6.sh` だけ」という前提が誤りだった**（`scripts/` しか grep していなかった）。2 度目は「受理ケース `env_io_roundtrip` が `read_file "main.hd"` を読むが、`corpus.rs` は本文から素の一時ディレクトリに書き出してビルドするのでケースの木が無い」。どちらも指揮者が直した。

**工程 3 の後の全走で `verify-m3.sh` の検査 11（`stdlib-types`）が赤になり、門番ではなく `stdlib` の形を直した。** この検査は `stdlib/*.hd` の最初の `module` / `implement` より前に `def` があると落ちる。既定実装を素の `def` として並べる形がこれに当たった。**利用者から既定の `def` が見えないことは実測している**（`default_args` の呼び出しは E204、利用者の `def default_args` は衝突しない）が、**それは `file_scopes.rs` が top-level の `def` を `file{N}$name` に改名する副作用であって、単一ファイルの経路では走らないので設計の保証ではない**。門番を「効果を宣言するファイルは免除」に変えると、免除の範囲が R37 が書いた 2 ファイルにちょうど合う形になり、AGENTS.md が名指ししている「門番を緩めて green にする」そのものになる。**赤を出した側（指揮者）が門番を書き換えるのは判定者と作成者の分離にも反する。** `stdlib/env.hd` と `stdlib/io.hd` を `module EnvDefault` / `implement EnvDefault as EnvDefault` の形に直し、対応を `op 名 → <効果名>Default.<op 名>` に変えた。module member は `values` ではなく `module_definitions` に入る（実測: `values["String.length"]` は false、`module_definitions["String.length"]` は true）。予約は自動で効き（`module EnvDefault` が既存経路で E205）、`namespaces.rs` は無変更で済んだ。

**審査の割当を 1 度切り替えた。** opencode（deepseek-v4.1-flash）が工程 2 と工程 3 で判定行を出さずに落ちた。工程 2 は作業ディレクトリの外（`/usr/include/gc/gc.h`）の読みを拒否されて停止し、前置きを足して再送したら通った。工程 3 は 100 分走ってから `runtime/hado_runtime.c` の読みの途中で落ち、**注入を戻さないまま止まっていた**（指揮者が `git checkout -- .` で復元）。ユーザー指定のフォールバック先である cursor（grok-4.6-xhigh）へ、前任が実測した門番 14 本の結果と検出済みの注入 4 件、未着手の注入 5 件を引き継いで再送した。**落ちた委譲先の線を回収しないと、次の担当が同じ浅い注入から始めることになる。**

**C は UTF-8 を一切見ない。** argv・env・cwd・ディレクトリ名は `Bytes` で返し、hado 側が `Bytes.to_string` に通す。非 UTF-8 の `"InvalidUtf8"` は R34 の機構がそのまま出す。C が返すのはスカラと `Bytes` だけで、`raise` は hado にある。I/O の失敗は状態の `Int` と、payload を取る別の intrinsic に分けた。**intrinsic はすべて引数を 1 つ以上取る**（`__argv_count` のような bare の `__` 名は E203 になるので、ダミーの `Int` `0` を足した）。

**仕様への疑義（5 件。R38 で判断する）:** `read_dir` の整列と `.`/`..` の除外、`read_stdin` の 2 回目、`env` の**値**の非 UTF-8、E2 の空 argv の `MissingDirectory`、そして**既定実装の契約名 `EnvDefault` / `IoDefault` の予約**（§47 の予約名の一覧に無い。既定実装を `module` に入れる決定から出た副産物）。

**24.9 の未確認から 2 件が解けた:** サブディレクトリの中の `handler Error` が E204 になる件（工程 1 で決定 4c として直した）と、ローカル束縛が prelude 操作名を影にする件（工程 0 で決定 4a として直した）。

**工程 3 の実装審査は 3 巡かかり、2 巡とも「判定器が指揮者の修正を守っていない」だった。** 1 巡目の重大は「`crates/hado/tests/corpus.rs` の `.current_dir(&work)` と `.stdin(Stdio::null())` を外しても、どの判定も落ちない」。原因は **`Command::output()` が stdin を未指定でも閉じる**ことで、`.stdin(null)` が冗長だったために注入で測れなかった。`spawn()` + `wait_with_output()` に変えて明示を効かせ、**同じテストバイナリを子として起動し直す判定**を置いた（親が子の stdin にバイト列を流し、cwd を `ambient/` にする。固定が外れると `stdin = 19` か `NotFound` で落ち、`ambient/written` の不在も見る）。2 巡目の重大は「**libtest はフィルタが 0 件でも終了 0 を返す**ので、`--exact` の名前がずれると子が何も走らせないまま親が緑になる」。親が子の印字（`launch: status=Some(0)` と `1 passed`）を要求して閉じた。3 巡目で合格。**自分で書いた修正には自分で穴を空ける。** 指揮者が builder の中断を受けて直した 4 箇所のうち、判定を伴っていたのは 1 つもなかった。

**残した軽微:** 親の `contains("1 passed")` は件数の等式ではない（審査 3 巡目の指摘）。`--exact` が 1 本に固定しているあいだは名前ずれを落とせるが、観測対象が 1 本でなくなると部分一致の余地が残る。`--nocapture` が観測路として必須になったことも副作用である。`tools/gates/main.hd` の `choose` の条件が常に真であること（field を読むための器）と、build の時間の判定が無いこと（手順書が要求していない）も据え置いた。

**所要（ログの首尾）:** builder が 7 本で 231 分、実装審査が 7 本で 215 分、計画審査が 6 本で 62 分。全走は 92 分。

## R38 仕上げと v0.3（2026-09-14）

24.5 の R38 を実装した。E2（`tools/gates/`）を実走させてシェル側 oracle と突き合わせ、R32〜R37 が溜めた仕様への疑義と実装の穴 14 件を裁定し、版と lock を締めた。**新しい言語機能は足していない。**

指揮・計画は claude（opus-5）。実装は codex（gpt-6-astra high）、審査は cursor（grok-4.6-xhigh）。計画審査は 2 巡。

| 工程 | 内容 | 審査 |
|---|---|---|
| 0 | `scripts/run-gates.sh` を新設（門番を選んで直列に走らせ、`<門番名>.log` を平坦に吐く）。`scripts/verify-r38.sh` を新設し、E2・oracle・生成時の期待値の三段一致を測る。38-c の探針 6 本 | 1 巡で合格 |
| 1 | 疑義 14 件の裁定。`docs/spec.md` に 6 文、`docs/bug.md` に 3 節、判定 38-d と 38-c の被覆 | 1 巡目 不合格（重大 1）→ 修正 |
| 2 | 判定 38-e（lock・版・台帳） | — |

### 裁定の結果（14 件）

**(a) 仕様に 1 文足した 6 件**（既存の文は書き換えていない）:

| 節 | 足した文の要旨 | 出所 |
|---|---|---|
| §45 | Array の `compare` は共通部分がすべて等しいときだけ短い方を先とする | R35 |
| §47 | 既定実装の契約名 `EnvDefault` `IoDefault` を契約の名前空間で予約し、利用者の同名の契約宣言は E205 | R37 |
| §49 | `Int.parse` の受理文法（**往復から導いた下限**と**本節が決めた上限**をラベルで分けた） | R36 |
| §49 | `read_dir` は名前だけを返し、`.` と `..` を除き、UTF-8 のバイト順に整列する | R37 |
| §49 | `read_stdin` の 2 回目は空の `Bytes` | R37 |
| §49 | `env` の**値**が非 UTF-8 のときも `"InvalidUtf8"` | R37 |

**(b) `docs/bug.md` に 1 節ずつ書いた 3 件**（各節にそのまま実行できる再現手順を置き、判定 38-d がそれを実行して再現を確かめる）: 未使用の効果に E513 が出ない（`dependencies.rs:17` が `Item::Effect` を宣言の集計から除外している）、効果行付きの関数値を返す module member の build が `specialization failed: Mismatch`、効果行を省いた再帰生成器が E306。

**(c) 何も足さなかった 5 件**: ローカル束縛の影（R37 工程 0 で解決）、サブディレクトリの `handler Error`（R37 工程 1 で解決）、外の `def raise = 1`（**E513 ではなく E205**。R37 が `namespaces.rs:156-162` で入れた）、`type T = (Int, Bool)`（**コード無しではなく E100**。`parser.rs:1176-1179`）、E2 の空 argv の `MissingDirectory`（利用者のコードなので §49 の表の外）。

**#8「関数を返す handler の build が `callee is not a closure`」は再現しなかった。** R33 の審査が追加探針で踏んだと記録していたが、abort 経路（`apply (with H -> raise { message = "x" }) 3` に `abort raise _ => inc`）でも正常経路でも `7` を印字して終了 0 になる。計画審査・builder・実装審査の 3 者が独立に同じ結論に達した。**伝聞のまま `docs/bug.md` に書いていたら、文書のほうが嘘になっていた。** 手順書に「裁定の前に必ず現状を再現して確かめる」と書いた効き目である。

### 計画審査が 2 巡で致命 2 件・重大 2 件を捕まえた

**致命 2 件は裁定の分類の誤りである。** #10（外の `def raise = 1` が E513）と #7（`type T = (Int, Bool)` がコード無し）を (b) に置いていたが、審査が実測して E205 と E100 であることを示した。**このまま出していたら、builder が `docs/bug.md` に再現しない節を 2 つ足し、判定 38-d がそれを落としていた。**

**重大 1 件は E2 の入力の形である。** 決定 1 を `<門番名>/<門番名>.log` の入れ子で書いていたが、審査が E2 を build して、入れ子では `read_file` が `EISDIR` になり `IoFailure` で終了 1 になること、直下のファイルなら正しく集計することを実測した。**R37 の 37-h が確立した「ディレクトリ直下のファイル群」という E2 の入力の契約を、手順書を書くときに勝手に変えていた。** 平坦に直し、サブディレクトリの負の対照を判定に足した。

**もう 1 件の重大は判定の形である。** 38-b を「E2 と oracle の一致」だけにしていたが、それだと oracle が壊れたときに **E2 を壊れた oracle に合わせる**直し方が通ってしまう。生成時に控えた期待値を第 3 の足にした。実測: oracle だけを `tail -1` → `head -1` に壊すと 113/131 で落ちる。

### 実装審査の指摘は 2 つとも「判定器が自分の対象を数えている」型だった

工程 1 の重大は「38-d が `docs/bug.md` の見出しの個数と実行件数を突き合わせていないので、実行リストを `[:-1]` にしても満点」。軽微 1 は「38-c の被覆が `^Array の \`compare\` は.*。$` しか見ていないので、`長い方を先とする` という嘘の文でも通る」。**文書から対象を抜き出して回す判定は、抜き出しの段で落としたものを数えられない。** 見出しの個数という独立した数え方と、探針が測る規則そのものの固定文字列で閉じた。

### 全走（2026-09-14、09:51:19→11:12:00 の 81 分）

**門番 37 本すべてが満点、`cargo test --offline --no-fail-fast` が 0 failed、赤はゼロ。** m0 7/7、m1-corpus 7/7、m1-spec 5/5、m2 8/8、m3 12/12、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r9〜r38 の 26 本すべて満点（r34 1360/1360、r35 714/714、r36 388/388、r37 254/254、**r38 140/140**）、record-constructors 1/1、lock 213 件 OK。**24.5 が R38 の終了条件に置いた「全門番が満点、`cargo test` 0 failed、oracle 一致」がすべて成立した。** `v0.3` を tag した。

### 実装審査が残した軽微（v0.4 に持ち越す）

- **38-c の被覆は全文一致である。** 嘘の文（意味を反転させた文）は落ちるので強くなったが、**意味を保ったまま語順や読点を変えた文でも赤になる**。仕様文を触らない前提では実害が無いが、次に §45 / §47 / §49 を改訂するときは判定器を先に緩める必要がある
- **38-d の不変条件はパース結果の長さしか見ていない。** `docs/bug.md` の見出しの個数と `reproductions()` の戻り値の長さを突き合わせるので、パースの段で落とす形は閉じたが、**ループ側でスライスする形（`for ... in cases[:-1]`）は依然通る**。完全に閉じるには回した回数を数える必要がある
- **build の時間の判定が無い。** 手順書 31 節が時間を合否にしていないため据え置いた（R34 の教訓「門番の時間切れは判定の失敗と区別する」）

**所要（ログの首尾）:** builder が 4 本で 79 分、実装審査が 4 本で 65 分、計画審査が 2 本で 25 分。全走は 80 分。

### builder の「中断」が 1 度あり、指揮者の発注書の矛盾だった

工程 1 の修正で、受け入れ条件 4（`verify-r37.sh` が満点）と条件 6（`verify-m*.sh` を走らせない）が衝突した。`verify-r37.sh:581,583` が内部で `m1-corpus` と `m7` を呼ぶためである。builder は門番を変えずに止め、3 コミットの成果を残した。指揮者が実測して `verify-r37.sh` 254/254 を確かめた。**判定器が内部で何を呼ぶかを確かめずに「これは走らせるな」と書くと条件が矛盾する。** R37 で同じ型の間違い（受理ケースを起動するのは `verify-m6.sh` だけという思い込み）をしている。

## 門番の高速化（2026-09-14）

R38 の全走は 81 分、本節の実測でも 76.8 分（37 本、すべて満点）だった。原因は門番の入れ子である。m3 → m2 → m1-corpus → m0 の連鎖で同じ `cargo test` が 1 回の全走に 8 回実走され、m3 の単一スレッド全走（`-- --nocapture --test-threads=1`）がさらに 8 分以上を占めていた。**判定を緩めずに、同じ内容の実走を 1 回に畳んだ。** 計画は `docs/plan.md` 32 節。

### 原因（実測）

2026-09-14 18:44:53〜20:01:47 の直列全走（37 本、全門番 exit 0）。ログと所要は `target/gate-timing/times.tsv` と `logs/`。

| 門番 | 秒 | 入れ子で再走していた門番 |
|---|---|---|
| m0 | 191 | — |
| m1-corpus | 258 | m0 191 |
| m2 | 683 | m0 191、m1-spec 1、m1-corpus 258 |
| m3 | 1660 | m0、m1-spec、m1-corpus、m2 |
| m7 | 219 | m6 9 |
| r36 | 541 | m1-corpus、m7、m4、m5、m6 |
| r37 | 504 | m1-corpus、m7 |

m0・m1-corpus・m2・m3 だけで 2792 秒（46.5 分）。**同じ内容の実走を何度も払っていた。**

### やったこと（`docs/plan.md` 32 節の決定）

- `scripts/lib/gate-cache.sh`。`gate_cached` は作業ツリーの内容ハッシュとコマンドの完全な引数で結果を鍵付けし、成功だけを再利用する。失敗は保存しない
- `scripts/run-gates.sh` が走行スコープを発行し、入れ子の門番で共有する。終了時に消す
- 入れ子の門番呼び出し（m1-corpus→m0、m2→m0/m1-spec/m1-corpus、m3→m0/m1-spec/m1-corpus/m2、m7→m6、r36→m1-corpus/m7/m4/m5/m6、r37→m1-corpus/m7）を `gate_cached` に、`cargo test` を `cargo_cached` に通した
- 全走の全体テストを `cargo test --offline --no-fail-fast` に統一し、m0 / m2 / m3 / m7 で 1 回だけ実走
- m3 の単一スレッド全走を、並列の全走（終了コードと HM のテスト名）と lib の単体テスト `--nocapture --test-threads=1`（`lower_extra_def_ng` の印字）に分けた。判定 8 と 12 の条件は変えていない
- `scripts/verify-gate-cache.sh` を新設（内容の再利用、引数の分離、失敗を保存しない、内容への追随、走行スコープの分離、古い走行の掃除の 6 性質）
- `scripts/run-gates.sh` に任意の所要記録（`HADO_GATE_TIMES`）を足した。入れ子の run-gates には継がせない
- m3 も lib を読む。単独で走らせても m3 の入れ子全体で 1 つの走行スコープを共有する
- 古い走行のスコープ（既定 1 日より古いもの）は、新しい走行が始まるときに消す

### 実測（2026-09-14）

| | 短縮前 | 短縮後 |
|---|---|---|
| 門番の本数 | 37 | 38（`verify-gate-cache.sh` を新設） |
| 全走の所要 | 4606 秒 = **76.8 分**（18:44:53〜20:01:47） | 1002 秒 = **16.7 分**（20:52:19〜21:09:01） |
| 赤 | 0 | 0 |
| `cargo test` | 734 passed / 0 failed | 734 passed / 0 failed |

門番ごとの所要（秒、短縮前 → 短縮後）。20 秒以上変わったものだけ:

| 門番 | 前 | 後 |
|---|---|---|
| m1-corpus | 258 | 47 |
| m2 | 683 | 52 |
| m3 | 1660 | 31 |
| m7 | 219 | 13 |
| r36 | 541 | 77 |
| r37 | 504 | 38 |

### 判定を緩めていないこと

短縮前後の全 37 門番で、末尾の `PASSED: n/N` が 1 文字も変わらず、`^NG:` の数も 0 のままである（比較は `target/gate-timing/logs/` と `optimized-logs/`）。新しい `verify-gate-cache.sh` は 6/6。`cargo test --offline --no-fail-fast` は 734 passed / 0 failed。

### 自審査と全走で見つけて直したもの

1 度目の短縮後の全走で **r38 が 139/140 で赤**になった。`HADO_GATE_TIMES` が r38 の合成木の `run-gates.sh` に継がれ、一時ディレクトリからの相対パスへの追記が stderr に出たためである。判定 38-a が「stderr が空」を要求して落ちた。**計測用の環境変数が判定器の入力に混ざっていた。** 最外の run-gates だけが記録し、入れ子には継がせない形に直し、追記が失敗しても stderr を汚さない形にした。

自審査で 2 つ見つけた。1 つ目は **m3 が lib を読んでおらず、単独で走らせると入れ子の畳み込みが効かなかった**こと。m3 は Python から `gate-cache-run.sh` を呼ぶため、走行スコープが子ごとに別になっていた。m3 も lib を読む形に直し、単独の所要は 1660 秒 → 301 秒（12/12）になった。2 つ目は **走行スコープが `target/gate-cache/` に残る**こと（25 個、844K を確認）。古い走行のスコープ（既定 1 日より古いもの）を新しい走行の始めに消す形にし、判定 `stale run scopes are pruned` を足した。直した後の全走は 38 本すべて満点で、走行後に残るスコープは 0 個である。

### 仕様への疑義

なし（`docs/spec.md`、`crates/`、`runtime/`、`stdlib/`、`tests/` は触っていない）。

### 限界（v0.4 に残す）

- キャッシュは走行 1 回に閉じる。走行をまたぐと、乱数で探針を作る r33〜r38 を凍結し、root / 非 root のような内容に現れない環境差を結果に持ち越すためである
- `cargo run` を事前ビルドのバイナリに置き換える最適化は入れていない
- 門番の並列実行は入れていない（R38 の「直列に走らせ」を維持）
- 単独で門番を走らせたときの走行スコープは、走行の終わりには消えない。次に走行が始まったとき、既定 1 日より古ければ消える（全走のスコープは run-gates が終わりに消す）

## 門番の層別と検査の棚卸し（2026-09-15）

計画は `docs/plan.md` 33 節。32 節で全走は 16.7 分になったが、目的ごとに回す頻度が無い。3 つの作業（層別、ラウンド門番 r9〜r38 の棚卸し、`docs/spec.md` の文章固定検査の棚卸し）を 1 ラウンドで行った。**門番は 1 つも消していない。判定条件は緩めていない。**

### 実装

- `scripts/lib/gate-tiers.sh` を門番名の唯一の正にした。`hado_tier_minimal_gates <層>` が各門番を最小の層に 1 回ずつ割り当て、`hado_tier_gates <層>` が累積を返す。**層は分割、実行は累積**（quick / quick+core / 全 38 本）なので、3 層は 38 本を重複なく覆う。
- `scripts/run-gates.sh --tier quick|core|full`。`--tier core 出力先 verify-r33` のように層へ門番を足せる（重複は落とす）。`--print-tier` は一覧を出すだけで出力ディレクトリも走行スコープも作らない。層も明示も無い引数なしの全走は full と同じ扱いで、従来の `run-gates.sh 出力先` でも文書層まで回る。quick は門番の前に `cargo build --offline` を 1 回実走し、ログ末尾を集計器に合わせて `PASSED: n/N` に揃える。core の終わりに full でのみ回す門番を列挙する。
- 文書層は `scripts/lint-docs.sh`（7 検査）として分離。`verify-*.sh` ではないので 38 本に数えず、full の最後だけが回す。
- 部分 `cargo test` の実走を `cargo_test_cached` が全走へ畳む。畳む条件は、同じ走行の `cargo test --offline --no-fail-fast` が成功し、**そのログに「対象の `tests/<名前>.rs` の実行」と続く「`test result: ok`」の証拠がある**ときだけ。`--test <名前>` 以外の引数（`--` 以降、`-q`、`--lib`、`--features`、`--release`）があれば実走する。**証拠を要求するので、対象の改名・削除や赤の全走で検出力は落ちない。** 赤い全走のログは証拠として残さない。

### 層の実測（2026-09-15）

| 層 | コマンド | 所要 | 目標 | 赤 |
|---|---|---|---|---|
| quick | `run-gates.sh --tier quick target/tier/quick` | **1.7 秒** | ≦ 2 分 | 0 |
| core | `run-gates.sh --tier core target/tier/core` | **6 分 19 秒** | ≦ 7 分 | 0 |
| full | `run-gates.sh --tier full target/tier/full` | **13 分 39 秒** | ≦ 18 分 | 0 |

quick は `cargo build --offline`（1/1）、`verify-gate-cache`（14/14）、`verify-m1-spec`（1/1）。core は m0 7/7・m1-corpus 7/7・m1-spec 1/1・m2 8/8・m3 12/12・m4 133/133・m5 145/145・m6 71/71・m7 3/3・r38 138/138・gate-cache 14/14。core の所要は m0 190 秒（全体の半分）と m1-corpus 47・m2 52 が主である。

### full の門番ごとの `PASSED: n/N`（33-b）

32 節の実測と一致した。**N が変わったのは 3 本だけで、いずれも検査を移したか足した結果である。**

| 門番 | 32 節 | 今回 | 差の理由 |
|---|---|---|---|
| m1-spec | 5/5 | 1/1 | 検査 1・2・4・5 を lint-docs へ移した |
| r38 | 140/140 | 138/138 | 38-e の版と台帳の 2 検査を lint-docs へ移した |
| gate-cache | 6/6 | 14/14 | 層の自己判定 5 と cargo 再利用の対照 3 を足した（33-e が許す増加） |

他は m0 7/7、m1-corpus 7/7、m2 8/8、m3 12/12、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r9 5/5、r11〜r17 と r18〜r22 と r28 と record-constructors が各 1/1、r23 3/3、r24 2/2、r25 4/4、r26 2/2、r29 24/24、r30 16/16、r31 25/25、r32 45/45、r33 73/73、r34 1360/1360、r35 714/714、r36 388/388、r37 254/254。`^NG:` は全ログで 0。lint-docs は 7/7。`cargo test` は 734 passed / 0 failed（m0 の `OK: 2 cargo-test passed=734`）。

### ラウンド門番の棚卸し（r9〜r38）

| 門番 | 検査 | どこが守っているか | 判定 |
|---|---|---|---|
| r9 | 5 | r9_syntax / r9_grammar / parse_corpus / r9_order / check_cli / r9_names / r9_unused / r9_module_types / prelude / r9_coverage / r9_print / emit_mono / corpus の各テストバイナリ | m0 の全走と重複（実走のみ畳む） |
| r11〜r17 | 各 1 | r11_comments 〜 r17_files | m0 の全走と重複 |
| r18〜r22 | 各 1 | r18_spelling 〜 r22_namespaces | m0 の全走と重複 |
| r23 | 3 | 23-a r9_grammar、23-b E1xx と §50 の一致、23-c r23_syntax | a・c は m0、b は m1-corpus/m2 と重複。消さずに残す |
| r24 | 2 | 24-a..e r24_effects、24-f E308/E522/E605 と §50 の一致 | a は m0、f は m1-corpus と重複 |
| r25 | 4 | 25-a/c corpus の `--exact`、25-b E509 と §50、25-c r25_effects/r14_rails/emit_mono、25-e accept に `Ok(`/`Error(` が無い | a・b・c は重複。**25-e は r9〜r28 で唯一の固有検査** |
| r26 | 2 | 26-a/b/c/e r26_effects、26-d §50 との一致 | どちらも重複 |
| r28 | 1 | 28-a/b/c r28_effects | m0 の全走と重複 |
| r29 | 24 | SIGPIPE / EPIPE / `/dev/full` の実プロセス | **固有** |
| r30 | 16 | `capture` handler の合成 | **固有** |
| r31 | 25 | yield 地点の継続と `$Outcome` の型 | **固有** |
| r32 | 45 | ライブラリ単位 | **固有** |
| r33 | 73 | 型の共有。うち `cargo test -q --test r9_grammar` / `r23_syntax` の 2 本は m0 と重複 | 固有（2 本は畳まずに残した。下の「迷った点」） |
| r34 | 1360 | Bytes と String の生成探針 | 標本の**広さ**が固有 |
| r35 | 714 | 配列の生成探針 | 標本の**広さ**が固有 |
| r36 | 388 | m1-corpus/m7/m4/m5/m6 の再走は 32 節のキャッシュで 1 回。上乗せ分は固有 | 一部重複（再走は既に 1 回） |
| r37 | 254 | m1-corpus/m7 の再走は 1 回。上乗せ分は固有 | 一部重複（再走は既に 1 回） |
| r38 | 138 | 38-a 合成木の runner、38-b E2 oracle、38-c の探針、38-d bug.md の再現、38-e lock | **固有** |

`crates/hado/tests/r18_checks.rs` はどの門番からも参照されていない（`verify-r18.sh` は `r18_spelling` を走らせる）。全走の `cargo test` には入る。今回は触っていない。

### 削除・移動した検査と根拠（33-c）

**削除した検査は 1 つも無い。** 移したのは 6 つで、すべて `scripts/lint-docs.sh` が同じ検査を回す。

| 元 | 移動先 | 移した根拠 |
|---|---|---|
| m1-spec 検査 1（見出し §1〜§42 が 1 回ずつ、消していない） | lint-docs 検査 1 | spec の書き方であって処理系の契約ではない |
| m1-spec 検査 2（新設節 §43〜§59 が §42 より後） | lint-docs 検査 2 | 同上 |
| m1-spec 検査 4（`spec-coverage` が計画の表 2.2/2.3/9.2/11.2/13.2/18.4/24.2 と一致） | lint-docs 検査 3 | 文書どうしの対応。処理系は §50 の表だけを読む（検査 3 は m1-spec に残る） |
| m1-spec 検査 5（条件列の逐語「パーサは改行を見ない」） | lint-docs 検査 4 | 文の固定。処理系の契約は各テストが持つ |
| r38-e 版 `# 言語仕様案 v0.3` | lint-docs 検査 5 | 版は spec と spec-coverage の**一致**に一般化した（固定値を消した） |
| r38-e `## R38 ` の存在 | lint-docs 検査 6 | 台帳の運用。**最新ラウンド**の節の存在に一般化した |

r38-c の 6 つの逐語 regex は lint-docs 検査 7 へ移し、`verify-r38.sh` の 38-c は
**probe の key と §番号の対応・その見出しの実在・否定対照（manifest の § を差し替える／見出しを消す）**に置き換えた。文を固定しなくても、規則の振る舞いは 38-c の探針 6 本が実走で測る。**38-c(6) の `EnvDefault`/`IoDefault` の E205 は、この探針が r38 に残るので保護は落ちない。** 38-e の corpus lock は r38 に残した。

`tests/corpus.sha256` は更新していない。`tests/`・`stdlib/`・`crates/`・`runtime/` の差分は空である（`git status --short` と `git diff --stat` で確認）。corpus の実数は accept 119 ファイル・reject 69 コード・stdlib 10 ファイルで、§50 の 69 行と一致する（m1-corpus の検査 5 が緑）。`cargo test` は 734 件のまま。

### 33-f（層が実在の門番と一致し、重複なく覆う）

`verify-gate-cache.sh` に機械検査を足した。実在の `scripts/verify-*.sh` の集合と層の一覧が一致すること、最小層に重複が無いこと、`run-gates.sh --print-tier` が同じ一覧を返すこと、quick ⊆ core ⊆ full かつ full が 38 本であること。合成木で、引数なしの全走が文書層まで回ること、`--tier` に明示した門番が重複せず足されること、相対の出力先が呼び出し側の作業ディレクトリで解決されることも測る。14/14。

### 実装審査（claude / fable 5.1）と対応

重大は無し。中 2 件と軽微を直した。

- **`--tier core 出力先 verify-r33` が明示した門番を黙って捨てていた。** 層の一覧で `selectors` を上書きしていたため。層に足して重複を落とす形に直し、合成木の対照を足した。
- **引数なしの全走（README の `run-gates.sh 出力先`）で文書層が回らなかった。** m1-spec 検査 1・2・4・5 と r38 の固定がこの形から抜けていた。引数なしを full と同じ扱いに直し、合成木の対照を足した。
- 証拠判定の awk に `Doc-tests` の境界を足した（対象が結果行を出さずに落ちると、続く `Doc-tests hado` の ok を証拠と誤認し得た）。
- 赤い全走のログを残さないようにした（「成功だけを保存する」原則との食い違い）。
- 出力ディレクトリの相対パスを呼び出し側の作業ディレクトリ基準に戻した。`--tier bogus` が空のディレクトリを作らないよう、層の検証を mkdir の前に移した。
- lint-docs の台帳検査の glob を `verify-r*.sh` から `verify-r[0-9]*.sh` に狭めた。

2 巡目で 9 件すべてが閉じていることを確認した。残った軽微も直した。core の「full 専用の一覧」が明示的に足した門番を差し引いていなかったので、この走行で回した `gates` との差に変えた。赤い全走のログ消しは、呼び出し側が `set -e` と pipefail だと `rm` に達する前に抜け得たので、パイプをやめて一時ファイルに写す形にした。相対の出力先の自己判定を足した（合計 14/14）。

### 迷った点

- **層の意味。** 計画の表は m1-spec を quick と core の両方に置く。これを「実行は累積」と読んで、一覧は最小層に 1 回だけ持つ形にした。層が分割でなければ 33-f の「重複なく覆う」と両立しない。
- **r33 の 2 本。** `cargo test --offline -q --test r9_grammar` と `r23_syntax` は m0 の全走と重複するが、`-q` は再利用の allowlist の外なので畳んでいない（r33 は 14 秒）。計画の棚卸しの中心は r9〜r28 の重複と r36/r37 の再走なので、境界として残した。
- **`crates/hado/tests/r18_checks.rs`。** どの門番からも参照されていない。全走には入るので緑だが、門番としての持ち主が無い。今回は触らない。
- **`verify-m0.sh` の awk。** 同じ `Doc-tests` の境界が無い（既存）。今回は触っていない。

### 仕様への疑義

なし（`docs/spec.md`、`crates/`、`runtime/`、`stdlib/`、`tests/` は触っていない）。

## 門番の並列実行（2026-09-15）

計画は `docs/plan.md` 34 節。33 節の直列 full は 13 分 39 秒で、門番ごとの秒の総和 826 秒が wall とほぼ等しく、12 コアが遊んでいた。`--jobs N`（既定 1=直列）で判定を 1 つも緩めずに並列化した。**門番は 38 本のまま。判定条件は緩めていない。**

### 実装

- `scripts/lib/gate-tiers.sh` に `hado_tier_serial_gates`（gate-cache・m0・m1-corpus・m2・m3）を足した。前段の一覧の唯一の正で、`verify-gate-cache.sh` が実在の門番の部分集合であることを機械で見る。
- `scripts/lib/gate-cache.sh`。`gate_cached` を flock（`$dir/lock`）＋取ってからの再確認＋実走＋`out` → `status` の temp → rename にした。実走するコマンドへ lock の fd を継がせない。全走テストのログも成功時だけ rename で公開し、既に公開済みなら再公開しない。同じ id を保持したまま再入したら待たずに赤で止める。`HADO_GATE_RUNS` を設定すると実走と再利用を label 付きで 1 行残す（診断）。
- `scripts/run-gates.sh`。`--jobs N`（既定 1、正の整数のみ）を足した。1 のときは 33 節の直列ループをそのまま使う。2 以上は前段（直列の一覧 ∩ 走行する門番）→ 後段（N 並列）→ 集約。終了コードと所要は `mktemp -d` に門番ごとの 1 ファイルとして残し、親が一覧の順に集約する（status の無いワーカーは赤）。前段の後と後段の後に内容ハッシュを比べ、変われば分類漏れとして赤にする。走行自身がログを書く出力先はハッシュから外す。前段に赤があれば後段を直列へ落とす。中断時はワーカーを子孫ごと止める。
- 入れ子の label を `verify-<名>` に統一した（`run-gates.sh` の `verify-verify-m0` と `verify-m2.sh` の `verify-m0.sh`）。同じ門番は走行内で 1 回だけ実走する。
- `scripts/verify-gate-cache.sh` を 14/14 から 26/26 へ。並行呼び出しの 1 回化、公開順、label 一致、barrier、並列度の上限、赤の非隠蔽、`HADO_GATE_TIMES` の 1 行、不正な `--jobs`、安定性 guard の対照、前段赤の直列フォールバック、出力先がツリーの中の対照を測る。

### 判定の実測

- **34-a（既定が 33 節と一致）。** 33 節版（b099e65）と 34 節版の `run-gates.sh` を同じ合成木で走らせ、stdout（出力先の正規化後）・stderr（0 バイト）・ログ・trace が byte 一致。`verify-r38.sh` の 38-a は 138/138 で緑のまま。**直列 full は 12 分 41 秒（761.39 秒）で、33 節の 13 分 39 秒より 58 秒短い**（label 統一で入れ子の再走が消えた分）。出力と `PASSED: n/N` は不変。
- **34-b（直列と並列の PASSED が一致）。** 直列・`--jobs 2`・`4`・`8` の 4 走行で、38 本の `PASSED: n/N` が 1 本ずつ完全一致。全ログで `^NG:` は 0、終了コード 0、`lint-docs` は 7/7。
- **34-c（cargo test）。** m0 の `cargo-test passed=734`。4 走行とも同じ。
- **34-d（corpus）。** r38 38-e の corpus lock が 4 走行とも `OK=213 NG=0 status=0`。`tests/corpus.sha256` は未更新で、`tests/`・`stdlib/`・`crates/`・`runtime/` の差分は空。accept の `main.hd` は 104・reject のコードは 69・stdlib の `.hd` は 10 で、全走行同じ（同一ツリー。m1-corpus は 7/7）。
- **34-e（赤を隠さない）。** 合成木に赤を 1 本混ぜた `--jobs 4` は exit 1 で赤のログが残る。前段が赤のときは後段を直列へ落とす（自己判定）。
- **34-f（自己判定）。** `verify-gate-cache.sh` は 26/26。手元の変異（status を out より先に公開、前段を fan-out に混ぜる、`--jobs` の上限を無視）でいずれも NG になることを確かめた。
- **34-g（実走回数）。** `HADO_GATE_RUNS` を付けた直列と `--jobs 8` の全走で、`verify-*` の label ごとの実走が 1 回。入れ子の再走は無い。
- **34-h（安定性 guard）。** `--jobs 2/4/8` の stdout に guard の発火は無い。後段がツリーを恒久的に変える対照では赤になり、出力先がツリーの中の対照では赤にならない（自己判定）。
- **34-i（速度）。** 下表。

### 速度（34-i）

| 走行 | wall | CPU（user+sys） | 短縮 | 門番秒の総和 | 赤 |
|---|---|---|---|---|---|
| 直列（既定） | 12 分 41 秒（761.39） | 1235.12 | 1.00× | 761 | 0 |
| `--jobs 2` | 9 分 14 秒（553.55） | 1270.10 | 1.38× | 789 | 0 |
| `--jobs 4` | 7 分 52 秒（472.44） | 1313.79 | 1.61× | 831 | 0 |
| `--jobs 8` | 7 分 47 秒（466.73） | 1360.31 | **1.63×** | 873 | 0 |

`--jobs 8` の 466.73 秒は計画の理論下限（約 466 秒）にほぼ一致する。前段は m0 195・m1-corpus 48・m2 36・m3・gate-cache で約 294 秒あり、直列で残る。後段の最長は r35（直列 137 秒、8 並列で 156 秒）なので下限は約 450 秒。`--jobs 4` と `8` がほぼ同じなのは、後段の臨界路が r35 1 本で、前段が全走の 4 割を占めるためである。CPU は 8 並列で 1.10 倍に増える（cargo の target ロックと鍵計算の競合）。門番ごとの秒は並列度が上がるほど伸びる（r35 137→156、r34 135→153、r36 53→65）が、wall は縮む。

### 実装審査（claude / fable 5.1）と対応

重大は無し。中 6 件と軽微を直した。

- **中断時に門番が孤児で残った。** trap の kill が親の subshell にしか当たらず、flock を握る subshell と門番本体が生き残っていた（審査の実測で 6 プロセス）。`pgrep -P` で子孫を葉から止める形にし、合成木の対照で orphan が 0 になることを確かめた。
- **前段が赤いと後段で入れ子が再走した。** 赤い門番は probe を戻し切らずに終わり得る。前段の赤を見たら後段を直列へ落とす（赤は既に確定。判定は緩まない）。
- **barrier の自己判定が不安定だった。** 前段の門番が marker を置く直前に sleep 0.3 を入れ、前段を fan-out に混ぜた実装を確実に捕まえる。
- **公開順の自己判定が順序を測っていなかった。** `mv` を包んで公開先の順（out → status）を記録する形にした。
- **自己判定が外側の `HADO_GATE_RUNS` を汚した。** 判定器の冒頭で unset し、`HADO_GATE_TIMES` の検査は `env -u HADO_GATE_RUNS` も外す。
- **出力先がツリーの中だと安定性 guard が誤爆した。** 走行自身が書く出力先を内容ハッシュから外した（`pwd -P` の実体パス比較）。README と 34 節に書いた。
- 軽微: 同 id の再入を検出して赤で止める、`wait -n -p` で reap 済み pid を追跡から外す、到達不能なログ削除を消す、並列度の上限の自己判定を足す。label を grep する lint は見送った（m7 の `verify-m6-with-prelude` など例外があり脆い。代わりに `label_agreement_case` が runner と入れ子の一致を測る）。

### 迷った点

- **status 欠落の自己判定。** 「ワーカーが status を書く前に死ぬ」合成が難しい（門番はワーカーの孫で、`$PPID` を殺してもワーカーは生き残る）。集約はフェイルクローズドのままにし、検査は足していない。
- **実測の分離。** 実装審査と同時に core を回したら m1-corpus が 246 秒に伸びた。審査が同時に実門番と変異検査を走らせていたためで、分離した測定では m0 → m1-corpus の再生が `HADO_GATE_RUNS` の `hit verify-m0` で確認できた。以後の全走は直列に並べて回した。
- **`--jobs 4` と `8` の差が小さい。** 前段（m0 と入れ子）が下限を決める。前段を縮めるには m0 の全走テストを分ける改修が要る（今ラウンドのスコープ外）。

### 仕様への疑義

なし（`docs/spec.md`、`crates/`、`runtime/`、`stdlib/`、`tests/` は触っていない）。

## R39 仕様書の再構成（2026-09-15）

計画は `docs/plan.md` 35 節。`docs/spec.md` は節を末尾に足す形で育ち、文法（旧 §41）の後ろに原則と字句、型（旧 §36・§37）が module の後ろ、引数の分岐形（旧 §52・§53）がエラーコード表の後ろにあった。並びを読み順に直し、節番号を 1..59 に振り直した。**言語規則は変えていない。**

### 実装

- `docs/spec.md` をタイトル・部 `# 第X部 …`・節 `## N. …` の 3 層にした。前書き（規範性の語と目次）を足し、節は §1〜§59 を読み順に並べた。本文は節単位で丸ごと移し、節内の `第k節`・`§k` だけを写像で置換した。
- `docs/spec-map.md` を新設した。旧 1..59 から新 1..59 への全単射と、各節の新しい所属を持つ。`docs/plan.md`・`docs/ledger.md` の §N は決定と実行の記録なので書き換えず、この表を通して読む。
- 追随した参照: `docs/spec-coverage.md`・`docs/spec-trace.md`・`docs/corpus.md`・`README.md`・`docs/bug.md`・`docs/wants.md`、`tests/accept`・`tests/reject`・`tests/library` の `// spec:` 175 ファイル、`tests/corpus.sha256` の 175 行、`scripts/`（`lint-docs.sh`・`verify-m1-spec.sh`・`verify-m1-corpus.sh`・`verify-m3.sh`・`verify-m7.sh`・`verify-r23.sh`〜`verify-r26.sh`・`verify-r32.sh`・`verify-r34.sh`・`verify-r36.sh`・`verify-r37.sh`・`verify-r38.sh`）、`crates/hado/tests/r9_grammar.rs`・`r23_syntax.rs`、`crates/hado/src/` と `crates/hado/tests/` の節番号のコメント。
- `scripts/verify-r39.sh` を新設し、`scripts/lib/gate-tiers.sh` の full 層に `r39` を足した。`scripts/lint-docs.sh` の見出しの検査は「§1〜§59 が `## N.` として 1 回ずつ読み順に存在し、節が部の下にある」に置き換えた。`git diff` による見出しの削除の検査は番号の振り直しと両立しないので外し、内容の欠落は `verify-r39.sh` の保存検査で測る。

### 判定の実測

- **39-a（構造・写像・保存・参照）。** `bash scripts/verify-r39.sh` は 5/5。内訳は `sections: 59, part headings: 13`、`map rows: 59 (bijection)`、`preserved: 59 sections from v0.3`、`codes: 69, corpus tags resolve and match`、`live document references resolve`。保存検査は `git show v0.3:docs/spec.md` の各節の本文に節参照の置換を当てたものが、写像先の新しい節の本文に現れることを見る。
- **39-b（文書層）。** `bash scripts/lint-docs.sh` は 7/7。`coverage: §22=1 §50=1 §56=4`。
- **39-c（コーパスと門番）。** 変更した門番を 1 本ずつ実走した: `verify-m1-spec.sh` 1/1、`verify-m1-corpus.sh` 7/7、`verify-m2.sh` 8/8、`verify-m3.sh` 12/12、`verify-m7.sh` 3/3、`verify-r34.sh` 1360/1360、`verify-r36.sh` 388/388、`verify-r37.sh` 254/254、`verify-r38.sh` 138/138、`verify-r39.sh` 5/5、`lint-docs.sh` 7/7。
- **39-d（Rust）。** `cargo test --offline -q --test r9_grammar --test r23_syntax --test parse_corpus` は r9_grammar 5、r23_syntax 9、parse_corpus 1 の全て passed（exit 0）。
- **39-e（corpus-lock）。** `tests/corpus.sha256` は 213 行のまま、うち 175 行のハッシュを更新した。`sha256sum --strict --status -c tests/corpus.sha256` は exit 0。`verify-m3.sh` の check10 も緑。accept の `main.hd` は 104、reject のコードは 69、`tests/library/` は 4。
- **39-f（全走）。** `bash scripts/run-gates.sh --jobs 8 --tier full /tmp/hado-gates-r39` は exit 0、40 本、real 7 分 51 秒（466 秒）。`^NG:` は 0。全走のログは一時ディレクトリに置いたため、変更した 11 本は `target/r39-gates/` に写して上に数えた。
- **39-f（層別）。** `bash scripts/run-gates.sh --tier quick target/r39-gates/quick` は 7 秒（build、gate-cache 26/26、m1-spec 1/1）。`--tier core target/r39-gates/core` は 353 秒で、gate-cache 26/26、m0 7/7（cargo test の全走）、m1-corpus 7/7、m1-spec 1/1、m2 8/8、m3 12/12、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r38 138/138。どちらも exit 0 で `^NG:` は 0。
- **39-g（規則の不変）。** v0.3 の §50 と新しい §57 の 69 行を照合し、コード・短い名・段階が一致、アンカーの欄が節参照の置換の像、節の列が写像の像であることを確かめた。`git diff --stat -- stdlib runtime` は空、`crates/` の差分は節番号の参照（コメントと `r9_grammar.rs`・`r23_syntax.rs`・`unit/parser.rs` が見る見出し文字列）の追随だけである。

全走の後に `docs/plan.md` の 39-g を書き換えながら `verify-m1-corpus.sh` を回し、check7 が `protected-files-changed-during-verification` で 6/7 になった。検査が正しく効いた例で、編集を止めて回し直すと 7/7 である。

### 迷った点

- **§5 と §6 の順。** 文法の概形（旧 §41）をリテラル（旧 §43）より先に置いた。§5 の「字句と構文の形はこの節を正とし、第6節と第8節は値の意味だけを定める」が素直に読める並びを選んだ。字句の規則が 2 節に分かれる点は残る。
- **記録と現行の線引き。** `docs/plan.md`・`docs/ledger.md` の §N は当時の番号のまま残し、`docs/spec-map.md` を橋にした。決定と実行の記録を書き換えると、当時の判断が何を根拠にしたか読めなくなる。現行の文書とコードのコメントは追随させた。
- **節の重複。** 環境変数の文（旧 §37 と §48）、除算の効果（旧 §36 と §49）、字句の規則（旧 §41 と §43）は同じ事実を 2 か所に持つ。統合は本文の書き換えなので今ラウンドの外に置いた。
- **見出しの削除の検査。** 旧 `lint-docs.sh` の `git diff` による「見出しを消していない」検査は、番号の振り直しで必ず赤になる。内容の保存は `verify-r39.sh` が v0.3 との照合で測る形に替えた。
- **tag との対応。** 版は v0.3 のままで、`git tag v0.3` は並べ替える前の本文を持つ。命題は同じで、対応は `docs/spec-map.md` が示す。

### 仕様への疑義

- §5（旧 §41）は「字句と構文の形はこの節を正とし」と書きながら、字句の一部（リテラルとコメント）の規則は §6 にある。並びを変えても意味は保たれているが、規範の置き場としては 2 節にまたがる。統合は別ラウンドとする。

## R40 文法の概形を付録へ移す（2026-09-15）

計画は `docs/plan.md` 36 節。R39 で §5（第II部 字句と文法の先頭）に置いた文法の概形は、言語仕様を読む開発者が最初に読むものではない。参照用の付録へ移し、空いた分だけ残りの節を繰り上げた。**言語規則は変えていない。** 版を v0.3.1 に上げ、tag `v0.3.1` を打った。

### 実装

- `docs/spec.md` の文法の概形を §59（付録の最後）へ移した。付録は コア構文（§57）・典型コード（§58）・文法の概形（§59）。§1〜§4 はそのまま、§5（文法の概形）は §59、§6〜§57 は 1 つ繰り上げて §5〜§56、§58・§59 は §57・§58。本文は節単位で移し、節内の参照だけを写像で置換した。
- `docs/spec-map.md` を v0.3・R39・現行の 3 列にした。`docs/plan.md` 35 節と R39 の台帳は R39 の番号を使うので、そこからも引ける。
- 追随した参照: `docs/spec-coverage.md`・`docs/spec-trace.md`・`docs/corpus.md`・`README.md`・`docs/bug.md`・`docs/wants.md`、コーパスの `// spec:` 174 ファイル、`tests/corpus.sha256` の 174 行、門番（`lint-docs.sh`・`verify-m1-spec.sh`・`verify-m1-corpus.sh`・`verify-m2.sh`・`verify-m3.sh`・`verify-m7.sh`・`verify-r23.sh`〜`verify-r26.sh`・`verify-r32.sh`・`verify-r34.sh`・`verify-r36.sh`・`verify-r37.sh`・`verify-r38.sh`）、`crates/hado/tests/r9_grammar.rs`・`r23_syntax.rs`、`crates/` の節番号のコメント。
- `scripts/verify-r39.sh` を `scripts/verify-r40.sh` に置き換え、`scripts/lib/gate-tiers.sh` の full の `r39` を `r40` にした。本文保存の検査は tag `v0.3` を基準にしたままである。
- 版を v0.3.1 に上げ、`docs/spec.md`・`docs/spec-coverage.md`・`docs/spec-trace.md` の版を揃えた。tag は `v0.1.1` などと同じ軽量 tag にした。

### 判定の実測

- **40-a（構造・写像・保存・参照）。** `bash scripts/verify-r40.sh` は 6/6。`sections: 59, part headings: 13, last: 付録`、`map rows: 59 (3 columns, each a permutation)`、`preserved: 59 sections from v0.3`、`codes: 69, corpus tags resolve and match`、`version: v0.3.1`。
- **40-b（文書層）。** `bash scripts/lint-docs.sh` は 7/7。`coverage: §21=1 §49=1 §55=4`。
- **40-c（門番）。** `verify-m1-spec.sh` 1/1、`verify-m1-corpus.sh` 7/7、`verify-m2.sh` 8/8、`verify-m3.sh` 12/12、`verify-m7.sh` 3/3、`verify-r34.sh` 1360/1360、`verify-r36.sh` 388/388、`verify-r37.sh` 254/254、`verify-r38.sh` 138/138。
- **40-d（層別）。** `--tier quick target/r40-gates/quick` は 7 秒（build、gate-cache 26/26、m1-spec 1/1）。`--tier core target/r40-gates/core` は 352 秒で、gate-cache 26/26、m0 7/7（cargo test の全走）、m1-corpus 7/7、m1-spec 1/1、m2 8/8、m3 12/12、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r38 138/138。`--jobs 8 --tier full target/r40-gates/full` は 40 本、472 秒。3 走行とも exit 0 で `^NG:` は 0。
- **40-e（cargo test）。** `cargo test --offline -q --test r9_grammar --test r23_syntax --test parse_corpus` は r9_grammar 5、r23_syntax 9、parse_corpus 1 の全て passed（exit 0）。
- **40-f（corpus-lock）。** `tests/corpus.sha256` は 213 行のまま、174 行のハッシュを更新した。`sha256sum --strict --status -c tests/corpus.sha256` は exit 0。`verify-m3.sh` の check10 も緑。
- **40-g（規則の不変）。** v0.3 の §50 と現行の §56 の 69 行を照合し、コード・短い名・段階が一致、アンカー欄が節参照の置換の像、節の列が写像の像であることを確かめた。`git diff --stat -- stdlib runtime` は空。
- **40-h（版と tag）。** `docs/spec.md` と `docs/spec-coverage.md` の版は v0.3.1。軽量 tag `v0.3.1` は最終コミットを指し、tag `v0.3` は動いていない（`git rev-parse v0.3^{commit}` は d1c251d のまま）。

### 迷った点

- **付録の位置。** 文法の概形を付録の最後（コア構文・典型コードの後）に置いた。付録の先頭に置く案もあったが、既存の 2 つの付録の並びを変えない方を選んだ。
- **版を上げるか。** 言語規則が同じなので v0.3 のまま tag を動かす案もあった。tag は動かさず、並びの版として v0.3.1 を打った。`v0.3` は並べ替え前の本文を指したままで、保存の検査の基準になる。
- **典型コードの例の抽出。** R39 では典型コードが最後の節だったので、`verify-m1-corpus.sh` の check2 は節の終端を書いていなかった。R40 では次に文法の概形が来るため、文法の例（`int :=` など）まで拾って赤になった。節見出しで止める形に直した。門番が前提の変化を炙り出した例である。
- **裸の節番号。** `§N` の一括置換では、`lint-docs.sh` と `verify-r38.sh` の対応表にある裸の整数（`(22,` など）は置換されない。手で追随させ、門番の赤で拾った。

### 仕様への疑義

- 文法の概形（§59）とコア構文（§57）はどちらも構文の一覧を持つ。付録に並ぶので重複が目に付く。統合は別ラウンドとする。

## R41 旧仕様の記載を外す（2026-09-15）

計画は `docs/plan.md` 37 節。R40 は節番号を振り直した後も、v0.3・R39 の番号を現行に写す `docs/spec-map.md` と、tag `v0.3` の本文との照合を門番に残した。文書には最新の snapshot だけを置けばよいという判断になり、対応表と保存の検査を外した。**言語規則は変えていない。** 版は v0.3.1 のまま、tag `v0.3.1` を最終コミットに付け直した。

### 実装

- `docs/spec-map.md` を削除した。旧番号を現行に写す表は持たない。
- `docs/spec.md` の前書きから旧 tag・旧番号の説明を外し、「版 v0.3.1 は git tag `v0.3.1` に対応する。」だけを残した。
- `scripts/verify-r40.sh` を `scripts/verify-r41.sh` にし、3 列の写像と tag `v0.3` との本文保存の検査を落とした。検査は 5 つ。節の並び（文法の概形が §59）、最新のみ（対応表が無く、前書きに v0.3.1 以外の版と `spec-map` が無い）、コーパスのタグと §56 の表、現行文書の §参照、版。
- `scripts/lib/gate-tiers.sh` の full の `r40` を `r41` にした。
- `docs/plan.md` 35・36 節とこの台帳の R39・R40 は、当時の番号と `docs/spec-map.md` への言及を残した。当時の判断が何を根拠にしたかの記録である。

### 判定の実測

- **41-a（最新の仕様の自己整合）。** `bash scripts/verify-r41.sh` は 5/5。`sections: 59, part headings: 13, last: 付録`、`current only: version ['v0.3.1'], no spec-map in the live documents or gates`、`codes: 69, corpus tags resolve and match`、`version: v0.3.1`。
- **41-b（文書層）。** `bash scripts/lint-docs.sh` は 7/7。
- **41-c（層の一覧）。** `bash scripts/verify-gate-cache.sh` は 26/26。
- **41-d（層別）。** `--tier quick target/r41-gates/quick` は 7 秒で exit 0（build 1/1、gate-cache 26/26、m1-spec 1/1）。`--tier core target/r41-gates/core` は 345 秒で exit 0（gate-cache 26/26、m0 7/7、m1-corpus 7/7、m1-spec 1/1、m2 8/8、m3 12/12、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r38 138/138）。`--jobs 8 --tier full target/r41-gates/full` は 40 本、471 秒。3 走行とも exit 0 で `^NG:` は 0。
- **41-e（cargo test）。** `cargo test --offline -q --test r9_grammar --test r23_syntax --test parse_corpus` は r9_grammar 5、r23_syntax 9、parse_corpus 1 の全て passed（exit 0）。
- **41-f（門番の参照）。** `docs/spec-map.md` が無く、現行文書と門番に `spec-map` の参照が無い（41-a の検査 2 で実測）。
- **41-g（tag）。** 軽量 tag `v0.3.1` を最終コミットに付け直した。tag `v0.3` は動いていない。

### 迷った点

- **旧版への言及の残し方。** 削除の対象は対応表と、番号を現行に写すための記載とした。`docs/spec.md` の `--`（v0.1 の記法）は、旧版を参照するためではなく `--` を拒否する理由の説明なので残した。README の「`docs/` の文書は常に現行版。過去の版は git tag で読む。」は方針の記述なので残した。
- **plan.md 35・36 節の参照先。** 両節は `docs/spec-map.md` を作る・作り直すと書いており、削除後は参照先が無い。当時の手順の記録なので書き換えず、R41 で外したことをここに書く。
- **保証の置き場。** 本文保存の検査を外すと、再構成が本文を変えていない保証は門番から消える。R39・R40 の実測で確かめた一度きりの性質であり、以後は門番に残さない。
- **tag の付け直し。** 仕様の本文は変わらないが前書きが変わるので、`v0.3.1` を最終コミットに付け直した。版は上げない。

### 仕様への疑義

- 該当なし。言語規則は変えていない。

## R42 節番号を増やしても動かない形にする（2026-09-15）

計画は `docs/plan.md` 38 節。本文と付録を 1 本の通し番号で振っていたため、本文の途中に 1 節増えるだけで corpus のタグ・エラー表の節欄・門番・コメントまで全部ずらすことになっていた（R39・R40 で 2 回行った）。付録を文字の別系列にし、本文の追加は小数番号で行う形にして、以後は既存の番号を動かさない。**言語規則は変えていない。** 本文 §1〜§56 の番号と内容も変えていない。

### 実装

- `docs/spec.md` の付録 3 節を `## 付録A コア構文`・`## 付録B 典型コード`・`## 付録C 文法の概形` にし、目次と、付録を引く本文（E100 の節欄と説明）を追随させた。
- 付録を引く参照を追随させた（`docs/spec-trace.md` 6、`docs/spec-coverage.md` 6、`docs/corpus.md` 2、`docs/wants.md` 1、`crates/` のコメント 6、Rust テスト 2）。
- corpus の `// spec: §58` 11 件を `付録B`、`§59` 1 件を `付録C` にし、`tests/corpus.sha256` の 12 行を更新した。
- 門番を追随させた。`lint-docs.sh` は本文（整数と小数）と付録（A から）を分けて検査し、`heading_exists` は `§N` と `付録X` の両方を引く。`verify-m1-corpus.sh` は典型コードの節を `付録B` にし、タグの正規表現を `§N`・`§N.M`・`付録X` に広げた。`verify-m1-spec.sh` も同じ形にした。`verify-m2.sh` と `verify-r23.sh` のタグとラベル。`verify-r41.sh` を `verify-r42.sh` にし、`scripts/lib/gate-tiers.sh` の full を `r42` にした。
- 保守の規約を `AGENTS.md`（守ること）と `docs/corpus.md`（タグの節）に書いた。本文の新しい節は直前に置きたい節の直後に小数で入れ、整数は本文の末尾にだけ足す。付録は `付録A` から順に足す。

### 判定の実測

- **42-a（構造・参照）。** `bash scripts/verify-r42.sh` は 5/5。`sections: 56 (1..56), appendix: ['付録A', '付録B', '付録C']`、`current only: version ['v0.3.1'], no spec-map in the live documents or gates`、`codes: 69, corpus tags resolve and match`、`version: v0.3.1`。
- **42-b（文書層）。** `bash scripts/lint-docs.sh` は 7/7。
- **42-c（層の一覧）。** `bash scripts/verify-gate-cache.sh` は 26/26。
- **42-d（層別）。** `--tier quick target/r42-gates/quick` は 7 秒で exit 0（build 1/1、gate-cache 26/26、m1-spec 1/1）。`--tier core target/r42-gates/core` は 356 秒で exit 0（gate-cache 26/26、m0 7/7、m1-corpus 7/7、m1-spec 1/1、m2 8/8、m3 12/12、m4 133/133、m5 145/145、m6 71/71、m7 3/3、r38 138/138）。`--jobs 8 --tier full target/r42-gates/full` は 40 本、471 秒で exit 0。3 走行とも `^NG:` は 0。
- **42-e（cargo test）。** `cargo test --offline -q --test r9_grammar --test r23_syntax --test parse_corpus` は r9_grammar 5、r23_syntax 9、parse_corpus 1 の全て passed（exit 0）。
- **42-f（番号の据え置き）。** 本文の整数は 1〜56 のまま。corpus と現行文書の付録参照は `付録A`・`付録B`・`付録C` になった。`grep '§5[789]'`（計画と台帳の記録を除く）は 0 件。
- 変更した門番: `verify-m1-spec.sh` 1/1、`verify-m1-corpus.sh` 7/7、`verify-m2.sh` 8/8、`verify-m3.sh` 12/12、`verify-m7.sh` 3/3、`verify-r23.sh` 3/3、`verify-r34.sh` 1360/1360、`verify-r36.sh` 388/388、`verify-r37.sh` 254/254、`verify-r38.sh` 138/138。

### 迷った点

- **付録の見出しの形。** `## 付録A コア構文` とした（`## A. コア構文` ではない）。本文の `## N.` と見分けが付き、機械参照 `付録A` がそのまま見出しの先頭になる。
- **小数の例。** 規約には `§N.M` と書いた。`§20.1` のような実在しない番号を例にすると、参照の実在を検査する 42-a(5) が赤になる。具体例は計画 38 節と本節に置いた。
- **門番のタグ解析。** 付録を足した時点で、`verify-m1-corpus.sh` の 3 か所と `lint-docs.sh`・`verify-m1-spec.sh` の参照検査が数字だけを前提にしていたことが赤で分かった。`§N`（整数・小数）と `付録X` を受ける形に広げた。
- **本文への挿入。** 挿入は「直前に置きたい節の直後に `§N.1`」で行う。既に `§N.1` があるときは `§N.2` を足す。この範囲の番号だけは動きうるが、他の節には影響しない。

### 仕様への疑義

- 該当なし。言語規則は変えていない。

## R43 標準出力と基本関数（2026-09-16）

計画は `docs/plan.md` 39 節。v0.3.1 には実行中の印字が無く、出力は `main` の返り値 1 回だけだった。stdlib も CLI を 1 本書くのに要る基本関数を欠いていた。`Io` の 5 つ目の操作 `log`（String と改行 1 つを stdout へ）と、`String.join` / `trim`、`Int.abs` / `min` / `max`、`List.head` / `nth` / `take` / `drop` を足した。**prelude に操作を 1 つ足すので言語の意味が広がる。** 計画 16 節 Q5 の「出力関数の綴り」は `log` に決まり、R29 の「実行中の print 効果を足すときは再考する」への回答を §55 に書いた。

### 実装

- `stdlib/io.hd` に `reply log(contents: String): Unit {Error}` と `IoDefault` の `def log` を足した。既定実装は `let status = __io_log contents -> if status == 0 => () | raise { message = io_failure_message status }`。R37 の既定実装の経路（1 引数は `specialize_function` と `call_direct`）がそのまま使えた。
- `runtime/hado_runtime.c` の `hado_io_log` は `fwrite` と `fputc` と `fflush` を 1 つの status に畳み、失敗はすべて 4 にする。`io_status_from_errno` は使わない（EBADF・EPIPE・ENOSPC を同じ `"IoFailure"` にするため）。`print_failed` は触らないので、失敗は捕捉できる値になり、捕捉されなければ `main` の raise 経路が、捕捉されても `main` 末尾の `check_stdout` が終了コードを決める。
- `stdlib/string.hd` に `join`（区切りは要素の間だけ）と `trim`（ASCII の空白 6 種）、`stdlib/int.hd` に `abs`（i64 の最小値はその値自身）と `min` と `max`、`stdlib/list.hd` に `head` と `nth` と `take` と `drop` を足した。リストの 4 つは腕の深さを 1 に保つ交互再帰で、範囲外は `None` か `[]` にする。
- `trim` だけは C の `__string_trim` にした。§5 のエスケープに CR が無く、char のコードを取る intrinsic も無いので、hado のソースだけでは CRLF の CR を除けない。除く 6 種は ASCII で、UTF-8 の継続バイトは 0x80 以上なのでバイト単位でも文字を割らない。
- `crates/hado/src/runtime_abi.rs` の `RuntimeFunction` に `IoLog` と `StringTrim` を足した。enum と `ALL` の末尾に足し、既存の `id()` を動かしていない。`bootstrap`・`mono.rs`・`infer.rs` は触っていない。
- `docs/spec.md` §55 に member の型と意味、失敗の `message` の表の行、`docs/spec-trace.md` に 4 行を足した。**§55 の見出しは増やしていない**（`^## 55\.` を 1 個と数える門番があるため）。

### 判定の実測

- **43-a（仕様と stdlib の一致）。** `bash scripts/verify-r43.sh` の 43-a は OK。§55 の関数集合と member の型が stdlib の 4 ファイルと一致し、`effect Io` に `reply log` がある。
- **43-b（`log` の正常系）。** 生成した 10 形（空・ASCII・`\n`・`\t`・`"`・`\`・マルチバイト・長い）で、stdout が `文字列 + "\n"` と `main` の値の連結に一致、終了 0、stderr は空。
- **43-c（`log` の失敗）。** stdout を閉じた実行・`/dev/full`・読み側を閉じたパイプの 3 つとも終了 1 で stderr は `IoFailure\n`。パイプは 141 でない。`handler Error` で捕まえた場合は `write_file` で書いたファイルが `IoFailure` で、終了コードは `check_stdout` により 1。
- **43-d（`String.trim` / `join`）。** trim 12 形（空白 6 種の全部・全角空白・内部の空白・空・CRLF・マルチバイト隣接）と join 10 形（空リスト・要素 1 つ・空文字列・空の区切り・制御バイト）。期待値は `python3` の `str.strip(" \t\n\v\f\r")` と `str.join` から導いた。
- **43-e（`Int.abs` / `min` / `max`）。** i64 の最小値・最小値+1・-1・0・1・最大値-1・最大値を含む 9 値と 7 対。`abs (-9223372036854775808)` はその値自身、`min` と `max` は大小。i64 の最小値は literal に書けない（E112）ので `Int.parse "-9223372036854775808"` で作る。
- **43-f（`List.head` / `nth` / `take` / `drop`）。** 長さ 0〜4 の Int と String のリスト、count と index の負・0・長さ±1・長さ+2 を決定的に含む形。期待値は `python3` のスライスと添字から導いた。**最初の実装は `nth` が 1 つずれており、生成した探針が `Some(1)` と `None` の食い違いで捕まえた**（`nth_from` が `nth_more` に `index - 1` を渡していた）。
- **43-g（コーパス）。** 新しい member を使う受理コーパスを本文から選び（ファイル名は判定器に埋め込まない）、`spec: §55`・item・`needs: prelude`・`expect` の 1 行・lock の一致・build と実行の期待 stdout を測った。item の集合は `Env と Io` と `String 操作`。
- **43-h（`Io` の 5 つ目の操作）。** 5 つの腕を持つ `handler Io` は受理、`log` の腕を落とすと E605。`verify-r37.sh` は 262/262、`crates/hado/tests/r37_world.rs` の既定実装の照合は 5 操作。
- **43-i（`cargo test`）。** `cargo test --offline --no-fail-fast` は 734 passed、0 failed。
- **43-j（層別と門番の一覧）。** `verify-gate-cache.sh` は 26/26（r43 を足した一覧が実在の門番と一致し、3 層が重複なく覆う）。3 層の全走は下の「層別の実測」を見よ。
- **43-k（エラーコード）。** §56 の表は 69 行、拒否コーパスは 69 本、受理コーパスは 106 本。増えていない。
- 変更した門番の実測: `verify-r43.sh` 12/12、`verify-m1-corpus.sh` 7/7、`verify-m7.sh` 3/3、`verify-r34.sh` 1360/1360、`verify-r36.sh` 388/388、`verify-r37.sh` 262/262。

### 層別の実測

- `bash scripts/run-gates.sh --tier quick target/r43-gates/quick` は 9 秒で exit 0（build 1/1、gate-cache 26/26、m1-spec 1/1）。
- `bash scripts/run-gates.sh --tier core target/r43-gates/core` は 545 秒で exit 0（gate-cache 26/26、m0 7/7、m1-corpus 7/7、m1-spec 1/1、m2 8/8、m3 12/12、m4 135/135、m5 145/145、m6 71/71、m7 3/3、r38 138/138）。
- `bash scripts/run-gates.sh --jobs 8 --tier full target/r43-gates/full` は 41 本の門番と `lint-docs` を 746 秒で exit 0。新しい `verify-r43.sh` は 12/12、`lint-docs` は 7/7、`verify-r42.sh` は 5/5。3 走行とも `^NG:` は 0。

### 迷った点

- **`trim` を C の intrinsic にしたこと。** hado のソースだけでは CR を書けず、`String.get` は `{Error}` なので条件列の条件にも置けない（E308）。`__string_get` を使う手もあったが、バイト単位の除算は結果が文字単位と一致するので、C の 1 関数のほうが短く、空白の集合が 1 箇所に閉じる。
- **`String.get` と条件列。** 実測で `E308: effects are not allowed in branch conditions` を踏んだ。`Pure` でない member は条件列の条件に置けない。stdlib を書くときの一般的な制約として記録する。
- **コーパスの item を増やさなかったこと。** `Int` と `List` の新しい member は全域で拒否形が無い。item を増やすと `verify-m1-corpus.sh` の免除の一覧を広げることになり、門番を緩める変更になる。coverage は `verify-r43.sh` が持ち、受理側は `String 操作` と `Env と Io` の 2 本に寄せた。
- **`verify-r36.sh` の件数の固定。** `prelude inventory: 39/39` を直書きしていた。member を足すと 48 になり、以後も古くなる。AGENTS の「件数を直書きしない」に従い、`n/n` の一致と非零を見る形に変えた（計画 39 節 決定 10）。緩めではなく、固定していた数を §55 由来にした。
- **`verify-r37.sh` の 37-i の対照。** item の札を 1 ファイルから落とす対照は、同じ item のファイルが 2 本までは `expect` と `expect-error` の不在で捕まったが、`log` を足して 3 本になると素通りする。item の札は item の性質なので、全部の札を落として空を拒む対照に変えた。ファイルごとの `needs: prelude` と lock の対照は残っている。
- **`List.nth` のずれ。** 交互再帰の helper に渡す数を 1 つ間違えていた。手書きの probe では気づきにくく、生成した探針が最初に落ちた所である。
- **`verify-r43.sh` の 43-g。** 最初は 2 つのファイル名を埋め込んでいたが、AGENTS の「コーパスのファイル名を判定器へ埋め込まない」に反するので、新しい member を使うファイルを本文から選ぶ形に直した。
- **入れ子の worktree と内容ハッシュ。** リポジトリの下の `.worktrees/rename`（別のブランチの git worktree。`.git/info/exclude` にある）が走っており、その `target/gate-cache` が毎秒書かれていた。`hado_gate_fingerprint` は `target/` と `.git/` しか除かないので、2 回続けた鍵が食い違い、`verify-gate-cache.sh` は 20/26 になった。`./.worktrees/*` を除いて鍵は安定し、26/26 に戻った（計画 39 節 決定 11）。門番の判定は変えていない。
- **走行中の編集。** `verify-m1-corpus.sh` の検査 7 は `docs` `runtime` `scripts` を開始時に固定し、走行中に変わると赤にする。開発の途中で門番を走らせながら別のファイルを編集し、`protected-files-changed-during-verification` の偽の赤を 2 回出した。門番を回すときは木を触らない。

### 仕様への疑義

- **`Int.abs` の i64 最小値。** §5 は Int の演算の wrap を定めていない。反転が表現できないのでその値自身を返すと §55 に明記した。言語の他の演算（`+` や `*`）の桁溢れも同じく §5 が定めておらず、R43 では触っていない。
- **`log` の失敗と §54 の関係。** §54 は `main` の値の印字の失敗を終了コード 1 とする。R29 は「実行中の print 効果を足すときはこの決定を再考する」と申し送った。R43 は `log` の失敗を捕捉できる `Error` とし、終了コードは §54 の 2 経路（捕捉されない raise と `main` 末尾の `check_stdout`）が決めるとした。§54 の本文は変えていない。
- **`trim` の空白の集合。** §55 は定めていない。ASCII の 6 種とし、全角空白は除かないと明記した。
- **`String` の単位と `trim` の実装の単位。** §55 の `String` は文字（Unicode スカラ値）の列だが、`trim` の実装はバイト単位である。除く 6 種が ASCII なので結果は一致するが、実装の単位が仕様の単位と違うことを記録する。
- **`log` が予約名になること。** §37 により、以後の利用者の `def log` と他の効果の `reply log` は E205 になる（実測: 前者は `E205`、後者は `E205: duplicate declaration log`）。既存のコーパスと stdlib に `log` の宣言は無かった。
## R44 効果の操作の名前を once / again に変える（2026-09-16）

### 背景と決定

効果の操作の種別 `reply` / `capture` を `once` / `again` に改名した。`capture` はクロージャが環境を捕まえることを指す語と同じで、`crates/hado/src/{mono,repr,emit}.rs` の `captures` と意味が重なる。操作の意味（呼び出し位置で 1 回返る / 継続をもう一度走らせられる）を名前にする。`abort` と `resume` は動かさない。手順と決定は `docs/plan.md` 40 節。

改名は 3 層に及ぶ。言語の見える面（キーワード、仕様、コーパス、stdlib、診断文）、実装の内部識別子、コーパスのディレクトリ名である。クロージャの `captures` は別概念なので残した。

### 実装

- コミットを 6 つに割った。計画（docs）→ 内部識別子（refactor）→ 仕様（docs）→ 判定器（test、赤の実測）→ 挙動（feat）→ 許容される赤の追記（docs）。識別子の改名を振る舞いの変更から先に切ったので、feat の差分はソースの綴りだけになった。
- 内部識別子を `OpKind::Once` / `OpKind::Again`、`KwOnce` / `KwAgain`、`mono.rs` の `contains_again` `is_again_node` `again_answer_of` `again_do` `again_operation` `again_result_type` `bind_again_outcome` `again_bind_function` `again_bind_yield` `lift_again_arguments` `again_answers` にした。`repr.rs` `emit.rs` の `captures` と `capture offset overflow`、`parser.rs` の "cannot capture a user binding" は動かしていない。
- 字句は `once` / `again` をキーワードにし、`reply` / `capture` を `name` に戻した。**別名は残さない。** 旧綴りは効果の宣言の位置で E100 になり、利用者の名前としては使える。
- 見える面を追随させた。§37 の種別表と例、§38 の `again` の腕、§53、§55、付録A、付録C（キーワード一覧と `op_kind`）、§56 の E124 の短い名前（`resume-outside-again`）、`docs/spec-trace.md` `docs/wants.md` `docs/bug.md`、`docs/plan.md` の現行の記述。**台帳の過去のラウンドは当時の語のまま残した。**
- コーパスの 24 ディレクトリを `git mv` した（`capture_*` → `again_*`、`reply_*` → `once_*`、`nested_captures` → `nested_agains`、`nested_same_capture` → `nested_same_again`）。`tests/corpus.sha256` は改名後のパスで作り直し、215 本になった。
- `scripts/verify-r44.sh` を新設し、`scripts/lib/gate-tiers.sh` の full に `r43` と並べて `r44` を足した。
- **`\n` エスケープの直後のキーワードは `\b` で語境界が取れない。** Rust テストと門番に埋め込んだ hado ソース（`'effect Foo\nreply tick: Int\n'` など）は `\n` が実改行ではなく `\` と `n` なので、`\breply\b` は `n` と `r` の間を境界と見なさない。最初の一括置換はこれを取り逃していた。後続文字で切り直し（`reply([^A-Za-z0-9_]|$)`）、`r23_syntax` `r24_effects` `r25_effects` `r26_effects` `r28_effects` `r31_effects` `r37_world` `r9_grammar` のテストと `verify-r26` `r30` `r31` `r33` `r34` `r37` `r43` を改名した。`capture_output` と `--nocapture` は後続が `_` なので巻き込まない。

### 判定の実測

- **44-a〜44-f（R44 の門番）。** `bash scripts/verify-r44.sh` は 6/6。字句は `once` / `again` の受理、`effect E reply x: Int` の E100、`def reply = 1` と `def capture = 2` の受理、仕様は効果の語としての `reply` / `capture` が 0 件、コーパスは拒否 69 コードと lock 215 本と実行、旧綴りの排除は受理コーパスのうち語を含む 30 本が全滅（語を使わない 74 本は対象外）、生成文法は `["once","abort","again"]`。
- **44-g（全テスト）。** `cargo test --offline --no-fail-fast` は exit 0。
- **44-h（層別）。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full` は 41 本を走らせ、`lint-docs` 以外は全て `^NG:` 0。`verify-r43.sh` 12/12、`verify-r44.sh` 6/6、`verify-r34.sh` 1360/1360、`verify-r35.sh` 714/714、`verify-r37.sh` 262/262、`verify-gate-cache.sh` 26/26。
- **工程 3 の赤。** 判定器を足した直後の `bash scripts/verify-r44.sh` は PASSED 2/6。44-a は新綴りが E100、44-d はコーパスに新綴りが無く対象 0 本、44-e は生成器が旧綴り、44-f は仕様から例とキーワード一覧を読む `r23_syntax` の 2 本が赤だった。

### 経緯

- **R44 の作業中に main の R43 が閉じた**（`fa6fd59` `33a4145`）。R44 を `33a4145` へ rebase し、R43 が足した `tests/accept/io_log` `tests/accept/string_join_trim`、lock の 2 行、`gate-tiers.sh` の `r43` を取り込んだ。
- 競合は 3 つ。`scripts/lib/gate-tiers.sh` は full の並びを `... r42 r43 r44` にした。`scripts/verify-r37.sh` は R43 が足した 5 つ目の `log` の腕と `'log'` の名前を残しつつ `once` にした。`tests/corpus.sha256` は競合を捨てて、rebase 後の木から 215 本で作り直した。
- rebase の途中で `git rebase --continue` がエディタを開き、60 秒で切れた。`GIT_EDITOR` を指定して再開した。

### 迷った点

- **旧綴りを利用者の名前として許すか。** §49 はキーワードが名前にならないことだけを定め、外した語が名前になることを明記していない。外す側を選び、`def reply = 1` を受理にした。既存の `.hd` に `reply` / `capture` を使う名前が 0 件であることは実測した。
- **台帳の扱い。** 計画は現行版なので改名し、台帳は当時の実行記録なので過去の語を残した。同じ文書層でも役割で分けた。
- **診断文の英語。** `resume is only allowed in an again arm` は `an` に直した。`again handler missing answer` など mono の文字列は機械的に置き換えたままで、文法的には不格好である。文言の改善は R45 以降に送る。

### 仕様への疑義

- **旧綴りが名前になることを §49 は明記していない。** 決定 2 は外す側を選んだ。§49 に一文を足すかは R45 以降に送る。
- **`once` / `again` をキーワードにすることの既存コードへの影響は 0 件**（`tests/` `stdlib/` `tools/` の `.hd` を実測）。


## R45 複数の scrutinee とパターンの腕の条件列（2026-09-16）

### 背景と決定

パターン分岐は scrutinee を 1 つしか取れず、腕の中に分岐を置けなかった。2 つの値の構造で分岐した後に先頭同士を比較する `merge` は 3 定義に割れ、`merge_heads` は位置引数を 7 つ運んでいた。`if left, right | [], _ => ...` の scrutinee の列と、腕の本体に `=> if | c => x | y` の条件列だけを置ける形を足した。手順と決定は `docs/plan.md` 41 節。

`=>` を省く形を取らなかった理由（fallback との曖昧さ）と、ネストの上限が「パターンの下に条件列 1 段」で止まることは計画 41 節の背景と決定 2 にある。

### 実装

- コミットは計画（docs）→ 仕様（docs）→ 判定器とコーパス（test、赤の実測 2/8）→ 構文と印字（feat）→ 探針の修正（test）→ 検査（feat）→ 単相化（feat）→ gate-tiers（test）→ 追跡表と wants（docs）→ merge。実装の 3 工程は `.worktrees/r45`（branch `r45-pattern-columns`）で cursor grok 4.6 xhigh が書き、指揮者が審査してコミットした。
- AST は `PatternBranch { scrutinees: Vec<Node>, arms: Vec<(Vec<Pattern>, Node)>, fallback }`。腕の条件列は本体が `CondBranch` である形で、新しい `NodeKind` は無い。列の数が合わない腕は E125（腕の `|`）。**列のカンマは外側の呼び出しの引数のカンマと区別できないので、`=>` が続くまで列に確定しない**（`| A, 3` は fallback `A` と呼び出しの引数 `3` にもなる）。腕の条件列は最初の fallback で終わり、その後の `|` は外側の腕（`record_fallback_end` を呼ばない）。
- 網羅性は Bool と ADT で分かれていた入口を `matrix_coverage` に統一し、行ごとに列の数だけ Cell を並べて `coverage::covers` に渡す。`covers` は元から行列の左からの特殊化だったので、判定の中身は変わっていない。E512 / E503 は行の全列が変数か `_` のときだけ。E520・E507・E508・E515 は 1 列のまま。
- 単相化は列が 2 つ以上のときだけ行列を左から特殊化して `Switch` を入れ子にし、`_` と変数の行を各 constructor の特殊化に持ち込む。1 列は既存の `constructor_switch` を残した（入れ子パターンの failure 継続の形を固定するテストが IR を見ているため）。Bool の列は `Switch` ではなく `Branch`。`_` の行の本体の複製は 2 列 × 3 行で 2、3 列 × 4 行で 3（上限は constructor 数の列数乗）。
- `stdlib/list.hd` の `merge` を 1 定義にし、`merge_right` `merge_heads` を消した。`List.sort` を使う受理コーパスが 0 本だったので `tests/accept/list_sort/` を足した。受理は `columns_merge` `arm_conditions` と合わせて 3 本、拒否は `E125` の 1 本。lock は 219 本。
- 版を `v0.4` に上げ、軽量 tag `v0.4` を最終コミットに打った（`docs/spec.md` `docs/spec-coverage.md` `docs/spec-trace.md` と `scripts/verify-r42.sh` の版の固定）。言語意味が増えたので小数第 2 位ではなく第 1 位を上げた。`v0.3.1` は動かさない。
- `scripts/verify-r45.sh` を新設し、full に `r45` を足した。45-a は判定器が同じ行列で網羅性を計算し、45-b と 45-f は Python の並べ替えと照合する。

### 判定の実測

- **45-a〜45-f（R45 の門番）。** `bash scripts/verify-r45.sh` は 8/8。45-a は seed 45 で受理 16、E402 16、E403 4 の 36 本、45-b は乱数の 12 組の merge の実行と E401 のアンカー（腕の `if`）、1 本の条件の受理、3 つの入れ子の E101、45-c は E125 の両方向（腕の `|`）と E512 / E503 の行規則、45-d は `| None | Some(v) => ...` の E404、45-f は 12 本のリストの昇順と降順の `List.sort`。
- **45-g（全テスト）。** `cargo test --offline --no-fail-fast` は 63 target 全て 0 failed（作業ツリーで実測）。
- **45-h（層別）。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full` は 41 本を走らせ、39 本が `^NG:` 0。`verify-r45.sh` 8/8、`verify-r34.sh` 1360/1360、`verify-r35.sh` 714/714、`verify-r36.sh` 388/388、`verify-r37.sh` 262/262、`verify-m3.sh` 12/12。赤は 2 本。`lint-docs` 6/7 は台帳の R45 節が未記入のため（この節を書いた後に単独で緑）。`verify-r42.sh` 4/5 は §56 の行数を `69` と直書きしていたためで（E125 で 70 行）、拒否コーパスのディレクトリ数から導く形に直して単独で 5/5。
- **工程 2 の赤。** 判定器を足した直後の `bash scripts/verify-r45.sh` は PASSED 2/8 で、赤は全て `if` の後のカンマが E100 になるため。工程 3 の直後は 4/8（E125、文法生成器、45-d、lock）で `cargo test` は corpus と prelude の 2 target が多列未実装で赤、工程 4 の直後は 6/8 で同じ 2 target、工程 5 で 8/8 と 0 failed。
- **R45 の最終全走（R46 の着手時）。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r45final` は 43 本を走らせ、`^NG:` 0、exit 0。前節で赤だった `lint-docs`（7/7）と `verify-r42.sh`（5/5）も緑で、R45 の終了時に許容される赤は無い。

### 経緯

- **cursor grok の headless 実行は使えなかった。** `cursor-agent -p` は xhigh も high も、ファイルを 1 つ読む探針から `RetriableError: [resource_exhausted]` で出力 0 バイト（一語の応答だけの探針は通る。gpt-5.3-codex の headless は通る）。同じ xhigh を Herdr の対話セッション（`herdr agent start ... -- --model cursor-grok-4.6-xhigh --force`）で起動し、依頼書の絶対パスを `agent prompt` で渡すと、3 工程とも完走した（構文 約 30 分、検査 約 25 分、単相化 約 30 分）。
- 探針の欠陥が 2 つあった。45-a は使わない型まで宣言して E513 が先に当たり、45-d は fallback の `None` が戻り型 `Int` と合わず E305（型）が E404（網羅性）より先に当たった。どちらも工程 3 の報告で判明し、探針を直した。
- `git merge -F -` は標準入力を読めない（`could not read file '-'`）。merge が失敗したまま門番を起動してしまい、止めてから merge をやり直した。止めた走行の probe は木に残っていなかった。

### 迷った点

- **Bool の E402 / E403 の文言。** 1 列の Bool 専用の文言（`Boolean patterns do not cover both values`）を ADT と同じ `not all constructor values are covered` に揃えた。コードだけを見るテストは赤にならなかった。
- **末尾呼び出しの探針の条件。** `n == 0` は腕の条件列が E511（リテラル比較の列）になるので `n <= 0` にした。

### 仕様への疑義

- **入れ子パターンと列をまたぐ同名束縛。** `Pair(x, x)` は今も受理される（実測）。`| x, x =>` も同じ。§26 に規則が無く、R46 以降に送る。
- **fallback を持たない腕の条件列が続く腕を飲む形。** `| Some(x) => if | x > 0 => x | None => 0` は `None` を条件として読み E305 になる。§26.1 に書いた。専用の診断は頻発してから。

## R46 正準形の診断と `check --fix`（2026-09-16）

### 背景と決定

R45 の最終全走が緑で閉じたので、次は診断と fmt（言語意味を増やさない）に移った。Hado は冗長な書き方を拒否するため、診断が「どう書けばよいか」を言わないと日々の編集が摩擦になる。正規形印字は既にあるので、正準形を診断に載せ、局所的な書き換えを機械で当てる材料は揃っていた。全 AST の再印字 formatter を先に作るとコメントを失うので、まわりの木を変えずに書ける書き換えから始めた。手順と決定は `docs/plan.md` 42 節、仕様は §57。

### 実装

- `crates/hado/src/fixes.rs` を新設した。`Fix { code, line, col, start, end, replacement, canonical }` を AST から導く。`Span` は行・桁しか持たないので、ソースの行頭表と `char_indices` でバイト位置へ直す。桁は文字数である（§43）。
- 対象は E510（式の `Nil` / `Cons`）、E516（パターンの `Nil` / `Cons`）、E517（`=> if | ...` の条件列）。E517 の site は `ast.long_condition_functions` の `def` の位置から、括弧の深さ 0 の最初の `=>` を字句で探す。E510 / E516 は `Call` / `Apply` / `Constructor` の `Cons` と `Name` の `Nil` を木の走査で探す。
- 書き換えの範囲に `//` を含む `Fix` は出さない。重なる範囲は最も外側だけを残すので、`Cons(1, Cons(2, Nil))` は 1 つの `[1, 2]` になる。字句の長さが要る綴り（`=>`、`if`、`)`）だけを表で持つ。
- `hado check [--print-type] [--fix] <file>` にした。`--fix` は報告されたアンカーと一致する書き換えを 1 つだけ当てて再検査する（上限 16 回）。`Project::canonical_hint` と `Project::apply_canonical_fix` が、診断の path から物理ファイルを引く。既定の `check` / `build` / `dump-mono` はソースを書き換えない。診断の末尾に `canonical form: <正準形>` を足す（E510 / E516 / E517）。
- `source_print` の `item` と `list_pattern` を crate 内へ公開し、正準形の文字列を既存の印字から作る（E517 は `def` の印字、E510 / E516 は List の印字）。

### 判定の実測

- **46-a〜46-f（R46 の門番）。** `bash scripts/verify-r46.sh` は 7/7。46-a は seed 46 で 10 本の `=> if |` を書き換えて実行し、コメントの保存と 2 回目の冪等を測る。46-b は 10 本の `Cons` の連鎖、46-c は 10 本の `Cons` パターン。46-d は拒否される 3 本で `check` と `build` の前後で sha256 が変わらない。46-e は E501 で終了コード 1 とファイル不変。46-f は §57 の文言、`wants.md` の参照、full の層、corpus lock。
- **Rust 単体テスト。** `cargo test --offline -q --test r46_fixes` は 4 passed。`fixes` の範囲と置換、コメントを含む範囲を出さないこと、日本語コメント越しのバイト位置、`fix_at` の一致を測る。
- **工程 3 の赤。** 判定器を足した直後は `written_list_forms_become_literals` が「連鎖の各綴りに編集が出る」と誤って期待して赤（重なりの除去で 1 本に畳まれる）。期待を直して緑。
- **R46 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r46` は 44 本を走らせ、`^NG:` 0、exit 0。`verify-r46.sh` は 7/7、`lint-docs` は §57 を足しても 7/7、`verify-r42.sh` は 5/5、`verify-m1-corpus.sh` は 7/7。`cargo test --offline --no-fail-fast` は 63 target 全て 0 failed、exit 0。

### 迷った点

- **`--fix` の適用単位。** ファイル内の全 `Fix` を一括で当てず、報告されたアンカーと一致する 1 つだけを当てる。検査器は最初の診断しか返さないので、当てた書き換えが別の診断を呼ぶことがあり、1 つずつなら原因が追いやすい。上限 16 回。
- **`Cons` の連鎖に `//` があるとき。** 外側の `Fix` は出さないが内側は出る。今は報告されたアンカーが外側なのでファイルは変わらない。部分適用を許すかは頻発してから決める（計画 42 節の疑義）。
- **E517 の空白。** `=>` の前と `if` の後の空白とタブも範囲に含め、`def classify score` の行末に空白が残らないようにした。改行は消さない。

### 仕様への疑義

- **診断の `canonical form:` は仕様の言語意味ではない。** §57 は CLI の振る舞いとして書いた。`fmt` の書き換えの単位（一括か 1 つずつか）は §57 では決めていない。
- **`fmt` はまだ無い。** §57 の `--fix` は局所的な書き換えだけで、レイアウトや全体の正規化はしない。`wants.md` の CLI と編集ループの 11 に残した。

## R47 正準形の修正を E515 と E501 へ広げる（2026-09-16）

### 背景と決定

R46 の `check --fix` は E510 / E516 / E517 を直す。残る日常的な正準形の拒否は、§31 の `def f x => if x | ...`（E515）と §41 の `let x = v -> x`（E501）だった。R46 の `Fix` の一覧と適用器をそのまま使い、規則を 2 つ足した。手順と決定は `docs/plan.md` 43 節、仕様は §57。

### 実装

- `check_sole_parameter_branch` の構造判定を `crate::check::sole_parameter_branch` に切り出し、検査器と修正器が同じ述語を使う。条件がずれて「検査器が出さない形を書き換える」ことがない。
- E515 の範囲は「引数の直前の空白から scrutinee の終わりまで」で、置換は空文字列。型注釈の無い引数は括弧を取らない（`def f (x) =>` は E100）ので、始まりは引数の名前である。`fn` の形は範囲が違うので対象外にした。
- E515 の正準形は `source_print::arm_function`。`def` を写して引数名を `$argument` にし、既存の `item` に印字させる。`rec` はそのまま残り、`def rec f | ...` になる。
- E501 は `NodeKind::Let` で `pat` と本体が同じ名前のときだけ。範囲は `let` から本体の名前の終わりまで、置換は `=` の直後から本体の直前の `->` までのソース。分離点を本体の直前の `->` にしたので、値の式の入れ子の `let` の `->` を分離点と取り違えない（`let x = let y = 1 -> y -> x` は外側から順に `let y = 1 -> y`、`1` になる）。`LetQ` と `do` は対象外。
- コメントを消さない規則は R46 と同じで、E515 / E501 の範囲にも当てる。

### 判定の実測

- **47-a〜47-e（R47 の門番）。** `bash scripts/verify-r47.sh` は 7/7。47-a は seed 47 で 10 本の 1 引数関数（`rec` とコメントを混ぜる）を書き換えて実行し、コメントの保存と 2 回目の冪等を測る。47-b は 10 本の同一束縛（リテラルと関数適用）、47-c は 8 本の入れ子の `let`。47-d は範囲に `//` を含む 2 本が変わらないこと。47-e は §57 の E515 / E501、`wants.md` の R47、full の層、`r47_fixes` の 5 テスト、corpus lock。
- **R47 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r47` は 45 本を走らせ、`^NG:` 0、exit 0。`verify-r46.sh` と `verify-r47.sh` はどちらも 7/7、`lint-docs` 7/7、`verify-m1-corpus.sh` 7/7。`cargo test --offline --no-fail-fast` は 64 target 全て 0 failed、exit 0。
- **46-e の差し替え。** R46 の 46-e は「直せない診断」の例に E501 を使っていた。R47 で E501 が直せるようになったため、正準形を持ちまだ直せない E507（`if slot | Some(x) => Some(x) | None => None`）に差し替えた。判定の意味（終了コード 1 とファイル不変）は変えていない。

### 迷った点

- **括弧付きの引数。** 計画では `def f (x) =>` も範囲に入れるとしていたが、型注釈の無い括弧付き引数は E100 で存在しないと実測で分かった（`def f (x) =>` は `expected :, found )`）。分岐を消し、計画と仕様を直した。
- **`fn` を直さない。** `fn(| ...)` は範囲が `def` と違う。E515 は `fn` にも出るが、R47 では `def` だけを直し、`fn` は診断のまま残す。
- **E501 の正準形はソースの写し。** 値の式をそのまま移すので、値の外側の空白は落ちる。印字ではないのでコメントも（範囲の外なら）そのまま残る。

### 仕様への疑義

- **`fn` の E515 と E517。** `fn(| ...)` の書き換えの範囲を §57 は決めていない。`fn` を入れるときに決める。
- **値の外側の空白。** E501 の置換は `=` と `->` の間の空白を落とす。レイアウトの正準形は `fmt` の範囲である。

## R48 `hado run` と安全な出力先（2026-09-16）

### 背景と決定

R46・R47 で「書いて、直す」が一巡したので、次は「実行する」を閉じた。CLI は `check` / `dump-mono` / `build` だけで、`build` は入力の隣に実行ファイルを書く。書き込みできない根や、入力を汚したくない場面で使えない。言語側の材料（`log`、`args`、`read_stdin`、`raise` の既定実装）は既にあったので、足したのは CLI の縦切りである。手順と決定は `docs/plan.md` 44 節、仕様は §58。

- `hado run <file.hd> [-- <arg>...]` は一時ディレクトリに実行ファイルを作り、標準入力・標準出力・標準エラーと現在の作業ディレクトリをそのまま渡して実行し、子プロセスの終了コードを返す。実行の後にディレクトリごと消すので、入力の隣には何も残らない。
- `hado build [-o <path>] <file.hd>` を足した。既定の出力先は入力の拡張子を除いた隣のままで、`-o` のときだけそのパスへ書く。
- `Project::build` の失敗を `report` に通した。`compile_to_object` が `error.to_string()` に落としていたので、`build` と `run` だけ `canonical form:` が出ていなかった（実測: `def main = let x = 1 -> x` の `build` は E501 の本文だけを出していた）。これで `check` と同じ綴りになる。
- 子プロセスがシグナルで終わったときは終了コードを 1 とし、`run: the program was terminated by a signal` を出す。子プロセスの exit code が無いので選んだ。

### 実装

`crates/hado/src/main.rs` は命令ごとの解析を 1 つのループにまとめ、`run` の `--` の後を子プロセスの引数として集める。`crates/hado/src/project.rs` の `build` は `is_library` と `check` の診断を `report` で綴る。`RunDirectory` が一時ディレクトリの作成と Drop での削除を持つ。

### 判定の実測

- **48-a〜48-e（R48 の門番）。** `bash scripts/verify-r48.sh` は 8/8。48-a は seed 48 で 6 つの `main` の値を出し、`TMPDIR` を判定器の下に向けて一時ディレクトリが空になることと入力の隣が `main.hd` だけであることを測る。48-b は 6 つの引数リスト（空白入りと `-n` と非 ASCII を混ぜる）が `args` にその数だけ見えること、48-c は 4 つの `raise` の `message` と終了コード 1、48-d は 4 つの標準入力の往復、48-e は `build -o` が要求したパスにだけ書き既定の `build` が入力の隣に書くこと。48-e は §58、`wants.md` の R48、full の層、`run_cli` の 6 テスト、corpus lock も測る。
- **R48 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r48` は 46 本を走らせ、`^NG:` 0、exit 0。`verify-r48.sh` は 8/8、`verify-r46.sh` と `verify-r47.sh` はどちらも 7/7、`lint-docs` 7/7、`verify-m1-corpus.sh` 7/7。`cargo test --offline --no-fail-fast` は 65 target 全て 0 failed、exit 0。

### 迷った点

- **`--` を必須にするか。** `run main.hd a b` と書けた方が短いが、`--flag` のような入力と紛れる。`--` の後だけを子プロセスの引数にし、`--` の前の旗は使用法の誤り（終了コード 2）にした。
- **一時ディレクトリを残すか。** 失敗の調査には残した方が便利だが、`run` の目的は「入力の隣を汚さない実行」なので毎回消す。必要になったら `--keep` を足す。
- **`build` の既定の出力先。** 一時領域へ変える案はあるが、既存の門番と `build` の利用（入力の隣の実行ファイルを直接叩く）を変えるので、R48 では既定を動かさず `-o` を足すだけにした。

### 仕様への疑義

- **任意の終了コードを言語から返せない。** `raise` は既定実装が 1 を返す。`exit(code)` を足すかは、終了コードを分ける実プログラムが出てから決める（`wants.md` の 25）。
- **`run` の引数の綴り。** `--` が無いときを「引数無し」とした。`cargo run` に合わせただけなので、仕様が決めるほどの理由は無い。
- **`build` の既定を一時領域にするか。** コーパスを汚さない利点はあるが、`build` の成果物を人が使う位置が変わる。`-o` を足した R48 では決めない。

## R49 効果行付き関数値の build 失敗を直す（2026-09-16）

### 背景と決定

`docs/bug.md` に 3 件残っていた既知の不具合のうち、「効果行付きの関数値を返す member の build が失敗する」（R33、裁定 #9）を直した。`check` は通り、`build` と `run` は `specialization failed: Mismatch` で落ちる。R48 で `hado run` を入れたので、この穴は「書いて、直して、走らせる」の途中で踏むようになっていた。手順と決定は `docs/plan.md` 45 節。

原因は単相化の型表現にあった。単相化の `Type` は効果行を持たない（`Ty::Fn(args, result, _) → Type::Closure(args, result)` で行を落とす）。逆に `Type::Closure` を `Ty` に戻す `From<&Type> for Ty` は `Ty::fn_of` を使い、行を `Row::pure()` にしていた。`Signature::specialize_result` と呼び出し先を特殊化する `specialize_function` は、呼び出し側の `Type` から作った `Ty` を定義の scheme と単一化するので、scheme の `Fn(String => String {Error})` と `Type` 由来の pure な `Fn` がぶつかっていた。

- `ty_with_fresh_rows` を足し、`Type` から `Ty` へ戻すときの閉包の行を `Row::pure()` ではなく新しい行変数にした。行を決めるのは定義の scheme であり、行を持たない `Type` ではない。
- 差し替えたのは scheme と単一化する 4 箇所: `Signature::specialize_result`、`specialize_function` の concrete、`constructor_fields`、record の field 取得。`construct` は `Ty` をそのまま `Type` に戻すだけなので `Ty::from` のままにした。
- 仕様は変えていない。効果は型に現れるという §20 の規則は既にあり、単相化がそれに追いついていなかった。

最小再現は次の形である（`check` は 0、`build` は 1）。ラムダ式を直接渡す形（`apply (fn(t => ...)) "7"`）は落ちない。型が推論の中で閉じ、`Type` へ往復しないためである。`def` の宣言型や `def` が返す関数値を経由したときだけ落ちていた。

```ocaml
def apply f x => f x
def make(s: String): Fn(String => String {Error}) Pure => fn(t => String.concat s (Int.to_string (Int.parse t)))
def main = apply (make "x") "7"
```

`docs/bug.md` から該当節を消した（同文書の「直したら節を消し、経緯は git 履歴に残す」に従う）。節と再現の 1 対 1 は `verify-r38.sh` が数えるので、節を消しても数は合う。

### 実装

`crates/hado/src/mono.rs` に `ty_with_fresh_rows` を足し、`From<&Type> for Ty` の閉包が `Row::pure()` を作る経路を、scheme と単一化する 4 箇所で置き換えた。`crates/hado/tests/build_cli.rs` に、効果行付きの関数値を `def` から受け取って多相関数へ渡す回帰テストを足した。

### 判定の実測

- **49-a〜49-d（R49 の門番）。** `bash scripts/verify-r49.sh` は 6/6。49-a は seed 49 で 6 本（前置・後置の文字列、`Int.parse` の有無、`let` 経由と直接）を生成し、`check` が緑・`build` が緑・実行の出力が判定器の計算と一致することを測る。49-b は `docs/bug.md` に該当節が無いことと `build_cli` の 6 テスト。49-c は効果行の無い関数値（多相関数へ直接渡す形・ラムダ式）と、効果行付き関数値をフィールドに持つ record の 3 形。49-d は full の層の `r49` と corpus lock。control は `ty_with_fresh_rows` を `Ty::from` に落とす分岐を環境変数で入れて測り、49-a と 49-b（`build_cli`）が赤になることを確かめた。
- **直した節の再現。** 最小再現は `check` が 0 のまま `build` が 0 になり、`run` が `x7` を印字して終了 0。`build_cli` の回帰テストは 6 passed。
- **R49 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r49` は 47 本を走らせ、47 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。`verify-r49.sh` は 6/6、`docs/bug.md` の節と再現の 1 対 1 を数える `verify-r38.sh` も緑。`lint-docs` は 7/7。`cargo test --offline --no-fail-fast` は 65 target 全て 0 failed（773 passed）、exit 0。

### 迷った点

- **`Type` に効果行を持たせるか。** 単相化の型表現に行を戻す案は、codegen が行を使わないので広い変更になる。行は「定義の scheme が決める」と決め、`Type` から `Ty` へ戻すときだけ新しい行変数を渡した。
- **受理コーパスへ足すか。** 実装の不具合なので `tests/accept` ではなく CLI の判定と Rust テストに置いた。受理コーパスは仕様の適合を測る場所である。
- **判定 49-c を 3 形まとめて 1 本にするか。** 効果行の無い関数値と record のフィールドを 1 本にまとめた。数が増えたら分ける。

### 仕様への疑義

- **効果行を持つ関数値がパラメータ型の中に現れる場合。** `Fn(Fn(A => B {Error}) => C)` の単相化は同じ経路で直る見込みだが、R49 の判定には入れていない。動く形が出たら足す。
- **`Type` が行を持たない設計。** 行を持つようにするかは、効果が codegen に効くようになるときに決める。

## R50 スタックの枯渇を診断にする（2026-09-16）

### 背景と決定

`docs/bug.md` の 3 件のうち、R49 で効果行付き関数値の build 失敗を閉じた。残る「check は通るが実行できない」は `again` の深いループの SIGSEGV である。§53 は `again` を含むループがスタックを反復回数に比例して消費すると決めているが、尽きたときの観測される振る舞いを決めていなかった。R50 はそこを診断へ閉じた。手順と決定は `docs/plan.md` 46 節、仕様は §53.1。

現状の実測（`tests/accept/again_loop/main.hd` の形、`ulimit -s` は 8192 KB）: 反復 100,000 と 200,000 は完走、400,000 と 1,000,000 は SIGSEGV（終了 139、メッセージ無し）。1 反復あたり約 20〜40 バイトを消費している。`docs/plan.md` 17.8 の「約 3 万反復」は Koka 3.2.3 の実測で、Hado の現在の実装はもっと遅く尽きる。**尽きるまでの回数は仕様に書かない**（実装依存の定数になるため）。

- `runtime/hado_runtime.c` にスタックの番人を足した。起動時に `getrlimit(RLIMIT_STACK)` と `main` の局所変数のアドレスからスタックの底を概算する。
- 代替シグナルスタック（64 KB）の上に SIGSEGV と SIGBUS のハンドラを置く。枯渇した状態では通常のスタックにハンドラを積めないためである。
- フォールトしたアドレスが底の前後 1 MB に入るときだけ枯渇とみなし、`hado: stack overflow` の 1 行を stderr に出して終了コード 1 で終わる。終了コード 1 は §54 の二値のうち 1 で、`raise` や `print failed` との区別は stderr の内容がつける。
- 底から遠いフォールトは先のハンドラ（Boehm GC など）へそのまま渡し、無ければ既定の動作に戻して raise し直す。診断を足すことで本物の不具合が見えなくなるのを避ける。
- stdout は flush しない。フォールトが stdio の錠を握った中で起きた場合、`fflush` が同じ錠を待って止まる。バッファに残った分は失われることを §53.1 に書いた。
- 言語の `Error` にはしていない。`raise` と同じ経路に載せるには `$Outcome` の巻き戻しが要り、生成コードの呼び出し規約に踏み込む。`handler` で捕まえられない診断までを R50 の範囲とした（`wants.md` の 35 に残した）。

### 実装

`runtime/hado_runtime.c` の `main` が `GC_INIT()` の後に `install_stack_guard()` を呼ぶ。番人は起動時の 1 回だけで、ループの中には何も足していない。`_XOPEN_SOURCE` を明示し、`-std=c11 -Wall -Werror` で runtime を読み込む `r37_world` の探針からも読めるようにした（最初の版は `siginfo_t` と `SA_ONSTACK` が c11 で見えず、`r37_world` の 1 テストが赤になった）。

### 判定の実測

- **50-a〜50-e（R50 の門番）。** `bash scripts/verify-r50.sh` は 6/6。50-a は反復 300,000 の生成プログラムを `ulimit -s 1024`（1 MB）で走らせ、終了コード 1・`hado: stack overflow`・空の stdout を測る。50-b は同じ実行ファイルを 64 MB で走らせて完走（`300000` を印字、終了 0）。50-c は浅いループ（1,000）を 1 MB で完走。50-e は runtime を読み込んだ C の探針にスタックの底から遠いフォールト（`raise(SIGSEGV)`）を起こさせ、メッセージを出さずシグナル死のままであることを測る。50-d は §53.1 と追跡表と full の層の `r50` と corpus lock。control は終了コード・stdout・stderr の期待値を 1 つずつずらして落ちることを確かめる。
- **実装前の赤。** 50-a は終了コード -11（SIGSEGV）、stderr が空で `PASSED: 4/5`。
- **既定スタックでの再現。** 反復 400,000 の実行ファイルを `ulimit -s 8192` で走らせると、実装後は `hado: stack overflow` を出して終了 1。
- **通常の実行。** `def main = 42` は `42` と終了 0、`1 / 0` は `DivisionByZero` と終了 1。
- **C の探針。** 50-e の探針を素で走らせると終了 139 で stderr は空のまま（枯渇でないフォールトを握り潰していない）。

- **R50 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r50` は 48 本を走らせ、48 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。`verify-r50.sh` は 6/6、`lint-docs` は 7/7。`cargo test --offline --no-fail-fast` は 65 target 全て 0 failed（773 passed）、exit 0。

### 迷った点

- **反復回数の上限を仕様に書くか。** `wants.md` の 17 は「上限を仕様に書く」としていたが、実装依存の定数なので書かないと決めた。実測も Koka 3.2.3 の約 3 万ではなく Hado は約 30 万〜40 万で、実装を変えれば動く。
- **枯渇を言語の `Error` にするか。** `handler` で捕まえられる形にするには生成コードの巻き戻しに踏み込むので、R50 はメッセージと終了コードの診断までとした。
- **枯渇でないフォールトの扱い。** 握り潰すと本物の不具合が見えなくなるので、先のハンドラへ渡して既定の動作に戻す。50-e が C の探針で固定する。
- **診断で `fflush(stdout)` をするか。** stdio の錠を握った中で落ちると待ち続けるのでしない。バッファに残った stdout は失われる。
- **`RLIMIT_STACK` が無限のとき。** 8 MB を仮定するので、`ulimit -s unlimited` の下では底の推定が実際より上になり、枯渇を診断できないことがある。既定の有限の上限では効く。無限の環境を測る必要が出たら直す。

### 仕様への疑義

- **枯渇の終了コードを `raise` と同じ 1 にするか。** §54 の二値（0 と 1）と「区別は stderr の内容がつける」に従って 1 にした。区別する値が要る実プログラムが出たら決める（`wants.md` の 25）。
- **stdout のバッファを失うこと。** §53.1 には書いたが、これはランタイムの実装から来る観測である。`handler` で捕まえられる `{Error}` にするとき、印字の扱いと一緒に決める。

## R51 実用 CLI を門番の走行に組み込む（2026-09-16）

### 背景と決定

`tools/gates/main.hd` は門番のログを読んで集計する Hado の CLI で、`report/` のライブラリと合わせて動いていた。しかし `scripts/run-gates.sh` からは呼ばれておらず、判定器の素材として使われているだけだった（`run-gates.sh` の 232 行のコメントは「ログは tools/gates/main.hd の集計に混ざるので末尾を `PASSED: n/N` に揃える」と、組み込まれる前提を既に書いていた）。目標の優先順位 2 は「自分用の小さな CLI を Hado で常用し、その不足だけを次に足す」である。R51 はこの 1 本を門番の走行に組み込み、常用を始めた。手順は `docs/plan.md` 47 節。

生成したログのディレクトリで道具の振る舞いを先に実測した。`read_dir` は名前を UTF-8 のバイト順で返すので、赤の一覧もその順になる。

| ディレクトリ | 出力 | 終了 |
|---|---|---|
| 空 | `0/0 passed; red: none` | 0 |
| `a.log` が `PASSED: 3/3` | `3/3 passed; red: none` | 0 |
| `a.log` が `1/3`、`b.log` が `2/2` | `3/5 passed; red: a.log` | 0 |
| `PASSED` 行の無い `.log` | stderr が `InvalidLine` | 1 |
| `PASSED:` の無い `notes.tsv` | stderr が `InvalidLine` | 1 |
| `PASSED: 1/x` | stderr が `InvalidInt` | 1 |
| 引数無し | stderr が `MissingDirectory` | 1 |
| 無いディレクトリ | stderr が `NotFound` | 1 |

入口の契約は**「ディレクトリの全ての項目が `PASSED: n/N` の行を持つ」**で、緩めない。走行のログは走行自身の成果物で、Hado が読めないなら走行の欠陥だからである。

- `scripts/run-gates.sh` の最後（`lint-docs` の後）に集計を出す。数え方は Hado 側に 1 つだけ置き、bash で数え直さない。
- 集計は `TALLY: <出力>` として標準出力に出す。出力先ディレクトリには書かない。自分自身を読むディレクトリなので、自分のログを混ぜると入口の契約を破る。
- 集計の失敗は走行を赤にする。既に赤の走行では赤のままなので、増える赤は「ログが不完全」という情報になる。
- 層と選択に関わらず走らせる。`hado` は `${CARGO_TARGET_DIR:-target}/debug/hado` を使い、PATH を探さない。バイナリか `tools/gates/main.hd` の無い走行（判定器が作る合成木）では黙って飛ばす。成功した走行が stderr に何も書かないことは `verify-r38.sh` と `verify-gate-cache.sh` が固定している契約で、`TALLY:` の行が無いこと自体が印になる。
- 仕様は変えない。`tools/gates` はリポジトリの道具で、言語仕様の外にある。

### 実装

`scripts/run-gates.sh` の末尾に集計の呼び出しを足した。`tools/gates/main.hd` は無変更で、判定器も工具も既存のものがそのまま使えた。`hado run` が入力の隣に何も書かないこと（R48 の一時ディレクトリ）が効いていて、出力先のログを読むだけの道具を走行の中から安全に呼べる。

### 判定の実測

- **51-a〜51-d（R51 の門番）。** `bash scripts/verify-r51.sh` は 5/5。51-a は本数 0〜5・`passed <= total`・緑と赤を混ぜたログのディレクトリを 6 つ生成し、集計が判定器の計算した合計と赤の一覧に一致すること、集計がディレクトリに何も書かないことを測る。51-b は入口の契約の 4 形。51-c は合成木で `run-gates.sh --jobs 1 --tier full` を走らせ、緑と赤の門番で `TALLY: 1/2 passed; red: verify-stub-b.log` と走行の赤、`PASSED:` を書かない門番で `TALLY: failed` と `InvalidLine` を測る。51-d は層と文書と corpus lock。control は誤った集計を受け取らないことを先に測る。
- **組み込み前の赤。** `PASSED: 3/5`。51-c と 51-d が NG で、工具だけを測る 51-a と 51-b は赤くならない。
- **実走の煙。** `bash scripts/run-gates.sh --jobs 1 target/gates-r51-smoke verify-r51` は `TALLY: 5/5 passed; red: none` を出して終了 0。

- **R51 の全走で見つかった赤。** 最初の実装は、集計を飛ばす理由を `TALLY: skipped (...)` として stderr に出していた。全走で `verify-r38.sh`（`PASSED: 137/138`）と `verify-gate-cache.sh`（`PASSED: 25/26`）が赤くなった。どちらも合成木を走らせて**成功した走行の stderr が空であること**を固定しており、そこへこの 1 行が混ざっていた。門番を緩めず、こちらの決定を変えて黙って飛ばすようにした（`docs/plan.md` 47 節の決定 4 を直した）。直した後の 3 本は `verify-r38.sh` が `PASSED: 138/138`、`verify-gate-cache.sh` が `PASSED: 26/26`、`verify-r51.sh` が `PASSED: 5/5`。
- **R51 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r51` は 49 本を走らせ、49 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。最後の行は集計自身が出した `TALLY: 3574/3574 passed; red: none`。`verify-m0.sh` が全走テストを含むので、処理系の全走テストもこの走行の中で緑である。

### 迷った点

- **集計の失敗を赤にするか。** 赤にした。ログは走行自身の成果物で、Hado が読めないのは走行の欠陥である。既に赤の走行では赤のままなので、増える赤は「ログが不完全」という情報になる。赤の理由が集計の失敗で上書きされて読めなくなる危険は、`TALLY: failed (...)` を stderr に出し、門番のログはそのまま残ることで避けている。
- **飛ばすときに何か出すか。** 最初は出したが 2 本の門番が赤くなったので黙った（上の赤の記録）。`TALLY:` の行が無いことが印になる。
- **非ログの項目を読み飛ばすか。** 読み飛ばさない。入口の契約を緩めると、`PASSED:` を書かずに落ちた門番のログが黙って数から漏れる。`HADO_GATE_TIMES` を出力先の中に向けると契約を破るので、記録先は走行のログの外に置く。
- **前の走行のログが残った出力先。** `run-gates.sh` は出力先を消さないので、同じディレクトリに部分走を重ねると古いログも数える。消すかは、部分走の使い方が固まってから決める。

### 仕様への疑義

- 無し。`tools/gates` はリポジトリの道具で、言語仕様の外にある。R51 は仕様本文もエラーコードも変えていない。

## R52 `hado fmt` と `build --fix`（2026-09-16）

### 背景と決定

R46・R47 が §57 の局所的な書き換え（E501 / E510 / E515 / E516 / E517）と `check --fix` を作った。`fixes::fixes(source)` はソース 1 つ分の書き換えを全部返すが、`check --fix` は報告された診断のアンカーに一致する 1 つだけを使う。計画 42 節は「`fmt` を入れるときに、書き換えを一括で当てるか 1 つずつかをもう一度決める」と判断を残し、`wants.md` の 11 は `fmt` と `build --fix` を「残り」としていた。R52 は一括を選び、`hado fmt [--check]` と `build --fix` を入れた。手順は `docs/plan.md` 48 節。

先に材料を実測した。書き換えを持つ拒否コーパスは 5 件（E501 / E510 / E515 / E516 / E517 が 1 件ずつ）で、5 件とも `check --fix` の後に `check` が終了 0 になる（受理へ変わる）。

- `hado fmt [--check] <file.hd>` は入力根のソース（§50）の書き換えを診断を待たずに一括で当てる。既定は書き込み、`fmt: <相対パス> (<n> rewrites)` を出す。
- `--check` は書かず、`fmt: <相対パス> is not canonical (<n> rewrites)` を出して 1 つでもあれば終了 1。門番に使える。
- `fmt` は検査をしない。型の検査も、書き換えを持たない正準形違反（E507 / E508）の報告もしない。残りは `check` が報告する。
- `check --fix` の単位（報告されたアンカーに一致する 1 つ）は §57 のまま変えない。`fmt` は書けるものを全部書く点で別の道具である。
- `build --fix` は作る前に同じ整形を当ててから読み直して作る。
- レイアウト（空白と改行）の正規化はしない。正規形印字（`canonical_program`）はコメントを落とすので、コメントを保つ設計が決まるまで CLI に繋がない。
- 対象は入力根のソース全部で、入力の隣の下位ディレクトリのファイルも含む。

### 判定器が見つけた書き換えの穴

一括で当てるには、書き換えが**綴りを確かめてから**出ることが要る。正準形の `[]` と書かれた `Nil` は同じ AST（`PatternKind::Name("Nil")`）になるので、`Nil` の綴りを前提に長さ 3 を決め打ちしていた E510 / E516 の編集は、`[]` の上で当てると 1 文字余分に食った。52-a が `def empty | Nil => true` を整形させたとき、1 回目の走査で `Nil` を `[]` にした後、2 回目の走査が `[] ` を `[]` に、3 回目が `[]=` を `[]` にして `| []> true` を生んだ。`check --fix` は報告された診断のアンカーで照合するので、この穴に当たっていなかった。

`spelled(source, lines, anchor, text)` で綴りを確かめてから出し、constructor のパターン（`Nil`）は値の束縛ではないので E501 の対象から外した。`fixes()` が自分の出力に対して冪等になった。

### 実装

`fixes::format`（尽きるまで一括、上限 64 回）、`fixes::spelled`、`project::source_paths`（§50 のソースを相対パスと組みで返す。読み込みは呼び出し側に残すので、字句の誤りを持つファイルが他を隠さない）、`main.rs` の `format_sources`。`read_files` の道のりは `collect_paths` に切り出して順序の規則を 1 つにした。

### 判定の実測

- **52-a〜52-f（R52 の門番）。** `bash scripts/verify-r52.sh` は 7/7。52-a は書き換えを持つ拒否コーパス（`// error:` のヘッダで選ぶ）で `fmt` と `check --fix` の結果が同一バイトになり、`fmt` の後は `check` が終了 0 になることを測る。52-b は型の誤りに隠れた正準形違反（`def f x => x + 1`、`def main = f true`、`def g = let y = 5 -> y`）で、`check --fix` は変えず `fmt` は変える。52-c は `--check` が書かないことと冪等。52-d は入力根の下位ディレクトリまで直ること。52-e は `build --fix`。control は正準形のファイルを「変わる」と言わないことを先に測る。
- **実装前の赤。** `PASSED: 1/7`（corpus lock だけが緑）。
- **判定器の入力の誤り。** 3 つとも判定器側だった。52-b の生成入力が `def f x = x + 1`（`=>` の落ち）で E100 になっていた。52-b の 2 つの写しを同じ入力根に置いていて、入力根の全部を見る `fmt` が両方を直していた。52-d の library の `def` が未使用で E513 が残っていた。直した後は 7/7。
- **既存の門番。** `verify-r46.sh` 7/7、`verify-r47.sh` 7/7、`verify-r48.sh` 8/8、`verify-r51.sh` 5/5。`cargo test` は `check_cli` の 1 本だけが赤になった（usage の 1 行を逐語で固定していた。命令が 2 つ増えたので期待を直した）。直した後は 7 passed。

- **R52 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r52` は 50 本を走らせ、50 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3581/3581 passed; red: none`。

### 迷った点

- **一括か 1 つずつか。** 一括にした。診断に隠れた書き換えに届き、1 つのソースを何度も解析し直さず、`fixes()` の一覧をそのまま使える（共有の目的に合う）。`check --fix` の §57 の単位は変えない。
- **`fmt` が検査をするか。** しない。`fmt` は字句と AST だけを見て速く、責務が `check` と混ざらない。代わりに書き換えを持たない正準形違反（E507 / E508）は `fmt` では直らず、`check` で初めて見える。
- **上限の回数。** `check --fix` は 16 回、`fmt` は 64 回。どちらも仕様には書かない（実装の都合で、尽きないときは診断して終了 1）。
- **綴りの確認を `fixes()` に入れたこと。** `check --fix` の観測は変わらない（診断は綴りを伴って出る）が、`fixes()` の意味を「その場で当ててよい書き換え」に狭めた。`hint()` も同じ一覧を使うので、`canonical form:` の出方も変わらない。
- **`fmt` の出力を標準出力にしたこと。** 変更したファイルの一覧は成果であって診断ではない。`--check` の門番としての使い方も標準出力で読める。

### 仕様への疑義

- **一括は、検査器が報告しない（前の診断に隠れた）書き換えにも届く。** 書き換えは意味を変えないので安全側だが、「診断に出ていないものを書き換えてよいか」は言語の意味ではなく道具の立場の問題である。52-b がその差を固定している。審査の指摘が欲しい。
- **`fmt` は「直せなかった違反」を報告しない。** E507 / E508 の書き換えを足すときに、`fmt` の終了コードが整形の可否だけを表す今の形でよいかを決める。
- **レイアウト。** `wants.md` の 21 は「レイアウトと一意な意味保存の書き換えは `hado fmt` が書く」としているが、R52 は書き換えだけを入れた。コメントを保つ再印字の設計（コメントを AST に持たせるか、空白を編集として直すか）は未着手である。

## R53 入力根の走査から `.` 始まりを外す（2026-09-16）

### 背景と決定

`wants.md` の 38 が「実験用 `.hd` が同じディレクトリにあるとビルドが落ちる（§50、R17）。`.` 始まりを読まない、などは名前空間と衝突しない範囲で」と判断を残していた。§50 は入口のファイルのディレクトリ（根）の下の**全ての**サブディレクトリと `.hd` `.hdi` を読み、§51 はサブディレクトリの名前が `name` の字句でなければ CLI が拒否すると定めていた。

実測した摩擦は 3 つである。

| 根の中身 | 変更前の `check` |
|---|---|
| `main.hd` + `.git/`（中に `config`） | 終了 1、`invalid namespace directory: .git` |
| `main.hd` + `.probe.hd`（誤り） | 終了 1、`.probe.hd` の E305 が読まれる |
| `main.hd` + `.scratch/helper.hd`（誤り） | 終了 1、`invalid namespace directory: .scratch` |
| `main.hd` + `Bad/`（空） | 終了 1、`invalid namespace directory: Bad` |
| `.probe.hd` を明示して `check` | 終了 1、E305（明示したものは読まれる） |

いちばん重いのは 1 行目で、**git の倉庫の根では Hado の検査とビルドができない**。2・3 行目は試し書きの隔離先が無いことである。

- **決定 1: `.` で始まる名前のエントリは読まない。** ファイルもディレクトリも同じ規則で、ディレクトリなら中も読まない。規則を 1 つにする（Unix の隠し名前の慣習と同じ）。
- **決定 2: 明示して渡したファイルは読む。** 入口は渡されたものだから `.` 始まりでも読む。そのファイルのディレクトリが根になったとき、根の中の他の `.` 始まりは決定 1 に従う。
- **決定 3: §51 の拒否より前に外す。** `.` 始まりは `name` の字句になり得ないので「読まない」にする。`invalid namespace directory` は `.` 以外の不正な名前（`Bad` `1x`）にだけ出る。門番は緩めない。
- **決定 4: `fmt` と `build --fix` も同じ一覧を使う。** `project::source_paths` が `collect_paths` を共有するので自動的にそうなる。
- **決定 5: 診断コードを足さない。** 拒否は CLI のままで対象が減るだけ。`.` 始まりのディレクトリは名前空間に登録されないので `::` からも参照できない。
- **決定 6: 仕様は §50 と §51 に 1 文ずつ。** 追跡表の §50 の行を伸ばす。
- **決定 7: 「無視する」のであって「エラーにする」のではない。** 無言で無視すると名前空間の綴りの誤りが静かに消えるので、`.` 以外の不正な名前は今までどおり拒否する。

### 実装

`collect_sources` のループの先頭で、エントリの名前が `.` で始まれば `continue` する 3 行。`read_dir` の結果は名前で整列済みなので、順序の規則（入口、残りは相対パスのバイト順）は変わらない。

### 判定の実測

- **53-a〜53-f（R53 の門番）。** `bash scripts/verify-r53.sh` は 7/7。53-a は `.git/` を持つ根で `check` と `build` が通り、`run` が 42 を出し、control として `Bad/` と `1x/` が `invalid namespace directory` で拒否される。53-b は `.probe.hd` が読まれず（`check` 終了 0、`fmt --check` 終了 0）、`.` 無しの同じファイルは E305 で読まれる。53-c は `.scratch/` と入れ子が読まれず、同じ中身を `.` 無しに置いた 2 つの対照がどちらも読まれて落ちる。53-d は明示した `.probe.hd` が読まれ、`.` 始まりのディレクトリを根にする入力を `check` と `run` が読む。53-f は `fmt` と `build --fix` が隔離先のファイルを書かない。
- **実装前の赤。** `PASSED: 1/7`（corpus lock だけが緑）。`Bad/` の拒否と `.` 無しの誤りが読まれる control は最初から緑である。
- **判定器の入力の誤り。** 3 つとも判定器側だった。53-a の control が「根そのものが不正な名前」を作っていた（根の名前は検査されない）。53-c の制御が E100 を先に踏んで E305 を見られなかったので、中身ごとに 2 つの対照に分けた。53-d の入れ子の入力がファイル局所の `def` を別ファイルから参照していて E204 になった（§50）。直した後は 7/7。
- **既存の門番。** 走査を共有する `verify-r17.sh` 1/1、`verify-r22.sh` 1/1、`verify-r32.sh` 45/45、`verify-r52.sh` 7/7。`tests/` `stdlib/` `tools/` に `.` 始まりのエントリは無く、`invalid namespace directory` を期待する門番も無いことを先に実測した。

- **R53 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r53` は 51 本を走らせ、51 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3588/3588 passed; red: none`。

### 迷った点

- **ファイルもディレクトリも外す。** ディレクトリだけにすると規則が 2 つになる（`.` 始まりのファイルは読む）。試し書きは `probe.hd` の 1 枚のことも `.scratch/` のこともあるので、両方外す。
- **警告を出さない。** `.` 始まりを「見つけたが読まない」と報告する案は採らない。隠し名前は意図的な印なので、報告すると `fmt --check` のような静かな道具の出力が濁る。
- **設定ファイルを足さない。** `.hadoignore` のような仕組みは §50 の「設定ファイルと検索パスは無い」を変える。R53 は名前の規則だけで足りる。
- **明示した入口と `fmt` の組み合わせ。** `hado fmt .probe.hd` はそのファイルを書くが、根の走査からは `.probe.hd` が外れるだけで、根の**他の**普通の名前のソースは R52 の決定 10（対象は入力根のソース全部）に従って対象になる。隔離の意図とは少しずれるが、`fmt` の対象の規則を 2 つに割らない方を選んだ。審査の指摘が欲しい。

## R54 `hado test`（2026-09-16）

### 背景と決定

`wants.md` の 40（中）が「`hado test`。R20 の複数入口を順に `run` する。`*_test.hd`。assert マクロは足さない（§4）。10 の後」と、`hado run`（R48）の後に置いていた。R20 が複数入口を、R48 がビルドと実行と終了コードを入れたので、足りないのは「どれがテストかを決めて、順に走らせて、結果をまとめる」部分だけだった。

§54 のとおり `main` まで届いた `raise` は `Exception` の `message` を stderr に印字して終了コード 1、正常終了は 0 である。したがって合否は終了コードで足りる。実測した材料は次のとおり。

| 形 | `hado run` |
|---|---|
| `def main = check 1`（`Unit {Error}` を返し、合格） | 何も印字せず終了 0 |
| `def main = raise { message = "boom" }` | stderr に `boom`、終了 1 |
| `def main = 1 + true` | stderr に E305、終了 1 |
| `def helper = 0`（`main` が無い） | stderr に E206、終了 1 |
| 下位ディレクトリの `module` を使うテスト | 期待どおり 0 / 1 |

- **決定 1: `hado test <file.hd>`。** 入力根（§50）の直下の `*_test.hd` を、§50 の順序（相対パスのバイト順）で順に `run` する。
- **決定 2: 合否は終了コード。** 言語意味も診断コードも assert マクロも足さない（§4、`wants.md` 40）。テストは `if 条件 => () | raise { message = "..." }` を書く。
- **決定 3: 選ぶのは根の直下だけ。** `main` を持てるのは根のファイルだけなので（§50、§52）、下位ディレクトリの `*_test.hd` はプロジェクトの一員だがテストではない。
- **決定 4: 1 本ごとに報告し、落ちても止めない。** `ok` / `failed (exit <n>)` / `failed (signal)` / `failed (build)`。診断は標準エラー。
- **決定 5: まとめの行。** `test: <合格>/<全部> passed`。全部合格なら終了 0、1 本でも落ちれば 1。
- **決定 6: テストが 1 本も無ければ終了 1。** `test: no *_test.hd in the input root`。綴り違い（`main_tests.hd`）を静かに通さない（計画 7 節の「迷ったら拒否」）。
- **決定 7: 標準入出力はテストにそのまま渡す。** `run` と同じ（`wants.md` の「順に `run` する」）。捕捉は R54 の範囲外。
- **決定 8: テストに引数を渡さない。** `--` は `run` だけのもの。
- **決定 9: 既定の `check` / `build` / `fmt` はテストを特別扱いしない。** `*_test.hd` も普通のソースである（§50、§58.1）。
- **決定 10: 仕様は `## 58.2.` を §58.1 の直後。**

### 実装

`project::test_paths`（根の直下の `*_test.hd` を §50 の順で返す。`collect_paths` を共有するので `.` 始まりは外れる、§50）と、`main.rs` の `run_tests`。`run_program` のビルドと実行を `execute`（`Outcome::Exited` / `Signalled` / `Failed`）に切り出して `run` と共有した。`run` の報告（終了コード、`run: the program was terminated by a signal`、診断の印字）は変えていない。

### 判定の実測

- **54-a〜54-f（R54 の門番）。** `bash scripts/verify-r54.sh` は 7/7。54-a は合格する 3 本が相対パスのバイト順に走り、テストが stdout に書いた値と 1 本ごとの行と `3/3 passed` を逐語で比べる。54-b は `raise` するテストの前後が走り、`2/3 passed` と終了 1（stderr に `boom`）。54-c は型の誤りと `main` の欠落（E206）が `failed (build)`。54-d は `*_test.hd` が 1 本も無ければ標準エラーに `no *_test.hd` で終了 1（`mytest.hd` と `main_tests.hd` はテストではない）。54-e は下位ディレクトリの `util_test.hd` がプロジェクトの一員でありながら走らないこと（名前が `*_test.hd` なので、根の直下の規則が無ければ選ばれてしまう形で測る）。54-f は文書と層と lock。
- **実装前の赤。** `PASSED: 1/7`（corpus lock だけが緑）。
- **既存の門番。** `verify-r48.sh` 8/8（`run` の切り出しのあと）、`verify-r52.sh` 7/7、`verify-r53.sh` 7/7。`cargo test` の `check_cli` は usage の 1 行を逐語で固定しているので期待を直した。
- **層への登録の抜け。** 判定器は足したが `scripts/lib/gate-tiers.sh` への `r54` の登録を落としていた。54-f が層を見る判定なので赤で気づき、別のコミットで直した。

### 根は 1 つのプログラムであること

`hado test` は 1 本ごとに §50 のプロジェクトとして読み直して作る。したがって 1 つのテストファイルの型の誤りは**他のテストのビルドも落とし**、全部が `failed (build)` になる（54-c がこの形を固定している）。§50 のとおり根は 1 つのプログラムなので正しい振る舞いだが、テストの隔離を期待すると驚く。仕様 §58.2 に書いた。

- **R54 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r54` は 52 本を走らせ、52 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3595/3595 passed; red: none`。

### 迷った点

- **テストの出力を捕捉するか。** しない。`wants.md` の「順に `run` する」に従い、標準入出力をそのまま渡す。落ちたテストだけ出力を見せる形（`cargo test` 風）は、捕捉の設計と `--nocapture` が要るので R54 の外に置いた。
- **テストが 1 本も無いときの終了コード。** 1 にした。0 にすると `main_tests.hd` のような綴り違いが CI を静かに通る。計画 7 節の「迷ったら拒否」に合わせた。
- **テストに引数を渡せるようにするか。** しない。`run` で足りる。
- **`*_test.hd` を特別扱いする既定のコマンドは無い。** `check` はテストも普通に検査する。テストだけを検査する道具は要らなくなった（`check` が全部を見るので）。

### 仕様への疑義

- **根の誤りが全部のテストを落とす。** §50 の「根は 1 つのプログラム」から正しく出る帰結だが、テストの単位とプロジェクトの単位が一致していない。テストごとに隔離した根を作る案（`*_test.hd` を別の根に置く）は §50 の意味を変えるので、審査の指摘が欲しい。
- **`failed (build)` の診断がテストの本数だけ繰り返し出る。** 1 本ごとに読み直すためである。診断を 1 回だけにする設計（先に 1 回読んで落ちたら止める）は、走行を止める決定 4 と衝突する。

## R55 任意の定義の型の印字（2026-09-16）

### 背景と決定

`wants.md` の 12（高）「`--print-type` を任意の `def` に。効果が型に載るので `main` だけでは足りない」に答えた。`hado check --print-type` は R18 から `main` の型と効果行だけを印字していた。効果行が編集の主な関心である以上、`main` 以外の推論結果を見る手段が要る。

`Checked` には既に `definitions`（入口ファイルの外の `def`）と `modules`（契約が宣言する member）があったので、足りないのは名前で引く道と、**全称量化された変数の読みやすい印字**だった。推論の変数は内部の番号（`t902`）で、そのままでは読めない。

決定は 8 件（計画 51 節）。`--print-type=<名前>` を足す（値なしの形は変えない）。変数は `A` `B` … `Z` `A1` … と出現順に改名する。効果行は §20 の印字に開いた行の変数を `| A` で足す。`Ty` 自身の印字は変えない。名前が無ければ終了 1、空の名前は用法の誤りで終了 2。ライブラリ検査でも名前付きなら印字する。仕様は §58.3。名前で引く道は 1 つにする。

### 実測した材料

`Checked` の鍵の綴りを一時的な印字で測った。`parse_files` は入口ファイルを最初に `isolate(ast, 0)` するので入口は常に file 0 で、その `def` は `main` だけが `main`、残りは `file0$<名前>` で入る。契約の member は `lib::Util.helper`、`List.map`、`Int.to_string` のような利用者に見える綴りで `modules`（契約）と `module_definitions`（実装）の両方に入る。実装側の型は契約より多相でよいので、印字するのは契約が宣言する型にした。

値の `def` の型は検査器では引数 0 の `Fn` として持たれる（`infer.rs` の `definition()` が `Ty::fn_with(vec![], ty, row)` を作る）。この形は文法で書けないことを実測で確かめた（`Fn( => Int)` は E100、`fn => 42` も E100）ので、印字だけ本体の型と効果行に開くことにした。

### 実装

`types.rs` の `Scheme::canonical` と `Printer`。`Display for Ty` と同じ形（§16・§41 の後置適用、`$` の落とし、引数 1 つは括弧なし）を組み、変数だけを改名する。行は `Row::Closed` が既知のラベル、`Row::Var` が `{A}`、`Row::Open` が `{Io | A}`。ラベルの並びは §20 のとおりアルファベット順。`file_scopes::entry_name` に `file0$` の規則を 1 つだけ置き、`FileScope::internal_name` もそれを使う。`Checked::type_of` が `main`（§54 のとおり効果行を型の隣に持つ）、入口ファイルの外の `def`、`<契約名>.<member>` の順に引く。`main.rs` は `--print-type=<名前>` を読み、印字を `print_type` にまとめて `check --fix` とも共有した。

### 判定の実測

- **55-a〜55-f（R55 の門番）。** `bash scripts/verify-r55.sh` は 7/7。55-a は値の `def` が `Int {A}`（型変数の残らない値）、`A {Error | B}`（型変数と開いた行の残る値）。55-b は `def identity x => x` が `Fn(A => A {B})`（行を書かなければ暗黙の効果変数として全称量化される、§20）。55-c は `reader` が `Fn(String => String {Error})`、`twice` が `Fn(Fn(A => A {B}), A => A {B})`、`pure`（`Pure` を書いた `def`）が `Fn(Int => Int)` で、行変数の有無が書き分けられること。55-d は `--print-type=main` と `--print-type` が同じバイト（値なしの形の control は `def main = 42` が `Int`）。55-e は `lib::Util.helper` が契約の `Fn(Int => Int)`（実装の `def helper x => x` は `Fn(A => A {B})`）で、無い名前は標準エラーに 1 行を出して終了 1。55-f は文書と層と lock。
- **実装前の赤。** `PASSED: 1/7`（corpus lock だけが緑）。
- **既存の門番。** `verify-r18.sh`（型の綴りの往復）PASSED: 1/1、`verify-r32.sh`（ライブラリの `--print-type` が何も印字しないことを含む）45/45、`verify-r48.sh` 8/8、`verify-r50.sh` 6/6。`check_cli` は usage を逐語で固定しているので期待を直した。
- **実演。** 入口ファイルの `def` を一通り引いた: `answer` → `Int {A}`、`identity` → `Fn(A => A {B})`、`pure` → `Fn(Int => Int)`、`twice` → `Fn(Fn(A => A {B}), A => A {B})`、`reader` → `Fn(String => String {Error})`、`main` → `String {Error}`（値なしの形と一致）、`lib::Util.helper` → `Fn(Int => Int)`、`List.map` → `Fn(A List, Fn(A => B {C}) => B List {C})`、`Int.to_string` → `Fn(Int => String)`。無い名前は `check: no definition named nope in target/r55probe4/main.hd` で終了 1、空の名前は usage で終了 2。

- **R55 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r55` は 53 本を走らせ、53 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3602/3602 passed; red: none`（R54 の 3595 に R55 の 7 件が加わった）。

### 迷った点

- **値の `def` の印字。** 検査器の引数 0 の `Fn` をそのまま出すと `def answer = 42` が `Fn( => Int {B})` になる。この形は文法で書けない（E100）ので、本体の型と効果行（`Int {A}`）を出すことにした。`main` も値の `def` だが、検査器が効果行を型の隣に持つので、`--print-type` と同じ組み方をする道を `type_of` に置いた。
- **member は契約の型か実装の型か。** 契約にした（§47 のとおり実装は契約より多相でよいので、実装の型は利用者がその名前で使える型ではない。契約に無い実装の `def` は外から見えない、E204）。
- **`main` の一致を判定にするか。** した。同じ定義を 2 つの道で聞いた文字列が違うのは驚きなので、55-d が固定する。
- **空の名前の扱い。** 用法の誤り（終了 2）にした。`--print-type=` を「名前なし」と読むと、書き間違いが `main` の印字になって静かに通る。

### 仕様への疑義

- **行を書かない `def` は効果多相になる。** §20 の「書かなければ推論に任せ、暗黙の効果変数として全称量化する」に従うと、本体が純粋な `def identity x => x` の型は `Fn(A => A {B})` になる（実装もそう持つ。`isolate_effects` が新しい行変数から始めるため）。この印字は仕様に忠実だが、`{B}` は「効果は任意」を意味するので、多くの `def` に現れる。より精密にするなら一般化の時点で制約の無い行変数を空の行に閉じる（`identity : Fn(A => A)`）ことになるが、それは §20 の文と効果多相の範囲を変える。審査の指摘が欲しい。
- **`Ty` 自身の印字と本節の印字が別規則になる。** 診断（E305 など）と `main` の印字は今も `t902` を出し、行変数を隠す。§58.3 はそれらを変えないと定めたので、同じ型が場所によって違う文字列になる。E305 を読める形にするのは `wants.md` の 3 として残した。

## R56 型の食い違いの印字（2026-09-16）

### 背景と決定

`wants.md` の 3（高）「E305 に期待型と実型を載せる。`--print-type` はいま `main` だけ」の前半。R55 が後半を閉じたので、残りは診断である。優先順位 1 の本文の「診断に正準形・関連位置・期待型/実型がなければ強みが日々の摩擦になる」のうち、正準形（書き換えの `canonical form:`）と関連位置（アンカー）は R46〜R52 で入っており、型の印字が残っていた。

材料の実測で分かったのは 3 つ。(1) 型変数が内部の番号（`t900`）で出るので読めず、番号は実装の都合で変わる。(2) 適用の引数で食い違ったとき、アンカーは正しく実引数を指すのに、本文は**関数型どうし**を出していた。利用者が知りたいのはその引数の型と仮引数の型である。(3) 引数以外の食い違いは 2 つの型の順序が箇所によって揃っていない（`if` の腕は実型が先、注釈は期待型が先）。

決定は 7 件（計画 52 節）。適用の引数の食い違いは `expected <仮引数の型>, found <実引数の型>`、それ以外は対称な `cannot unify <a> with <b>`、E306 も `cannot unify`。診断の型変数は §58.3 と同じ改名をし、1 つの診断では 1 つの対応表を使う。対は既にどの引数で食い違ったかを求めている探針から取り、単一化を試す前に解決する。仕様は §15.1。

### 実装

`types::readable_pair` は §58.3 の `Printer` を共有し、2 つの型を 1 つの対応表で歩く。`Display for Ty` を使わないので、内部の番号は本文に出ない。行は §58.3 と同じく開いた行の変数を出す（`Fn(A => B {C})`）。隠すと効果変数が消えて `Pure` に見えるためである。

`infer.rs` の `unify` はこの対で本文を組む。`apply` は `application_argument_span` を `application_mismatch` に広げ、位置に加えて仮引数型と実引数型も返す。E305 のときだけ `expected ..., found ...` に差し替える（役割が確定している箇所だけ名指しする）。対は `probe.resolve` を**単一化の前**に呼んで作る。探針は clone だが、失敗した単一化の途中経過を印字しないためである。

### 判定の実測

- **56-a〜56-f（R56 の門番）。** `bash scripts/verify-r56.sh` は 7/7。56-a は `add1 "hi"` が `expected Int, found String` で、アンカーが実引数の先頭（行と列は生成した空行と字下げから判定器が計算する）。control は同じ形で型が合えば終了 0。56-b は仮引数が未確定の型変数のままの形（`twice true (fn(y => y) 1)` が `expected Fn(A => A {B}), found Bool`）、実引数にだけ変数が残る形（`apply2 fn(x => x > 0)` が `expected Fn(Int => Int), found Fn(Int => Bool {A})`）、第 1 引数で確定する形（`expected Int, found Bool`）。56-c は出現検査が `cannot unify A with Fn(A => B {C})` で、両側に現れる `A` が同じ名前。56-d は `[1, true]` が `cannot unify Bool with A`、`if` の腕が `cannot unify String with Int`。56-e は 7 つの形のどれも `t<数字>` を本文に出さない。56-f は文書と層と lock。
- **実装前の赤。** `PASSED: 1/7`（corpus lock だけが緑）。
- **文言の変更で赤になった既存の門番。** `verify-r33.sh` の 33-f2（`StringMonoid.combine (StringMonoid.of "a") 1`）は `expected String, found Int`、33dist（`A.read (B.make ...)`）は `expected A.Hidden, found B.Hidden` になった。どちらも「E305 が両側の型を名指しする」ことを測っているので、針を `cannot unify` から `expected ...` / `found ...` に直し、意図（両側が出る、`.T` が出ない、A と B が混ざらない）はそのまま測るようにした。門番は 73/73。期待の更新は仕様 §15.1 の追加に伴うものである。
- **実演。** `add1 "hi"` → `main.hd:3:17: error[E305]: expected Int, found String`、`twice (fn(y => y + 1)) true` → `expected Int, found Bool`、`self_apply` の `f f` → `error[E306]: cannot unify A with Fn(A => B {C})`、`[1, true]` → `cannot unify Bool with A`。

- **全走で見つかった記録のずれ。** 最初の全走（`target/gates-full-r56`）は 3608/3609 で `verify-r38` が赤だった。`docs/bug.md` の再現記録が `cannot unify t<数字> with Fn(Int => Int List {Error})` のままで、§15.1 の印字（変数は `A`、開いた行は `{Error | B}`）と一致しなくなっていた。記録は現行版でなければならないので、正規化の sed を §15.1 の形に合わせ（死んだ `t[0-9]+` の規則を落とし、行の変数と裸の変数を `<type>` に揃える）、記録した出力を `cannot unify <type> with Fn(Int => Int List {Error | <type>})` に直した。不具合の主張（E306 になること、衝突する型が `Fn(Int => Int List {Error ...})` であること）は変えていない。`verify-r38.sh` は 138/138。
- **R56 の全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r56b` は 54 本を走らせ、54 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3609/3609 passed; red: none`（R55 の 3602 に R56 の 7 件が加わった）。`cargo test` は 773 passed。

### 迷った点

- **1 つのコードに 2 つの言い方。** E305 は箇所によって役割が確定したりしなかったりするので、確定する箇所（適用の引数）だけ `expected` / `found` と名指しし、それ以外は対称な `cannot unify` のままにした。全部を名指しにするには各単一化の呼び出しに役割を与える必要があり、E306（出現検査）や record の field のように役割が 1 対 1 でない箇所も混ざる。審査の指摘が欲しい。
- **診断で行変数を出すか。** 出した（`Fn(A => B {C})`）。R55 と同じ理由で、隠すと効果多相が `Pure` に見える。ただし診断の本文は長くなる。`Fn(A => B)` と短く書いて効果は別行にする案は、1 行の診断という形を変えるので採らなかった。
- **実装後の実測で期待を 3 つ直した。** `twice (fn(y => y + 1)) true` の仮引数は第 1 引数で Int に確定するので `expected Int, found Bool` が仕様から出る正しい値である。出現検査の型は行変数まで出るので `cannot unify A with Fn(A => B {C})` が正しい。`twice (fn(y => y)) true` は**通る**（ラムダの型変数が Bool に解ける）ので、未確定の型変数を測る形を `twice true (fn(y => y) 1)` に変えた。**判定器の期待を実装に合わせたのではなく、仕様の規則から値を導き直した。**
- **過去のラウンドの実測表は当時の語のまま残した。** `docs/plan.md` の 24.7 と 33 節には `cannot unify ... t328` の形が残っている。あれは日付とコミットを添えた記録で、門番は読んでいない。台帳も同じ扱いである。

### 仕様への疑義

- **引数以外の食い違いの役割は決まっていない。** §15.1 は「順序は箇所によって揃っていない」と書いて対称な言い方で逃げた。`wants.md` の 3 の「期待型と実型」を全部に効かせるなら、`if` の腕・注釈・record の field・List の要素のそれぞれで役割を決めて仕様に書く必要がある。実装の都合ではなく言語の説明として決めるべきなので、審査の指摘が欲しい。
- **アンカーと対の関係。** 適用の引数の食い違いは、アンカーが実引数なのに本文の対は（仮引数型, 実引数型）の順である。§15.1 はこの順を定めたが、`expected` がアンカーの位置の型ではない（アンカーの型は `found`）ことを読者が誤解しないかは審査で見て欲しい。
- **`mono.rs` の内部エラーは依然として `specialization failed: ...` の形で利用者に出る。** あれは型の誤りではなく不具合の報告なので §15.1 の対象外にしたが、`t<数字>` が混ざる経路が別にあるかは確かめていない（R56 の探針は `check` の E305/E306 だけを見ている）。

## R57 E507 と E508 の書き換え（2026-09-16）

### 背景と決定

`wants.md` の 1（高）と 11（高、`hado fmt`）の残り。R46〜R52 で `check --fix` / `fmt` / `build --fix` の仕組みと E510 / E516 / E517 / E515 / E501 の書き換えを入れた。§57 の一覧に残っていたのは E507（再包装）と E508（再伝播）で、どちらも §45 / §36 に正準形が書かれているのに書き換えを出していなかった。

材料の実測で分かったことは 5 つ。(1) 分岐の形は 3 つある（`if E | ...`、第31節の `def f | ...`、`fn(| ...)`）。後の 2 つは同じ 1 つの引数 `$argument` に落ちるので、書き換えは関数の頭を組み直す必要がある。(2) **E508 の書き換えは位置を選ぶ。** `let p? =` の伝播先は囲む関数なので、本体でない `if` を素直に直すと型が通らない（実測: E508 だけを出す `let y = if ... ->` の形を直すと `error[E305]: cannot unify Int Option with R`）。(3) `let p? =` の `p` は `Option` の**中**の値の pattern で、リテラルは置けない（`let 1? =` は E402）。だから内側の pattern を移す。(4) 腕がすべて再構築の分岐（`| Some(v) => Some(v) | None => None`）は E507 と E508 の両方に当たるが、報告されるのは E507 で、書き換えも短い方（scrutinee）が良い。(5) `fn(| ...)` の分岐の直後の字句は、その `fn(` を閉じる `)` である。範囲を字句の手前までにすると `)` が余る。

決定は 7 件（計画 53 節）。E507 の正準形は scrutinee。`if` の形は `parser::canonical` を書き、第31節の形は `def f value => value` / `fn(value => value)`。fallback を持つ形は出さない。E508 は本体ちょうどの分岐だけで、腕 2 本・2 本目が `None` の再伝播・1 本目が `Some` 1 引数・その束縛名がすべて本体で使われること、を条件に `let <内側>? = <scrutinee> -> <本体>` にする。分岐の範囲は `Ast` の新しい表から取る。診断の `canonical form:` は改行を空白に畳んだ 1 行にする。

### 実装

`Ast` に `branch_spans: Vec<(Span, Span, Span)>`（分岐の開始・最初の `|`・分岐の直後の字句）を足した。`parse_pattern_branch` が最後に 1 件積む。既存の `branch_anchors` と同じ経路で合成する（prelude の足し込み、functor の行ずらし、空の表を作る 4 か所）。`fixes` は分岐の開始位置で引く。

検査との共有は `check::rewrap_branch`（E507 の条件そのもの、scrutinee を返す）と `check::repropagated_arm`（E508 の条件そのもの、`None` の腕と本体を返す）に切り出し、`check_rewrap` / `check_repropagation` と `fixes` の両方が呼ぶ。E515 と同じ方針で、判定が実装間でずれないようにする。

`fixes.rs` の新しい書き換えは 3 つ。E507 の `if` の形は分岐を `parser::canonical(scrutinee)` に置き換える（名前とリテラルは裸なので、どの位置に置いても綴りが変わらない）。E507 の第31節の形は定義まるごとを `def f value => value` / `fn(value => value)` にする。E508 は `let <内側>? = <scrutinee> -> <本体>` を組み立て、形に応じて `def f value => ` / `fn(value => ` を付ける。scrutinee と本体の綴りはソースの切片から写し、綴りの検査は `parser::list_pattern_source` を使う。範囲は字句から取り、直後の字句の手前まで空白を戻す。`fn` の形だけは対応する `)` の次までにする（(5)）。アンカーは `None` の腕の `|` で、pattern の直前の字句から取る（検査が `branch_anchors` で寄せる位置と同じ）。`is_fixable` に E507 と E508 を足した。

### 判定の実測

- **57-a〜57-g（R57 の門番）。** `bash scripts/verify-r57.sh` は 8/8。57-a は生成した Option 値で `if x` が `x` に、`if (x |> id_opt)` が `(x |> id_opt)` になり、直したプログラムの実行値が scrutinee の値と一致する。57-b は `def f value => value` と `fn(value => value)` で、実行値が入力と一致する。57-c は本体ちょうどの `if` が `let user? = find_user id -> Some(user + 1)` になり、`Some` の入力で変換値、`None` の入力で `None` が出る（伝播）。57-d は第31節と `fn` の形が `def g value => let user? = value -> ...` / `fn(value => let user? = value -> ...)` になる。57-e は負の対照（本体でない E508、fallback 付き E507、束縛名の無い E508、範囲にコメントを含む E507）のどれも `canonical form:` を出さず、`check --fix` も `fmt` もファイルを変えない。57-f は 3 つの形で `check --fix` / `fmt` / `build --fix` が同じ本文を作り、`fmt` が冪等である。57-g は §57・§58.1・追跡表・層・wants と corpus lock。
- **実装前の赤。** `PASSED: 1/8`（corpus lock だけが緑）。期待は仕様の形から判定器側で書いた。
- **実装で見つけて直した判定器の誤り。** 本体でない E508 の探針は最初 `let y = if ... -> y` としたが、これは E501（同一束縛）が先に報告されて E508 を測れない。Option を引数に取る関数で消費する形に変えた。出力の比較は印字の改行を 1 つ足し、負の引数は `(-4)` と括弧でくくった。
- **既存の門番。** `verify-r46.sh` は 7/7（46-e の「直せない診断」の例を E505 に差し替えた）、`verify-r47.sh` は 7/7、`verify-r52.sh` は 7/7。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r57` は 55 本を走らせ、55 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3617/3617 passed; red: none`（R56 の 3609 に R57 の 8 件が加わった）。`cargo test` は 773 passed。
- **実装後の見直しで見つけた捕捉。** 第31節と第32節の形は引数を `value` と名付けるので、本体が `value` を自由に参照していると、その参照が新しい引数に移る。素朴な書き換えを手で当てた形は `Some(201)` を `Some(202)` に変え、型は通る（`merge` の引数がどちらも `Int Option` のため）。`names::references` は自由な名前を返すので、それを調べて参照があれば書き換えを出さないことにした（§57）。負の対照を 57-e に足した。

### 迷った点

- **E508 の条件を厳しくした。** 束縛名が 1 つも無い形（`| Some(Wrap) => ...`）と、本体で使われない形は書き換えない。`do` が正準形になるからである。逆に、束縛名が**すべて**使われていることを要求した（一部だけ使う形は、書き換えると残りが E505 になる）。
- **E507 と E508 の重なり。** 腕がすべて再構築の分岐は両方に当たる。報告は E507 が先で、正準形も短いので、E508 の書き換えは E507 の形には出さないことにした（重なりの規則に任せると、同じ範囲のどちらが勝つかが読めない）。
- **`if` の形の置換に `canonical` を使う。** ソースの切片を写すと、`f a + b` のように位置で意味が変わる形があり得る。`canonical` は名前とリテラルを裸にするので、読みやすさを保ったままどの位置でも同じ綴りになる。E508 の scrutinee は `->` の直後なので切片を写す。
- **第31節の形の引数名は `value`。** 腕がすべて再構築なので本体は引数そのもので、元の引数の名前は残っていない（`$argument` はソースに書けない）。名前は `value` に固定した。固定すると本体の自由な `value` を捕捉し得るので、そのときは書き換えを出さない（`value1` のような名前を探す案は、正準形が本体の名前で変わるので採らなかった）。
- **`fmt` が書ける違反は尽きる。** §58.1 の「書き換えが尽きたソースには `fmt` が書ける正準形違反が残っていない」は、E507 / E508 を足したことで対象が広がった。文書の「E507 / E508 は直せない」という記述を、未使用と、形が局所でないコードに差し替えた。

### 仕様への疑義

- **本体でない位置の E508 の正準形は決まっていない。** §36 は `if find_user id | Some(user) => ... | None => None` を `let user? = ...` にする、と本体の形だけを書いている。本体でない分岐（`let y = if ... ->`）は伝播先が違うので同じ形にできない。`Option.map` のような別の正準形を決めるのか、そのまま許すのかは言語の判断で、実装の都合では決められない。審査の指摘が欲しい。
- **fallback を持つ E507 の正準形も決まっていない。** §45 の canonical form は `result` だが、`| Some(x) => Some(x) | result` の形を `result` に畳むと scrutinee の評価回数が変わる。効果があるときは意味が変わるので、書き換えを出さないことにした。仕様の側で「畳まない」と書くか、正準形を別に決める必要がある。
- **`let p? =` に置ける pattern の範囲が §36 からは読めない。** q1〜q4 の探針では `Some(Some(v))`・`P(a, b)`・record は通り、リテラルは E402 になった。E508 の書き換えはこの範囲に依存している（網羅しない pattern は元の分岐も E402 になるので実害は無い）。§35 の `let` の pattern の条件を読めば足りるかは確かめていない。

## R58 E205 と E602 の関連位置（2026-09-16）

### 背景と決定

`wants.md` の 2（高）「関連位置。E205 の 1 回目の宣言、E602 の欠けている member。1 件報告は崩さない」。診断の中身は R46〜R57 が正準形を、R55・R56 が期待型と実型を入れた。残る 3 つ目が関連位置である。

材料の実測で分かったことは 6 つ。(1) E205 の重複は 2 つの検出から出る。`ambiguous_declarations::record`（ディレクトリの名前空間）と `names.rs::register`（ファイルごとの名前空間と、式・型の中の field）で、どちらも `seen: HashSet<String>` しか持たず 1 回目の位置を捨てている。(2) ファイルをまたぐ重複は `other.hd:1:6` のように後ろのファイルを指し、1 回目は entry 側にある。(3) E602 の member の検査は `infer.rs` にあり、契約側の `declaration`（`SigDef` か `TypeDef`）を持っている。(4) 診断の座標は検査の中では全ファイルを連結した行で、`project::source_diagnostic` がパスとファイル内の行に直す。(5) prelude は利用者のソースの外に合成され、組み込み型には位置が無い。(6) 1 件報告なので行を増やせない。

決定は 6 件（計画 54 節）。関連位置はメッセージの続きに `; <ラベル> at <パス>:<行>:<列>` として同じ 1 行に書き、ラベルはコードごとに 1 つ（E205 は `first declared`、E602 は `declared in the contract`）。**メッセージに混ぜず `Diagnostic` の構造として持つ**（座標を `source_diagnostic` で直すため）。E205 は重複した名前・根の名前との衝突・外側の型との衝突・`library` の重複で出し、prelude と組み込み型との衝突と `type_realizations` では出さない。E602 は契約の member の検査 5 つで、契約側の宣言を指す。

### 実装

`parser` に `Related { path, line, col }` と `Diagnostic::related: Option<Related>` を足した。`Display` はコードのラベルがあるときだけ続きを書き、`1 行` は崩さない。`project::source_diagnostic` は `localize(files, path, line)` を切り出し、アンカーと関連位置の両方を同じ規則でファイル内の座標に直す（`file{index}$` の印を消す処理もそのまま効く）。既存の 50 か所の `Diagnostic` の組み立てには `related: None` を機械的に足した（新しい欄の代価である）。

位置を持つようにした表は 4 つ。`ambiguous_declarations` の `seen`、`names.rs` の `Declared`（`Namespaces` と型引数と field の一覧）、`namespaces` の `Symbols`、`project::library_roots` の 1 つ目の `library`。名前から `Span` への表にして、重複を見つけたときに 1 回目の位置を読む。`namespaces` では prelude と組み込み型の名前に `line: 0` の番兵を入れ、`site` がそれを落とす（利用者のソースに無い位置を出さないため）。`error_at` を足して、位置を持つ形だけが関連位置を渡す。

E602 の 5 か所（欠けた member、定義の形、型、多相性、欠けた型）は `error_related` で契約側の `name_span` を渡す。契約が `.hdi` でも `source_diagnostic` がそのパスに直す。

### 判定の実測

- **58-a〜58-i（R58 の門番）。** `bash scripts/verify-r58.sh` は 10/10。58-a は生成した空行と字下げの下で、2 回目の宣言が 1 回目を指す。58-b は `main.hd` と `other.hd` の重複で、後ろのファイルの診断が entry を指す。58-c は値と record の field。58-d は根の名前との衝突（サブディレクトリの宣言が根を指す）と `library` の 2 つ目。58-e は `.hdi` の契約の欠けた member と型の食い違いで、`store.hdi` の member を指す。58-f は契約が entry ファイルにある形。58-g は prelude の操作名との衝突と抽象型の循環が `; ` を出さないこと。58-h は `check --fix` と `fmt` がファイルを変えないこと。58-i は §56・表の 2 行・追跡表・層・wants と corpus lock。
- **実装前の赤。** `PASSED: 3/10`（58-g・58-h・corpus lock だけが緑）。位置を出さない形の対照と corpus lock は実装前から緑である。
- **実測した位置。** 同一ファイルの `type Color` 2 つは `main.hd:2:6: ... duplicate declaration Color; first declared at main.hd:1:6`。ファイルをまたぐと同じメッセージで `other.hd:1:6` から `main.hd:1:6` を指す。`.hdi` の契約では `main.hd:1:1: error[E602]: missing signature member; declared in the contract at store.hdi:3:5`。根の名前との衝突は `sub/other.hd:1:6: ... declaration conflicts with root name; first declared at main.hd:1:6`。`library` の 2 つ目は `other.hd:1:1: ... duplicate library declaration; first declared at library.hd:1:1`。
- **既存のテスト（触ったところ）。** `cargo test --offline --no-fail-fast --test corpus --test check_names --test check_modules --test r22_namespaces --test r32_library` は 14・3・4・19・15 passed、0 failed。
- **実装の形。** 14 ファイル、203 行の追加と 106 行の削除。うち 7 ファイルは `related: None` の機械的な追加だけである。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r58` は 56 本を走らせ、56 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3627/3627 passed; red: none`（R57 の 3617 に R58 の 10 件が加わった）。`cargo test --offline --no-fail-fast` は 773 passed、0 failed。

### 迷った点

- **`Diagnostic` の欄にするか、メッセージに混ぜるか。** メッセージに混ぜれば既存の 50 か所を触らずに済むが、座標を `source_diagnostic` で直せない（ファイルをまたぐと行番号が嘘になる）。欄にして、機械的に `related: None` を足す方を選んだ。新しい欄の代価が 7 ファイルの機械的な差分として見えている。
- **位置の無い宣言の表し方。** prelude と組み込み型は利用者のソースに位置を持たない。`Option<Span>` の表にするか、`line: 0` の番兵にするかを選び、番兵にした（表を 2 つ持つと綴りがずれる）。`site` が番兵を落とすので、`has` の判定はそのままである。
- **E205 の位置を出す範囲。** `type_realizations` の「実装の中の型が外の型と衝突」は、外の型の位置を呼び出し側から通す必要があるので今回は出さない。同じコードの中で位置が出る形と出ない形が混ざるので、表の行に両方を書いた。
- **E602 の関連位置は member の名前。** `def` の頭ではなく名前にした（E205 が名前を指すのと揃える）。「多相性が足りない」でも契約側の member の名前を指す。
- **record の field の重複のメッセージ。** `Payload { left; left = other }` の E205 は `name_resolution` の経路で、メッセージは `invalid name reference left` のままである。E205 には合わないが、メッセージの変更は今回の射程の外なので触っていない。

### 仕様への疑義

- §56 のエラー表の「アンカー」欄が、アンカーの規則と関連位置の規則の 2 つを持つようになった。列を分けるか、関連位置を別の節に置くかは仕様の判断である。
- prelude の名前との衝突のメッセージは `declaration conflicts with root name` である（prelude の名前しか無い場合も同じ）。関連位置を出さないことが表に書かれたぶん、この食い違いが見えやすくなった。既存の綴りなので変えていない。
- E305 の関連位置（wants 3 の残り）と、期待型・実型の役割の明示は未着手である。

## R59 未使用の効果と handler の E513（2026-09-16）

### 背景と決定

`docs/bug.md` の 1 件目「未使用の効果に E513 が出ない（R32、裁定 #12）」。§37 は効果を、§38 は handler を使わなければ E513 とし、§39 は未使用の判定を効果と handler にも当て、§56 の表の E513 の行もそう書いている。計画 18.5 は「E513 を effect と handler に広げる」を既存コードの変更として挙げていた。実装は無く、`def main = 0` の後に `effect Unused` と `once unused: Int` を置いても終了 0 だった。

材料の実測で穴は 2 つあった。1 つは診断が出ないこと。もう 1 つは、効果の操作の型にしか現れない名前が偽の E513 になること（`type Money` を `once charge (m: Money): Unit` の中でだけ使うと `main.hd:1:1: error[E513]`）。受理コーパスの `effect_forms` と `again_inner_a` は、操作が handler の腕に**実装**されているだけで呼ばれていないので、効果を使ったと数える規則に「handler の見出し」が要る。拒否コーパスの `tests/reject/E522` は、効果を効果行に書くだけで操作を呼ばない形で、期待する唯一の欠陥が行の重複の E522 なので、規則に「型や効果行に現れる名前」も要る。これは計画 55 節の決定 9（行に現れるだけでは数えない）と衝突したので、決定を差し替えた。

決定は 9 件（計画 55 節）。`Key` に `Effect` と `Handler` を足し、未使用のアンカーは宣言の見出しにする。効果を使うのは、名前が型や効果行に現れるか、操作の名前を参照するか、handler の見出しが効果を指すとき。handler を使うのは `with` が名前を書くとき。効果を使えばその操作の型に出る名前も到達で使われる。宣言された handler は腕が辿る先と同じく根にする。ライブラリ検査では効果と handler を公開面の根にする。名前で数え（ディレクトリで分けない）、型推論のまとまりの鍵の集合は変えない。

### 実装

`dependencies.rs` の 1 ファイル。`Key` に `Effect` と `Handler` を足し、`declaration()` が `effect.span` と `handler.span` を返すようにした。`Collector::new` が `checked` と要素から、constructor の表に加えて効果の名前の集合・操作の名前から効果への表・handler の名前から宣言への表を作る。`Item::Effect` の枝が操作の引数の型と戻り型を辿り、`Item::Handler` の枝が効果を参照してから腕を辿る。`expression` は操作の名前を効果に読み替え、`with` の handler の名前を handler に読み替える。`graph()` の最後で、型の参照が宣言された効果の名前なら効果の参照を足す（決定 9）。`diagnostics` は宣言された handler の見出しの効果を根に足し、ライブラリ検査の公開面に効果と handler を足す。`inference_groups` は `graph.retain` で効果と handler を外し、推論の順序を動かさない。

### 判定の実測

- **59-a〜59-i（R59 の門番）。** `bash scripts/verify-r59.sh` は 10/10。59-a は生成した空行と字下げの下で未使用の効果が `effect` の見出しを指し、効果の無い対照は受理。59-b は未使用の handler が `handler` の見出しを指し、`with` で使う形は build と実行値が一致。59-c は到達できる def からの操作の参照、操作の型にだけ現れる型（診断なし。effect を外した対照は E513）、効果行に名前が現れる形（E522 であって E513 ではない）。59-d は `library` の公開面では受理、同じ 2 つを entry に置くと E513。59-e は prelude の免除。59-f は到達しない def からの操作の参照が効果を救わないこと。59-g は 2 つの未使用の効果で 1 行。59-h は `check --fix` と `fmt` が書かないこと。59-i は §37・追跡表・層・bug.md と corpus lock。
- **実装前の赤。** `PASSED: 2/10`（59-e の prelude の免除と corpus lock だけが緑）。
- **実装で見つけた門番の誤り 2 件。** 59-c の探針は `def f (): Int {Ef} => op` としたが、hado の関数は引数が要る（`error[E100]: function requires a parameter`）。引数を使う `def f (x: Int): Int {Ef} => x + op` に直して赤を測り直した。もう 1 つは操作の型の探針で、`with H -> ()` とすると本体が効果を使わないため `error[E706]: main cannot have a user-defined effect` になり、E513 の代わりに E706 を測っていた。`tests/accept/handler_forms/main.hd` と同じく handler の腕の中の `with` で handler を使う形に直した。どちらも直した門番で赤を測り直している（2/10）。
- **コーパスが見つけた制約。** 実装後に `cargo test --offline --no-fail-fast --test corpus` が `tests/reject/E522/main.hd: expected E522, got main.hd:4:1: error[E513]: unused top-level declaration` で落ちた。E513 は E522 と同じ冗長の段階なので、位置の早い効果の宣言が勝っていた。決定 9 を差し替え（型や効果行の名前も使ったと数える）、corpus 判定は 4 passed に戻った。
- **決定の差し替えの実測。** `tests/reject/E522` と同じ形（`effect Ask` / `once ask: Int` / `def g (x: Int): Int {Ask, Ask} => x` / `def main = g 1`）は、行を数えない実装で `main.hd:1:1: error[E513]`、数える実装で `main.hd:3:27: error[E522]: duplicate effect in row`。
- **射程外の既存の振る舞い。** `def main = with H -> 0` のように本体が効果を使わない `with` は、効果が `main` に残って `main.hd:5:1: error[E706]` になる（効果の型を使わない最小の形でも再現する。このラウンドの変更の前後で同じ）。
- **効果と handler を持つ門番の予備走。** `verify-r32`（45/45）`verify-r33`（73/73）`verify-r34`（1360/1360）`verify-r35`（714/714）`verify-r36`（388/388）`verify-r37`（262/262）`verify-r43`（12/12）`verify-r44`（6/6）`verify-r50`（6/6）`verify-r52`（7/7）`verify-r55`（7/7）。
- **全走（1 回目、中断）。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r59` は m0 の検査 2 で `NG: 2 cargo-test status=101 passed=772 (<2 or nonzero)` になった。`crates/hado/tests/r37_world.rs` の探針が、宣言するだけで誰も名指ししない `effect Bar` を持っていたためである（§37 の E513 が正しく出た）。探針を、注釈の行が `Bar` を名指しする場合にだけ `effect Bar` と `once other` を置く形に直した（判定と場合分けは変えていない）。`cargo test` は 773 passed に戻った。この走行は他の門番の判定を待たずに中断し、走行の記録は `target/gates-full-r59` に残っている。
- **全走（2 回目）。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r59b` は 56 本を走らせ、56 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3637/3637 passed; red: none`（R58 の 3627 に R59 の 10 件が加わった）。`cargo test --offline --no-fail-fast` は 773 passed、0 failed。

### 迷った点

- **handler の見出しを根にした。** 効果を「handler の見出しが使う」と決めただけでは足りなかった。未使用の handler は `main` からの到達に入らないので、その handler の参照は閉包に入らず、効果の E513 が先に出る（59-b と 59-d が最初これで落ちた）。既存の「宣言された handler はプログラムの一部で、腕が辿る先は根と同じ」という規則に合わせ、見出しの効果も根に足した。
- **型の参照から効果への読み替え。** 効果行は `TypeExpr` の `effects` に入っているので、`type_expression` が既に辿っている。`graph()` の最後で型の参照を読み替える形にすると、署名・実装・functor のどの行でも同じ規則が当たる。
- **名前で数える。** 同じ操作名を 2 つのディレクトリの効果が宣言していても、参照が 1 つあれば両方を使ったと数える。偽の E513 を出さない側に倒した。
- **推論の順序。** 効果と handler を推論のまとまりに入れると形が変わるので、`graph.retain` で外した。外しても推論の順序は今までと同じである。
- **既存の探針が未使用の効果を持っていた。** 規則を実装するまで、効果は未使用でも咎められなかったので、R37 の探針は「装飾のための `Bar`」を置いていた。規則を狭めるのではなく探針を直した。§37 と、1 ファイル 1 欠陥を固定する拒否コーパスが正で、探針の方が仕様から離れていたためである。緩めたのは探針の入力だけで、判定は同じである。

### 仕様への疑義

- §37 は「一度も使わなければ」とだけ書き、効果の「使う」を定めていなかった。今回「名前が型や効果行に現れるか、操作を参照するか、handler を宣言すること」と書いた。この境界（とくに、別のディレクトリの同名の操作を参照したときに両方を数える粗さ）は審査の対象である。
- `with H -> rest` の本体が `H` の効果を使わないとき、`main` に効果が残って E706 になる。§38 は「`rest` を `H` の下で評価する」とだけ書くので、実装が余分に効果を足しているのか、仕様に「使わない `with` は効果を消さない」と書くべきなのかが決まらない。仕様を変えず、ここに書く。
- E513 のメッセージは未使用の `def`・`type`・契約・実装・効果・handler で同じ `unused top-level declaration` である。表の行は「未使用の宣言」なので揃っているが、効果と handler だけ別のメッセージにする余地はある。

## R60 E305 の役割と関連位置（2026-09-16）

### 背景と決定

`docs/wants.md` の 3「E305 に期待型と実型を載せる」（高）。R56 が適用の引数に `expected <仮引数の型>, found <実引数の型>` を入れ（§15.1）、R58 が関連位置の書式を入れた（§56）。残りは「引数以外の食い違いの役割の明示」と「関連位置」である。§15.1 は第 1 形を適用の引数だけに許し、それ以外は対称な `cannot unify <a> with <b>` としていた。E305 は日々いちばん出るコードで、どちらが期待でどちらが実際かを書かない文面は、正準形の拒否を直す体験を濁していた。手順は `docs/plan.md` 56 節。

材料の実測で、役割が確定しアンカーが食い違った側を指す箇所は 5 つあった。値の定義の本体と宣言された戻り型（`main.hd:1:1` の 1 行、アンカーは定義の先頭）、条件式の腕どうし（`main.hd:1:39`、アンカーは後の腕）、条件式の条件（`main.hd:1:27`、アンカーは条件）、レコード literal のフィールド（`main.hd:2:28`、アンカーは値）、適用の引数（R56 のまま）。

外したものも実測で決めた。演算子の被演算子は、`true && x` のアンカーが式の先頭 `true`（食い違うのは右の `x`）、`1 + x` のアンカーが `1` になる。§56 のアンカーは「型が食い違った式の先頭」なので、`expected Bool, found Int` とすると、その位置が `Int` であるかのように読める。比較の被演算子は左右のどちらも宣言された期待でない。リストの要素とパターンの腕は、期待の側がまだ解けていない型変数 `A` として印字される（`def f (x: Int): Int List => [1, "a"]`、`type O = | Mine(Int) | Nothing` の腕）。`let` の注釈は構文が無い（E100）。

決定は §15.1 の第 1 形を役割の確定する 5 箇所へ広げ、うち 2 箇所にだけ関連位置を足すこと（定義の戻り型、先の腕。ラベルは E305 で 1 つ `expected here`）。アンカーは §56 の表のまま動かさない。演算子・比較・リストの要素・パターンの腕は対称な文面のまま。コードと書き換えは足さず、1 件報告も崩さない。

### 実装

`parser.rs` の `related_label` に `E305 => expected here` を足した（§56 のラベルはコードごとに 1 つなので、位置の役割はコードが決める）。`infer.rs` に `expect`（期待の型・実際の型・アンカー・期待の位置）と、それを式 1 つに当てる `branch_result_expect` を足した。当てたのは 4 箇所で、値の定義の本体（`expect(&expected, &result, def.span, Some(annotation.span))`）、`BoolBranch` の腕と `conditions` の腕（関連位置は先の腕。自分の腕と一致するときは落とす）、`condition` の `Bool`、`record` のフィールドである。既存の `branch_result` は、レコードのフィールドのほかにリストの要素・let の本体・literal のパターン・腕が共有しているので変えず、確定した箇所だけを新しい入口に寄せた。

### 判定の実測

- **60-a〜60-j（R60 の門番）。** `bash scripts/verify-r60.sh` は 10/10。60-a 定義の本体と宣言された戻り型（文面 `expected Int, found String`、アンカーは定義の先頭、関連位置は戻り型の先頭で `expected here`）、60-b 条件式の腕（アンカーは後の腕、関連位置は先の腕）、60-c 条件式の条件、60-d レコードのフィールド、60-e 適用の引数（R56 のまま）、60-f 演算子と比較は対称な文面のままで食い違いの無い対照は受理、60-g 1 件報告、60-h 書き換えなし、60-i 文書と層、60-j corpus lock。生成した空行と字下げの下で、位置は判定器側で計算している。
- **実装前の赤。** `PASSED: 5/10`。60-a〜60-d と 60-i が NG。計画 56 節は「実装前は 60-a〜60-e が赤いはず」と書いたが、60-e は R56 の既存の振る舞いなので初めから緑だった（予想が誤りだった。ここに記す）。
- **R56 の門番の期待値が仕様に追い越された。** `bash scripts/verify-r56.sh` は 56-d「引数以外の食い違いは cannot unify のまま」が NG になった。R56 の時点の仕様では正しかったので、R60 の仕様に合わせて直した。条件式の腕は文面と先の腕の関連位置をそのまま照合し、リストの要素は対称な文面のままであることも残した。判定の強さは変えていない（部分一致ではなく文面全体と位置の照合）。直した後は 7/7。`cargo test --offline --no-fail-fast` は 773 passed、0 failed（変更した箇所を固定する単体テストは無かった）。
- **文面を持つ門番の掃き出し。** `grep -ln "cannot unify" scripts/verify-*.sh` は `verify-r56.sh` と `verify-r60.sh` だけだった。`verify-m0`（7/7）`verify-m1-corpus`（7/7）`verify-r57`（8/8）`verify-r50`（6/6）`verify-r52`（7/7）`verify-r48`（8/8）`verify-r55`（7/7）は赤くならず、`verify-r58`（10/10）と `verify-r37`（262/262）も緑である。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r60` は 56 本を走らせ、56 本のログがすべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは集計自身が出した `TALLY: 3647/3647 passed; red: none`（R59 の 3637 に R60 の 10 件が加わった）。`verify-m0.sh` の `cargo-test` は `passed=773`、`verify-r60.sh` は 10/10、`verify-r56.sh` は 7/7 である。

### 迷った点

- **演算子を最初の計画に入れていた。** 計画の初版は算術と論理の被演算子も stage 1 に入れていた。`true && x` のアンカーが式の先頭 `true` になることを測って外した。期待の型は演算子が決めるが、位置は食い違わない側を指し得るので、`expected Bool, found Int` は位置の意味を偽る。計画 56 節を差し替えてコミットした。
- **共有する `branch_result` を触らなかった。** レコードのフィールドと条件式の腕は同じ入口を通るが、リストの要素と let の本体と literal のパターンも通る。リストは期待の側が解けていない型変数なので対称な文面のままにしたい。新しい入口を足して確定した箇所だけを寄せた。
- **関連位置がアンカーと同じときは落とす。** 条件式の最初の腕は、自分の型を期待として他の腕と比べる。自分の位置を関連位置に出すと意味がないので、位置が一致するときは `None` にした。
- **R56 の門番を直した。** 期待値を仕様に合わせて書き換えるのは、門番を緩めて green にすることと紙一重である。今回は (1) 仕様の方を先に書き換えた、(2) 照合は文面全体と位置のままで強さを落としていない、(3) 対称な文面が残ることを同じ検査の中で測り続けている、の 3 つで区別した。

### 仕様への疑義

- **演算子のアンカーは食い違わない側を指し得る。** `def f (x: Int): Bool => true && x` の E305 は `main.hd:1:25`（`true` の位置）に出る。§56 の「型が食い違った式の先頭」をそのまま読んだ結果である。演算子の箇所で役割を名指しするなら、アンカーの規則を「食い違った被演算子の先頭」に変えるか、式のアンカーとは別に役割を書く必要がある。今回は仕様を変えず、対称な文面のままにした。
- **リストの要素とパターンの腕は、期待の側が解けていない型変数として印字される。** `cannot unify String with A` の `A` は文脈の型（`Int`）に解けるはずで、解ける前に印字されている。§15.1 は診断ごとに 1 つの対応表を使うと定めるが、解けた型を出す順序については何も言わない。期待の側を名指しできるようにするには、この順序の穴を先に閉じる必要がある。
- **§56 の「ラベルはコードごとに 1 つ」で足りるか。** E305 の関連位置は、定義の戻り型と先の腕という 2 つの役割を持つ。`expected here` という 1 つのラベルで両方を指している。箇所ごとのラベル（`declared here` / `first arm`）の方が読みやすいが、§56 の規則を書き換える必要がある。

## R61 再帰定義の省略した効果行（2026-09-16）

### 背景と決定

`docs/bug.md` の最後の 1 件「効果行を省いた再帰生成器が E306 になる」（R35、裁定 #11）。R35 の台帳は原因を未調査のまま先送りし、25 ラウンド残っていた。優先順位 3「check は通るが実行できないをなくす」の逆側で、正しいプログラムを拒否する穴である。手順は `docs/plan.md` 57 節。

§20 は「行は書かなくてよい。書かなければ推論に任せ、暗黙の効果変数として全称量化する」と定める。§9 は E307 を「再帰呼び出しにおいて、引数の型と仮引数の型の単一化が出現検査に失敗した場合」に限る。よって効果行を省いた再帰定義は受理され、行は推論されて効果変数として全称量化されるのが仕様である。実装の不具合で、仕様の疑義ではない。

原因は、再帰呼び出しの単一化が定義のプレースホルダの行をその呼び出しの行に束縛していたことである。計測器で同定した。定義のプレースホルダ `Var(896)` は再帰呼び出しの単一化で `Fn([Int] => _ {Var 897})` になり、本体の推論結果は行が `{Error | Var 897}` になる。同じ `Var 897` が定義の行と本体の行の末尾の両方に現れるので、群の最後の単一化が `897 = {Error | 897}` を要求して出現検査に落ちていた。

決定は、再帰呼び出しの効果をその呼び出し自身の新しい行とし、定義の行は本体の行との単一化で決めること。呼び出しの効果の行は定義の行に含まれるので、省略した行の効果は呼び出し側まで届いたままである。2 度目以降の再帰呼び出しも、相互再帰の群も同じ扱いになる。書いた行の検査（`contain_row`）と E307 の機構（引数の出現検査）には触れていない。

### 実装

`crates/hado/src/infer.rs` の適用の 1 箇所。再帰呼び出しのときだけ、呼ばれる側の行に新しい行を使う。

```rust
let callee_row = if recursive { self.inference.fresh_row() } else { row.clone() };
self.unify(&function, &Ty::fn_with(argument_types.clone(), result.clone(), callee_row), span)
```

### 判定の実測

- **61-a〜61-g（R61 の門番）。** `bash scripts/verify-r61.sh` は 61-f を除いて緑。61-a bug.md の形（`check` が受理、`build` して実行すると `5`、`--print-type=generate` が `Fn(Int => Int List {Error | A})`）、61-b 非再帰と相互再帰の受理、61-c 書いた `{Io}` と本体の `Error` は `E305` のまま・`{Error}` は受理、61-d 省略した行のユーザー効果は main で `E706`・handler で囲むと `15` を出して終了 0、61-e `def rec poly x => poly [x]` は `E307`、61-g corpus lock。
- **実装前の赤。** 実装を `git stash push -- crates/hado/src/infer.rs` で戻して測り、`PASSED: 3/7`。61-a・61-b・61-d が NG で、61-c と 61-e は実装前から緑だった（計画 57 節の予想どおり）。
- **修正の中身の確認。** 省略した行は開いた行として出る。`--print-type=generate` は `Fn(Int => Int List {Error | A})` を出し、非再帰の `def f (n: Int): Int List => [n % 7]` も同じ開いた行を出す（省略した行の印字は非再帰でもこうである）。効果は消えない。省略した行のユーザー効果は main まで届き `E706` になり、handler で囲めば受理されて `15` を出す。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r61` は 57 本を走らせ、すべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは `TALLY: 3654/3654 passed; red: none`（R60 の 3647 に R61 の 7 件）。`verify-m0.sh` の `cargo-test` は `passed=773`。効果の推論に触ったので効果の門番を個別に見ると、r24（2/2）r32（45/45）r33（73/73）r34（1360/1360）r35（714/714）r36（388/388）r37（262/262）r52（7/7）r55（7/7）r59（10/10）r60（10/10）r61（7/7）で、赤は無い。`verify-r38.sh`（138/138）は `bug.md` の節が 0 でも再現の数と一致する。`verify-r49.sh` の「直した節が消えている」検査も緑である。

### 迷った点

- **行を分けることが意味を保つか。** 再帰呼び出しの効果を未拘束の行にすると、本体はその呼び出しの効果を「何でもよい」と仮定する。しかしその行は定義の行に含まれるので、定義の行は「`Error` と、その行」になり、呼び出し側が具体化する。61-d はこの経路を測っている（効果が消えず main まで届く）。
- **計測器を木に入れた。** `unify` の E306 の枝に一時的な `eprintln!` を置いて変数を同定し、`git stash push -- crates/hado/src/infer.rs` と組み合わせて赤も測った。計測器は同じラウンドで外し、コミットには残っていない。
- **`bug.md` の節を消した。** `verify-r38.sh` は `docs/bug.md` の全節の再現手順を実行し、節の数と再現の数が一致することを測る。節が 0 でも数は一致するので緑のままである（実測で確認した）。経緯はこの台帳と git 履歴に残る。

### 仕様への疑義

- **省略した行の精度。** 再帰定義の省略した行は `{Error | A}` と開いて出る。§20 の「暗黙の効果変数として全称量化」とは読めるが、`Error` しか出さない定義の行が `{Error}` に閉じないこと（最小の解を選ばないこと）は仕様に書かれていない。非再帰の省略も同じ開いた行を出すので、このラウンドで変えた性質ではない。閉じた最小の行にするには、行の既定化の設計が要る。
- **再帰定義の行の方程式。** 本体の行が定義の行を含むとき、`ρ = {Error | ρ}` のような方程式になる。今回は呼び出し側の行を未拘束のまま残すことで解いているが、§20 は再帰定義の行をどう解くかを書いていない。

## R62 catch-all の `_` を fallback の正準形にする（2026-09-16）

### 背景と決定

`docs/wants.md` の 1「冗長の診断に正準形を出す」（高）の残り。R46 / R47 / R57 が E510 / E516 / E517 / E515 / E501 / E507 / E508 の書き換えを入れ、`check --fix`・`hado fmt`・`build --fix` が `crates/hado/src/fixes.rs` の同じ表を共有している。残りとして wants が挙げる 7 つのコードを実測し、§24 と §29 が正準形を明記している E520 と E503 を射程に選んだ。手順は `docs/plan.md` 58 節。

E503 は §29 が「fallback の canonical form は `| default`」と書いている。catch-all の `_` の腕は fallback の形が正準形なので、`_ =>` を消す局所の書き換えになる。E520 は §24 が `if condition => yes | no` を正準形と定め、Bool リテラルの pattern 分岐と腕が 1 本だけの条件列を禁じている。

E520 の書き換えは入れられなかった。`fixes.rs` は式の範囲を `branch_range` で取るが、これは `ast.branch_spans` に載っている枝の位置でしか働かない。E520 の形は引数や `let` の値など枝でない位置にも現れ、その場合は式の終わりの位置が AST に無い（`Span` は行と列だけで、式の終わりを持たない）。E503 は `_` と `=>` の 2 つのトークンから範囲を作れるので、こちらだけを入れた。E520 の書き換えは式の終わりの位置を AST に持たせてからにする。

### 実装

`fixes.rs` に `catch_all_fallback_fix` を足し、`is_fixable` に `E503` を加え、`walk_expression` の `NodeKind::PatternBranch` から呼ぶ。固定したのは、腕が 1 本だけで fallback が無いこと、その腕の pattern が `PatternKind::Underscore` であること（`Member(name, _)` のような入れ子は対象外）、消す範囲に `//` を含まないこと。`canonical` には腕の本体の正準形を `| body` の形で入れる（消す範囲は `_ =>` なので、表示する正準形と実際の置換文字列は別である）。

### 判定の実測

- **62-a〜62-f（R62 の門番）。** `bash scripts/verify-r62.sh` は 6/6。62-a 診断が `canonical form: | 0` を出し、`check --fix` と `hado fmt` が `| 0` にし、直したソースが受理され、`fmt --check` が 0、62-b `_ =>` の後ろのコメントが残ること、62-c §24 の除外（入れ子の Bool リテラルと 2 条件列）が受理され、規則そのものは E520 を出し続けること、62-d 既存の E501 と新しい E503 が一括の `fmt` で settle すること、62-e 仕様と追跡表と層、62-f corpus lock。
- **実装前の赤。** `git stash push -- crates/hado/src/fixes.rs` で戻して測り、`PASSED: 3/6`（62-a・62-b・62-d が NG。62-c・62-e・62-f は緑）。
- **門番が自分の材料の欠陥を捕まえた。** 最初の混合の材料は `if n | 0 => 1 | _ => n` で、E503 ではなく E402（non-Boolean literal patterns require a fallback）が先に当たっていた。コンストラクタの腕に直した。1 ファイル 1 欠陥の規約が門番の側でも効いている。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r62` は 58 本を走らせ、すべて `PASSED: n/n`、`^NG:` 0、exit 0。締めは `TALLY: 3660/3660 passed; red: none`（R61 の 3654 に R62 の 6 件）。`verify-m0.sh` の `cargo-test` は `passed=773`。`verify-r57.sh`（8/8）と `verify-r60.sh`（10/10）と `verify-r61.sh`（7/7）は書き換えの表を共有するので個別にも緑を確認した。

### 迷った点

- **E520 を落とした。** 計画 58 節は 2 つを射程にしていたが、範囲の道具が無いことを実装で確かめて 1 つに絞り、計画に追記して門番も 6 つに直した。判定の強さは落としていない（E520 の規則そのものは門番で測り続ける）。
- **消す範囲と表示する正準形を分けた。** 範囲を腕全体（`| _ => 0`）にすると表示は素直だが、腕の中のコメントで書き換えが止まる。範囲を `_ =>` にすると後ろのコメントが残るので、`canonical` には本体の正準形を入れて表示だけを informative にした。
- **`no_rewrite` を使わなくなった。** 最初の門番はコメントの場合に書き換えを出さないことを測っていたが、範囲を `_ =>` にしたので書き換えは出る。コメントが残ることを測る形に変えた。

### 仕様への疑義

- **式の終わりの位置が AST に無い。** `Span` は行と列だけで幅を持たないので、枝でない位置の式を書き換えるには、トークン列を走査するか `branch_range` のような枝の表が要る。E520 だけでなく、引数や `let` の値にある形を正準形にするときは同じ壁に当たる。
- **複数の scrutinee の catch-all。** `_, _` のような 2 つ以上の pattern の catch-all も E503 になるが、この書き換えは腕が 1 本の場合だけを扱う。fallback の形が 1 つの式である以上、複数の pattern の catch-all を fallback に落とす書き換えは書けない（fallback は pattern を持たない）。
- **正準形の印字。** E520 の書き換えでは条件と腕の本体を `parser::canonical` で印字する。この関数は「完全に括弧を付けた正準形」と書かれているので、`if (a && b) => 1 | 2` のように括弧が増える可能性がある。E503 では本体の印字だけなので、このラウンドでは現れていない。

## R63 Bool リテラルの腕を Boolean 分岐の正準形にする（2026-09-16）

### 背景と決定

R62（58 節）が落とした E520 の書き換えを入れた。R62 は「E520 の形は引数や `let` の値にも現れ、その場合は式の終わりの位置が AST に無いので範囲を作れない」として落としたが、これは `branch_range` と `branch_entry` を読んで判断した誤りだった。実測すると `branch_range(Tail::If)` は枝の位置だけでなく、引数の位置と `let` の値の位置でも効く。式の終わりの位置を持たせる必要は無かった。

§24 は `if condition => yes | no` を正準形と定め、Bool リテラルの pattern 分岐を E520 とする。腕は真偽で選び直せばよいので、書かれた順が逆でも値は変わらない。`check.rs` の `check_boolean_form`（304 行から 322 行）が規則の正本で、診断の位置は `node.span`。手順は `docs/plan.md` 59 節。

### 実装

`fixes.rs` に `boolean_branch_fix` を足し、`is_fixable` に `E520` を加え、`walk_expression` の `NodeKind::PatternBranch` から `catch_all_fallback_fix` と並べて呼ぶ。固定したのは、scrutinee が 1 つ、すべての腕の pattern が Bool リテラル 1 つ（`PatternKind::Literal` の中の `NodeKind::Bool`）、そして（腕 2 本で fallback 無し）または（腕 1 本で fallback 有り）であること。腕 2 本と fallback が同居する形は E520 だが Bool の分岐は 2 本なので正準形が無く、書き換えない。範囲は `branch_range(node, Tail::If, ...)`、`canonical` は `flatten` した置換文字列。

### 判定の実測

- **63-a〜63-f（R63 の門番）。** `bash scripts/verify-r63.sh` は 6/6。63-a 腕 2 本、63-b 腕 1 本 + fallback、いずれも診断が `canonical form: if b => 1 | 2` を出し、`check --fix` と `hado fmt` がその形にし、直したソースが受理され、`fmt --check` が 0。63-c E503 と同じファイルで一括の `fmt` が両方を settle。63-d §24 の除外（入れ子の Bool リテラル、2 条件列）が受理され、規則は E520 を出し続け、引数の位置と `let` の値の位置も書き換わる。63-e 仕様と追跡表と層、63-f corpus lock。
- **実装前の赤。** 実装を入れる前の門番は `PASSED: 2/6`（63-a・63-b・63-c・63-e が NG。63-d・63-f は緑）。
- **門番が材料の欠陥を 3 つ捕まえた。** (1) 混合の材料は分岐を入れ子にしていて E101 が先に当たっていた、(2) `type T` と `g` が `main` から使われず E513 が先に当たっていた、(3) 63-e の §24 の言い回しの想定が違っていた。いずれも材料側を直した。
- **branch_range の射程。** 定義の本体、引数の位置、`let` の値の位置の 3 つで書き換えが出ることを実測した。効かない位置は見つかっていない。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r63` は 60 本を走らせ、exit 0、締めは `TALLY: 3666/3666 passed; red: none`（R62 の 3660 に R63 の 6 件）。`verify-m0.sh` の `cargo-test` は `passed=773`。書き換えの表を共有する 4 本も個別に緑（r57 8/8、r60 10/10、r61 7/7、r62 6/6、r63 6/6）。

### 迷った点

- **R62 の台帳の記述を訂正した。** 「引数や `let` の値では範囲を作れない」は読んで判断した誤りで、測れば効いた。R62 のラウンドで E520 を落とした判断は、測らずに読んだことが原因である。
- **腕 2 本 + fallback は書き換えない。** E520 ではあるが、Bool の分岐は 2 本なので正準形が無い。診断はそのまま出る。

### 仕様への疑義

- **腕が 1 本だけの条件列。** §24 は `if | b => x | y` も E520 とするが、条件の位置が腕ではないので `if b => x | y` への書き換えでは表せない。R62 から引き続きスコープ外である。
- **正準形の印字の括弧。** `parser::canonical` は「完全に括弧を付けた正準形」と書かれている。今回の材料（Bool の変数と整数リテラル）では括弧は現れなかったが、条件や腕が複合式のときの印字は測っていない。

## R64 fmt のレイアウトの正規化（2026-09-16）

### 背景と決定

`docs/wants.md` の 11（高）の残り「レイアウトの正規化」。`def main =\n\t1   `（タブの字下げ、行末の空白 3 つ、末尾の改行なし）を実測すると `fmt --check` は 0、`fmt` は何も書かず、`check` も 0 だった。レイアウトは正規化されていなかった。

決定は 5 つ。行末の空白（スペースとタブ）を消し、ファイルの終わりを改行 1 つにする。空のファイルはそのまま。書き換えを当てた後に当てる。`check --fix` は診断のアンカーに一致する書き換えだけを当てるので、レイアウトは当てない（§57 のまま）。数の綴りは変えず、変更の合計を数える。字下げの幅（タブをスペースに直すか）と CRLF と空行の数は決めない。手順は `docs/plan.md` 60 節。

### 実装

`fixes.rs` の `format` を、書き換えが尽きたことを `settled` で持つ形に直し、書き換えの後に `layout` を当てて数に足す。`layout` は行ごとに行末の空白を落とし、末尾の改行を 1 つにする。空の入力はそのまま返す。`check --fix` は `fix_at` を通るので `layout` には来ない。

### 判定の実測

- **64-a〜64-f（R64 の門番）。** `bash scripts/verify-r64.sh` は 6/6。64-a タブ字下げ + 行末の空白 + 末尾の改行なしが `fmt --check` で `is not canonical (2 rewrites)` と報告され（書き込まない）、`fmt` が `fmt: main.hd (2 rewrites)` を出して `def main =\n\t1\n` にし（タブの字下げは残る）、2 回目は何も変えず、`fmt --check` は 0。64-b 正準なファイルには触れない。64-c 空のファイルは終了コード 0 で何も出さない。64-d `check --fix` はレイアウトを当てない。64-e 仕様と追跡表と層、64-f corpus lock。
- **実装前の赤。** `PASSED: 4/6`（64-a と 64-e が NG。64-b〜64-d・64-f は緑）。
- **門番の期待値の誤りを 1 つ直した。** `fmt` の行は入力根からの相対パスなので `fmt: main.hd (2 rewrites)` である。最初 `fmt: a (2 rewrites)` と書いていた。赤は 64-a の終了コードの判定で出ていたので、赤そのものは本物である。
- **空のファイル。** `fmt` は終了コード 0 で何も出さない（実測）。計画の「変わらない」と一致した。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r64` は 61 本を走らせ、exit 0、締めは `TALLY: 3672/3672 passed; red: none`（R63 の 3666 に R64 の 6 件）。`verify-m0.sh` の `cargo-test` は `passed=773`。レイアウトは `fmt` の一括整形にだけ入れたので、`fmt` を走らせる既存の門番（r52 / r57 / r62 / r63）も緑。走行の後に `git status --short` は空で、走行がリポジトリのソースを書き換えていない。

### 迷った点

- **タブの字下げを残す。** 幅を決めるとソースの見た目を勝手に変えるので、決めないことにした。行末の空白と末尾の改行は意味を持たないので消す。
- **数の意味。** 「書き換え」の数が「変更」の合計に広がる。綴りは §58.1 のままである。`fmt --check` の門番は数を見るので、意味は仕様に書いた。

### 仕様への疑義

- **CRLF。** 行末の空白として `\r` を消すかどうかを決めていない。`\r` は「行末の空白（スペースとタブ）」の外なので、CRLF のファイルはそのまま通る。改行コードを正規化するかは決めていない。
- **空行の数。** ファイルの中の連続する空行は触らない。レイアウトの正規化の一部とするかは決めていない。

## R65 診断の機械可読な出力（2026-09-16）

### 背景と決定

`docs/wants.md` の 6（中）。優先順位 1 の「将来の LSP code action」は診断を機械可読に読む入口を要る。材料の実測で、整形は `Project::report`（`project.rs:70`）の 1 か所、正準形は `canonical_hint`（`project.rs:80`）が別に返す、CLI の印字は `main.rs` の 4 か所、`--format` は無い、`Diagnostic.code` は `Option<String>`、`Related` のラベルは `related_label` がコードから決める、と分かっていた（計画 61 節）。

決定は 6 つ。1 行 1 診断の JSON。フィールドは `path` `line` `col` `code` `message` `canonical` `related` の 7 つ。`--format json` は 4 か所の印字を切り替え、既定は今の 1 行のまま。`message` に `; canonical form: ...` を混ぜず `canonical` に入れる。印字先と終了コードは変えない。`fmt` と `dump-mono` は受けない。

### 実装

`project.rs` に `report_json` と、`option_string` / `json_string`（`"` `\` 改行 復帰 タブ と 0x20 未満の文字を逃がす）を足した。`parser::related_label` を `pub(crate)` にした。`main.rs` は `--format json` を `check` / `build` / `run` / `test` で受け、4 か所の `eprintln!` を `print_diagnostic` に集約した。2 か所は入れ子の関数の中にあり `main` の局所変数が見えないので、切り替えは `static FORMAT_JSON: AtomicBool` に置いた（このファイルは既に `AtomicU64` を同じ流儀で使っている）。

### 判定の実測

- **65-a〜65-g（R65 の門番）。** `bash scripts/verify-r65.sh` は 7/7。65-a 1 行で `json.loads` が読め、path が `main.hd` で終わり、line / col が `1 25`、code が `E520`、message が `use a Boolean branch` で始まる。65-b `canonical` が `if b => 1 | 2` で message に混ざらず、`related` が `null`。65-c E205 の `related` が line 1 と `first declared`。65-d 既定の 1 行が `error[E520]` と `; canonical form: if b => 1 | 2` を含み `{` で始まらない。65-e `--format yaml` と値を付けない `--format` が終了コード 2。65-f 仕様と追跡表と層、65-g corpus lock。
- **実装前の赤。** `PASSED: 3/7`（65-a・65-b・65-c・65-f が NG）。計画は 65-e も赤いと見ていたが、未知の引数は元から用法の誤りで終了コード 2 なので緑だった。予想が外れた 1 件として記録する。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r65` は 62 本を走らせ、exit 0、締めは `TALLY: 3679/3679 passed; red: none`（R64 の 3672 に R65 の 7 件）。`verify-m0.sh` の `cargo-test` は `passed=773`。診断の印字を変えたので、診断の 1 行を期待値に持つ門番（r57 / r60 / r62 / r63 / r64）も緑である。

### 迷った点

- **印字の切り替えに `AtomicBool` を使った。** 4 か所のうち 2 か所が入れ子の関数の中にあり、`main` の局所変数を渡すには関数の引数を増やす必要がある。このファイルが既に `AtomicU64` をグローバルに使っているので、同じ流儀にした。引数を増やすより差分が小さい。
- **`fmt` と `dump-mono` は `--format` を受けない。** `fmt` の `fmt: ...` と `test` の `test: ...` は診断ではないので JSON にしない（§58.4 に書いた）。

### 仕様への疑義

- **`label` が引けないとき。** `related` を持つのに `related_label` が `None` を返すコードは、いまのところ無い（§56 の 3 つはすべてラベルを持つ）。仕様では `null` と書いたが、そのような診断を作らないことを門番で固定してはいない。

## R66 一巡の通し確認（2026-09-16）

### 目的

依頼の「書いて、直して、実行し続けられる一巡」が実際に通ることを、実バイナリで最初から最後まで測る。コードは変えないので、この節は実測の記録である。

### 実測

わざと冗長なプログラムを書いた（タブの字下げ、行末の空白、`| _ => 0` の catch-all、`if b | true => 1 | false => 2`、`let v = ... ->` の束縛）。

```
$ hado check main.hd
main.hd:4:18: error[E503]: use a fallback instead of a catch-all underscore pattern; canonical form: | 0
$ hado check --format json main.hd        # 1 行目
{"path": "main.hd", "line": 4, "col": 18, "code": "E503", "message": "use a fallback instead of a catch-all underscore pattern", "canonical": "| 0", ...
$ hado fmt main.hd
fmt: main.hd (3 rewrites)
$ cat main.hd                             # 直った後
type T = | A | B

def pick (t: T): Int =>
	if t | A => 1 | 0

def choose (b: Bool): Int => if b => 1 | 2

def main =
  let v = pick A ->
  log (Int.to_string (v + choose true))
$ hado check main.hd ; echo accepted=$?
accepted=0
$ hado run main.hd
2
```

### 分かったこと

- `check` は 1 件だけを canonical form 付きで出し（§56 の 1 件報告）、同じ内容が `--format json` で機械可読に読める（§58.4）。
- `fmt` は canonical の書き換え 2 件（E503、E520）とレイアウト 1 件を一括で当て、`3 rewrites` と報告した（§57、§58.1）。
- 直したファイルは `check` を終了コード 0 で通り、`run` が期待どおり `2` を出力した（`pick A` が 1、`choose true` が 1）。argv・stdio・終了コードは門番 r48 / r54 が固定している。
- タブの字下げは残る（§58.1 で「字下げのタブは変えない」と決めたとおり）。
- `let v = pick A ->` の束縛は E501 にならない（`v` を本体で使っているため）。E501 は「そのまま返す束縛」だけである。

### 残り

- E512 の書き換え（設計は `docs/wants.md` の 1 に確定。条件ごと scrutinee に畳む）。
- `tools/gates/main.hd` を `--format json` に食わせて、Hado 側で見つかる不足を出す（依頼の「一本の実用 CLI を継続運用」）。
- 終了コード（wants 25、§54 の二値の反転になる）。


## R67 受理コーパスの実行確認（2026-09-16）

### 背景と決定

依頼の優先順位 3「check は通るが実行できない、をなくす」は、`docs/bug.md` の 1 件（R61 で閉じた）だけでなく門番の穴の問題でもある。実測すると、`scripts/verify-m2.sh` から `verify-m5.sh` まで `run` / `build` を呼ぶ門番は 0 件、`tests/accept` に期待出力のファイルは 0 件で、受理コーパス 109 件は検査と型の印字だけを確かめられ、一度も走らせていなかった。

決定は 3 つ。受理ファイルのヘッダが宣言する `// expect:`（stdout）と `// expect-error:`（終了とエラー名）を、実際に `build` して走らせて比べる。`check` が 0 なら `build` も 0 でなければならない（優先順位 3 の主張そのもの）。シグナル死は赤とし、件数も名前も判定器には書かず実数とタグから導く。

### 実装

`scripts/verify-r67.sh` を新設した。`cargo metadata` からバイナリの位置を取り、`tests/accept/*/main.hd` の各件を `check` → `build -o <一時ディレクトリ>` → 実行の順に確かめ、タグに照らす。`full` 層に r67 を登録した。

### 判定の実測

- **67-a / 67-b（初回から緑）。** `R67: fixtures=109 compared=101 errored=8 untagged=0 red=0`、`PASSED: 1/1`。`check` が 0 の 109 件すべてで `build` が 0 で、走らせた件にシグナル死は 0。
- **比較の中身。** stdout を宣言と比べた 101 件はすべて一致。`// expect-error:` の 8 件は終了コードが非 0 で、宣言された名前（`stopped`、`DivisionByZero`、`IndexOutOfRange` など）が stderr にあった。タグの無い件は 0 で、走らせた本数 + エラー終了 + タグ無し = 109（実数）。
- **これは「穴が無い」ではなく「穴を塞いだ」の実測である。** 実装前は build / run が壊れていても層は見逃した。いまは受理ファイルの宣言がそのまま実行で確かめられる。
- **全走。** `bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r67` は 3680 本、exit 1、`TALLY: 3679/3680 passed; red: lint-docs.log`。赤は「そのラウンドの台帳の節が無い」ための lint-docs である（この節を書く前だった）。この節を足した後に `bash scripts/lint-docs.sh` を回して 7/7 に戻した。他の 3679 本（r67 を含む）は緑で、この節は docs だけを変える。

### 迷った点

- **`// expect:` を stdout の期待値に使った。** `hado run` は `main` の値を印字する（第54節）ので、コーパスの宣言とそのまま比べられる。この宣言は今まで一度も実行で確かめられていなかった。
- **赤にする範囲。** 終了コードの値そのものは第54節が二値と決めているので、`expect-error` は「非 0 で名前が stderr にある」で足りる。値の 1 か 2 かは見ていない。
- **計画の見込みは外れた。** 手順書では「赤が出たら実用性の穴なので 1 件ずつ直す」と書いたが、赤は 0 件だった。コーパスは宣言どおりに動いていた。

### 見つけた摩擦

- **報告の癖。** `git status --short` のあとに無条件で `(clean)` と出していたため、実際には `fixture` と `ordinary` が未追跡で残っているのに「clean」と報告していた（R67 の門番を回す前の時点で既に残っていた）。中身は `fixture` が `roundtrip`（9 バイト）、`ordinary` が `file`（4 バイト）のテキストで、ELF ではなかった。はじめ `crates/hado/tests/corpus.rs:96` の `std::fs::write(work.join("fixture"), contents)` を原因と書いたが、同テストの `observe_launch` は `Scratch`（一時ディレクトリ、`Drop` で削除）の中の `work` に書き、中身も `root.file_name()` なので、これは誤りだった（中身が `roundtrip` である時点で一致しない）。`cargo test --offline` の後に `git status --short` が空であることを実測したので、リポジトリのテストは根に書き残さない。中身の語（`roundtrip`、`file`）から見て、このセッションの前半に私が手で置いた探り書きの残りである。削除した。以後は空のときだけ `(clean)` と出す。

- **未追跡ファイルの本当の出どころ（追測）。** R67 の全走をもう一度回すと `run-gates: the working tree changed during the parallel phase` で exit 1 になった（TALLY は `3680/3680 passed; red: none` のまま）。走行の後に `fixture` と `ordinary` が再び現れていた。**書き手が分かった。** 全走を回しながら 0.5 秒ごとにツリーを見張り（背景ジョブ。前景では呼び出しの時間上限に当たる）、`fixture` と `ordinary` が現れた瞬間（21:32:56）に `ps` で走っていた門番を出すと `scripts/verify-r67.sh` だけであった。原因はこの門番が `hado build -o <一時ディレクトリ>` で作った実行ファイルを**リポジトリの根を作業ディレクトリにして走らせていた**ことで、ファイル I/O の受理ファイルが `fixture` / `ordinary` を根に書いていた。実行を作業ディレクトリごとの一時ディレクトリで行うように直し（第58節の `run` と同じ隔離）、判定は同じ（`fixtures=109 compared=101 errored=8 untagged=0 red=0`）のまま、走行の後に根が汚れないことを確かめた。`cargo test --offline` だけなら根は汚れない（実測）。処方は、この種の fixture を一時ディレクトリへ書くことである（`scripts/verify-r36.sh` と同型の門番を洗う）。`ordinary` の書き手はまだ特定していない。

- **修正後の全走（追測）。** 実行を作業ディレクトリで行うように直した後、`bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r67c` は `TALLY: 3680/3680 passed; red: none` で **exit 0**、走行の後に `git status --short` は空であった。修正の前は同じ TALLY で exit 1（並行相でツリーが変わった）になり、根に `fixture` と `ordinary` が残っていた。緑の走行が緑で終わるようになった。

## R68 E512 の書き換え（2026-09-16、工程 2 まで）

### 背景と決定

`docs/plan.md` の 63 節。E512（`top-level variable patterns are forbidden`、§26）は `check.rs` の `catch_all_row`（436 行）が、腕の pattern がすべて `_` か変数で、どれかが変数のとき出す。正準形は fallback だが、fallback の形は E507（`branches reconstruct the value`）を出し、`hado fmt` はその形を書き換えない（R57 の E507 の書き換えは fallback のある形を避ける。実測）。決定は 4 つ。腕が 1 本で pattern がすべて `_` か変数、どれかが変数、scrutinee が単純な変数のときだけ対象にする。本体の束縛を scrutinee の名前に置き換える（重ならない複数の編集でよい。範囲に `//` があれば書かない）。生成した形が E507 を出すなら条件ごと scrutinee に畳む。`fmt` と `check --fix` で同じ書き換えを共有する。

### 門番（工程 2）

`scripts/verify-r68.sh` を新設し、full 層に登録した。判定は「E512 が消えた」ではなく「**直したファイルが `check` を通る**」まで見る（68-a）。68-c は直す前後の実行結果を比べるが、期待値は判定器に書かず、手で書いた冗長でない版（`def f (t: T): T => t`）と同じ出力になることで導く。68-d は対象外の形（束縛を使わない・scrutinee が関数呼び出し・範囲にコメント）を書き換えないことを見る。実行は作業ディレクトリごとの一時ディレクトリで行う（R67 で直した隔離と同じ）。

### 実測

- **実装前の赤:** `PASSED: 1/5`。68-a は `check --fix` が終了コード 1 のまま（`main.hd:2:36: error[E512]: top-level variable patterns are forbidden`）、68-b は `fmt` の後も受理されず、68-c は `check --fix` が 1 を返すため実行の比較に届かない。68-e は追跡表と仕様の記述が無い。
- **68-d は緑である。** まだ何も書き換えないので当然の緑で、実装の後も緑でなければならない（対象外の形を書き換え始めたら赤になる）。
- **`lint-docs` の一時的な赤:** 工程 2 のコミットの直後に 6/7 になった（そのラウンドの台帳の節が無いという既知の規則）。この節を書いて 7/7 に戻した。

### 次の工程

- 工程 3: `fixes.rs` に E512 の書き換えを足す。生成した形が E507 を出すなら、決定 3 の畳み込みまで入れる（測ってから決める）。
- 工程 4・5: 仕様の 1 文と追跡表、台帳の追測と全走。

### R68 の設計上の疑義（工程 3 の着手前に測ったこと）

`fixes.rs` の E503 の書き換え（`catch_all_fallback_fix`、223 行）は、pattern の位置から**トップレベルの最初の `Arrow`** までをトークンで走査し、`Arrow` の終わり（`+ 2`）から続く空白を読み飛ばして、`_` と `=>` を消す範囲を 1 つ作る。`PatternBranch` からの呼び出しは 814〜816 行で、`catch_all_fallback_fix` と `boolean_branch_fix` を続けて呼び、`Fix` を 1 つずつ `found` に積む。

E512 はこれをそのまま広げられない。`| y => y` から `y => ` を消すだけでは本体の `y` が未結合になり、直したファイルが `check` を通らない（**68-a の判定そのものに落ちる**）。本体の `y` を scrutinee に置き換える編集が**もう 1 つ**要る。ところが `check --fix` が 1 回に当てる `Fix` が 1 つだけなら、範囲を消す編集と本体を置き換える編集の片方しか当たらず、中途半端なファイルが残る。R62 の E503 は 1 つの範囲を消すだけなので、この点はまだ測られていない。

したがって工程 3 の前に次を測る。(1) `check --fix` は同じ枝に積まれた複数の `Fix` を 1 回で全部当てるか、先頭の 1 つだけか。(2) 1 つだけなら、本体の範囲全体を置き換える**単一の編集**（本体のソースを写して名前だけ置き換えた文字列を、本体の span に当てる）で組めるか。`push_edit`（`found, code, anchor, replacement, source, lines, length`）は範囲に `//` があるとき書かない規則を持っているので、単一編集にすればその規則もそのまま使える。この 2 つが決まるまで `fixes.rs` は触らない（決まらないまま書くと、直したファイルが別の誤りで落ちる書き換えになる）。

### R68 の疑義の実測（`check --fix` は 1 回に 1 範囲）

読んで確かめた。`fixes.rs` の `Fix` は `{ code, line, col, start, end, replacement, canonical }` で、**範囲を 1 つだけ**持つ。`fixes(source)` は重なる編集のうち外側だけを残して `Vec<Fix>` を返す。`project.rs` の `apply_canonical_fix`（117 行）は `error.code` が書き換え可能なとき `fixes::fix_at(&source, code, error.line, error.col)` で **1 つの `Fix`** を取り、`source[..start]` + `replacement` + `source[end..]` を書く。

したがって `check --fix` が 1 回に当てるのは **1 範囲だけ**である。E512 を「`y => ` を消す編集」と「本体の `y` を置き換える編集」の 2 つで組むと、1 回目の後に未結合の名前が残り、直したファイルが `check` を通らない（68-a が赤のまま）。**単一編集で組むのが必須**と決まる。

単一編集の形は、pattern の先頭から本体の終わりまでを 1 範囲として、写した本体の名前だけを scrutinee に置き換えた文字列に差し替える。残る不明点は**ノードの終わりのオフセットの求め方**である（`LineMap` は位置からオフセットを出すが、範囲の終わりは span の終わりが要る）。同じ「枝の全体を差し替える」書き換えが既にある（E507 の `rewrap_if_fix`、R57）ので、そこから求め方を写すのが次の 1 歩である。`push_edit`（`found, code, anchor, replacement, source, lines, length`）が範囲に `//` があるとき書かない規則を持っているので、単一編集にすればその規則もそのまま使える。

### R68 の実装レシピ（E507 の枝全体の書き換えから写した）

`fixes.rs` の `rewrap_if_fix`（537 行）が、枝の全体を差し替える書き換えの作り方をそのまま示している。

- `let scrutinee = crate::check::rewrap_branch(node)?;` で「腕が組み直している値」を取り、アーム形（`$argument`）と `fallback.is_some()` を外す。**fallback がある形を避ける**理由がコメントで書いてある（fallback は scrutinee を再評価するので、1 回に畳むと効果の回数が変わり得る）。R63 で測った「fallback 形は `fmt` が書き換えない」はこれで説明がつく。
- **`let (start, end) = branch_range(node, Tail::If, source, lines, tokens, index, branches)?;`** — 枝の全体の開始と終了のオフセットはこれで取る。`branches` は `Ast.branch_spans`（`&[(Span, Span, Span)]`）。
- `if source[start..end].contains("//") { return None; }` — コメントを含む範囲は書かない。
- `let replacement = parser::canonical(scrutinee);` — 差し替える文字列は正準形の印字から作る。
- `Fix { code, line: node.span.line, col: node.span.col, start, end, replacement, canonical }` — `line` / `col` は**診断の位置**でなければならない（`fix_at` が診断の line / col で突き合わせるため）。E507 の診断は枝を指すので `node.span`、E512 の診断は pattern を指すので `pattern.span` を使う。

E512 に写すとこうなる。最後の腕で、行が変数 1 つ、`fallback.is_none()`、scrutinee が `scrutinees` の 1 つで `NodeKind::Name` のときだけ対象にする。範囲は **pattern の先頭から枝の終わりまで**（腕が最後なので `branch_range(...).1` が本体の終わりになる）。差し替える文字列は、本体のソースを写して束縛の名前を scrutinee の名前に置き換えたもの。`canonical` は `"| "` にそれを足した形（E503 の `| 0` と同じ流儀）。

**残っている不明点は 3 つ。** (1) `PatternKind::Name` / `NodeKind::Name` から名前の文字列を取り出す口（`rewrap_if_fix` は `name == "$argument"` と比較しているので文字列として使えるが、`Display` か field かは未確認）。(2) 本体の置き換えは**トークン単位**でやる必要がある（`y` を素朴に文字列置換すると `yx` の `y` まで置き換わる）。`tokens` と `index` が渡っているので、本体の範囲のトークンを並べ直して束縛のトークンだけ差し替える。(3) 置き換えた名前が本体で影になっていないかの判定（内側の `let` が同名を束縛する場合）を構文だけで決められるか。

この 3 つは `fixes.rs` の他の書き換え（名前を読むもの、トークンを並べ直すもの）を 30 行ほど読めば決まる。次に実装に入るラウンドは、そこから始める。

### R68 の残る不明点の解消（工程 3 の直前）

- **名前の比較。** `fixes.rs` では `NodeKind::Name(name) if name == "$argument"`（548 行）と `PatternKind::Name(bound)`（803 行）で、名前は `&str` と `==` で比較できる。**名前の文字列を取り出す口は要らない。** 束縛の綴りは、pattern の span のトークンをソースから切り出して得る。
- **トークン単位の置き換え。** `fixes.rs` にトークンの text フィールドは無い。`Tok` は種別と span を持つだけなので、**隣り合うトークンのオフセットの間をソースから切り出す**（`TokenIndex::position(span)`（1060 行）と `lines.offset` で並べる）ことで、元の綴りのまま本文を作り直せる。長さを知る必要がない。束縛と同じ綴りのトークンだけを scrutinee の綴りに差し替えるので、`yx` の `y` を巻き込まない。
- **影の判定。** 構文だけで決めるのは保留し、**安全側に倒す**。本体に `let` が現れる形は書き換えない（同名を束縛していなくても書き換えない。過度に保守的だが、誤った置き換えより良い）。68-d の対象外の形には影響しない。

これで工程 3 に必要な材料は揃った。実装は (1) `catch_all_variable_fix` を `catch_all_fallback_fix` の隣に足す（対象は最後の腕・行が変数 1 つ・`fallback.is_none()`・`scrutinees` が `NodeKind::Name` の 1 つ）、(2) 範囲は `lines.offset(source, pattern.span)` から `branch_range(node, Tail::If, ...)?.1` まで、(3) 範囲に `//` があれば書かない、(4) 差し替えは本体のトークンを並べ直して束縛と同じ綴りだけを scrutinee の綴りにする、(5) `Fix { code: "E512", line: pattern.span.line, col: pattern.span.col, start, end, replacement, canonical: "| " + replacement }`、(6) `PatternBranch` の腕（814 行の隣）から呼ぶ。本体に `let` があるときは (1) の条件で外す。

### R68 の実装の実測（工程 3 の途中、`fixes.rs` を書いた）

書いてビルドして門番を回した。`catch_all_variable_fix` を `catch_all_fallback_fix` の隣に足し、`PatternBranch` の腕から呼び、`is_fixable` に E512 を加えた。**ビルドは 1 回で通った**（`cargo build` exit 0）。

門番は `PASSED: 2/5`。`check --fix` は E512 を消し、本体の束縛を scrutinee に置き換えた形（`if t | A => A | t`）を書く。**ところがその形は E507**（`branches reconstruct the value`）で、直したファイルは `check` を通らない。68-a・68-b・68-c が赤で、68-d（対象外の形は触らない）と 68-e（仕様・追跡表・層・corpus lock）は緑。

`crate::check::rewrap_branch(node)` を先に試す畳み込みも入れたが、効かなかった。**置き換える前の木を見ている**ので、束縛 `y` を返す腕（`| y => y`）は「組み直している」と見なされず `None` が返る。E507 の判定は**置き換えた後の木**に要る。

正しい形は見えている。書き換えの文字列を作ったあと、**その文字列を `parser::parse_source` で解析し直し、新しい枝に `rewrap_branch` を当てる**。scrutinee が返れば枝全体を `parser::canonical(scrutinee)` に差し替える 1 範囲を出し、返らなければ今の置き換えの 1 範囲を出す。どちらも 1 範囲なので `check --fix` の制約（1 回に 1 範囲）に合う。次のラウンドはここから書く（読む必要はもう無い）。

### R68 の工程 3〜5 の実測（門番 5/5）

`catch_all_variable_fix` が動いた。ただし**最初の版は直したファイルを E507 で落とした**（`if t | A => A | t` を書いた）。畳み込みを 2 通り試して、2 つ目で通った。

- 元の木で `crate::check::rewrap_branch(node)` を呼ぶ版は効かない。置き換える前の木を見るので、束縛 `y` を返す腕（`| y => y`）は「組み直している」と見なされない。
- 置き換えたソースを `fixes(&candidate)` に掛けて E507 の書き換えを拾う版も効かない。置き換えた後の腕は **fallback** になり、E507 の書き換えは `fallback.is_some()` を避ける（fallback は scrutinee を再評価し得るため）。**これは正しい拒否**で、この経路では畳めない。
- 通った版は、**各腕を自分で検査する**。scrutinee が単純な変数であることは対象条件で保証されているので、畳んでも効果の回数は変わらない。コンストラクタの腕（`PatternKind::Name` で大文字参照）は本体の正準形が自分の pattern の綴りと等しいこと、変数・`_` の腕は本体の正準形が scrutinee の正準形と等しいことを要求し、**すべて満たすときだけ**枝全体を `parser::canonical(scrutinee)` に差し替える 1 範囲を出す。判定できない形は畳まない（安全側）。

門番の実測（`bash scripts/verify-r68.sh`）:
```
OK: 68-a check --fix makes E512 accepted
OK: 68-b fmt does the same and is idempotent
OK: 68-c the value is unchanged
OK: 68-d the four shapes are left alone
OK: 68-e spec, trace, tier, corpus lock
PASSED: 5/5
```

残りは 2 つ。**仕様の 1 文**（§57 の書き換えの一覧に E512 を足す）と、**全走**（r68 を層に入れて緑に戻すことの確認）。この 2 つは次のラウンドの頭で行う。

### R68 の全走

`bash scripts/run-gates.sh --jobs 3 --tier full target/gates-full-r68` は `gates-exit=0`、`TALLY: 3685/3685 passed; red: none`。`RUN: verify-r68` が層に入り、r68 の 5 つの検査が 3680 から 3685 への増分そのものである。全走の後も木は綺麗（`git status --short` 空）。同じジョブで `cargo test --offline` も走らせた（結果はジョブの出力に続く）。これで R68 を閉じる。

### R68 の `cargo test`

`cargo test --offline` を**終了コードを保ったまま**回した（出力は `target/cargo-test-r68.log`）。`cargo-test-exit=0`、`test result: FAILED` の行は 0 本、全ターゲットの合計は `passed=773 failed=0`。前のラウンドで `| tail -4` に終了コードを食われて未確認だった点は、これで埋まった。R68 に未確認事項は無い。

### R69 の材料（残る 4 つの書き換え候補）

`docs/spec.md` のエラー表から、まだ書き換えの無い冗長は 4 つ。

| コード | 名前 | 節 | 対象 | 再現（拒否コーパス） |
|---|---|---|---|---|
| E502 | identical-branches | §42 | `if`。第32節の形では `def` か `fn` | `tests/reject/E502/main.hd` |
| E504 | let-question-underscore | §35 | `_` | `tests/reject/E504/main.hd` |
| E506 | implicit-discard | §40 | `_` | `tests/reject/E506/main.hd` |
| E511 | cond-to-literal-pattern | §26 | `if`。第32節の形では `def` か `fn` | `tests/reject/E511/main.hd` |

E502 の再現は `tests/reject/E502/main.hd` の中身そのもの（`// error: E502` のヘッダを持つ）。

```
def f x =>
  if x > 1 => 1 | 1
def main = f 1
```

腕と fallback の本体が同じで、正準形は分岐全体をその本体にすること。**枝の全体を 1 範囲で差し替える形なので、E507 の畳み込みと同じ道具（`branch_range(node, Tail::If, ...)` と `parser::canonical`、`Fix` の `line`/`col` は診断の位置）がそのまま使える。** 4 つのうち次に着手するのは E502 が最短（E504 と E506 は `_` の位置、E511 は腕の条件列に関わるので、対象条件の実測がもう少し要る）。

### R69 の赤の実測（E502）

拒否コーパスの再現を一時ディレクトリ（`target/e502-probe`、測定後に削除）に写して測った。

```
$ hado check target/e502-probe/main.hd
main.hd:4:3: error[E502]: identical branch results at 4:15 and 4:19
check-exit=1
$ hado check --fix target/e502-probe/main.hd
main.hd:4:3: error[E502]: identical branch results at 4:15 and 4:19
fix-exit=1
-- file after --fix --     4 行目は if x > 1 => 1 | 1 のまま
```

E502 はまだ書き換えを持たないので `--fix` は何も変えず、`check` は 1 のまま。診断には**同じ結果の 2 つの位置**（4:15 と 4:19）が関連位置として出ており、正準形のヒント（`; canonical form:`）は書き換えが無いため出ていない。R69 の門番はここを赤の起点にする（R68 の 68-a と同じく、「E502 が消えた」ではなく「直したファイルが `check` を通る」ことを判定に入れる）。

## R69 E502 の書き換え（2026-09-16、工程 1〜2）

### 背景と決定

同じ結果の腕と fallback（`if x > 1 => 1 | 1`）を共通の本体 1 つに書き換える。正準形は分岐全体をその本体にした形で、**枝の全体を 1 範囲で差し替える**だけで済む（E507 の畳み込みと同じ道具が使える）。条件を落として良いのは条件の評価が効果を持たないときだけなので、対象条件（仕様 §42 が許す形）を実測してから決める。

### 門番（工程 2）

`scripts/verify-r69.sh` を新設し、full 層に登録した。判定は「E502 が消えた」ではなく「**直したファイルが `check` を通る**」まで見る（69-a）。期待値は判定器に書かず、再現のソースから共通の本体を切り出して手書き版（`def f x => 1`）と実行結果を比べる（69-c）。実行は作業ディレクトリごとの一時ディレクトリで行う。

### 実測

- **実装前の赤:** `PASSED: 1/5`。69-a は `check --fix` が終了コード 1 のまま（`main.hd:4:3: error[E502]: identical branch results at 4:15 and 4:19`）、69-b は `fmt` の後も受理されず、69-c は比較に届かない。69-e は `## 64.` がまだ無いため赤（追跡表の行と層の登録は済み、corpus lock は緑）。
- **69-d は緑である。** まだ何も書き換えないので当然の緑で、実装の後も緑でなければならない（結果が違う形・コメントを含む形を書き換え始めたら赤になる）。
- 診断には同じ結果の 2 つの位置（4:15 と 4:19）が関連位置として出ており、正準形のヒントは書き換えが無いため出ていない（前ラウンドの実測）。

### 次の工程

- 工程 1 の残り: `docs/plan.md` に `## 64.` を書き、条件の効果の扱いを実測で決める（69-e の plan 検査はこれで緑になる）。
- 工程 3〜5: `fixes.rs` に枝全体を共通の本体へ差し替える 1 範囲を足す。仕様 §57 の一覧に E502 を足し、全走と `cargo test`。

### R69 の対象条件の実測（条件の効果と、同じ結果の判定）

一時ディレクトリで 2 つ測った（測定後に削除）。

```
$ hado check target/e502-effect/main.hd          # if g x > 1 => 1 | 1
main.hd:3:3: error[E502]: identical branch results at 3:17 and 3:21
call-scrutinee-exit=1
$ hado check target/e502-effect/par.hd           # if x > 1 => 1 | (1)
par.hd:2:3: error[E502]: identical branch results at 2:15 and 2:20
parenthesized-exit=1
```

- **条件が効果を持ち得ても E502 は出る。** scrutinee が関数呼び出しの形（`g x > 1`）でも出たので、checker は条件の効果を要求していない。したがって分岐全体を共通の本体に置き換える書き換えは、**条件の評価を落とすことになり得る**（`g x` が効果を持てば、その回数が変わる）。「効果を持ち得ない条件のときだけ書き換える」という対象条件が必須で、それ以外の形は正準形を示すだけにする。R68 の畳み込みを「scrutinee が単純な変数」に限ったのと同じ考え方で、今度は条件側の制限になる。
- **同じ結果の判定は字面ではなく正準形で行われる。** `1` と `(1)` でも E502 が出た。書き換えの共通本体は `parser::canonical` で作ればよい。

残る設計は「効果を持ち得ない条件」の構文的な判定である。名前・リテラル・それらの比較は安全側に置けるが、呼び出しを含む形は入れない。この判定を実測で詰めてから工程 3（`fixes.rs`）に入る。

### R69 の対象条件の構文（`NodeKind` の形）と、掛ける節

`NodeKind` の変種は 79〜125 行にあり、効果を持ち得ない条件の判定は次の形で安全側に書ける（呼び出し・フィールド・レコード・リスト・分岐・`let`・`?` はすべて含めない）。

```rust
fn is_effect_free(node: &Node) -> bool {
    match &node.kind {
        NodeKind::Int(_) | NodeKind::Float(_) | NodeKind::Char(_) | NodeKind::Str(_)
        | NodeKind::Bool(_) | NodeKind::Unit | NodeKind::Name(_) => true,
        NodeKind::Neg(inner) | NodeKind::Not(inner) => is_effect_free(inner),
        NodeKind::Bin(_, left, right) => is_effect_free(left) && is_effect_free(right),
        _ => false,
    }
}
```

書き換えはこの判定を通った条件のときだけ出す。共通の本体は `parser::canonical` で作り（`1` と `(1)` が同じ結果と見なされることを実測済み）、腕の本体と fallback のすべてが同じ正準形であることを要求する。範囲は `branch_range(node, Tail::If, …)` の 1 範囲、`Fix` の `line`/`col` は診断の位置。

**掛ける節が 1 つ違う。** E502 の再現は `if x > 1 => 1 | 1` で、条件が `x > 1` の**条件分岐**（`NodeKind::CondBranch`）である。`PatternBranch` だけを見ている今の walk では掛からないので、`CondBranch` の腕の形（条件の列・腕の本体・fallback のフィールド名）を 3 行ほど読んでから `fixes.rs` に入る。`Tail::If` は末尾位置の印なので両方に使える。

### R69 の実装の実測（掛からない）

`CondBranch` の形が分かった（`arms: Vec<(Node, Node)>` の条件と本体の対、`fallback: Option<Box<Node>>`）ので、`is_effect_free`、`identical_branches_fix`（条件がすべて効果を持ち得ないこと、腕の本体と fallback の正準形がすべて同じであること、`branch_range(node, Tail::If, …)` の 1 範囲）、`is_fixable` への `"E502"`、`CondBranch` の腕からの呼び出しを書いた。**ビルドは通った（exit 0）が、門番は `PASSED: 1/5` のままで、`check --fix` はファイルを変えない。** つまり書き換えが 1 つも出ていない。

見込みの原因は `branch_range` に渡す `branches`（`Ast.branch_spans`）に `CondBranch` の範囲が入っていないこと。`fixes.rs` が `CondBranch` を一度も名指ししていないのに対し、`branch_range` は node をその一覧と突き合わせて範囲を出すので、一覧に無ければ `None` が返り、書き換えは黙って出ない。次は「その一覧を作っている場所が `CondBranch` を記録しているか」を確かめるのが最初の 1 歩（`branch_spans` を作る側を grep する）。

**確認できないまま書き換えを commit しない**ため、`fixes.rs` は元に戻した（木は綺麗、門番は 1/5 のまま）。書いた関数は上の設計どおりで、掛ける場所（`branches` の一覧）が分かれば戻せる。

### R69 の掛からない原因の追測（`branch_spans` の作り手）

`branch_spans` を grep した。**書き込みは 1 箇所だけ**で、`parser.rs:1844` の `self.branch_spans.push((span, first_pipe, self.span()));`（枝の開始・最初の `|`・枝の終わり）。`first_pipe` を一緒に持つ形なので、条件分岐と pattern 分岐が共有する「論理枝」の解析で押されている可能性が高い。読む側は `fixes.rs:53` で `walk_item(..., &ast.branch_spans, ...)` に渡され、`functor_instances.rs:43` では 3 つの span を付け替えているだけである。

したがって**前ラウンドの見込み（`CondBranch` の範囲が一覧に無い）は弱くなった**。一覧に入っているのに書き換えが出ないなら、残る原因は `branch_range` 自身の突き合わせ方である（`Tail::If` と node の種類や span の位置で引いている）。次は `branch_range` の本体（`fixes.rs`、`TokenIndex` の近く）を 15 行ほど読み、`CondBranch` を引けるかを確かめる。引けないなら `CondBranch` 用に枝の範囲を node の span から直接出す道を選ぶ（共有の一覧の契約を壊さない）。

### R69 の掛からない原因の確定（`branch_range` と `parse_pattern_branch`）

`fixes.rs` の `branch_range`（825 行）は `let (_, _, after) = *branch_entry(node.span, branches)?;` で一覧を引き、`start = lines.offset(source, node.span)`、`end` は `Tail::Fn` のときだけ `)` を探して伸ばす。**`branch_entry` が `None` なら書き換えは出ない。**

その一覧を押しているのは `parser.rs:1844` の 1 行で、これは **`parse_pattern_branch` の中**にある。つまり **pattern 分岐のときだけ枝の範囲が登録され、条件分岐（`CondBranch`）は登録されない。** 前々ラウンドの「`CondBranch` の範囲が無いのが原因」はこれで**確定**した（間で「弱まった」と書いたのは、push の所在を確かめずに三つ組の形から推したためで、直接読んで訂正した）。

道は 2 つ。(a) 条件分岐の解析側にも同じ形で `branch_spans` を押す。この場合、`rewrap_if_fix`（E507）など範囲を引く側が条件分岐でも範囲を得られるようになるので、**既存の書き換えの振る舞いが変わる**（R57 の門番と全走で確かめる必要がある）。(b) `CondBranch` 用に枝の範囲を node の span から直接出す。共有の一覧に触れないが、終わりの求め方（本体の終わり）を自分で持つことになる。

次は (a) の影響範囲を先に測る（条件分岐の枝が範囲を持つと、どの検査・どの書き換えが変わるか）。(a) が安全なら (a)、(b) のほうが狭いなら (b)。

### R69 の道の選択（条件分岐の枝の範囲を登録する）

`branch_spans` / `branch_entry` / `branch_range` の読み手を grep した。**すべて `fixes.rs` の中**で、他のモジュールは読んでいない（`functor_instances.rs` は span を付け替えるだけ）。書き換え関数はそれぞれ自分の節で node の種類を縛っている。

- `rewrap_if_fix`（E507）は `let NodeKind::PatternBranch { fallback, .. } = &node.kind else { return None };` で **pattern 分岐を要求**するので、条件分岐の範囲が登録されても E507 は条件分岐に掛からない。
- `catch_all_fallback_fix`（E503/E512）は最後の腕の pattern を見るので同じく pattern 分岐が前提。
- つまり、条件分岐の枝の範囲を登録して新たに掛かり得るのは、**`CondBranch` を明示的に受け取る書き換えだけ**である。

したがって道は **(a) を選ぶ**: 条件分岐の解析の終わりにも `parse_pattern_branch` と同じ形で `self.branch_spans.push((span, first_pipe, self.span()));` を足す。影響は fixes の中に閉じ、`CondBranch` を受ける書き換え（E502）だけが新たに掛かる。全走と R57 の門番で、他の書き換えの件数が変わらないことを確かめる。

実装の手順: (1) 条件分岐の解析関数（`if 条件 => 本体 | 本体` を読む側）の終わりに push を 1 行足す。(2) 台帳に残した `is_effect_free` と `identical_branches_fix` を `fixes.rs` に戻す（`branch_range(node, Tail::If, …)` が今度は条件分岐でも範囲を返す）。(3) `is_fixable` に `"E502"`、walk の `CondBranch` の腕から呼ぶ。(4) 門番 5/5、`## 64.`、仕様 §57、全走。

### R69 の実装の再測（まだ掛からない）

条件分岐の解析（`parser.rs:1912` の `parse_cond_branch` と `parse_condition_column`）の `Ok(Node { kind: NodeKind::CondBranch … })` の直前に `self.branch_spans.push((span, self.span(), self.span()));` を足し、`is_effect_free`、`identical_branches_fix`、`is_fixable` の `"E502"`、walk の `CondBranch` の腕（`fixes.rs:984`、`PatternBranch` の腕の直前）を入れた。**ビルドは通り（exit 0）、walk の腕も所定の位置にあるのに、門番は `PASSED: 1/5` のまま**で書き換えが 1 つも出ない。

掛からない理由は `identical_branches_fix` の 4 つの門のどれか（(1) すべての条件が効果を持ち得ない、(2) 結果が 2 つ以上、(3) 正準形がすべて同じ、(4) `branch_range` が範囲を返す）か、そもそも walk がその node に来ていないかである。**推測を重ねずに計器を入れる**のが次の 1 歩: `identical_branches_fix` の入口と 4 つの門の直前に一時的な `eprintln!` を置いて 1 回走らせ、どこで `None` になるかを見る（分かったら消す）。この形なら 1 回の実行で 4 つの候補を切り分けられる。

確認できない書き換えを残さないため、`fixes.rs` と `parser.rs` は戻した（木は綺麗、門番は 1/5 のまま）。関数の設計は前のラウンドの台帳にある。

### R69 の計器の実測（書き換えは呼ばれていない）

`identical_branches_fix` の入口と 4 つの門の前に一時的な `eprintln!`（`P502 …`）を置き、条件分岐の枝の push も入れて、拒否コーパスの再現を一時ディレクトリに写して `check --fix` を直接走らせた（門番は stderr を捕まえるので、直接走らせないと計器が見えない）。

```
$ cargo run --offline -q -- check --fix target/e502-probe/main.hd 2>&1 | grep -E "P502|error"
main.hd:4:3: error[E502]: identical branch results at 4:15 and 4:19
```

**`P502 entry` が 1 行も出ない。** つまり `identical_branches_fix` は**一度も呼ばれていない**。4 つの門のどれかで `None` になっているのではなく、**walk がその node に来ていない**。読みで候補を絞るより計器が速いという判断は当たり、1 回の実行で 5 つの候補が 1 つに絞れた。

理由の見込み: 再現は `def f x => if x > 1 => 1 | 1` で、`def f x => …` は**アーム形**（`parse_pattern_branch` を `$argument` で呼ぶ形）なので、`if` はアームの本体として **`PatternBranch` の中に入れ子**になる。walk の `PatternBranch` の腕は書き換え関数を呼ぶだけで、**アームの本体へ降りていない**ため、入れ子の `CondBranch` に到達しない。E512/E503 が同じ位置で効いていたのは、あちらがその `PatternBranch` 自体に掛かる書き換えだったからである。

次の手は 2 つ。(a) walk がアームの本体へ降りるようにする（他の書き換えにも影響するので全走で確かめる）。(b) `PatternBranch` の腕から、各アームの本体が `CondBranch` のときに `identical_branches_fix` を呼ぶ（入れ子 1 段に限った狭い道）。`E502` は `def`/`fn` の本体にも直接書けるので、(a) のほうが本筋だが影響範囲を測ってから選ぶ。

計器と書き換えは元に戻した（木は綺麗、門番は 1/5 のまま）。

### R69 の計器の結果の確定（walk がアームの本体へ降りていない）

walk の分岐の腕（`fixes.rs` 915〜944 行）の末尾はこうなっている。

```rust
            for (patterns, _) in arms {
                for pattern in patterns {
                    walk_pattern(pattern, source, lines, tokens, index, found);
                }
            }
        }
        NodeKind::Fn { params, body } => {
```

**腕の本体（`_`）は捨てられ、pattern だけが降りられる。** 一方 `NodeKind::Fn { params, body }` の本体は降りられる。`def f x => if x > 1 => 1 | 1` はアーム形なので、`if` は `$argument` を scrutinee に持つ `PatternBranch` の**腕の本体**に入れ子になり、そこへ降りる道が無い。だから `identical_branches_fix` が一度も呼ばれない（計器の `P502 entry` が出なかったことと一致する）。`def f (t: T): T => if t | A => A | y => y` で E512/E503 が効いていたのは、あちらが `Fn` の本体として降りられる位置だったからである。

**道は (a) を選ぶ**: 分岐の腕でも本体を降りる（`for (_, body) in arms { walk(body …) }` に相当する再帰を足す）。これは「その枝に掛かる書き換え」ではなく「枝の中の式に掛かる書き換え」を新たに届かせるので、**既存の書き換えの件数が変わり得る**。R57 の門番（E507 の形）と全走で、意図した増分だけであることを確かめる。

実装の手順: (1) 分岐の腕でアームの本体も降りる（`fallback` も同じく降りる）。(2) 計器で `P502 entry` が出ることを確かめてから計器を外す。(3) `fixes.rs` の `is_effect_free` と `identical_branches_fix`、`is_fixable` の `"E502"`、`CondBranch` の腕、`parser.rs` の条件分岐の枝の push を入れる。(4) 門番 5/5、`## 64.`、仕様 §57、全走と `cargo test`。

### R69 の walk の構造（1 段だけ見る）

`Fn { params, body }` の腕（`fixes.rs:945`）も**本体へ降りていない**。`rewrap_arm_fix` と `repropagate_fix` をその `Fn` 自体に掛けるだけで、`body` を歩き直していない。腕の一覧は `_ => {}` で終わる（962 行）。pattern は `walk_pattern` で入れ子まで降りるが、**式は「渡された node 1 段」しか見ない**。入口は `walk_item`（459 行）で、`fixes()`（53 行）が項目ごとに呼ぶ。

これは設計として一貫している。書き換えは「その node 自体が冗長か」を見るもので、入れ子の式の冗長は**その式が walk に渡されたときに**掛かる。だから `def f (t: T): T => if t | A => A | y => y`（本体が `Fn` の本体 → 分岐が 1 段目）では E512 が掛かり、`def f x => if x > 1 => 1 | 1`（アーム形の腕の本体 → 分岐が 2 段目）では掛からない。

道 (a) の実装は、分岐の腕で `for (_, body) in arms` と `fallback` について**式の walk を再帰で呼ぶ**ことになる。呼ぶ関数の名前は `walk_item`（459 行）の本体が式の walk をどう呼んでいるかを見れば分かる（次の 1 歩）。再帰を足すと、腕の本体に入れ子になった**すべての**冗長が新たに掛かり得るので、R57 の門番と全走で件数の増分を確かめる（意図した増分は E502 の 1 件だけであるべき）。

### R69 の再測（腕の本体へ降りても呼ばれない）

`walk_node` の分岐の腕の末尾に `for (_, body) in arms { walk_node(body, source, lines, tokens, index, branches, found); }` と fallback への再帰を足し、計器付きで直接走らせた。**`P502 entry` はやはり 1 行も出ない**（ビルドは通っている）。

したがって「腕の本体に降りていない」という前ラウンドの見込みでは説明がつかない。残る候補は 2 つ。(1) この `def f x => …`（アーム形）の本体が `walk_node` に渡されていない（`walk_item` の入口の作り方）。(2) `if x > 1 => 1 | 1` が `CondBranch` 以外の種類で作られている。

**次は `walk_node` の入口で、訪れた node の種類を 1 行だけ印字する計器**を入れる（`std::mem::discriminant` か、`match` で種類名を返す小さな関数）。1 回の実行で候補が 2 つに絞れる。推測を重ねない。

計器と書き換えは戻した（木は綺麗、門番は 1/5 のまま）。

## R70 診断を JSON で読む小さな道具（優先順位 2 へ切り替え）

### 背景

目標の優先順位 2 は「`hado run` と安全な出力先を作り、実プログラムを一本運用する」。`--format json`（§58.4）は R65 で入っているので、次は**その JSON を実際に Hado のプログラムから読む**縦切りを作る。E502 は台帳に材料と設計を残したまま、いったん離れる（`verify-r69.sh` と赤もそのまま残す）。

### 実測（`--format json` の出力の形）

```
== tests/reject/E502/main.hd
{"path": "main.hd", "line": 4, "col": 3, "code": "E502", "message": "identical branch results at 4:15 and 4:19", "canonical": null, "related": null}
exit=1
== tests/reject/E511/main.hd
{"path": "main.hd", "line": 4, "col": 1, "code": "E511", "message": "use literal patterns for equality tests of one variable", "canonical": null, "related": null}
exit=1
```

この形を見てから、道具（`tools/` の下の Hado プログラム）の入力を決める。診断は 1 行 1 件の JSON なので、行に分けて読み、`"code"` を取り出して数える形が最小の縦切りになる。

### R70 の道具の下敷き（`tools/gates/main.hd`、23 行）

既に動いている道具から、使える API と書き方をそのまま取る。

```
1:def last_passed lines =>
2:  Option.ok_or
4:      if String.starts_with line "PASSED: " => Some(line) | selected))
7:def read_report directory name =>
8:  let path = String.concat directory (String.concat "/" name) ->
9:  let line = last_passed (String.split (Bytes.to_string (read_file path)) "\n") ->
10:  report::Report.parse_line (String.concat name (String.concat " " line))
12:def summarize directory =>
13:  List.fold (read_dir directory) report::Report.empty fn(tally, name =>
16:def choose (tally: report::Report.T): report::Report.T =>
20:def main =
```

読み取れること。ファイルは `read_file path` でバイト列として読み、`Bytes.to_string` で文字列にする。`String.split s "\n"`、`String.concat`、`String.starts_with` がある。`let x = e ->` の形で値を次へ渡し、分岐は `if 条件 => a | b`、集計は `List.fold list init fn(acc, x => …)` と `Option.ok_or`。モジュールは同じディレクトリの下に置く（`report::Report.parse_line` がその例）。

**道具の設計（この下敷きに合わせる）。** `tools/diagnostics/main.hd` は引数のファイルを `read_file` して行に分け、各行からコードを取り出して `List.fold` でコードごとの件数を数える。JSON は 1 行 1 件で `"code": "E502"` の形（前ラウンドの実測）なので、`String.split line "\"code\": \""` の 2 番目を、さらに `"` で割った先頭がコードになる。

**残る不明点は 1 つだけ。** `split` が返すリストから**要素を 1 つ選ぶ**書き方（`List` のどの関数を使うか、リストパターンが書けるか）がまだ分からない。次は `grep -n "def " stdlib/List.hd` で `at` / `nth` / `head` の有無を見る（1 回で決まる）。それ以外（読み・分割・集計・印字・呼び出し）は下敷きで分かっている。

### R70 の最後の不明点（`stdlib/list.hd`）

`stdlib/List.hd` ではなく **小文字の `stdlib/list.hd`** である（`array` / `bytes` / `env` / `error` / `float` / `int` / `io` / `list` / `option` / `string`）。

```
2:module List
3:def (A, B) map(values: A List, f: Fn(A => B)): B List
5:def (A, B) fold(values: A List, initial: B, f: Fn(B, A => B)): B
6:def A length(values: A List): Int Pure
10:def A head(values: A List): A Option Pure
11:def A nth(values: A List, index: Int): A Option Pure
15:def rec map values f => if values | [] => [] | [head, ...tail] => [f he
19:def rec length | [] => 0 | [_, ...tail] => 1 + length tail
```

**要素は `List.nth values index`（`Option`）か、リストパターン `| [head, ...tail]` で取れる。** 再帰は `def rec`。これで道具を書くのに足りないものは無い。

**道具の形（確定）。**
1. 引数のファイルを `Bytes.to_string (read_file path)` で読み、`String.split … "\n"` で行にする。
2. 各行を `String.split line "\"code\": \""` し、`List.nth … 1`（`Option`）→ それを `String.split … "\""` して `List.head` がコード。診断行でなければ飛ばす。
3. `List.fold` でコードごとの件数を積む（`Option` の値を扱うので `Option.ok_or` を下敷きと同じように使う）。
4. 合計と、コードごとの件数を印字する。

**まだ確かめていない 1 点**は、タプルのリスト（`(code, count)`）の作り方と、同値の比較（`==`）が使えるかどうか。どちらもコンパイルすれば分かる（通らなければ `String` の比較関数を `stdlib/string.hd` で見る）。次ラウンドはここから書く。

### R70 の道具の初稿と、コンパイラが教えた 2 つのこと

`tools/diagnostics/main.hd` の初稿を書いて `check` に掛けた（下敷きから写した API だけを使い、標準入力から JSON を読む形にした。argv の API を知らなくても、`hado check --format json f | hado run tools/diagnostics/main.hd` で繋げられるため）。

```
$ cargo run --offline -q -- check tools/diagnostics/main.hd
main.hd:4:13: error[E100]: expected a value type or annotated contract parameters
```
**E100**: `module Diagnostics` を書くと、そのファイルの `def` は型注釈が要る。下敷き（`tools/gates/main.hd`）が注釈なしで通っているのは `module` を書いていないからで、**道具は `module` 行を書かない**のが正しい（ファイル名が `main.hd` であることもあり、`module` 名と食い違う）。

```
$ cargo run --offline -q -- check tools/diagnostics/main.hd
main.hd:8:14: error[E105]: tuples are not supported; `(a, b)` is not an expression
```
**E105**: タプルは式として使えない。`(code, 1)` で数える設計はそのままでは書けない。**コードと件数の組はレコードか名前付き構築子にする**必要がある。下敷きの `tools/gates/report/report.hd` が `Report.T` という型で集計を持っているので、**その書き方を次に読んで写す**のが一番速い（同じ「集計を持つ小さな型」である）。

初稿は `tools/diagnostics/main.hd` に未追跡のまま置いた（次ラウンドでレコードに直して `check` を通してからコミットする）。台帳にはこの 2 つのエラーを実測として残す。

### R70 の道具が動いた（`E502 1`）

`tools/diagnostics/main.hd` は `check` を通り、実際に走った。

```
$ cargo run --offline -q -- check --format json tests/reject/E502/main.hd 2>&1 | cargo run --offline -q -- run tools/diagnostics/main.hd
E502 1
```

標準入力から JSON の診断を読み、コードごとの件数を出す。実プログラムを一本、`hado run` で運用する最初の縦切りである（優先順位 2）。

**コンパイラが教えたこと（すべて実測、初稿からの 5 つ）。**
1. `module` 行を書くと、そのファイルの `def` には型注釈が要る（E100）。道具は `module` を書かない。
2. タプルは式として使えない（E105）。コードと件数の組は持てないので、**数える側を「まだ印字していないコードか」で判定する**形にして、組そのものを不要にした。
3. **枝を枝の中に入れられない（E101）。** 枝ごとに関数を分ける（`summarize` / `summarize_one` / `emit` / `line_for`）。
4. 相互再帰は関係する関数すべてに `rec` が要る（E208）。
5. `let p? =` は `Option` 用（E303）。`{Error}` の効果は素の `let x = e ->` で通る。

この 5 つはどれも「書いてみるまで分からなかった」もので、目標の「run で不足を実測で発見する」がそのまま起きた形になっている。

## R70 の門番（工程 4 の実測）

`scripts/verify-r70.sh` を書いて回した。

```
OK: 70-a the tool passes check
OK: 70-b the tool's counts match the JSON for E502
OK: 70-c the tool's counts match the JSON for E511
OK: 70-d no diagnostics prints nothing
NG: 70-e tier, trace, plan, corpus lock (missing: tier trace plan)
PASSED: 4/5
```

70-e だけが赤で、これは層・系譜・手順書を書く前の実測（R68/R69 と同じ順序）。期待値は門番が JSON を python で独立に数えて導き、道具の出力と整列して比べる。**ファイル名も件数も判定器に書いていない。** 層 `full` への `r70` 登録は済み（70-e の不足が `tier trace plan` から `trace plan` に減って確認できた）。

**`docs/spec-trace.md` と `docs/plan.md` への追記は失敗した。** 前ラウンドで記録した §42 の行をそのまま锚に使ったが一致しなかった（空白か記号の差）。次は行の全文ではなく `verify-r69.sh` のような一意な部分文字列で探し、その行の直後に挿入する。`## 65.` も同時に書く。**木を中途半端に汚さないため、門番と層の登録だけをコミットし、残りは次の 1 回で書く。**

## R70 の締め（5/5）

`docs/spec-trace.md` に §58.4 の行を入れ（`verify-r69.sh` を部分文字列に持つ行の直後）、`docs/plan.md` に `## 65.` を書いた。1 ファイルずつ、失敗しても他を止めない形にしたのが効いて、両方とも 1 回で入った。

```
$ bash scripts/lint-docs.sh
PASSED: 7/7
$ bash scripts/verify-r70.sh
OK: 70-d no diagnostics prints nothing
OK: 70-e tier, trace, plan, corpus lock
PASSED: 5/5
```

R70 の成果: `tools/diagnostics/main.hd`（Hado で書いた、診断 JSON を標準入力から読みコードごとに数える道具）、`scripts/verify-r70.sh`（期待値を JSON から独立に導く 5 つの検査）、層 `full` への `r70` 登録、系譜 1 行、手順書 `## 65.`。優先順位 2 の「実プログラムを一本運用する」が、`hado run` で 1 本動くところまで来た。

## R70 の全走

```
TALLY: 3691/3695 passed; red: verify-r69.log
gates-exit=1
cargo test --offline: exit 0, passed=773 failed=0, "test result: FAILED" の行数=0
```

層別 full の赤は `verify-r69.log` **ただ 1 本**（Park 中の R69 が 1/5 で 4 件）。R70 の門番はこの層の中で走っており、赤の一覧に無いので通っている。`cargo test --offline` は終了コード 0、`test result: FAILED` の行は 0。

**R70 の締め。** 優先順位 2 の「`hado run` で実プログラムを一本運用する」が、`tools/diagnostics/main.hd`（診断 JSON を標準入力から読み、コードごとに数える）として 1 本動き、門番 5/5、層 `full` に登録され、系譜と手順書 `## 65.` が付いた。次の fix-it（E504 + E506）の材料もラウンド 65 で実測済み（正準形は `do e ->`、受理コーパスに `let _` は無い、仕様は §35 が 1235 行・§40 が 1474 行、エラー表は 2061 / 2063）。
