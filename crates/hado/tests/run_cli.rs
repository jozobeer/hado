use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("hado-run-cli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, source: &str) -> PathBuf {
    let path = dir.join("main.hd");
    std::fs::write(&path, source).unwrap();
    path
}

fn hado() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hado"))
}

#[test]
fn run_prints_the_main_value_and_leaves_no_binary() {
    let dir = scratch("value");
    let source = write(&dir, "def main = 3\n");
    let output = hado().arg("run").arg(&source).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"3\n");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    // 入力の隣には何も残らない（一時ディレクトリは `run` が消す）。
    let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(entries, vec![std::ffi::OsString::from("main.hd")]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn run_forwards_the_arguments_after_the_separator() {
    let dir = scratch("args");
    let source = write(&dir, "def main =\n  let argv = args ->\n  List.length argv\n");
    let output = hado().arg("run").arg(&source).args(["--", "a", "b", "c", "d"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"4\n");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn run_passes_standard_input_through() {
    let dir = scratch("stdin");
    let source = write(&dir, "def main =\n  let bytes = read_stdin ->\n  Bytes.to_string bytes\n");
    let mut child = hado()
        .arg("run")
        .arg(&source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"hi").unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"hi\n");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn run_returns_the_program_exit_code() {
    let dir = scratch("raise");
    let source = write(&dir, "def main = raise { message = \"boom\" }\n");
    let output = hado().arg("run").arg(&source).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("boom"), "{:?}", output.stderr);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_writes_to_the_requested_output() {
    let dir = scratch("output");
    let source = write(&dir, "def main = 42\n");
    let target = dir.join("out").join("prog");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    let built = hado().arg("build").arg("-o").arg(&target).arg(&source).output().unwrap();
    assert_eq!(built.status.code(), Some(0), "{built:?}");
    assert!(target.is_file(), "no executable at the requested path");
    // 既定の出力先（入力の隣）には書かない。
    assert!(!source.with_extension("").exists());
    let output = Command::new(&target).output().unwrap();
    assert_eq!(output.stdout, b"42\n");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_reports_the_canonical_form() {
    let dir = scratch("canonical");
    let source = write(&dir, "def main = let x = 1 -> x\n");
    let built = hado().arg("build").arg(&source).output().unwrap();
    assert_eq!(built.status.code(), Some(1), "{built:?}");
    let stderr = String::from_utf8_lossy(&built.stderr);
    assert!(stderr.contains("error[E501]"), "{stderr}");
    assert!(stderr.contains("canonical form: 1"), "{stderr}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn run_at_a_compiler_checkout_root_reads_only_the_entry_tree() {
    let dir = scratch("checkout");
    std::fs::create_dir_all(dir.join("crates/hado/src")).unwrap();
    std::fs::write(dir.join("crates/hado/src/project.rs"), []).unwrap();
    std::fs::create_dir_all(dir.join("scripts/__pycache__")).unwrap();
    std::fs::write(dir.join("scripts/__pycache__/x"), []).unwrap();
    std::fs::create_dir_all(dir.join("tests/other")).unwrap();
    std::fs::write(dir.join("tests/other/main.hd"), "def main = \"other\"\n").unwrap();
    let source = write(&dir, "def main = \"hello\"\n");
    let dotted = dir.join("./main.hd");
    for path in [&source, &dotted] {
        let output = hado().arg("run").arg(path).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{path:?} {output:?}");
        assert_eq!(output.stdout, b"hello\n", "{path:?}");
        assert!(output.stderr.is_empty(), "{path:?} {:?}", output.stderr);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn run_does_not_treat_a_crates_hado_namespace_as_a_compiler_checkout() {
    let dir = scratch("namespace");
    std::fs::create_dir_all(dir.join("crates/hado")).unwrap();
    std::fs::create_dir_all(dir.join("tests/other")).unwrap();
    std::fs::write(dir.join("tests/other/main.hd"), "def main = \"other\"\n").unwrap();
    let source = write(&dir, "def main = \"hello\"\n");
    let output = hado().arg("run").arg(&source).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E100]"), "{stderr}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn run_skips_dunder_directories_that_are_not_namespaces() {
    let dir = scratch("dunder");
    std::fs::create_dir_all(dir.join("scripts/__pycache__")).unwrap();
    std::fs::write(dir.join("scripts/__pycache__/x"), []).unwrap();
    let source = write(&dir, "def main = \"hello\"\n");
    let output = hado().arg("run").arg(&source).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"hello\n");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn run_reads_dunder_named_source_files() {
    let dir = scratch("dunder-file");
    std::fs::write(dir.join("__types.hd"), "type User = { name: String }\n").unwrap();
    let source = write(&dir, "def name (user: User): String => user.name\ndef main = name { name = \"hello\" }\n");
    let output = hado().arg("run").arg(&source).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(output.stdout, b"hello\n");
    std::fs::remove_dir_all(dir).unwrap();
}
