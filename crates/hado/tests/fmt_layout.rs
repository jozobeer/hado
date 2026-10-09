//! §58.1: `hado fmt` は §57 の書き換えを当てたあと、ソース全体を正準形へ
//! 印字し直す。字下げと改行がその一部なので、字下げと改行だけが違う 2 つの
//! ソースは同じバイト列になる。§57 の「書き換える範囲に `//` を含むときは、
//! その書き換えを出さない」は印字し直しにも効くので、コメントは消えない。
//!
//! 測るのは 6 つ。flat と ragged の同一、コメントの前後関係、冪等、
//! 受理コーパス・stdlib・tools の往復、並置の最後の引数の `if`、右端の
//! 被演算子の `if`。

use hado::fixes::format;
use hado::parser::parse_source;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hado-fmt-layout-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// 入力の隣に `main.hd` を置いて `hado run` する。`hado` は入力の親を根として
/// 配下の `.hd` を読むので、1 つの根に 1 ファイルだけを置く。
fn run_source(source: &str, name: &str) -> String {
    let dir = scratch(name);
    let path = dir.join("main.hd");
    fs::write(&path, source).expect("書ける");
    let out = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("run")
        .arg(&path)
        .output()
        .expect("hado run");
    let _ = fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "{name}: run が失敗した:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// 印字し直した綴りが解析と検査を通ることを確かめる。
fn assert_reprints(name: &str, source: &str) -> String {
    let once = formatted(source);
    assert_eq!(formatted(&once), once, "{name}: 冪等でない:\n{once}");
    parse_source(&once, "t").unwrap_or_else(|error| panic!("{name}: {error}:\n{once}"));
    hado::check_source(&once, "t").unwrap_or_else(|error| panic!("{name}: {error}:\n{once}"));
    once
}

/// 分岐が「右端の被演算子で、後ろに何も続かない位置」に来るとき、`fmt` は
/// `(if …)` と括弧で囲む。`if(...)` は閉じの `)` の直後に閉じ・区切り・入力の
/// 終わりが来ると E100 になる（§12.1）。並置の最後の引数だけでは足りない。
#[test]
fn rightmost_operand_branches_survive_the_reprint() {
    let cases = [
        (
            "sub",
            "def main = Int.to_string (100 - (if true => 59 | 0))\n",
            "41\n",
        ),
        (
            "neg",
            "def main = Int.to_string (- (if true => 41 | 0))\n",
            "-41\n",
        ),
        (
            "neg-argument",
            "def f x => x\ndef main = Int.to_string (f (- (if true => 42 | 0)))\n",
            "-42\n",
        ),
        (
            "not",
            "def b = not (if true => false | true)\ndef main = Int.to_string (if b => 1 | 0)\n",
            "1\n",
        ),
        (
            "and",
            "def b = true && (if true => true | false)\ndef main = Int.to_string (if b => 1 | 0)\n",
            "1\n",
        ),
        (
            "lt",
            "def b = 1 < (if true => 2 | 0)\ndef main = Int.to_string (if b => 1 | 0)\n",
            "1\n",
        ),
    ];
    for (name, source, want) in cases {
        assert_eq!(
            run_source(source, &format!("{name}-before")),
            want,
            "{name}: 入力が動かない:\n{source}"
        );
        let once = assert_reprints(name, source);
        assert_eq!(
            run_source(&once, &format!("{name}-after")),
            want,
            "{name}: fmt の後で値が変わった:\n{once}"
        );
    }
}

/// 右端の被演算子は並置の引数だけではない。入れ子・リストの要素・record の
/// field・束縛の本体・`fn` の本体・`|>` の右辺も同じ規則で囲む。
#[test]
fn rightmost_operand_branches_survive_in_every_position() {
    let cases = [
        (
            "nested-bin",
            "def main = Int.to_string (1 + (2 - (if true => 1 | 0)))\n",
        ),
        (
            "nested-neg",
            "def main = Int.to_string (1 + (- (if true => 1 | 0)))\n",
        ),
        (
            "list-element",
            "def v = [1 + (if true => 1 | 0)]\ndef main = Int.to_string (List.length v)\n",
        ),
        (
            "list-element-neg",
            "def v = [- (if true => 1 | 0)]\ndef main = Int.to_string (List.length v)\n",
        ),
        (
            "record-field",
            "type R = { a: Int }\ndef r = { a = 1 + (if true => 1 | 0) }\ndef main = Int.to_string r.a\n",
        ),
        (
            "binding-body",
            "def main = 1 as y -> Int.to_string (y + (if true => 1 | 0))\n",
        ),
        (
            "fn-body",
            "def main = Int.to_string (fn(x => x + (if true => 1 | 0)) 0)\n",
        ),
        (
            "fn-last-argument",
            "def main = Int.to_string (List.length (List.map [1] fn(x => x + (if true => 1 | 0))))\n",
        ),
        (
            "pipe-right",
            "def inc x => x + 1\ndef dec x => x - 1\ndef main = Int.to_string (1 |> (if true => inc | dec))\n",
        ),
        (
            "call-argument",
            "def main = Int.to_string (Option.unwrap_or (Some (1 + (if true => 1 | 0))) 0)\n",
        ),
    ];
    for (name, source) in cases {
        assert_reprints(name, source);
    }
}

/// 演算子を `(…)` で包む位置（`wrap` が足す括弧の中）も「後ろに何も続かない
/// 位置」である。包んだ `)` が §12.1 の閉じを兼ねるので、その中の右端の分岐も
/// `(if …)` にする。ここを漏らすと `fmt` が E100 を書く。
#[test]
fn wrapped_operand_branches_survive_the_reprint() {
    let cases = [
        (
            "wrapped-neg-argument",
            "def f (a: Int, b: Int): Int => a + b\ndef main = Int.to_string (f (- (if true => 1 | 0)) 2)\n",
            "1\n",
        ),
        (
            "wrapped-bin-left",
            "def main = Int.to_string ((1 + (if true => 1 | 0)) * 2)\n",
            "4\n",
        ),
        (
            "wrapped-bin-argument",
            "def f (a: Int, b: Int): Int => a + b\ndef main = Int.to_string (f (1 + (if true => 1 | 0)) 2)\n",
            "4\n",
        ),
        (
            "wrapped-not-argument",
            "def f (a: Bool, b: Int): Int => if a => b | 0\ndef main = Int.to_string (f (not (if true => false | true)) 3)\n",
            "3\n",
        ),
        (
            "wrapped-pipe-left",
            "def inc x => x + 1\ndef dec x => x - 1\ndef main = Int.to_string ((1 |> (if true => inc | dec)) + 1)\n",
            "3\n",
        ),
        (
            "wrapped-nested-bin",
            "def main = Int.to_string ((10 - (2 - (if true => 1 | 0))) * 2)\n",
            "18\n",
        ),
    ];
    for (name, source, want) in cases {
        assert_eq!(
            run_source(source, &format!("{name}-before")),
            want,
            "{name}: 入力が動かない:\n{source}"
        );
        let once = assert_reprints(name, source);
        assert_eq!(
            run_source(&once, &format!("{name}-after")),
            want,
            "{name}: fmt の後で値が変わった:\n{once}"
        );
    }
}

/// 入れ子の `-` は `neg_text` が `wrap` を通さず自分で `-(…)` と綴る（`--` が
/// コメントの印になるのを避けるため、§7）。この `)` も §12.1 の閉じを兼ねるので、
/// 中身の右端は続きの無い位置になる。続きを足したときだけ壊れる。
#[test]
fn nested_negation_branches_survive_the_reprint() {
    let cases = [
        (
            "nested-neg-add",
            "def main = Int.to_string (- (- (if true => 1 | 0)) + 5)\n",
            "6\n",
        ),
        (
            "nested-neg-pipe",
            "def inc x => x + 1\ndef main = Int.to_string (- (- (if true => 1 | 0)) |> inc)\n",
            "2\n",
        ),
        (
            "nested-neg-binding",
            "def main = Int.to_string (- (- (if true => 1 | 0)) as y -> y + 1)\n",
            "2\n",
        ),
        (
            "nested-neg-comparison",
            "def b2i b => if b => 1 | 0\ndef main = Int.to_string (b2i (- (- (if true => 1 | 0)) < 5))\n",
            "1\n",
        ),
        (
            "nested-neg-triple",
            "def main = Int.to_string (- (- (- (if true => 1 | 0))) + 5)\n",
            "4\n",
        ),
        (
            "nested-neg-implement",
            "module M\ndef n: Int\n/module\nimplement M as M\ndef n = - (- (if true => 2 | 0)) + 1\n/implement\ndef main = Int.to_string M.n\n",
            "3\n",
        ),
    ];
    for (name, source, want) in cases {
        assert_eq!(
            run_source(source, &format!("{name}-before")),
            want,
            "{name}: 入力が動かない:\n{source}"
        );
        let once = assert_reprints(name, source);
        assert_eq!(
            run_source(&once, &format!("{name}-after")),
            want,
            "{name}: fmt の後で値が変わった:\n{once}"
        );
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
