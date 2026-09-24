# LSP 門番の起動待ちを応答の予算から外す（lsp-probe-lock）

## 手順書

### 背景

- main の CI（full 層、`--jobs 12`）は PR #34 の着地から赤が続き、8 走のうち 7 走が
  `verify-lsp-minimal` の 1 本だけで落ちる。落ちるのはどれも最初の探針で、
  `NG: lifecycle-shutdown probe error: timeout: lsp client`（10/11）。PR の core 層と
  手元の全走は緑。
- 判定器 `scripts/lsp-check.py` の `open()` は、ファイルを開いてから client の初期化までを
  30 秒で待つ。README の `cmd`（`scripts/hado-lsp.sh`）は `cargo run --offline -q -- lsp`
  なので、この 30 秒に cargo の build lock の待ちが入る。full 層は前段（gate-cache と
  m0〜m3）の後に 12 本を同時に起こし、`verify-clippy` と `verify-cognitive-complexity`
  が同じ瞬間に lock を取りに行く（`docs/rounds/spec-model.md` の着地後の全走）。

### 決定

1. `open()` の待ちを 2 段に分ける。起動器（`hado-lsp.sh` → cargo）が exec で
   `hado lsp` に置き換わるまでは見張り `LAUNCH_SECONDS`（300 秒。cargo を起こす
   `run_hado` と同じ）で待ち、30 秒の予算は exec の後の initialize の応答だけに掛ける。
2. 30 秒の予算は延ばさない。README の `cmd`・`scripts/hado-lsp.sh`・門番の層と並列度は
   変えない。
3. exec の判定は、Neovim の子プロセスの名前が全て `hado` であること。見張りは
   `vim.wait` の条件に置かず、200 ms ごとに眠ってから見る（条件に置くと `ps` を
   回し続ける。実測）。

### code-complete の一周目

- 意図: 門番の赤を「サーバが応答しない・遅い」の意味に限る。並走する門番の cargo が
  握る lock で赤にしない。
- 仮説: 予算を exec の前と後に分ければ、lock 待ちは見張りに入り、サーバの欠陥は
  30 秒で赤のまま残る。cargo は `run` で exec するので、Neovim から見た子の pid は
  変わらず、名前が `cargo` から `hado` に変わる。
- 判定器: 下の「判定」。
- スコープ: `scripts/lsp-check.py`、`scripts/verify-lsp-minimal.sh` の見出し、本ファイル。
- 未確認: CI で lock を握っていたのがどの門番か。full 層の CI が緑になるか。

### 期待値の根拠

- 30 秒は `lsp-minimal` が置いた応答の予算で、手元の単独走では 11 本の合計が 8 秒に
  収まる。この予算は変えない。
- 見張りの 300 秒は、同じ判定器が cargo を起こす `run_hado` の timeout と同じ。

### 成果物

`scripts/lsp-check.py`、`scripts/verify-lsp-minimal.sh` の見出し、本ファイル。

### 工程

lock の保持で赤を再現 → `open()` の待ちを 2 段に分ける → 欠陥の注入と実測 →
full 層の CI（workflow_dispatch）。

### 判定

- lock（`target/debug/.cargo-lock`）を 60 秒握った状態で `bash scripts/verify-lsp-minimal.sh`
  が、修正前は `NG: lifecycle-shutdown probe error: timeout: lsp client`、修正後は
  `PASSED: 11/11`。
- 欠陥を注入すると赤のまま。`hado` の名で initialize に応答しないサーバは約 30 秒で
  `timeout: lsp client`、すぐ終わる起動器も `timeout: lsp client`、exec しない起動器は
  見張りで `timeout: lsp launcher`。
- 負荷の無い単独走が `PASSED: 11/11`。
- full 層の CI で `verify-lsp-minimal` が緑。

### 許容される赤

無し。

### スコープ外

- `scripts/run-gates.sh` の順序と並列度。clippy の門番を前段に移しても、ほかの門番も
  cargo を起こすので、取り合いは残る。

### 中断の条件

full 層の CI で同じ探針が `timeout: lsp client` で落ちたら、lock 待ちの仮説が外れて
いるので、実装を足さずに探索へ戻る。

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （門番基盤） | LSP 門番の応答の予算は exec の後のサーバだけに掛け、起動器の cargo の待ちを含めない | 道具 | 成功 | `scripts/verify-lsp-minimal.sh` |

## 実測

main の CI の赤（2026-09-24、PR #34 から #39 の着地の 7 走。緑は #32 の 1 走）。どれも
最初の探針だけが落ちる。

```
$ for id in 36019567479 36019603279 36028040400 36030615878 36065819499 36070372065 36070396096; do
>   gh run download $id -n gate-logs -D g-$id >/dev/null 2>&1
>   printf '%s ' $id; grep -h -E '^NG|^PASSED' g-$id/verify-lsp-minimal.log | tr '\n' ' '; echo
> done
36019567479 NG: lifecycle-shutdown probe error: timeout: lsp client PASSED: 10/11
36019603279 NG: lifecycle-shutdown probe error: timeout: lsp client PASSED: 10/11
36028040400 NG: lifecycle-shutdown probe error: timeout: lsp client PASSED: 10/11
36030615878 NG: lifecycle-shutdown probe error: timeout: lsp client PASSED: 10/11
36065819499 NG: lifecycle-shutdown probe error: timeout: lsp client PASSED: 10/11
36070372065 NG: lifecycle-shutdown probe error: timeout: lsp client PASSED: 10/11
36070396096 NG: lifecycle-shutdown probe error: timeout: lsp client PASSED: 10/11
```

起動器は exec で置き換わる。lock を 0.5 秒握ってから、README の `cmd` を設定した Neovim で
`.hd` を開き、100 ms ごとに子の pid と名前を記録すると、同じ pid の名前が lock の解放後に
`hado` になる。Neovim に他の子は無い（出力は抜粋）。

```
$ (flock target/debug/.cargo-lock sleep 0.5 &); sleep 0.05
$ nvim --headless -u NONE -i NONE -c 'luafile probe.lua'; cat out.txt
before
after-edit 3635078:cargo
t300 3635078:cargo
t400 3635078:hado
initialized 3635078:hado
```

修正前（`7a28e26` の親）。lock を 60 秒握ると最初の探針が 30 秒で落ちる。

```
$ (flock target/debug/.cargo-lock sleep 60 &); sleep 0.5
$ time bash scripts/verify-lsp-minimal.sh 2>&1 | grep -E 'NG|PASSED'
NG: lifecycle-shutdown probe error: timeout: lsp client
PASSED: 10/11
real	1m4.633s
user	0m1.444s
sys	0m0.607s
```

修正後。同じ条件で緑になる。単独走は 7.85 秒で、修正前の 7.4 秒より約 0.4 秒延びる
（見張りを 200 ms 刻みで見る分と推定。内訳は測っていない）。

```
$ (flock target/debug/.cargo-lock sleep 60 &); sleep 0.5
$ /usr/bin/time -f '%e s real, %U user, %S sys' bash scripts/verify-lsp-minimal.sh 2>&1 | grep -E 'NG|PASSED|real'
PASSED: 11/11
66.09 s real, 3.56 user, 6.27 sys
$ /usr/bin/time -f '%e s real, %U user, %S sys' bash scripts/verify-lsp-minimal.sh 2>&1 | grep -E 'NG|PASSED|real'
PASSED: 11/11
7.85 s real, 1.76 user, 1.22 sys
```

見張りを `vim.wait` の条件に置いた版（間隔 20 ms と 200 ms）は、同じ lock の 60 秒で
どちらも `sys` が 45 秒前後だった。

```
$ (flock target/debug/.cargo-lock sleep 60 &); sleep 0.5
$ time bash scripts/verify-lsp-minimal.sh 2>&1 | grep -E 'NG|PASSED'   # 間隔 200 ms の版
PASSED: 11/11
real	1m5.045s
user	0m16.435s
sys	0m44.706s
```

欠陥の注入。README と偽の `scripts/hado-lsp.sh` だけを置いた根を作り、判定器の先頭の
探針 `lifecycle-shutdown` だけを走らせる（`inject.py` は `scripts/lsp-check.py` を
読み込んで `CHECKS[0]` を呼び、5 番目の引数で `LAUNCH_SECONDS` を差し替える）。
`hado` は `/bin/sleep` の写しで、initialize に応答しない。

```
$ python3 inject.py scripts/lsp-check.py "$nvim" fake-mute w-mute    # exec "$bin/hado" 1000
NG: lifecycle-shutdown probe error: timeout: lsp client
elapsed 30.4s
$ python3 inject.py scripts/lsp-check.py "$nvim" fake-dies w-dies    # exit 1
NG: lifecycle-shutdown probe error: timeout: lsp client
elapsed 30.6s
$ python3 inject.py scripts/lsp-check.py "$nvim" fake-stuck w-stuck 5   # sleep 1000（exec しない）
NG: lifecycle-shutdown probe error: timeout: lsp launcher
elapsed 5.2s
```

CI の形を手元で真似た走行（clippy の成果物を消し、2 コアに固定して clippy の門番 2 本と
判定器を同時に起こす）では、修正前も通った。この機械では clippy が lock を 30 秒以上
握らないので、CI の遅れは手元では出ない。赤の再現は上の lock の保持で行う。

```
$ for judge in lsp-check.orig.py scripts/lsp-check.py; do   # 修正前、修正後
>   cargo clean -q && cargo build --offline -q
>   taskset -c 0,1 bash scripts/verify-clippy.sh >c1.log 2>&1 &
>   taskset -c 0,1 bash scripts/verify-cognitive-complexity.sh >c2.log 2>&1 &
>   /usr/bin/time -f 'judge %e s real' taskset -c 0,1 python3 $judge "$nvim" "$PWD" j 2>&1 | grep -E 'NG|PASSED|real'
>   wait
> done
PASSED: 11/11
judge 17.64 s real
PASSED: 11/11
judge 20.04 s real
```
