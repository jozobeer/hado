use hado::check_source;

#[test]
fn application_type_errors_point_to_the_failing_argument() {
    for padding in 0..20 {
        for (prefix, argument, suffix, code) in [
            ("def add x => x + 1\ndef main = add ", "true", "", "E305"),
            (
                "def add (x: Int, y: Int) => x + y\ndef main = add 1 ",
                "true",
                "",
                "E305",
            ),
            ("def add x => x + 1\ndef main = ", "true", " |> add", "E305"),
            (
                "def add (x: Int, y: Int) => x + y\ndef main = 1 |> add ",
                "true",
                "",
                "E305",
            ),
            (
                "type Box = | Pack(Int)\ndef main = Pack(",
                "true",
                ")",
                "E305",
            ),
            (
                "def self_apply x => x ",
                "x",
                "\ndef main = self_apply 1",
                "E306",
            ),
        ] {
            let prefix = format!("{prefix}\n{}", " ".repeat(padding));
            let source = format!("{prefix}{argument}{suffix}");
            let error = check_source(&source, "argument-anchor.hd").err().unwrap();
            assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
            assert_eq!(
                (error.line, error.col),
                (
                    prefix.bytes().filter(|b| *b == b'\n').count() + 1,
                    padding + 1
                ),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn application_arity_and_non_function_errors_keep_their_own_anchors() {
    for (source, code, anchor) in [
        (
            "type Holder = | Hold(Fn(Int => Int))\ndef main = Hold(fn(a, b => a + b))",
            "E301",
            "Hold(fn",
        ),
        ("def main = 1 |> true", "E305", "true"),
    ] {
        let error = check_source(source, "application-anchor.hd").err().unwrap();
        let prefix = &source[..source.find(anchor).unwrap()];
        assert_eq!(error.code.as_deref(), Some(code), "{error}");
        assert_eq!(
            (error.line, error.col),
            (
                prefix.bytes().filter(|b| *b == b'\n').count() + 1,
                prefix.rsplit('\n').next().unwrap().len() + 1
            ),
            "{error}"
        );
    }
}

const SOURCE_PATH: &str = "generated-order.hd";

fn code(source: &str) -> String {
    match check_source(source, SOURCE_PATH) {
        Err(error) => error
            .code
            .clone()
            .unwrap_or_else(|| panic!("uncoded diagnostic: {error:?}"))
            .to_string(),
        Ok(_) => panic!("expected a diagnostic for {source}"),
    }
}

#[test]
fn types_precede_redundancy() {
    assert_eq!(code("def main = 1 + \"bad\" as x -> x"), "E305");
}

#[test]
fn redundancy_precedes_monomorphization() {
    assert_eq!(code("def main = 1 as unused -> []"), "E505");
}

#[test]
fn later_type_errors_precede_earlier_coverage_errors() {
    let source = "def a x => if x | Some(v) => v\ndef z = 1 + true\ndef main = a (Some(z))";
    assert_eq!(code(source), "E305");
}

#[test]
fn remainder_is_int_only() {
    for index in 1..3 {
        assert_eq!(code(&format!("def main = {index}.5 % 2.0")), "E305");
    }
}

#[test]
fn name_cycles_precede_type_errors() {
    let source = "def a = b\ndef b = a\ndef main = 1 + true";
    assert_eq!(code(source), "E207");
    let source = "def a x => b x\ndef b x => a x\ndef main = 1 + true";
    assert_eq!(code(source), "E208");
}

#[test]
fn later_coverage_errors_precede_earlier_redundancy() {
    let source = "def a = 1 as x -> x\ndef z x => if | x > 0 => 2\ndef main = a + z 1";
    assert_eq!(code(source), "E401");
}

#[test]
fn redundancy_uses_source_order_across_definitions() {
    let first = "def a = 1 as x -> x";
    let second = "def z = 2 as unused -> 3";
    for (left, right, expected) in [(first, second, "E501"), (second, first, "E505")] {
        let source = format!("{left}\n{right}\ndef main = a + z");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some(expected));
        assert_eq!(error.line, 1);
    }
}

#[test]
fn non_exhaustive_branches_point_to_if_not_the_scrutinee() {
    for padding in 0..20 {
        for (input, arms) in [("true", "true => 1"), ("Some(1)", "Some(x) => x")] {
            let prefix = format!("def main = {}", " ".repeat(padding));
            let source = format!("{prefix}if   {input} | {arms}");
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E402"));
            assert_eq!((error.line, error.col), (1, prefix.len() + 1));
        }
    }
}

#[test]
fn arm_diagnostics_point_to_pipe_with_arbitrary_spacing() {
    for padding in 0..12 {
        for (prefix, suffix, code) in [
            ("def main = if true | true => 1 | false => 2 ", "0", "E403"),
            ("def main = if 1 | 1 => 10 ", "1 => 20 | 0", "E405"),
            ("def main = if | 1 > 0 => 1 ", "1 > 0 => 2 | 0", "E406"),
        ] {
            let prefix = format!("{prefix}{}", " ".repeat(padding));
            let source = format!("{prefix}| {}{suffix}", " ".repeat(padding + 2));
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some(code), "{source}");
            assert_eq!((error.line, error.col), (1, prefix.len() + 1), "{source}");
        }
    }
}

#[test]
fn all_type_references_resolve_before_expression_type_errors() {
    for declaration in [
        "type Box = { value: Missing }",
        "type Box = | Pack(Missing)",
        "def f (value: Missing) => value",
        "def f value: Missing => value",
        "def f = fn(value: Missing => value)",
    ] {
        let source = format!("def broken = 1 + true\n{declaration}\ndef main = broken");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E204"), "{source}");
        assert_eq!(
            (error.line, error.col),
            (2, declaration.find("Missing").unwrap() + 1)
        );
    }
}

#[test]
fn recursive_type_errors_point_to_the_changed_argument() {
    for padding in 0..20 {
        for (prefix, suffix) in [
            ("type T Tree = | Leaf(T) | Next(", "T List Tree)"),
            ("type (A, B) Pair = | End | Next((", "B, A) Pair)"),
        ] {
            let prefix = format!("{prefix}{}", " ".repeat(padding));
            let source = format!("{prefix}{suffix}\ndef main = 1");
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E302"));
            assert_eq!((error.line, error.col), (1, prefix.len() + 1));
        }
    }
}

#[test]
fn recursive_type_errors_use_source_order_across_declarations() {
    let first = "type T A = | WrapA(T List A)";
    let second = "type T Z = | WrapZ(T Option Z)";
    for (left, right) in [(first, second), (second, first)] {
        for padding in 0..12 {
            let source = format!("{}{left}\n{right}\ndef main = 1", " ".repeat(padding));
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E302"));
            let changed = if left == first { "T List" } else { "T Option" };
            assert_eq!(
                (error.line, error.col),
                (1, padding + left.find(changed).unwrap() + 1)
            );
        }
    }
}

#[test]
fn unresolved_values_and_intrinsics_precede_type_errors() {
    for (body, expected) in [
        ("missing 1", "E204"),
        ("Unknown.value 1", "E204"),
        ("if 1 | Unknown(x) => x | 0", "E204"),
        ("__string_length 1", "E202"),
        ("__string_length", "E203"),
    ] {
        let source = format!("def broken = 1 + true\ndef other = {body}\ndef main = broken");
        assert_eq!(code(&source), expected, "{source}");
    }
}

#[test]
fn recursion_and_unbound_names_share_source_order() {
    let cycle = "def a x => b x\ndef b x => a x";
    let unknown = "def unknown = missing";
    for (prefix, expected) in [
        (format!("{cycle}\n{unknown}"), "E208"),
        (format!("{unknown}\n{cycle}"), "E204"),
    ] {
        let error = check_source(&format!("{prefix}\ndef main = a unknown"), SOURCE_PATH)
            .err()
            .unwrap();
        assert_eq!(error.code.as_deref(), Some(expected));
        assert_eq!(error.line, 1);
    }
}

#[test]
fn mixed_recursion_errors_use_the_first_declaration() {
    for (prefix, expected) in [
        ("def ping x => seed\ndef seed = ping 0", "E208"),
        ("def seed = ping 0\ndef ping x => seed", "E207"),
    ] {
        let error = check_source(&format!("{prefix}\ndef main = seed"), SOURCE_PATH)
            .err()
            .unwrap();
        assert_eq!(error.code.as_deref(), Some(expected));
        assert_eq!((error.line, error.col), (1, 1));
    }
}

#[test]
fn type_errors_are_ordered_independently_of_definition_dependencies() {
    for padding in 0..20 {
        let source = format!(
            "def first = {}true + later\ndef later = 1 + \"bad\"\ndef main = first",
            " ".repeat(padding)
        );
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E305"));
        assert_eq!((error.line, error.col), (1, 13 + padding));
    }
}

#[test]
fn broken_dependencies_do_not_create_earlier_type_errors() {
    for use_site in [
        "later + 1",
        "later 1",
        "later.value",
        "later as x -> Some(x + 1)",
    ] {
        let source = format!("def first = {use_site}\ndef later = 1 + \"bad\"\ndef main = first");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E305"), "{source}: {error}");
        assert_eq!(error.line, 2, "{source}: {error}");
    }
}

#[test]
fn invalid_function_bodies_preserve_known_call_arity() {
    let source = "def first = later 1 2\ndef later x => x + true\ndef main = first";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E301"));
    assert_eq!((error.line, error.col), (1, 13));
}

#[test]
fn failed_patterns_do_not_leak_shadowing_bindings() {
    let source = "type A = { good: Int }\ntype B = { bad: Int }\ntype (X, Y) Pair = | Pair(X, Y)\ndef main = { good = 1 } as x -> (Pair({ bad = 1 }, 2) as Pair(x, Some(n)) -> n) + x.good";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn definition_type_errors_do_not_stop_source_order_checks() {
    for padding in 0..20 {
        for definition in [
            "def rec later x => later [x]",
            "def later x: String => x + 1",
        ] {
            let source = format!(
                "def first = {}true + later 1\n{definition}\ndef main = first",
                " ".repeat(padding)
            );
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E305"), "{source}: {error}");
            assert_eq!(
                (error.line, error.col),
                (1, 13 + padding),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn recursive_type_failures_keep_call_arity_without_cascades() {
    for (use_site, expected, line) in [("later 1", "E307", 2), ("later 1 2", "E301", 1)] {
        let source =
            format!("def first = {use_site}\ndef rec later x => later [x]\ndef main = first");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some(expected), "{source}: {error}");
        assert_eq!(error.line, line, "{source}: {error}");
    }
}

#[test]
fn invalid_module_bodies_do_not_fake_polymorphism_errors() {
    for (definition, expected) in [
        ("def f x => x + true", "E305"),
        ("def f x => if x => fn(y => y + y) x | false", "E305"),
        ("def rec f x => f [x]", "E307"),
    ] {
        let source =
            format!("def main = M.f 1\nmodule S def A f(x: A): A\n/module\nimplement S as M {definition} /implement");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some(expected), "{source}: {error}");
    }
}

#[test]
fn invalid_module_bodies_retain_explicit_signature_mismatches() {
    for definition in [
        "def f (x: Int) => x + true",
        "def f x y => x + y + true",
        "def f = fn(x, y => x + y + true)",
    ] {
        let source =
            format!("def main = M.f 1\nmodule S def A f(x: A): A\n/module\nimplement S as M {definition} /implement");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E602"), "{source}: {error}");
        assert_eq!((error.line, error.col), (4, 1));
    }
}

#[test]
fn deferred_numeric_errors_use_source_order_not_record_schema_order() {
    let prefix = "type R = { a: Bool; b: Bool }\ndef main = { b = fn(x => ";
    let source = format!("{prefix}x + x) true; a = fn(y => y + y) true }}");
    let error = check_source(&source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E305"));
    let line = prefix.lines().last().unwrap();
    assert_eq!((error.line, error.col), (2, line.len() + 1), "{error}");
}

#[test]
fn invalid_recursive_groups_do_not_leave_unresolved_projections() {
    let source = "type A = { value: Int }\ntype B = { value: Bool }\ndef rec later x => x.value as ignored -> later [x]\ndef main = later 1";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E307"), "{error}");
}

#[test]
fn non_record_projection_is_a_positioned_type_error() {
    for prefix in [
        "def later x: String => x + 1",
        "def later (x: Int): Int => x + 1",
    ] {
        let source = format!("def first = later.value\n{prefix}\ndef main = first");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
        assert_eq!((error.line, error.col), (1, 13));
    }
}

#[test]
fn deferred_non_record_projection_keeps_the_receiver_position() {
    let prefix = "def get x => ";
    let source = format!(
        "type A = {{ value: Int }}\ntype B = {{ value: Bool }}\n{prefix}x.value\ndef main = get 1"
    );
    let error = check_source(&source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert_eq!((error.line, error.col), (3, prefix.len() + 1));
}

#[test]
fn module_type_errors_do_not_stop_earlier_type_checks() {
    for padding in 0..20 {
        for implementation in ["def run x => true", "def other x => x"] {
            let source = format!("def first = {}true + M.run 1\n\n\ndef main = first\nmodule S def run(x: Int): Int\n/module\nimplement S as M {implementation} /implement", " ".repeat(padding));
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E305"), "{source}: {error}");
            assert_eq!((error.line, error.col), (1, 13 + padding));
        }
    }
}

#[test]
fn failed_modules_retain_public_contracts_without_fake_names() {
    for (signature, implementation, use_site) in [
        ("def run(x: Int): Int", "def run x => true", "M.run 1"),
        ("type T def make(x: Int): T", "def make x => x", "M.make 1"),
        ("def A id(x: A): A", "def id x => x + 1", "M.id true"),
    ] {
        let source = format!("def first = {use_site}\n\n\ndef main = first\nmodule S {signature}\n/module\nimplement S as M {implementation}\n/implement");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E602"), "{source}: {error}");
        assert_eq!((error.line, error.col), (7, 1));
    }
}

#[test]
fn failed_modules_roll_back_shared_inference_and_pending_projections() {
    let source = "def main = M.run 1 + get { value = 1 }\ntype A = { value: Int }\ntype B = { value: Bool; extra: Int }\ndef get x => x.value\nmodule S def run(x: Int): Int /module\nimplement S as M def run x => get { value = x > 0; extra = x } /implement";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
    assert_eq!((error.line, error.col), (6, 1));

    let source = "type A = { value: Int }\ntype B = { value: Bool }\n\n\ndef main = M.run 1\nmodule S def run(x: Int): Int\n/module\nimplement S as M def run x y => x.value as ignored -> y /implement";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
    assert_eq!((error.line, error.col), (8, 1));
}

#[test]
fn module_errors_follow_source_order_even_with_outer_dependencies() {
    let source = "def helper x => x > 0\ndef main = M.run 1 + N.run 1\nimplement S as M def run x => helper x /implement\nimplement S as N def run x => true /implement\nmodule S def run(x: Int): Int /module";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
    assert_eq!((error.line, error.col), (3, 1));
}

#[test]
fn recursive_type_errors_do_not_hide_earlier_independent_type_errors() {
    let declaration = "type T Tree = | Leaf(Int) | Next(T List Tree)";
    let first = "def first = true + 1";
    let source = format!("{first}\n{declaration}\ndef main = first");
    let error = check_source(&source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert_eq!((error.line, error.col), (1, 13));

    let source = format!("{declaration}\n{first}\ndef main = first");
    let error = check_source(&source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E302"), "{error}");
    assert_eq!(
        (error.line, error.col),
        (1, declaration.find("T List").unwrap() + 1)
    );
}

#[test]
fn recursive_type_recovery_preserves_known_payloads_and_breaks_growing_cycles() {
    for declaration in [
        "type T Tree = | Leaf(Int) | Next(T List Tree)",
        "type T Tree = | Leaf(Int) | Next(T List Branch)\ntype T Branch = | Back(T Tree)",
    ] {
        let source = format!("def first = Leaf(true)\n{declaration}\ndef main = first");
        assert_eq!(code(&source), "E305", "{source}");

        let source = format!("{declaration}\ndef main = Leaf(1) == Leaf(2)");
        assert_eq!(code(&source), "E302", "{source}");
    }
}

#[test]
fn polymorphic_recursion_points_to_the_recursive_application() {
    for padding in 0..20 {
        for (prefix, call) in [
            ("def rec poly x => ", "poly [x]"),
            ("def rec poly x => ", "[x] |> poly"),
            ("def rec poly x =>\n    ", "poly [x]"),
        ] {
            let prefix = format!("{prefix}{}", " ".repeat(padding));
            let source = format!("{prefix}{call}\ndef main = poly 1");
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E307"), "{error}");
            assert_eq!(
                (error.line, error.col),
                (
                    prefix.lines().count(),
                    prefix.lines().last().unwrap().len() + 1
                ),
                "{error}"
            );
        }
    }
}

#[test]
fn later_type_changing_recursive_calls_are_not_general_occurs_checks() {
    let prefix = "def rec poly x => if true => poly x | ";
    let source = format!("{prefix}poly [x]\ndef main = poly 1");
    let error = check_source(&source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E307"), "{error}");
    assert_eq!((error.line, error.col), (1, prefix.len() + 1), "{error}");
}

#[test]
fn recursive_call_anchors_follow_bindings_and_mutual_calls() {
    for (prefix, call, suffix) in [
        (
            "def rec poly x => poly as invoke -> ",
            "invoke [x]",
            "\ndef main = poly 1",
        ),
        (
            "def rec f x => ",
            "g [x]",
            "\ndef rec g y => f y\ndef main = f 1",
        ),
        (
            "def rec poly x => if true => fn(poly => poly 1) fn(n => n) | ",
            "poly [x]",
            "\ndef main = poly 1",
        ),
        (
            "def rec poly x y => ",
            "poly y [x]",
            "\ndef main = poly 1 1",
        ),
    ] {
        let source = format!("{prefix}{call}{suffix}");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E307"), "{error}");
        assert_eq!((error.line, error.col), (1, prefix.len() + 1), "{error}");
    }
}

#[test]
fn indirect_polymorphic_recursion_points_to_its_definition() {
    for index in 0..3 {
        let name = format!("poly{index}");
        for (helper, body) in [
            ("", format!("fn(invoke => invoke [x]) {name}")),
            (
                "",
                format!("fn(invoke => invoke [x]) as call -> call {name}"),
            ),
            ("def apply f x => f x\n", format!("apply {name} [x]")),
            ("def identity x => x\n", format!("(identity {name}) [x]")),
        ] {
            let prefix = format!("{}{}{}", "\n".repeat(index), helper, " ".repeat(index));
            let source = format!("{prefix}def rec {name} x => {body}\ndef main = {name} 1");
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E307"), "{source}: {error}");
            assert_eq!(
                (error.line, error.col),
                (index + helper.lines().count() + 1, index + 1),
                "{source}: {error}"
            );
        }
    }
    assert_eq!(
        code("def main = fn(invoke => invoke invoke) fn(x => x)"),
        "E306"
    );
    check_source(
        "def rec poly x => fn(poly => poly [x]) fn(values => values)\ndef main = poly 1",
        SOURCE_PATH,
    )
    .unwrap();
}

#[test]
fn function_equality_points_to_the_operator() {
    for padding in 0..20 {
        for operator in ["==", "!="] {
            let prefix = format!(
                "def identity x => x\ndef main = identity{}",
                " ".repeat(padding)
            );
            let source = format!("{prefix}{operator} identity");
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E703"), "{error}");
            assert_eq!(
                (error.line, error.col),
                (2, prefix.rsplit('\n').next().unwrap().len() + 1),
                "{error}"
            );
        }
    }
}

#[test]
fn polymorphic_comparison_constraints_keep_their_source_anchor() {
    for (prefix, expression, suffix, code) in [
        ("def equal x y => x ", "== y", "\ndef alias = equal\ndef identity x => x\ndef main = alias identity identity", "E703"),
        ("def cmp x y => ", "compare x y", "\ndef alias = cmp\ndef identity x => x\ndef main = alias identity identity", "E704"),
        ("def identity x => x\ndef main = M.equal identity identity\nmodule S def T equal(x: T, y: T): Bool\n/module\nimplement S as M def equal x y => x", "== y", "\n/implement", "E703"),
        ("def identity x => x\ndef main = (identity ", "== identity)", " == true", "E703"),
    ] {
        let source = format!("{prefix}{expression}{suffix}");
        let error = check_source(&source, SOURCE_PATH).err().unwrap_or_else(|| panic!("accepted: {source}"));
        assert_eq!(error.code.as_deref(), Some(code), "{error}");
        assert_eq!((error.line, error.col), (prefix.lines().count(), prefix.rsplit('\n').next().unwrap().len() + 1), "{error}");
    }
    for source in [
        "def equal x y => x == y\ndef alias = equal\ndef main = alias 1 2",
        "def cmp x y => compare x y\ndef alias = cmp\ndef main = alias 1 2",
        "def main = M.equal 1 2\nmodule S def T equal(x: T, y: T): Bool /module\nimplement S as M def equal x y => x == y /implement",
    ] {
        check_source(source, SOURCE_PATH).unwrap_or_else(|error| panic!("{source}: {error}"));
    }
}

#[test]
fn field_name_errors_are_ordered_with_unresolved_value_names() {
    for field_first in [false, true] {
        let field = "def field (x: R) => x.missing";
        let value = "def value = no_such_value";
        let (first, second, anchor) = if field_first {
            (field, value, "missing")
        } else {
            (value, field, "no_such_value")
        };
        let source = format!(
            "type R = {{ known: Int }}\n{first}\n{second}\ndef main = field {{ known = 1 }}"
        );
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
        assert_eq!(
            (error.line, error.col),
            (2, first.find(anchor).unwrap() + 1),
            "{error}: {source}"
        );
    }
}

#[test]
fn independent_fields_survive_unresolved_payload_type_names() {
    for broken in [
        "type Broken = { value: Missing }",
        "type Broken = | Bad(Missing)",
    ] {
        let field = "def field (x: R) => x.missing";
        let source = format!(
            "type R = {{ known: Int }}\n{field}\n{broken}\ndef main = field {{ known = 1 }}"
        );
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
        assert_eq!(
            (error.line, error.col),
            (2, field.find("missing").unwrap() + 1),
            "{error}"
        );
    }
}

#[test]
fn independent_fields_precede_later_duplicate_declarations() {
    let field = "def field (x: R) => x.missing";
    for duplicates in [
        "def dup = 1\ndef dup = 2",
        "type Dup = | One\ntype Dup = | Two",
        "type First = | Same\ntype Second = | Same",
    ] {
        let source = format!(
            "type R = {{ known: Int }}\n{field}\n{duplicates}\ndef main = field {{ known = 1 }}"
        );
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
        assert_eq!(
            (error.line, error.col),
            (2, field.find("missing").unwrap() + 1),
            "{error}"
        );
    }
}

#[test]
fn ambiguous_declarations_do_not_create_speculative_field_errors() {
    for source in [
        "type R = { known: Int }\ndef early = dup.missing\ndef dup = 1\ndef dup = { known = 1 }\ndef main = early",
        "def early (x: Dup) => x.missing\ntype Dup = { known: Int }\ntype Dup = { missing: Int }\ndef main = early",
        "type R = { known: Int }\ndef early = Same({ known = 1 }) as Same(x) -> x.missing\ntype First = | Same(R)\ntype Second = | Same(Int)\ndef main = early",
    ] {
        let error = check_source(source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E205"), "{error}");
    }
}

#[test]
fn duplicate_recovery_respects_module_scopes() {
    let cases = [
        "def main = 1\nmodule S\n/module\nimplement S as M\n/implement\n type R = { known: Int }\n def field (x: R) => x.missing\n def dup = 1\n def dup = 2",

        "type R = { known: Int }\ndef field (x: R) => x.missing + M.get 1\n\n\ndef main = field { known = 1 }\nmodule S def get(x: Int): Int /module\nimplement S as M type Private = | First type Private = | Second def get x => x /implement",
    ];
    for source in cases {
        let before = &source[..source.find("missing").unwrap()];
        let error = check_source(source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
        assert_eq!(
            (error.line, error.col),
            (
                before.lines().count(),
                before.rsplit('\n').next().unwrap().len() + 1
            ),
            "{error}"
        );
    }
}

#[test]
fn outer_duplicates_precede_implementation_local_field_errors() {
    let source = "type R = | First\ntype R = | Second\ndef main = 1\nmodule S\n/module\nimplement S as M type Local = { known: Int } def field (x: Local) => x.missing /implement";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E205"));
    assert_eq!((error.line, error.col), (2, 6));
}

#[test]
fn duplicate_bodies_still_report_independent_field_errors() {
    for first in [true, false] {
        let broken = "def dup (x: R) => x.missing";
        let other = "def dup x => x";
        let (a, b) = if first {
            (broken, other)
        } else {
            (other, broken)
        };
        let source = format!("type R = {{ known: Int }}\n{a}\n{b}\ndef main = dup {{ known = 1 }}");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        let (code, line, col) = if first {
            ("E204", 2, broken.find("missing").unwrap() + 1)
        } else {
            ("E205", 3, broken.find("dup").unwrap() + 1)
        };
        assert_eq!(error.code.as_deref(), Some(code), "{error}");
        assert_eq!((error.line, error.col), (line, col), "{error}");
    }
}

#[test]
fn ambiguous_module_interfaces_do_not_choose_a_return_type() {
    let prefix = "type A = { ok: Int }\ntype B = { bad: Int }\ndef early = (M.make 1).ok\n";
    for declarations in [
        "module S def make(x: Int): A /module\nmodule S def make(x: Int): B /module\nimplement S as M def make x => { ok = x } /implement",
        "module SA def make(x: Int): A /module\nmodule SB def make(x: Int): B /module\nimplement SA as M def make x => { ok = x } /implement\nimplement SB as M def make x => { bad = x } /implement",
        "module S def make(x: Int): A def make(x: Int): B /module\nimplement S as M def make x => { ok = x } /implement",
    ] {
        let source = format!("{prefix}def main = early\n{declarations}");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E205"), "{error}");
    }
}

#[test]
fn ambiguous_interfaces_keep_possible_member_and_type_names() {
    for source in [
        "def main = M.first 1\nmodule S def first(x: Int): Int /module\nmodule S def second(x: Int): Int /module\nimplement S as M def first x => x /implement",
        "type Box = | Wrap(M.T)\n\n\n\ndef main = 1\nmodule S type T /module\nmodule S /module\nimplement S as M type T = Int /implement",
    ] {
        let error = check_source(source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E205"), "{error}");
    }
}

#[test]
fn ambiguous_module_bodies_retain_independent_diagnostics() {
    for suffix in [
        "module S\n/module\nmodule S\n/module",
        "module S\n/module\nimplement S as M\n/implement",
    ] {
        let prefix =
            "def main = 1\nimplement S as M\n/implement\n type R = { known: Int }\n def bad (x: R) => x.";
        let source = format!("{prefix}missing\n{suffix}");
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
        assert_eq!(
            (error.line, error.col),
            (
                prefix.lines().count(),
                prefix.rsplit('\n').next().unwrap().len() + 1
            ),
            "{error}"
        );
    }
}

#[test]
fn ambiguous_interfaces_still_reject_names_absent_from_every_candidate() {
    let source = "def main = M.missing 1\nmodule SA def first(x: Int): Int /module\nmodule SB def second(x: Int): Int /module\nimplement SA as M def first x => x /implement\nimplement SB as M def second x => x /implement";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
    assert_eq!(
        (error.line, error.col),
        (1, source.find("missing").unwrap() + 1)
    );
}
