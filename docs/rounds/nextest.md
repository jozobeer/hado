# 門番の全体テストを nextest で走らせる（nextest）

## 手順書

### 背景

`cargo test` はテスト二進を 1 本ずつ回すので、全体の所要は二進ごとの最長のテストの和になる（`docs/plans/test-speed.md` の原因 3）。nextest はテスト単位で二進をまたいで並べるので、所要は最長のテスト 1 本に近づく。

f546cfa で全体テストを両方で 1 回ずつ回した（同じ機械で自前 runner が 2 プロセス動いていた）。

| コマンド | 所要 | 結果 |
|---|---|---|
| `cargo test --offline --no-fail-fast` | 89 秒 | 854 passed（75 二進） |
| `cargo nextest run --offline --no-fail-fast` | 45 秒 | 854 passed |

doctest は 0 件（`Doc-tests hado` が `running 0 tests`）なので、nextest が doctest を走らせないことで落ちるテストは無い。最長は `hado::corpus corpus` の 37 秒である。

全体テストは `verify-m0`・`verify-m2`・`verify-m3`・`verify-m7` が同じ `cargo test --offline --no-fail-fast` を `cargo_cached` で呼び、1 回の走行で 1 度に畳まれている。4 本は libtest の人向けの出力（`test result:`、`Running tests/<名>.rs`、`test <名> ... ok`）を読んで判定している。

### 決定

- 全体テストの入口を `scripts/nextest-full.sh` にする。`.config/nextest.toml` の profile `gate` で走らせ、JUnit からテストごとに `nextest-case<TAB>PASS|FAIL|SKIP<TAB><二進 ID><TAB><テスト名>` の行を出す。門番はこの行だけを読む。
- nextest は `scripts/nextest-setup.sh` が版 0.9.148 を GitHub の digest で照合して `target/nextest-setup` に取ってくる。PATH の `cargo-nextest` は使わない。`scripts/treesitter-setup.sh` と同じ形で、CI のワークフローは変えない（門番が `curl` で取る）。
- `scripts/lib/gate-cache.sh` に `nextest_full_cached` を足し、m0・m2・m3・m7 はこれを呼ぶ。同じ id なので 1 回の走行で実走は 1 度である。
- m0 の検査 8 の `not-run` と `empty` は `not-run-or-empty` に併せる。JUnit は 0 件の二進のテストスイートを出さないので、2 つを区別できない。どちらも赤であることは変わらない。
- 却下: m0 だけを移す。m2・m3・m7 が `cargo test` の全走を残すので、全体テストが 2 回走る。
- 却下: nextest の人向けの出力を読む。版で形が変わる。JUnit は nextest の安定した出力である。
- 却下: `cargo install cargo-nextest`。走行ごとにコンパイルが要る。
- 却下: `--message-format libtest-json`。nextest の実験機能で、環境変数での有効化が要る。

### 期待値の根拠

- `nextest-case` 行は JUnit の `testcase` 1 件に 1 行で、`failure` か `error` の子があれば FAIL、`skipped` なら SKIP、どちらも無ければ PASS である。`scripts/verify-nextest.sh` が stub の junit から期待値を決めて測る。
- 走らせる前に junit を消すので、書かれなかった走行で前の走行の結果を読まない。nextest が 0 で終わっても junit が無ければ赤にする。
- m0 の件数・walk・tail・対象の二進、m2 の `parser::tests::left_assoc_add_and_mul`、m3 の HM のテスト名は、libtest の出力で見ていたものを `nextest-case` 行で見る。判定の条件は変えない。

### 成果物

- `.config/nextest.toml`
- `scripts/nextest-setup.sh`・`scripts/nextest-full.sh`
- `scripts/lib/gate-cache.sh` の `nextest_full_cached`・`scripts/gate-cache-run.sh` の `--nextest-full`
- `scripts/verify-m0.sh`・`scripts/verify-m2.sh`・`scripts/verify-m3.sh`・`scripts/verify-m7.sh`
- `scripts/verify-nextest.sh`（quick）
- `AGENTS.md`・`CONTRIBUTING.md` のコマンド
- `docs/issues/gate-cache-libtest-evidence.md`
- 本ファイル

### 工程

1. `cargo test` と nextest で全体テストの所要と件数を測る。
2. 入口と取得の script を書き、`bash scripts/nextest-full.sh` で 854 件が `nextest-case` 行になることを見る。
3. m0・m2・m3・m7 を `nextest_full_cached` へ移し、同じ走行スコープで 4 本を回して実走が 1 回であることを見る。
4. `verify-nextest.sh` を書き、実装を壊して赤になることを見る。失敗を返す stub で m0・m2・m3・m7 が赤になることを見る。
5. quick と core の層を回し、PR の CI の所要を前の PR と比べる。

### 判定

- `bash scripts/verify-nextest.sh` が `PASSED: 6/6`。
- 同じ走行スコープで m0 `8/8`・m2 `8/8`・m3 `11/11`・m7 `2/2`、`HADO_GATE_RUNS` の `nextest-full` の `run` が 1 行。
- core 層の `TALLY` に赤が無い。

### 許容される赤

なし。

### スコープ外

- `cargo test --test <名>` を直接呼ぶ門番（`verify-r11`・`verify-r12`・`verify-postfix-binding`・`verify-localize-message-positions`・`verify-string-split-linear`）。部分テストは二進 1 本なので、nextest の並べ方の利点が小さい。
- 全走の証拠で部分テストを省く仕組みの片付け（`docs/issues/gate-cache-libtest-evidence.md`）。
- `target/` のキャッシュ、Rust の版の固定と更新。

### 中断の条件

nextest と `cargo test` で件数か結果が食い違えば、移さずに食い違いを報告して止める。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | 全体テストは固定した版の nextest で 1 回の走行に 1 度走り、門番は JUnit 由来のテストごとの行で判定する | 道具 | 成功 | `scripts/verify-nextest.sh` |

## 実測

2026-10-13、f546cfa の上。全体テストの所要（同じ機械で自前 runner が動いていた）。

```
$ cargo test --offline --no-fail-fast
cargo test rc=0 secs=89
$ cargo nextest run --offline --no-fail-fast
nextest rc=0 secs=45
     Summary [  44.086s] 854 tests run: 854 passed, 0 skipped
```

入口の判定器と、壊した実装での赤（1 行目が緑、以下は各改変での出力）。

```
$ bash scripts/verify-nextest.sh | tail -1
PASSED: 6/6
$ # junit を消さない
NG: a stale junit is not read as the result of a run that wrote none
$ # error を PASS に読む
NG: junit cases become nextest-case lines with the nextest status
$ # verify-m7 を cargo_cached test --offline --no-fail-fast に戻す
NG: no gate runs the full suite through cargo test
```

同じ走行スコープで 4 本を回す（各行は門番の終了コード・所要・`PASSED` 行）。nextest の実走は 1 回で、残りは再利用。

```
$ for g in m0 m2 m3 m7; do bash scripts/verify-$g.sh; done
m0 rc=0 secs=54 PASSED: 8/8
m2 rc=0 secs=26 PASSED: 8/8
m3 rc=0 secs=8 PASSED: 11/11
m7 rc=0 secs=0 PASSED: 2/2
$ cut -f1,2 "$HADO_GATE_RUNS" | sort | uniq -c | grep nextest
      3 hit	nextest-full
      1 run	nextest-full
```

walk だけが FAIL で他の二進が無い junit を返す stub（終了コード 100）で回す。

```
$ HADO_NEXTEST_BIN=stub bash scripts/verify-m0.sh
NG: 2 nextest status=100 passed=0 (<2 or nonzero)
NG: 3 walk: walk-test-not-passing
NG: 4 tail: tail-test-not-passing
NG: 8 test-targets: not-run-or-empty-named_record_constructors ...
PASSED: 4/8
$ HADO_NEXTEST_BIN=stub bash scripts/verify-m2.sh
NG: 7 nextest+assoc: nextest-exit!=0 associativity-probe-not-run
$ HADO_NEXTEST_BIN=stub bash scripts/verify-m3.sh
NG: 8 glue+unit-hm
$ HADO_NEXTEST_BIN=stub bash scripts/verify-m7.sh
PASSED: 1/2
```
