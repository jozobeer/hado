use hado::{check_source, parser};

fn random(state: &mut u64, limit: u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state >> 32) % limit
}

fn list_expression(state: &mut u64, depth: usize) -> String {
    if depth == 0 {
        return random(state, 1000).to_string();
    }
    let count = random(state, 5) as usize;
    let values = (0..count)
        .map(|_| list_expression(state, depth - 1))
        .collect::<Vec<_>>();
    if count > 1 && random(state, 2) == 1 {
        return format!(
            "[{}, ...[{}]]",
            values[..count - 1].join(", "),
            values[count - 1]
        );
    }
    format!("[{}]", values.join(", "))
}

fn bodies(ast: &parser::Ast) -> Vec<&parser::Node> {
    ast.items
        .iter()
        .filter_map(|item| match item {
            parser::Item::Def(def) => Some(match &def.kind {
                parser::DefKind::Value(body) | parser::DefKind::Function { body, .. } => body,
            }),
            _ => None,
        })
        .collect()
}

fn assert_round_trip(source: &str) {
    let original = parser::parse_source(source, "round.hd").unwrap();
    let printed = parser::canonical_program(&original);
    let restored = parser::parse_source(&printed, "round.hd").unwrap();
    assert!(
        !printed.contains("Cons") && !printed.contains("Nil"),
        "{printed}"
    );
    assert_eq!(bodies(&original), bodies(&restored), "{source}\n{printed}");
    assert_eq!(parser::canonical_program(&restored), printed);
}

fn rejected(source: &str, code: &str, anchor: &str) {
    let error = check_source(source, "generated.hd").err().unwrap();
    assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
    let prefix = &source[..source.find(anchor).unwrap()];
    assert_eq!(
        (error.line, error.col),
        (
            prefix.matches('\n').count() + 1,
            prefix.rsplit('\n').next().unwrap().chars().count() + 1
        ),
        "{source}: {error}"
    );
}

#[test]
fn generated_lists_round_trip_with_canonical_patterns() {
    let mut state = 0x1246;
    for (depth, width) in [(1usize, 1usize), (2, 1), (3, 1), (1, 6), (3, 6)] {
        let pattern = format!("[{}, ...tail]", vec!["[_, ..._]"; width].join(", "));
        let source = format!(
            "def probe | {pattern} => 1 | 0\ndef main = {}",
            list_expression(&mut state, depth)
        );
        assert_round_trip(&source);
    }
}

#[test]
fn list_coverage_uses_lengths_and_nested_rest_patterns() {
    for prefix in ["", "\n "] {
        let source = format!("{prefix}def f | [] => 0 | [_, ..._] => 1\ndef main = f [7]");
        check_source(&source, "coverage.hd").unwrap();
        rejected(
            &format!("{prefix}def f | [] => 0 | [_] => 1\ndef main = f [7]"),
            "E402",
            "def f",
        );
        rejected(
            &format!(
                "{prefix}def f | [_, ..._] => 1 | [_, _, ..._] => 2 | [] => 0\ndef main = f [7]"
            ),
            "E403",
            "| [_, _,",
        );
        rejected(
            &format!("{prefix}def f | [] => 0 | [] => 1 | [_, ..._] => 2\ndef main = f [7]"),
            "E405",
            "| [] => 1",
        );
    }
}

#[test]
fn written_list_constructors_report_original_name_positions() {
    for prefix in ["", "\n "] {
        for (body, name) in [
            ("Cons(7, [])", "Cons"),
            ("Some(Nil)", "Nil"),
            ("[Cons(7, [])]", "Cons"),
        ] {
            rejected(&format!("{prefix}def main = {body}"), "E510", name);
        }
        for (arms, input, name) in [
            ("Nil => 0 | [_, ..._] => 1", "[7]", "Nil"),
            ("Cons(_, _) => 1 | [] => 0", "[7]", "Cons"),
            (
                "Some(Nil) => 0 | Some([_, ..._]) => 1 | None => 2",
                "Some([7])",
                "Nil",
            ),
            (
                "[Cons(_, _)] => 1 | [] => 0 | [_, ..._] => 2",
                "[[7]]",
                "Cons",
            ),
        ] {
            rejected(
                &format!("{prefix}def f | {arms}\ndef main = f ({input})"),
                "E516",
                name,
            );
        }
    }
}

#[test]
fn invalid_spreads_are_syntax_errors_on_both_sides() {
    for form in ["[...xs]", "[1, ...xs, 2]", "[1, ...xs, ...xs]"] {
        let error = parser::parse_source(&format!("def f xs => {form}"), "invalid.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"), "{form}: {error}");
    }
    for form in [
        "[...xs]",
        "[_, ...xs, _]",
        "[_, ...[]]",
        "[_, ...Some(x)]",
        "[_, ...1]",
    ] {
        let error =
            parser::parse_source(&format!("def f | {form} => 1 | 0"), "invalid.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"), "{form}: {error}");
    }
}

fn execute(source: &str, expected: &str, serial: usize) {
    let dir = std::env::temp_dir().join(format!("hado-r12-{}-{serial}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("probe.hd");
    std::fs::write(&path, source).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("build")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result = std::process::Command::new(path.with_extension(""))
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        format!("{expected}\n")
    );
    assert!(result.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn spread_expressions_execute_with_independently_computed_output() {
    for (serial, (values, head)) in [
        (Vec::<String>::new(), "1".to_string()),
        (
            ["2", "3", "4"].into_iter().map(str::to_string).collect(),
            "5".to_string(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let expected = format!(
            "[{}]",
            std::iter::once(head.clone())
                .chain(values.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
        execute(
            &format!(
                "def prepend xs value => [value, ...xs]\ndef main = [0, ...prepend [{}] {head}]",
                values.join(", ")
            ),
            &format!("[0, {}", &expected[1..]),
            serial,
        );
    }
}

#[test]
fn list_decomposition_and_reconstruction_remains_redundant() {
    rejected(
        "def f | [] => [] | [head, ...tail] => [head, ...tail]\ndef main = f [1]",
        "E507",
        "def f",
    );
}
