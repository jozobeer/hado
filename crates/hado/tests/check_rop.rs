use hado::{check_source, types::Ty};

#[test]
fn propagation_preserves_container_and_error_type() {
    let source = "def main = let? x = Some(1) -> Some(x + 2)";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt("Option".into(), vec![Ty::Int])
    );
    let source = "def main = do? (1 / 2) -> Ok(true)";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt(
            "Result".into(),
            vec![Ty::Bool, Ty::Adt("DivisionError".into(), vec![])]
        )
    );
    for source in [
        "def main = let? x = Some(1) -> Ok(x)",
        "def main = do? (1 / 2) -> Error(true)",
        "def main = let? x = Some(1) -> x",
    ] {
        assert_eq!(
            check_source(source, "t").err().unwrap().code.as_deref(),
            Some("E305"),
            "{source}"
        );
    }
}

#[test]
fn propagation_requires_option_or_result() {
    assert_eq!(
        check_source("def main = let? x = 1 -> x", "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E303")
    );
    assert!(check_source("def main = do? true -> Some(1)", "t").is_err());
}
