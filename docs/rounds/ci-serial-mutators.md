# main の全走に見つかった 2 つの偽の赤を直す（ci-serial-mutators）

## 手順書

### 背景

`jobs.gates.timeout-minutes` を 360 分へ上げた後の main の push 走行 `38276533203`
（`7ee19a9`、full 層）は、打ち切りを解いたが、2 つの門番が赤で終わった。

| 門番 | 赤の理由 | 分類 |
|---|---|---|
| verify-clippy | `function comment_line is never used`（`crates/hado/src/postfix_print.rs:155`） | 偽の赤（木の競合） |
| verify-r57 / r62 / r63 | 綴り直し・`=>` の後のコメント・Boolean 腕の併合の検査が落ちる | 偽の赤（`verify-postfix-binding` の変異との競合） |
| verify-treesitter | `marks=[]`（検査 5） | 既知の flake（`docs/issues/flaky-treesitter-extmark-probe.md`） |

`comment_line` は同じファイルの 200 行目（`place_comments`）から使われている。赤は、
同じファンアウトで走る `verify-fmt-layout` が `crates/hado/src/postfix_print.rs` から
`comment_line` の唯一の使用を一時的に消して `cargo build` している窓に、
`verify-clippy` が重なって変異中の木を読んだためである。clean な木では clippy は
exit 0 で終わる（下の実測）。

同じ走行の後半で `verify-r57`・`verify-r62`・`verify-r63` も落ちた。これらは
`postfix_print.rs` と `parser.rs` の印字結果を測る門番で、`verify-postfix-binding` が
同じ 2 ファイルへ当てる変異（`drop-option-question` など）と重なって赤になっていた。
clean な木では 3 本とも緑である（下の実測）。

この競合は赤だけでなく時間も食う。`verify-m7`（core）の `cargo_cached test --offline
--no-fail-fast` は、m0 が公開した clean な指紋の全走ログに畳まれるはずが、fan-out の
変異で指紋が変わって実走し直していた。走行 `38276533203` では 46 分走っていた。
変異を直列へ移すと fan-out の指紋は clean に戻り、この実走は消える。

`hado_tier_serial_gates`（`scripts/lib/gate-tiers.sh`）は「共有ツリーを書き換える
門番をファンアウトの前段で直列に完走させる」一覧である（`docs/plan.md` 34 節）。
ところが `crates/hado/src` を書き換える門番は 6 本あるのに、一覧には 1 本も無かった。

### 決定

- `hado_tier_serial_gates` に、`crates/hado/src` を一時的に書き換えて `cargo build` する
  6 本を足す: `fmt-layout`・`localize-message-positions`・`restore-option-question`・
  `pure-closure-in-type-variable-argument-fails-specialization`・
  `recursive-effectful-closure-specialization-reads-outcome-as-int`・`postfix-binding`。
  書き換えは fan-out の前に直列で完走し、同じ木を読む門番と重ならない。
- `scripts/treesitter-hl.lua` は `redraw!` の完了（`nvim__redraw({ flush = true })`）を
  待ってから記録を読み、`marks <全記録数> <配置数>` を出す。`scripts/treesitter-check.py`
  は 0 件を件数つきで「探針が置かなかった」と赤にする。待っても 0 件なら本物の欠陥である。
- 却下: 全門番の直列化。並列の利点を失う。
- 却下: 変異門番を private な木の複製で走らせる。cold build が高く、門番の所要が増える。

### 期待値の根拠

- 一覧を増やすのは fan-out の前段で走る門番だけで、層と判定条件は変えない。
  `verify-gate-cache.sh` は一覧が実在の門番の部分集合であることを既に機械で見る。
- 探針の待ちは `redraw!` が予約した描画の完了を観測するもので、assert を弱めない。
  0 件なら従来どおり赤である。

### 成果物

- `scripts/lib/gate-tiers.sh` — 直列の一覧。
- `scripts/treesitter-hl.lua` — 描画の完了待ちと件数の出力。
- `scripts/treesitter-check.py` — 0 件の理由と件数。
- `docs/rounds/ci-serial-mutators.md` — 本ファイル。

### 工程

1. 赤の走行のログと手元の再現で、赤を偽の赤と flake に分類する。
2. 6 本を直列の一覧へ足す。
3. 探針を描画の完了待ちへ直し、件数を出す。
4. `verify-gate-cache.sh`・`verify-treesitter.sh`・`lint-docs.sh` を回す。
5. main へ着地し、full 層の走行が red なしで終わることを見る。

### 判定

- clean な木で `cargo clippy --offline --all-targets -- -D warnings` が exit 0。
- `bash scripts/verify-gate-cache.sh` が `PASSED: 26/26`。
- `bash scripts/verify-treesitter.sh` が `PASSED: 5/5`（CPU を飽和させた状態でも）。
- main の push 走行が `TALLY: n/n passed; red: none` と文書層の `PASSED:` を出す。

### 許容される赤

なし。

### スコープ外

- full 層の所要の短縮（private な木、cache、runner の増設）。
- 層と門番の判定条件。

### 中断の条件

直列へ足しても競合が残るなら、変異門番を private な木で走らせる設計へ切り替える。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| 34 | 層別の並列は、共有ツリーを書く門番を fan-out の前段で直列に完走させる | 道具 | 成功 | `scripts/lib/gate-tiers.sh` |

## 実測

診断（2026-10-12）。走行 `38276533203` の赤（fan-out の途中）。

```
$ docker exec hado-ci-docker grep -H 'NG:' /home/runner/_work/hado/hado/target/ci-gates/*.log
verify-clippy.log:NG: cargo clippy --offline --all-targets -- -D warnings is clean
verify-treesitter.log:NG: Neovim marks each promised token with its capture group (no @comment mark at 0:0-16 ('// comment 12345'); marks=[])
```

clean な木の clippy。

```
$ cargo clippy --offline --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.96s
CLIPPY_EXIT=0
```

着地前の検査（treesitter は load `17.77` の下で回した）。

```
$ bash scripts/verify-gate-cache.sh | tail -1
PASSED: 26/26

$ bash scripts/verify-treesitter.sh | tail -4
OK: Neovim marks each promised token with its capture group
PASSED: 5/5
corpus files: 181
goldens: 47
extmarks: 39

$ cat /proc/loadavg
17.77 17.21 13.29 21/2263 3
```

clean な木の `verify-r57`・`verify-r62`・`verify-r63`。

```
$ bash scripts/verify-r57.sh | tail -1
PASSED: 6/6
$ bash scripts/verify-r62.sh | tail -1
PASSED: 4/4
$ bash scripts/verify-r63.sh | tail -1
PASSED: 4/4
```

走行 `38276533203` の fan-out で実走し直していた `verify-m7` の全走。

```
$ grep -n cargo scripts/verify-m7.sh
89:if cargo_cached test --offline --no-fail-fast; then ...

$ docker exec hado-ci-docker ps -eo etime,args | grep 'verify-m7'
      46:15 bash scripts/verify-m7.sh
      46:15 ... cargo test --offline --no-fail-fast
```
