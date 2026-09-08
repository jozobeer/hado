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
}

#[test]
fn left_assoc_add_and_mul() {
    assert_eq!(
        parse_expr_ok("def main = 1 + 2 + 3"),
        "((1 + 2) + 3)"
    );
    assert_eq!(
        parse_expr_ok("def main = 10 / 2 / 5"),
        "((10 / 2) / 5)"
    );
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
fn parens_override_precedence() {
    assert_eq!(
        parse_expr_ok("def main = (1 + 2) * 3"),
        "((1 + 2) * 3)"
    );
}

#[test]
fn comments_and_newlines_are_ignored() {
    assert_eq!(
        parse_expr_ok("def main = 1 // hi\n + 2"),
        "(1 + 2)"
    );
}

#[test]
fn non_assoc_comparison_rejected() {
    let d = parse_source("def main = 1 < 2 < 3", "t").unwrap_err();
    assert!(d.message.contains("non-associative"), "got: {d}");
}

#[test]
fn application_and_pipe() {
    assert_eq!(
        parse_expr_ok("def main = f x y"),
        "f x y"
    );
    assert_eq!(
        parse_expr_ok("def main = 1 |> add 2 3"),
        "(1 |> add 2 3)"
    );
}

#[test]
fn constructor_and_field() {
    assert_eq!(
        parse_expr_ok("def main = Some(x)"),
        "Some(x)"
    );
    assert_eq!(
        parse_expr_ok("def main = M.inc 2"),
        "M.inc 2"
    );
    assert_eq!(
        parse_expr_ok("def main = user.name"),
        "user.name"
    );
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
        parse_expr_ok("def main = let x = 1 -> x + 1"),
        "(let x = 1 -> (x + 1))"
    );
    assert_eq!(
        parse_expr_ok("def main = let x? = f y -> x"),
        "(let x? = f y -> x)"
    );
    assert_eq!(
        parse_expr_ok("def main = do g x -> x"),
        "(do g x -> x)"
    );
    assert_eq!(
        parse_expr_ok("def main = fn x => x + 1"),
        "(fn x => (x + 1))"
    );
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
fn defs_modules_types() {
    assert!(parse_source("def add x y => x + y", "t").is_ok());
    assert!(parse_source("def rec ping x => pong x", "t").is_ok());
    assert!(parse_source("type List2<T> = | Nil2 | Cons2(T, List2<T>)", "t").is_ok());
    assert!(parse_source("module S def inc(x: Int): Int", "t").is_ok());
    assert!(parse_source("implement S as M def inc x => x + 1", "t").is_ok());
}

#[test]
fn adt_accepts_leading_pipe() {
    assert!(parse_source(
        "type Nest<T> =\n  | Leaf(T)\n  | Node(Nest<List<T>>)",
        "t",
    )
    .is_ok());
    assert!(parse_source("type Color =\n  | Red\n  | Green\n  | Blue", "t").is_ok());
    assert!(parse_source("type List2<T> = | Nil2 | Cons2(T, List2<T>)", "t").is_ok());
    assert_eq!(
        type_of("type Color =\n  | Red\n  | Green\n  | Blue"),
        Some(type_expr(TypeKind::Adt(vec![named_type("Red"), named_type("Green"), named_type("Blue")]))),
    );
}

#[test]
fn fn_type_group() {
    // §37: `Fn<(Int, Int), Int>` is the v0.1 function type. The `( )`
    // is an argument group, not a tuple.
    assert_eq!(
        match type_of("type F = { function: Fn<(Int, Int), Int> }") {
            Some(TypeExpr { kind: TypeKind::Record(mut fields), .. }) => Some(fields.remove(0).ty),
            _ => None,
        },
        Some(type_expr(TypeKind::Apply(
            "Fn".into(),
            vec![type_expr(TypeKind::Group(vec![named_type("Int"), named_type("Int")])), named_type("Int")],
        ))),
    );
    assert!(parse_source("def f(x: Fn<(Int, Int), Int>): Int => 1", "t").is_ok());
    assert!(parse_source("type F = { function: Fn<Int, String> }", "t").is_ok());
}

#[test]
fn not_unary_precedence() {
    // `not` sits in the same tier as unary `-` (§44), above `&&` and the
    // comparisons.
    assert_eq!(parse_expr_ok("def main = not true"), "(not true)");
    assert_eq!(parse_expr_ok("def main = not (1 == 2)"), "(not (1 == 2))");
    assert_eq!(
        parse_expr_ok("def main = not true && false"),
        "((not true) && false)"
    );
    assert_eq!(
        parse_expr_ok("def main = not 1 == 2"),
        "((not 1) == 2)"
    );
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

#[test]
fn e102_rec_on_value() {
    let d = parse_source("def rec pi = 3.14159", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E102"));
}

#[test]
fn e105_no_tuple() {
    let d = parse_source("def main = (1, 2)", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E105"));
}

#[test]
fn e105_tuple_pattern() {
    // §37: tuple syntax in a pattern is also E105, not a plain error.
    let d = parse_source("def main = let (a, b) = x -> a", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E105"));
    let d = parse_source("def main = if x | (a, b) => a | 0", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E105"));
}

#[test]
fn e106_constructor_call_form() {
    let d = parse_source("def main = Some 1", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E106"));
}

#[test]
fn e107_operator_overloading() {
    let d = parse_source("def (+) a b => a", "t").unwrap_err();
    assert_eq!(d.code.as_deref(), Some("E107"));
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
        ("type List2<T> = | Nil2 | Cons2(T, List2<T>)", vec!["T"]),
        ("type Pair<A, B> = { first: A; second: B }", vec!["A", "B"]),
        ("type Count = | Count(Int)", vec![]),
    ] {
        let ast = parse_source(source, "t").unwrap();
        let Item::TypeDef(def) = &ast.items[0] else { panic!("expected type") };
        assert_eq!(def.params.iter().map(|param| param.name.as_str()).collect::<Vec<_>>(), expected);
    }
    println!("typedef_params_ok: 1");
}

#[test]
fn lower_rejects_extra_items() {
    let source = "def other = 1\ndef main = 1 + 2";
    let ast = parse_source(source, "t").unwrap();
    assert!(lower_to_m0(&ast).is_none());
    let error = crate::compile_to_object(source, std::path::Path::new("unused.o"))
        .unwrap_err();
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
    TypeExpr { kind, span: Span { line: 1, col: 1 } }
}

fn named_type(name: &str) -> TypeExpr { type_expr(TypeKind::Name(name.into())) }
