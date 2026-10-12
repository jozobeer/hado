# `verify-string-split-linear` の検査 i の床を時計の分解能から導く（timing-floor）

## 手順書

### 背景

main（`3723b9a`）の full 全走は `verify-string-split-linear` の検査 i（join）だけが
赤い（`docs/rounds/full-tier-reds.md`）。`small median 0.182s is below the 0.200s floor`
で、比は 1.9〜2.0 である。線形であることを正しく判定しているのに赤い。

赤は `assert_linear` の床 `small_total >= SMALL_FLOOR`（`SMALL_FLOOR = 0.2`）が、
比の分母ではなく**目標時間の秒の絶対値**を見ているためである。機械が速いほど目標
時間が縮み、退行が無くても割れる。`docs/issues/flaky-string-split-timing-gate.md`
が原因を特定している（高負荷ではなく、床が機械の速さに依存すること）。

### 決定

- 床を比の分母（対照を引いた差 `small`）に移し、**時計の分解能の tick 数**で表す。
  `TIMER_RESOLUTION = 1e-6`（`resource.getrusage` はマイクロ秒）、
  `SMALL_FLOOR_TICKS = 1000`（1 ms）。秒の絶対値も目標時間も式に入らない。
- 比の判定は変えない。`RATIO = 2.5`、warm-up 1 回 + 5 組、対照減算、組ごとの差の
  中央値。線形性の判定は比が持ち、床は「分母が分解能に埋もれない」ことだけを測る。
- 却下: 床を対照の測定値に対する比にする案。split は対照（入力の読み込みと
  `String.length`）が重く、差が対照より小さい（実測で e は 0.24 < 0.34、f は
  0.13 < 0.28）。対照に対する比では split と join で意味が揃わない。
- 却下: 床を「差が複数回のばらつきを上回ること」にする案。e/f の差は同じ機械でも
  揺れの 3.5〜4.2 倍しかなく、負荷時に新しい偽赤を作る（実測）。
- 却下: `RATIO` を上げる案。issue の例を通してしまい、検査 j が較正を失う。
- 却下: 検査 i を消す案。検査 i は §55 の「`String.join` は定数回の走査」を測って
  おり、二乗の実装で赤になる（実測）。

### 期待値の根拠

- §55: 「`String.length` は O(1) とする。`String.split` と `String.join` は、入力の
  長さに対して定数回の走査で行う。」
- 線形なら入力を 2 倍にして比は 2 に近づき、二乗なら 4 に近づく。閾値 2.5 は元の
  門番のまま据え置く。
- 床は分母が時計の分解能に埋もれないことだけを測る。`getrusage` の量子は 1 µs、
  引き算どうしの量子は 2 tick なので、1000 tick（1 ms）は 3 桁上にある。実測した
  分母は 0.13〜0.32s（1.3e5〜3.2e5 tick）で 2〜3 桁上にある。

### 成果物

- `scripts/verify-string-split-linear.sh` — `assert_linear` の床
  （`TIMER_RESOLUTION`・`SMALL_FLOOR_TICKS`）。
- `docs/rounds/timing-floor.md`。
- `docs/issues/flaky-string-split-timing-gate.md` — 削除。

### 工程

1. main で赤を再現し、検査 i の値を記録する。
2. 分母と揺れを実測し、床の桁を決める。
3. 床を分解能の tick 数へ直す（`test:`）。
4. 二乗の `join`（`docs/rounds/string-split-linear.md` の変更前の prelude 形）を
   一時的に当て、検査 i が赤になることを確かめる。
5. 元に戻し、静かな機械で 3 回、CPU 飽和で 1 回回す。
6. quick と full の層別を回す。

### 判定

- `bash scripts/verify-string-split-linear.sh` が 3 回とも `PASSED: 11/11`。
- 二乗の `join` で `PASSED: 10/11`（検査 i が赤）。
- CPU 飽和でも `PASSED: 11/11`。
- `bash scripts/run-gates.sh --tier quick` が赤なし、`--tier full` が赤なし。

### 許容される赤

無し。ただし、同じ計算機で他の重い作業（とくにメモリ帯域を使うもの）が走って
いると、比の判定（e・f・h）が揺れて赤になることがある（実測に 2 例）。これは床
ではなく比の性質で、`AGENTS.md` の「静かな機械で測る」で扱う。CPU を飽和させた
だけでは赤にならない（実測）。

### スコープ外

- 比の判定の作り直し（`RUNS` の増加・小と大の交互測定）。issue の案は閾値 2.5 の
  据え置きを決めており、`verify-map-by-builtin-compare.sh` と同じ形を保つ。
- `verify-cranelift-opt-level.sh` の `SPLIT_FLOOR`。

### 中断の条件

- 分母が分解能に対して測れない（床を割る）とき。
- 二乗の `join` で検査 i が赤にならないとき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §55 | `String.length` は O(1) | 実行 | — | `scripts/verify-string-split-linear.sh` 検査 b |
| §55 | `String.split` と `String.join` は定数回の走査 | 実行 | — | `scripts/verify-string-split-linear.sh` 検査 e〜i |

## 実測

### 赤（main、`docs/rounds/full-tier-reds.md` の記録）

```
$ sed -n '/split ascii lines/,$p' target/gates-full/verify-string-split-linear.log
split ascii lines: small=0.492s large=0.974s subtracted 0.192->0.396 ratio 2.06
split multibyte lines: small=0.363s large=0.721s subtracted 0.109->0.215 ratio 1.97
g: small=0.381s large=0.739s subtracted 0.234->0.455 ratio 1.95
h: small=0.348s large=0.699s subtracted 0.176->0.350 ratio 1.99
AssertionError: join: small median 0.182s is below the 0.200s floor
NG: i perf: join is linear in the element count
PASSED: 10/11
```

### 分母と揺れ（床の桁を決める実測）

各サイズの分母（対照を引いた差）と、組ごとの差の散らばり（5 組の最大−最小）を
別の探針で測った。分母は 0.13〜0.26s で、揺れは 4〜57 ms だった。`getrusage` の
量子は 1 µs なので、1000 tick（1 ms）の床は分母より 2 桁以上小さい。

### 直した門番（静かな機械で 3 回）

load average 1.5 の静かな機械で 3 回連続して回した。3 回とも `join` の目標時間は
旧い床 0.2s を割り、直した門番は比 1.97〜2.03 で緑である。

```
$ bash scripts/verify-string-split-linear.sh
join: small=0.174s large=0.356s subtracted 0.161->0.327 ratio 2.03
PASSED: 11/11
$ bash scripts/verify-string-split-linear.sh
join: small=0.183s large=0.360s subtracted 0.169->0.333 ratio 1.97
PASSED: 11/11
$ bash scripts/verify-string-split-linear.sh
join: small=0.179s large=0.357s subtracted 0.166->0.329 ratio 1.98
PASSED: 11/11
```

`small=0.174`・`0.183`・`0.179` はどれも `0.2` 未満である。旧い門番
（`small_total >= 0.2`）はこの 3 回すべてで赤くなり、直した門番は緑になる。

### 対象を壊すと赤（二乗の join）

`stdlib/string.hd` の `join` を `docs/rounds/string-split-linear.md` の変更前の
prelude 形（`concat acc (concat head separator)` を畳む）へ一時的に戻すと、検査 i
だけが赤になる。意味の検査 a は緑のままである。

```
$ bash scripts/verify-string-split-linear.sh
OK: a semantics: split and join equations and EmptySeparator
OK: b length: String.length scans no bytes and does not slow with length
OK: c layout: runtime unit tests after the HadoString header migration
OK: d abi: every HadoString *-returning function classified and measured
OK: e perf: split on ASCII lines is linear
OK: f perf: split on Japanese lines is linear
OK: g perf: split with a growing ASCII separator is linear
OK: h perf: split with a growing Japanese separator is linear
subprocess.TimeoutExpired: Command '[.../i/target-small/program']' timed out after 10.0 seconds
NG: i perf: join is linear in the element count
OK: j control: the subtracted ratio judge rejects the issue example
OK: k lengths: R1 leftovers Float.to_string 0.1 and Char.to_string あ
PASSED: 10/11
```

### CPU 飽和（遅い機械）

別プロセス 12 本で CPU を飽和させて回すと、CPU 時間は約 2 倍になるが、比は変わらず
緑である。

```
$ bash scripts/verify-string-split-linear.sh
split ascii lines: small=1.017s large=2.038s subtracted 0.444->0.870 ratio 1.96
split multibyte lines: small=0.812s large=1.598s subtracted 0.268->0.522 ratio 1.94
g: small=0.776s large=1.553s subtracted 0.501->0.992 ratio 1.98
h: small=0.721s large=1.455s subtracted 0.352->0.687 ratio 1.95
join: small=0.317s large=0.637s subtracted 0.286->0.580 ratio 2.03
PASSED: 11/11
```

### 層別

quick。

```
$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 51/51 passed; red: none
```

full（1 回）。

```
$ bash scripts/run-gates.sh --tier full target/gates-full
TALLY: 3894/3894 passed; red: none
$ grep -E '^(join|split|g:|h:)|^PASSED' target/gates-full/verify-string-split-linear.log
split ascii lines: small=0.527s large=1.024s subtracted 0.215->0.428 ratio 1.99
split multibyte lines: small=0.387s large=0.744s subtracted 0.124->0.225 ratio 1.82
g: small=0.386s large=0.744s subtracted 0.240->0.452 ratio 1.88
h: small=0.364s large=0.714s subtracted 0.178->0.348 ratio 1.95
join: small=0.180s large=0.362s subtracted 0.166->0.334 ratio 2.01
PASSED: 11/11
```

full 全走の `join: small=0.180s` も旧い床を割る値である。全走そのものが、直した
門番で緑になることを示している。

### 負荷時の比の揺れ（床とは別）

同じ計算機で他の重い作業が走っているとき、比の判定が揺れて赤になる例を 2 つ
観測した。どちらも分母（差）は分解能の上にあり、床ではなく比の揺れである。

```
$ bash scripts/verify-string-split-linear.sh
AssertionError: split multibyte lines: control-subtracted ratio 3.70 > 2.5 (small=0.136s large=0.503s)
NG: f perf: split on Japanese lines is linear
PASSED: 10/11
```

```
$ bash scripts/verify-string-split-linear.sh
AssertionError: h: control-subtracted ratio 3.46 > 2.5 (small=0.190s large=0.658s)
NG: h perf: split with a growing Japanese separator is linear
PASSED: 10/11
```

### 木

変異は残っていない。

```
$ git status --short
$ git log --oneline -3
3482021 test: 検査 i の床を目標時間の絶対値から時計の分解能へ
3723b9a Merge pull request #88 from jozobeer/full-tier-reds
8edf94b Merge main into full-tier-reds
```

### 着地後の全走（2026-10-13、main `b6f7126`）

main を checkout した側（`.worktrees/postlanding-main`）で回した。full 層（quick・
core・full の累積に文書層を足した 84 本）は `TALLY: 3894/3894 passed; red: none` で
赤は 1 本も無い。`cargo test --offline` は full 層の門番 `verify-m0` の検査 2 が
`cargo-test passed=850` で緑である（終了コード 0、`FAILED` は 0）。quick 層の門番
6 本は full 層の走行に含まれ、すべて緑である。

```
$ bash scripts/run-gates.sh --tier full target/.gates-full | tail -1
TALLY: 3894/3894 passed; red: none
$ ./target/debug/hado run tools/gates/main.hd -- target/.gates-full
3894/3894 passed; red: none
$ grep cargo-test target/.gates-full/verify-m0.log
OK: 2 cargo-test passed=850
$ for f in verify-ci verify-gate-cache verify-m1-spec verify-repl verify-rustfmt verify-spec-model; do tail -1 target/.gates-full/$f.log; done
PASSED: 8/8
PASSED: 26/26
PASSED: 1/1
PASSED: 10/10
PASSED: 2/2
PASSED: 3/3
$ tail -1 target/.gates-full/verify-string-split-linear.log
PASSED: 11/11
```

`join: small` は着地後の full 全走でも床（1 ms）を割るが、床が時計の分解能から導かれるので検査 i は緑である。
