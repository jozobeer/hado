use hado::check_source;

#[test]
fn every_user_declaration_kind_is_checked_for_reachability() {
    for padding in 0..12 {
        for declaration in [
            "def unused x => x + 1".to_owned(),
            format!("type Unused{padding} = | UnusedValue{padding}"),
            format!("module type Unused{padding} = sig def value(x: Int): Int end"),
            format!("module M{padding} : S = struct def value x => x + 1 end"),
        ] {
            let prefix = if declaration.starts_with("module M") { "module type S = sig def value(x: Int): Int end\n" } else { "" };
            let source = format!("{prefix}{}{declaration}\ndef main = 1", " ".repeat(padding));
            let error = check_source(&source, "unused.hd").err().expect("unused declaration accepted");
            assert_eq!(error.code.as_deref(), Some("E513"), "{source}");
            // An unused module also makes its signature unreachable; the earlier declaration wins.
            let expected = if prefix.is_empty() { (1, padding + 1) } else { (1, 1) };
            assert_eq!((error.line, error.col), expected, "{source}");
        }
    }
}

#[test]
fn references_through_modules_signatures_and_inferred_records_are_used() {
    for source in [
        "type Box = | Pack(Int)\ndef main = Pack(1)",
        "type Box = { value: Int }\ndef main = { value = 1 }",
        "type Box = | Pack(Int)\nmodule type S = sig def make(x: Int): Box end\nmodule M : S = struct def make x => Pack(x) end\ndef main = M.make 1",
        "def add x => x + 1\nmodule type S = sig def run(x: Int): Int end\nmodule M : S = struct def run x => add x end\ndef main = M.run 1",
        "def main = List.length [1, 2]",
        "def main = 1",
    ] {
        if let Err(error) = check_source(source, "used.hd") { panic!("{source}: {error}"); }
    }
}

#[test]
fn unused_module_is_reported_even_when_its_signature_is_used() {
    for index in 0..12 {
        let source = format!("module type S = sig def run(x: Int): Int end\nmodule Unused{index} : S = struct def run x => x + 1 end\nmodule Used : S = struct def run x => x + 1 end\ndef main = Used.run {index}");
        let error = check_source(&source, "unused-module.hd").err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E513"));
        assert_eq!((error.line, error.col), (2, 1));
    }
}

#[test]
fn definitions_and_modules_follow_dependencies_in_any_source_order() {
    let declarations = [
        "module type S = sig def run(x: Int): Int end",
        "def increment x => x + 1",
        "module A : S = struct def run x => increment x end",
        "def twice x => A.run (A.run x)",
        "module B : S = struct def run x => twice x end",
        "def main = B.run 40",
    ];
    for rotation in 0..declarations.len() {
        for reverse in [false, true] {
            let mut items = declarations.to_vec();
            items.rotate_left(rotation);
            if reverse { items.reverse(); }
            let source = items.join("\n");
            assert_eq!(check_source(&source, "dependencies.hd").unwrap().main, hado::types::Ty::Int);
        }
    }
}

#[test]
fn module_calls_to_outer_definitions_execute() {
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("hado-outer-def-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    let source = "def main = M.run 20\nmodule M : S = struct def run x => add (add x) end\ndef add x => x + 1\nmodule type S = sig def run(x: Int): Int end";
    std::fs::write(&path, source).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&path).output().unwrap();
    assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
    let output = Command::new(path.with_extension("")).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"22\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
