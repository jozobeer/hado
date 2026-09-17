# R21追加決定: 名前付きfieldのconstructor（2026-09-09）（r21-addendum）
旧 `docs/plan.md` 15 節。実測は `docs/ledger.md`。

## 手順書

ユーザー承認: `type T = Payload { id: Id.T; name: String }` のTは型名、Payloadはconstructor名。位置payloadを `()`、名前付きfieldを `{}` で直接書き、値生成とパターンにも同じconstructor名を使う。§35/47と§39/56の旧record例の疑義は、局所record＋型別名への回避ではなく、この構文とconstructor付き抽象型実体化の追加で解消する。

この工程をR21の完了判定より前に挿入する。仕様→独立審査→コーパス候補の独立審査→lock→判定器のred→実装→全門番とCursor実走審査の順とする。成果物は `docs/spec.md`、本計画、`docs/spec-coverage.md`、`docs/ledger.md`、受理/拒否コーパス、lock、`crates/hado/**`、`scripts/`。既存record全件の構文移行は要求しない。

判定条件:

1. 単一constructorの省略形、ADT列の名前付き/位置payload混在、型引数、生成・印字の往復を検証する。
2. field順を変えて生成し、書かれた順の評価と宣言順のfield対応を独立した期待値で確認する。同じfield集合の別型を混同しない。
3. constructor付きパターンの同名束縛・別名・省略・入れ子、分岐網羅性とlet全域性のE402を検証する。
4. 未知field E204、重複E205、不足E301、型不一致E305、位置/名前形式の取り違えE301をcheck/build/dump-monoの位置摂動で確認する。
5. 単一constructorのfield参照、複数constructorの直接field拒否、公開抽象型の不透明性E601とconstructor非公開E204を検証する。
6. §39/56のFunctor例を新構文へ揃え、異なる適用の型の分離と全R21判定器を通す。新規判定器 `verify-record-constructors.sh` を全門番に含める。

R21の基礎実装は `2a7b1ef`。Cursor再審査は既知旧record疑義を除く範囲でPASS。全門番はまだ赤であり、R21完了を意味しない。追加工程の終了後、R22とv0.1.3の全条件へ戻る。
