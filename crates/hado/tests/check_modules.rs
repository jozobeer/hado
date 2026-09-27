use hado::{check_source, types::Ty};

#[test]
fn signature_matching_exports_only_declared_values() {
    let prefix =
        "module S\ndef inc (x: Int): Int\n/module\nimplement S as M\ndef hidden = 2\ndef inc x => x + hidden\n/implement";
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
fn abstract_types_hide_record_representation() {
    let prefix = "module S\ntype T\ndef make (x: Int): T\n/module\nimplement S as M\ntype Representation = { value: Int }\ntype T = Representation\ndef make x => { value = x }\n/implement";
    assert_eq!(
        check_source(&format!("def main = M.make 1\n{prefix}"), "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E705")
    );
    let source = format!("def main = M.make 1 as item ->\n  item.value\n{prefix}");
    assert_eq!(
        check_source(&source, "t").err().unwrap().code.as_deref(),
        Some("E601")
    );
    assert!(check_source(&format!("def main = {{ value = 1 }}\n{prefix}"), "t").is_err());
}

#[test]
fn recursive_type_restrictions_apply_inside_modules() {
    let source = "def main = M.value 1\nmodule S\ndef value (x: Int): Int\n/module\nimplement S as M\ntype T Nest = | Leaf(T) | Next(T List Nest)\ndef value x => x\n/implement";
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
        let source = format!(
            "def main = M.value 1\nmodule S\ndef value (x: Int): Int\n/module\nimplement S as M\n{body}\n/implement"
        );
        assert!(check_source(&source, "t").is_err(), "{source}");
    }
}

#[test]
fn qualified_abstract_annotations_preserve_module_identity() {
    let prefix = "module S\ntype T\ndef make (x: Int): T\n/module\nimplement S as M\ntype Representation = { x: Int }\ntype T = Representation\ndef make x => { x = x }\n/implement\nimplement S as N\ntype Representation = { x: Int }\ntype T = Representation\ndef make x => { x = x }\n/implement";
    let source = format!("def accept (x: M.T) => x == M.make 1 def main = accept (M.make 1) && N.make 1 == N.make 1\n{prefix}");
    assert_eq!(check_source(&source, "t").unwrap().main, Ty::Bool);
    let source =
        format!("def accept (x: M.T) => x == M.make 1 def main = accept (N.make 1)\n{prefix}");
    assert_eq!(
        check_source(&source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn nested_modules_are_rejected_before_name_resolution() {
    // `implement` を式の中に置いた入れ子は、名前解決より前に E100 で落ちる。
    let source = "def main = (implement S as Inner\ndef run x => x + 1\n/implement)\nmodule S\ndef run (x: Int): Int\n/module";
    assert_eq!(
        check_source(source, "nested.hd")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E100")
    );
}

#[test]
fn signature_specialization_preserves_comparison_constraints() {
    let source = "def inc x => x + 1\ndef main = M.equal inc\nmodule S\ndef equal (f: Fn(Int => Int)): Bool\n/module\nimplement S as M\ndef equal f => f == f\n/implement";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E703")
    );
}

#[test]
fn checked_modules_retain_separate_internal_record_schemas() {
    let source = "def main = A.make 1 == A.make 1 && B.make 1 == B.make 1\nmodule S\ntype T\ndef make (x: Int): T\n/module\nimplement S as A\ntype Representation = { first: Int }\ntype T = Representation\ndef make x => { first = x }\n/implement\nimplement S as B\ntype Representation = { second: Int }\ntype T = Representation\ndef make x => { second = x }\n/implement";
    let checked = check_source(source, "t").unwrap();
    assert_eq!(checked.module_records["A.Representation"].0, vec!["first"]);
    assert_eq!(checked.module_records["B.Representation"].0, vec!["second"]);
    assert!(!checked.records.contains_key("T"));

    let source = "def main = A.make 1 == A.make 1 && B.make 1 == B.make 1\nmodule S\ntype T\ndef make (x: Int): T\n/module\nimplement S as A\ntype Representation = | Empty | Wrap(Int)\ntype T = Representation\ndef make x => Wrap(x)\n/implement\nimplement S as B\ntype Representation = | Wrap(Int) | Empty\ntype T = Representation\ndef make x => Wrap(x)\n/implement";
    let checked = check_source(source, "t").unwrap();
    assert_eq!(checked.module_constructors["A.Wrap"].0, 1);
    assert_eq!(checked.module_constructors["B.Wrap"].0, 0);
    assert_eq!(checked.module_constructors["A.Empty"].0, 0);
    assert!(!checked.constructors.contains_key("Wrap"));

    let source = "def main = A.get (A.make 1)\nmodule S\ntype T\ndef make (x: Int): T\ndef get (value: T): Int\n/module\nimplement S as A\ntype Representation = { value: Int }\ntype T = Representation\ndef make x => { value = x }\ndef get item => item.value\n/implement";
    let checked = check_source(source, "t").unwrap();
    let mut inference = hado::types::Inference::default();
    let ty = inference.instantiate(&checked.module_records["A.Representation"].1);
    assert_eq!(
        ty,
        Ty::fn_of(vec![Ty::Int], Ty::Adt("A.Representation".into(), vec![]))
    );
    let make = inference.instantiate(&checked.module_definitions["A.make"]);
    let Ty::Fn(args, result, _) = make else {
        panic!("make is a function");
    };
    assert_eq!(args, vec![Ty::Int]);
    assert_eq!(*result, Ty::Adt("A.Representation".into(), vec![]));
    assert!(checked
        .composite_types
        .values()
        .any(|ty| ty == &Ty::Adt("A.Representation".into(), vec![])));
}

#[test]
fn abstract_annotations_match_exported_function_types_across_modules() {
    let source = "def main = Outer.run 42\nmodule S\ntype T\ndef make (x: Int): T\ndef get (value: T): Int\n/module\nmodule API\ndef run (x: Int): Int\n/module\nimplement S as Inner\ntype Representation = { value: Int }\ntype T = Representation\ndef make x => { value = x }\ndef get item => item.value\n/implement\nimplement API as Outer\ndef consume (item: Inner.T) => Inner.get item\ndef run x => consume (Inner.make x)\n/implement";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn generic_signatures_require_independent_type_parameters() {
    for source in [
        "def main = 0\nmodule S\ndef A id (x: A): A\n/module\nimplement S as M\ndef id x => x + 1\n/implement",
        "def main = 0\nmodule S\ndef (A, B) convert (x: A): B\n/module\nimplement S as M\ndef convert x => x\n/implement",
    ] {
        assert!(check_source(source, "t").is_err(), "{source}");
    }
    let source = "def main = if M.id true => M.id 42 | 0\nmodule S\ndef A id (x: A): A\n/module\nimplement S as M\ndef id x => x\n/implement";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}
