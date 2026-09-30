use super::*;

fn parse_expr_ok(src: &str) -> String {
    canonical(parse_source(src, "t").unwrap().main().unwrap())
}

/// Type of the first top-level `type` item, if present.
fn type_of(src: &str) -> Option<TypeExpr> {
    match parse_source(src, "t").unwrap().items.into_iter().next() {
        Some(Item::TypeDef(td)) => td.ty,
        _ => None,
    }
}

#[test]
fn precedence_mul_over_add() {
    assert_eq!(parse_expr_ok("def main = 1 + 2 * 3"), "(1 + (2 * 3))");
    assert_eq!(parse_expr_ok("def main = 2 * 3 + 1"), "((2 * 3) + 1)");
    assert_eq!(parse_expr_ok("def main = (1 + 2) * 3"), "((1 + 2) * 3)");
}

#[test]
fn left_assoc_add_and_mul() {
    assert_eq!(parse_expr_ok("def main = 1 + 2 + 3"), "((1 + 2) + 3)");
    assert_eq!(parse_expr_ok("def main = 10 / 2 / 5"), "((10 / 2) / 5)");
}

#[test]
fn comparisons_and_logic() {
    assert_eq!(
        parse_expr_ok("def main = 1 < 2 && 3 >= 4"),
        "((1 < 2) && (3 >= 4))"
    );
    assert_eq!(
        parse_expr_ok("def main = 1 < 2 || 3 > 4"),
        "((1 < 2) || (3 > 4))"
    );
    assert_eq!(parse_expr_ok("def main = 1 == 2"), "(1 == 2)");
}

#[test]
fn unary_minus_binds_tightest() {
    assert_eq!(parse_expr_ok("def main = - 2 * 3"), "((-2) * 3)");
    assert_eq!(parse_expr_ok("def main = 1 - - 2"), "(1 - (-2))");
}

#[test]
fn comments_and_newlines_are_ignored() {
    assert_eq!(parse_expr_ok("def main = 1 // hi\n + 2"), "(1 + 2)");
}

#[test]
fn non_assoc_comparison_rejected() {
    let d = parse_source("def main = 1 < 2 < 3", "t").unwrap_err();
    assert!(d.message.contains("non-associative"), "got: {d}");
}

#[test]
fn application_and_pipe() {
    assert_eq!(parse_expr_ok("def main = f x y"), "f x y");
    assert_eq!(parse_expr_ok("def main = 1 |> add 2 3"), "(1 |> add 2 3)");
}

#[test]
fn constructor_and_field() {
    assert_eq!(parse_expr_ok("def main = Some(x)"), "Some(x)");
    assert_eq!(parse_expr_ok("def main = M.inc 2"), "M.inc 2");
    assert_eq!(parse_expr_ok("def main = user.name"), "user.name");
}

#[test]
fn record_and_list_literals() {
    assert_eq!(
        parse_expr_ok("def main = { name = \"a\"; age = 3 }"),
        "{ name = \"a\"; age = 3 }"
    );
    assert_eq!(parse_expr_ok("def main = [1, 2, 3]"), "[1, 2, 3]");
    assert_eq!(parse_expr_ok("def main = []"), "[]");
}

#[test]
fn bindings_and_fn() {
    assert_eq!(
        parse_expr_ok("def main = 1 as x -> x + 1"),
        "(1 as x -> (x + 1))"
    );
    assert_eq!(parse_expr_ok("def main = g x -> x"), "(g x -> x)");
    assert_eq!(
        parse_expr_ok("def main = fn(x => x + 1)"),
        "fn(x => (x + 1))"
    );
}

/// §11・§34: `let` と `do` と `as p? ->` は廃止され、E100 になる。
#[test]
fn removed_binding_keywords_are_e100() {
    for source in [
        "def main = let x = 1 -> x",
        "def main = let x? = f y -> x",
        "def main = do g x -> x",
        "def main = f y as x? -> x",
    ] {
        let d = parse_source(source, "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E100"), "{source}: {d}");
    }
}

/// §12.1: `if` の閉じは本体の `->` の有無と続きの有無で決まる。
#[test]
fn if_close_follows_the_rule() {
    for source in [
        "def main = if true => 1 | 0",
        "def main = if (a + b) * 2 > c => 1 | 0",
        "def main = if(c => 1 | 2) + 1",
        "def main = if((a + b) * 2 > c => 1 | 0) as n -> n",
    ] {
        let parsed = parse_source(source, "t");
        assert!(parsed.is_ok(), "{source}: {:?}", parsed.err());
    }
    for (source, want) in [
        ("def main = if true => 1 | log \"a\" -> 2", "E100"),
        ("def main = if true => 1 | 2 /if", "E100"),
        ("def main = if true => 1 | 0 /if", "E100"),
        ("def main = if true => if x | A => f(x) | 0", "E101"),
    ] {
        let d = parse_source(source, "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some(want), "{source}: {d}");
    }
}

#[test]
fn branches() {
    assert_eq!(
        parse_expr_ok("def main = if x > 10 => Large | Small"),
        "(if (x > 10) => Large | Small)"
    );
    assert_eq!(
        parse_expr_ok("def main = if | x > 1 => 1 | 0"),
        "(if | (x > 1) => 1 | 0)"
    );
    assert_eq!(
        parse_expr_ok("def main = if x | Some(v) => v | 0"),
        "(if x | Some(v) => v | 0)"
    );
}

#[test]
fn adt_accepts_leading_pipe() {
    assert!(parse_source("type T Nest =\n  | Leaf(T)\n  | Node(T List Nest)", "t",).is_ok());
    assert!(parse_source("type Color =\n  | Red\n  | Green\n  | Blue", "t").is_ok());
    assert!(parse_source("type T List2 = | Nil2 | Cons2(T, T List2)", "t").is_ok());
    assert_eq!(
        type_of("type Color =\n  | Red\n  | Green\n  | Blue"),
        Some(type_expr(TypeKind::Adt(
            ["Red", "Green", "Blue"]
                .into_iter()
                .map(|name| crate::parser::ConstructorDecl {
                    name: crate::parser::DeclaredName {
                        name: name.into(),
                        span: Span::point(1, 1)
                    },
                    payload: crate::parser::ConstructorPayload::Nullary
                })
                .collect()
        ))),
    );
}

#[test]
fn not_unary_precedence() {
    // `not` sits in the same tier as unary `-` (§7), above `&&` and the
    // comparisons.
    assert_eq!(parse_expr_ok("def main = not true"), "(not true)");
    assert_eq!(parse_expr_ok("def main = not (1 == 2)"), "(not (1 == 2))");
    assert_eq!(
        parse_expr_ok("def main = not true && false"),
        "((not true) && false)"
    );
    assert_eq!(parse_expr_ok("def main = not 1 == 2"), "((not 1) == 2)");
    assert_eq!(parse_expr_ok("def main = not not true"), "(not (not true))");
    assert_eq!(parse_expr_ok("def main = - not true"), "(-(not true))");
    assert_eq!(parse_expr_ok("def main = not - 1"), "(not (-1))");
}

#[test]
fn e101_nested_branch() {
    let d = parse_source(
        "def f x =>\n  if x\n  | 1 => if x\n    | 2 => 5\n    | 0\n  | 0",
        "t",
    )
    .unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E101"));
    assert_eq!(d.line, 3, "inner `if` is on line 3 of the source, got {d}");
}

fn round_trip(source: &str) {
    let ast = match parse_source(source, "t") {
        Ok(ast) => ast,
        Err(error) => panic!("{source}: {error}"),
    };
    let printed = canonical_program(&ast);
    let restored = match parse_source(&printed, "t") {
        Ok(ast) => ast,
        Err(error) => panic!("{source} -> {printed:?}: {error}"),
    };
    assert_eq!(ast.main(), restored.main(), "{source}\n{printed}");
    assert_eq!(printed, canonical_program(&restored), "{printed}");
}

#[test]
fn multi_column_pattern_branch_prints_and_round_trips() {
    assert_eq!(
        parse_expr_ok("def main = if a, b | [], _ => b | a"),
        "(if a, b | [], _ => b | a)"
    );
    round_trip("def main = if a, b | [], _ => b | _, [] => a | 0");
    round_trip("def main = if a, b, c | 1, 2, 3 => 0 | 1");
}

#[test]
fn pattern_column_count_is_e125_at_the_arm_pipe() {
    let source = "def f (a: Int List, b: Int List): Int =>\n  if a, b\n  | [] => 0\n  | [_, ..._], [_, ..._] => 1\n  | 2\ndef main = f [] []";
    let error = parse_source(source, "t").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E125"), "{error}");
    assert_eq!((error.line, error.col), (3, 3), "{error}");
    let extra = parse_source("def main = if a, b | [], _, _ => 0 | 1", "t").unwrap_err();
    assert_eq!(extra.code.as_deref(), Some("E125"), "{extra}");
    assert_eq!((extra.line, extra.col), (1, 20), "{extra}");
}

#[test]
fn arm_conditions_print_as_if_column_and_round_trip() {
    assert_eq!(
        parse_expr_ok("def main = if x | Some(v) => if | v > 0 => v | 0 | None => 1"),
        "(if x | Some(v) => if | (v > 0) => v | 0 | None => 1)"
    );
    round_trip("def main = if x | Some(v) => if | v > 0 => v | 0 | None => 1");
    round_trip("def f | Pass(score) => if | score >= 90 => \"excellent\" | \"pass\" | Fail => \"fail\"\ndef main = f");
}

#[test]
fn arm_conditions_end_at_fallback_so_later_pattern_arms_remain() {
    let ast = parse_source(
        "def main = if x | Some(v) => if | v > 0 => v | 0 | None => 1",
        "t",
    )
    .unwrap();
    let NodeKind::PatternBranch { arms, fallback, .. } = &ast.main().unwrap().kind else {
        panic!("expected pattern branch");
    };
    assert_eq!(
        arms.len(),
        2,
        "{:?}",
        arms.iter()
            .map(|(p, _)| p.iter().map(|p| p.to_string()).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    assert!(fallback.is_none());
    assert!(matches!(arms[0].1.kind, NodeKind::CondBranch { .. }));
    assert!(!matches!(arms[1].1.kind, NodeKind::CondBranch { .. }));
}

#[test]
fn pattern_shaped_fallback_before_an_arm_still_records_e404() {
    let source =
        "def f (x: Int Option): Int =>\n  if x\n  | None\n  | Some(v) => v\ndef main = f None";
    let ast = parse_source(source, "t").unwrap();
    assert_eq!(
        ast.branches_after_fallback.len(),
        1,
        "{:?}",
        ast.branches_after_fallback
    );
    assert_eq!(
        ast.branches_after_fallback[0],
        crate::lexer::Span::point(4, 3).until(crate::lexer::Span::point(4, 4))
    );
}

#[test]
fn pattern_fallback_does_not_steal_an_enclosing_comma() {
    // §12.1: `,` と `)` は中の `if` の閉じも兼ねるので、印は要らない。
    round_trip("def main = Cons(if x | None => 1 | 0, xs)");
    round_trip("def main = with H(if x | None => 1 | 0, 2) -> 3");
    round_trip("def main = [if x | None => 1 | 0, 2]");
}

#[test]
fn e105_tuple_pattern() {
    // §20: tuple syntax in a pattern is also E105, not a plain error.
    let d = parse_source("def main = (a, b) as p -> a", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E105"));
    let d = parse_source("def main = if x | (a, b) => a | 0", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E105"));
}

#[test]
fn lower_m0_subset() {
    let ast = parse_source("def main = 1 + 2 + 3", "t").unwrap();
    let prog = lower_to_m0(&ast).unwrap();
    assert_eq!(
        prog.body,
        Expr::Add(
            Box::new(Expr::Add(Box::new(Expr::Int(1)), Box::new(Expr::Int(2)))),
            Box::new(Expr::Int(3))
        )
    );
    assert!(lower_to_m0(&parse_source("def other = 1", "t").unwrap()).is_none());
    assert!(lower_to_m0(&parse_source("def main = 1 * 2", "t").unwrap()).is_none());
}

#[test]
fn typedef_preserves_parameters() {
    for (source, expected) in [
        ("type T List2 = | Nil2 | Cons2(T, T List2)", vec!["T"]),
        ("type (A, B) Pair = { first: A; second: B }", vec!["A", "B"]),
        ("type Count = | Count(Int)", vec![]),
    ] {
        let ast = parse_source(source, "t").unwrap();
        let Item::TypeDef(def) = &ast.items[0] else {
            panic!("expected type")
        };
        assert_eq!(
            def.params
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
    println!("typedef_params_ok: 1");
}

#[test]
fn lower_rejects_extra_items() {
    let source = "def other = 1\ndef main = 1 + 2";
    let ast = parse_source(source, "t").unwrap();
    assert!(lower_to_m0(&ast).is_none());
    let error = crate::compile_to_object(source, std::path::Path::new("unused.o")).unwrap_err();
    assert!(error.contains("E513"), "{error}");
    println!("lower_extra_def_ng: 1");
    for extra in ["type Extra = | Extra(Int)", "def main = 4"] {
        let source = format!("def main = 1 + 2\n{extra}");
        assert!(lower_to_m0(&parse_source(&source, "t").unwrap()).is_none());
    }
}

#[test]
fn m0_parse_entry() {
    let toks = crate::lexer::lex("def main = 10 + 20").unwrap();
    let prog = parse(&toks).unwrap();
    assert_eq!(
        prog.body,
        Expr::Add(Box::new(Expr::Int(10)), Box::new(Expr::Int(20)))
    );
    let toks = crate::lexer::lex("def main = 1 < 2").unwrap();
    assert!(parse(&toks).is_err());
}

fn type_expr(kind: TypeKind) -> TypeExpr {
    TypeExpr {
        kind,
        span: Span::point(1, 1),
        name_span: Span::point(1, 1),
        effects: None,
    }
}
