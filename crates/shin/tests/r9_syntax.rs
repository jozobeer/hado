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

#[test]
fn struct_aliases_only_realize_declared_abstract_types() {
    for index in 0..20 {
        let prefix = format!("module M{index} : S{index} = struct type Extra{index} = ");
        for body in ["Int", "List<Int>", "Fn<Int, Int>", "Other.T"] {
            let source = format!("{prefix}{body} end\nmodule type S{index} = sig type T end\ndef main = 1");
            let error = parse_source(&source, "generated").unwrap_err();
            assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
            assert_eq!((error.line, error.col), (1, prefix.len() + 1));
        }

        for body in ["type T = Int", "type Private = | Wrap(Int) type T = Private", "type Private = { value: Int } type T = Private"] {
            let source = format!("module M{index} : S{index} = struct {body} end\nmodule type S{index} = sig type T end\ndef main = 1");
            parse_source(&source, "generated").unwrap_or_else(|error| panic!("{error}: {source}"));
        }
    }
}

#[test]
fn abstract_type_aliases_cannot_declare_type_parameters() {
    for spacing in ["", "  ", "\n  "] {
        let prefix = format!("module type S = sig type T end\nmodule M : S = struct type T{spacing}");
        let source = format!("{prefix}<A> = List<A> end\ndef main = 1");
        let error = parse_source(&source, "generated").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
        assert_eq!((error.line, error.col), (prefix.lines().count(), prefix.rsplit('\n').next().unwrap().len() + 1));
    }
}

#[test]
fn alias_context_uses_prelude_and_preserves_signature_name_errors() {
    let prefix = "module M : LIST = struct type Extra = ";
    let source = format!("{prefix}Int end\ndef main = 1");
    let error = shin::check_source(&source, "generated").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
    assert_eq!((error.line, error.col), (1, prefix.len() + 1));
    for (source, code) in [
        ("module M : Missing = struct type T = Int end\ndef main = 1", "E204"),
        ("module type S = sig type T end\nmodule type S = sig end\nmodule M : S = struct type T = Int end\ndef main = 1", "E205"),
    ] {
        let error = shin::check_source(source, "generated").err().unwrap();
        assert_eq!(error.code.as_deref(), Some(code), "{error}");
    }
}

#[test]
fn alias_errors_precede_later_syntax_errors_across_declaration_boundaries() {
    for signature_first in [false, true] {
        for tail in ["end\ndef main = [)", "def broken = [) end\ndef main = 1", "def broken = [)"] {
            let signature = "module type S = sig type T end\n";
            let prefix = if signature_first { signature } else { "" };
            let source = format!("{prefix}module M : S = struct type Extra = Int {tail}\n{}", if signature_first { "" } else { signature });
            // An unterminated struct cannot expose a later top-level signature.
            if !signature_first && !tail.contains("end") { continue; }
            let error = parse_source(&source, "generated").unwrap_err();
            let expected = source.find("Int").unwrap();
            let before = &source[..expected];
            assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
            assert_eq!((error.line, error.col), (before.lines().count(), before.rsplit('\n').next().unwrap().len() + 1), "{source}: {error}");
        }
    }
    let source = "module M : LIST = struct type Extra = Int end\ndef main = [)";
    let error = shin::check_source(source, "generated").err().unwrap();
    assert_eq!((error.line, error.col), (1, source.find("Int").unwrap() + 1));
}

#[test]
fn syntax_recovery_keeps_scopes_and_never_hides_parse_errors() {
    for source in [
        "def broken = [)\nmodule type S = sig type T end\nmodule M : S = struct type Extra = Int end",
        "def broken = [)\nmodule Bad = struct module type S = sig end end\nmodule type S = sig type T end\nmodule M : S = struct type T = Int end",
        "def broken = [)\nmodule type S = sig def broken(x: Int) Int end\nmodule M : S = struct type Extra = Int end",
    ] {
        let error = parse_source(source, "generated").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"));
        assert_eq!((error.line, error.col), (1, 15), "{error}: {source}");
    }
    let source = "module M : S = struct type T = Int end\nmodule type S = sig type T def broken(x: Int) Int end";
    let error = parse_source(source, "generated").unwrap_err();
    assert_eq!(error.line, 2, "an incomplete signature cannot prove an alias invalid: {error}");
    for source in [
        "module type S = sig type T end\nmodule M : S = struct type T = Int def broken = [)",
        "module type S = sig type T end\nmodule M : S = struct type T = Int",
        "def main = [)",
    ] {
        assert!(parse_source(source, "generated").is_err());
        assert!(shin::check_source(source, "generated").is_err());
    }
}

#[test]
fn recovery_does_not_promote_signatures_from_a_broken_module() {
    let source = "module M : S = struct type T = Int end\nmodule Bad = struct module type S = sig end end\nmodule type S = sig type T end\ndef main = 1";
    let error = parse_source(source, "generated").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E113"), "{error}");
    assert_eq!((error.line, error.col), (2, 1));
}

#[test]
fn alias_parameters_precede_a_malformed_alias_body() {
    let prefix = "module type S = sig type T end\nmodule M : S = struct type T";
    let source = format!("{prefix}<A> = List<) end\ndef main = 1");
    let error = parse_source(&source, "generated").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E100"));
    assert_eq!((error.line, error.col), (2, prefix.rsplit('\n').next().unwrap().len() + 1), "{error}");
}

#[test]
fn alias_context_precedes_an_error_inside_its_type_arguments() {
    let prefix = "module M : S = struct type Extra = ";
    let source = format!("{prefix}List<) end\nmodule type S = sig type T end\ndef main = 1");
    let error = parse_source(&source, "generated").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E100"));
    assert_eq!((error.line, error.col), (1, prefix.len() + 1), "{error}");
}
