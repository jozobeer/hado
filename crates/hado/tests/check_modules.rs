use hado::{check_source, types::Ty};

#[test]
fn signature_matching_exports_only_declared_values() {
    let prefix = "module S def inc(x: Int): Int\nimplement S as M def hidden = 2 def inc x => x + hidden";
    assert_eq!(
        check_source(&format!("def main = M.inc 1\n{prefix}"), "t")
            .unwrap()
            .main,
        Ty::Int
    );
    assert_eq!(
        check_source(&format!("def main = M.hidden\n{prefix}"), "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E204")
    );
}

#[test]
fn signature_matching_rejects_missing_and_mismatched_values() {
    for body in ["def other = 1", "def inc x => true"] {
        let source = format!("def main = 1\nmodule S def inc(x: Int): Int\nimplement S as M {body}");
        assert!(check_source(&source, "t").is_err());
    }
}

#[test]
fn abstract_types_hide_record_representation() {
    let prefix = "module S type T def make(x: Int): T\nimplement S as M type Representation = { value: Int } type T = Representation def make x => { value = x }";
    assert!(check_source(&format!("def main = M.make 1\n{prefix}"), "t").is_ok());
    let source = format!("def main = let item = M.make 1 -> item.value\n{prefix}");
    assert_eq!(
        check_source(&source, "t").err().unwrap().code.as_deref(),
        Some("E601")
    );
    assert!(check_source(&format!("def main = {{ value = 1 }}\n{prefix}"), "t").is_err());
}

#[test]
fn recursive_type_restrictions_apply_inside_modules() {
    let source = "def main = M.value 1\nmodule S def value(x: Int): Int\nimplement S as M type T Nest = | Leaf(T) | Next(T List Nest) def value x => x";
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
        let source = format!("def main = M.value 1\nmodule S def value(x: Int): Int\nimplement S as M {body}");
        assert!(check_source(&source, "t").is_err(), "{source}");
    }
}

#[test]
fn qualified_abstract_annotations_preserve_module_identity() {
    let prefix = "module S type T def make(x: Int): T\nimplement S as M type Representation = { x: Int } type T = Representation def make x => { x = x }\nimplement S as N type Representation = { x: Int } type T = Representation def make x => { x = x }";
    let source = format!("def accept (x: M.T) => x == M.make 1 def main = accept (M.make 1) && N.make 1 == N.make 1\n{prefix}");
    assert_eq!(check_source(&source, "t").unwrap().main, Ty::Bool);
    let source = format!("def accept (x: M.T) => x == M.make 1 def main = accept (N.make 1)\n{prefix}");
    assert_eq!(
        check_source(&source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn nested_modules_are_rejected_before_name_resolution() {
    let source = "def main = (implement S as Inner def run x => x + 1)\nmodule S def run(x: Int): Int";
    assert_eq!(check_source(source, "nested.hd").err().unwrap().code.as_deref(), Some("E100"));
}

#[test]
fn unused_private_module_definition_is_rejected() {
    let source = "def main = M.used 1\nmodule S def used(x: Int): Int\nimplement S as M def used x => x def dead x => x";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E514")
    );
}

#[test]
fn signature_specialization_preserves_comparison_constraints() {
    let source = "def inc x => x + 1\ndef main = M.equal inc\nmodule S def equal(f: Fn(Int => Int)): Bool\nimplement S as M def equal f => f == f";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E703")
    );
}

#[test]
fn checked_modules_retain_separate_internal_record_schemas() {
    let source = "def main = A.make 1 == A.make 1 && B.make 1 == B.make 1\nmodule S type T def make(x: Int): T\nimplement S as A type Representation = { first: Int } type T = Representation def make x => { first = x }\nimplement S as B type Representation = { second: Int } type T = Representation def make x => { second = x }";
    let checked = check_source(source, "t").unwrap();
    assert_eq!(checked.module_records["A.Representation"].0, vec!["first"]);
    assert_eq!(checked.module_records["B.Representation"].0, vec!["second"]);
    assert!(!checked.records.contains_key("T"));
}

#[test]
fn checked_modules_retain_internal_constructor_tags_and_schemes() {
    let source = "def main = A.make 1 == A.make 1 && B.make 1 == B.make 1\nmodule S type T def make(x: Int): T\nimplement S as A type Representation = | Empty | Wrap(Int) type T = Representation def make x => Wrap(x)\nimplement S as B type Representation = | Wrap(Int) | Empty type T = Representation def make x => Wrap(x)";
    let checked = check_source(source, "t").unwrap();
    assert_eq!(checked.module_constructors["A.Wrap"].0, 1);
    assert_eq!(checked.module_constructors["B.Wrap"].0, 0);
    assert_eq!(checked.module_constructors["A.Empty"].0, 0);
    assert!(!checked.constructors.contains_key("Wrap"));
}

#[test]
fn internal_record_schemas_use_qualified_result_types() {
    let source = "def main = A.make 1\nmodule S type T def make(x: Int): T\nimplement S as A type Representation = { value: Int } type T = Representation def make x => { value = x }";
    let checked = check_source(source, "t").unwrap();
    let mut inference = hado::types::Inference::default();
    let ty = inference.instantiate(&checked.module_records["A.Representation"].1);
    assert_eq!(ty, Ty::Fn(vec![Ty::Int], Box::new(Ty::Adt("A.Representation".into(), vec![]))));
    let make = inference.instantiate(&checked.module_definitions["A.make"]);
    assert_eq!(make, ty);
    assert!(checked.composite_types.values().any(|ty| ty == &Ty::Adt("A.Representation".into(), vec![])));
}

#[test]
fn abstract_annotations_match_exported_function_types_across_modules() {
    let source = "def main = Outer.run 42\nmodule S type T def make(x: Int): T def get(value: T): Int\nmodule API def run(x: Int): Int\nimplement S as Inner type Representation = { value: Int } type T = Representation def make x => { value = x } def get item => item.value\nimplement API as Outer\ndef consume (item: Inner.T) => Inner.get item\ndef run x => consume (Inner.make x)";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn generic_signatures_require_independent_type_parameters() {
    for source in [
        "def main = 0\nmodule S def A id(x: A): A\nimplement S as M def id x => x + 1",
        "def main = 0\nmodule S def (A, B) convert(x: A): B\nimplement S as M def convert x => x",
    ] {
        assert!(check_source(source, "t").is_err(), "{source}");
    }
    let source = "def main = if M.id true => M.id 42 | 0\nmodule S def A id(x: A): A\nimplement S as M def id x => x";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}
