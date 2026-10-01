//! 後置の束縛と `if`・`fn` の閉じを固定する（§11・§12・§34・§35・§53・§57）。
//!
//! 3 つのことを測る。
//! - 受理コーパス・stdlib・tools が 1 通りの正準形で書かれ、同じ構文木へ往復する。
//! - `tests/postfix-close/accept` の閉じの形が構文段階を通る（意味検査は見ない）。
//! - `tests/postfix-close/reject` の閉じの不足・余分・食い違いが E100 になる。

use hado::parser::{canonical_program, parse_source};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn hado_files(root: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&repo_root().join(root), &mut found);
    found.sort();
    assert!(!found.is_empty(), "{root} に .hd が無い");
    found
}

fn collect(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("hd" | "hdi")
        ) {
            found.push(path);
        }
    }
}

/// 正準形の印字が入力と一致し、その印字が再び同じ木へ戻る。
#[test]
fn corpus_is_canonical_and_round_trips() {
    let mut count = 0;
    for root in ["tests/accept", "stdlib", "tools"] {
        for path in hado_files(root) {
            let path = path.to_str().expect("utf-8 path");
            let source = fs::read_to_string(path).expect("readable corpus file");
            let ast = parse_source(&source, path).unwrap_or_else(|error| {
                panic!("{path}: {}", error);
            });
            let printed = canonical_program(&ast);
            assert_eq!(body(&source), body(&printed), "{path}: 正準形ではない");
            let again = parse_source(&printed, path)
                .unwrap_or_else(|error| panic!("{path}: 正準形を読み直せない: {}", error));
            assert_eq!(canonical_program(&again), printed, "{path}: 往復で変わった");
            count += 1;
        }
    }
    assert!(count > 0);
}

/// 行コメントと空行を落とした本文。正準形の綴りをコメント抜きで見比べる。
fn body(source: &str) -> String {
    let mut out = Vec::new();
    for line in source.lines() {
        let text = match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        };
        let text = text.trim_end();
        if !text.is_empty() {
            out.push(text.to_string());
        }
    }
    out.join("\n")
}

fn expected_code(path: &Path) -> Option<String> {
    let source = fs::read_to_string(path).ok()?;
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("// error:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// 閉じの形（正例）は構文段階を通る。意味検査は要らない。
#[test]
fn close_shapes_parse() {
    let mut count = 0;
    for path in hado_files("tests/postfix-close/accept") {
        let path = path.to_str().expect("utf-8 path");
        let source = fs::read_to_string(path).expect("readable");
        parse_source(&source, path).unwrap_or_else(|error| {
            panic!("{path}: {}", error);
        });
        count += 1;
    }
    assert!(count >= 20, "閉じの正例が足りない: {count}");
}

/// 閉じの不足・余分・食い違いは E100。旧形の `let`・`do` も E100。
#[test]
fn close_defects_are_e100() {
    let mut count = 0;
    for path in hado_files("tests/postfix-close/reject") {
        let want = expected_code(&path).expect("// error: がある");
        let path = path.to_str().expect("utf-8 path");
        let source = fs::read_to_string(path).expect("readable");
        let error = match parse_source(&source, path) {
            Ok(_) => panic!("{path}: 誤りを見逃した（want {want}）"),
            Err(error) => error,
        };
        assert_eq!(error.code.as_deref(), Some(want.as_str()), "{path}");
        count += 1;
    }
    assert!(count >= 10, "閉じの負例が足りない: {count}");
}

/// §57: 局所修正（`--fix`）と一括修正（`fmt`）が書く置換も後置の綴りで、
/// 書いた結果を読み直して検査できる。旧形の `let`・`do` を書き戻さない。
#[test]
fn canonical_rewrites_write_parseable_postfix() {
    for name in ["E506", "E507", "E508", "E520"] {
        let path = repo_root().join("tests/reject").join(name).join("main.hd");
        let source = fs::read_to_string(&path).expect("readable");

        // `check --fix`: 診断のアンカーに合う置換を 1 つ当てる。
        let error = hado::check_source(&source, name)
            .err()
            .unwrap_or_else(|| panic!("{name}: 診断が出ない"));
        let code = error.code.clone().expect("code");
        assert_eq!(code.as_ref(), name, "{name}: 別の診断が先に出た");
        let fix = hado::fixes::fix_at(&source, &code, error.line, error.col)
            .unwrap_or_else(|| panic!("{name}: 書き換えが無い"));
        let fixed = format!(
            "{}{}{}",
            &source[..fix.start],
            fix.replacement,
            &source[fix.end..]
        );
        parse_source(&fixed, name)
            .unwrap_or_else(|error| panic!("{name}: --fix の出力を読み直せない: {error}\n{fixed}"));
        hado::check_source(&fixed, name).unwrap_or_else(|error| {
            panic!("{name}: --fix の出力が検査を通らない: {error}\n{fixed}")
        });

        // `fmt`: 当てられる書き換えを一括で当てる。
        let (formatted, applied) = hado::fixes::format(&source)
            .unwrap_or_else(|limit| panic!("{name}: {limit} 回で収束しない"))
            .unwrap_or_else(|| panic!("{name}: 書き換えが 1 つも出ない"));
        assert!(applied > 0);
        parse_source(&formatted, name).unwrap_or_else(|error| {
            panic!("{name}: fmt の出力を読み直せない: {error}\n{formatted}")
        });
        hado::check_source(&formatted, name).unwrap_or_else(|error| {
            panic!("{name}: fmt の出力が検査を通らない: {error}\n{formatted}")
        });
        assert!(
            hado::fixes::format(&formatted).expect("settles").is_none(),
            "{name}: fmt が冪等でない"
        );
    }
}

/// §12.1・§57: `if(...)` の閉じを持つ枝でも、E520 の書き換えは閉じ `)` ごと
/// 置き換えて `if(...)` の綴りを保つ。開き括弧だけを残して壊さない。
#[test]
fn canonical_rewrites_keep_parenthesized_branch_close() {
    let source = "def f (b: Bool): Int =>\n  if(b | true => 1 | false => 2) as v ->\n  v + 1\ndef main = f true\n";
    let error = hado::check_source(source, "paren.hd")
        .err()
        .expect("E520 が出ない");
    assert_eq!(error.code.as_deref(), Some("E520"));
    let fix = hado::fixes::fix_at(source, "E520", error.line, error.col).expect("書き換えが無い");
    let fixed = format!(
        "{}{}{}",
        &source[..fix.start],
        fix.replacement,
        &source[fix.end..]
    );
    assert!(
        fixed.contains("if(b => 1 | 2) as v ->"),
        "閉じを壊した: {fixed}"
    );
    parse_source(&fixed, "paren.hd")
        .unwrap_or_else(|error| panic!("--fix の出力を読み直せない: {error}\n{fixed}"));
    hado::check_source(&fixed, "paren.hd")
        .unwrap_or_else(|error| panic!("--fix の出力が検査を通らない: {error}\n{fixed}"));
}

/// §11: 正準形の印字も後置で、旧形の `let`・`do` を出さない。
#[test]
fn canonical_spelling_uses_postfix_bindings() {
    let source = "def main =\n  1 as x ->\n  x + 1 as y ->\n  y\n";
    let ast = parse_source(source, "postfix.hd").unwrap();
    let printed = hado::parser::canonical(ast.main().expect("main"));
    assert_eq!(printed, "(1 as x -> ((x + 1) as y -> y))");
    assert!(!printed.contains("let"), "{printed}");
    assert!(!printed.contains("do "), "{printed}");
    parse_source(&format!("def main = {printed}"), "postfix.hd")
        .unwrap_or_else(|error| panic!("正準形を読み直せない: {error}\n{printed}"));
}
