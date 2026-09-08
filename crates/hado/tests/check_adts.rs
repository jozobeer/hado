use hado::{check_source, types::Ty};

#[test]
fn generic_constructors_infer_payloads() {
    let source = "type Choice<T> = | Empty | Present(T)\ndef main = Present(7)";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt("Choice".into(), vec![Ty::Int])
    );
}

#[test]
fn constructor_parameters_share_constraints() {
    let source = "type Pair<T> = | Pair(T, T)\ndef main = Pair(1, true)";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn prelude_types_are_available_without_definitions() {
    assert_eq!(
        check_source("def main = Some(3)", "t").unwrap().main,
        Ty::Adt("Option".into(), vec![Ty::Int])
    );
    assert_eq!(
        check_source("def main = [true]", "t").unwrap().main,
        Ty::Adt("List".into(), vec![Ty::Bool])
    );
    assert!(check_source("type Option = | Other\ndef main = 1", "t").is_err());
}

#[test]
fn lists_and_constructor_patterns_share_element_types() {
    assert_eq!(
        check_source("def main = [1, 2, 3]", "t").unwrap().main,
        Ty::Adt("List".into(), vec![Ty::Int])
    );
    assert!(check_source("def main = [1, true]", "t").is_err());
    let source =
        "def unwrap | Some(value) => value | None => 0\ndef main = unwrap (Some(8))";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    let source = "def main = if Some(1) | Some(value) => value | None => false";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn pattern_bindings_do_not_escape_their_arm() {
    let source = "def main = if Some(1) | Some(value) => value | None => value";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E204")
    );
}

#[test]
fn recursive_type_cycles_preserve_parameters() {
    for source in [
        "type Tree<T> = | Leaf(T) | Branch(Tree<List<T>>)\ndef main = 1",
        "type A<T> = | ANode(B<List<T>>)\ntype B<U> = | BNode(A<U>)\ndef main = 1",
        "type Pair<A, B> = | End | Next(Pair<B, A>)\ndef main = 1",
    ] {
        assert_eq!(
            check_source(source, "t").err().unwrap().code.as_deref(),
            Some("E302"),
            "{source}"
        );
    }
    let source = "type A<T> = | AEnd | ANode(B<T>)\ntype B<U> = | BNode(A<U>)\ndef main = if ANode(BNode(AEnd)) == ANode(BNode(AEnd)) => 1 | 0";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    let source = "type Wrap<T> = | Wrapped(List<List<T>>)\ndef main = if Wrapped([[1]]) == Wrapped([[1]]) => 1 | 0";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn only_ok_has_the_empty_unit_payload_form() {
    let source = "def main = if true => Ok() | Error(false)";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt("Result".into(), vec![Ty::Unit, Ty::Bool])
    );
    let invalid = "type Event = | Done(Unit) | Failed\ndef main = Done()";
    assert_eq!(check_source(invalid, "t").err().unwrap().code.as_deref(), Some("E106"));
    let source = "type Event = | Done(Unit) | Failed\ndef main = Done(())";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt("Event".into(), vec![])
    );
    let source = "type Event = | Count(Int) | Failed\ndef main = Count()";
    assert!(check_source(source, "t").is_err());
}

#[test]
fn nullary_constructor_patterns_constrain_the_scrutinee_type() {
    let source = "type Color = | Red | Blue\ndef f | Red => 1 | 0\ndef main = f (Some(1))";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
    let source = "def f | None => 1 | 0\ndef main = f (Some(1))";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn main_requires_a_monomorphic_type() {
    for source in ["def main = []", "def main = Some([])"] {
        assert_eq!(check_source(source, "t").err().unwrap().code.as_deref(), Some("E701"));
    }
    let source = "def empty (n: Int): List<Int> => if n == 0 => [] | [n]\ndef main = empty 0";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Adt("List".into(), vec![Ty::Int]));
}
