# `print-postfix` が読めなかった項目を黙って落とし、空のファイルを書く（print-postfix-drops-unconvertible-items）
優先度: 低。

## 現象

移行の入口で構文として読めなかった項目は、診断も出さずに AST から落ち、その分だけ空になったファイルが出力先へ書かれる。`?` の廃止で `let p? =` が E100 になったので、この経路に新しく入る入力が増えた。

```
$ printf 'def f value => let x? = value -> x\n' > /tmp/l/legacy.hd
$ cargo run --offline -q -- print-postfix /tmp/l /tmp/out
print-postfix: files=1 hd=1 hdi=0 skipped=0 dropped-comments=0
$ wc -c /tmp/out/legacy.hd
0 /tmp/out/legacy.hd
```

`skipped=0` なので、走らせた側からは「1 本写した」ように見える。字句で読めない入力だけが `skip: <path>: <diagnostic>` を出して `skipped` に数えられる。

## 原因

`crates/hado/src/main.rs` の `convert_postfix_file` は `parse_for_conversion` の戻り値をそのまま `canonical_program` へ渡す。`crates/hado/src/parser.rs` の `parse_for_conversion` は `parse_tokens_partial_with` の `ast` だけを返し、`PartialParse::diagnostics` を捨てる。構文で読めなかった項目は木に残らないので、出力は空になる。

## 案

- 推奨: `parse_partial_for_conversion` を使い、診断が 1 つでもあればその内容を `skip:` として出して `Ok(None)` を返す（`skipped` に数える）。空のファイルを書かない。
- 次点: 出力が空になったファイルを数え、末尾の要約に `emptied=<n>` として出す。書き込みはそのままにする（移行の下流が空ファイルを期待しているなら壊さない）。
- 退ける: `?` の E100 だけを特別扱いして `let p? =` を含むファイルを弾く。他の壊れた入力が同じ経路に残る。

## 完了条件

- 構文で読めない項目を含むファイルは、診断つきで `skipped` に数えられ、出力先へ書かれない。
- 空のファイルが黙って書かれないことを門番が測る（`scripts/verify-r57.sh` か `scripts/verify-r71.sh` の隣）。
- `cargo test --offline --test postfix_close` が緑で、`bash scripts/run-gates.sh --tier full` が red なし。

## 由来

`docs/rounds/do-option-absent.md` の「仕様への疑義」から。`?` の廃止（D2）で `let p? =` が E100 になり、移行の入口がこの経路に入ることを実測で確かめた。`docs/rounds/postfix-binding.md`（`print-postfix` の新設）の残り。
