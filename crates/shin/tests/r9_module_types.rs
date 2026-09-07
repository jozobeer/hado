use shin::{check_source, types::Ty};
use std::process::Command;

const SIGNATURE: &str = "module type S = sig type T def make(x: Int): T def get(value: T): Int end";

fn module(name: &str, representation: &str, make: &str, get: &str) -> String {
    format!("module {name} : S = struct type T = {representation} def make x => {make} def get value => {get} end")
}

#[test]
fn abstract_types_can_be_instantiated_with_existing_types() {
    for (prefix, representation, make, get) in [
        ("", "Int", "x", "value + 1"),
        ("type Data = | Wrap(Int)", "Data", "Wrap(x)", "if value | Wrap(x) => x"),
        ("type Data = { number: Int }", "Data", "{ number = x }", "value.number"),
    ] {
        let source = format!("{prefix}\n{SIGNATURE}\n{}\ndef main = M.get (M.make 7)", module("M", representation, make, get));
        assert_eq!(check_source(&source, "module-types.shin").unwrap().main, Ty::Int);
    }
}

#[test]
fn abstract_instantiations_preserve_module_identity() {
    let source = format!("{SIGNATURE}\n{}\n{}\ndef main = N.get (M.make 7)", module("M", "Int", "x", "value + 1"), module("N", "Int", "x", "value + 1"));
    assert_eq!(check_source(&source, "module-types.shin").err().unwrap().code.as_deref(), Some("E305"));
}

#[test]
fn native_execution_uses_the_concrete_representation() {
    let dir = std::env::temp_dir().join(format!("shin-module-alias-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (case, (prefix, representation, make, get)) in [
        ("", "Int", "x", "value + 1"),
        ("type Data = | Wrap(Int)", "Data", "Wrap(x)", "if value | Wrap(x) => x + 1"),
        ("type Data = { number: Int }", "Data", "{ number = x }", "value.number + 1"),
    ].into_iter().enumerate() {
      for index in 1..6 {
        let path = dir.join(format!("case{case}_{index}.shin"));
        let source = format!("{prefix}\n{SIGNATURE}\n{}\ndef main = M.get (M.make {index})", module("M", representation, make, get));
        std::fs::write(&path, source).unwrap();
        let built = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&path).output().unwrap();
        assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
        let output = Command::new(path.with_extension("")).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{}\n", index + 1).as_bytes());
        assert!(output.stderr.is_empty());
    }
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opaque_values_cannot_hide_function_payloads_from_monomorphization() {
    for (representation, make) in [
        ("Fn<Int, Int>", "fn y => x + y"),
        ("{ callback: Fn<Int, Int> }", "{ callback = fn y => x + y }"),
    ] {
        let source = format!("module type S = sig type T def make(x: Int): T end\nmodule M : S = struct type T = {representation} def make x => {make} end\ndef main = M.make 1");
        assert_eq!(check_source(&source, "module-types.shin").err().unwrap().code.as_deref(), Some("E702"));
    }
}

#[test]
fn signature_type_variables_shadow_abstract_type_names() {
    let source = "module type S = sig type T def make(x: Int): T def identity<T>(x: T): T end\nmodule M : S = struct type T = Int def make x => x def identity x => x end\ndef main = M.identity true";
    assert_eq!(check_source(source, "module-types.shin").unwrap().main, Ty::Bool);
}
