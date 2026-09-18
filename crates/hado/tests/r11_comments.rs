use hado::lexer::{self, Span};

#[test]
fn generated_comment_markers_preserve_tokens_or_report_first_character() {
    let mut state = 0x1143_u64;
    for _ in 0..48 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let padding = " ".repeat((state >> 32) as usize % 17);
        let value = (state % 1000) as i64;
        for (prefix, suffix) in [
            (format!("{padding}"), format!("\ndef main = {value}")),
            (format!("def main = {padding}"), format!("\n{value}")),
            (format!("def main = {value} {padding}"), "\n".into()),
        ] {
            let expected = Span {
                line: 1,
                col: prefix.chars().count() + 1,
            };
            for marker in ["--", "#"] {
                let source = format!("{prefix}{marker} comment{suffix}");
                let error = lexer::lex_full(&source).unwrap_err();
                assert_eq!(error.code, Some("E104"), "{source}: {error:?}");
                assert_eq!(
                    (error.line, error.col),
                    (expected.line, expected.col),
                    "{source}"
                );
            }
            let source = format!("{prefix}// comment{suffix}");
            hado::check_source(&source, "generated.hd").unwrap();
        }
    }
}

#[test]
fn slash_division_and_comment_have_distinct_runtime_results() {
    let dir = std::env::temp_dir().join(format!("hado-r11-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, source, expected) in [
        ("division", "def main = 6 / 2", "3\n"),
        ("comment", "def main = 1 // 2", "1\n"),
        ("quoted", "def main = \"-- # //\"", "-- # //\n"),
    ] {
        let case_dir = dir.join(format!("{name}"));
        std::fs::create_dir_all(&case_dir).unwrap();
        let path = case_dir.join("main.hd");
        std::fs::write(&path, source).unwrap();
        let build = std::process::Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg("build")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let output = std::process::Command::new(path.with_extension(""))
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
    }
    std::fs::remove_dir_all(dir).unwrap();
}
