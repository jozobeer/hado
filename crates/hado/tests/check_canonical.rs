use hado::{check, parser::parse_source};

#[test]
fn identity_binding_reports_its_location() {
    let ast = parse_source("def f v => v as x ->\n  x\ndef main = f 1", "t").unwrap();
    let error = check::check_ast(&ast, "t").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E501"));
    assert_eq!((error.line, error.col), (1, 12));
}

#[test]
fn identity_binding_is_found_inside_nested_expressions() {
    let ast = parse_source("def main = Some(1 as x ->\n  x)", "t").unwrap();
    assert_eq!(
        check::check_ast(&ast, "t").unwrap_err().code.as_deref(),
        Some("E501")
    );
    let ast = parse_source("def main = 1 as x ->\n  x + 2", "t").unwrap();
    check::check_ast(&ast, "t").unwrap();
}

#[test]
fn duplicate_patterns_point_to_the_previous_arm() {
    let ast = parse_source("def f x => if x\n| 1 => 10\n| 1 => 20\n| 0", "t").unwrap();
    let error = check::check_ast(&ast, "t").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E405"));
    assert_eq!(error.line, 3);
    assert!(error.message.contains("2:3"));
    let ast = parse_source("def f | 1 => 10 | 2 => 20 | 0", "t").unwrap();
    check::check_ast(&ast, "t").unwrap();
}

#[test]
fn duplicate_patterns_ignore_bound_variable_names() {
    for source in [
        "def f | Some(x) => x | Some(y) => y | 0",
        "def f | Some(_) => 1 | Some(x) => 2 | 0",
    ] {
        let ast = parse_source(source, "t").unwrap();
        assert_eq!(
            check::check_ast(&ast, "t").unwrap_err().code.as_deref(),
            Some("E405")
        );
    }
    let ast = parse_source("def f | Some(1) => 1 | Some(2) => 2 | 0", "t").unwrap();
    check::check_ast(&ast, "t").unwrap();
}

#[test]
fn repeated_or_unreachable_conditions_are_rejected() {
    for source in [
        "def f x | x > 1 => 1 | x > 1 => 2 | 0",
        "def f x | true => 1 | x > 1 => 2 | 0",
    ] {
        let ast = parse_source(source, "t").unwrap();
        assert_eq!(
            check::check_ast(&ast, "t").unwrap_err().code.as_deref(),
            Some("E406")
        );
    }
    let ast = parse_source("def f x | x > 2 => 1 | x > 1 => 2 | 0", "t").unwrap();
    check::check_ast(&ast, "t").unwrap();
}

#[test]
fn identical_branches_ignore_local_names_but_preserve_free_names() {
    for source in [
        "def main = if true => 1 | 1",
        "def f x => if true => x + 1 | x + 1",
        "def main = if true => fn(x => x + 1) | fn(y => y + 1)",
    ] {
        let ast = parse_source(source, "t").unwrap();
        assert_eq!(
            check::check_ast(&ast, "t").unwrap_err().code.as_deref(),
            Some("E502")
        );
    }
    let ast = parse_source("def main = if true => fn(x => x + a) | fn(y => y + b)", "t").unwrap();
    check::check_ast(&ast, "t").unwrap();
    let ast = parse_source("def main = if true => 1 | 2", "t").unwrap();
    check::check_ast(&ast, "t").unwrap();
}

#[test]
fn condition_columns_require_fallback_even_after_true() {
    for source in ["def f x | x > 1 => 1", "def main = if | true => 1"] {
        let ast = parse_source(source, "t").unwrap();
        assert_eq!(
            check::check_ast(&ast, "t").unwrap_err().code.as_deref(),
            Some("E401")
        );
    }
    let ast = parse_source("def f x | x > 1 => 1 | 0", "t").unwrap();
    check::check_ast(&ast, "t").unwrap();
}

#[test]
fn non_boolean_literal_patterns_require_fallback_at_any_depth() {
    for pattern in [
        "1",
        "1.5",
        "'a'",
        "\"abc\"",
        "Some(1)",
        "{ value = Some(1) }",
    ] {
        let source = format!("def f | {pattern} => 1");
        let ast = parse_source(&source, "t").unwrap();
        assert_eq!(
            check::check_ast(&ast, "t").unwrap_err().code.as_deref(),
            Some("E402"),
            "{source}"
        );
        let source = format!("{source} | 0");
        check::check_ast(&parse_source(&source, "t").unwrap(), "t").unwrap();
    }
    let ast = parse_source(
        "def f | Some(true) => 1 | Some(false) => 0 | None => -1",
        "t",
    )
    .unwrap();
    check::check_ast(&ast, "t").unwrap();
}

#[test]
fn equality_columns_for_one_variable_require_pattern_syntax() {
    for conditions in [
        "x == 1",
        "x == 1 => 1 | x == 2",
        "x == 'a'",
        "x == \"a\"",
        "x == true",
        "x == 1.5",
    ] {
        let source = format!("def f x | {conditions} => 2 | 0");
        let ast = parse_source(&source, "t").unwrap();
        assert_eq!(
            check::check_ast(&ast, "t").unwrap_err().code.as_deref(),
            Some("E511"),
            "{source}"
        );
    }
    for conditions in [
        "x == 1 => 1 | y == 2",
        "x == 1 => 1 | x > 2",
        "x == y",
        "f x == 1",
    ] {
        let params = if conditions.contains('y') { "x y" } else { "x" };
        let source = format!("def f {params} | {conditions} => 2 | 0");
        check::check_ast(&parse_source(&source, "t").unwrap(), "t").unwrap();
    }
}

#[test]
fn nullary_constructors_keep_their_identity_during_normalization() {
    let source = "type Color = | Red | Blue\ndef f | Red => 1 | Blue => 2\ndef main = f Red";
    hado::check_source(source, "t").unwrap();
    let source =
        "type Color = | Red | Blue\ndef f | Red => 1 | Red => 2 | Blue => 3\ndef main = f Red";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E405")
    );
}

#[test]
fn broader_constructor_pattern_makes_later_arm_unreachable() {
    let source = "def f | Some(_) => 1 | Some(3) => 2 | 0\ndef main = f (Some(1))";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E403")
    );
    let source = "def f | Some(3) => 1 | Some(_) => 2 | 0\ndef main = f (Some(1))";
    hado::check_source(source, "t").unwrap();
}

#[test]
fn fallback_must_be_last_in_both_branch_forms() {
    for source in [
        "def main = if | 1 > 2 => 1 | 0 | 2 > 3 => 2",
        "def main = if | 1 > 2 => 1 | 0 | 2",
        "def main = if 1 | 1 => 10 | 0 | 2 => 20",
    ] {
        assert_eq!(
            hado::check_source(source, "t")
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("E404"),
            "{source}"
        );
    }
}

#[test]
fn unused_let_binding_respects_shadowing() {
    for source in [
        "def main = 1 as x ->\n  2",
        "def main = 1 as x ->\n  fn(x => x + 2) 3",
    ] {
        assert_eq!(
            hado::check_source(source, "t")
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("E505")
        );
    }
    for source in [
        "def main = 1 as x ->\n  fn(y => x + y) 3",
        "def main = 1 as x ->\n  x + 1 as x ->\n  x + 2",
    ] {
        hado::check_source(source, "t").unwrap();
    }
}

#[test]
fn rebuilding_every_constructor_is_redundant() {
    let source = "def f | Some(v) => Some(v) | None => None\ndef main = f (Some(1))";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E507")
    );
    for source in [
        "def f | Some(v) => Some(v + 1) | None => Some(0)\ndef main = f (Some(1))",
        "type Pair = | Pair(Int, Int)\ndef f | Pair(a, b) => Pair(b, a)\ndef main = f (Pair(1, 2))",
    ] {
        hado::check_source(source, "t").unwrap();
    }
}

#[test]
fn record_rebuilding_ignores_field_order_but_preserves_values() {
    let prefix = "type Pair = { left: Int; right: Int }\n";
    let suffix = "\ndef main = f { left = 1; right = 2 }";
    let source =
        format!("{prefix}def f | {{ left = a; right = b }} => {{ right = b; left = a }}{suffix}");
    assert_eq!(
        hado::check_source(&source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E507")
    );
    let source =
        format!("{prefix}def f | {{ left = a; right = b }} => {{ right = a; left = b }}{suffix}");
    hado::check_source(&source, "t").unwrap();
}

#[test]
fn unchanged_none_branch_requires_railway_syntax() {
    let source = "def f | Some(x) => Some(x + 1) | None => None\ndef main = f (Some(1))";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E508")
    );
    let source = "def f | Some(x) => Some(x + 1) | None => Some(0)\ndef main = f (None)";
    hado::check_source(source, "t").unwrap();
}

#[test]
fn selective_none_handling_is_not_plain_repropagation() {
    let source = "def f | Some(1) => Some(1) | Some(0)\ndef main = f (Some(2))";
    hado::check_source(source, "t").unwrap();
}

#[test]
fn record_binding_checks_each_bound_name() {
    let prefix = "type Pair = { a: Int; b: Int }\ndef main = ";
    for pattern in ["{ a; b }", "{ a = x; b }"] {
        let source = format!("{prefix}{{ a = 1; b = 2 }} as {pattern} -> b + 1");
        assert_eq!(
            hado::check_source(&source, "t")
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("E505")
        );
    }
    let source = format!("{prefix}{{ a = 1; b = 2 }} as {{ a = _; b }} -> b + 1");
    hado::check_source(&source, "t").unwrap();
}

#[test]
fn unused_railway_binding_requires_do() {
    let source = "def main = Some(1) as x? ->\n  Some(2)";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E505")
    );
    hado::check_source("def main = Some(1) as x? ->\n  Some(x + 2)", "t").unwrap();
}

#[test]
fn body_type_error_precedes_discarded_railway_syntax() {
    let source = "def main = Some(1) as _? ->\n  1";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E305")
    );
}

#[test]
fn boolean_patterns_must_cover_both_values_without_redundant_fallback() {
    for (source, code) in [
        ("def main = if true | true => 1", "E402"),
        ("def main = if true | true => 1 | false => 0 | 2", "E403"),
    ] {
        assert_eq!(
            hado::check_source(source, "t")
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some(code)
        );
    }
    for source in [
        "def main = if true => 1 | 0",
        "def main = if false => 1 | 0",
    ] {
        hado::check_source(source, "t").unwrap();
    }
}

#[test]
fn restricted_payload_does_not_hide_missing_constructor() {
    let source = "def f | Some(true) => 1\ndef main = f (Some(true))";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E402")
    );
    hado::check_source(
        "def f | Some(true) => 1 | 0\ndef main = f (Some(true))",
        "t",
    )
    .unwrap();
}

#[test]
fn boolean_payload_coverage_preserves_argument_correlations() {
    let prefix = "type Pair = | Pair(Bool, Bool)\ndef f ";
    let suffix = "\ndef main = f (Pair(true, false))";
    let source = format!("{prefix}| Pair(true, true) => 1 | Pair(false, false) => 2{suffix}");
    assert_eq!(
        hado::check_source(&source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E402")
    );
    let source = format!("{prefix}| Pair(true, _) => 1 | Pair(false, _) => 2{suffix}");
    hado::check_source(&source, "t").unwrap();
    let source = format!("{prefix}| Pair(true, _) => 1 | Pair(false, _) => 2 | 0{suffix}");
    assert_eq!(
        hado::check_source(&source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E403")
    );
}

#[test]
fn generated_boolean_payload_subsets_match_finite_domain_oracle() {
    for mask in 1u8..16 {
        let arms: String = (0u8..4)
            .filter(|value| mask & (1 << value) != 0)
            .map(|value| {
                format!(
                    " | Pair({}, {}) => {}",
                    value & 2 != 0,
                    value & 1 != 0,
                    value + 10
                )
            })
            .collect();
        let source = format!(
            "type Pair = | Pair(Bool, Bool)\ndef f{arms}\ndef main = f (Pair(false, false))"
        );
        let checked = hado::check_source(&source, "t");
        if mask.count_ones() == 4 {
            checked.unwrap();
        } else {
            assert_eq!(
                checked.err().unwrap().code.as_deref(),
                Some("E402"),
                "mask {mask}"
            );
        }
        let source = source.replace("\ndef main", " | 0\ndef main");
        let checked = hado::check_source(&source, "t");
        if mask.count_ones() == 4 {
            assert_eq!(checked.err().unwrap().code.as_deref(), Some("E403"));
        } else {
            checked.unwrap();
        }
    }
    let source = "type Pair = | Pair(Bool, Bool)\ndef f | 0\ndef main = f (Pair(false, false))";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E100")
    );
}

#[test]
fn nested_adt_coverage_tracks_inner_alternatives() {
    for (arms, code) in [
        ("Some(Some(v)) => v | None => 0", Some("E402")),
        ("Some(Some(v)) => v | Some(None) => 1 | None => 0", None),
        (
            "Some(Some(v)) => v | Some(None) => 1 | None => 0 | 2",
            Some("E403"),
        ),
    ] {
        let source = format!("def f | {arms}\ndef main = f (Some(Some(1)))");
        let result = hado::check_source(&source, "t");
        match code {
            Some(code) => assert_eq!(result.err().unwrap().code.as_deref(), Some(code)),
            None => {
                result.unwrap();
            }
        }
    }
}

#[test]
fn record_pattern_coverage_uses_field_names_and_omitted_fields() {
    let prefix = "type Flags = { a: Bool; b: Bool }\ndef f ";
    let suffix = "\ndef main = f { a = true; b = false }";
    for (arms, expected) in [
        (
            "| { a = true; b = true } => 1 | { b = false; a = false } => 2",
            Some("E402"),
        ),
        ("| { a = true } => 1 | { a = false } => 2", None),
        ("| { a = true } => 1 | { a = false } => 2 | 0", Some("E403")),
    ] {
        let source = format!("{prefix}{arms}{suffix}");
        let result = hado::check_source(&source, "t");
        match expected {
            Some(code) => assert_eq!(result.err().unwrap().code.as_deref(), Some(code)),
            None => {
                result.unwrap();
            }
        }
    }
}
