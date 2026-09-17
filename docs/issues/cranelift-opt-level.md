# Cranelift の `opt_level` を測る（cranelift-opt-level）
優先度: 低。

## 現象

`crates/hado/src/emit.rs:23` で `opt_level` は `"none"` である。いまの遅さの主因ではない（2026-09-17）。`"speed"` にしたときの差は未計測。

```
$ grep -n 'opt_level' crates/hado/src/emit.rs
23:        .set("opt_level", "none")
```

| ループ | 反復 | 時間 |
| --- | --- | --- |
| 文字列比較 | 1000 万 | 0.08 s |
| 末尾再帰 | 1000 万 | 0.14 s |

## 原因

`crates/hado/src/emit.rs:23` が Cranelift の `opt_level` を `"none"` に固定している。`"speed"` との差を測った記録が無く、既定を選ぶ根拠が無いまま残っている。

## 案

- 同じ 2 つのループと string-split-linear の入力で `"speed"` と比べ、差が 2 倍未満なら閉じる。
- 上げるなら `build` の既定にし、`check` は触らない。

## 完了条件

- 計測の表がラウンドファイルにある。同じ 2 つのループと string-split-linear の入力それぞれで `"none"` と `"speed"` を warm-up 後 5 回測り、各入力の比（`"none"` の中央値 ÷ `"speed"` の中央値）を出す。3 つの比の中央値を判定値とする。
- 判定値が 2 倍未満なら `crates/hado/src/emit.rs:23` は `"none"` のままとし、`build` と `hado run` の既定を変えない。
- 判定値が 2 倍以上なら `build` と `hado run` の既定を `"speed"` にし、生成した実行ファイルの実行時間で同じ判定値を再現する。`hado run` は `crates/hado/src/main.rs:266` の `execute` が読み込み・コンパイル・リンクをしてから子を走らせるので、`hado run` の計測は子プロセスの実行時間を別に取り、起動・コンパイル・リンク込みの end-to-end は別の表として記録して判定には使わない。`check` は emit を通らないので、`check` の診断と `--print-type` が変わらないことを対照に置く。

## 由来

`docs/wants.md` に無い。
