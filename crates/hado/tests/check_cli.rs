use std::process::Command;

#[test]
fn cli_usage_and_basic_types() {
    for args in [vec![], vec!["check"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_hado"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"usage: hado check [--print-type[=<name>]] [--fix] <file> | dump-mono <file> | build [-o <path>] [--fix] <file.hd> | run <file.hd> [-- <arg>...] | fmt [--check] <file.hd> | test <file.hd>\n");
    }
    let dir = std::env::temp_dir().join(format!("hado-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    for (source, ty) in [
        ("def main = 1 + 2", "Int"),
        ("def main = 1.5 * 2.0", "Float"),
        ("def main = not (1 < 2)", "Bool"),
    ] {
        std::fs::write(&path, source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_hado"))
            .args(["check", "--print-type"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(output.stdout, format!("{ty}\n").as_bytes());
        assert!(output.stderr.is_empty());
        let output = Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg("check")
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    std::fs::write(&path, "\ndef main = 1 + true").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with(&format!(
            "{}:2:12: error[E305]:",
            path.file_name().unwrap().to_string_lossy()
        )),
        "{stderr}"
    );
    assert_eq!(stderr.lines().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn float_remainder_reports_a_positioned_type_error() {
    let error = hado::check_source("def main = 1.0 % 2.0", "float.hd")
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "float.hd:1:12: error[E305]: Float has no remainder operator"
    );
}

#[test]
fn cli_rejects_noncanonical_identity_binding() {
    let dir = std::env::temp_dir().join(format!("hado-canonical-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    std::fs::write(&path, "def main = let x = 1 -> x").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "{}:1:12: error[E501]: binding x is returned unchanged at 1:25; canonical form: 1\n",
            path.file_name().unwrap().to_string_lossy()
        )
    );
}

#[test]
fn reject_corpus_reports_declared_codes_through_cli() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reject");
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path().join("main.hd");
        if !path.is_file() {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let expected = source
            .lines()
            .find_map(|line| line.strip_prefix("// error: "))
            .unwrap()
            .trim();
        let output = Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg("check")
            .arg(&path)
            .output()
            .unwrap();
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        let baseline = match hado::project::Project::load(&path) {
            Ok(project) => project.report(
                &project
                    .check()
                    .err()
                    .expect("reject corpus passed library checker"),
            ),
            Err(error) => error.to_string(),
        };
        assert_eq!(
            diagnostic,
            format!("{baseline}\n"),
            "project diagnostic changed: {}",
            path.display()
        );
        let positioned = diagnostic
            .split_once(':')
            .filter(|(file, _)| {
                !file.is_empty()
                    && std::path::Path::new(file).is_relative()
                    && !std::path::Path::new(file)
                        .components()
                        .any(|part| matches!(part, std::path::Component::ParentDir))
            })
            .map(|(_, positioned)| positioned);
        let valid_position_and_code = positioned.is_some_and(|positioned| {
            let mut parts = positioned.splitn(3, ':');
            let line = parts.next().and_then(|value| value.parse::<usize>().ok());
            let col = parts.next().and_then(|value| value.parse::<usize>().ok());
            line.is_some_and(|line| line > 0)
                && col.is_some_and(|col| col > 0)
                && parts
                    .next()
                    .is_some_and(|message| message.starts_with(&format!(" error[{expected}]: ")))
        });
        if output.status.code() != Some(1)
            || !output.stdout.is_empty()
            || diagnostic.lines().count() != 1
            || !valid_position_and_code
        {
            failures.push(format!(
                "{} expected {expected}: status={:?}, stdout={:?}, stderr={diagnostic:?}",
                path.display(),
                output.status.code(),
                output.stdout
            ));
        }
        checked += 1;
    }
    assert!(checked > 0);
    failures.sort();
    assert!(
        failures.is_empty(),
        "{} failures in {checked} cases:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn dump_mono_exposes_typed_arithmetic() {
    let dir = std::env::temp_dir().join(format!("hado-mono-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    std::fs::write(&path, "def main = 1 + 2").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("dump-mono")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with(
        "fn main() -> Int\nBinary { op: Add, left: Constant(Int(1)), right: Constant(Int(2)) }\n"
    ));
    assert!(text.contains("name: \"print_Int\""));
}

#[derive(Debug, PartialEq, Eq)]
struct Rejection {
    file: String,
    line: usize,
    column: usize,
    code: String,
}

fn positioned_rejection(path: &std::path::Path, code: &str) -> Result<Rejection, String> {
    let output = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("check")
        .arg(path)
        .output()
        .unwrap();
    let text = String::from_utf8(output.stderr).unwrap();
    if output.status.code() != Some(1) || !output.stdout.is_empty() || text.lines().count() != 1 {
        return Err(format!(
            "invalid rejection: {:?} {text}",
            output.status.code()
        ));
    }
    let (file, rest) = text.split_once(':').ok_or_else(|| text.clone())?;
    let relative = std::path::Path::new(file);
    if file.is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
        || !path.parent().unwrap().join(relative).is_file()
    {
        return Err(text);
    }
    let mut parts = rest.splitn(3, ':');
    let line = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| text.clone())?;
    let column = parts
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| text.clone())?;
    if line == 0
        || column == 0
        || !parts
            .next()
            .is_some_and(|s| s.starts_with(&format!(" error[{code}]: ")))
    {
        return Err(text);
    }
    Ok(Rejection {
        file: file.into(),
        line,
        column,
        code: code.into(),
    })
}

fn copy_case_sources(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let path = entry.unwrap().path();
        let target = destination.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_case_sources(&path, &target);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("hd" | "hdi")
        ) {
            std::fs::copy(path, target).unwrap();
        }
    }
}

#[test]
fn every_reject_preserves_code_and_column_after_header_line_insertion() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reject");
    let dir = std::env::temp_dir().join(format!("hado-reject-shift-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for entry in std::fs::read_dir(corpus).unwrap() {
        let case = entry.unwrap().path();
        let original = case.join("main.hd");
        if !original.is_file() {
            continue;
        }
        let source = std::fs::read_to_string(&original).unwrap();
        let (header, code) = source
            .lines()
            .enumerate()
            .find_map(|(i, line)| {
                line.strip_prefix("// error:")
                    .map(|code| (i + 1, code.trim()))
            })
            .unwrap();
        let copied = dir.join(case.file_name().unwrap());
        copy_case_sources(&case, &copied);
        let path = copied.join("main.hd");
        let baseline = positioned_rejection(&original, code);
        let copied_baseline = positioned_rejection(&path, code);
        if copied_baseline != baseline || baseline.is_err() {
            failures.push(format!(
                "{code}: copy changed diagnostic: original {baseline:?}, copy {copied_baseline:?}"
            ));
        }
        let shifted = source
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{line}\n{}", if i + 1 == header { "\n" } else { "" }))
            .collect::<String>();
        std::fs::write(&path, shifted).unwrap();
        let actual = positioned_rejection(&path, code);
        let expected = baseline.map(|mut rejection| {
            if rejection.file == "main.hd" && rejection.line > header {
                rejection.line += 1;
            }
            rejection
        });
        if actual != expected || expected.is_err() {
            failures.push(format!("{code}: expected {expected:?}, got {actual:?}"));
        }
        checked += 1;
    }
    std::fs::remove_dir_all(dir).unwrap();
    assert!(checked > 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_reject_preserves_its_actual_diagnostic_file_after_leading_newline() {
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reject");
    let directory =
        std::env::temp_dir().join(format!("hado-reject-file-shift-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for item in std::fs::read_dir(corpus).unwrap() {
        let case = item.unwrap().path();
        let original = case.join("main.hd");
        if !original.is_file() {
            continue;
        }
        let source = std::fs::read_to_string(&original).unwrap();
        let code = source
            .lines()
            .find_map(|line| line.strip_prefix("// error:"))
            .unwrap()
            .trim();
        let copied = directory.join(case.file_name().unwrap());
        copy_case_sources(&case, &copied);
        let entry = copied.join("main.hd");
        let baseline = positioned_rejection(&original, code);
        let initial = positioned_rejection(&entry, code);
        checked += 1;
        if initial != baseline || baseline.is_err() {
            failures.push(format!(
                "{code}: copy changed diagnostic: {baseline:?}, {initial:?}"
            ));
            continue;
        }
        let mut expected = baseline.unwrap();
        let diagnostic_file = copied.join(&expected.file);
        let before = std::fs::read_to_string(&diagnostic_file).unwrap();
        std::fs::write(&diagnostic_file, format!("\n{before}")).unwrap();
        expected.line += 1;
        let actual = positioned_rejection(&entry, code);
        if actual.as_ref() != Ok(&expected) {
            failures.push(format!("{code}: expected {expected:?}, got {actual:?}"));
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
    assert!(checked > 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
