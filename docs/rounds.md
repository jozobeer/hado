# ラウンドの置き方

ラウンドは `docs/plan.md` の末尾に連番で足さない。`docs/rounds/` にはラウンドファイル以外を置かない。

## 識別子

ラウンドの識別子は slug である。字種は `[a-z0-9][a-z0-9-]*`。`r<数字>` と `r<数字>-r<数字>` は下の対応表の旧節のファイル名に予約し、新しい slug には使わない。R 番号は R81 で終わる。ブランチ名・worktree のディレクトリ名・ラウンドファイル名・門番の `# round:` 行に同じ slug を使う。

## ラウンドファイル

1 ラウンド 1 ファイル。`docs/rounds/<slug>.md` に手順書・系譜・実測を同居させる。

```
# <題>（<slug>）
## 手順書
## 系譜
## 実測
```

手順書の内側の型は従来の節と同じで、背景・決定・期待値の根拠・成果物・工程・判定・許容される赤・スコープ外・中断の条件を書く。系譜は `docs/spec-trace.md` と同じ 5 列の表である。実測にはコマンドと出力の末尾をコードフェンスで置く。

旧 `docs/plan.md` から移したファイルは、題の次の行に「旧 `docs/plan.md` N 節。実測は `docs/ledger.md`。」とあり、実測は台帳を指す。

## 門番の宣言行

各 `scripts/verify-*.sh` の冒頭のコメント塊に `# tier: quick|core|full` をちょうど 1 つ置く。位置ではなく `^# tier: ` の一意性で判定する。`scripts/lib/gate-tiers.sh` は一覧を持たず、門番を走査して層ごとの集合を導く。

新方式の門番は `# round: <slug>` を持つ。`scripts/lint-docs.sh` 検査 6 は、`# round:` を持つ各門番について `docs/rounds/<slug>.md` が存在し、`## 実測` の下にコードフェンス（実測の出力）があることを見る。既存の `verify-r*.sh` は `# round:` を持たない。slug が `r<数字>` や `r<数字>-r<数字>` の形なら検査は赤になる。

## 系譜の追記先

`docs/spec-trace.md` の表は R81 の行で止める。以後のラウンドの系譜は、各ラウンドファイルの `## 系譜` に同じ 5 列で書く。

## 台帳

`docs/ledger.md` の R81 までの実測記録は凍結している。本文も参照も書き換えない。以後の実測は各ラウンドファイルの `## 実測` に書く。

ラウンドに属さない検討で仕様変更に至らなかった案と、採用した版ごとの決定は、`docs/plans/<名前>.md` に書く（`docs/ledger.md` と `docs/plan.md` は凍結した記録である）。

版ごとの決定と版をまたぐ計画は `docs/plans/<名前>.md` が持つ。2 本が同じ版の表を触るなら、先に着地する側を決めてから始める。

## 旧節番号 → ファイル名

対応表は凍結する。新ラウンドは足さない。

| 旧 `docs/plan.md` の節 | ラウンド | ファイル |
|---|---|---|
| 7 | R6 | `docs/rounds/r6.md` |
| 8 | R8〜R9 | `docs/rounds/r8-r9.md` |
| 10 | R10〜R13 | `docs/rounds/r10-r13.md` |
| 12 | R14〜R17 | `docs/rounds/r14-r17.md` |
| 14 | R18〜R22 | `docs/rounds/r18-r22.md` |
| 15 | R21追加決定 | `docs/rounds/r21-addendum.md` |
| 19 | R23〜R27 | `docs/rounds/r23-r27.md` |
| 20 | R28 | `docs/rounds/r28.md` |
| 21 | R29 | `docs/rounds/r29.md` |
| 22 | R30 | `docs/rounds/r30.md` |
| 23 | R31 | `docs/rounds/r31.md` |
| 25 | R32 | `docs/rounds/r32.md` |
| 26 | R33 | `docs/rounds/r33.md` |
| 27 | R34 | `docs/rounds/r34.md` |
| 28 | R35 | `docs/rounds/r35.md` |
| 29 | R36 | `docs/rounds/r36.md` |
| 30 | R37 | `docs/rounds/r37.md` |
| 31 | R38 | `docs/rounds/r38.md` |
| 35 | R39 | `docs/rounds/r39.md` |
| 36 | R40 | `docs/rounds/r40.md` |
| 37 | R41 | `docs/rounds/r41.md` |
| 38 | R42 | `docs/rounds/r42.md` |
| 39 | R43 | `docs/rounds/r43.md` |
| 40 | R44 | `docs/rounds/r44.md` |
| 41 | R45 | `docs/rounds/r45.md` |
| 42 | R46 | `docs/rounds/r46.md` |
| 43 | R47 | `docs/rounds/r47.md` |
| 44 | R48 | `docs/rounds/r48.md` |
| 45 | R49 | `docs/rounds/r49.md` |
| 46 | R50 | `docs/rounds/r50.md` |
| 47 | R51 | `docs/rounds/r51.md` |
| 48 | R52 | `docs/rounds/r52.md` |
| 49 | R53 | `docs/rounds/r53.md` |
| 50 | R54 | `docs/rounds/r54.md` |
| 51 | R55 | `docs/rounds/r55.md` |
| 52 | R56 | `docs/rounds/r56.md` |
| 53 | R57 | `docs/rounds/r57.md` |
| 54 | R58 | `docs/rounds/r58.md` |
| 55 | R59 | `docs/rounds/r59.md` |
| 56 | R60 | `docs/rounds/r60.md` |
| 57 | R61 | `docs/rounds/r61.md` |
| 58 | R62 | `docs/rounds/r62.md` |
| 59 | R63 | `docs/rounds/r63.md` |
| 60 | R64 | `docs/rounds/r64.md` |
| 61 | R65 | `docs/rounds/r65.md` |
| 62 | R67 | `docs/rounds/r67.md` |
| 63 | R68 | `docs/rounds/r68.md` |
| 64 | R69 | `docs/rounds/r69.md` |
| 65 | R70 | `docs/rounds/r70.md` |
| 66 | R71 | `docs/rounds/r71.md` |
| 67 | R72 | `docs/rounds/r72.md` |
| 68 | R73 | `docs/rounds/r73.md` |
| 69 | R74 | `docs/rounds/r74.md` |
| 70 | R75 | `docs/rounds/r75.md` |
| 71 | R76 | `docs/rounds/r76.md` |
| 72 | R77 | `docs/rounds/r77.md` |
| 73 | R78 | `docs/rounds/r78.md` |
| 74 | R79 | `docs/rounds/r79.md` |
| 75 | R80 | `docs/rounds/r80.md` |
| 76 | R81 | `docs/rounds/r81.md` |
