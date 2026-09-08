use hado::{check_source, types::Ty};

#[test]
fn self_recursion_uses_a_monomorphic_placeholder() {
    let source = "def rec down n => if n == 0 => true | down (n - 1)\ndef main = down 3";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
    let source = "def rec bad n => if n == 0 => true | bad false\ndef main = bad 3";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn self_reference_requires_rec_but_shadowing_does_not() {
    let source = "def loop x => loop x\ndef main = loop 1";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E201")
    );
    let source = "def f f => f\ndef main = f 1";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Int);
}

#[test]
fn forward_definitions_and_mutual_recursion_are_inferred() {
    let source = "def main = (count > 0) && id true\ndef count = id 1\ndef id x => x";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
    let source = "def main = even 4\ndef rec even n => if n == 0 => true | odd (n - 1)\ndef rec odd n => if n == 0 => false | even (n - 1)";
    assert_eq!(check_source(source, "t").unwrap().main, Ty::Bool);
    let source = "def rec f x => g x\ndef rec g x => if true => f false | x\ndef main = f 1";
    assert_eq!(
        check_source(source, "t").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn mutual_cycles_reject_values_and_require_rec() {
    for (source, code) in [
        (
            "def rec ping x => seed\ndef seed = ping 0\ndef main = seed",
            "E207",
        ),
        ("def a x => b x\ndef b x => a x\ndef main = a 1", "E208"),
    ] {
        assert_eq!(
            hado::check_source(source, "t")
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some(code)
        );
    }
}

#[test]
fn type_changing_recursion_reports_poly_recursion() {
    let source = "def rec poly x => poly [x]\ndef main = poly 1";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E307")
    );
    let source = "def rec bad x => x x\ndef main = bad 1";
    assert_eq!(
        hado::check_source(source, "t")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E306")
    );
}
