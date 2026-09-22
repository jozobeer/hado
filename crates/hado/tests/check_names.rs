use hado::{check_source, types::Ty};

#[test]
fn intrinsics_are_only_callable_from_stdlib() {
    for path in [
        "user.hd",
        "not_stdlib/a.hd",
        "stdlib_name/a.hd",
        "/tmp/stdlib/string.hd",
    ] {
        let error = check_source("def main = __string_length \"hi\"", path)
            .err()
            .unwrap();
        assert_eq!(error.code.as_deref(), Some("E202"), "{path}");
    }
    let checked = check_source("def main = __string_length \"hi\"", "stdlib/string.hd").unwrap();
    assert_eq!(checked.main, Ty::Int);
    for path in ["user.hd", "stdlib/string.hd"] {
        let error = check_source("def main = __string_length", path)
            .err()
            .unwrap();
        assert_eq!(error.code.as_deref(), Some("E203"));
    }
}

#[test]
fn old_module_implementation_syntax_is_rejected() {
    let source = "module Hidden = struct def value = 1 end\ndef main = 1";
    let error = check_source(source, "t").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E100"));
    assert_eq!((error.line, error.col), (1, source.find('=').unwrap() + 1));
}

#[test]
fn namespaces_allow_type_module_coexistence_but_reject_duplicates() {
    let source = "type M = | MkM   def main = if MkM == MkM => M.id 1 | 0\nmodule S def id(x: Int): Int\nimplement S as M def id x => x";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    let source = "def main = M.id 1\nmodule S def id(x: Int): Int\nimplement S as M def id x => x\nimplement S as M def id x => x";
    assert!(check_source(source, "t").is_err());
}
