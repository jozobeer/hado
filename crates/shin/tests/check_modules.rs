use shin::{check_source, types::Ty};

#[test]
fn signature_matching_exports_only_declared_values() {
    let prefix = "module type S = sig def inc(x: Int): Int end\nmodule M : S = struct def hidden = 2 def inc x => x + hidden end\n";
    assert_eq!(
        check_source(&format!("{prefix}def main = M.inc 1"), "t")
            .unwrap()
            .main,
        Ty::Int
    );
    assert_eq!(
        check_source(&format!("{prefix}def main = M.hidden"), "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E205")
    );
}

#[test]
fn signature_matching_rejects_missing_and_mismatched_values() {
    for body in ["def other = 1", "def inc x => true"] {
        let source = format!("module type S = sig def inc(x: Int): Int end\nmodule M : S = struct {body} end\ndef main = 1");
        assert!(check_source(&source, "t").is_err());
    }
}

#[test]
fn abstract_types_hide_record_representation() {
    let prefix = "module type S = sig type T def make(x: Int): T end\nmodule M : S = struct type T = { value: Int } def make x => { value = x } end\n";
    assert!(check_source(&format!("{prefix}def main = M.make 1"), "t").is_ok());
    let source = format!("{prefix}def main = let item = M.make 1 -> item.value");
    assert_eq!(
        check_source(&source, "t").err().unwrap().code.as_deref(),
        Some("E602")
    );
    assert!(check_source(&format!("{prefix}def main = {{ value = 1 }}"), "t").is_err());
}

#[test]
fn recursive_type_restrictions_apply_inside_modules() {
    let source = "module type S = sig def value(x: Int): Int end\nmodule M : S = struct type Nest<T> = Leaf(T) | Next(Nest<List<T>>) def value x => x end\ndef main = M.value 1";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E302")
    );
}

#[test]
fn duplicate_module_members_are_not_silently_overwritten() {
    for body in [
        "def value x => x def value x => x + 1",
        "type T = { x: Int } type T = { y: Int } def value x => x",
    ] {
        let source = format!("module type S = sig def value(x: Int): Int end module M : S = struct {body} end def main = M.value 1");
        assert!(check_source(&source, "t").is_err(), "{source}");
    }
}

#[test]
fn qualified_abstract_annotations_preserve_module_identity() {
    let prefix = "module type S = sig type T def make(x: Int): T end module M : S = struct type T = { x: Int } def make x => { x = x } end module N : S = struct type T = { x: Int } def make x => { x = x } end";
    let source = format!("{prefix} def accept (x: M.T) => true def main = accept (M.make 1)");
    assert_eq!(check_source(&source, "t").unwrap().main, Ty::Bool);
    let source = format!("{prefix} def accept (x: M.T) => true def main = accept (N.make 1)");
    assert_eq!(
        check_source(&source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn nested_modules_are_available_only_in_their_scope() {
    let prefix = "module type S = sig def run(x: Int): Int end module Outer : S = struct module Inner : S = struct def run x => x + 1 end def run x => Inner.run x end";
    assert_eq!(
        check_source(&format!("{prefix} def main = Outer.run 1"), "t")
            .unwrap()
            .main,
        Ty::Int
    );
    assert_eq!(
        check_source(&format!("{prefix} def main = Inner.run 1"), "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E205")
    );
}

#[test]
fn unused_private_module_definition_is_rejected() {
    let source = "module type S = sig def used(x: Int): Int end\nmodule M : S = struct def used x => x def dead x => x end\ndef main = M.used 1";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E515")
    );
}

#[test]
fn signature_specialization_preserves_comparison_constraints() {
    let source = "module type S = sig def equal(f: Fn<Int, Int>): Bool end\nmodule M : S = struct def equal f => f == f end\ndef inc x => x + 1\ndef main = M.equal inc";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E704")
    );
}

#[test]
fn checked_modules_retain_separate_internal_record_schemas() {
    let source = "module type S = sig type T def make(x: Int): T end\nmodule A : S = struct type T = { first: Int } def make x => { first = x } end\nmodule B : S = struct type T = { second: Int } def make x => { second = x } end\ndef main = 0";
    let checked = check_source(source, "t").unwrap();
    assert_eq!(checked.module_records["A.T"].0, vec!["first"]);
    assert_eq!(checked.module_records["B.T"].0, vec!["second"]);
    assert!(!checked.records.contains_key("T"));
}

#[test]
fn checked_modules_retain_internal_constructor_tags_and_schemes() {
    let source = "module type S = sig type T def make(x: Int): T end\nmodule A : S = struct type T = | Empty | Wrap(Int) def make x => Wrap x end\nmodule B : S = struct type T = | Wrap(Int) | Empty def make x => Wrap x end\ndef main = 0";
    let checked = check_source(source, "t").unwrap();
    assert_eq!(checked.module_constructors["A.Wrap"].0, 1);
    assert_eq!(checked.module_constructors["B.Wrap"].0, 0);
    assert_eq!(checked.module_constructors["A.Empty"].0, 0);
    assert!(!checked.constructors.contains_key("Wrap"));
}

#[test]
fn internal_record_schemas_use_qualified_result_types() {
    let source = "module type S = sig type T def make(x: Int): T end\nmodule A : S = struct type T = { value: Int } def make x => { value = x } end\ndef main = 0";
    let checked = check_source(source, "t").unwrap();
    let mut inference = shin::types::Inference::default();
    let ty = inference.instantiate(&checked.module_records["A.T"].1);
    assert_eq!(ty, Ty::Fn(vec![Ty::Int], Box::new(Ty::Adt("A.T".into(), vec![]))));
    let make = inference.instantiate(&checked.module_definitions["A.make"]);
    assert_eq!(make, ty);
    assert!(checked.composite_types.values().any(|ty| ty == &Ty::Adt("A.T".into(), vec![])));
}

#[test]
fn nested_abstract_annotations_match_exported_function_types() {
    let source = "module type S = sig type T def make(x: Int): T def get(value: T): Int end\nmodule type API = sig def run(x: Int): Int end\nmodule Outer : API = struct\nmodule Inner : S = struct type T = { value: Int } def make x => { value = x } def get item => item.value end\ndef consume (item: Inner.T) => Inner.get item\ndef run x => consume (Inner.make x)\nend\ndef main = Outer.run 42";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn generic_signatures_require_independent_type_parameters() {
    for source in [
        "module type S = sig def id<A>(x: A): A end module M : S = struct def id x => x + 1 end def main = 0",
        "module type S = sig def convert<A, B>(x: A): B end module M : S = struct def convert x => x end def main = 0",
    ] {
        assert!(check_source(source, "t").is_err(), "{source}");
    }
    let source = "module type S = sig def id<A>(x: A): A end module M : S = struct def id x => x end def main = if M.id true => M.id 42 | 0";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}
