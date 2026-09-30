use hado::{check_source, types::Ty};

#[test]
fn maybe_answers_the_absence_as_an_option() {
    let source =
        "def f (x: Int): Int {Absent} => if x > 0 => x | absent\ndef main = with Maybe -> f 1";
    assert_eq!(
        check_source(source, "t").unwrap().main,
        Ty::Adt("Option".into(), vec![Ty::Int])
    );
    let source = "def main = 1 / 2 ->\n  true";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
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
