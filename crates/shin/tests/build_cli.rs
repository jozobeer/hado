use std::process::Command;

#[test]
fn build_executes_abstract_record_module() {
    let dir = std::env::temp_dir().join(format!("shin-abstract-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.shin");
    std::fs::write(&source, "module type S = sig type T def make(x: Int): T def get(value: T): Int end\nmodule M : S = struct type T = { value: Int } def make x => { value = x } def get item => item.value end\nmodule N : S = struct type T = { padding: Bool; other: Int } def make x => { padding = true; other = x } def get item => item.other end\ndef main = M.get (M.make 30) + N.get (N.make 12) + M.get (M.make 0) + N.get (N.make 0)").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_executes_module_adt_patterns() {
    let dir = std::env::temp_dir().join(format!("shin-adt-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.shin");
    std::fs::write(&source, "module type S = sig type T def make(x: Int): T def get(value: T): Int end\nmodule M : S = struct type T = | Empty | Wrap(Int) def make x => if x == 0 => Empty | Wrap x def get item => if item | Empty => 0 | Wrap(x) => x end\nmodule N : S = struct type T = | Wrap(Int) | Empty def make x => if x == 0 => Empty | Wrap x def get item => if item | Empty => 0 | Wrap(x) => x end\ndef main = M.get (M.make 30) + N.get (N.make 12) + M.get (M.make 0) + N.get (N.make 0)").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_links_functions_and_preserves_output_on_type_error() {
    let dir = std::env::temp_dir().join(format!("shin-build-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.shin");
    let executable = dir.join("example");
    std::fs::write(&source, "def twice n => n * 2\ndef calculate n => twice n + 1\ndef main = calculate 20").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(&executable).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"41\n");
    let original = std::fs::read(&executable).unwrap();
    std::fs::write(&source, "def main = 1 + true").unwrap();
    let failed = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&source).output().unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("E305"));
    assert_eq!(std::fs::read(&executable).unwrap(), original);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_executes_nested_parenthesized_constructors() {
    let dir = std::env::temp_dir().join(format!("shin-constructor-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.shin");
    std::fs::write(&source, "type Pair = | Pair(Int, Option<Int>)\ndef sum pair => if pair | Pair(left, Some(right)) => left + right | Pair(left, None) => left\ndef main = sum (Pair(30, Some(12)))").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_executes_nested_abstract_module_function_values() {
    let dir = std::env::temp_dir().join(format!("shin-nested-module-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.shin");
    std::fs::write(&source, "module type S = sig type T def make(x: Int): T def get(value: T): Int end\nmodule type API = sig def run(x: Int): Int end\nmodule Outer : API = struct\n  module Inner : S = struct\n    type T = { value: Int }\n    def make x => { value = x }\n    def get item => item.value\n  end\n  def consume (item: Inner.T) => let getter = Inner.get -> getter item\n  def run x => consume (Inner.make x)\nend\ndef main = Outer.run 42\n").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_shin")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
