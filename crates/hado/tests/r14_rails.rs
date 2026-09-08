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
        (prefix.matches('\n').count() + 1,
         prefix.rsplit('\n').next().unwrap().chars().count() + 1),
        "{source}: {error}"
    );
}

#[test]
fn generated_patterns_round_trip_with_an_independent_question_token() {
    let mut state = 0x140012;
    for _ in 0..24 {
        let name = format!("value{}", random(&mut state, 10000));
        for pattern in [name.clone(), format!("{{ field = {name} }}"),
            format!("Box({name})"), format!("{{ field = Box({name}) }}")] {
            let source = format!("def f input => let {pattern}? = input -> Some({name})");
            let ast = parser::parse_source(&source, "rails.hd").unwrap();
            let printed = parser::canonical_program(&ast);
            assert!(!printed.contains("let?"), "{printed}");
            assert!(printed.contains("? ="), "{printed}");
            let restored = parser::parse_source(&printed, "rails.hd").unwrap();
            assert_eq!(parser::canonical_program(&restored), printed);
            let spaced = parser::parse_source(&source.replace("? =", " ? ="), "rails.hd").unwrap();
            assert_eq!(parser::canonical_program(&spaced), printed);
        }
    }
}

#[test]
fn old_rail_keywords_are_rejected_at_the_question_mark() {
    for prefix in ["", "\n\n  "] {
        for body in ["let? x = Some(1) -> Some(x + 1)", "do? Some(1) -> Some(2)"] {
            let source = format!("{prefix}def main = {body}");
            let error = parser::parse_source(&source, "rails.hd").unwrap_err();
            let before = &source[..source.find('?').unwrap()];
            assert_eq!(error.code.as_deref(), Some("E100"), "{error}");
            assert_eq!((error.line, error.col), (before.matches('\n').count() + 1,
                before.rsplit('\n').next().unwrap().chars().count() + 1));
        }
    }
}

#[test]
fn generated_rail_errors_preserve_stage_and_pattern_anchors() {
    let mut state = 0x14303504;
    for _ in 0..16 {
        let n = random(&mut state, 1000);
        for (body, code, anchor) in [
            (format!("let x? = {n} -> Some(x + 1)"), "E303", n.to_string()),
            (format!("let _? = {n} -> Some(1)"), "E303", n.to_string()),
            (format!("do {n} -> Some(1)"), "E303", n.to_string()),
            (format!("let _? = Some({n}) -> Some(1)"), "E504", "_".to_string()),
            (format!("let _? = Ok({n}) -> Ok(1)"), "E504", "_".to_string()),
            (format!("let Some(x)? = Some(Some({n})) -> Some(x + 1)"), "E402", "Some(x)".to_string()),
        ] {
            let padding = format!("{}{}", "\n".repeat(random(&mut state, 4) as usize),
                " ".repeat(random(&mut state, 7) as usize));
            rejected(&format!("{padding}def main = {body}"), code, &anchor);
        }
    }
}

fn execute(source: &str, expected: &str, serial: usize) {
    let dir = std::env::temp_dir().join(format!("hado-r14-{}-{serial}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    std::fs::write(&path, source).unwrap();
    let build = std::process::Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("build").arg(&path).output().unwrap();
    assert!(build.status.success(), "{source}\n{}", String::from_utf8_lossy(&build.stderr));
    let result = std::process::Command::new(path.with_extension("")).output().unwrap();
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), format!("{expected}\n"));
    assert!(result.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn generated_chains_execute_success_and_each_failure_path() {
    let mut state = 0x144;
    for serial in 0..8 {
        let n = random(&mut state, 1000) + 1;
        for fail in 0..4 {
            let source = format!(
                "type Box = | Box(Int)\ntype Entry = {{ field: Box }}\n\
                 def first (flag: Int): Result<Entry, Int> => if flag == 1 => Error({n}) | Ok({{ field = Box({n}) }})\n\
                 def step (flag: Int, stage: Int): Result<Unit, Int> => if flag == stage => Error({n} + stage) | Ok()\n\
                 def run flag => let {{ field = Box(x) }}? = first flag -> do step flag 2 -> do step flag 3 -> let Box(y)? = Ok(Box(x + 4)) -> Ok(y + 1)\n\
                 def unwrap | Ok(x) => x | Error(e) => 0 - e\n\
                 def main = unwrap (run {fail})"
            );
            let expected = match fail { 0 => n as i64 + 5, 1 => -(n as i64), _ => -(n as i64 + fail as i64) };
            execute(&source, &expected.to_string(), serial * 4 + fail);
        }
    }
}

#[test]
fn generated_option_chains_propagate_none_at_each_step() {
    let mut state = 0x1407;
    for serial in 0..8 {
        let n = random(&mut state, 1000) + 1;
        for fail in 0..4 {
            let source = format!(
                "def first flag => if flag == 1 => None | Some({n})\n\
                 def step flag stage => if flag == stage => None | Some(())\n\
                 def run flag => let x? = first flag -> do step flag 2 -> do step flag 3 -> Some(x + 5)\n\
                 def unwrap | Some(x) => x | None => 0\n\
                 def main = unwrap (run {fail})"
            );
            let expected = if fail == 0 { n + 5 } else { 0 };
            execute(&source, &expected.to_string(), 100 + serial * 4 + fail);
        }
    }
}
