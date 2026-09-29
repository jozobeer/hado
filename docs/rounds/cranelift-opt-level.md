# Cranelift の `opt_level` の既定を測って決める（cranelift-opt-level）

計画は `docs/plans/issue-backlog.md` の R レーン 4 番目。着手時の main は `d8062d5`。

## 手順書

### 一周目の計画

| 要素 | 内容 |
|---|---|
| 意図 | `crates/hado/src/emit.rs` の `make_isa` が Cranelift の `opt_level` を `"none"` に固定しており、`"speed"` との差を測った記録が無い。測って issue の分岐に従って既定を決め、ビルダーとレビュアーが「なぜその既定か」をラウンドファイルの表から読めるようにする（推定） |
| 仮説 | 3 本の比はどれも 1.1 倍に届かず、既定は `"none"` のままになる。理由は、末尾再帰は §53 の `return_call` で既にループになっており、文字列比較と `String.split` の本体は C の runtime（`memcmp`・`hado_string_split`）で、Cranelift の最適化が届く範囲が小さいため（推定）。2 水準を測るには、既定を変えずに水準を選べる切り替え（`HADO_OPT_LEVEL`）と、`build` と `run` の既定を別に持てる配線が要る（推定） |
| 判定器 | issue の完了条件をそのまま使う。3 入力（`==` 10000000 回、末尾再帰 10000000 回、`string-split-linear` の入力）× 2 水準を warm-up 後 5 回測り、比（`"none"` の中央値 ÷ `"speed"` の中央値）を出す。3 本すべて 1.1 倍以上かつ比の中央値が 2 倍以上のときだけ `"speed"`、それ以外は `"none"`。観測は両水準で先に照合し、期待値は判定器がソースと入力から独立に計算する。門番 `scripts/verify-cranelift-opt-level.sh`（`# tier: full`・`# round: cranelift-opt-level`） |
| スコープ | 触る: `crates/hado/src/emit.rs`・`crates/hado/src/project.rs`・`crates/hado/src/main.rs`（水準の選択と `build`/`run` の既定）、`scripts/verify-m0.sh`（ISA flags の錠の当て先）、`scripts/verify-cranelift-opt-level.sh`（新設）、`tests/perf/opt-level/`（追跡ソースと入力生成）、`scripts/gate-tiers.lock`、本ファイル、`docs/issues/cranelift-opt-level.md` の削除。触らない: `docs/spec.md`（言語仕様は変えない）・`docs/plan.md`・`docs/ledger.md`・`docs/spec-trace.md`・`docs/plans/`・`stdlib/`・`tests/accept`・`tests/reject`・`tests/library`・他のレーンの worktree |
| 未確認 | 3 本の比が 1.1 倍を超えるか（超えない見込み）。`string-split-linear` の入力の中央値が 200 ms の床を満たすか。`HADO_OPT_LEVEL` が `hado run` の内側の build にも効くか。`"speed"` の分岐で `build` と `run` の既定を分ける配線が要るか（今回は `"none"` の枝なら不要） |

### 背景

`crates/hado/src/emit.rs:23` の `make_isa` は `opt_level` を `"none"` に固定している。issue の現象（2026-09-17）は文字列比較 1000 万回 0.08 s、末尾再帰 1000 万回 0.14 s で、「いまの遅さの主因ではない」と書く。`"speed"` との差は未計測で、既定を選ぶ根拠が無い。

`docs/cranelift-0.135.md` は既定が `none` でも `set("opt_level", "none")` を書く、と定める。`scripts/verify-m0.sh` 検査 6 が `emit.rs` と `tests/tail.rs` の `opt_level` の値（`none`）を錠にしている。

`string-split-linear`（R1、着地済み）の完了条件は、`scripts/verify-string-split-linear.sh` 検査 e で 1 行 64 文字の ASCII 入力を `String.split "\n"` する。本ラウンドの 3 本目の入力はその生成手順（`lines(1000000, 64)`）をそのまま使う。

### 決定

- 決定 1: 2 水準を測る切り替えは `HADO_OPT_LEVEL`（`none`・`speed` だけを受ける環境変数）にする。門番が同じ木・同じコンパイラで 2 水準の実行ファイルを作れる。ツリーを書き換えずに測れることを優先する。言語仕様には出さない（`check` は emit を通らないので影響しない）。
- 決定 2: `build` と `run` の既定は `emit::BUILD_OPT_LEVEL`・`emit::RUN_OPT_LEVEL` の 2 つの定数に分ける。issue は `build` を `"speed"` にしても `run` は end-to-end の表で別に決めると定めるので、片方だけ動かせる配線を先に置く。今回は `"none"` の枝なので両方 `"none"`。
- 決定 3: 追跡ソースは `tests/perf/opt-level/{string_compare,tail_recursion,split}/main.hd` に 1 ケース 1 ディレクトリで置く。`Project::load` は入力の親ディレクトリの `.hd` を全部読むので、同じディレクトリに 3 本置くと混ざる。
- 決定 4: 入力の生成手順は `tests/perf/opt-level/split_input.py` に置き、出力の sha256 とバイト数を `split_input.sha256` に錠する。「同じ生成手順で再生成してバイト一致」は、生成器の出力が錠と一致することと、生成器が検査 e の `lines` と同じ式を持つことで測る。
- 決定 5: 期待値は判定器がソースと入力から計算する。文字列比較はソースの `==` の被演算子を独立に `==` した真偽と反復数・基底・加算幅から、末尾再帰はソースの基底・加算幅・反復数から、`split` は再生成した入力の行数から。コンパイラの出力から期待値を採らない。
- 決定 6: 門番の層は `full`。3 入力 × 2 水準の実行ファイルを作り、65 MB の入力を生成して warm-up + 5 回 × 6 本を測る。`quick`・`core` では費用に見合う変更が入らない（このラウンドの成果物は `emit.rs` の既定の選択だけ）。
- 決定 7: `"speed"` の枝の `run` の end-to-end の表は、判定が `"speed"` のときだけ測る。今回は `"none"` の枝なので、`run` の既定は `"none"` の錠だけを見る。

### 期待値の根拠

- 文字列比較: `tests/perf/opt-level/string_compare/main.hd` の `"hado" == "hado"` の被演算子を判定器が独立に `==` する。真なら `基底 + 反復数 × 加算幅`、偽なら `基底`。反復数は 10000000 でなければ赤。
- 末尾再帰: `tests/perf/opt-level/tail_recursion/main.hd` の基底（`count ... 0` の第 2 引数）と加算幅（`acc + 1` の `1`）と反復数（10000000）から `基底 + 反復数 × 加算幅`。自己呼び出しが腕の全体（`| count ...`）にあることを末尾再帰の形として見る。
- `split`: `split_input.py` を再生成し、錠（sha256・バイト数）と一致することを見る。期待値は再生成したバイト列の改行数 + 1。`split/main.hd` の区切りリテラルを読んで数える。
- 時間: 各実行ファイルを warm-up 1 回のあと 5 回測り、子プロセスの CPU 時間（user + sys）の中央値を使う。負荷で待たされても CPU 時間は動かず、対照（同じ実行ファイルの他水準）と同条件で比べられる。65 MB の `split` は中央値が 200 ms の床を大きく超える。
- `check` の対照: `HADO_OPT_LEVEL` の 2 値で `hado check --print-type=main` と拒否の診断を比べ、`check` が emit を通らないことを測る。

### 成果物

- `tests/perf/opt-level/string_compare/main.hd`・`tail_recursion/main.hd`・`split/main.hd` — 計測の入力（追跡ソース）。
- `tests/perf/opt-level/split_input.py`・`split_input.sha256` — 3 本目の入力の生成手順と錠。
- `crates/hado/src/emit.rs` — `BUILD_OPT_LEVEL`・`RUN_OPT_LEVEL`・`selected_opt_level` と `make_isa(opt_level)`。
- `crates/hado/src/project.rs`・`crates/hado/src/main.rs` — 水準を `build`・`run` へ配る。
- `scripts/verify-m0.sh` — ISA flags の錠を定数へ当て直す。
- `scripts/verify-cranelift-opt-level.sh` — 本ラウンドの門番（`# tier: full`・`# round: cranelift-opt-level`）。
- `scripts/gate-tiers.lock` — 門番を足した分の作り直し。
- `docs/rounds/cranelift-opt-level.md`（本ファイル）。
- `docs/issues/cranelift-opt-level.md` の削除。

### 工程

1. Red: 追跡ソースと入力生成と門番を置き、`opt_level` の切り替えが無い木で赤を確かめる。
2. Green: `emit.rs` に定数と `selected_opt_level` と `make_isa(opt_level)` を足し、`project.rs`・`main.rs` に配る。`verify-m0.sh` の錠を定数へ当て直す。
3. 判定器の較正: 比の判定関数を issue の例（`3.0`・`3.0`・`0.1`、`10.0`・`1.1`・`1.1`）で確かめる。
4. 変異で赤を確認する（追跡ソースの再構成・反復不足・別計算・短い入力）。
5. `cargo test --offline`・`--tier quick`・`--tier core`・`lint-docs.sh`・`verify-spec-contracts.sh` を 1 回ずつ。
6. `git merge main` の後、`corpus-lock.sh --check` の差分を読んで作り直し、`gate-tiers-lock.sh` を作り直し、PR にする（マージしない）。

### 判定

- `bash scripts/verify-cranelift-opt-level.sh` が `PASSED: n/n`。
- 判定の結論（`"none"` か `"speed"`）が `emit::BUILD_OPT_LEVEL` と一致する。`"none"` なら `RUN_OPT_LEVEL` も `"none"`。
- `cargo test --offline` が 0 failed。quick 層と core 層が赤なし。
- `bash scripts/lint-docs.sh`・`bash scripts/verify-spec-contracts.sh` が赤なし。

### 許容される赤

無し。時間の判定は CPU 時間の中央値を使う。比が 1.1 をまたいだときは、負荷ではなく実行ファイルの違いを疑い、単独で測り直してから赤と判断する。

### スコープ外

- `opt_level` をユーザー向けの CLI や言語仕様にすること（既定を決めるのがこのラウンド）。
- `enable_verifier` など他の Cranelift 設定。
- `string-split-linear` の入力の生成手順そのものの変更。

### 中断の条件

- 3 本の比が負荷で安定せず、判定がひっくり返るとき（実測値と負荷を添えて止まる）。
- `HADO_OPT_LEVEL` が `hado run` の内側の build に効かないと分かったとき。
- 判定の結論と `emit.rs` の既定が一致しないとき（人が決める）。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §53 | 末尾呼び出しはスタックを消費しない | 実行 | — | `tests/perf/opt-level/tail_recursion/main.hd`・`scripts/verify-cranelift-opt-level.sh` |
| §55 | `String.split` と `String.join` は定数回の走査 | 実行 | — | `tests/perf/opt-level/split/main.hd`・`scripts/verify-cranelift-opt-level.sh` |
| §21 | `==` は Fn を含まない全型の構造比較 | 実行 | — | `tests/perf/opt-level/string_compare/main.hd`・`scripts/verify-cranelift-opt-level.sh` |
| §54 | `Int` の `main` は印字せず終了コードになる | 実行 | — | `scripts/verify-cranelift-opt-level.sh`（`Int.to_string` で包む） |

## 実測

### 着手前の再現（main `d8062d5`、`opt_level` は `none`）

```
$ grep -n 'opt_level' crates/hado/src/emit.rs
23:        .set("opt_level", "none")
```

現象の 2 本のループを main の木で測る（子プロセスの CPU 時間の中央値、warm-up 1 回 +
5 回、追跡ソースと同じ形）。

```
string_compare: none=0.0981s
tail_recursion: none=0.0428s
split_input:    none=0.5878s   (64999999 bytes, 1000000 lines)
```

### 門番の赤（`opt_level` の切り替えが無い実装）

```
$ bash scripts/verify-cranelift-opt-level.sh
sources: expected string_compare=10000000 tail_recursion=10000000
OK: a sources: tracked loops, iterations, == operands and the tail arm
OK: b input: regenerated bytes match the string-split-linear lock
AssertionError: HADO_OPT_LEVEL had no effect: both levels produced the same executable
NG: c observe: both levels match the independent expectations
NG: d measure: two levels, warm-up + 5 runs, per-input floor and the verdict
NG: e default: emit defaults follow the verdict
OK: f control: check diagnostics and --print-type do not depend on the level
OK: g judge: the ratio decision is calibrated on the issue examples
PASSED: 4/7
```

### 門番の緑

```
$ bash scripts/verify-cranelift-opt-level.sh
sources: two loops at 10000000 iterations, string == operands and the tail arm read from the tracked sources
sources: expected string_compare=10000000 tail_recursion=10000000
OK: a sources: tracked loops, iterations, == operands and the tail arm
input: regenerated 64999999 bytes (sha256 6f7551b861ea...), 1000000 lines for the split separator
OK: b input: regenerated bytes match the string-split-linear lock
observe: 6 executables match the independent expectations at both levels
observe: the two levels produce different machine code for split, string_compare, tail_recursion
OK: c observe: both levels match the independent expectations
measure string_compare: none=0.0977s speed=0.0968s ratio=1.009
measure tail_recursion: none=0.0428s speed=0.0419s ratio=1.021
measure split: none=0.5900s speed=0.5744s ratio=1.027
judge: median=1.021 lower-bound input=string_compare ratio=1.009 verdict=none
OK: d measure: two levels, warm-up + 5 runs, per-input floor and the verdict
default: verdict=none, BUILD_OPT_LEVEL=none, RUN_OPT_LEVEL=none, the default build equals the none build
OK: e default: emit defaults follow the verdict
control: check diagnostics and --print-type are identical at both levels
OK: f control: check diagnostics and --print-type do not depend on the level
judge: 7 ratio triples classified as the issue requires (median-only and max-only judges are rejected)
OK: g judge: the ratio decision is calibrated on the issue examples
mutations: fewer iterations, no string ==, a non-tail call, a shorter input, and median-only / max-only judges are all rejected
OK: h mutations: the listed bad implementations are rejected
PASSED: 8/8
```

### 判定値の表

比は `"none"` の中央値 ÷ `"speed"` の中央値。判定値は 3 本の比の中央値。

| 入力 | `"none"` 中央値 | `"speed"` 中央値 | 比 | 下限 1.1 | 中央値 2.0 |
|---|---|---|---|---|---|
| `==` 10000000 回 | 0.0977 s | 0.0968 s | 1.009 | 下回る | 下回る |
| 末尾再帰 10000000 回 | 0.0428 s | 0.0419 s | 1.021 | 下回る | 下回る |
| `string-split-linear` の入力 | 0.5900 s | 0.5744 s | 1.027 | 下回る | 下回る |

- 判定値（比の中央値）: **1.021**（2.0 未満）。
- 下限を決めた入力: **`==` 10000000 回**（比 1.009 が 3 本の最小）。3 本すべてが 1.1 倍を下回る。
- 結論: **`"none"`**。`"speed"` にしない条件（3 本すべて 1.1 倍以上かつ中央値 2 倍以上、の補集合）に当たる。`emit::BUILD_OPT_LEVEL` と `emit::RUN_OPT_LEVEL` は `"none"` のままで、既定は変えない。
- 余裕: 比は 1.009〜1.027 で、下限 1.1 まで 7〜9 % の余裕がある。`split` の小さい側の中央値は 0.5744 s で、床 0.2 s の 2.9 倍。測り方は子プロセスの CPU 時間の中央値（warm-up 1 回 + 5 回）で、負荷で待たされても比が動かない。実行ファイルは両水準で別の機械語になることを先に確かめている（`observe`）。

### `hado run` の end-to-end の表

`hado run` は起動・コンパイル・リンクをしてから子を走らせる（`main.rs` の `execute`）。判定は
`"none"` なので `run` の既定には end-to-end の表を使わないが、参考に測る（同じコンパイラで
`HADO_OPT_LEVEL` だけを変える）。

```
run-e2e string_compare: none=1.2766s speed=1.2868s ratio=0.992
run-e2e tail_recursion: none=1.2179s speed=1.2182s ratio=1.000
run-e2e split:          none=1.7478s speed=1.7475s ratio=1.000
```

3 本とも 1.1 倍未満で、仮に `build` を `"speed"` にしても `run` は `"none"` になる。

### 変異で赤を確認する

追跡ソースの反復数を 10000000 から 9999999 に落とすと赤になる。

```
$ sed -i 's/10000000/9999999/' tests/perf/opt-level/string_compare/main.hd
$ bash scripts/verify-cranelift-opt-level.sh
NG: a sources: tracked loops, iterations, == operands and the tail arm
...
PASSED: 3/8
```

3 本目の入力の生成を短くする（`COUNT = 1000`）と赤になる。

```
$ sed -i 's/^COUNT = 1000000/COUNT = 1000/' tests/perf/opt-level/split_input.py
$ bash scripts/verify-cranelift-opt-level.sh
OK: a sources: tracked loops, iterations, == operands and the tail arm
NG: b input: regenerated bytes match the string-split-linear lock
...
PASSED: 4/8
```

`==` を別の計算に差し替える・自己呼び出しを末尾位置から外す・中央値だけ／最大値だけの
判定は、門番の検査 `h mutations` が常時落とす（実装を変異させずに較正として残す）。

### 実装の門番

```
$ bash scripts/verify-m0.sh
OK: 6 isa-flags
PASSED: 8/8

$ bash scripts/lint-docs.sh
PASSED: 16/16

$ bash scripts/verify-spec-contracts.sh
PASSED: 2/2

$ bash scripts/corpus-lock.sh --check     # 差分なし（コーパスは触っていない）
$ bash scripts/gate-tiers-lock.sh         # cranelift-opt-level full を足した 1 行
75
```

層別（2026-09-29）。

```
$ bash scripts/run-gates.sh --jobs 4 --tier quick target/gates-quick
TALLY: 41/41 passed; red: none

$ bash scripts/run-gates.sh --jobs 4 --tier core target/gates-core
TALLY: 714/714 passed; red: none
```

`cargo test --offline` は `verify-m0.sh` の検査 2 が `--no-fail-fast` で実走し、
`OK: 2 cargo-test passed=789`。

着地の直前の main は `d8062d5`（`origin/main` も同じ）で、取り込む差分は無い。


