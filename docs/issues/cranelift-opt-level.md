# Cranelift の `opt_level` を測る（cranelift-opt-level）
優先度: 低。

## 現象

`crates/hado/src/emit.rs:23` で `opt_level` は `"none"` である。文字列比較 1000 万回が 0.08 s、末尾再帰 1000 万回が 0.14 s なので、いまの遅さの主因ではない（2026-09-17）。`"speed"` にしたときの差は未計測。

## 案

- 同じ 2 つのループと string-split-linear の入力で `"speed"` と比べ、差が 2 倍未満なら閉じる。
- 上げるなら `build` の既定にし、`check` は触らない。

## 完了条件

- 計測の表がラウンドファイルにある。

## 由来

`docs/wants.md` に無い。
