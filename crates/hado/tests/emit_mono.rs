#[test]
fn scalar_function_body_lowers_to_verified_clif() {
    let source = "def calculate n => n * 2 + 3\ndef main = calculate 4";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let function = hado::emit::lower_mono_function(&program.functions[0], &checked).unwrap();
    let flags = cranelift_codegen::settings::Flags::new(cranelift_codegen::settings::builder());
    cranelift_codegen::verify_function(&function, &flags).unwrap();
    let clif = function.display().to_string();
    assert!(clif.contains("imul"));
    assert!(clif.contains("iadd"));
    assert_eq!(function.signature.params.len(), 2);
}

#[test]
fn derived_comparison_lowers_branches_to_verified_clif() {
    let source = "def main = compare 3 5";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let function = hado::emit::lower_mono_function(&program.functions[0], &checked).unwrap();
    let flags = cranelift_codegen::settings::Flags::new(cranelift_codegen::settings::builder());
    cranelift_codegen::verify_function(&function, &flags).unwrap();
    let clif = function.display().to_string();
    assert!(clif.contains("icmp slt"));
    assert!(clif.contains("brif"));
    assert!(clif.contains("return"));
}

#[test]
fn unary_operations_and_float_division_generate_typed_instructions() {
    for (source, instruction) in [
        ("def negate x => -x\ndef main = negate 3", "ineg"),
        ("def negate x => -x / 2.0\ndef main = negate 3.0", "fdiv"),
        ("def flip x => not x\ndef main = flip true", "icmp eq"),
    ] {
        let checked = hado::check_source(source, "t").unwrap();
        let ast = hado::parser::parse_source(source, "t").unwrap();
        let program = hado::mono::lower_program(&ast, &checked).unwrap();
        let function = hado::emit::lower_mono_function(&program.functions[0], &checked).unwrap();
        let flags = cranelift_codegen::settings::Flags::new(cranelift_codegen::settings::builder());
        cranelift_codegen::verify_function(&function, &flags).unwrap();
        assert!(
            function.display().to_string().contains(instruction),
            "{}",
            function.display()
        );
    }
}

#[test]
fn direct_calls_import_the_fixed_signature() {
    let source = "def inc x => x + 1\ndef twice x => inc x + 2\ndef main = twice 3";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let source = program
        .functions
        .iter()
        .find(|f| f.name == "twice")
        .unwrap();
    let function = hado::emit::lower_mono_function(source, &checked).unwrap();
    let flags = cranelift_codegen::settings::Flags::new(cranelift_codegen::settings::builder());
    cranelift_codegen::verify_function(&function, &flags).unwrap();
    assert!(function.display().to_string().contains("call fn"));
}

#[test]
fn recursive_tail_positions_emit_return_call() {
    let source =
        "def rec down n => if n == 0 => 0 | n - 1 as next ->\n  down next\n  /if\ndef main = down 3";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let function = hado::emit::lower_mono_function(
        program
            .functions
            .iter()
            .find(|function| function.name == "down")
            .unwrap(),
        &checked,
    )
    .unwrap();
    let flags = cranelift_codegen::settings::Flags::new(cranelift_codegen::settings::builder());
    cranelift_codegen::verify_function(&function, &flags).unwrap();
    let clif = function.display().to_string();
    assert!(clif.contains("return_call fn"));
    assert!(clif.contains("= call fn"));
}

#[test]
fn closures_execute_direct_values_and_captured_environments() {
    for (index, source, expected) in [
        (
            0,
            "def main = 7 as offset ->\n  fn(x => x + offset) as f ->\n  f 3",
            "10\n",
        ),
        (
            1,
            "def main = 7 as offset ->\n  fn(x => x + offset) as f ->\n  f 3 + f 5",
            "22\n",
        ),
    ] {
        let path =
            std::env::temp_dir().join(format!("hado-closure-{}-{index}", std::process::id()));
        hado::build::executable(source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn records_execute_with_declared_layout() {
    let source = "type Pair = { enabled: Bool; left: Int; right: Int }\ndef make n => { right = n + 1; enabled = true; left = n }\ndef main = make 20 as p ->\n  if p.enabled => p.left + p.right | 0";
    let path = std::env::temp_dir().join(format!("hado-record-{}", std::process::id()));
    hado::build::executable(source, "t", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"41\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn constructor_payloads_execute_with_tagged_layout() {
    let source = "type T Box = | Wrap(T)\ndef main = Wrap(40) as Wrap(x) ->\n  x + 2";
    let path = std::env::temp_dir().join(format!("hado-constructor-{}", std::process::id()));
    hado::build::executable(source, "t", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn constructor_switches_execute_immediate_and_boxed_variants() {
    for (index, source, expected) in [
        (0, "type Flag = | On | Off\ndef choose | On => 17 | Off => 29\ndef main = choose Off", "29\n"),
        (1, "def choose | Some(n) => n + 1 | None => 9\ndef main = choose (Some(41))", "42\n"),
        (2, "def choose (option: Int Option) => if option | Some(n) => n + 1 | None => 9\ndef main = choose None", "9\n"),
        (3, "def main = Some(40) as option ->\n  if(option | Some(n) => n | None => 9) + 2", "42\n"),
        (4, "def main = None as option ->\n  if(option | Some(n) => n | None => 9) + 2", "11\n"),
    ] {
        let path = std::env::temp_dir().join(format!("hado-switch-{}-{index}", std::process::id()));
        hado::build::executable(source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn integer_division_handles_zero_and_wrapping_boundary() {
    for (index, expression, stdout, stderr, status) in [
        (0, "7 / 2", "3\n", "", 0),
        (1, "7 % 2", "1\n", "", 0),
        (2, "7 / 0", "", "DivisionByZero\n", 1),
        (
            3,
            "(-9223372036854775807 - 1) / -1",
            "-9223372036854775808\n",
            "",
            0,
        ),
        (4, "-7 / 2", "-3\n", "", 0),
        (5, "-7 % 2", "-1\n", "", 0),
    ] {
        let source = format!("def main = {expression}");
        let path =
            std::env::temp_dir().join(format!("hado-division-{}-{index}", std::process::id()));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(status),
            "{expression}: {output:?}"
        );
        assert_eq!(output.stdout, stdout.as_bytes(), "{expression}");
        assert_eq!(output.stderr, stderr.as_bytes(), "{expression}");
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn nested_literal_patterns_merge_values_without_reaching_impossible_paths() {
    for (index, argument, expected) in [(0, "Some(true)", "42\n"), (1, "None", "2\n")] {
        let source = format!("def main = {argument} as option ->\n  if(option | Some(true) => 40 | Some(false) => 9 | None => 0) + 2");
        let path = std::env::temp_dir().join(format!(
            "hado-nested-pattern-{}-{index}",
            std::process::id()
        ));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn string_literals_are_linked_as_static_data() {
    let source = "type Item = { text: String; value: Int }\ndef main = { text = \"あ😀\"; value = 42 } as item ->\n  item.value";
    let checked = hado::check_source(source, "t").unwrap();
    let ast = hado::parser::parse_source(source, "t").unwrap();
    let program = hado::mono::lower_program(&ast, &checked).unwrap();
    let path = std::env::temp_dir().join(format!("hado-static-string-{}", std::process::id()));
    let object = path.with_extension("o");
    hado::emit::compile_mono_to_object(&program, &checked, &object).unwrap();
    let bytes = std::fs::read(&object).unwrap();
    assert!(bytes
        .windows("あ😀".len())
        .any(|part| part == "あ😀".as_bytes()));
    hado::link_executable(&object, &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    std::fs::remove_file(object).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn string_equality_compares_contents_across_static_objects() {
    for (index, text, expected) in [(0, "あ😀", "1\n"), (1, "あ😁", "0\n")] {
        let source =
            format!("def equal text => text == \"あ😀\"\ndef main = if equal \"{text}\" => 1 | 0");
        let path = std::env::temp_dir().join(format!(
            "hado-string-equality-{}-{index}",
            std::process::id()
        ));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn derived_string_comparison_executes_lexicographic_order() {
    for (index, left, right, expected) in [
        (0, "あ", "あ😀", "-1\n"),
        (1, "あ😀", "あ😀", "0\n"),
        (2, "😀", "あ", "1\n"),
    ] {
        let source = format!("def main = compare \"{left}\" \"{right}\"");
        let ir = hado::dump_mono(&source, "t").unwrap();
        assert!(ir.contains("compare_String"));
        let path = std::env::temp_dir().join(format!(
            "hado-string-compare-{}-{index}",
            std::process::id()
        ));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn record_equality_uses_derived_field_comparisons() {
    for (index, value, expected) in [(0, 3, "1\n"), (1, 4, "0\n")] {
        let source = format!("type Inner = {{ count: Int }}\ntype Item = {{ inner: Inner; text: String }}\ndef main = if {{ inner = {{ count = 3 }}; text = \"あ\" }} == {{ text = \"あ\"; inner = {{ count = {value} }} }} => 1 | 0");
        let path = std::env::temp_dir().join(format!(
            "hado-record-equality-{}-{index}",
            std::process::id()
        ));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(hado::dump_mono(&source, "t").unwrap().contains("eq_Data"));
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn recursive_adt_equality_compares_tags_and_payloads() {
    for (index, left, right, expected) in [
        (0, "[1, 2, 3]", "[1, 2, 3]", "1\n"),
        (1, "[1, 2, 3]", "[1, 2, 4]", "0\n"),
        (2, "[1, 2]", "[1]", "0\n"),
        (3, "Some(3)", "None", "0\n"),
    ] {
        let source = format!("def main = if {left} == {right} => 1 | 0");
        let path =
            std::env::temp_dir().join(format!("hado-adt-equality-{}-{index}", std::process::id()));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn record_compare_uses_declaration_order() {
    for (index, x, y, expected) in [(0, 1, 9, "-1\n"), (1, 2, 1, "1\n")] {
        let source = format!("type Pair = {{ first: Int; second: Int }}\ndef main = compare {{ second = {y}; first = {x} }} {{ first = 2; second = 0 }}");
        let path = std::env::temp_dir().join(format!(
            "hado-record-compare-{}-{index}",
            std::process::id()
        ));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn adt_compare_uses_constructor_order_and_recursive_payloads() {
    for (index, source, expected) in [
        (
            0,
            "type Choice = | Zebra | Apple\ndef main = compare Zebra Apple",
            "-1\n",
        ),
        (1, "def main = compare [1, 2] [1, 2]", "0\n"),
        (2, "def main = compare [2] [1, 9]", "1\n"),
    ] {
        let path =
            std::env::temp_dir().join(format!("hado-adt-compare-{}-{index}", std::process::id()));
        hado::build::executable(source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn runtime_calls_keep_types_and_remain_non_tail() {
    use hado::mono::{Constant, Expression, Program};
    use hado::runtime_abi::RuntimeFunction;
    let text = |s: &str| Expression::Constant(Constant::String(s.into()));
    let joined =
        Expression::call_runtime(RuntimeFunction::StringConcat, vec![text("あ"), text("😀")])
            .unwrap();
    let entry = Expression::call_runtime(RuntimeFunction::StringLength, vec![joined]).unwrap();
    assert!(matches!(entry, Expression::CallRuntime { .. }));
    assert!(Expression::call_runtime(
        RuntimeFunction::StringLength,
        vec![Expression::Constant(Constant::Int(1))]
    )
    .is_err());
    let checked = hado::check_source("def main = 0", "t").unwrap();
    let path = std::env::temp_dir().join(format!("hado-runtime-call-{}", std::process::id()));
    let object = path.with_extension("o");
    hado::emit::compile_mono_to_object(
        &Program {
            entry,
            functions: vec![],
        },
        &checked,
        &object,
    )
    .unwrap();
    hado::link_executable(&object, &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"2\n");
    std::fs::remove_file(object).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn stdlib_intrinsics_connect_source_types_to_runtime_calls() {
    let source = "def main = __string_length (__string_concat \"あ\" \"😀\")";
    let path = std::env::temp_dir().join(format!("hado-intrinsics-{}", std::process::id()));
    hado::build::executable(source, "stdlib/test.hd", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"2\n");
    let ir = hado::dump_mono(source, "stdlib/test.hd").unwrap();
    assert!(ir.contains("CallRuntime"));
    assert!(!ir.contains("TailCall"));
    assert!(hado::check_source(source, "user.hd").is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn boolean_and_unit_main_use_derived_printers() {
    for (index, source, expected) in [
        (0, "def main = 1 < 2", "true\n"),
        (1, "def main = 2 < 1", "false\n"),
        (2, "def main = ()", ""),
    ] {
        let path =
            std::env::temp_dir().join(format!("hado-simple-print-{}-{index}", std::process::id()));
        hado::build::executable(source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn char_main_encodes_unicode_scalars_as_utf8() {
    for (index, character) in ['A', '😀'].iter().enumerate() {
        let source = format!("def main = '{character}'");
        let path =
            std::env::temp_dir().join(format!("hado-char-print-{}-{index}", std::process::id()));
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{character}\n").as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn float_main_passes_f64_values_to_the_runtime() {
    let path = std::env::temp_dir().join(format!("hado-float-print-{}", std::process::id()));
    hado::build::executable("def main = 1.5 * 2.5", "t", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"3.75\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn integer_to_string_preserves_signed_boundaries() {
    for (index, expression, expected) in [
        (0, "-42", "-42\n"),
        (1, "(-9223372036854775807 - 1)", "-9223372036854775808\n"),
    ] {
        let path =
            std::env::temp_dir().join(format!("hado-int-string-{}-{index}", std::process::id()));
        hado::build::executable(
            &format!("def main = __int_to_string ({expression})"),
            "stdlib/test.hd",
            &path,
        )
        .unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn float_to_string_preserves_numeric_values() {
    for (index, expression, expected) in [(0, "1.25", "1.25\n"), (1, "0.0", "0\n")] {
        let path =
            std::env::temp_dir().join(format!("hado-float-string-{}-{index}", std::process::id()));
        hado::build::executable(
            &format!("def main = __float_to_string ({expression})"),
            "stdlib/test.hd",
            &path,
        )
        .unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn char_to_string_preserves_utf8() {
    for (index, character) in ['A', '😀'].iter().enumerate() {
        let path =
            std::env::temp_dir().join(format!("hado-char-string-{}-{index}", std::process::id()));
        let source = format!("def main = __char_to_string '{character}'");
        hado::build::executable(&source, "stdlib/test.hd", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{character}\n").as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn enum_main_prints_the_selected_constructor() {
    for (index, variant) in ["Zebra", "Apple"].iter().enumerate() {
        let path =
            std::env::temp_dir().join(format!("hado-enum-print-{}-{index}", std::process::id()));
        let source = format!("type Choice = | Zebra | Apple\ndef main = {variant}");
        hado::build::executable(&source, "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{variant}\n").as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn result_main_selects_output_stream_and_exit_status() {
    for (index, expression, stdout, stderr, status) in [
        (0, "7 / 2", "3\n", "", 0),
        (1, "7 / 0", "", "DivisionByZero\n", 1),
    ] {
        let path =
            std::env::temp_dir().join(format!("hado-result-main-{}-{index}", std::process::id()));
        hado::build::executable(&format!("def main = {expression}"), "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(status));
        assert_eq!(output.stdout, stdout.as_bytes());
        assert_eq!(output.stderr, stderr.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn record_main_formats_fields_in_declaration_order() {
    let path = std::env::temp_dir().join(format!("hado-record-print-{}", std::process::id()));
    let source =
        "type Item = { count: Int; enabled: Bool }\ndef main = { enabled = true; count = 3 }";
    hado::build::executable(source, "t", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"{ count = 3; enabled = true }\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn option_main_formats_payload_and_nullary_variant() {
    for (index, expression, expected) in [
        (0, "Some(3)", "Some(3)\n"),
        (1, "if true => None | Some(3)", "None\n"),
    ] {
        let path =
            std::env::temp_dir().join(format!("hado-option-print-{}-{index}", std::process::id()));
        hado::build::executable(&format!("def main = {expression}"), "t", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn recursive_list_main_uses_finite_formatter_functions() {
    let source = "def main = [1, 2, 3]";
    let path = std::env::temp_dir().join(format!("hado-list-print-{}", std::process::id()));
    hado::build::executable(source, "t", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"[1, 2, 3]\n");
    let dump = hado::dump_mono(source, "t").unwrap();
    assert!(dump.contains("format_Data"));
    assert!(!dump.contains("unavailable"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn functions_can_reference_top_level_values() {
    let path = std::env::temp_dir().join(format!("hado-global-value-{}", std::process::id()));
    hado::build::executable(
        "def offset = 3\ndef add n => n + offset\ndef main = add 39",
        "t",
        &path,
    )
    .unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn module_value_thunks_preserve_lexical_scope() {
    let source = "def main = A.add 10 + B.add 20\nmodule S def add(x: Int): Int\n/module\nimplement S as A def offset = 2 def add x => x + offset\n/implement\nimplement S as B def offset = 5 def add x => x + offset\n/implement";
    let path = std::env::temp_dir().join(format!("hado-module-values-{}", std::process::id()));
    hado::build::executable(source, "t", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"37\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn polymorphic_global_function_values_specialize_the_thunk_result() {
    let source = "def identity = fn(x => x)\ndef main = if identity true => identity 42 | 0";
    let path = std::env::temp_dir().join(format!("hado-polymorphic-thunk-{}", std::process::id()));
    hado::build::executable(source, "t", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"42\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unit_output_prints_option_payloads_without_a_status_channel() {
    for (index, (expression, ty, stdout)) in [
        ("Some(())", "Void Option", "Some(())\n"),
        ("None", "Void Option", "None\n"),
    ]
    .into_iter()
    .enumerate()
    {
        let path =
            std::env::temp_dir().join(format!("hado-unit-option-{}-{index}", std::process::id()));
        hado::build::executable(
            &format!("def identity (x: {ty}): {ty} => x\ndef main = identity ({expression})"),
            "unit-print.hd",
            &path,
        )
        .unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, stdout.as_bytes());
        assert!(output.stderr.is_empty());
        std::fs::remove_file(path).unwrap();
    }
}
