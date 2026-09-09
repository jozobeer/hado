use hado::{check_source, types::Ty};

#[test]
fn propagation_preserves_container_and_error_type() {
    let source = "def main = let x? = Some(1) -> Some(x + 2)";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt("Option".into(), vec![Ty::Int])
    );
    let source = "def main = do (1 / 2) -> true";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
    for source in [
        "def main = let x? = Some(1) -> x + 1",
        "def main = let x? = Some(1) -> x",
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
        check_source("def main = let x? = 1 -> x", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E303")
    );
    assert_eq!(
        check_source("def main = do true -> Some(1)", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E303")
    );
}
