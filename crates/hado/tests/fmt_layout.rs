//! §58.1: `hado fmt` は §57 の書き換えを当てたあと、ソース全体を正準形へ
//! 印字し直す。字下げと改行がその一部なので、字下げと改行だけが違う 2 つの
//! ソースは同じバイト列になる。§57 の「書き換える範囲に `//` を含むときは、
//! その書き換えを出さない」は印字し直しにも効くので、コメントは消えない。
//!
//! 測るのは 5 つ。flat と ragged の同一、コメントの前後関係、冪等、
//! 受理コーパス・stdlib・tools の往復、並置の最後の引数の `if`。

use hado::fixes::format;
use hado::parser::parse_source;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn formatted(source: &str) -> String {
    match format(source) {
        Ok(Some((text, _))) => text,
        Ok(None) => source.to_string(),
        Err(limit) => panic!("{limit} 回で収束しない:\n{source}"),
    }
}

/// issue の現象。字下げと改行だけが違う 2 つは同じバイト列になる。
#[test]
fn flat_and_ragged_become_the_same_bytes() {
    let flat = "def size x => if x > 10 => \"large\" | \"small\"\ndef main = size 3\n";
    let ragged = "def size x =>\n      if x > 10\n  => \"large\"\n            | \"small\"\ndef main = size 3\n";
    assert_ne!(flat, ragged);
    assert_eq!(formatted(flat), formatted(ragged));
    assert_eq!(
        formatted(flat),
        "def size x => if x > 10 => \"large\" | \"small\"\ndef main = size 3\n"
    );
}

/// 定義の前のコメントは同じ定義の前に、定義の中のコメントは同じ定義の中に残る。
#[test]
fn comments_stay_before_and_inside_their_definition() {
    let source = "\
// --- 前置き ---
// spec: §41
def f x => x + 1
def g y =>
  // 腕の中のコメント
  if y > 0 => 1 | 0
";
    let text = formatted(source);
    assert!(
        text.contains("// --- 前置き ---\n// spec: §41\ndef f x => x + 1\n"),
        "{text}"
    );
    for comment in ["// --- 前置き ---", "// spec: §41", "// 腕の中のコメント"] {
        assert!(text.contains(comment), "{comment} が消えた:\n{text}");
    }
    // コメントは同じ定義の前にあり、定義の並びは変わらない。
    let index = |needle: &str| text.find(needle).expect("ある");
    assert!(index("// spec: §41") < index("def f x =>"));
    assert!(index("def f x =>") < index("def g y =>"));
    assert!(index("// 腕の中のコメント") > index("def g y =>"));
    assert!(index("// 腕の中のコメント") < index("if y > 0"));
}

/// 並置の最後の引数の `if` は括弧で囲む。`if(...)` だと続きが無く E100 になる（§12.1）。
#[test]
fn last_juxtaposition_argument_if_stays_grouped() {
    for (source, want) in [
        (
            "def main = Int.to_string (if true => 41 | 0)\n",
            "def main = Int.to_string (if true => 41 | 0)\n",
        ),
        (
            "def f x y => x + y\ndef main = Int.to_string (f 2 (if true => 40 | 0))\n",
            "def f x y => x + y\ndef main = Int.to_string (f 2 (if true => 40 | 0))\n",
        ),
        (
            "def main = Int.to_string (if Some(1) | Some(n) => n + 40 | 0)\n",
            "def main = Int.to_string (if Some(1) | Some(n) => n + 40 | 0)\n",
        ),
        (
            "def f x | x > 0 => x as y -> y | 0\ndef main = Int.to_string (f (if true => 41 | 0))\n",
            "def f x | x > 0 => x | 0\ndef main = Int.to_string (f (if true => 41 | 0))\n",
        ),
    ] {
        let text = formatted(source);
        assert_eq!(text, want, "source:\n{source}");
        parse_source(&text, "t").unwrap_or_else(|error| panic!("{error}: {text}"));
        hado::check_source(&text, "t").unwrap_or_else(|error| panic!("{error}: {text}"));
        assert_eq!(formatted(&text), text, "冪等でない:\n{text}");
    }
}

/// 印字し直しは 1 通りに決まる。もう一度当てても何も変わらない。
#[test]
fn formatting_is_idempotent_and_settles() {
    let source = "def f x =>\n   x as y ->\n   if y > 0\n => 1\n | 0\n";
    let once = formatted(source);
    assert_eq!(formatted(&once), once);
    assert!(
        format(&once).expect("収束する").is_none(),
        "2 回目に書き換えが出た:\n{once}"
    );
}

/// 受理コーパス・stdlib・tools の全ファイルが、印字し直しで変わらず、
/// もう一度当てても変わらない。
#[test]
fn corpus_stdlib_and_tools_are_canonical_and_idempotent() {
    let mut count = 0;
    for root in ["tests/accept", "stdlib", "tools"] {
        for path in hado_files(root) {
            let path = path.to_str().expect("utf-8 path");
            let source = fs::read_to_string(path).expect("readable");
            let once = formatted(&source);
            assert_eq!(once, source, "{path}: 印字し直しで変わった");
            assert_eq!(formatted(&once), once, "{path}: 冪等でない");
            count += 1;
        }
    }
    assert!(count > 0);
}

fn hado_files(root: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    collect(&repo_root().join(root), &mut found);
    found.sort();
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
