use shin::lexer::{lex_full, TokKind};
use shin::parser::parse_source;

#[test]
fn literal_diagnostics_and_anchors() {
    for padding in 0..8 {
        let prefix = format!("{}def main = ", " ".repeat(padding));
        for (literal, code, offset) in [
            ("\"bad\\q\"", "E110", 4),
            ("''", "E111", 0),
            ("'ab'", "E111", 0),
            ("'\\q'", "E111", 0),
            ("99999999999999999999", "E112", 0),
        ] {
            let error = parse_source(&format!("{prefix}{literal}"), "generated").unwrap_err();
            assert_eq!(error.code.as_deref(), Some(code), "{literal}");
            assert_eq!((error.line, error.col), (1, prefix.len() + offset + 1));
        }
    }
}

#[test]
fn char_escapes_are_decoded() {
    for (escape, value) in [("n", '\n'), ("t", '\t'), ("\\", '\\'), ("'", '\'')] {
        let tokens = lex_full(&format!("'\\{escape}'")).unwrap();
        assert_eq!(tokens[0].kind, TokKind::Char(value));
    }
}

#[test]
fn generated_syntax_diagnostics() {
    for index in 0..20 {
        let constructor = format!("Case{index}");
        for (source, code) in [
            (format!("def main = {index} < {} < {}", index + 1, index + 2), "E100"),
            (format!("type Choice{index} = {constructor} | Other{index}"), "E108"),
            (format!("def f{index} => {index}"), "E109"),
            (format!("def main = {constructor} {index}"), "E106"),
            (format!("def main = {index} |> {constructor}"), "E106"),
            (format!("module M{index} = struct def f x => x end"), "E113"),
        ] {
            let error = parse_source(&source, "generated").unwrap_err();
            assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
        }
    }
}
