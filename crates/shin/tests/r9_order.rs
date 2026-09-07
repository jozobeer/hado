use shin::check_source;

const SOURCE_PATH: &str = "generated-order.shin";

fn code(source: &str) -> String {
    match check_source(source, SOURCE_PATH) {
        Err(error) => error.code.clone().unwrap_or_else(|| panic!("uncoded diagnostic: {error:?}")),
        Ok(_) => panic!("expected a diagnostic for {source}"),
    }
}

#[test]
fn types_precede_redundancy() {
    for index in 0..20 {
        let source = format!("def main = let x = {index} + \"bad\" -> x");
        assert_eq!(code(&source), "E305");
    }
}

#[test]
fn redundancy_precedes_monomorphization() {
    assert_eq!(code("def main = let unused = 1 -> Nil"), "E505");
}

#[test]
fn later_type_errors_precede_earlier_coverage_errors() {
    let source = "def a x => if x | Some(v) => v\ndef z = 1 + true\ndef main = a (Some(z))";
    assert_eq!(code(source), "E305");
}

#[test]
fn remainder_is_int_only() {
    for index in 1..20 {
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
    let source = "def a = let x = 1 -> x\ndef z x => if | x > 0 => 2\ndef main = a + z 1";
    assert_eq!(code(source), "E401");
}

#[test]
fn redundancy_uses_source_order_across_definitions() {
    let first = "def a = let x = 1 -> x";
    let second = "def z = let unused = 2 -> 3";
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
            ("def f | Ok(x) => Ok(x + 1) ", "Error(e) => Error(e)\ndef main = f (1 / 1)", "E508"),
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
    for index in 0..20 {
        for declaration in [
            format!("type Box{index} = {{ value: Missing{index} }}"),
            format!("type Box{index} = | Pack{index}(Missing{index})"),
            format!("def f (value: Missing{index}) => value"),
            format!("def f value: Missing{index} => value"),
            format!("def f = fn (value: Missing{index}) => value"),
        ] {
            let source = format!("def broken = 1 + true\n{declaration}\ndef main = broken");
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E204"), "{source}");
            assert_eq!((error.line, error.col), (2, declaration.find(&format!("Missing{index}")).unwrap() + 1));
        }
    }
}

#[test]
fn recursive_type_errors_point_to_the_changed_argument() {
    for padding in 0..20 {
        for (prefix, suffix) in [
            ("type Tree<T> = | Leaf(T) | Next(Tree<", "List<T>>)"),
            ("type Pair<A, B> = | End | Next(Pair<", "B, A>)"),
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
    let first = "type A<T> = | WrapA(A<List<T>>)";
    let second = "type Z<T> = | WrapZ(Z<Option<T>>)";
    for (left, right) in [(first, second), (second, first)] {
        for padding in 0..12 {
            let source = format!("{}{left}\n{right}\ndef main = 1", " ".repeat(padding));
            let error = check_source(&source, SOURCE_PATH).err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E302"));
            let changed = if left == first { "List" } else { "Option" };
            assert_eq!((error.line, error.col), (1, padding + left.find(changed).unwrap() + 1));
        }
    }
}

#[test]
fn unresolved_values_and_intrinsics_precede_type_errors() {
    for index in 0..20 {
        for (body, expected) in [
            (format!("missing{index} {index}"), "E204"),
            (format!("Unknown{index}.value {index}"), "E204"),
            (format!("if {index} | Unknown{index}(x) => x | 0"), "E204"),
            ("__string_length 1".into(), "E202"),
            ("__string_length".into(), "E203"),
        ] {
            let source = format!("def broken = 1 + true\ndef other = {body}\ndef main = broken");
            assert_eq!(code(&source), expected, "{source}");
        }
    }
}

#[test]
fn recursion_and_unbound_names_share_source_order() {
    let cycle = "def a x => b x\ndef b x => a x";
    let unknown = "def unknown = missing";
    for (prefix, expected) in [(format!("{cycle}\n{unknown}"), "E208"), (format!("{unknown}\n{cycle}"), "E204")] {
        let error = check_source(&format!("{prefix}\ndef main = a unknown"), SOURCE_PATH).err().unwrap();
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
        let error = check_source(&format!("{prefix}\ndef main = seed"), SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some(expected));
        assert_eq!((error.line, error.col), (1, 1));
    }
}

#[test]
fn type_errors_are_ordered_independently_of_definition_dependencies() {
    for padding in 0..20 {
        let source = format!("def first = {}true + later\ndef later = 1 + \"bad\"\ndef main = first", " ".repeat(padding));
        let error = check_source(&source, SOURCE_PATH).err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E305"));
        assert_eq!((error.line, error.col), (1, 13 + padding));
    }
}

#[test]
fn broken_dependencies_do_not_create_earlier_type_errors() {
    for use_site in ["later + 1", "later 1", "later.value", "let? x = later -> Some(x + 1)"] {
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
    let source = "type A = { good: Int }\ntype B = { bad: Int }\ntype Pair<X, Y> = | Pair(X, Y)\ndef main = let x = { good = 1 } -> (let Pair(x, Some(n)) = Pair({ bad = 1 }, 2) -> n) + x.good";
    let error = check_source(source, SOURCE_PATH).err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}
