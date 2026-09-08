use std::path::PathBuf;
use std::process::Command;

fn workdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hado-walk-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn walk() {
    // Distinct expectations: no single constant output can satisfy all cases.
    // Ignoring the AST (which numbers are added) fails at least one case.
    let cases = [
        ("def main = 1 + 2", "3"),
        ("def main = 10 + 20", "30"),
        ("def main = 4 + 5 + 6", "15"),
        ("def twice n => n * 2\ndef main = twice 21", "42"),
    ];
    for (i, (src, expected)) in cases.iter().enumerate() {
        let dir = workdir();
        let obj = dir.join(format!("walk{i}.o"));
        let exe = dir.join(format!("walk{i}"));
        hado::compile_to_object(src, &obj).expect("compile walk");
        hado::link_executable(&obj, &exe).expect("link walk");
        let out = Command::new(&exe).output().expect("run walk");
        assert!(
            out.status.success(),
            "case {i} ({src}): guest exit status: {:?}",
            out.status
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(stdout.trim(), *expected, "case {i}: {src}");
    }
}
