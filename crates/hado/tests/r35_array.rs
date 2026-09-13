use hado::{check_source, mono::Type, types::Ty};

#[test]
fn array_annotation_and_arity_are_registered() {
    let checked = check_source("def f (xs: Int Array) => xs\ndef main = 0", "stdlib/probe.hd").unwrap();
    assert_eq!(checked.main, Ty::Int);
    for annotation in ["Array", "(Int, Bool) Array"] {
        assert!(check_source(&format!("def f (xs: {annotation}) => xs\ndef main = 0"), "stdlib/probe.hd").is_err());
    }
}

#[test]
fn array_is_reserved_and_uses_a_pointer() {
    let error = check_source("type A Array = { value: A }\ndef main = 0", "t").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E205"));
    let checked = check_source("def main = 0", "t").unwrap();
    let ty = Type::Data("Array".into(), vec![Type::Int]);
    assert_eq!(hado::repr::machine_type(&ty, &checked).unwrap(), hado::repr::MachineType::Pointer);
    assert_eq!(Type::try_from(&Ty::from(&ty)), Ok(ty));
}

#[test]
fn intrinsic_names_preserve_access_and_monomorphic_checks() {
    for name in ["__array_length", "__array_get", "__array_alloc", "__array_fill_unchecked"] {
        assert_eq!(check_source(&format!("def main = {name} 0"), "t").err().unwrap().code.as_deref(), Some("E202"));
        assert_eq!(check_source(&format!("def main = {name}"), "t").err().unwrap().code.as_deref(), Some("E203"));
    }
    assert_eq!(check_source("def main = __unknown 0", "stdlib/probe.hd").err().unwrap().code.as_deref(), Some("E204"));
    assert_eq!(check_source("def main = __string_length 0", "stdlib/probe.hd").err().unwrap().code.as_deref(), Some("E305"));
    assert!(check_source("def main = __string_length \"abc\"", "stdlib/probe.hd").is_ok());
}

#[test]
fn polymorphic_intrinsics_agree_on_element_types() {
    for element in ["Int", "Float", "Bool", "Char", "String", "Int Array"] {
        let source = format!("def f (xs: {element} Array, value: {element}): {element} Pure => __array_get (__array_fill_unchecked xs 0 value) 0\ndef make (n: Int): {element} Array Pure => __array_alloc n\ndef main = 0");
        check_source(&source, "stdlib/probe.hd").unwrap();
    }
    assert_eq!(check_source("def f (xs: Int Array) => __array_fill_unchecked xs 0 true\ndef main = 0", "stdlib/probe.hd").err().unwrap().code.as_deref(), Some("E305"));
}

#[test]
fn array_runtime_allocates_zeroed_eight_byte_slots() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("hado-array-runtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("test.c");
    std::fs::write(&source, r#"
#include "hado_runtime.h"
#include <assert.h>
#include <string.h>
int32_t hado_entry(void) {
    const int64_t counts[] = {0, 1, 3, 19, 30000};
    for (size_t c = 0; c < sizeof(counts) / sizeof(counts[0]); ++c) {
        HadoArray *array = hado_array_alloc(counts[c]);
        assert(array != NULL);
        assert(hado_array_length(array) == counts[c]);
        assert(offsetof(HadoArray, slots) == 8);
        assert(sizeof(array->slots[0]) == 8);
        for (int64_t i = 0; i < counts[c]; ++i) {
            assert(array->slots[i] == 0);
            array->slots[i] = (uint64_t)(i + 1);
        }
        for (int64_t i = 0; i < counts[c]; ++i) assert(array->slots[i] == (uint64_t)(i + 1));
        assert(hado_array_length(array) == counts[c]);
    }
    return 0;
}
"#).unwrap();
    let object = dir.join("test.o");
    assert!(std::process::Command::new("cc").args(["-std=c11", "-Wall", "-Werror", "-c"]).arg(&source).arg("-I").arg(root.join("runtime")).arg("-o").arg(&object).status().unwrap().success());
    let executable = dir.join("test");
    hado::link_executable(&object, &executable).unwrap();
    assert!(std::process::Command::new(&executable).status().unwrap().success());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn array_public_members_match_specification() {
    let spec = include_str!("../../../docs/spec.md");
    let expected = spec.split_once("module Array\ndef A length").unwrap().1.split_once("\nmodule Int").unwrap().0;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stdlib/array.hd");
    let source = std::fs::read_to_string(path).unwrap();
    let actual = source.split_once("module Array\ndef A length").unwrap().1.split_once("\nimplement Array as Array").unwrap().0;
    assert_eq!(actual.trim(), expected.trim());
}

#[test]
fn polymorphic_allocation_specializes_and_slots_execute() {
    let dir = std::env::temp_dir().join(format!("hado-array-slots-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (value, expected) in [("9876543210", "9876543210"), ("-0.25", "-0.25"), ("true", "true"), ("'𐀀'", "𐀀"), ("\"あ😀\"", "あ😀")] {
        let source = format!("def make n x => __array_fill_unchecked (__array_alloc n) 1 x\ndef main = __array_get (make 3 ({value})) 1");
        let object = dir.join("test.o");
        hado::compile_source_to_object(&source, "stdlib/probe.hd", &object).unwrap();
        let executable = dir.join("test");
        hado::link_executable(&object, &executable).unwrap();
        let output = std::process::Command::new(&executable).output().unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn array_comparisons_reject_function_elements_recursively() {
    for (declaration, value) in [
        ("", "Array.of_list [fn(x => x + 1)]"),
        ("", "Array.of_list [Array.of_list [fn(x => x + 1)]]"),
        ("type Cell = { value: Fn(Int => Int) }\n", "Array.of_list [{ value = fn(x => x + 1) }]"),
    ] {
        for (expression, code) in [("a == a", "E703"), ("a != a", "E703"), ("compare a a", "E704")] {
            let source = format!("{declaration}def main = let a = {value} -> {expression}");
            let error = check_source(&source, "t").err().unwrap_or_else(|| panic!("accepted {source}"));
            assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
        }
    }
}

#[test]
fn existing_list_record_and_opaque_function_equality_stays_rejected() {
    for source in [
        "def main = [fn(x => x + 1)] == [fn(x => x + 1)]",
        "type Cell = { run: Fn(Int => Int) }\ndef main = { run = fn(x => x + 1) } == { run = fn(x => x + 1) }",
        "def main = let a = Store.make 1 -> a == a\nmodule Store\ntype T\ndef make(x: Int): T Pure\nimplement Store as Store\ntype T = Fn(Int => Int)\ndef make x => fn(y => x + y)",
    ] {
        let error = check_source(source, "t").err().unwrap_or_else(|| panic!("accepted {source}"));
        assert_eq!(error.code.as_deref(), Some("E703"), "{source}: {error}");
    }
}
