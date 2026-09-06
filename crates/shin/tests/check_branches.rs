use shin::{check_source, types::Ty};

#[test]
fn branches_unify_conditions_and_results() {
    for source in [
        "def main = if true => 1 | 2",
        "def main = if | 1 > 2 => 1 | 2 > 3 => 2 | 3",
    ] {
        assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
    }
    for source in [
        "def main = if 1 => 1 | 2",
        "def main = if true => 1 | false",
        "def main = if | 1 => 2 | 3",
        "def main = if | 1 > 2 => 1 | 2 > 3 => true | 3",
    ] {
        assert_eq!(
            check_source(source, "t").err().unwrap().code.as_deref(),
            Some("E305"),
            "{source}"
        );
    }
}
