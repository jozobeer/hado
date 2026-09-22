use hado::{check, parser, types::Ty};

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
    assert_eq!(
        function.to_string(),
        "Fn(Store.T, (Int, String) Result List => Bool)"
    );
}
