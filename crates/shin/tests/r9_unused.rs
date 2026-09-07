use shin::check_source;

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
            let error = check_source(&source, "unused.shin").err().expect("unused declaration accepted");
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
        assert!(check_source(source, "used.shin").is_ok(), "{source}");
    }
}
