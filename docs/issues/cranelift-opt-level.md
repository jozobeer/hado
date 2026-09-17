# Cranelift の `opt_level` を測る（cranelift-opt-level）
優先度: 低。

## 現象

`crates/hado/src/emit.rs:23` で `opt_level` は `"none"` である。文字列比較 1000 万回が 0.08 s、末尾再帰 1000 万回が 0.14 s なので、いまの遅さの主因ではない（2026-09-17）。`"speed"` にしたときの差は未計測。

## 原因

`crates/hado/src/emit.rs:23` が Cranelift の `opt_level` を `"none"` に固定している。`"speed"` との差を測った記録が無く、既定を選ぶ根拠が無いまま残っている。

## 案

- 同じ 2 つのループと string-split-linear の入力で `"speed"` と比べ、差が 2 倍未満なら閉じる。
- 上げるなら `build` の既定にし、`check` は触らない。

## 完了条件

- 計測の表がラウンドファイルにある。同じ 2 つのループと string-split-linear の入力それぞれで `"none"` と `"speed"` を warm-up 後 5 回測り、各入力の比（`"none"` の中央値 ÷ `"speed"` の中央値）を出す。3 つの比の中央値を判定値とする。
- 判定値が 2 倍未満なら `crates/hado/src/emit.rs:23` は `"none"` のままとし、`build` と `hado run` の既定を変えない。
- 判定値が 2 倍以上なら `build` と `hado run` の既定を `"speed"` にし、生成した実行ファイルと `hado run` の両方で計測して同じ判定値を再現する。`check` は emit を通らないので、`check` の診断と `--print-type` が変わらないことを対照に置く。

## 由来

`docs/wants.md` に無い。
