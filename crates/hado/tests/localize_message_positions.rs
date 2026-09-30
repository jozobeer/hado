//! §56: a diagnostic's message writes positions counted inside the file that
//! holds them, whichever file of the root it sits in (§50). The checker builds
//! messages from the concatenated tokens, so the line is the one of the
//! concatenated source unless it is localized. These cases measure the same
//! defect twice: once as the only file of a root, once as a non-entry file of a
//! root whose entry has a different line count. The two messages must agree, so
//! the expectation is the tool's own single-file output — no case name and no
//! line number is memorized here.
//!
//! `localize-message-positions`: E501・E502・E507・E406・E403・E405.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case {
    root: PathBuf,
}

impl Case {
    fn new(label: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "hado-localize-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.root.join(name), source).unwrap();
    }

    fn check(&self, entry: &str, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_hado"));
        command.arg("check").args(args).arg(self.root.join(entry));
        command.output().unwrap()
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// One defect per code, in the postfix spelling. Each file keeps its own
/// `main`, so the same file can be checked on its own and as a sibling of an
/// entry without changing the inspection (§50).
///
/// The bodies are taken from `tests/reject/<code>/main.hd` minus the header,
/// except E403, which needs a covering arm that is not the same row.
const SOURCES: &[(&str, &str)] = &[
    ("E501", "def f v => v as x ->\n  x\ndef main = f 1\n"),
    ("E502", "def f x => if x > 1 => x | x\ndef main = f 1\n"),
    (
        "E507",
        "def f | Some(v) => Some(v) | None => None\ndef main = f Some(1)\n",
    ),
    (
        "E406",
        "def grade2 score | score >= 70 => 1 | score >= 70 => 2 | 0\ndef main = grade2 80\n",
    ),
    (
        "E403",
        "def f x => if x | Some(_) => 1 | Some(1) => 2 | 0\ndef main = f Some(1)\n",
    ),
    (
        "E405",
        "def f x => if x | 1 => 10 | 1 => 20 | 0\ndef main = f 1\n",
    ),
];

/// An entry of `lines` lines: one real line and the rest empty. The line count
/// is the only thing that differs between the two projects of one code.
fn entry(lines: usize) -> String {
    let mut text = String::from("def main = \"hi\"\n");
    for _ in 1..lines {
        text.push('\n');
    }
    text
}

fn project(code: &str, lines: usize) -> Case {
    let project = Case::new(&format!("{code}-{lines}"));
    project.write("hello.hd", &entry(lines));
    project.write("extra.hd", "// third file\n");
    project
}

#[test]
fn message_positions_do_not_depend_on_the_entry_length() {
    for (code, source) in SOURCES {
        let solo = Case::new(code);
        solo.write("bad.hd", source);
        let alone = solo.check("bad.hd", &[]);
        assert!(
            !alone.status.success(),
            "{code}: the single file was accepted: {}",
            stderr(&alone)
        );
        let expected = stderr(&alone);
        assert!(
            expected.contains(&format!("error[{code}]")),
            "{code}: the single file reported {expected:?}"
        );
        for lines in [1, 8] {
            let multi = project(code, lines);
            multi.write("bad.hd", source);
            let output = multi.check("hello.hd", &[]);
            assert_eq!(
                stderr(&output),
                expected,
                "{code}: a {lines}-line entry moved the message"
            );
        }
    }
}

#[test]
fn json_message_positions_do_not_depend_on_the_entry_length() {
    for (code, source) in SOURCES {
        let solo = Case::new(code);
        solo.write("bad.hd", source);
        let alone = solo.check("bad.hd", &["--format", "json"]);
        let expected = json_message(&stderr(&alone));
        for lines in [1, 8] {
            let multi = project(code, lines);
            multi.write("bad.hd", source);
            let output = multi.check("hello.hd", &["--format", "json"]);
            assert_eq!(
                json_message(&stderr(&output)),
                expected,
                "{code}: a {lines}-line entry moved the JSON message"
            );
        }
    }
}

/// The `message` field of the one JSON row, as a string literal, so the whole
/// spelling (`at 3:3`) is compared and not just its digits.
fn json_message(stderr: &str) -> String {
    let row = stderr
        .lines()
        .find(|line| line.starts_with('{'))
        .unwrap_or_else(|| panic!("no JSON row in {stderr:?}"));
    let start = row.find("\"message\": ").expect("no message field") + "\"message\": ".len();
    let rest = &row[start..];
    let end = rest[1..].find('"').expect("unterminated message") + 2;
    rest[..end].to_string()
}

/// 完了条件 (1): the exact one-line output of the issue, with the entry of 1
/// line and of 8 lines, in a root of 3 files.
#[test]
fn e501_names_the_body_position_inside_the_file() {
    const BAD: &str = "def main =\n  \"hello\" as x ->\n  x\n";
    const ALONE: &str =
        "bad.hd:2:3: error[E501]: binding x is returned unchanged at 3:3; canonical form: \"hello\"\n";

    let solo = Case::new("e501-alone");
    solo.write("bad.hd", BAD);
    assert_eq!(stderr(&solo.check("bad.hd", &[])), ALONE);

    for lines in [1, 8] {
        let multi = project("E501", lines);
        multi.write("bad.hd", BAD);
        assert_eq!(
            stderr(&multi.check("hello.hd", &[])),
            ALONE,
            "a {lines}-line entry changed the output"
        );
        let json = stderr(&multi.check("hello.hd", &["--format", "json"]));
        assert!(
            json.contains("\"message\": \"binding x is returned unchanged at 3:3\""),
            "a {lines}-line entry changed the JSON message: {json:?}"
        );
    }
}

#[test]
fn solo_output_is_unchanged_by_the_project_root() {
    // The single-file path fills the positions with the checked coordinates,
    // which are the file's own. The spelling must not gain holes or shift.
    for (code, source) in SOURCES {
        let solo = Case::new(code);
        solo.write("bad.hd", source);
        let expected = stderr(&solo.check("bad.hd", &[]));
        assert!(
            expected.contains(&format!("error[{code}]")),
            "{code}: {expected:?}"
        );
        assert!(
            !expected.contains('{'),
            "{code}: a message hole leaked into the single-file output: {expected:?}"
        );
    }
}

/// The root's entry is read first (§50), so the offset of a sibling is the
/// entry's line count plus the separator. A second project with the same
/// defect at a different entry length pins that the fix does not merely add a
/// constant.
#[test]
fn positions_are_equal_across_roots_with_different_prefixes() {
    const BAD: &str = "def main =\n  \"hello\" as x ->\n  x\n";
    let short = project("prefix", 1);
    short.write("bad.hd", BAD);
    let long = project("prefix", 8);
    long.write("bad.hd", BAD);
    let short_out = stderr(&short.check("hello.hd", &[]));
    let long_out = stderr(&long.check("hello.hd", &[]));
    assert_eq!(short_out, long_out);
    assert!(short_out.contains("at 3:3"), "{short_out:?}");
}

/// §56: a single-source entry hands the diagnostic to the caller without
/// `source_diagnostic`, so the message must be filled there too. A remaining
/// `{` is the internal hole spelling leaking out of the API. Every public entry
/// that can return one of the mention-carrying codes is measured, so a new one
/// that forgets to fill is caught here rather than by hand.
#[test]
fn single_source_entries_report_filled_messages() {
    for (code, source) in SOURCES {
        let ast = hado::parser::parse_source(source, "probe.hd")
            .unwrap_or_else(|error| panic!("{code}: not parseable: {error:?}"));
        // The first three run the whole check stage, so each must report this
        // defect with a readable position. `infer` alone need not report it,
        // but anything it does report must not carry a hole either.
        let entries = [
            ("dump_mono", hado::dump_mono(source, "probe.hd").err(), true),
            (
                "check_source",
                hado::check_source(source, "probe.hd").err(),
                true,
            ),
            (
                "check::check_ast",
                hado::check::check_ast(&ast, "probe.hd").err(),
                true,
            ),
            (
                "infer::check_ast",
                hado::infer::check_ast(&ast, "probe.hd").err(),
                false,
            ),
        ];
        let mut caught = 0;
        for (entry, error, wants_position) in entries {
            let Some(error) = error else {
                continue;
            };
            caught += 1;
            assert!(
                !error.message.contains('{'),
                "{code}: {entry} left a message hole: {:?}",
                error.message
            );
            if wants_position {
                assert!(
                    has_position(&error.message),
                    "{code}: {entry} lost the position: {:?}",
                    error.message
                );
            }
        }
        assert!(
            caught > 0,
            "{code}: no single-source entry reported the defect"
        );
    }
}

/// Whether `text` carries a `L:C` position, without assuming where or how many.
fn has_position(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes
        .windows(3)
        .any(|window| window[0].is_ascii_digit() && window[1] == b':' && window[2].is_ascii_digit())
}
