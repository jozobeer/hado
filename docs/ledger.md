# 実行台帳 v0.1

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
