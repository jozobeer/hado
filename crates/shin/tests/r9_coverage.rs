use shin::check_source;

#[test]
fn catch_all_patterns_are_exhaustive_before_redundancy_checks() {
    for index in 0..20 {
        for (arms, argument) in [
            (format!("Some(value) => value + {index} | _ => 0"), "Some(1)"),
            (format!("true => {index} | _ => -1"), "true"),
        ] {
            let source = format!("def choose | {arms}\ndef main = choose ({argument})");
            assert_eq!(check_source(&source, "coverage.shin").err().unwrap().code.as_deref(), Some("E503"), "{source}");
        }
    }
}

#[test]
fn variable_arm_is_not_decomposition_and_reconstruction() {
    for index in 0..20 {
        let name = format!("value{index}");
        let source = format!("def identity | {name} => {name}\ndef main = identity {index}");
        assert_eq!(check_source(&source, "coverage.shin").err().unwrap().code.as_deref(), Some("E512"));
    }
}

#[test]
fn sole_parameter_branch_checks_all_four_conditions() {
    for source in [
        "def f value => if value | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def f value => if value | Some(value) => value + 1 | 0\ndef main = f (Some(1))",
        "def main = (fn value => if value | Some(x) => x + 1 | 0) (Some(1))",
    ] {
        assert_eq!(check_source(source, "coverage.shin").err().unwrap().code.as_deref(), Some("E515"));
    }
    for source in [
        "def f value extra => if value | Some(x) => x + extra | extra\ndef main = f (Some(1)) 2",
        "def f (value: Option<Int>) => if value | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def f value: Int => if value | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def f value => if value | Some(_) => Option.is_some value | false\ndef main = f (Some(1))",
        "def f value => (if value | Some(x) => x | 0) + 1\ndef main = f (Some(1))",
        "def f | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def main = (fn | Some(x) => x + 1 | 0) (Some(1))",
    ] {
        check_source(source, "coverage.shin").unwrap();
    }
}

#[test]
fn let_patterns_must_cover_the_entire_payload_type() {
    for index in 0..20 {
        for (prefix, tail) in [
            ("def main = let ", format!("Some(value) = Some({index}) -> value + 1")),
            ("def main = let? ", format!("Some(value) = Some(Some({index})) -> Some(value + 1)")),
        ] {
            let error = check_source(&format!("{prefix}{tail}"), "coverage.shin").err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E402"));
            assert_eq!((error.line, error.col), (1, prefix.len() + 1));
        }
        let source = format!("type Id = | Id(Int)\ndef main = let Id(value) = Id({index}) -> value + 1");
        check_source(&source, "coverage.shin").unwrap();
    }
}

#[test]
fn do_requires_a_rail_and_records_require_a_candidate() {
    for index in 0..20 {
        let source = format!("def main = do? {index} -> Some(1)");
        assert_eq!(check_source(&source, "coverage.shin").err().unwrap().code.as_deref(), Some("E303"));
        let source = format!("def main = {{ missing{index} = 1 }}");
        assert_eq!(check_source(&source, "coverage.shin").err().unwrap().code.as_deref(), Some("E304"));
    }
}

#[test]
fn unused_bindings_include_parameters_and_nested_patterns() {
    assert_eq!(check_source("def f _ x => x + 1\ndef main = f 1 2", "coverage.shin").err().unwrap().code.as_deref(), Some("E100"));
    for (source, name) in [
        ("def f used unused => used + 1\ndef main = f 1 2", "unused"),
        ("def main = (fn used unused => used + 1) 1 2", "unused"),
        ("def f outer => let outer = 1 -> outer + 2\ndef main = f 1", "outer"),
        ("def f | Some(unused) => 1 | 0\ndef main = f (Some(1))", "unused"),
        ("type Pair = { left: Int; right: Int }\ndef f (pair: Pair) => if pair | { left; right } => left\ndef main = f { left = 1; right = 2 }", "right }"),
    ] {
        let expected_offset = source.find(name).unwrap();
        let before = &source[..expected_offset];
        let expected_line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let expected_col = before.rsplit('\n').next().unwrap().len() + 1;
        let error = check_source(source, "coverage.shin").err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E505"), "{source}");
        assert_eq!((error.line, error.col), (expected_line, expected_col));
    }
    for source in [
        "def f | Some(_) => 1 | 0\ndef main = f (Some(1))",
        "def f x => (fn y => x + y) 1\ndef main = f 1",
        "def f | Some(x) => (fn y => x + y) 1 | 0\ndef main = f (Some(1))",
    ] {
        check_source(source, "coverage.shin").unwrap();
    }
}
