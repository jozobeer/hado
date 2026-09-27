# `verify-string-split-linear` の線形性の検査が高負荷で偽赤になる（flaky-string-split-timing-gate）
優先度: 中。

## 現象

`scripts/verify-string-split-linear.sh` の検査 e〜i（線形性）は、対照を引いた比を 1 回だけ測って 2.5 と比べる。計算機が混んでいると比が振れ、退行が無くても赤になる。main の CI で起きた（2026-09-27、PR #50 の着地 `1425789`）。

```
$ gh run view --job=108488487550 --log | grep -a TALLY
TALLY: 3748/3749 passed; red: verify-string-split-linear.log

$ tail -6 verify-string-split-linear.log
AssertionError: h: control-subtracted ratio 2.74 > 2.5 (small=0.216s large=0.591s)
NG: h perf: split with a growing Japanese separator is linear
PASSED: 9/10

$ bash scripts/verify-string-split-linear.sh   # 空いた計算機で
h: small=0.451s large=0.859s subtracted 0.271->0.509 ratio 1.88
OK: h perf: split with a growing Japanese separator is linear
PASSED: 10/10
```

同じ木を含む `14be875`（run 36272426720）と `3247613`（run 36281233911）の main の CI は success。着地の判定に使う `bash scripts/run-gates.sh --tier full` が偽赤を出すと、着地を止める規約（`docs/plans/issue-backlog.md` の停止条件 4）と最後の受け入れ条件が、実装と無関係に振れる。

## 原因

- `scripts/verify-string-split-linear.sh` の `assert_linear` は、`small` と `large` の 2 点を 1 回ずつ測り、`large` から `small` を引いた値の比を 1 つの閾値（`RATIO = 2.5`）と比べる。実測が 0.2〜0.6 秒の台に載ると、数 % の揺れが比では十数 % になり、閾値をまたぐ。
- 検査 i（join）は下限 0.200s を割ると赤になる形で、単独再測では緑に戻った（R2 のラウンドファイルの実測）。同じ性質。
- 検査 j（対照が issue の例を赤にする）は対照の設計を見る検査なので妥当。揺れるのは e〜i。

## 案

- 推奨: 各サイズを複数回測り、**比の中央値**で判定する（閾値 2.5 は据え置き。退行を測る力は落とさない）。下限（0.200s）で赤にする形も同じ中央値に揃える。
- 次点: `large` の入力を大きくして比の分解能を上げる（門番の所要時間が伸びる）。
- 退ける: 閾値を上げる。issue の例（`total ratio` が通す例）を検査 j が赤にできなくなる。

## 完了条件

- e〜i が、別プロセスで CPU を飽和させた状態でも緑になる（手元で再現して実測を残す）。
- 検査 j は引き続き赤（対照の設計が効いていること）。
- `bash scripts/run-gates.sh --tier full` が red なし。

## 由来

`docs/wants.md` になし。PR #50 の着地後の main の CI（run 36272319919）の赤の切り分けから。R1 の `docs/rounds/string-split-linear.md` の `## 実測` と、`docs/rounds/gate-optimize-flag.md` の着地後の項。
