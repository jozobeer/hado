use std::process::Command;

#[test]
fn cli_usage_and_basic_types() {
    for args in [vec![], vec!["check"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_shin"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"usage: shin check [--print-type] <file> | dump-mono <file> | build <file.shin>\n");
    }
    let path = std::env::temp_dir().join(format!("shin-cli-{}.shin", std::process::id()));
    for (source, ty) in [
        ("def main = 1 + 2", "Int"),
        ("def main = 1.5 * 2.0", "Float"),
        ("def main = not (1 < 2)", "Bool"),
    ] {
        std::fs::write(&path, source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_shin"))
            .args(["check", "--print-type"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(output.stdout, format!("{ty}\n").as_bytes());
        assert!(output.stderr.is_empty());
        let output = Command::new(env!("CARGO_BIN_EXE_shin"))
            .arg("check")
            .arg(&path)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    std::fs::write(&path, "\ndef main = 1 + true").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_shin"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with(&format!("{}:2:12: error[E305]:", path.display())),
        "{stderr}"
    );
    assert_eq!(stderr.lines().count(), 1);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn diagnostics_without_codes_keep_source_position() {
    let error = shin::check_source("def main = 1.0 % 2.0", "float.shin")
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "float.shin:1:12: error: Float has no remainder operator"
    );
}

#[test]
fn cli_rejects_noncanonical_identity_binding() {
    let path = std::env::temp_dir().join(format!("shin-canonical-{}.shin", std::process::id()));
    std::fs::write(&path, "def main = let x = 1 -> x").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_shin"))
        .arg("check")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "{}:1:12: error[E501]: binding x is returned unchanged at 1:25\n",
            path.display()
        )
    );
}

#[test]
fn reject_corpus_reports_declared_codes_through_cli() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/reject");
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("shin") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let expected = source
            .lines()
            .find_map(|line| line.strip_prefix("-- error: "))
            .unwrap()
            .trim();
        let output = Command::new(env!("CARGO_BIN_EXE_shin"))
            .arg("check")
            .arg(&path)
            .output()
            .unwrap();
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        let positioned = diagnostic.strip_prefix(&format!("{}:", path.display()));
        let valid_position_and_code = positioned.is_some_and(|positioned| {
            let mut parts = positioned.splitn(3, ':');
            let line = parts.next().and_then(|value| value.parse::<usize>().ok());
            let col = parts.next().and_then(|value| value.parse::<usize>().ok());
            line.is_some_and(|line| line > 0) && col.is_some_and(|col| col > 0)
                && parts.next().is_some_and(|message| message.starts_with(&format!(" error[{expected}]: ")))
        });
        if output.status.code() != Some(1) || !output.stdout.is_empty()
            || diagnostic.lines().count() != 1 || !valid_position_and_code {
            failures.push(format!("{} expected {expected}: status={:?}, stdout={:?}, stderr={diagnostic:?}",
                path.display(), output.status.code(), output.stdout));
        }
        checked += 1;
    }
    assert!(checked > 0);
    failures.sort();
    assert!(failures.is_empty(), "{} failures in {checked} cases:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn dump_mono_exposes_typed_arithmetic() {
    let path = std::env::temp_dir().join(format!("shin-mono-{}.shin", std::process::id()));
    std::fs::write(&path, "def main = 1 + 2").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_shin"))
        .arg("dump-mono")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("fn main() -> Int\nBinary { op: Add, left: Constant(Int(1)), right: Constant(Int(2)) }\n"));
    assert!(text.contains("name: \"print_Int\""));
}
