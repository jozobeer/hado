# 部分テストを全走の証拠で省く仕組みに証拠の出し手がいない（gate-cache-libtest-evidence）
優先度: 低。

## 現象

全体テストを nextest で走らせた後でも、同じ走行の `verify-r11` は部分テストを実走する。

```
$ export HADO_GATE_CACHE_RUN=r11-probe HADO_GATE_RUNS=/tmp/r11.runs
$ bash scripts/verify-m7.sh | tail -1; bash scripts/verify-r11.sh | tail -1; cut -f1-2 /tmp/r11.runs
PASSED: 2/2
PASSED: 1/1
run	nextest-full
run	cargo:test --offline --test r11_comments
```

## 原因

`scripts/lib/gate-cache.sh` の `cargo_test_cached` は、`cargo_cached` が `hado_gate_is_full_test` の形（`cargo test --offline --no-fail-fast`）で残した全走ログに対象の `Running` と ok があるときだけ実走を省く。全体テストは `nextest_full_cached` に移り（`docs/rounds/nextest.md`）、その形で全体テストを走らせる門番は無い（`scripts/verify-nextest.sh` の `no_libtest_full_run`）。証拠のログが作られないので、省く規則は働かず、仕組みと `scripts/verify-gate-cache.sh` の `narrow_reuse_case` などの検査だけが残っている。赤にはならない（実走に倒れる）。

## 案

- 推奨 A: 証拠の仕組みを消し、`verify-r11.sh` と `verify-r12.sh` は `cargo_cached` で部分テストを走らせる。`scripts/verify-gate-cache.sh` の該当の 3 検査も消す。
- 退ける B: `nextest-case` 行を証拠として読むよう `cargo_test_cached` を書き換える。部分テストを省くために、全体テストの結果の読み方を 2 か所に持つことになる。
- 退ける C: `verify-r11.sh` と `verify-r12.sh` を消し、対象の存在は `verify-m0.sh` の検査 8 に任せる。2 本が守る対象の名前が m0 の固定の組に無い。

## 完了条件

- `scripts/lib/gate-cache.sh` に全走ログを証拠に読む関数が無く、`scripts/verify-gate-cache.sh` が緑。
- `verify-r11.sh` と `verify-r12.sh` が緑。

## 由来

`docs/rounds/nextest.md` の着地で証拠の出し手が無くなった。
