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
        let dir = std::env::temp_dir().join(format!(
            "hado-r32-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, file: &str, source: &str) {
        let path = self.0.join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, source).unwrap();
    }
    fn command(&self, command: &str, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg(command)
            .arg(self.0.join(entry))
            .output()
            .unwrap()
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const LIBRARY: &str = "def helper value => value + 1\nmodule Store\ndef read (value: Int): Int\n/module\nimplement Store as Memory\ndef read value => helper value\n/implement";

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
    assert_eq!(
        check.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(check.stderr.is_empty());
    for command in ["build", "dump-mono"] {
        let output = case.command(command, "lib.hd");
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.starts_with("lib.hd:1:1: error[E206]: "), "{stderr}");
        assert!(
            stderr.contains("check") && stderr.contains("library"),
            "{stderr}"
        );
        assert!(!case.0.join("lib").exists());
    }
}

#[test]
fn library_is_accepted_and_recorded() {
    let ast = parse("library");
    assert_eq!(ast.library.map(|span| (span.line, span.col)), Some((1, 1)));
    // The span is the declaration, not the start of the file.
    let ast = parse("// header\n\nlibrary\n");
    assert_eq!(ast.library.map(|span| (span.line, span.col)), Some((3, 1)));
}

#[test]
fn library_may_be_followed_by_outer_items_and_sections() {
    for source in [
        "library\ndef main = 1",
        "library\ntype T = | Case",
        "library\nmodule M\ndef f (x: Int): Int\n/module",
        "library\neffect E\nabort fail",
    ] {
        assert!(parse(source).library.is_some(), "{source}");
    }
}

#[test]
fn repeated_library_is_left_to_the_assembler() {
    // §52: 同一ファイルの 2 つ目は名前解決の E205 であって、構文エラーではない。
    assert!(parse("library\nlibrary").library.is_some());
}

#[test]
fn def_main_is_still_an_item_next_to_library() {
    let ast = parse("library\ndef main = 1");
    assert!(ast
        .items
        .iter()
        .any(|item| matches!(item, Item::Def(def) if def.name == "main")));
}

#[test]
fn library_after_the_first_heading_is_a_syntax_error() {
    // 付録C: outer_item は最初の見出しより前にだけ現れる。
    let error = parse_source(
        "def main = 1\neffect E\nabort stop\nlibrary\n",
        "generated.hd",
    )
    .expect_err("library after a section heading must be rejected");
    assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
    assert_eq!((error.line, error.col), (4, 1), "{error}");
}

#[test]
fn main_of_a_library_directory_is_e206_before_a_duplicate_library() {
    // §56: E205 と E206 は同じ名前解決の段階なので、アンカー位置のソース順で選ぶ。
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

#[test]
fn argument_main_reports_the_declaration_position_for_every_command() {
    // §56: check / build / dump-mono は同じアンカー（その `def main`）を指す。
    let case = Case::new();
    let source = "\n\ndef main x => x";
    case.write("main.hd", source);
    let line = source[..source.find("def main").unwrap()]
        .matches('\n')
        .count()
        + 1;
    for command in ["check", "build", "dump-mono"] {
        let output = case.command(command, "main.hd");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(output.status.code(), Some(1), "{command}: {stderr}");
        assert!(
            stderr.starts_with(&format!("main.hd:{line}:1: error[E206]: ")),
            "{command}: {stderr}"
        );
    }
    assert!(!case.0.join("main").exists());
}

#[test]
fn paths_resolve_from_the_innermost_library_root() {
    // §51, §52: a nested `library` makes its directory the root of its own
    // subtree, so `leaf::L` is read from `inner/`, not from the outer `lib/`.
    let case = Case::new();
    case.write("app/main.hd", "def main = lib::inner::M.go 7");
    case.write("app/lib/lib.hd", "library");
    case.write("app/lib/inner/inner.hd", "library");
    case.write(
        "app/lib/inner/leaf/l.hdi",
        "module L\ndef go (x: Int): Int\n/module",
    );
    case.write(
        "app/lib/inner/leaf/lm.hd",
        "implement L as LM\ndef go x => x + 3\n/implement",
    );
    case.write(
        "app/lib/inner/m.hd",
        "implement leaf::L as M\ndef go x => leaf::LM.go x\n/implement",
    );
    let project = hado::project::Project::load(&case.0.join("app/main.hd")).unwrap();
    project.check().unwrap();
}

#[test]
fn library_directory_rejects_an_argument_main_in_a_separate_file() {
    // §52: 禁止は定義の形によらない。値定義だけを探してはいけない。
    let case = Case::new();
    case.write("lib.hd", "library");
    let source = "def main x => x\nmodule S\ndef read (value: Int): Int\n/module\nimplement S as Memory\ndef read value => main value\n/implement";
    case.write("main.hd", source);
    let line = source[..source.find("def main").unwrap()]
        .matches('\n')
        .count()
        + 1;
    let error = match hado::project::Project::load(&case.0.join("lib.hd")) {
        Ok(_) => panic!("expected E206"),
        Err(hado::project::LoadError::Diagnostic(error)) => error,
        Err(other) => panic!("unexpected load error: {other}"),
    };
    assert_eq!(error.code.as_deref(), Some("E206"), "{error}");
    assert_eq!((error.line, error.col), (line, 1), "{error}");
    assert_eq!(error.path, "main.hd", "{error}");
}

#[test]
fn a_library_root_conflicts_with_the_prelude_embedded_and_alone() {
    // §51, §52: a library root does not compare with the user root, but the
    // prelude stays visible inside the library, so its own `type Option` is E205.
    let case = Case::new();
    case.write("app/main.hd", "def main = 1");
    case.write("app/lib/lib.hd", "library\n\ntype Option = | Local");
    for (entry, path) in [("app/main.hd", "lib/lib.hd"), ("app/lib/lib.hd", "lib.hd")] {
        let project = hado::project::Project::load(&case.0.join(entry)).unwrap();
        let error = match project.check() {
            Ok(_) => panic!("expected E205 for {entry}"),
            Err(error) => error,
        };
        assert_eq!(error.code.as_deref(), Some("E205"), "{entry}: {error}");
        assert_eq!(error.path, path, "{entry}: {error}");
        assert_eq!((error.line, error.col), (3, 6), "{entry}: {error}");
    }
}

#[test]
fn a_library_root_conflicts_with_its_subdirectories() {
    // §51: the library root is the root of its subtree, so a same-named
    // declaration in a subdirectory points at the subdirectory's declaration.
    let types = Case::new();
    types.write("app/main.hd", "def main = 1");
    types.write("app/lib/lib.hd", "library\n\ntype Found = | A");
    types.write("app/lib/store/store.hd", "type Found = | B");
    let project = hado::project::Project::load(&types.0.join("app/main.hd")).unwrap();
    let error = match project.check() {
        Ok(_) => panic!("expected E205 for the type conflict"),
        Err(error) => error,
    };
    assert_eq!(error.code.as_deref(), Some("E205"), "{error}");
    assert_eq!(error.path, "lib/store/store.hd", "{error}");
    assert_eq!((error.line, error.col), (1, 6), "{error}");

    let contracts = Case::new();
    contracts.write("app/main.hd", "def main = 1");
    contracts.write(
        "app/lib/lib.hd",
        "library\nmodule Store\ndef read (x: Int): Int\n/module",
    );
    contracts.write(
        "app/lib/store/store.hd",
        "module Store\ndef read (x: Int): Int\n/module",
    );
    let project = hado::project::Project::load(&contracts.0.join("app/main.hd")).unwrap();
    let error = match project.check() {
        Ok(_) => panic!("expected E205 for the contract conflict"),
        Err(error) => error,
    };
    assert_eq!(error.code.as_deref(), Some("E205"), "{error}");
    assert_eq!(error.path, "lib/store/store.hd", "{error}");
    assert_eq!((error.line, error.col), (1, 8), "{error}");
}
