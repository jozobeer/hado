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
