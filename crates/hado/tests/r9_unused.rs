use hado::check_source;

#[test]
fn every_user_declaration_kind_is_checked_for_reachability() {
    for padding in 0..3 {
        for declaration in [
            "def unused x => x + 1".to_owned(),
            format!("type Unused{padding} = | UnusedValue{padding}"),
            format!("module Unused{padding} def value(x: Int): Int /module"),
            format!("implement S as M{padding} def value x => x + 1 /implement"),
        ] {
            let contract = if declaration.starts_with("implement") {
                "module S def value(x: Int): Int /module"
            } else {
                ""
            };
            let source = format!(
                "def main = 1\n{}{declaration}\n{contract}",
                " ".repeat(padding)
            );
            let error = check_source(&source, "unused.hd")
                .err()
                .expect("unused declaration accepted");
            assert_eq!(error.code.as_deref(), Some("E513"), "{source}: {error}");
            assert_eq!(
                (error.line, error.col),
                (2, padding + 1),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn references_through_modules_signatures_and_inferred_records_are_used() {
    for source in [
        "type Box = | Pack(Int)\ndef main = Pack(1)",
        "type Box = { value: Int }\ndef main = { value = 1 }",
        "type Box = | Pack(Int)\n\n\ndef main = M.make 1\nmodule S def make(x: Int): Box /module\nimplement S as M def make x => Pack(x) /implement",
        "def add x => x + 1\n\n\ndef main = M.run 1\nmodule S def run(x: Int): Int /module\nimplement S as M def run x => add x /implement",
        "def main = List.length [1, 2]",
        "def main = 1",
    ] {
        if let Err(error) = check_source(source, "used.hd") { panic!("{source}: {error}"); }
    }
}

#[test]
fn unused_module_is_reported_even_when_its_signature_is_used() {
    let index = 0;
    let source = format!("def main = Used.run {index}\nmodule S def run(x: Int): Int /module\nimplement S as Unused{index} def run x => x + 1 /implement\nimplement S as Used def run x => x + 1 /implement");
    let error = check_source(&source, "unused-module.hd").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E513"));
    assert_eq!((error.line, error.col), (3, 1));
}

#[test]
fn definitions_and_modules_follow_dependencies_in_any_source_order() {
    let declarations = [
        "module S def run(x: Int): Int /module",
        "def increment x => x + 1",
        "implement S as A def run x => increment x /implement",
        "def twice x => A.run (A.run x)",
        "implement S as B def run x => twice x /implement",
        "def main = B.run 40",
    ];
    for rotation in 0..3 {
        for reverse in [false, true] {
            let mut items = declarations.to_vec();
            items.rotate_left(rotation);
            if reverse {
                items.reverse();
            }
            let (sections, outer): (Vec<_>, Vec<_>) = items
                .into_iter()
                .partition(|item| item.starts_with("module") || item.starts_with("implement"));
            let source = format!("{}\n{}", outer.join("\n"), sections.join("\n"));
            assert_eq!(
                check_source(&source, "dependencies.hd").unwrap().main,
                hado::types::Ty::Int
            );
        }
    }
}

#[test]
fn module_calls_to_outer_definitions_execute() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("hado-outer-def-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    let source = "def main = M.run 20\n\ndef add x => x + 1\nimplement S as M def run x => add (add x) /implement\nmodule S def run(x: Int): Int /module";
    std::fs::write(&path, source).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("build")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = Command::new(path.with_extension("")).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"22\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
