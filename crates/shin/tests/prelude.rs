#[test]
fn scalar_prelude_wrappers_check_and_execute() {
    let modules = [
        include_str!("../../../stdlib/string.shin"),
        include_str!("../../../stdlib/int.shin"),
        include_str!("../../../stdlib/float.shin"),
    ].join("\n");
    for (index, expression, expected) in [
        (0, "String.length (String.concat \"あ\" \"😀\")", "2\n"),
        (1, "Int.to_string (-42)", "-42\n"),
        (2, "Float.to_string 1.25", "1.25\n"),
    ] {
        let source = format!("{modules}\ndef main = {expression}");
        let path = std::env::temp_dir().join(format!("shin-prelude-{}-{index}", std::process::id()));
        shin::build::executable(&source, "stdlib/test.shin", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn scalar_prelude_is_implicit_without_exposing_intrinsics() {
    let path = std::env::temp_dir().join(format!("shin-implicit-prelude-{}", std::process::id()));
    shin::build::executable("def main = String.length (Int.to_string 12345)", "user.shin", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"5\n");
    assert!(shin::check_source("def main = __string_length \"a\"", "user.shin").is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn generic_prelude_operations_execute() {
    let cases = [
        ("List.map [1, 2] (fn x => x + 1)", "[2, 3]\n"),
        ("List.filter [1, 2, 3] (fn x => x > 1)", "[2, 3]\n"),
        ("List.fold [1, 2, 3] 0 (fn sum value => sum + value)", "6\n"),
        ("List.reverse [1, 2, 3]", "[3, 2, 1]\n"),
        ("List.append [1, 2] [3]", "[1, 2, 3]\n"),
        ("[3, 1, 2, 1] |> List.sort compare", "[1, 1, 2, 3]\n"),
        ("Option.unwrap_or (Option.map (Some(41)) (fn x => x + 1)) 0", "42\n"),
        ("Option.is_some (Some(true))", "true\n"),
        ("Option.ok_or (Some(42)) \"missing\"", "42\n"),
        ("Result.map (7 / 2) (fn x => x + 1)", "4\n"),
        ("Result.is_ok (Result.map_error (7 / 0) (fn | DivisionByZero => \"division failed\"))", "false\n"),
    ];
    for (index, (expression, expected)) in cases.iter().enumerate() {
        let path = std::env::temp_dir().join(format!("shin-generic-prelude-{}-{index}", std::process::id()));
        shin::build::executable(&format!("def main = {expression}"), "user.shin", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success(), "{expression}: {output:?}");
        assert_eq!(output.stdout, expected.as_bytes(), "{expression}");
        assert!(output.stderr.is_empty());
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn float_comparison_orders_nan_last_and_equates_signed_zero() {
    for (index, (expression, expected)) in [
        ("compare (0.0 / 0.0) (0.0 / 0.0)", "0\n"),
        ("compare (0.0 / 0.0) 1.0", "1\n"),
        ("compare 1.0 (0.0 / 0.0)", "-1\n"),
        ("compare (-0.0) 0.0", "0\n"),
        ("compare (-1.0 / 0.0) (1.0 / 0.0)", "-1\n"),
    ].iter().enumerate() {
        let path = std::env::temp_dir().join(format!("shin-float-order-{}-{index}", std::process::id()));
        shin::build::executable(&format!("def main = {expression}"), "user.shin", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes(), "{expression}");
        std::fs::remove_file(path).unwrap();
    }
}
