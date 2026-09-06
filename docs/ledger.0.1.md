# 実行台帳 v0.1

`docs/plan.0.1.md` を code-perfection-multiagent で実行した記録。ラウンド単位で追記する。

## 役割割当

| 役割 | エージェント | モデル | 実行方式 |
|---|---|---|---|
| commander | claude（本セッション） | claude-opus-5[1m] | — |
| planner | cursor-agent | cursor-grok-4.6-xhigh | headless（`-p --force --trust`）|
| plan-reviewer | opencode | opencode/deepseek-v4-flash | headless（`run --dir`）|
| builder | opencode | opencode/muse-spark-1.3-contributor-free | headless（Herdr ペイン内で実行）|
| reviewer | opencode | opencode/deepseek-v4-flash | headless（`run --dir` 使い捨てコピー）|
| fixer | opencode | opencode/muse-spark-1.3-contributor-free | builder と同系 |

ユーザー明示指定に従う。スキル既定からの逸脱と理由:

- planner を自分ではなく cursor に割り当てた（スキルは「planner は常に自分」）。ループの指揮・戻り先の最終判定・ラウンド分割は commander が握る。
- muse spark は `-contributor-free` の版を使う（ユーザー指定・2026-09-06）。R1 は `opencode/muse-spark-1.3` で実施済み。
- builder と reviewer が同じ CLI（opencode）。「別ベンダー」の文面には反するが、意図（盲点の非重複）はモデル系列が別（muse-spark 1.3 / deepseek v4 flash）で満たすと判断した。

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

