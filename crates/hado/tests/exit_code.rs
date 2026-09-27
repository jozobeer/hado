use std::path::{Path, PathBuf};
use std::process::Command;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hado-exit-code-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn source(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("main.hd");
    std::fs::write(&path, text).unwrap();
    path
}

/// Build the file into `dir/program` and run it, returning (code, stdout, stderr).
fn built(text: &str, name: &str) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let dir = scratch(name);
    let path = source(&dir, text);
    let object = dir.join("program.o");
    let exe = dir.join("program");
    hado::project::Project::load(&path)
        .unwrap()
        .compile_to_object(&object)
        .unwrap();
    hado::link_executable(&object, &exe).unwrap();
    let out = Command::new(&exe).output().unwrap();
    let result = (out.status.code(), out.stdout, out.stderr);
    let _ = std::fs::remove_dir_all(dir);
    result
}

fn hado() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hado"))
}

fn run(text: &str, name: &str) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let dir = scratch(name);
    let path = source(&dir, text);
    let out = hado().arg("run").arg(&path).output().unwrap();
    let result = (out.status.code(), out.stdout, out.stderr);
    let _ = std::fs::remove_dir_all(dir);
    result
}

/// §54: a `main` of type Int is the program's exit code, and nothing is printed.
#[test]
fn int_main_is_the_exit_code_on_both_paths() {
    for value in ["0", "1", "3", "255"] {
        let text = format!("def main = {value}\n");
        let want = value.parse::<i32>().unwrap();
        for (label, got) in [
            ("built", built(&text, &format!("int-{value}"))),
            ("run", run(&text, &format!("int-run-{value}"))),
        ] {
            assert_eq!(got.0, Some(want), "{label} main = {value}: {got:?}");
            assert!(got.1.is_empty(), "{label} main = {value}: {got:?}");
            assert!(got.2.is_empty(), "{label} main = {value}: {got:?}");
        }
    }
}

/// §54: an Int outside 0..=255 is diagnosed before it is narrowed to the ABI's
/// i32, so neither a negative value nor 256 is normalized by the OS.
#[test]
fn int_main_out_of_range_is_diagnosed_before_narrowing() {
    let cases = [
        ("-1", "def main = -1\n"),
        ("256", "def main = 256\n"),
        ("max", "def main = 9223372036854775807\n"),
        (
            "min",
            "def main = -9223372036854775807 - 1\n",
        ),
    ];
    for (label, text) in cases {
        for (kind, got) in [
            ("built", built(text, &format!("range-{label}"))),
            ("run", run(text, &format!("range-run-{label}"))),
        ] {
            assert_eq!(got.0, Some(1), "{kind} {label}: {got:?}");
            assert!(got.1.is_empty(), "{kind} {label}: {got:?}");
            assert_eq!(
                String::from_utf8_lossy(&got.2).trim(),
                "hado: exit code out of range",
                "{kind} {label}: {got:?}"
            );
        }
    }
}

/// §54: a raise that reaches main still exits 1.
#[test]
fn raised_error_still_exits_one() {
    let text = "def main = raise { message = \"boom\" }\n";
    let got = run(text, "raise");
    assert_eq!(got.0, Some(1), "{got:?}");
    assert_eq!(String::from_utf8_lossy(&got.2).trim(), "boom", "{got:?}");
}

/// §55: `log` writes stdout, `log_error` writes stderr, and each adds exactly
/// one newline. A Void main prints nothing.
#[test]
fn log_goes_to_stdout_and_log_error_goes_to_stderr() {
    let log = run("def main =\n  log \"out\" ->\n  ()\n", "log");
    assert_eq!(log.0, Some(0), "{log:?}");
    assert_eq!(log.1, b"out\n", "{log:?}");
    assert!(log.2.is_empty(), "{log:?}");

    let log_error = run("def main =\n  log_error \"err\" ->\n  ()\n", "log-error");
    assert_eq!(log_error.0, Some(0), "{log_error:?}");
    assert!(log_error.1.is_empty(), "{log_error:?}");
    assert_eq!(log_error.2, b"err\n", "{log_error:?}");

    let both = built(
        "def main =\n  log \"out\" ->\n  log_error \"err\" ->\n  ()\n",
        "both",
    );
    assert_eq!(both.0, Some(0), "{both:?}");
    assert_eq!(both.1, b"out\n", "{both:?}");
    assert_eq!(both.2, b"err\n", "{both:?}");
}

/// §55: a failing `log_error` is a catchable Error whose message is IoFailure.
/// The entry point is the *file's own* entry, so calling it directly would
/// return the guest exit code; instead the probe runs through the shell so the
/// descriptor can be closed.
fn log_error_probe(name: &str, source_text: &str, close_stderr: bool) -> (Option<i32>, Vec<u8>) {
    let dir = scratch(name);
    let path = source(&dir, source_text);
    let object = dir.join("program.o");
    let exe = dir.join("program");
    hado::project::Project::load(&path)
        .unwrap()
        .compile_to_object(&object)
        .unwrap();
    hado::link_executable(&object, &exe).unwrap();
    let command = if close_stderr {
        format!("{} 2>&-", exe.display())
    } else {
        format!("{} 2>{}", exe.display(), dir.join("err").display())
    };
    let out = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(&dir)
        .output()
        .unwrap();
    let stderr = std::fs::read(dir.join("err")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(dir);
    (out.status.code(), stderr)
}

#[test]
fn log_error_failure_is_a_catchable_io_failure() {
    // The handler records the message beside the program, so the caught value
    // is observed even when stderr is unusable.
    let catching = "handler Error as Observe\nabort raise e =>\n  write_file \"observed\" (Bytes.of_string e.message) ->\n  ()\nreturn value => value\ndef main =\n  with Observe ->\n  log_error \"x\"\n";
    let (code, _) = log_error_probe("log-error-caught", catching, true);
    assert_eq!(code, Some(0), "a caught failure must not change the exit code");

    // Without a handler, the failure reaches main and exits 1.
    let (code, stderr) = log_error_probe(
        "log-error-uncaught",
        "def main =\n  log_error \"x\" ->\n  ()\n",
        true,
    );
    assert_eq!(code, Some(1), "{stderr:?}");
    assert_eq!(String::from_utf8_lossy(&stderr).trim(), "IoFailure");
}

/// §54: a main of another type still prints and exits 0.
#[test]
fn non_int_main_still_prints_and_exits_zero() {
    for (text, want) in [
        ("def main = \"hello\"\n", "hello"),
        ("def main = true\n", "true"),
        ("def main = ()\n", ""),
    ] {
        let got = run(text, &format!("typed-{}", want.len()));
        assert_eq!(got.0, Some(0), "{text:?}: {got:?}");
        let stdout = String::from_utf8_lossy(&got.1);
        assert_eq!(stdout.trim_end_matches('\n'), want, "{text:?}: {got:?}");
    }
}
