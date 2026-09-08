use hado::{mono::Type, types::Ty};

#[test]
fn mono_types_reject_variables_at_any_depth() {
    for ty in [
        Ty::Var(7),
        Ty::Adt("List".into(), vec![Ty::Var(7)]),
        Ty::Fn(vec![Ty::Int], Box::new(Ty::Var(7))),
    ] {
        assert!(Type::try_from(&ty).is_err());
    }
}

#[test]
fn mono_types_preserve_concrete_data_and_closure_signatures() {
    let ty = Ty::Fn(
        vec![Ty::Adt("List".into(), vec![Ty::Bool])],
        Box::new(Ty::Int),
    );
    assert_eq!(
        Type::try_from(&ty).unwrap(),
        Type::Closure(
            vec![Type::Data("List".into(), vec![Type::Bool])],
            Box::new(Type::Int)
        )
    );
}

#[test]
fn literal_lowering_preserves_values_and_types() {
    for (source, expected) in [
        ("42", Type::Int),
        ("1.5", Type::Float),
        ("true", Type::Bool),
        ("'x'", Type::Char),
        ("\"hello\"", Type::String),
        ("()", Type::Unit),
    ] {
        let ast = hado::parser::parse_source(&format!("def main = {source}"), "t").unwrap();
        let expression = hado::mono::lower_literal(ast.main().unwrap()).unwrap();
        assert_eq!(expression.ty(), expected);
    }
    let ast = hado::parser::parse_source("def main = 42", "t").unwrap();
    assert_eq!(
        hado::mono::lower_literal(ast.main().unwrap()).unwrap(),
        hado::mono::Constant::Int(42)
    );
}

#[test]
fn unary_lowering_preserves_operations() {
    for (source, expected) in [
        ("-42", Type::Int),
        ("-1.5", Type::Float),
        ("not false", Type::Bool),
    ] {
        let ast = hado::parser::parse_source(&format!("def main = {source}"), "t").unwrap();
        let expression = hado::mono::lower_expression(ast.main().unwrap()).unwrap();
        assert_eq!(expression.ty(), expected);
        assert!(matches!(expression, hado::mono::Expression::Unary { .. }));
    }
}

#[test]
fn arithmetic_lowering_keeps_nested_operations() {
    let ast = hado::parser::parse_source("def main = 1 + 2 * 3", "t").unwrap();
    let expression = hado::mono::lower_expression(ast.main().unwrap()).unwrap();
    assert_eq!(expression.ty(), Type::Int);
    let hado::mono::Expression::Binary { op, right, .. } = expression else {
        panic!("expected binary")
    };
    assert_eq!(op, hado::parser::BinOp::Add);
    assert!(matches!(
        *right,
        hado::mono::Expression::Binary {
            op: hado::parser::BinOp::Mul,
            ..
        }
    ));
    let ast = hado::parser::parse_source("def main = 1.5 - 0.5", "t").unwrap();
    assert_eq!(
        hado::mono::lower_expression(ast.main().unwrap())
            .unwrap()
            .ty(),
        Type::Float
    );
    let ast = hado::parser::parse_source("def main = 1 + true", "t").unwrap();
    assert!(hado::mono::lower_expression(ast.main().unwrap()).is_err());
}

#[test]
fn scalar_comparisons_lower_to_boolean_results() {
    for expression in [
        "1 < 2",
        "1.0 >= 2.0",
        "true == false",
        "'a' != 'b'",
        "\"a\" == \"b\"",
    ] {
        let ast = hado::parser::parse_source(&format!("def main = {expression}"), "t").unwrap();
        assert_eq!(
            hado::mono::lower_expression(ast.main().unwrap())
                .unwrap()
                .ty(),
            Type::Bool
        );
    }
    let ast = hado::parser::parse_source("def main = 1 == true", "t").unwrap();
    assert!(hado::mono::lower_expression(ast.main().unwrap()).is_err());
}

#[test]
fn short_circuit_operators_lower_to_branches() {
    use hado::mono::{Constant, Expression};
    for (source, conjunction) in [("false && true", true), ("true || false", false)] {
        let ast = hado::parser::parse_source(&format!("def main = {source}"), "t").unwrap();
        let expression = hado::mono::lower_expression(ast.main().unwrap()).unwrap();
        assert_eq!(expression.ty(), Type::Bool);
        let Expression::Branch {
            then, otherwise, ..
        } = expression
        else {
            panic!("expected branch")
        };
        if conjunction {
            assert_eq!(*otherwise, Expression::Constant(Constant::Bool(false)));
        } else {
            assert_eq!(*then, Expression::Constant(Constant::Bool(true)));
        }
    }
    let ast = hado::parser::parse_source("def main = if true => 1 | 2", "t").unwrap();
    assert_eq!(
        hado::mono::lower_expression(ast.main().unwrap())
            .unwrap()
            .ty(),
        Type::Int
    );
}

#[test]
fn condition_columns_preserve_source_order_and_fallback() {
    use hado::mono::{Constant, Expression};
    let ast =
        hado::parser::parse_source("def main = if | 1 > 2 => 10 | 3 > 4 => 20 | 30", "t").unwrap();
    let expression = hado::mono::lower_expression(ast.main().unwrap()).unwrap();
    let Expression::Branch {
        then, otherwise, ..
    } = expression
    else {
        panic!("first branch")
    };
    assert_eq!(*then, Expression::Constant(Constant::Int(10)));
    let Expression::Branch {
        then, otherwise, ..
    } = *otherwise
    else {
        panic!("second branch")
    };
    assert_eq!(*then, Expression::Constant(Constant::Int(20)));
    assert_eq!(*otherwise, Expression::Constant(Constant::Int(30)));
}

#[test]
fn let_lowering_distinguishes_shadowed_variables() {
    use hado::mono::Expression;
    let ast =
        hado::parser::parse_source("def main = let x = 1 -> let x = x + 2 -> x + 3", "t").unwrap();
    let expression = hado::mono::lower_expression(ast.main().unwrap()).unwrap();
    let Expression::Let {
        id: outer, body, ..
    } = expression
    else {
        panic!("outer let")
    };
    let Expression::Let {
        id: inner,
        value,
        body,
    } = *body
    else {
        panic!("inner let")
    };
    assert_ne!(outer, inner);
    let Expression::Binary { left, .. } = *value else {
        panic!("initializer")
    };
    assert!(matches!(*left, Expression::Variable { id, .. } if id == outer));
    let Expression::Binary { left, .. } = *body else {
        panic!("body")
    };
    assert!(matches!(*left, Expression::Variable { id, .. } if id == inner));
}

#[test]
fn division_lowering_preserves_integer_failure_type() {
    for source in ["4 / 2", "4 % 2"] {
        let ast = hado::parser::parse_source(&format!("def main = {source}"), "t").unwrap();
        assert_eq!(
            hado::mono::lower_expression(ast.main().unwrap())
                .unwrap()
                .ty(),
            Type::Data(
                "Result".into(),
                vec![Type::Int, Type::Data("DivisionError".into(), vec![])]
            )
        );
    }
    let ast = hado::parser::parse_source("def main = 4.0 / 2.0", "t").unwrap();
    assert_eq!(
        hado::mono::lower_expression(ast.main().unwrap())
            .unwrap()
            .ty(),
        Type::Float
    );
    let ast = hado::parser::parse_source("def main = 4.0 % 2.0", "t").unwrap();
    assert!(hado::mono::lower_expression(ast.main().unwrap()).is_err());
}

#[test]
fn top_level_values_lower_in_dependency_order() {
    let source = "def main = b + a\ndef b = a * 2\ndef a = 3";
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let checked = hado::check_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    assert_eq!(program.functions.len(), 2);
    assert!(program.functions.iter().all(|function| function.signature.parameters.is_empty()));
    assert!(matches!(program.entry, hado::mono::Expression::Binary { .. }));
}

#[test]
fn direct_calls_require_the_fixed_function_signature() {
    use hado::mono::{Constant, Expression, FunctionId, Signature};
    let signature = Signature {
        parameters: vec![Type::Int, Type::Bool],
        result: Type::Float,
    };
    let arguments = vec![
        Expression::Constant(Constant::Int(1)),
        Expression::Constant(Constant::Bool(true)),
    ];
    let call = Expression::call_direct(FunctionId(0), &signature, arguments).unwrap();
    assert_eq!(call.ty(), Type::Float);
    assert!(Expression::call_direct(FunctionId(0), &signature, vec![]).is_err());
    assert!(Expression::call_direct(
        FunctionId(0),
        &signature,
        vec![
            Expression::Constant(Constant::Bool(true)),
            Expression::Constant(Constant::Int(1))
        ]
    )
    .is_err());
}

#[test]
fn functions_are_specialized_by_argument_types() {
    let ast = hado::parser::parse_source(
        "def id x => x\ndef main = let a = id 1 -> (a > 0) && id true",
        "t",
    )
    .unwrap();
    let program =
        hado::mono::lower_program(&ast, &hado::infer::check_ast(&ast, "t").unwrap()).unwrap();
    assert_eq!(program.functions.len(), 2);
    assert_eq!(program.functions[0].signature.parameters, vec![Type::Int]);
    assert_eq!(program.functions[1].signature.parameters, vec![Type::Bool]);
    let ast = hado::parser::parse_source("def id x => x\ndef main = id 1 + id 2", "t").unwrap();
    assert_eq!(
        hado::mono::lower_program(&ast, &hado::infer::check_ast(&ast, "t").unwrap())
            .unwrap()
            .functions
            .len(),
        1
    );
}

#[test]
fn pipe_lowering_supplies_the_first_argument() {
    use hado::mono::{Constant, Expression};
    let ast = hado::parser::parse_source(
        "def choose a b => if b => a | 0\ndef main = 7 |> choose true",
        "t",
    )
    .unwrap();
    let program =
        hado::mono::lower_program(&ast, &hado::infer::check_ast(&ast, "t").unwrap()).unwrap();
    let Expression::TailCall { arguments, .. } = program.entry else {
        panic!("direct call")
    };
    assert_eq!(
        arguments,
        vec![
            Expression::Constant(Constant::Int(7)),
            Expression::Constant(Constant::Bool(true))
        ]
    );
    assert_eq!(
        program.functions[0].signature.parameters,
        vec![Type::Int, Type::Bool]
    );
}

#[test]
fn tail_calls_are_marked_only_in_result_positions() {
    use hado::mono::Expression;
    let ast = hado::parser::parse_source(
        "def id x => x\ndef f x => let a = id x -> if true => id a | id (a + 1)\ndef main = f 2",
        "t",
    )
    .unwrap();
    let program =
        hado::mono::lower_program(&ast, &hado::infer::check_ast(&ast, "t").unwrap()).unwrap();
    assert!(matches!(program.entry, Expression::TailCall { .. }));
    let function = program.functions.iter().find(|f| f.name == "f").unwrap();
    let Expression::Let { value, body, .. } = &function.body else {
        panic!("let")
    };
    assert!(matches!(**value, Expression::CallDirect { .. }));
    let Expression::Branch {
        then, otherwise, ..
    } = &**body
    else {
        panic!("branch")
    };
    assert!(matches!(**then, Expression::TailCall { .. }));
    assert!(matches!(**otherwise, Expression::TailCall { .. }));
}

#[test]
fn checked_program_exposes_polymorphic_definition_signatures() {
    let checked = hado::check_source("def id x => x\ndef main = id 1", "t").unwrap();
    let mut inference = hado::types::Inference::default();
    let signature = inference.instantiate(&checked.definitions["id"]);
    inference
        .unify(&signature, &Ty::Fn(vec![Ty::Bool], Box::new(Ty::Bool)))
        .unwrap();
    assert_eq!(
        inference.resolve(&signature),
        Ty::Fn(vec![Ty::Bool], Box::new(Ty::Bool))
    );
}

#[test]
fn specialization_resolves_return_type_before_body_lowering() {
    let checked = hado::check_source("def id x => x\ndef main = id 1", "t").unwrap();
    let signature =
        hado::mono::Signature::specialize(&checked.definitions["id"], &[Type::Bool]).unwrap();
    assert_eq!(signature.parameters, vec![Type::Bool]);
    assert_eq!(signature.result, Type::Bool);
    assert!(hado::mono::Signature::specialize(&checked.definitions["id"], &[]).is_err());
}

#[test]
fn recursive_functions_use_fixed_targets() {
    let source = "def rec down n => if n == 0 => true | down (n - 1)\ndef main = down 3";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("Function {").count(), 3);
    assert_eq!(
        output.matches("TailCall { function: FunctionId(0)").count(),
        2
    );
    let source = "def rec even n => if n == 0 => true | odd (n - 1)\ndef rec odd n => if n == 0 => false | even (n - 1)\ndef main = even 4";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("Function {").count(), 4);
    assert_eq!(
        output.matches("TailCall { function: FunctionId(0)").count(),
        2
    );
    assert_eq!(
        output.matches("TailCall { function: FunctionId(2)").count(),
        1
    );
}

#[test]
fn concrete_named_function_values_create_closures() {
    let output = hado::dump_mono("def inc x => x + 1\ndef main = let f = inc -> f 4", "t").unwrap();
    assert!(output.contains("MakeClosure { captures: [], function: FunctionId(0)"));
    assert!(output.contains("TailCallClosure"));
    assert_eq!(output.matches("Function {").count(), 2);
}

#[test]
fn checked_lambdas_retain_types_resolved_by_their_calls() {
    let source = "def main = fn(x => x) 4";
    let checked = hado::check_source(source, "t").unwrap();
    assert_eq!(
        checked.value_types[&(1, source.find("fn(").unwrap() + 1)],
        hado::types::Ty::Fn(vec![hado::types::Ty::Int], Box::new(hado::types::Ty::Int))
    );
}

#[test]
fn anonymous_closures_capture_outer_bindings() {
    let source = "def main = let offset = 7 -> let f = fn(x => x + offset) -> f 3";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    assert_eq!(program.functions.len(), 1);
    assert_eq!(program.functions[0].captures, vec![(0, Type::Int)]);
    assert_eq!(program.functions[0].signature.parameters, vec![Type::Int]);
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("captures: [Variable { id: 0, ty: Int }]"));
    assert!(output.contains("TailCallClosure"));
}

#[test]
fn nested_closures_forward_captures_and_keep_parameter_bindings() {
    use hado::mono::Expression;
    let source = "def main = let seed = 7 -> let make = fn(x => fn(y => seed + x + y)) -> let f = make 2 -> f 3";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let outer = program
        .functions
        .iter()
        .find(|f| f.id == hado::mono::FunctionId(0))
        .unwrap();
    let inner = program
        .functions
        .iter()
        .find(|f| f.id == hado::mono::FunctionId(1))
        .unwrap();
    assert_eq!(outer.captures, vec![(0, Type::Int)]);
    assert_eq!(
        inner.captures,
        vec![(0, Type::Int), (outer.parameters[0], Type::Int)]
    );
    let Expression::MakeClosure {
        function, captures, ..
    } = &outer.body
    else {
        panic!("nested closure")
    };
    assert_eq!(*function, inner.id);
    assert_eq!(
        *captures,
        vec![
            Expression::Variable {
                id: 0,
                ty: Type::Int
            },
            Expression::Variable {
                id: outer.parameters[0],
                ty: Type::Int
            }
        ]
    );
}

#[test]
fn lambda_parameter_does_not_capture_shadowed_outer_value() {
    let source = "def main = let x = 7 -> let f = fn(x => x + 1) -> x + f 3";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    assert!(program.functions[0].captures.is_empty());
    assert_ne!(program.functions[0].parameters[0], 0);
}

#[test]
fn lambdas_inside_polymorphic_functions_are_specialized_per_call() {
    let source = "def wrap x => let f = fn(y => y) -> f x\ndef main = (wrap 3 > 0) && wrap true";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("name: \"wrap\"").count(), 2);
    assert!(output.contains("signature: Signature { parameters: [Int], result: Int }"));
    assert!(output.contains("signature: Signature { parameters: [Bool], result: Bool }"));
    assert_eq!(output.matches("name: \"lambda_").count(), 2);
}

#[test]
fn polymorphic_named_values_use_the_reference_type() {
    let source = "def id x => x\ndef main = let f = id -> f 3";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("MakeClosure { captures: [], function: FunctionId(0), signature: Signature { parameters: [Int], result: Int } }"));
    assert!(output.contains("TailCallClosure"));
}

#[test]
fn constructor_values_lower_to_typed_construction() {
    let output = hado::dump_mono("def main = Some(3)", "t").unwrap();
    assert!(output.contains("Construct { constructor: \"Some\", fields: [Constant(Int(3))], result: Data(\"Option\", [Int]) }"));
}

#[test]
fn constructors_lower_as_data_and_reject_pipe_application() {
    let output = hado::dump_mono("type Flag = | On | Off\ndef main = On", "t").unwrap();
    assert!(output
        .contains("Construct { constructor: \"On\", fields: [], result: Data(\"Flag\", []) }"));
    let error = hado::dump_mono("def main = 3 |> Some", "t").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E106"));
}

#[test]
fn list_literals_lower_in_order_with_typed_nil() {
    use hado::mono::{Constant, Expression};
    let source = "def main = [3, 7]";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let ty = Type::Data("List".into(), vec![Type::Int]);
    let nil = Expression::Construct {
        constructor: "Nil".into(),
        fields: vec![],
        result: ty.clone(),
    };
    let tail = Expression::Construct {
        constructor: "Cons".into(),
        fields: vec![Expression::Constant(Constant::Int(7)), nil],
        result: ty.clone(),
    };
    assert_eq!(
        program.entry,
        Expression::Construct {
            constructor: "Cons".into(),
            fields: vec![Expression::Constant(Constant::Int(3)), tail],
            result: ty
        }
    );
    assert!(hado::dump_mono("def main = [[], [true]]", "t").is_ok());
}

#[test]
fn records_keep_field_names_and_nominal_type() {
    let source = "type User = { name: String; age: Int }\ndef main = { age = 3; name = \"a\" }";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("Record { fields: [(\"age\", Constant(Int(3))), (\"name\", Constant(String(\"a\")))], result: Data(\"User\", []) }"));
}

#[test]
fn record_projections_preserve_each_intermediate_type() {
    let source = "type Inner = { x: Int }\ntype Outer = { inner: Inner }\ndef main = let obj = { inner = { x = 3 } } -> obj.inner.x";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("field: \"inner\", result: Data(\"Inner\", [])"));
    assert!(output.contains("field: \"x\", result: Int"));
    let source = "type T Box = { value: T }\ndef main = let box = { value = true } -> box.value";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("field: \"value\", result: Bool"));
}

#[test]
fn record_let_patterns_evaluate_the_initializer_once() {
    let source = "type Pair = { left: Int; right: Int }\ndef pair n => { left = n; right = n + 1 }\ndef main = let { left; right = r } = pair 3 -> left + r";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("CallDirect {").count(), 1);
    assert_eq!(output.matches("RecordGet {").count(), 2);
    assert!(output.contains("field: \"left\", result: Int"));
    assert!(output.contains("field: \"right\", result: Int"));
}

#[test]
fn literal_pattern_branches_evaluate_scrutinee_once_and_keep_order() {
    let source = "def value x => x + 1\ndef main = if value 1 | 1 => 10 | 2 => 20 | 30";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("CallDirect {").count(), 1);
    let first = output.find("right: Constant(Int(1))").unwrap();
    let second = output.find("right: Constant(Int(2))").unwrap();
    assert!(first < second);
    assert!(output.contains("otherwise: Constant(Int(30))"));
}

#[test]
fn boolean_branches_keep_true_and_false_results() {
    use hado::mono::{Constant, Expression};
    for condition in [true, false] {
        let source = format!("def main = if {condition} => 10 | 20");
        let ast = hado::parser::parse_source(&source, "t").unwrap();
        let checked = hado::check_source(&source, "t").unwrap();
        let program = hado::mono::lower_program(&ast, &checked).unwrap();
        assert_eq!(
            program.entry,
            Expression::Branch {
                condition: Box::new(Expression::Constant(Constant::Bool(condition))),
                then: Box::new(Expression::Constant(Constant::Int(10))),
                otherwise: Box::new(Expression::Constant(Constant::Int(20))),
            }
        );
    }
}

#[test]
fn record_pattern_branches_bind_fields_in_the_arm() {
    let source = "type User = { age: Int }\ndef age | { age = n } => n + 1\ndef main = age { age = 3 }";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("RecordGet {"));
    assert!(output.contains("field: \"age\", result: Int"));
    assert!(output.contains("name: \"age\""));
}

#[test]
fn constructor_let_patterns_project_typed_payloads() {
    let source = "type T Box = | Wrap(T)\ndef main = let Wrap(x) = Wrap(3) -> x + 1";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("Project {"));
    assert!(output.contains("constructor: \"Wrap\", index: 0, result: Int"));
}

#[test]
fn nullary_constructor_patterns_lower_to_switch() {
    let source =
        "type Flag = | On | Off\ndef code | On => 1 | Off => 2\ndef main = code On";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("Switch {"));
    assert!(output.contains("arms: [(\"On\", Constant(Int(1))), (\"Off\", Constant(Int(2)))]"));
    assert!(output.contains("fallback: None, result: Int"));
}

#[test]
fn constructor_switch_binds_payload_only_inside_matching_arm() {
    let source = "def extract | Some(x) => x + 1 | None => 0\ndef main = extract (Some(3))";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("Switch {"));
    assert!(output.contains("arms: [(\"Some\", Let {"));
    assert!(output.contains("constructor: \"Some\", index: 0, result: Int"));
    assert!(output.contains("(\"None\", Constant(Int(0)))"));
}

#[test]
fn nested_nullary_patterns_check_both_tags() {
    let source =
        "def classify | Some(None) => 1 | 0\ndef main = classify (Some(Some(3)))";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("Switch {").count(), 2);
    assert!(output.contains("arms: [(\"None\", Constant(Int(1)))]"));
    assert_eq!(
        output.matches("fallback: Some(Constant(Int(0)))").count(),
        2
    );
}

#[test]
fn repeated_constructor_arms_fall_through_in_source_order() {
    let source = "def classify | Some(None) => 1 | Some(_) => 2 | 0\ndef main = classify (Some(Some(3)))";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("arms: [(\"Some\",").count(), 1);
    assert!(output.contains("arms: [(\"None\", Constant(Int(1)))]"));
    assert!(output.contains("body: Constant(Int(2))"));
    assert!(output.contains("fallback: Some(Constant(Int(0)))"));
}

#[test]
fn constructor_payload_literals_test_before_falling_through() {
    let source = "def classify | Some(1) => 10 | Some(_) => 20 | 0\ndef main = classify (Some(2))";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("condition: Binary { op: Eq, left: Project {"));
    assert!(output.contains("right: Constant(Int(1))"));
    assert!(output.contains("then: Constant(Int(10))"));
    assert!(output.contains("body: Constant(Int(20))"));
}

#[test]
fn deep_constructor_patterns_guard_payload_bindings() {
    let source = "def extract | Some(Some(Some(x))) => x + 1 | 0\ndef main = extract (Some(Some(Some(3))))";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("Switch {").count(), 3);
    assert_eq!(
        output.matches("fallback: Some(Constant(Int(0)))").count(),
        3
    );
    assert!(output.contains("result: Int }, body: Binary { op: Add"));
}

#[test]
fn exhaustive_nested_constructor_patterns_need_no_source_fallback() {
    let source = "def extract | Some(None) => 0 | Some(Some(x)) => x | None => -1\ndef main = extract (Some(Some(3)))";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("Switch {").count(), 3);
    assert!(output.contains("constructor: \"Some\", index: 0, result: Int"));
    assert!(output.contains("fallback: None, result: Int"));
}

#[test]
fn exhaustive_boolean_payload_patterns_lower_without_fallback() {
    let source = "def classify | Some(true) => 1 | Some(false) => 2 | None => 0\ndef main = classify (Some(false))";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("right: Constant(Bool(true))"));
    assert!(output.contains("right: Constant(Bool(false))"));
    assert!(output.contains("otherwise: Unreachable { result: Int }"));
}

#[test]
fn record_pattern_columns_match_fields_in_source_order() {
    let source = "type Flag = { enabled: Bool }\ndef classify | { enabled = true } => 1 | { enabled = false } => 2\ndef main = classify { enabled = false }";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("condition: Binary { op: Eq, left: RecordGet {"));
    assert!(output.contains("right: Constant(Bool(true))"));
    assert!(output.contains("right: Constant(Bool(false))"));
    assert!(
        output.find("then: Constant(Int(1))").unwrap()
            < output.find("then: Constant(Int(2))").unwrap()
    );
}

#[test]
fn option_propagation_branches_before_the_continuation() {
    let source = "def main = let x? = Some(1) -> Some(x + 2)";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("arms: [(\"Some\", Let {"));
    assert!(output.contains("(\"None\", Construct { constructor: \"None\", fields: [], result: Data(\"Option\", [Int]) })"));
    assert!(output.contains("constructor: \"Some\", index: 0, result: Int"));
}

#[test]
fn result_propagation_preserves_error_when_success_type_changes() {
    use hado::mono::Expression;
    let source = "def main = do (1 / 2) -> Ok(true)";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let Expression::Let { id, body, .. } = program.entry else {
        panic!("single evaluation")
    };
    let Expression::Switch { arms, result, .. } = *body else {
        panic!("result switch")
    };
    let error_type = Type::Data("DivisionError".into(), vec![]);
    assert_eq!(
        result,
        Type::Data("Result".into(), vec![Type::Bool, error_type.clone()])
    );
    assert_eq!(arms[0].0, "Ok");
    assert!(matches!(arms[0].1, Expression::Construct { .. }));
    let Expression::Construct {
        constructor,
        fields,
        result: failure_type,
    } = &arms[1].1
    else {
        panic!("propagated error")
    };
    assert_eq!(constructor, "Error");
    assert_eq!(*failure_type, result);
    assert_eq!(
        fields,
        &vec![Expression::Project {
            value: Box::new(Expression::Variable {
                id,
                ty: Type::Data("Result".into(), vec![Type::Int, error_type.clone()])
            }),
            constructor: "Error".into(),
            index: 0,
            result: error_type,
        }]
    );
}

#[test]
fn recursive_list_fold_connects_construction_projection_and_calls() {
    let source = "def rec sum | [] => 0 | [head, ...tail] => head + sum tail\ndef main = sum [2, 3, 5]";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    assert_eq!(program.functions.len(), 1);
    assert_eq!(
        program.functions[0].signature.parameters,
        vec![Type::Data("List".into(), vec![Type::Int])]
    );
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(
        output
            .matches("CallDirect { function: FunctionId(0)")
            .count(),
        1
    );
    assert!(output.contains("constructor: \"Cons\", index: 1, result: Data(\"List\", [Int])"));
    assert!(output.contains("constructor: \"Cons\", index: 0, result: Int"));
}

#[test]
fn divergent_mutual_recursion_uses_the_call_site_result_type() {
    let source = "def rec ping x => pong x\ndef rec pong x => ping x\ndef main = false && ping 0";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(
        output
            .matches("signature: Signature { parameters: [Int], result: Bool }")
            .count(),
        2
    );
    assert!(output.contains("otherwise: Constant(Bool(false))"));
}

#[test]
fn divergent_function_values_keep_the_inferred_result_type() {
    let source = "def rec loop x => loop x\ndef main = let f = loop -> false && f 0";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("MakeClosure { captures: [], function: FunctionId(0), signature: Signature { parameters: [Int], result: Bool } }"));
    assert!(output.contains("TailCallClosure"));
}

#[test]
fn specialization_cache_distinguishes_result_types() {
    let source =
        "def rec loop x => loop x\ndef main = let flag = false && loop 0 -> if flag => loop 0 | 1";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    assert_eq!(program.functions.len(), 2);
    assert_eq!(program.functions[0].signature.result, Type::Bool);
    assert_eq!(program.functions[1].signature.result, Type::Int);
    for function in &program.functions {
        let hado::mono::Expression::TailCall {
            function: target,
            result,
            ..
        } = &function.body
        else {
            panic!("self tail call")
        };
        assert_eq!(*target, function.id);
        assert_eq!(*result, function.signature.result);
    }
    assert_ne!(program.functions[0].id, program.functions[1].id);
}

#[test]
fn qualified_module_calls_resolve_to_the_exported_function() {
    let source = "def main = M.inc 2\nmodule S def inc(x: Int): Int\nimplement S as M def inc x => x + 1";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("name: \"M.inc\""));
    assert!(output.contains(
        "TailCall { function: FunctionId(0), arguments: [Constant(Int(2))], result: Int }"
    ));
}

#[test]
fn module_internal_calls_resolve_private_functions_in_their_module() {
    let source = "def helper x => x + 10\ndef main = M.inc 2 + helper 3\nmodule S def inc(x: Int): Int\nimplement S as M def helper x => x + 1 def inc x => helper x";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("name: \"M.helper\""));
    assert!(output.contains("name: \"helper\""));
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let inc = program
        .functions
        .iter()
        .find(|f| f.name == "M.inc")
        .unwrap();
    let helper = program
        .functions
        .iter()
        .find(|f| f.name == "M.helper")
        .unwrap();
    assert!(
        matches!(inc.body, hado::mono::Expression::TailCall { function, .. } if function == helper.id)
    );
}

#[test]
fn module_exports_can_be_used_as_function_values() {
    let source = "def main = let f = M.inc -> f 2\nmodule S def inc(x: Int): Int\nimplement S as M def inc x => x + 1";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("MakeClosure { captures: [], function: FunctionId(0)"));
    assert!(output.contains("TailCallClosure"));
    assert!(output.contains("name: \"M.inc\""));
}

#[test]
fn sibling_module_definition_metadata_uses_distinct_paths() {
    let source = "def main = A.run 1 + B.run 1\nmodule S def run(x: Int): Int\nimplement S as A def run x => x + 1\nimplement S as B def run x => x + 2";
    let checked = hado::check_source(source, "t").unwrap();
    assert!(checked.module_definitions.contains_key("A.run"));
    assert!(checked.module_definitions.contains_key("B.run"));
    assert!(!checked.module_definitions.contains_key("run"));
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("name: \"A.run\""));
    assert!(output.contains("name: \"B.run\""));
}

#[test]
fn integer_compare_is_synthesized_as_an_ir_function() {
    let output = hado::dump_mono("def main = compare 3 5", "t").unwrap();
    assert!(output.contains("name: \"compare_Int\""));
    assert!(output.contains("Constant(Int(-1))"));
    assert!(output.contains("Constant(Int(0))"));
    assert!(output.contains("Constant(Int(1))"));
}

#[test]
fn scalar_compare_derivations_have_distinct_signatures() {
    for (source, name, ty) in [
        ("def main = compare false true", "compare_Bool", Type::Bool),
        ("def main = compare 'a' 'b'", "compare_Char", Type::Char),
        ("def main = compare () ()", "compare_Unit", Type::Unit),
    ] {
        let checked = hado::check_source(source, "t").unwrap();
        let ast = hado::parser::parse_source(source, "t").unwrap();
        let program = hado::mono::lower_program(&ast, &checked).unwrap();
        let function = program.functions.iter().find(|f| f.name == name).unwrap();
        assert_eq!(function.signature.parameters, vec![ty.clone(), ty.clone()]);
        assert_eq!(function.signature.result, Type::Int);
        if ty == Type::Unit {
            assert_eq!(
                function.body,
                hado::mono::Expression::Constant(hado::mono::Constant::Int(0))
            );
        }
    }
}

#[test]
fn scalar_compare_cache_reuses_only_matching_types() {
    let source = "def main = compare 1 2 + compare false true + compare 3 4 + compare 'a' 'b'";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    assert_eq!(program.functions.len(), 3);
    for (name, ty) in [
        ("compare_Int", Type::Int),
        ("compare_Bool", Type::Bool),
        ("compare_Char", Type::Char),
    ] {
        let functions: Vec<_> = program
            .functions
            .iter()
            .filter(|f| f.name == name)
            .collect();
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].signature.parameters, vec![ty.clone(), ty]);
    }
    let ids: std::collections::HashSet<_> = program.functions.iter().map(|f| f.id.0).collect();
    assert_eq!(ids.len(), 3);
}

#[test]
fn integer_printer_is_an_ordinary_function_with_external_body() {
    let function = hado::mono::derive_int_printer(hado::mono::FunctionId(12)).unwrap();
    assert_eq!(function.name, "print_Int");
    assert_eq!(function.signature.parameters, vec![Type::Int]);
    assert_eq!(function.signature.result, Type::Unit);
    assert!(matches!(function.body, hado::mono::Expression::CallRuntime { function: hado::runtime_abi::RuntimeFunction::PrintInt, .. }));
}

#[test]
fn dump_mono_includes_the_generated_main_printer() {
    let output = hado::dump_mono("def main = 42", "t").unwrap();
    assert!(output.contains("name: \"print_Int\""));
    assert!(output.contains("CallRuntime { function: PrintInt"));
}

#[test]
fn scalar_equality_has_one_derived_function_per_type() {
    for (expression, name) in [("1 == 1 && 2 != 3", "Int"), ("true == false", "Bool"), ("1.0 == 1.0", "Float"), ("'a' == 'b'", "Char"), ("\"a\" == \"b\"", "String"), ("() == ()", "Unit")] {
        let source = format!("def main = {expression}");
        let output = hado::dump_mono(&source, "t").unwrap();
        assert_eq!(output.matches(&format!("name: \"eq_{name}\"")).count(), 1, "{output}");
    }
}

#[test]
fn aggregate_equality_reuses_scalar_derivations() {
    let source = "type Pair = { left: Int; right: Int }\ndef main = { left = 1; right = 2 } == { left = 1; right = 2 }";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("name: \"eq_Int\"").count(), 1, "{output}");
    let source = "type Pair = | Pair(Int, Int)\ndef main = Pair(1, 2) == Pair(1, 2)";
    let output = hado::dump_mono(source, "t").unwrap();
    assert_eq!(output.matches("name: \"eq_Int\"").count(), 1, "{output}");
}

#[test]
fn parenthesized_constructor_calls_lower_with_payload_types() {
    let source = "def main = Some(42)";
    let output = hado::dump_mono(source, "t").unwrap();
    assert!(output.contains("constructor: \"Some\", fields: [Constant(Int(42))]"), "{output}");
}
