use shin::{check_source, types::Ty};

#[test]
fn top_level_functions_are_polymorphic() {
    let checked = check_source(
        "def id x => x\ndef a = id 12\ndef main = (a > 0) && id true",
        "t",
    )
    .unwrap();
    assert_eq!(checked.main, Ty::Bool);
}

#[test]
fn local_functions_are_monomorphic() {
    let error = check_source(
        "def main = let id = fn x => x -> let a = id 1 -> (a > 0) && id true",
        "t",
    )
    .err()
    .unwrap();
    assert_eq!(error.code.as_deref(), Some("E305"));
}

#[test]
fn application_checks_arity_and_occurs() {
    for (source, code) in [
        ("def f x y => x\ndef main = f 1", "E301"),
        ("def main = fn x => x x", "E306"),
        ("def main = missing", "E205"),
    ] {
        assert_eq!(
            check_source(source, "t").err().unwrap().code.as_deref(),
            Some(code)
        );
    }
}

#[test]
fn parameters_and_local_bindings_have_lexical_scope() {
    let checked = check_source(
        "def x = true\ndef f x => x\ndef main = let y = f 7 -> x && (y > 0)",
        "t",
    )
    .unwrap();
    assert_eq!(checked.main, Ty::Bool);
}

#[test]
fn pipe_supplies_the_first_argument() {
    let source = "def choose (a: Bool, b: Int) => b\ndef main = true |> choose 7";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    let source = "def choose (a: Bool, b: Int) => b\ndef main = 7 |> choose true";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn numeric_functions_respect_constraints_before_defaulting() {
    for (source, expected) in [
        (
            "def twice x => x + x\ndef main = twice 2",
            Ty::Int,
        ),
        (
            "def twice (x: Float) => x + x\ndef main = twice 1.5",
            Ty::Float,
        ),
        ("def main = (fn x => x + x) 1.5", Ty::Float),
    ] {
        assert_eq!(check_source(source, "t").unwrap().main, expected);
    }
    assert_eq!(
        check_source("def main = (fn x => x + x) true", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E305")
    );
}

#[test]
fn division_result_follows_resolved_operand_type() {
    assert_eq!(
        check_source("def main = (fn x => x / x) 2", "t")
            .unwrap()
            .main,
        Ty::Adt(
            "Result".into(),
            vec![Ty::Int, Ty::Adt("DivisionError".into(), vec![])]
        )
    );
    assert_eq!(
        check_source("def main = (fn x => x / x) 2.0", "t")
            .unwrap()
            .main,
        Ty::Float
    );
    assert!(check_source("def main = (fn x => x % x) 2.0", "t").is_err());
}

#[test]
fn function_annotations_check_arguments_and_results() {
    let source = "def apply (f: Fn<Int, Bool>, x: Int) => f x\ndef main = apply (fn n => n == 1) 2";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
    let source = "def apply (f: Fn<(Int, Bool), Int>) => f 1 true\ndef main = apply (fn a b => a)";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    let source = "def apply (f: Fn<Int, Bool>) => f 1\ndef main = apply (fn n => n + 1)";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn compare_is_a_polymorphic_value() {
    let source = "def a = compare 1 2\ndef main = a + compare true false";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    assert_eq!(
        check_source("def main = compare 1 true", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E305")
    );
    let source = "def apply f => f \"a\" \"b\"\ndef main = apply compare";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn deferred_division_results_flow_into_numeric_operators() {
    let source = "def main = -((fn x => x / x) 2.0)";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Float);
    let source = "def main = -((fn x => x / x) 2)";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn function_values_cannot_be_compared_for_equality() {
    let source = "def inc x => x + 1\ndef main = inc == inc";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E704")
    );
}

#[test]
fn equality_rejects_functions_inside_generic_containers() {
    for value in ["Some(inc)", "[inc]"] {
        let source = format!("def inc x => x + 1\ndef main = {value} == {value}");
        assert_eq!(
            check_source(&source, "t").err().unwrap().code.as_deref(),
            Some("E704")
        );
    }
}

#[test]
fn equality_rejects_fixed_function_fields() {
    let source = "type Handler = { run: Fn<Int, Int> }\ndef inc x => x + 1\ndef main = { run = inc } == { run = inc }";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E704")
    );
}

#[test]
fn equality_inspects_recursive_payloads_without_looping() {
    let source = "type Tree = Leaf(Int) | Branch(Tree, Tree)\ndef main = Leaf(1) == Leaf(2)";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
    let source = "type Tree = Leaf(Fn<Int, Int>) | Branch(Tree, Tree)\ndef inc x => x + 1\ndef main = Leaf(inc) == Leaf(inc)";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E704")
    );
}

#[test]
fn phantom_function_parameter_does_not_make_a_value_contain_a_function() {
    let source =
        "type Token<T> = Token\ndef eq (x: Token<Fn<Int, Int>>) => x == x\ndef main = eq Token";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
}

#[test]
fn compare_rejects_function_values_and_containers() {
    for value in ["inc", "[inc]"] {
        let source = format!("def inc x => x + 1\ndef main = compare {value} {value}");
        assert_eq!(
            check_source(&source, "t").err().unwrap().code.as_deref(),
            Some("E705")
        );
    }
}

#[test]
fn compare_constraints_follow_local_aliases_and_pipes() {
    for source in [
        "def inc x => x + 1\ndef main = let cmp = compare -> cmp inc inc",
        "def inc x => x + 1\ndef main = inc |> compare inc",
    ] {
        assert_eq!(
            check_source(source, "t").err().unwrap().code.as_deref(),
            Some("E705")
        );
    }
    let source = "def choose a b => a 1 + b 2\ndef apply compare x => compare x x\ndef inc x => x + 1\ndef main = apply choose inc";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn equality_constraints_wait_for_local_argument_types() {
    for source in [
        "def inc x => x + 1\ndef main = (fn x => x == x) inc",
        "def inc x => x + 1\ndef main = let eq = fn x => x == x -> eq inc",
    ] {
        assert_eq!(
            check_source(source, "t").err().unwrap().code.as_deref(),
            Some("E704")
        );
    }
    assert_eq!(
        check_source("def main = (fn x => x == x) 1", "t")
            .unwrap()
            .main,
        Ty::Bool
    );
}

#[test]
fn polymorphic_definitions_preserve_comparison_constraints() {
    for (definition, code) in [
        ("def eq x => x == x", "E704"),
        ("def eq x => compare x x", "E705"),
        ("def eq = compare", "E705"),
    ] {
        let call = if definition == "def eq = compare" {
            "eq inc inc"
        } else {
            "eq inc"
        };
        let source = format!("{definition}\ndef inc x => x + 1\ndef main = {call}");
        assert_eq!(
            check_source(&source, "t").err().unwrap().code.as_deref(),
            Some(code)
        );
    }
    let source = "def eq x => x == x\ndef a = eq 1\ndef main = a && eq true";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
}

#[test]
fn comparison_constraints_propagate_through_polymorphic_wrappers() {
    for (definition, code) in [
        ("def eq x y => x == y", "E704"),
        ("def eq = compare", "E705"),
    ] {
        let source = format!("{definition}\ndef invoke f x => f x x\ndef wrapped x => invoke eq x\ndef inc x => x + 1\ndef main = wrapped inc");
        assert_eq!(
            check_source(&source, "t").err().unwrap().code.as_deref(),
            Some(code)
        );
    }
}

#[test]
fn main_cannot_return_a_function_or_function_container() {
    for value in ["inc", "Some(inc)"] {
        let source = format!("def inc x => x + 1\ndef main = {value}");
        assert_eq!(
            check_source(&source, "t").err().unwrap().code.as_deref(),
            Some("E703")
        );
    }
}
