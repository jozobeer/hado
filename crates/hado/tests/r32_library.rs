use hado::parser::{parse_source, Item};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

fn parse(source: &str) -> hado::parser::Ast {
    parse_source(source, "generated.hd").unwrap_or_else(|error| panic!("{error}\n{source}"))
}

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r32-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, file: &str, source: &str) {
        std::fs::write(self.0.join(file), source).unwrap();
    }
    fn command(&self, command: &str, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado")).arg(command).arg(self.0.join(entry)).output().unwrap()
    }
}
impl Drop for Case { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

const LIBRARY: &str = "def helper value => value + 1\n\nmodule Store\ndef read(value: Int): Int\n\nimplement Store as Memory\ndef read value => helper value";

#[test]
fn load_decides_inspection_from_the_entry_file() {
    let case = Case::new();
    case.write("lib.hd", LIBRARY);
    let project = hado::project::Project::load(&case.0.join("lib.hd")).unwrap();
    assert!(project.is_library());
    project.check().unwrap();
    assert!(project.dump_mono().is_err());

    let case = Case::new();
    case.write("main.hd", "def main = 1");
    let project = hado::project::Project::load(&case.0.join("main.hd")).unwrap();
    assert!(!project.is_library());
    project.check().unwrap();
    assert!(project.dump_mono().is_ok());
}

#[test]
fn check_accepts_library_while_build_reports_entry_error() {
    let case = Case::new();
    case.write("lib.hd", LIBRARY);
    let check = case.command("check", "lib.hd");
    assert_eq!(check.status.code(), Some(0), "{}", String::from_utf8_lossy(&check.stderr));
    assert!(check.stderr.is_empty());
    for command in ["build", "dump-mono"] {
        let output = case.command(command, "lib.hd");
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.starts_with("lib.hd:1:1: error[E206]: "), "{stderr}");
        assert!(stderr.contains("check") && stderr.contains("library"), "{stderr}");
        assert!(!case.0.join("lib").exists());
    }
}

#[test]
fn library_is_accepted_and_recorded() {
    let ast = parse("library");
    assert_eq!(ast.library.map(|span| (span.line, span.col)), Some((1, 1)));
}

#[test]
fn library_span_is_the_first_declaration() {
    let ast = parse("// header\n\nlibrary\n");
    assert_eq!(ast.library.map(|span| (span.line, span.col)), Some((3, 1)));
}

#[test]
fn library_may_be_followed_by_outer_items_and_sections() {
    for source in [
        "library\ndef main = 1",
        "library\ntype T = | Case",
        "library\nmodule M\ndef f(x: Int): Int",
        "library\neffect E\nabort fail",
    ] {
        assert!(parse(source).library.is_some(), "{source}");
    }
}

#[test]
fn repeated_library_is_left_to_the_assembler() {
    // §59: 同一ファイルの 2 つ目は名前解決の E205 であって、構文エラーではない。
    assert!(parse("library\nlibrary").library.is_some());
}

#[test]
fn def_main_is_still_an_item_next_to_library() {
    let ast = parse("library\ndef main = 1");
    assert!(ast.items.iter().any(|item| matches!(item, Item::Def(def) if def.name == "main")));
}

#[test]
fn library_after_the_first_heading_is_a_syntax_error() {
    // §41: outer_item は最初の見出しより前にだけ現れる。
    let error = parse_source("def main = 1\neffect E\nabort stop\nlibrary\n", "generated.hd")
        .expect_err("library after a section heading must be rejected");
    assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
    assert_eq!((error.line, error.col), (4, 1), "{error}");
}

#[test]
fn main_of_a_library_directory_is_e206_before_a_duplicate_library() {
    // §50: E205 と E206 は同じ名前解決の段階なので、アンカー位置のソース順で選ぶ。
    let case = Case::new();
    case.write("main.hd", "def main = 1\nlibrary\nlibrary\n");
    let error = match hado::project::Project::load(&case.0.join("main.hd")) {
        Ok(_) => panic!("expected E206"),
        Err(hado::project::LoadError::Diagnostic(error)) => error,
        Err(other) => panic!("unexpected load error: {other}"),
    };
    assert_eq!(error.code.as_deref(), Some("E206"), "{error}");
    assert_eq!((error.line, error.col), (1, 1), "{error}");
}

#[test]
fn library_and_shared_are_not_names() {
    for source in ["def library = 1", "def shared = 1", "type shared = | A"] {
        let error = parse_source(source, "generated.hd").expect_err(source);
        assert_eq!(error.code.as_deref(), Some("E100"), "{source}: {error}");
    }
}
