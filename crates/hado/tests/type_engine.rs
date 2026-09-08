use hado::types::{Inference, Ty, TypeError};

#[test]
fn unify_substitutes_nested_types() {
    let mut inference = Inference::default();
    let variable = inference.fresh();
    let generic = Ty::Fn(
        vec![variable.clone()],
        Box::new(Ty::Adt("Option".into(), vec![variable])),
    );
    let concrete = Ty::Fn(
        vec![Ty::Int],
        Box::new(Ty::Adt("Option".into(), vec![Ty::Int])),
    );
    inference.unify(&generic, &concrete).unwrap();
    assert_eq!(inference.resolve(&generic), concrete);
    assert_eq!(
        inference.unify(&Ty::Int, &Ty::Bool),
        Err(TypeError::Mismatch)
    );
    assert_eq!(
        inference.unify(
            &Ty::Fn(vec![], Box::new(Ty::Int)),
            &Ty::Fn(vec![Ty::Int], Box::new(Ty::Int))
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
    let function = Ty::Fn(vec![parameter.clone()], Box::new(captured.clone()));
    let scheme = inference.generalize(&function, &[captured.clone()]);
    let first = inference.instantiate(&scheme);
    let second = inference.instantiate(&scheme);
    inference
        .unify(&first, &Ty::Fn(vec![Ty::Int], Box::new(Ty::Bool)))
        .unwrap();
    inference
        .unify(&second, &Ty::Fn(vec![Ty::String], Box::new(Ty::Bool)))
        .unwrap();
    assert_eq!(inference.resolve(&captured), Ty::Bool);
    assert_eq!(inference.resolve(&parameter), parameter);
    assert_eq!(
        inference.unify(&second, &Ty::Fn(vec![Ty::String], Box::new(Ty::Int))),
        Err(TypeError::Mismatch)
    );
}

#[test]
fn constrained_scheme_instantiates_predicate_with_function_type() {
    let mut inference = hado::types::Inference::default();
    let variable = inference.fresh();
    let function = hado::types::Ty::Fn(vec![variable.clone()], Box::new(hado::types::Ty::Bool));
    let origin = Some(hado::lexer::Span { line: 7, col: 13 });
    let scheme = inference.generalize_constrained(&function, &[], &[hado::types::ComparisonConstraint { ty: variable, code: "E704", span: origin }]);
    let (first, constraints) = inference.instantiate_constrained(&scheme);
    let hado::types::Ty::Fn(args, _) = first else {
        panic!("expected function")
    };
    assert_eq!(constraints, vec![hado::types::ComparisonConstraint { ty: args[0].clone(), code: "E704", span: origin }]);
    let (second, _) = inference.instantiate_constrained(&scheme);
    assert_ne!(
        second,
        hado::types::Ty::Fn(args, Box::new(hado::types::Ty::Bool))
    );
}

#[test]
fn failed_unification_does_not_leave_partial_substitutions() {
    use hado::types::{Inference, Ty, TypeError};
    let mut inference = Inference::default();
    let variable = inference.fresh();
    let left = Ty::Fn(vec![variable.clone(), Ty::Bool], Box::new(Ty::Unit));
    let right = Ty::Fn(vec![Ty::Int, Ty::String], Box::new(Ty::Unit));
    assert_eq!(inference.unify(&left, &right), Err(TypeError::Mismatch));
    assert_eq!(inference.resolve(&variable), variable);
    inference.unify(&variable, &Ty::Float).unwrap();
    assert_eq!(inference.resolve(&variable), Ty::Float);
}
