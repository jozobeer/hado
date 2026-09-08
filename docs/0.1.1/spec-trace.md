# 規範文の追跡表 v0.1

`docs/0.1.1/spec.md` の規範文（「〜とする」「〜でなければならない」「〜は E xxx」）が、どの検査段階で判定され、何が固定するか。エラーコードを持つ規範文は §50 の表と拒否コーパスが 1 対 1 で固定するので、ここにはコードを持たない規範文と、コードだけでは範囲が読めない規範文を置く。

「固定するもの」の列は R9 で作る判定器を指す。`verify-r9.sh` の項目名は `docs/0.1.1/plan.md` 8 節の R9 に対応する。

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| §2 | eta 展開の `def` は冗長としない | 冗長 | 受理 | `tests/accept/eta.shin` |
| §3 | 引数の型注釈は括弧付き引数構文でのみ書ける | 構文 | E100 | EBNF 生成器（R9-1） |
| §4 | E307 は再帰呼び出しの引数型と仮引数型の単一化が出現検査に失敗した場合 | 型 | E307 / E306 | `tests/reject/E307.shin`、`E306.shin` |
| §5 | `main` の要求は実行ファイルの規則 | 名前解決 | E206 | `tests/reject/E206.shin` |
| §8 | constructor を先頭に置く並置は E106。`inc(1)` は通す | 構文 | E106 | `tests/reject/E106.shin`、R9-1 の判定器 |
| §8 | constructor は関数値ではない。payload の数の不一致は E301 | 型 | E301 | R9-3 の判定器（0 個と arity+1 個） |
| §12 | record は必ず名前を持つ | 構文 | E100 | EBNF 生成器 |
| §12 | 型の別名は無い | 構文 | E100 | EBNF 生成器 |
| §16 | 負のリテラルパターン | 構文 | 受理 | EBNF 生成器、E405 の判定器 |
| §19 | `()` はパターンではない | 構文 | E100 | EBNF 生成器 |
| §22 | `let` `let?` のパターンは網羅的 | 網羅性 | E402 | R9-4 の判定器（refutable な `let`） |
| §23 | `do` は無い | 構文 | E100 | EBNF 生成器 |
| §25 | パイプの右辺に constructor は置けない | 構文 | E106 | R9-1 の判定器 |
| §27 | E505 は pattern 束縛と引数に及ぶ | 冗長 | E505 | R9-4 の判定器 |
| §27 | E513 は `type` `module type` `module` に及ぶ。prelude は免除 | 冗長 | E513 | R9-3 の判定器 |
| §34 | signature の型名は struct の有無によらず解決される | 名前解決 | E204 | R9-3 の判定器 |
| §34 | sig の型は抽象型のみ | 構文 | E100 | EBNF 生成器 |
| §35 | struct は sig の全 member と型を持ち、型が一致し、十分に多相 | 型 | E602 | `tests/reject/E602.shin`、R9-3 の判定器（4 種類） |
| §41 | 比較は非結合 | 構文 | E100 | EBNF 生成器 |
| §41 | 空白とコメントは区切りで、意味を持たない | 字句 | 受理 | R9-1 の空白摂動（識別子と `(` の間の空白の有無で同じ木になる） |
| §43 | Char のエスケープは 4 つ | 字句 | E111 | `tests/reject/E111.shin` |
| §44 | `%` は Int 専用 | 型 | E305 | R9-2 の ill-typed 生成器 |
| §44 | 短絡評価 | 実行 | 受理 | `tests/accept/short_and.shin` `short_or.shin` |
| §45 | `compare` は Fn を含まない全型 | 単相化 | E704 | `tests/reject/E704.shin`、`verify-m5.sh` の comparison プローブ |
| §46 | パターンに List リテラルの糖衣は無い | 構文 | E100 | EBNF 生成器 |
| §47 | shadowing は許す | 名前解決 | 受理 | `tests/accept/display.shin`（`Member(user) => user.name`） |
| §48 | 末尾位置の呼び出しはスタックを消費しない（3 経路） | 実行 | 受理 | `tests/accept/tail_self.shin` `tail_mutual.shin` `tail_closure.shin` |
| §49 | prelude は E513 を除く全ての検査を受ける | 全段階 | — | `stdlib/` が全プログラムに連結されて `check_source` を通ること。R9-3 で免除を E513 に閉じる |
| §50 | 段階順で最初の 1 件を報告する | 全段階 | — | R9-2 の判定器（同居させた欠陥） |
| §50 | 同一段階内はアンカー位置順 | 全段階 | — | R9-2 の判定器（位置の入れ替え） |
| §50 | アンカーは表の列どおり | 全段階 | — | 位置の摂動プローブ（`-- error:` 行の直後に空行）を全コードに広げる |
| §51 | Result の外側だけ剥がす | 実行 | 受理 | `tests/accept/create_user.shin` `division_error.shin` |
| §51 | List は `[1, 2]`、入れ子の String は literal 形 | 実行 | 受理 | R9-5 の印字プローブ |
| §51 | 最上位の String は生、Unit は無印字 | 実行 | 受理 | `tests/accept/display.shin` `unit.shin` |
| §51 | Float は指数なしの最短往復 | 実行 | 受理 | R9-5 の印字プローブ（`3.0` `0.1` `-0.0` と極端な値） |
| §52 | E515 の 4 条件 | 冗長 | E515 | `tests/reject/E515.shin`、R9-4 の判定器（条件を 1 つずつ外す 4 プローブ） |
