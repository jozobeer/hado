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
