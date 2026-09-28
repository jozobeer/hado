use hado::{check_source, types::Ty};
use std::process::Command;

const SIGNATURE: &str = "module S\ntype T\ndef make (x: Int): T\ndef get (value: T): Int\n/module";

fn module(name: &str, representation: &str, make: &str, get: &str) -> String {
    let realization = if representation.starts_with(['{', '|']) {
        format!("type Representation = {representation} type T = Representation")
    } else {
        format!("type T = {representation}")
    };
    format!("implement S as {name}\n{realization}\ndef make x => {make}\ndef get (value: T) => {get}\n/implement")
}

fn program(outer: &str, sections: &[&str], body: &str) -> String {
    format!("{outer}\ndef main = {body}\n{}", sections.join("\n"))
}

#[test]
fn cyclic_abstract_aliases_report_signature_mismatch_at_the_module() {
    for padding in 0..3 {
        let signature = "module S\ntype T\ntype U\ndef id (x: Int): Int\n/module";
        for body in [
            "type T = T type U = Int",
            "type T = U type U = T",
            "type T = T List type U = Int",
        ] {
            // 後置では `implement` を閉じまで綴るので、見出しの位置は
            // 組み立てた綴りから数える。ずれてもここで気づける。
            let implementation = format!(
                "{}implement S as M\n{body}\ndef id x => x\n/implement",
                " ".repeat(padding)
            );
            let source = program("", &[signature, &implementation], "M.id 1");
            let header = source.find("implement S as M").unwrap();
            let prefix = &source[..header];
            let error = check_source(&source, "cyclic-alias.hd").err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
            assert_eq!(
                (error.line, error.col),
                (
                    prefix.matches('\n').count() + 1,
                    prefix.rsplit('\n').next().unwrap().chars().count() + 1
                ),
                "{error}"
            );
        }
        for body in [
            "type T = U type U = Int",
            "type Data = | Done | Next(Data) type T = Data type U = Int",
        ] {
            let implementation = format!("implement S as M\n{body}\ndef id x => x\n/implement");
            assert_eq!(
                check_source(
                    &program(
                        "",
                        &[signature, &implementation],
                        &format!("M.id {padding}")
                    ),
                    "resolved-alias.hd"
                )
                .unwrap()
                .main,
                Ty::Int
            );
        }
    }
}

#[test]
fn abstract_types_can_be_instantiated_with_existing_types() {
    for (prefix, representation, make, get) in [
        ("", "Int", "x", "value + 1"),
        (
            "type Data = | Wrap(Int)",
            "Data",
            "Wrap(x)",
            "if value | Wrap(x) => x",
        ),
        (
            "type Data = { number: Int }",
            "Data",
            "{ number = x }",
            "value.number",
        ),
    ] {
        let source = program(
            prefix,
            &[SIGNATURE, &module("M", representation, make, get)],
            "M.get (M.make 7)",
        );
        assert_eq!(check_source(&source, "").unwrap().main, Ty::Int);
    }
}

#[test]
fn abstract_instantiations_preserve_module_identity() {
    let source = program(
        "",
        &[
            SIGNATURE,
            &module("M", "Int", "x", "value + 1"),
            &module("N", "Int", "x", "value + 1"),
        ],
        "N.get (M.make 7)",
    );
    assert_eq!(
        check_source(&source, "").err().unwrap().code.as_deref(),
        Some("E305")
    );
}

#[test]
fn native_execution_uses_the_concrete_representation() {
    let dir = std::env::temp_dir().join(format!("hado-module-alias-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (case, (prefix, representation, make, get)) in [
        ("", "Int", "x", "value + 1"),
        (
            "type Data = | Wrap(Int)",
            "Data",
            "Wrap(x)",
            "if value | Wrap(x) => x + 1",
        ),
        (
            "type Data = { number: Int }",
            "Data",
            "{ number = x }",
            "value.number + 1",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let index = 1;
        let case_dir = dir.join(format!("case{case}_{index}"));
        std::fs::create_dir_all(&case_dir).unwrap();
        let path = case_dir.join("main.hd");
        let source = program(
            prefix,
            &[SIGNATURE, &module("M", representation, make, get)],
            &format!("Int.to_string (M.get (M.make {index}))"),
        );
        std::fs::write(&path, source).unwrap();
        let built = Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg("build")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = Command::new(path.with_extension("")).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{}\n", index + 1).as_bytes());
        assert!(output.stderr.is_empty());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opaque_values_cannot_hide_function_payloads_from_monomorphization() {
    for (representation, make) in [
        ("Fn(Int => Int)", "fn(y => x + y)"),
        (
            "{ callback: Fn(Int => Int) }",
            "{ callback = fn(y => x + y) }",
        ),
    ] {
        let implementation = if representation.starts_with('{') {
            format!("implement S as M\ntype Representation = {representation}\ntype T = Representation\ndef make x => {make}\n/implement")
        } else {
            format!("implement S as M\ntype T = {representation}\ndef make x => {make}\n/implement")
        };
        let source = program(
            "",
            &[
                "module S\ntype T\ndef make (x: Int): T\n/module",
                &implementation,
            ],
            "M.make 1",
        );
        assert_eq!(
            check_source(&source, "").err().unwrap().code.as_deref(),
            Some("E702")
        );
    }
}

#[test]
fn signature_type_variables_shadow_abstract_type_names() {
    let source = program(
        "",
        &[
            "module S\ntype T\ndef make (x: Int): T\ndef T identity (x: T): T\n/module",
            "implement S as M\ntype T = Int\ndef make x => x\ndef identity x => x\n/implement",
        ],
        "M.identity true",
    );
    assert_eq!(check_source(&source, "").unwrap().main, Ty::Bool);
}

#[test]
fn opaque_projection_reports_the_dot() {
    for padding in 0..16 {
        let before_dot = format!("def main = (M.make 1){}", " ".repeat(padding));
        let source = format!("{before_dot}.value\nmodule S\ntype T\ndef make (x: Int): T\n/module\nimplement S as M\ntype Representation = {{ value: Int }}\ntype T = Representation\ndef make x => {{ value = x }}\n/implement");
        let error = check_source(&source, "opaque-field.hd").err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E601"));
        assert_eq!((error.line, error.col), (1, before_dot.len() + 1));
    }
}

#[test]
fn outer_data_can_reference_later_implementations_in_any_section_order() {
    let index = 1;
    for data in ["type Box = | Wrap(M.T)", "type Box = { value: M.T }"] {
        let implementation = module("M", "Int", "x", "value + 1");
        for sections in [
            [SIGNATURE, implementation.as_str()],
            [implementation.as_str(), SIGNATURE],
        ] {
            let body = if data.contains("Wrap") {
                format!("Wrap(M.make {index}) as Wrap(value) -> M.get value")
            } else {
                format!("{{ value = M.make {index} }} as box -> M.get box.value")
            };
            let source = program(data, &sections, &body);
            assert_eq!(
                check_source(&source, "")
                    .unwrap_or_else(|error| panic!("{error}: {source}"))
                    .main,
                Ty::Int
            );
        }
    }
}

#[test]
fn abstract_types_in_data_preserve_identity_and_opacity() {
    for (body, code) in [
        ("{ value = N.make 1 } as box -> M.get box.value", "E305"),
        ("{ value = M.make 1 } as box -> box.value.number", "E601"),
    ] {
        let source = program(
            "type Box = { value: M.T }",
            &[
                SIGNATURE,
                &module("M", "{ number: Int }", "{ number = x }", "value.number + 1"),
                &module("N", "{ number: Int }", "{ number = x }", "value.number + 1"),
            ],
            body,
        );
        let error = check_source(&source, "").err().unwrap();
        assert_eq!(error.code.as_deref(), Some(code), "{error}");
    }
    let source = program(
        "type Box = | Wrap(M.Private)",
        &[
            "module S\n\n/module",
            "implement S as M\ntype Private = | Hidden\n/implement",
        ],
        "1",
    );
    assert_eq!(
        check_source(&source, "").err().unwrap().code.as_deref(),
        Some("E204")
    );
}

#[test]
fn native_data_payloads_use_module_abstract_representations() {
    let dir = std::env::temp_dir().join(format!("hado-abstract-payload-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (case, (representation, make, get)) in [
        ("Int", "x", "value + 1"),
        ("{ number: Int }", "{ number = x }", "value.number + 1"),
    ]
    .into_iter()
    .enumerate()
    {
        for record in [false, true] {
            let index = 1;
            let data = if record {
                "type Box = { value: M.T }"
            } else {
                "type Box = | Wrap(M.T)"
            };
            let body = if record {
                format!("Int.to_string ({{ value = M.make {index} }} as box -> M.get box.value)")
            } else {
                format!("Int.to_string (Wrap(M.make {index}) as Wrap(value) -> M.get value)")
            };
            let case_dir = dir.join(format!("case{case}_{record}_{index}"));
            std::fs::create_dir_all(&case_dir).unwrap();
            let path = case_dir.join("main.hd");
            let source = program(
                data,
                &[&module("M", representation, make, get), SIGNATURE],
                &body,
            );
            std::fs::write(&path, source).unwrap();
            let built = Command::new(env!("CARGO_BIN_EXE_hado"))
                .arg("build")
                .arg(&path)
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{}",
                String::from_utf8_lossy(&built.stderr)
            );
            let output = Command::new(path.with_extension("")).output().unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, format!("{}\n", index + 1).as_bytes());
            assert!(output.stderr.is_empty());
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
