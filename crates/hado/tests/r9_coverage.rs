use hado::check_source;

#[test]
fn unresolved_field_receivers_report_e304_at_the_reference_start() {
    for index in [0, 5] {
        let field = format!("member{index}");
        for declarations in [String::new(), format!("type A = {{ {field}: Int; left: Bool }}\ntype B = {{ {field}: Int; right: Bool }}\n")] {
            let prefix = format!("{declarations}def read x =>\n{}", " ".repeat(index));
            let source = format!("{prefix}x . {field}\ndef main = read");
            let error = check_source(&source, "field-receiver.hd").err().unwrap();
            assert_eq!(error.code.as_deref(), Some("E304"), "{error}");
            assert_eq!((error.line, error.col), (prefix.bytes().filter(|b| *b == b'\n').count() + 1, index + 1), "{error}");
        }
        let source = format!("type A = {{ {field}: Int; left: Bool }}\ntype B = {{ {field}: Int; right: Bool }}\ndef read x => x.{field}\ndef main = read {{ {field} = 1; left = true }} + {{ {field} = 2; right = false }}.{field}");
        check_source(&source, "resolved-field.hd").unwrap();
    }
}

#[test]
fn unresolved_field_diagnostics_follow_source_order() {
    for (first, second) in [("z", "a"), ("a", "z")] {
        let prefix = format!("def {first} x => ");
        let source = format!("{prefix}x.absent\ndef {second} x => x.missing\ndef main = 1");
        let error = check_source(&source, "field-order.hd").err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E304"), "{error}");
        assert_eq!((error.line, error.col), (1, prefix.len() + 1), "{error}");
    }
    let source = "type R = { known: Int }\ndef f x => x.absent\ndef g (y: R) => y.nope\ndef main = g { known = 1 }";
    let prefix = &source[..source.find("nope").unwrap()];
    let error = check_source(source, "field-stage.hd").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
    assert_eq!(
        (error.line, error.col),
        (
            prefix.bytes().filter(|b| *b == b'\n').count() + 1,
            prefix.rsplit('\n').next().unwrap().len() + 1
        ),
        "{error}"
    );
}

#[test]
fn catch_all_patterns_are_exhaustive_before_redundancy_checks() {
    let source = "def choose | true => 0 | _ => -1\ndef main = choose (true)";
    assert_eq!(
        check_source(source, "coverage.hd")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E503"),
        "{source}"
    );
}

#[test]
fn sole_parameter_branch_checks_all_four_conditions() {
    for source in [
        "def f value => if value | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def f value => if value | Some(value) => value + 1 | 0\ndef main = f (Some(1))",
        "def main = fn(value => if value | Some(x) => x + 1 | 0) (Some(1))",
    ] {
        assert_eq!(
            check_source(source, "coverage.hd")
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("E515")
        );
    }
    for source in [
        "def f value extra => if value | Some(x) => x + extra | extra\ndef main = f (Some(1)) 2",
        "def f (value: Int Option) => if value | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def f value: Int => if value | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def f value => if value | Some(_) => Option.is_some value | false\ndef main = f (Some(1))",
        "def f value => (if value | Some(x) => x | 0) + 1\ndef main = f (Some(1))",
        "def f | Some(x) => x + 1 | 0\ndef main = f (Some(1))",
        "def main = fn(| Some(x) => x + 1 | 0) (Some(1))",
    ] {
        check_source(source, "coverage.hd").unwrap();
    }
}

#[test]
fn let_patterns_must_cover_the_entire_payload_type() {
    let prefix = "def main = Some(1) as ";
    let source = format!("{prefix}Some(value) -> value + 1");
    let error = check_source(&source, "coverage.hd").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E402"));
    assert_eq!((error.line, error.col), (1, prefix.len() + 1));
    let source = "type Id = | Id(Int)\ndef main = Id(1) as Id(value) ->\n  value + 1";
    check_source(source, "coverage.hd").unwrap();
}

#[test]
fn do_requires_a_rail_and_records_require_a_candidate() {
    let source = "def main = 0 -> Some(1)";
    assert_eq!(
        check_source(source, "coverage.hd")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E303")
    );
    let source = "def main = { missing = 1 }";
    assert_eq!(
        check_source(source, "coverage.hd")
            .err()
            .unwrap()
            .code
            .as_deref(),
        Some("E304")
    );
}

#[test]
fn unused_bindings_include_parameters_and_nested_patterns() {
    let error = check_source("def f _ x => x + 1\ndef main = f 1 2", "coverage.hd")
        .err()
        .unwrap();
    assert_eq!(error.code.as_deref(), Some("E519"));
    assert_eq!((error.line, error.col), (1, 7));
    for (source, name) in [
        ("def f used unused => used + 1\ndef main = f 1 2", "unused"),
        ("def main = fn(used, unused => used + 1) 1 2", "unused"),
        ("def f outer => 1 as outer ->\n  outer + 2\ndef main = f 1", "outer"),
        ("def f | Some(unused) => 1 | 0\ndef main = f (Some(1))", "unused"),
        ("type Pair = { left: Int; right: Int }\ndef f (pair: Pair) => if pair | { left; right } => left\ndef main = f { left = 1; right = 2 }", "right }"),
    ] {
        let expected_offset = source.find(name).unwrap();
        let before = &source[..expected_offset];
        let expected_line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let expected_col = before.rsplit('\n').next().unwrap().len() + 1;
        let error = check_source(source, "coverage.hd").err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E505"), "{source}");
        assert_eq!((error.line, error.col), (expected_line, expected_col));
    }
    for source in [
        "def f | Some(_) => 1 | 0\ndef main = f (Some(1))",
        "def f x => fn(y => x + y) 1\ndef main = f 1",
        "def f | Some(x) => fn(y => x + y) 1 | 0\ndef main = f (Some(1))",
    ] {
        check_source(source, "coverage.hd").unwrap();
    }
}

#[test]
fn record_patterns_resolve_the_whole_field_set() {
    let source = "def f value => value as { absent } ->\n  absent + 1\ndef main = f 1";
    let error = check_source(source, "coverage.hd").err().unwrap();
    assert_eq!(error.code.as_deref(), Some("E304"));
    assert_eq!(error.col, source.find('{').unwrap() + 1);
    let source = "type A = { x: Int; y: Int }\ntype B = { x: Int; z: Int }\ntype C = { y: Int; z: Int }\ndef f value => value as { x; y } ->\n  x + y\ndef main = f { x = 1; y = 2 } + { x = 1; z = 2 }.z + { y = 1; z = 2 }.z";
    check_source(source, "coverage.hd").unwrap();
}

#[test]
fn discarded_rail_binding_points_to_the_underscore() {
    for padding in [1, 5] {
        let prefix = format!("def main = Some(1) as{}", " ".repeat(padding + 1));
        let source = format!("{prefix}_? -> Some(2)");
        let error = check_source(&source, "coverage.hd").err().unwrap();
        assert_eq!(error.code.as_deref(), Some("E504"));
        assert_eq!((error.line, error.col), (1, prefix.len() + 1));
    }
}
