//! §58.1: `hado fmt` は §57 の書き換えを当てたあと、ソース全体を正準形へ
//! 印字し直す。字下げと改行がその一部なので、字下げと改行だけが違う 2 つの
//! ソースは同じバイト列になる。§57 の「書き換える範囲に `//` を含むときは、
//! その書き換えを出さない」は印字し直しにも効くので、コメントは消えない。
//!
//! 測るのは 4 つ。flat と ragged の同一、コメントの前後関係、冪等、
//! 受理コーパス・stdlib・tools の往復。

use hado::fixes::format;
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

/// §58.1: 見出しと同じ行の `=>` の後ろにコメントがある定義も、本体を次の行へ
/// 送ってコメントを同じ定義の中に残す（`verify-r62.sh` の 62-b と同じ形）。
/// 本体の最後の後ろのコメント（同じ行の行末）は本体の中ではない。
#[test]
fn a_comment_on_the_definition_line_stays_inside_the_definition() {
    let source = "def f x => // 腕の中のコメント\n  x + 1\ndef main = f 1\n";
    let text = formatted(source);
    assert_eq!(
        text,
        "def f x =>\n  // 腕の中のコメント\n  x + 1\ndef main = f 1\n"
    );
    assert_eq!(formatted(&text), text);
    // 本体の最後の後ろのコメント（同じ行の行末）は本体の中ではない。次の項目の
    // 直前に置く（§58.1 の第 1 文。字下げは元の位置のまま）。
    let trailing = "def f x => x + 1 // 行末\ndef main = f 1\n";
    let text = formatted(trailing);
    assert_eq!(text, "def f x => x + 1\n  // 行末\ndef main = f 1\n");
    assert_eq!(formatted(&text), text);
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
