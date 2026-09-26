use hado::lexer::{lex_full, TokKind};
use hado::parser::parse_source;

#[test]
fn module_declarations_are_top_level_only() {
    for padding in 0..20 {
        for nested in [
            "implement S as Inner",
            "module Inner",
            "implement S as Inner def broken = )",
        ] {
            let prefix = format!(
                "def main = 1\nmodule S\nimplement S as Outer def run x => (\n{}",
                " ".repeat(padding)
            );
            let source = format!("{prefix}{nested})");
            let error = parse_source(&source, "nested-module.hd").unwrap_err();
            assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
            assert_eq!((error.line, error.col), (4, padding + 1), "{error}");
        }
    }
}

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
    for index in [0u64, 9] {
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
            (format!("module M{index} = struct def f x => x end"), "E100"),
        ] {
            let error = parse_source(&source, "generated").unwrap_err();
            assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
        }
    }
}

#[test]
fn whitespace_does_not_change_calls() {
    for index in [0, 9] {
        for callee in [format!("f{index}"), format!("Case{index}")] {
            let baseline =
                parse_source(&format!("def main = {callee}({index})"), "generated").unwrap();
            for gap in [" ", "\n", " // gap\n "] {
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
    for index in [1, 19] {
        for source in [
            format!("def f{index} | -{index} => 1 | 0 def main = f{index} {index}"),
            format!("def main = fn(| -{index}.5 => true | false)"),
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
    for index in [0, 19] {
        for source in [
            format!("type Alias{index} = Int"),
            format!("type Sum{index} = | Case{index}({{ field: Int }})"),
            format!("module S{index} type T = Int"),
            format!("module S{index} type T<A>"),
        ] {
            let error = parse_source(&source, "generated").unwrap_err();
            assert_eq!(error.code.as_deref(), Some("E100"), "{source}");
        }
        let source = format!(
            "def main = M{index}.f {index}\nmodule S{index} type T def f(x: T): Int\nimplement S{index} as M{index} type T = Int def f x => x"
        );
        assert!(parse_source(&source, "generated").is_ok());
    }
}

#[test]
fn incomplete_grammar_forms_are_rejected() {
    for source in [
        "def main = fn(=> 1)",
        "def f(x) => x",
        "module S def f x",
        "module S def f(x: Int)",
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
    assert!(parse_source("def f _ => 1", "generated").is_ok());
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
    let source = "def main = fold values (Nil) fn(reversed, value => Cons(value, reversed))";
    let ast = parse_source(source, "generated").unwrap();
    let normal = hado::parser::canonical_program(&ast);
    let restored = parse_source(&normal, "normalized").unwrap();
    assert_eq!(ast.main(), restored.main(), "{normal}");
}

#[test]
fn fallback_requires_a_pattern_arm() {
    for source in ["def f | 5", "def main = fn(| 5)", "def main = if x | 5"] {
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
    assert!(parse_source("def main = fn(x => if x => 1 | 0)", "generated").is_ok());
    for source in [
        "def main = if true => fn(x => if x => 1 | 0) | fn(x => x)",
        "def main = if true => let x = 1 -> if x > 0 => 1 | 0 | 0",
        "def main = if true => do Ok() -> if true => 1 | 0 | 0",
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
    for index in [0, 9] {
        for source in [
            format!("def Upper{index} = 1"),
            format!("type lower{index} = | Case{index}"),
            format!("type Choice{index} = | lower{index}"),
            format!("type Box{index}<lower> = | Box{index}(Int)"),
            format!("type Record{index} = {{ Upper: Int }}"),
            format!("implement S as lower{index}"),
            format!("module lower{index}"),
            format!("module S def Upper{index}(x: Int): Int"),
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
        "def main = f",
        "def main = 1 + if true => 2 | 3",
        "def main = - if true => 2 | 3",
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
    for source in [
        "def main = f fn(x => x)",
        "def main = f (1 as x ->\n  x)",
        "def main = 1 + (if true => 2 | 3)",
    ] {
        assert!(parse_source(source, "generated").is_ok(), "{source}");
    }
}

#[test]
fn integer_field_receivers_remain_separate_from_float_tokens() {
    for value in [0, 7, 23] {
        let source = format!("def main = {value} . field{value}");
        let ast = parse_source(&source, "generated").unwrap();
        let normal = hado::parser::canonical_program(&ast);
        let restored =
            parse_source(&normal, "normalized").unwrap_or_else(|error| panic!("{error}: {normal}"));
        assert_eq!(ast.main(), restored.main());
    }
}

#[test]
fn implementation_aliases_only_realize_declared_abstract_types() {
    for index in [0, 19] {
        let prefix = format!("implement S{index} as M{index} type ");
        for body in ["Int", "Int List", "Fn(Int => Int)", "Other.T"] {
            let source =
                format!("def main = 1\n{prefix}Extra{index} = {body}\nmodule S{index} type T");
            parse_source(&source, "generated").unwrap();
            let error = hado::check_source(&source, "generated").err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
            assert_eq!((error.line, error.col), (2, prefix.len() + 1));
        }
        for body in [
            "type T = Int",
            "type Private = | Wrap(Int) type T = Private",
            "type Private = { value: Int } type T = Private",
        ] {
            let source = format!(
                "def main = 1\nimplement S{index} as M{index} {body}\nmodule S{index} type T"
            );
            parse_source(&source, "generated").unwrap_or_else(|error| panic!("{error}: {source}"));
        }
    }
}

#[test]
fn abstract_aliases_reject_legacy_type_parameter_spelling() {
    for spacing in ["", "  ", "\n  "] {
        let prefix = format!("module S type T\nimplement S as M type T{spacing}");
        let source = format!("{prefix}<A> = A List");
        let error = parse_source(&source, "generated").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
        assert_eq!(
            (error.line, error.col),
            (
                prefix.lines().count(),
                prefix.rsplit('\n').next().unwrap().len() + 1
            )
        );
    }
}

#[test]
fn alias_context_uses_prelude_and_preserves_signature_name_errors() {
    let prefix = "implement List as M type ";
    let source = format!("def main = 1\n{prefix}Extra = Int");
    let error = hado::check_source(&source, "generated").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
    assert_eq!((error.line, error.col), (2, prefix.len() + 1));
    for (source, code) in [
        ("def main = 1\nimplement Missing as M type T = Int", "E204"),
        (
            "def main = 1\nmodule S type T\nmodule S\nimplement S as M type T = Int",
            "E205",
        ),
    ] {
        let error = hado::check_source(source, "generated").err().unwrap();
        assert_eq!(error.code.as_deref(), Some(code), "{error}");
    }
}

#[test]
fn syntax_errors_precede_alias_name_errors_across_section_boundaries() {
    for signature_first in [false, true] {
        for broken in ["def broken = [)", "def broken x => [)"] {
            let signature = "module S type T";
            let implementation = format!("implement S as M type Extra = Int {broken}");
            let sections = if signature_first {
                format!("{signature}\n{implementation}")
            } else {
                format!("{implementation}\n{signature}")
            };
            let source = format!("def main = 1\n{sections}");
            let error = hado::check_source(&source, "generated").err().unwrap();
            let before = &source[..source.find(')').unwrap()];
            assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
            assert_eq!(
                (error.line, error.col),
                (
                    before.lines().count(),
                    before.rsplit('\n').next().unwrap().len() + 1
                ),
                "{source}: {error}"
            );
        }
    }
    let source = "def main = [)\nimplement List as M type Extra = Int";
    let error = hado::check_source(source, "generated").err().unwrap();
    assert_eq!((error.line, error.col), (1, "def main = [".len() + 1));
}

#[test]
fn syntax_recovery_keeps_scopes_and_never_hides_parse_errors() {
    for source in [
        "def broken = [)\nmodule S type T\nimplement S as M type Extra = Int",
        "def broken = [)\nmodule Bad = struct  end\nmodule S\nmodule S type T\nimplement S as M type T = Int",
        "def broken = [)\nmodule S def broken(x: Int) Int\nimplement S as M type Extra = Int",
    ] {
        let error = parse_source(source, "generated").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"));
        assert_eq!((error.line, error.col), (1, 15), "{error}: {source}");
    }
    let source = "implement S as M type T = Int\nmodule S type T def broken(x: Int) Int";
    let error = parse_source(source, "generated").unwrap_err();
    assert_eq!(
        error.line, 2,
        "an incomplete signature cannot prove an alias invalid: {error}"
    );
    for source in [
        "module S type T\nimplement S as M type T = Int def broken = [)",
        "module S type T\nimplement S as M type T =",
        "def main = [)",
    ] {
        assert!(parse_source(source, "generated").is_err());
        assert!(hado::check_source(source, "generated").is_err());
    }
}

#[test]
fn broken_heading_precedes_later_duplicate_contracts() {
    let source = "module Bad = struct  end\n\ndef main = 1\nimplement S as M type T = Int\nmodule S\nmodule S type T";
    let error = parse_source(source, "generated").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
    assert_eq!((error.line, error.col), (1, "module Bad ".len() + 1));
}

#[test]
fn legacy_alias_parameters_precede_a_malformed_alias_body() {
    let prefix = "module S type T\nimplement S as M type T";
    let source = format!("{prefix}<A> = (Int, ) List");
    let error = parse_source(&source, "generated").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E100"));
    assert_eq!(
        (error.line, error.col),
        (2, prefix.rsplit('\n').next().unwrap().len() + 1),
        "{error}"
    );
}

#[test]
fn malformed_alias_body_precedes_alias_name_resolution() {
    let prefix = "implement S as M type Extra =";
    let source = format!("{prefix}(Int, ) Result\nmodule S type T");
    let error = parse_source(&source, "generated").unwrap_err();
    assert_eq!(error.code.as_deref(), Some("E100"));
    assert_eq!(
        (error.line, error.col),
        (1, prefix.len() + "(Int, ".len() + 1),
        "{error}"
    );
}

#[test]
fn functor_and_application_lists_require_uppercase_nonempty_items() {
    for (prefix, invalid) in [
        ("implement S as Make(", ")"),
        ("implement S as Make(", "lower: S)"),
        ("implement S as Make(P: ", "lower)"),
        ("implement S as Make(P: S, ", ")"),
        ("implement S as A = ", "lower(I)"),
        ("implement S as A = Make(", ")"),
        ("implement S as A = Make(", "lower)"),
        ("implement S as A = Make(I, ", ")"),
    ] {
        for padding in 0..5 {
            let source = format!(
                "{}{}{}{}",
                "\n".repeat(padding % 3),
                prefix,
                " ".repeat(padding),
                invalid
            );
            let error = parse_source(&source, "functor-list.hd").unwrap_err();
            assert_eq!(error.code.as_deref(), Some("E100"), "{source}: {error}");
            assert_eq!(error.path, "functor-list.hd");
            let unit = padding == 0 && prefix.ends_with('(') && invalid == ")";
            let column = prefix.len() + padding + 1 - usize::from(unit);
            assert_eq!(
                (error.line, error.col),
                (padding % 3 + 1, column),
                "{source}: {error}"
            );
        }
    }
}

#[test]
fn applications_end_before_the_next_section_and_cannot_have_items() {
    for padding in 0..5 {
        let prefix = format!("implement S as A = Make(I)\n{}", " ".repeat(padding));
        for body in ["def extra = 1", "type T = Int"] {
            let source = format!("{prefix}{body}");
            let error = parse_source(&source, "application-body.hd").unwrap_err();
            assert_eq!(error.code.as_deref(), Some("E100"), "{source}: {error}");
            assert_eq!(error.path, "application-body.hd");
            assert_eq!((error.line, error.col), (2, padding + 1));
        }
        for section in [
            "module Next",
            "implement S as Next",
            "implement S as Next(P: S)",
            "implement S as Next = Make(J)",
        ] {
            let source = format!("{prefix}{section}");
            let ast = parse_source(&source, "application-section.hd").unwrap();
            assert!(
                matches!(&ast.items[0], hado::parser::Item::ModuleImpl(module) if matches!(module.form, hado::parser::ImplementationForm::Application { .. }) && module.items.is_empty())
            );
            assert_eq!(ast.items.len(), 2);
        }
    }
}

#[test]
fn functor_forms_preserve_generated_names_and_argument_order() {
    use hado::parser::{canonical_program, ImplementationForm, Item};
    for count in 1..4 {
        let params: Vec<_> = (0..count).map(|i| format!("P{i}: S{i}")).collect();
        let arguments: Vec<_> = (0..count).rev().map(|i| format!("I{i}")).collect();
        let source = format!(
            "implement Result as Make({}) type T = Int\nimplement Result as Applied = Make({})",
            params.join(", "),
            arguments.join(", ")
        );
        let ast = parse_source(&source, "functor-forms.hd").unwrap();
        for source in [&source, &canonical_program(&ast)] {
            let ast = parse_source(source, "functor-forms.hd").unwrap();
            let Item::ModuleImpl(definition) = &ast.items[0] else {
                panic!("missing functor");
            };
            let ImplementationForm::Functor(actual) = &definition.form else {
                panic!("functor form lost");
            };
            assert_eq!(definition.name, "Make");
            assert_eq!(
                actual
                    .iter()
                    .map(|param| (param.name.clone(), param.contract.clone()))
                    .collect::<Vec<_>>(),
                (0..count)
                    .map(|i| (format!("P{i}"), format!("S{i}")))
                    .collect::<Vec<_>>()
            );
            let Item::ModuleImpl(application) = &ast.items[1] else {
                panic!("missing application");
            };
            let ImplementationForm::Application {
                functor,
                arguments: actual,
            } = &application.form
            else {
                panic!("application form lost");
            };
            assert_eq!(application.name, "Applied");
            assert_eq!(functor.name, "Make");
            assert_eq!(
                actual
                    .iter()
                    .map(|argument| argument.name.clone())
                    .collect::<Vec<_>>(),
                arguments
            );
            assert!(application.items.is_empty());
        }
    }
}
