use hado::{check_source, parser};

fn random(state: &mut u64, limit: u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    (*state >> 32) % limit
}

fn rejected(source: &str, code: &str, anchor: &str) {
    let error = check_source(source, "rails.hd").err().unwrap();
    let prefix = &source[..source.find(anchor).unwrap()];
    assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
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
fn generated_patterns_round_trip_with_an_independent_question_token() {
    let name = "value1".to_string();
    for pattern in [
        name.clone(),
        format!("{{ field = {name} }}"),
        format!("Box({name})"),
        format!("{{ field = Box({name}) }}"),
    ] {
        let source = format!("def f input => input as {pattern}? -> Some({name})");
        let ast = parser::parse_source(&source, "rails.hd").unwrap();
        let printed = parser::canonical_program(&ast);
        assert!(!printed.contains("let?"), "{printed}");
        assert!(printed.contains(&format!("as {pattern}? ->")), "{printed}");
        let restored = parser::parse_source(&printed, "rails.hd").unwrap();
        assert_eq!(parser::canonical_program(&restored), printed);
        let spaced = parser::parse_source(&source.replace("? ->", " ? ->"), "rails.hd").unwrap();
        assert_eq!(parser::canonical_program(&spaced), printed);
    }
}

#[test]
fn old_rail_keywords_are_rejected_at_the_question_mark() {
    for prefix in ["", "\n\n  "] {
        for (body, keyword) in [
            ("let? x = Some(1) -> Some(x + 1)", "let"),
            ("do? Some(1) -> Some(2)", "do"),
        ] {
            let source = format!("{prefix}def main = {body}");
            let error = parser::parse_source(&source, "rails.hd").unwrap_err();
            let before = &source[..source.find(keyword).unwrap()];
            assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
            assert_eq!(
                (error.line, error.col),
                (
                    before.matches('\n').count() + 1,
                    before.rsplit('\n').next().unwrap().chars().count() + 1
                )
            );
        }
    }
}

#[test]
fn generated_rail_errors_preserve_stage_and_pattern_anchors() {
    let mut state = 0x14303504;
    for _ in 0..2 {
        let n = random(&mut state, 1000);
        for (body, code, anchor) in [
            (format!("{n} as x? -> Some(x + 1)"), "E303", n.to_string()),
            (format!("{n} as _? -> Some(1)"), "E303", n.to_string()),
            (format!("{n} -> Some(1)"), "E303", n.to_string()),
            (
                format!("Some(Some({n})) as Some(x)? -> Some(x + 1)"),
                "E402",
                "Some(x)".to_string(),
            ),
            (
                format!("Some(Some({n})) as Some(Some(x)) -> x + 1"),
                "E402",
                "Some(Some(x))".to_string(),
            ),
        ] {
            let padding = format!(
                "{}{}",
                "\n".repeat(random(&mut state, 4) as usize),
                " ".repeat(random(&mut state, 7) as usize)
            );
            rejected(&format!("{padding}def main = {body}"), code, &anchor);
        }
    }
}

/// §54 makes an Int main the exit code rather than a printed line. Ask the
/// compiler for the entry's main type and, when it is Int, wrap the trailing
/// body in `Int.to_string` so the stdout assertions keep measuring the value.
fn wrap_entry(path: &std::path::Path) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_hado"))
        .args(["check", "--print-type"])
        .arg(path)
        .output()
        .unwrap();
    // `check --print-type` renders an effectful Int as `Int {…}`; the effect
    // row does not change §54's rule, so any Int spelling counts.
    let ty = String::from_utf8_lossy(&output.stdout);
    let ty = ty.trim();
    if !output.status.success() || !(ty == "Int" || ty.starts_with("Int {")) {
        return;
    }
    let source = std::fs::read_to_string(path).unwrap();
    let marker = "def main =";
    let Some(at) = source.rfind(marker) else {
        return;
    };
    let after = source[at + marker.len()..].trim_start_matches([' ', '\t']);
    let mut cut = after.len();
    let mut start = after.find('\n').map(|i| i + 1);
    while let Some(begin) = start {
        let end = after[begin..]
            .find('\n')
            .map(|i| begin + i)
            .unwrap_or(after.len());
        let line = &after[begin..end];
        if !(line.is_empty() || line.starts_with([' ', '\t']) || line.starts_with('/')) {
            cut = begin;
            break;
        }
        start = if end < after.len() {
            Some(end + 1)
        } else {
            None
        };
    }
    std::fs::write(
        path,
        format!(
            "{}def main = Int.to_string ({}){}",
            &source[..at],
            &after[..cut],
            &after[cut..]
        ),
    )
    .unwrap();
}

fn execute(source: &str, expected: &str, serial: usize) {
    let (status, stdout, stderr) = run(source, serial);
    assert_eq!(status, Some(0), "{source}");
    assert_eq!(stdout, format!("{expected}\n"));
    assert!(stderr.is_empty(), "{source}: {stderr}");
}

fn run(source: &str, serial: usize) -> (Option<i32>, String, String) {
    let dir = std::env::temp_dir().join(format!("hado-r14-{}-{serial}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    std::fs::write(&path, source).unwrap();
    wrap_entry(&path);
    let build = std::process::Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("build")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{source}\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let result = std::process::Command::new(path.with_extension(""))
        .output()
        .unwrap();
    let stdout = String::from_utf8(result.stdout).unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    let status = result.status.code();
    std::fs::remove_dir_all(dir).unwrap();
    (status, stdout, stderr)
}

#[test]
fn generated_chains_execute_success_and_each_failure_path() {
    let mut state = 0x144;
    let n = random(&mut state, 1000) + 1;
    for fail in 0..4 {
        let source = format!(
            "type Box = | Box(Int)\ntype Entry = {{ field: Box }}\n\
             def first (flag: Int): Entry {{Error}} => if flag == 1 => raise {{ message = \"f{n}\" }} | {{ field = Box({n}) }}\n\
             def step (flag: Int, stage: Int): Void {{Error}} => if flag == stage => raise {{ message = \"s{n}-{fail}\" }} | ()\n\
             def run flag => first flag as {{ field = Box(x) }} -> step flag 2 -> step flag 3 -> x + 5\n\
             def main = run {fail}"
        );
        let (status, stdout, stderr) = run(&source, fail);
        match fail {
            0 => {
                assert_eq!(status, Some(0), "{source}");
                assert_eq!(stdout, format!("{}\n", n + 5));
                assert!(stderr.is_empty(), "{stderr}");
            }
            1 => {
                assert_eq!(status, Some(1), "{source}");
                assert!(stdout.is_empty(), "{stdout}");
                assert_eq!(stderr, format!("f{n}\n"));
            }
            _ => {
                assert_eq!(status, Some(1), "{source}");
                assert!(stdout.is_empty(), "{stdout}");
                assert_eq!(stderr, format!("s{n}-{fail}\n"));
            }
        }
    }
}

#[test]
fn generated_option_chains_propagate_none_at_each_step() {
    let mut state = 0x1407;
    let n = random(&mut state, 1000) + 1;
    for fail in 0..4 {
        // 破棄の段は `?` の束縛を使う。`e ->` は効果専用で、`Option` を伝播しない
        // （第35節、E303）。
        let source = format!(
            "def first flag => if flag == 1 => None | Some({n})\n\
             def step flag stage => if flag == stage => None | Some(())\n\
             def run flag => first flag as x? -> step flag 2 as _? -> step flag 3 as _? -> Some(x + 5)\n\
             def unwrap | Some(x) => x | None => 0\n\
             def main = unwrap (run {fail})"
        );
        let expected = if fail == 0 { n + 5 } else { 0 };
        // 同じ binary の他の連鎖テストと作業ディレクトリを分ける（`run` は
        // `serial` ごとに一時ディレクトリを作る）。
        execute(&source, &expected.to_string(), 200 + fail);
    }
}

#[test]
fn generated_absent_chains_answer_none_at_each_step() {
    let mut state = 0x1407;
    let n = random(&mut state, 1000) + 1;
    for fail in 0..4 {
        let source = format!(
            "def first (flag: Int): Int {{Absent}} => if flag == 1 => absent | {n}\n\
             def step (flag: Int, stage: Int): Void {{Absent}} => if flag == stage => absent | ()\n\
             def run flag => first flag as x -> step flag 2 -> step flag 3 -> x + 5\n\
             def unwrap | Some(x) => x | None => 0\n\
             def main = unwrap (with Maybe -> run {fail})"
        );
        let expected = if fail == 0 { n + 5 } else { 0 };
        execute(&source, &expected.to_string(), 100 + fail);
    }
}
