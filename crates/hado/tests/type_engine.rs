use hado::types::{Inference, Ty, TypeError};

#[test]
fn unify_substitutes_nested_types() {
    let mut inference = Inference::default();
    let variable = inference.fresh();
    let generic = Ty::fn_of(
        vec![variable.clone()],
        Ty::Adt("Option".into(), vec![variable]),
    );
    let concrete = Ty::fn_of(
        vec![Ty::Int],
        Ty::Adt("Option".into(), vec![Ty::Int]),
    );
    inference.unify(&generic, &concrete).unwrap();
    assert_eq!(inference.resolve(&generic), concrete);
    assert_eq!(
        inference.unify(&Ty::Int, &Ty::Bool),
        Err(TypeError::Mismatch)
    );
    assert_eq!(
        inference.unify(
            &Ty::fn_of(vec![], Ty::Int),
            &Ty::fn_of(vec![Ty::Int], Ty::Int)
        ),
        Err(TypeError::Arity)
    );
}

#[test]
fn occurs_rejects_indirect_infinite_types() {
    let mut inference = Inference::default();
    let a = inference.fresh();
    let b = inference.fresh();
    inference.unify(&a, &b).unwrap();
    assert_eq!(
        inference.unify(&b, &Ty::Adt("List".into(), vec![a])),
        Err(TypeError::Occurs)
    );
}

#[test]
fn generalize_preserves_environment_variables_and_freshens_others() {
    let mut inference = Inference::default();
    let captured = inference.fresh();
    let parameter = inference.fresh();
    let function = Ty::fn_of(vec![parameter.clone()], captured.clone());
    let scheme = inference.generalize(&function, &[captured.clone()]);
    let first = inference.instantiate(&scheme);
    let second = inference.instantiate(&scheme);
    inference
        .unify(&first, &Ty::fn_of(vec![Ty::Int], Ty::Bool))
        .unwrap();
    inference
        .unify(&second, &Ty::fn_of(vec![Ty::String], Ty::Bool))
        .unwrap();
    assert_eq!(inference.resolve(&captured), Ty::Bool);
    assert_eq!(inference.resolve(&parameter), parameter);
    assert_eq!(
        inference.unify(&second, &Ty::fn_of(vec![Ty::String], Ty::Int)),
        Err(TypeError::Mismatch)
    );
}

#[test]
fn constrained_scheme_instantiates_predicate_with_function_type() {
    let mut inference = hado::types::Inference::default();
    let variable = inference.fresh();
    let function = hado::types::Ty::fn_of(vec![variable.clone()], hado::types::Ty::Bool);
    let origin = Some(hado::lexer::Span { line: 7, col: 13 });
    let scheme = inference.generalize_constrained(&function, &[], &[hado::types::ComparisonConstraint { ty: variable, code: "E704", span: origin }]);
    let (first, constraints) = inference.instantiate_constrained(&scheme);
    let hado::types::Ty::Fn(args, _, _) = first else {
        panic!("expected function")
    };
    assert_eq!(constraints, vec![hado::types::ComparisonConstraint { ty: args[0].clone(), code: "E704", span: origin }]);
    let (second, _) = inference.instantiate_constrained(&scheme);
    assert_ne!(
        second,
        hado::types::Ty::fn_of(args, hado::types::Ty::Bool)
    );
}

#[test]
fn failed_unification_does_not_leave_partial_substitutions() {
    use hado::types::{Inference, Ty, TypeError};
    let mut inference = Inference::default();
    let variable = inference.fresh();
    let left = Ty::fn_of(vec![variable.clone(), Ty::Bool], Ty::Unit);
    let right = Ty::fn_of(vec![Ty::Int, Ty::String], Ty::Unit);
    assert_eq!(inference.unify(&left, &right), Err(TypeError::Mismatch));
    assert_eq!(inference.resolve(&variable), variable);
    inference.unify(&variable, &Ty::Float).unwrap();
    assert_eq!(inference.resolve(&variable), Ty::Float);
}
