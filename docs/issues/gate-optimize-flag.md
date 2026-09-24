# assert で書いた門番は -O で検査が消える（gate-optimize-flag）
優先度: 中。

## 現象

`python3` を `-O`（または `PYTHONOPTIMIZE`）で回すと `assert` が消える。`assert` で書いた検査は、条件が偽でも何も言わずに通る。

```
$ sed -i 's/Void は /Unit は /' docs/spec.md
$ bash scripts/lint-docs.sh 2>&1 | grep '^NG:'
NG: the spec keeps the Void type name and its spelling
$ PYTHONOPTIMIZE=1 bash scripts/lint-docs.sh 2>&1 | grep '^NG:'
NG: R38 spec sentences are pinned verbatim once each
```

`Unit` を戻したことを見る検査（`doc_type_spelling`）が消え、別の検査だけが赤になる。赤の数は 1 のままで、消えたことが出力から読めない。

## 原因

- `scripts/lint-docs.sh` の `doc_type_spelling` が `assert` で書かれており、`-O` で消えることを想定していない。同じ形の検査が他にもある。
- `scripts/verify-spec-model.sh` の `model_case` は `if not __debug__` を先に見て落ちるが、`scripts/lint-docs.sh` にはその門が無い。

## 案

推奨は (a)。

- (a) `scripts/lint-docs.sh` の python の先頭に `if not __debug__` の門を置き、理由を出して落とす。3 行で済み、`-O` のときに黙って通らない。
- (b) 検査を `assert` から `if ... : raise` へ書き換える。行数が増え、`assert` の簡潔さを失う。
- (c) `-O` で回さないと `AGENTS.md` に書く。環境変数が入っていると黙って通るので弱い。

## 完了条件

`PYTHONOPTIMIZE=1 bash scripts/lint-docs.sh` が、検査を消さずに理由を出して非 0 で終わる。門番がこれを測る（`scripts/lint-docs.sh` を `-O` つきで呼び、赤と理由の両方を見る）。

## 由来

`docs/wants.md` になし。PR #34 の審査 5 回目で `scripts/verify-spec-model.sh` に入れた門と同じ穴を、`scripts/lint-docs.sh` にも見つけたもの。
