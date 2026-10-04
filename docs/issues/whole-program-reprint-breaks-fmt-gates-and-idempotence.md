# 全体の印字し直しが既存の fmt 門番を赤にし、冪等でもない（whole-program-reprint-breaks-fmt-gates-and-idempotence）

優先度: 高。

## 現象

`hado fmt` が §57 の書き換えのあとにソース全体を印字し直す設計にすると、既存の門番 4 本が赤になり、`fmt` が冪等でなくなる。同じ門番を main で回すと緑である。

```
$ bash scripts/verify-r57.sh
NG: 57-f check --fix, fmt and build --fix share one rewrite
PASSED: 1/6

$ bash scripts/verify-r64.sh
AssertionError: (b'fmt: main.hd (1 rewrites)\n', b'')
NG: 64-c an empty file is untouched
PASSED: 1/4
```

r52（2/5、52-a・52-c・52-d）と r53（4/5、53-f）も赤になる。r62（1/4）と r68（1/4）は main でも同じ赤なので、この設計の起因ではない。

### 冪等でない

`fmt` の直後に `fmt --check` を当てると終了コード 1 を返す。`handler` の腕の間にある空行が、項目の外にある次の空行と重なり、1 回目の出力に空行が 2 行並ぶ。2 回目でようやく 1 行に縮む。

```
$ ./target/debug/hado fmt target/idem/main.hd
fmt: main.hd (1 rewrites)
$ ./target/debug/hado fmt --check target/idem/main.hd
fmt: main.hd is not canonical (1 rewrites)
$ echo $?
1
```

受理コーパス 146 本の字下げと空行を揺らした試験では、11 本が同じ理由で 1 回では収まらなかった。

### 仕様の文に反する

PR #76 が §58.1 に足した「空のファイルはそのままである」に反する（64-c）。同じ段落の「改行コード（CRLF）は変えない」にも反し、CRLF のファイルが LF だけに書き換わる（コメント末尾の `\r` も消える）。

## 原因

`crates/hado/src/fixes.rs` の `format` が、§57 の書き換えを尽くしたあと `canonical_program` で全体を印字し直す。印字器の配置（`crates/hado/src/postfix_print.rs`）は §57 の書き換えと同じ結果を出すようには作られておらず、`check --fix` と `fmt` が同じ書き換えを共有するという r52-a・r57-f の前提も満たさない。

## 案

- **A: 印字し直しを、§57 の書き換えが尽きたあとの追加の書き換えとして位置づけ直し、門番の期待を新しい設計へ揃える（推奨）。** ただし「`check --fix` と `fmt` が同じ書き換えを共有する」を仕様がまだ要求しているかを先に決める。要求しているなら、印字し直しは `fmt` だけのものとして §58.1 に明記する。
- B: 印字し直しをやめ、字下げと改行の書き換えを §57 の書き換えとして 1 つずつ足す。門番はそのままで緑になるが、`(f a) b` の欠陥（`fmt-reprints-nested-application-as-one-call.md`）を別に直す必要がある。
- 空のファイルと CRLF は、いずれの案でも §58.1 の文に合わせる（印字し直しの対象から外す）。

## 完了条件

- r52・r53・r57・r64 が緑になる（または、期待を新しい設計へ揃えた門番が緑になり、揃えた理由が `docs/rounds/` に書かれている）。
- `fmt` の直後の `fmt --check` が終了コード 0 を返す（冪等）。門番で測る。
- 空のファイルと CRLF のファイルが変わらない。門番で測る。
- 変異（空行の重なり、空のファイルの印字）で門番が赤になる。

## 由来

PR #76（`fmt-layout`、F5）のレビューで見つけた。`docs/plans/issue-backlog.md` の F5。仕様は §57（局所的な書き換え）、§58.1（一括整形）。
