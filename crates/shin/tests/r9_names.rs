use shin::{check_source, parser::Diagnostic};

fn error(source: &str) -> Diagnostic {
    check_source(source, "generated-names.shin").err().expect("expected a diagnostic")
}

#[test]
fn duplicate_definitions_report_the_second_name() {
    let mut state = 0x31ad_u64;
    for padding in 0..20 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let name = format!("value{state}");
        let indent = " ".repeat(padding);
        let source = format!("def {name} = 1\n{indent}def {name} = 2\ndef main = {name}");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.code.as_deref(), Some("E205"));
        assert_eq!((diagnostic.line, diagnostic.col), (2, padding + 5));
    }
}

#[test]
fn entry_form_is_checked_before_types() {
    for (source, position) in [
        ("def f x => x + true", (1, 1)),
        ("\n\ndef main x => x + true", (3, 1)),
        ("def main | Some(x) => x | 0", (1, 1)),
    ] {
        let diagnostic = error(source);
        assert_eq!(diagnostic.code.as_deref(), Some("E206"));
        assert_eq!((diagnostic.line, diagnostic.col), position);
    }
}

#[test]
fn diagnostic_numbers_follow_the_current_specification() {
    for (source, code) in [
        ("def main = __string_length \"hi\"", "E202"),
        ("def main = __string_length", "E203"),
        ("def main = missing", "E204"),
        ("def rec poly x => poly [x]\ndef main = poly 1", "E307"),
        ("def main = Nil", "E701"),
        ("def main = fn x => x + 1", "E702"),
        ("def f x => x + 1\ndef main = f == f", "E703"),
        ("def f x => x + 1\ndef main = compare f f", "E704"),
    ] {
        assert_eq!(error(source).code.as_deref(), Some(code), "{source}");
    }
}
