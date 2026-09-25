# `build`・`run`・`test` の検査の診断が `--format json` を無視する（build-json-diagnostics）
優先度: 中。

## 現象

§58.4 は `--format json` が `build` / `run` / `test` でも同じと定めるが、読み込みの後の検査（名前解決から契約まで）で止まる診断は、3 つとも既定の 1 行で出る（2026-09-25、canonical-in-syntax-errors の後の木）。`check --format json` は同じ入力で JSON 1 行を出す。

```
$ hado build --format json -o out main.hd    # tests/reject/E520/main.hd
main.hd:3:20: error[E520]: use a Boolean branch instead of Boolean pattern arms or a single condition arm; canonical form: if b => 1 | 0
$ hado run --format json main.hd
main.hd:3:20: error[E520]: use a Boolean branch instead of Boolean pattern arms or a single condition arm; canonical form: if b => 1 | 0
$ hado test --format json broken_test.hd    # 同じ本文
broken_test.hd:3:20: error[E520]: use a Boolean branch instead of Boolean pattern arms or a single condition arm; canonical form: if b => 1 | 0
test: broken_test.hd failed (build)
test: 0/1 passed
```

## 原因

`crates/hado/src/project.rs` の `build` は検査の診断を `report` で既定の 1 行の文字列にしてから返すので、`crates/hado/src/main.rs` の `build_command` と `execute` は形式を選べない。読み込みで止まる診断（E1xx など）は `load_error_report` が JSON にする。

## 案

- 推奨: `build` の失敗を、診断とそれ以外の文字列に分けて返し、`main.rs` の `print_diagnostic` と同じ経路で描く。
- 退ける: `Project` に形式の旗を持たせる。§58.4 の形式は CLI の選択で、LSP は使わない。

## 完了条件

- 上の 3 つが `check --format json` と同じ JSON 1 行（`code` が E520、`canonical` が `if b => 1 | 0` を含む）を出す。`test` は続けて `test: broken_test.hd failed (build)` を標準出力に出す。
- 既定の 1 行は変わらない。

## 由来

`docs/wants.md` に無い。canonical-in-syntax-errors の実測（`docs/rounds/canonical-in-syntax-errors.md`）。§58.4。
