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
