use hado::{check, parser, types::Ty};

#[test]
fn boolean_forms_are_redundant_without_loading_the_prelude() {
    for source in [
        "def f (b: Bool) => if b | true => 1 | false => 0",
        "def f (b: Bool) => if b | true => 1 | 0",
        "def main = if | true => 1 | 0",
        "def f | true => 1 | false => 0",
    ] {
        let ast = parser::parse_source(source, "boolean.hd").unwrap();
        let error = check::check_ast(&ast, "boolean.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E520"), "{source}: {error}");
    }
}

#[test]
fn direct_conditions_remain_canonical() {
    let ast = parser::parse_source("def f x | x > 0 => x | 0", "direct.hd").unwrap();
    check::check_ast(&ast, "direct.hd").unwrap();
}

#[test]
fn displayed_types_use_postfix_arguments_and_function_parentheses() {
    let abstract_type = Ty::Abstract("Store.T".into(), vec![]);
    let pair = Ty::Adt("Result".into(), vec![Ty::Int, Ty::String]);
    let nested = Ty::Adt("List".into(), vec![pair]);
    let function = Ty::fn_of(vec![abstract_type, nested], Ty::Bool);
    assert_eq!(function.to_string(), "Fn(Store.T, (Int, String) Result List => Bool)");
}
