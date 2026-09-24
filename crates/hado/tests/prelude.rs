#[test]
fn scalar_prelude_wrappers_check_and_execute() {
    // list.hd の `type T List` を下の型宣言の照合で見る。
    let modules = [
        include_str!("../../../stdlib/error.hd"),
        include_str!("../../../stdlib/list.hd"),
        include_str!("../../../stdlib/string.hd"),
        include_str!("../../../stdlib/int.hd"),
        include_str!("../../../stdlib/float.hd"),
    ]
    .join("\n");
    let type_declarations = modules
        .lines()
        .filter(|line| line.trim_start().starts_with("type "))
        .collect::<Vec<_>>();
    assert_eq!(
        type_declarations,
        [
            "type Exception = { message: String }",
            "type T List = | Nil | Cons(T, T List)",
        ],
        "the type declarations the language provides by name may be excluded"
    );
    let modules = modules
        .lines()
        .filter(|line| !line.starts_with("type "))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("def main = Float.to_string 1.25\n{modules}");
    let path = std::env::temp_dir().join(format!("hado-prelude-{}-2", std::process::id()));
    hado::build::executable(&source, "stdlib/test.hd", &path).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"1.25\n");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn scalar_prelude_is_implicit_without_exposing_intrinsics() {
    let path = std::env::temp_dir().join(format!("hado-implicit-prelude-{}", std::process::id()));
    hado::build::executable(
        "def main = String.length (Int.to_string 12345)",
        "user.hd",
        &path,
    )
    .unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"5\n");
    assert!(hado::check_source("def main = __string_length \"a\"", "user.hd").is_err());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn prelude_is_attached_when_a_directory_named_stdlib_is_on_the_path() {
    // A user project may live under a folder named `stdlib`. That is not the
    // language's stdlib/; §55 still concatenates prelude and `__` stays E202.
    let source = "def main = List.length [1, 2, 3]\n";
    assert!(
        hado::check_source(source, "/tmp/stdlib/app/main.hd").is_ok(),
        "an absolute path with a stdlib directory is still a user entry"
    );
    let error = hado::check_source(
        "def main = __string_length \"hi\"",
        "/tmp/stdlib/app/main.hd",
    )
    .err()
    .expect("user code under a stdlib directory cannot call __");
    assert_eq!(error.code.as_deref(), Some("E202"));

    let dir = std::env::temp_dir().join(format!("stdlib/hado-user-project-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    std::fs::write(&path, source).unwrap();
    let project = hado::project::Project::load(&path).expect("load under stdlib");
    project
        .check()
        .expect("prelude List is in scope for a project under stdlib");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn generic_prelude_operations_execute() {
    let cases = [
        (
            "Option.unwrap_or (Option.map (Some(41)) fn(x => x + 1)) 0",
            "42\n",
        ),
        ("Option.is_some (Some(true))", "true\n"),
        ("Option.ok_or (Some(42)) { message = \"missing\" }", "42\n"),
        ("7 / 2", "3\n"),
    ];
    for (index, (expression, expected)) in cases.iter().enumerate() {
        let path = std::env::temp_dir().join(format!(
            "hado-generic-prelude-{}-{index}",
            std::process::id()
        ));
        hado::build::executable(&format!("def main = {expression}"), "user.hd", &path).unwrap();
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
    ]
    .iter()
    .enumerate()
    {
        let path =
            std::env::temp_dir().join(format!("hado-float-order-{}-{index}", std::process::id()));
        hado::build::executable(&format!("def main = {expression}"), "user.hd", &path).unwrap();
        let output = std::process::Command::new(&path).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes(), "{expression}");
        std::fs::remove_file(path).unwrap();
    }
}
