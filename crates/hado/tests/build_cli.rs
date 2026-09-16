use std::process::Command;

#[test]
fn build_executes_abstract_record_module() {
    let dir = std::env::temp_dir().join(format!("hado-abstract-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.hd");
    std::fs::write(&source, "def main = M.get (M.make 30) + N.get (N.make 12) + M.get (M.make 0) + N.get (N.make 0)\nmodule S type T def make(x: Int): T def get(value: T): Int\nimplement S as M type Representation = { value: Int } type T = Representation def make x => { value = x } def get item => item.value\nimplement S as N type Representation = { padding: Bool; other: Int } type T = Representation def make x => { padding = true; other = x } def get item => item.other").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_executes_module_adt_patterns() {
    let dir = std::env::temp_dir().join(format!("hado-adt-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.hd");
    std::fs::write(&source, "def main = M.get (M.make 30) + N.get (N.make 12) + M.get (M.make 0) + N.get (N.make 0)\nmodule S type T def make(x: Int): T def get(value: T): Int\nimplement S as M type Representation = | Empty | Wrap(Int) type T = Representation def make x => if x == 0 => Empty | Wrap(x) def get | Empty => 0 | Wrap(x) => x\nimplement S as N type Representation = | Wrap(Int) | Empty type T = Representation def make x => if x == 0 => Empty | Wrap(x) def get | Empty => 0 | Wrap(x) => x").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_links_functions_and_preserves_output_on_type_error() {
    let dir = std::env::temp_dir().join(format!("hado-build-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.hd");
    let executable = dir.join("example");
    std::fs::write(&source, "def twice n => n * 2\ndef calculate n => twice n + 1\ndef main = calculate 20").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(&executable).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"41\n");
    let original = std::fs::read(&executable).unwrap();
    std::fs::write(&source, "def main = 1 + true").unwrap();
    let failed = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&source).output().unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("E305"));
    assert_eq!(std::fs::read(&executable).unwrap(), original);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_executes_nested_parenthesized_constructors() {
    let dir = std::env::temp_dir().join(format!("hado-constructor-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.hd");
    std::fs::write(&source, "type Pair = | Pair(Int, Int Option)\ndef sum | Pair(left, Some(right)) => left + right | Pair(left, None) => left\ndef main = sum (Pair(30, Some(12)))").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_executes_abstract_module_function_values() {
    let dir = std::env::temp_dir().join(format!("hado-nested-module-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.hd");
    std::fs::write(&source, "def main = Outer.run 42\nmodule S type T def make(x: Int): T def get(value: T): Int\nmodule API def run(x: Int): Int\nimplement S as Inner\n    type Representation = { value: Int } type T = Representation\n    def make x => { value = x }\n    def get item => item.value\nimplement API as Outer\n  def consume (item: Inner.T) => let getter = Inner.get -> getter item\n  def run x => consume (Inner.make x)").unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn build_executes_an_effectful_function_value_from_a_definition() {
    let dir = std::env::temp_dir().join(format!("hado-effect-closure-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("example.hd");
    std::fs::write(
        &source,
        "def apply f x => f x\ndef make(s: String): Fn(String => String {Error}) Pure => fn(t => String.concat s (Int.to_string (Int.parse t)))\ndef main = apply (make \"x\") \"7\"",
    )
    .unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&source).output().unwrap();
    assert!(built.status.success(), "{built:?}");
    let output = Command::new(dir.join("example")).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"x7\n");
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
