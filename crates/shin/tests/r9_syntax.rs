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

#[test]
fn whitespace_does_not_change_calls() {
    for index in 0..20 {
        for callee in [format!("f{index}"), format!("Case{index}")] {
            let baseline = parse_source(&format!("def main = {callee}({index})"), "generated").unwrap();
            for gap in [" ", "\n", " -- gap\n "] {
                let perturbed = parse_source(&format!("def main = {callee}{gap}({index})"), "generated").unwrap();
                assert_eq!(baseline.main(), perturbed.main());
            }
        }
    }
}

#[test]
fn arm_functions_and_negative_patterns_parse() {
    for index in 1..20 {
        for source in [
            format!("def f{index} | -{index} => 1 | 0 def main = f{index} {index}"),
            format!("def main = fn | -{index}.5 => true | false"),
            format!("def rec f{index} | None => 0 | Some(x) => f{index} None"),
            format!("def f{index} | Some (x) => x | None => {index}"),
        ] {
            assert!(parse_source(&source, "generated").is_ok(), "{source}");
        }
    }
}

#[test]
fn canonical_unit_payload_keeps_its_distinction() {
    let canonical = parse_source("def main = Ok()", "generated").unwrap();
    for source in ["def main = Ok ()", "def main = Ok ( )"] {
        assert_eq!(canonical.main(), parse_source(source, "generated").unwrap().main());
    }
    let nested = parse_source("def main = Ok(())", "generated").unwrap();
    assert_ne!(canonical.main(), nested.main());
}

#[test]
fn type_definition_contexts_are_distinct() {
    for index in 0..20 {
        for source in [
            format!("type Alias{index} = Int"),
            format!("type Sum{index} = | Case{index}({{ field: Int }})"),
            format!("module type S{index} = sig type T = Int end"),
            format!("module type S{index} = sig type T<A> end"),
        ] {
            let error = parse_source(&source, "generated").unwrap_err();
            assert_eq!(error.code.as_deref(), Some("E100"), "{source}");
        }
        let source = format!("module type S{index} = sig type T def f(x: T): Int end module M{index} : S{index} = struct type T = Int def f x => x end def main = M{index}.f {index}");
        assert!(parse_source(&source, "generated").is_ok());
    }
}
