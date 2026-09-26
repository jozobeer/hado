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
