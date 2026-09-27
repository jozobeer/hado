use hado::{check_source, types::Ty};

#[test]
fn propagation_preserves_container_and_error_type() {
    let source = "def main = Some(1) as x? ->\n  Some(x + 2)";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt("Option".into(), vec![Ty::Int])
    );
    let source = "def main = 1 / 2 ->\n  true";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
    for source in [
        "def main = Some(1) as x? ->\n  x + 1",
        "def main = Some(1) as x? ->\n  x",
    ] {
        assert_eq!(
            check_source(source, "t").err().unwrap().code.as_deref(),
            Some("E305"),
            "{source}"
        );
    }
}

#[test]
fn do_of_a_pure_value_is_rejected() {
    assert_eq!(
        check_source("def main = true ->\n  Some(1)", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E303")
    );
}
