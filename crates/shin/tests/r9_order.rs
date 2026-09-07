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
