# `verify-r34` の 34-c(3) の上限を同じ要素数の対照に対する比へ移す（r34-timing）

## 手順書

### 背景

`docs/plans/issue-backlog.md` の受け入れ条件は「main で `bash scripts/run-gates.sh --tier full` が red なし」である。main `2abdec1` の full 全走は `verify-r34` の 2 本だけが赤い（`PASSED: 1358/1360`）。

- `NG: 34-c(3) 3 万要素の of_list が 2 秒以内に一致`
- `NG: 34-c(3) 3 万要素の to_list の長さ・先頭・中間・末尾が 2 秒以内に一致`

同じ木で `verify-r34` を単独に回すと `PASSED: 1360/1360` である。負荷で 2 秒を超えるだけで、実装の欠陥ではない。`verify-string-split-linear` の床（`docs/rounds/timing-floor.md`）と同じ設計の問題である。

### 検査 34-c(3) が守っているもの

3 万要素の `Bytes.of_list` / `Bytes.to_list` が、要素数に対して定数回の走査で行われること（r34 が選んだ `__bytes_alloc` + `__bytes_fill_unchecked` の O(n) 構築が、1 要素ごとに全体を複製する O(n²) の構築へ戻らないこと）を、値の一致（打ち切りなし）と合わせて守っている。仕様本文に `Bytes` の計算量の文は無く、根拠は `docs/rounds/r34.md` の決定（`Bytes.of_list` の構築の形 (a) か (b)）である。

### 上限が守っていたもの（と、守れていなかったもの）

旧い上限「build + run が 2 秒以内」は O(n²) への退行で赤くなることだけを狙っていた。しかし実測では、線形の実装で build が約 1.05s、run が約 6ms である（静かな全走の `build+run=1081ms` と、直した検査が測る対象の run `target=0.0053s` の差）。時間のほぼ全部が hado の build（C のリンカ呼び出しを含む）で、上限は `of_list` の計算量ではなくビルド時間を測っていた。負荷時は build だけで 2 秒を超え、`(c3: build timed out or failed)` になる。静かな全走でも build+run は 1081ms / 1095ms で、2 秒までの余裕は 2 倍しかなかった。

### 決定

- 線形性の判定を、同じ 3 万要素を線形に 1 回走査する対照（生成器 + `List.length`）と、対象（同じ生成器 + `of_list` / `to_list`）の**子プロセスの CPU 時間（user + sys）**の比へ移す。対照と対象を 1 回ずつ交互に測り、組ごとの比の中央値が `RATIO_MAX = 8.0` 以下であることを要求する。比は同じ機械で同じ負荷の下で取るので、機械の速さと負荷に依らない。CPU 時間なので順番待ちの時間も入らない。
- 値の一致（長さ・先頭・中間・末尾、打ち切りなし、stderr 空、終了コード 0）は同じ検査の中で照合する。絶対時間の上限は置かない。
- 床は対照が時計の分解能の上にあること（`TIMER_RESOLUTION = 1e-6`、`CONTROL_FLOOR_TICKS = 1000`）だけを測る。`verify-string-split-linear` の床と同じ形である。
- 壁時計の `WALL_TIMEOUT = 10.0` は性能の上限ではなく、暴走した実装で門番が止まらないための枠である。実測の run は 6ms なので余裕は 3 桁ある。
- 却下: 入力サイズを 2 点（3 万と 6 万）で測り、要素数に対する増え方の比を見る案（`verify-string-split-linear` と同じ形）。実測で対照（生成器 + `List.length`）自体が 30000→60000 で 2.76 倍に伸び、`of_list` の差は 0.097→0.905ms で分解能から導いた床 1ms を割った。`of_list`（n バイトの 1 回の確保と書き込み）は生成器（n 個の cons の確保）より 1〜2 桁軽く、差を取ると雑音に埋もれる。この形は使えない。
- 却下: 上限を機械の速さから導く案。機械の速さを測っても、負荷で wall が伸びる分は消えない（build が負荷で 2 秒を超えた実測がある）。CPU 時間にすれば負荷の影響は消えるが、機械依存の絶対値は残る。比の方が強い。
- 却下: 対照を同じサイズの `Bytes.to_list` にする案。`to_list` 自身が被験対象なので、片方の退行がもう片方の対照を汚す。共通の線形な対照 1 本（生成器 + `List.length`）にした。
- 却下: 上限を消して一致の検査だけにする案。値の一致は打ち切りを捕まえるが、O(n²) への退行は捕まえない（実測: 二乗の `of_list` は値が一致したまま 2.22s かかる）。性能の退行を捕まえる役は要る。

### 期待値の根拠

- 生成器は `mod256 (n - 1)` を先頭に cons するので、`gen n` の先頭が `(n - 1) % 256`、末尾が `0` である。3 万要素の長さ・先頭・中間・末尾は `[30000, 47, 151, 0]` になる（判定器側で計算する。直書きしない）。
- 対照は `gen 30000 |> List.length |> Int.to_string` で `30000` を印字する。生成器と `List.length` の両方が要素数に線形である。
- 線形な `of_list` / `to_list` の比は 1.04〜1.33、二乗の `of_list` の比は 395.70 である（実測）。閾値 8.0 は両者の中間を桁で取る。

### 成果物

- `scripts/verify-r34.sh` — 34-c(3) の 2 本を `perf_c3`（対照に対する比 + 値の一致）へ置き換える。`builds_runs_within` と `timeout` の様式を削る。
- `docs/rounds/r34-timing.md`。

### 工程

1. main `2abdec1` から worktree `r34-timing` を切る。
2. 旧い上限が何を測っているかを実測する（build 約 1.05s / run 約 6ms）。
3. 2 点の入力サイズの案を実測し、差が床を割ることを確かめて却下する。
4. 同じ要素数の対照に対する比へ直す（`test:`）。
5. `stdlib/bytes.hd` の `of_list` / `to_list` を二乗の形へ戻し、検査が赤になることを確かめる（赤を先に測る）。
6. 元に戻し、静かな機械で緑、CPU 飽和で緑を確かめる。
7. quick / full の層別を回す。
8. 実測を本ファイルに書く（`docs:`）。

### 判定

- 二乗の `of_list` で 34-c(3) が赤（比が閾値を超える）、二乗の `to_list` で赤（10 秒の枠を超える）。
- 静かな機械で `PASSED: 1360/1360`。
- CPU 飽和で `PASSED: 1360/1360`。
- `bash scripts/run-gates.sh --tier quick` が赤なし、`--tier full` が赤なし。
- `git status --short` が空。

### 許容される赤

無し。

### スコープ外

- `verify-string-split-linear.sh` の比の判定（`RATIO` 2.5）と床（`timing-floor` が直した）。
- `verify-cranelift-opt-level.sh` の `SPLIT_FLOOR`。
- 仕様への計算量の文の追加（仕様本文に `Bytes` の計算量は無い。§55 の `String` の文とは別である）。

### 中断の条件

- 二乗の `of_list` / `to_list` で検査が赤にならないとき。
- 線形の実装の比が閾値に近づく（機械の中で対照が比の分母として使えない）とき。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §49 | `Bytes.of_list` の 0..255 の範囲外は `ByteOutOfRange`、`Bytes.to_list` は要素を先頭から返す | 実行 | `stdlib/bytes.hd` | `scripts/verify-r34.sh` 検査 34-c(2a)(2b)(3)(4a) |
| r34 | `Bytes.of_list` の構築は `__bytes_alloc` + `__bytes_fill_unchecked` で要素数に定数回の走査（仕様本文に計算量の文は無く、`docs/rounds/r34.md` の決定） | 実行 | `stdlib/bytes.hd` | `scripts/verify-r34.sh` 検査 34-c(3) |

## 実測

### 旧い上限が測っていたもの

負荷時の全走では build だけで 2 秒を超え、静かな全走でも build+run は 1.05〜1.10s である。直した検査が測る対象の run は 5.3ms なので、時間のほぼ全部が build である。

```
$ grep -n 'c3' .worktrees/postlanding-main/target/.gates-full/verify-r34.log
33:   (c3: build timed out or failed)
35:   (c3_to_list: build timed out or failed)
$ grep -n 'c3' .worktrees/timing-floor/target/gates-full/verify-r34.log
33:   (c3: build+run=1081ms run_status=0)
35:   (c3_to_list: build+run=1095ms run_status=0)
```

### 却下した案（入力サイズ 2 点の増え方）

同じ機械で 30000 と 60000 を測る。対照（生成器 + `List.length`）自体が 2.76 倍に伸び、`of_list` の差（0.097→0.905ms）は分解能から導いた床 1ms を割る。比 9.33 は分母が雑音であることの現れで、判定に使えない。

| サイズ | 対照 | 対象（+ `of_list`） | 差 |
|---|---|---|---|
| 30000 | 5.309ms | 5.406ms | 0.097ms |
| 60000 | 14.643ms | 15.548ms | 0.905ms |

### 赤（二乗の of_list / to_list）

`stdlib/bytes.hd` の実装を、`of_list` は 1 要素ごとに全体を複製する形へ、`to_list` は `List.append` で末尾に積む形へ一時的に戻す。コーパスロックも作り直して 34-j を緑に保つ（実際の変更でも stdlib を変えたら lock を作り直す）。

```diff
-def of_list values => if range_ok values => build values 0 (List.length values |> __bytes_alloc) | raise { message = "ByteOutOfRange" }
-def to_list bytes => elements bytes (__bytes_length bytes - 1) []
+def of_list values => if range_ok values => build values (__bytes_alloc 0) | raise { message = "ByteOutOfRange" }
+def to_list bytes => elements bytes 0 []
...
-def rec build values index target => if values | [] => target | [head, ...tail] => build tail (index + 1) (__bytes_fill_unchecked target index head)
-def rec elements bytes index acc => if index < 0 => acc | elements bytes (index - 1) [__bytes_get bytes index, ...acc]
+def rec build values target => if values | [] => target | [head, ...tail] => build tail (append_byte target head)
+def append_byte bytes value => copy bytes 0 (__bytes_fill_unchecked (__bytes_alloc (__bytes_length bytes + 1)) (__bytes_length bytes) value)
+def rec copy src index target => if index >= __bytes_length src => target | copy src (index + 1) (__bytes_fill_unchecked target index (__bytes_get src index))
+def rec elements bytes index acc => if index >= __bytes_length bytes => acc | elements bytes (index + 1) (List.append acc [__bytes_get bytes index])
```

34-c(3) の 2 本だけが赤になり、件数は元の full の赤と同じ 1358/1360 になる。値の一致は緑のままである。

```
$ cp target/probe-r34/bytes.hd.quad stdlib/bytes.hd
$ bash scripts/corpus-lock.sh && cargo build --offline --bin hado
258
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.59s
$ bash scripts/verify-r34.sh > target/probe-r34/verify-r34-mutated.log 2>&1; echo exit=$?
exit=1
$ grep -E '^(NG:|PASSED)' target/probe-r34/verify-r34-mutated.log
NG: 34-c(3) 3 万要素の of_list が一致し、要素数に線形
NG: 34-c(3) 3 万要素の to_list の長さ・先頭・中間・末尾が一致し、要素数に線形
PASSED: 1358/1360
$ grep -n 'AssertionError\|TimeoutExpired' target/probe-r34/verify-r34-mutated.log
1310:AssertionError: of_list: target/control ratio 395.70 > 8.0 (control=0.0056s target=2.2209s)
1329:subprocess.TimeoutExpired: Command '['/tmp/tmp.Mt6uakMDt2/c3-perf/to_list/to_list/program']' timed out after 10.0 seconds
```

変異は元に戻し、lock も戻す。

```
$ git checkout -- stdlib/bytes.hd tests/corpus.sha256
$ bash scripts/corpus-lock.sh --check; echo corpus_exit=$?
corpus_exit=0
```

### 緑（CPU 飽和で門番の全走）

別プロセス 12 本で CPU を飽和させ、load average 14〜15 の下で `verify-r34` を丸ごと回す。対照の CPU 時間は 5.1ms から 8.5ms へ伸びるが、比は 1.05 / 1.31 で変わらない。

```
$ end=$(( $(date +%s) + 5400 )); for i in $(seq 12); do ( while [ "$(date +%s)" -lt "$end" ]; do :; done ) & done
$ uptime
 00:53:43 up 29 days, 13:18,  1 user,  load average: 13.97, 14.71, 14.60
$ bash scripts/verify-r34.sh > target/probe-r34/verify-r34-saturated.log 2>&1; echo exit=$?
exit=0
$ grep -E '^(of_list|to_list): control|^PASSED' target/probe-r34/verify-r34-saturated.log | tail -3
of_list: control=0.0085s target=0.0089s ratio=1.05
to_list: control=0.0086s target=0.0112s ratio=1.31
PASSED: 1360/1360
```

### 層別（quick）

```
$ bash scripts/run-gates.sh --tier quick target/gates-quick
TALLY: 51/51 passed; red: none
```

### 層別（full、1 回）

load average 1.34 の静かな機械で 1 回だけ回す。赤は 0 本で、`verify-r34` の 2 本の比は 1.02 / 1.26、`verify-string-split-linear`（`timing-floor` が直した）も緑である。

```
$ bash scripts/run-gates.sh --tier full target/gates-full
TALLY: 3894/3894 passed; red: none
$ grep -E '^(of_list|to_list): control' target/gates-full/verify-r34.log
of_list: control=0.0052s target=0.0053s ratio=1.02
to_list: control=0.0052s target=0.0067s ratio=1.26
$ tail -1 target/gates-full/verify-r34.log
PASSED: 1360/1360
$ tail -1 target/gates-full/verify-string-split-linear.log
PASSED: 11/11
```

### 静的検査と lock

```
$ cargo fmt --all --check; echo fmt_exit=$?
fmt_exit=0
$ cargo clippy --offline --all-targets -- -D warnings 2>&1 | tail -1; echo clippy_exit=${PIPESTATUS[0]}
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.26s
clippy_exit=0
$ bash scripts/lint-docs.sh | tail -1
PASSED: 16/16
$ bash scripts/corpus-lock.sh --check; echo corpus_exit=$?
corpus_exit=0
$ bash scripts/gate-tiers-lock.sh --check; echo tiers_exit=$?
tiers_exit=0
```

### 木

変異は残っていない。

```
$ git status --short
$ git log --oneline -2 -- scripts/verify-r34.sh
302a321 test: 34-c(3) の上限を絶対時間から同じ要素数の対照に対する比へ
dc4f328 test: 印字を期待する探針を Int の終了コードに合わせる
```
