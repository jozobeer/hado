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

#[test]
fn constructors_require_all_payloads_and_are_not_function_values() {
    let mut state = 0x9ca7_u64;
    for _ in 0..24 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let arity = 1 + (state % 5) as usize;
        let name = format!("Pack{state}");
        let types = vec!["Int"; arity].join(", ");
        let declaration = format!("type Data = | {name}({types})");
        for expression in [name.clone(), format!("{name}({})", vec!["1"; arity + 1].join(", "))] {
            let diagnostic = error(&format!("{declaration}\ndef main = {expression}"));
            assert_eq!(diagnostic.code.as_deref(), Some("E301"));
            assert_eq!((diagnostic.line, diagnostic.col), (2, 12));
        }
        let source = format!("{declaration}\ndef main = {name}({})", vec!["1"; arity].join(", "));
        assert_eq!(check_source(&source, "generated-names.shin").unwrap().main,
            shin::types::Ty::Adt("Data".into(), vec![]));
    }
    assert_eq!(error("def apply f => f 1\ndef main = apply Some").code.as_deref(), Some("E301"));
    assert_eq!(error("def main = Nil(1)").code.as_deref(), Some("E301"));
}

#[test]
fn unused_signatures_resolve_all_type_names_at_the_reference() {
    let mut state = 0x118b_u64;
    for padding in 0..24 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let unknown = format!("Missing{state}");
        let prefix = format!("{}def convert<A>(value: Fn<A, ", " ".repeat(padding));
        let source = format!("module type API = sig\n{prefix}{unknown}>): A\nend\ndef main = 1");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.code.as_deref(), Some("E204"));
        assert_eq!((diagnostic.line, diagnostic.col), (2, prefix.len() + 1));
    }
}

#[test]
fn signature_type_variables_are_local_to_each_member() {
    let source = "module type S = sig def first<A>(x: A): A def second(x: A): Int end\ndef main = 1";
    let diagnostic = error(source);
    assert_eq!(diagnostic.code.as_deref(), Some("E204"));
    assert_eq!(diagnostic.col, source.find("second(x: A").unwrap() + "second(x: ".len() + 1);
    let valid = "module type S = sig type T def first<A>(x: A): A def second(x: T): Int end\nmodule M : S = struct type T = { value: Int } def first x => x def second x => x.value end\ndef main = M.first 1";
    check_source(valid, "generated-names.shin").unwrap();
}

#[test]
fn signatures_can_reference_exported_qualified_types() {
    let source = "module type S = sig type T def make(x: Int): T end\nmodule M : S = struct type T = { value: Int } def make x => { value = x } end\nmodule type API = sig def consume(value: M.T): Int end\ndef main = 1";
    let ast = shin::parser::parse_source(source, "generated-names.shin").unwrap();
    shin::names::resolve(&ast, "generated-names.shin").unwrap();
}

#[test]
fn duplicate_fields_and_type_variables_report_the_second_name() {
    for index in 0..20 {
        let field = format!("field{index}");
        let variable = format!("A{index}");
        for (prefix, suffix) in [
            (format!("type Record = {{ {field}: Int; "), format!("{field}: Bool }}\ndef main = 1")),
            (format!("module type S = sig def id<{variable}, "), format!("{variable}>(x: {variable}): {variable} end\ndef main = 1")),
            (format!("type Data<{variable}, "), format!("{variable}> = | Empty\ndef main = 1")),
            (format!("type Data = | Pack{index} | "), format!("Pack{index}\ndef main = 1")),
        ] {
            let diagnostic = error(&format!("{prefix}{suffix}"));
            assert_eq!(diagnostic.code.as_deref(), Some("E205"));
            assert_eq!((diagnostic.line, diagnostic.col), (1, prefix.len() + 1));
        }
    }
}

#[test]
fn aliases_reference_types_without_declaring_constructors() {
    let source = "module type S = sig type T type U end\nmodule M : S = struct type T = Int type U = Int end\ndef main = 1";
    let ast = shin::parser::parse_source(source, "generated-names.shin").unwrap();
    shin::names::resolve(&ast, "generated-names.shin").unwrap();
}

#[test]
fn signature_mismatches_report_the_module_for_all_four_forms() {
    for index in 0..20 {
        let member = format!("member{index}");
        for (signature, body) in [
            (format!("def {member}(x: Int): Int"), "".to_owned()),
            ("type T".to_owned(), "".to_owned()),
            (format!("def {member}(x: Int): Bool"), format!("def {member} x => x + 1")),
            (format!("def {member}<A>(x: A): A"), format!("def {member} x => x + 1")),
        ] {
            let source = format!("module type S = sig {signature} end\n{}module M : S = struct {body} end\ndef main = 1", " ".repeat(index));
            let diagnostic = error(&source);
            assert_eq!(diagnostic.code.as_deref(), Some("E602"), "{source}");
            assert_eq!((diagnostic.line, diagnostic.col), (2, index + 1));
        }
    }
}

#[test]
fn duplicate_record_values_report_the_second_field_before_type_errors() {
    for index in 0..20 {
        let name = format!("field{index}");
        let prefix = format!("def main = {{ {name} = 1; {}", " ".repeat(index));
        let source = format!("{prefix}{name} = true }}");
        let diagnostic = error(&source);
        assert_eq!(diagnostic.code.as_deref(), Some("E205"));
        assert_eq!((diagnostic.line, diagnostic.col), (1, prefix.len() + 1));
    }
}
