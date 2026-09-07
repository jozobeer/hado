use shin::lexer::{TokKind, lex_full};
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
    let mut state = 0x106u64;
    for _ in 0..20 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let index = state % 100000;
        let constructor = format!("Case{index}");
        for (source, code) in [
            (
                format!("def main = {index} < {} < {}", index + 1, index + 2),
                "E100",
            ),
            (
                format!("type Choice{index} = {constructor} | Other{index}"),
                "E108",
            ),
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
            let baseline =
                parse_source(&format!("def main = {callee}({index})"), "generated").unwrap();
            for gap in [" ", "\n", " -- gap\n "] {
                let perturbed =
                    parse_source(&format!("def main = {callee}{gap}({index})"), "generated")
                        .unwrap();
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
        assert_eq!(
            canonical.main(),
            parse_source(source, "generated").unwrap().main()
        );
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
        let source = format!(
            "module type S{index} = sig type T def f(x: T): Int end module M{index} : S{index} = struct type T = Int def f x => x end def main = M{index}.f {index}"
        );
        assert!(parse_source(&source, "generated").is_ok());
    }
}

#[test]
fn incomplete_grammar_forms_are_rejected() {
    for source in [
        "def main = fn => 1",
        "def f(x) => x",
        "def f _ => 1",
        "module type S = sig def f x end",
        "module type S = sig def f(x: Int) end",
        "def main = {}",
        "def main = let {} = x -> 1",
    ] {
        assert_eq!(
            parse_source(source, "generated")
                .unwrap_err()
                .code
                .as_deref(),
            Some("E100"),
            "{source}"
        );
    }
}

#[test]
fn overloaded_operator_is_the_anchor() {
    for padding in 0..8 {
        let source = format!("{}def (+) a b => a", " ".repeat(padding));
        let error = parse_source(&source, "generated").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E107"));
        assert_eq!(error.col, padding + 6);
    }
}

#[test]
fn application_groups_separate_constructor_arguments() {
    let source = "def main = fold values (Nil) (fn reversed value => Cons(value, reversed))";
    let ast = parse_source(source, "generated").unwrap();
    let normal = shin::parser::canonical_program(&ast);
    let restored = parse_source(&normal, "normalized").unwrap();
    assert_eq!(ast.main(), restored.main(), "{normal}");
}

#[test]
fn fallback_requires_a_pattern_arm() {
    for source in ["def f | 5", "def main = fn | 5", "def main = if x | 5"] {
        assert_eq!(
            parse_source(source, "generated")
                .unwrap_err()
                .code
                .as_deref(),
            Some("E100")
        );
    }
}

#[test]
fn lambdas_may_branch_but_cannot_hide_nested_branches() {
    assert!(parse_source("def main = fn x => if x => 1 | 0", "generated").is_ok());
    for source in [
        "def main = if true => fn x => if x => 1 | 0 | fn x => x",
        "def main = if true => let x = 1 -> if x > 0 => 1 | 0 | 0",
        "def main = if true => do? Ok() -> if true => 1 | 0 | 0",
    ] {
        assert_eq!(
            parse_source(source, "generated")
                .unwrap_err()
                .code
                .as_deref(),
            Some("E101")
        );
    }
}

#[test]
fn identifier_namespaces_follow_the_grammar() {
    for index in 0..12 {
        for source in [
            format!("def Upper{index} = 1"),
            format!("type lower{index} = | Case{index}"),
            format!("type Choice{index} = | lower{index}"),
            format!("type Box{index}<lower> = | Box{index}(Int)"),
            format!("type Record{index} = {{ Upper: Int }}"),
            format!("module lower{index} : S = struct end"),
            format!("module type lower{index} = sig end"),
            format!("module type S = sig def Upper{index}(x: Int): Int end"),
            format!("def main = {{ Upper{index} = 1 }}"),
            format!("def main = value.Upper{index}"),
            "def main = _".into(),
        ] {
            let error = parse_source(&source, "generated").unwrap_err();
            assert_eq!(error.code.as_deref(), Some("E100"), "{source}");
        }
    }
}

#[test]
fn prefix_expressions_require_grouping_inside_applications_and_operators() {
    for source in [
        "def main = f fn x => x",
        "def main = f let x = 1 -> x",
        "def main = 1 + if true => 2 | 3",
        "def main = - if true => 2 | 3",
    ] {
        assert_eq!(parse_source(source, "generated").unwrap_err().code.as_deref(), Some("E100"), "{source}");
    }
    for source in [
        "def main = f (fn x => x)",
        "def main = f (let x = 1 -> x)",
        "def main = 1 + (if true => 2 | 3)",
    ] {
        assert!(parse_source(source, "generated").is_ok(), "{source}");
    }
}

#[test]
fn integer_field_receivers_remain_separate_from_float_tokens() {
    for value in 0..24 {
        let source = format!("def main = {value} . field{value}");
        let ast = parse_source(&source, "generated").unwrap();
        let normal = shin::parser::canonical_program(&ast);
        let restored = parse_source(&normal, "normalized").unwrap_or_else(|error| panic!("{error}: {normal}"));
        assert_eq!(ast.main(), restored.main());
    }
}
