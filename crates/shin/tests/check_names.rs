use shin::{check_source, types::Ty};

#[test]
fn intrinsics_are_only_callable_from_stdlib() {
    for path in ["user.shin", "not_stdlib/a.shin", "stdlib_name/a.shin"] {
        let error = check_source("def main = __string_length \"hi\"", path)
            .err()
            .unwrap();
        assert_eq!(error.code.as_deref(), Some("E202"));
    }
    let checked = check_source(
        "def main = __string_length \"hi\"",
        "/tmp/stdlib/string.shin",
    )
    .unwrap();
    assert_eq!(checked.main, Ty::Int);
    for path in ["user.shin", "stdlib/string.shin"] {
        let error = check_source("def main = __string_length", path)
            .err()
            .unwrap();
        assert_eq!(error.code.as_deref(), Some("E203"));
    }
}

#[test]
fn module_requires_signature_annotation() {
    let source = "module Hidden = struct def value = 1 end\ndef main = 1";
    let error = check_source(source, "t").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E113"));
    assert_eq!((error.line, error.col), (1, 1));
}

#[test]
fn namespaces_allow_type_module_coexistence_but_reject_duplicates() {
    let source = "type M = | MkM module type S = sig def id(x: Int): Int end module M : S = struct def id x => x end def main = M.id 1";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    let source = "module type S = sig def id(x: Int): Int end module M : S = struct def id x => x end module M : S = struct def id x => x end def main = M.id 1";
    assert!(check_source(source, "t").is_err());
}
