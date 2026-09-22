use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hado-r17-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) {
        std::fs::write(self.0.join(name), source).unwrap();
    }
    fn command(&self, command: &str, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg(command)
            .arg(self.0.join(entry))
            .output()
            .unwrap()
    }
    fn reject(
        &self,
        command: &str,
        entry: &str,
        file: &str,
        source: &str,
        code: &str,
        anchor: &str,
    ) {
        let before = &source[..source.find(anchor).unwrap()];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().unwrap().chars().count() + 1;
        let output = self.command(command, entry);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let prefix = format!(
            "{}:{line}:{col}: error[{code}]:",
            std::path::Path::new(file)
                .strip_prefix(std::path::Path::new(entry).parent().unwrap())
                .unwrap()
                .display()
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).starts_with(&prefix),
            "expected {prefix}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn random(state: &mut u64, limit: usize) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((*state >> 32) as usize) % limit
}

#[test]
fn generated_programs_share_types_and_contracts_across_files() {
    let mut state = 0x170012;
    for _ in 0..2 {
        let case = Case::new();
        let count = 1 + random(&mut state, 4);
        let mut files = vec![String::new(); 1 + random(&mut state, 4)];
        let mut calls = Vec::new();
        let mut expected = 0;
        for contract in 0..count {
            let mut signature = format!("type Input{contract} = {{ value{contract}: Int }}\nmodule S{contract}\ndef run(x: Input{contract}): Int");
            for implementation in 0..2 + random(&mut state, 3) {
                let offset = random(&mut state, 100);
                let value = random(&mut state, 100);
                let name = format!("M{contract}_{implementation}");
                let section = format!("\nimplement S{contract} as {name}\ndef run x => x.value{contract} + {offset}\n");
                if random(&mut state, 2) == 0 {
                    signature.push_str(&section);
                } else {
                    let file = random(&mut state, files.len());
                    files[file].push_str(&section);
                }
                calls.push(format!("{name}.run {{ value{contract} = {value} }}"));
                expected += value + offset;
            }
            case.write(&format!("contract{contract}.hd"), &signature);
        }
        for (index, source) in files.iter().enumerate() {
            case.write(&format!("impl{index}.hd"), source);
        }
        // This file sorts before every implementation file but must remain a distinct program.
        case.write("a_types.hdi", "type Outer = { number: Int }");
        case.write(
            "main.hd",
            &format!(
                "def main = let result = {{ number = {} }} -> result.number",
                calls.join(" + ")
            ),
        );
        let output = case.command("build", "main.hd");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(case.0.join("main")).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn duplicate_names_follow_entry_then_byte_path_order() {
    for padding in 0..3 {
        for (first, second, anchor) in [
            ("module S", "module S", "S"),
            ("implement S as M", "implement S as M", "M"),
            ("type Data = | First", "type Data = | Second", "Data"),
        ] {
            for entry_first in [false, true] {
                let case = Case::new();
                let source = format!(
                    "{}{}{}",
                    "\n".repeat(padding % 3),
                    " ".repeat(padding),
                    second
                );
                let main = if entry_first {
                    format!("def main = 1\n{first}")
                } else {
                    "def main = 1".into()
                };
                case.write("z_entry.hd", &main);
                if !entry_first {
                    case.write("a_first.hd", first);
                }
                case.write("b_second.hd", &source);
                if first.starts_with("implement") {
                    case.write("contract.hdi", "module S");
                }
                case.reject(
                    "check",
                    "z_entry.hd",
                    "b_second.hd",
                    &source,
                    "E205",
                    anchor,
                );
            }
        }
    }
}

#[test]
fn file_role_errors_keep_original_paths_and_positions() {
    for padding in 0..2 {
        for (file, invalid, anchor) in [
            (
                "contract.hdi",
                "implement S as M\ndef value = 1",
                "implement",
            ),
            ("contract.hdi", "def helper x => x", "def"),
        ] {
            let case = Case::new();
            let main = if invalid.starts_with("implement") {
                "def main = M.value\nmodule S\ndef value: Int"
            } else {
                "def main = 1"
            };
            case.write("main.hd", main);
            let source = format!(
                "{}{}{}",
                "\n".repeat(padding % 3),
                " ".repeat(padding),
                invalid
            );
            case.write(file, &source);
            for command in ["check", "build", "dump-mono"] {
                case.reject(command, "main.hd", file, &source, "E100", anchor);
            }
        }
        let case = Case::new();
        let source = format!(
            "{}{}def helper x => x + 1\ndef main = helper 1",
            "\n".repeat(padding % 3),
            " ".repeat(padding)
        );
        case.write("main.hd", &source);
        case.write("other.hd", "");
        // A1: check on a file without main is a library inspection, so main.hd's
        // unreferenced `helper` is E513; build/dump-mono still require an entry.
        case.reject(
            "check",
            "other.hd",
            "main.hd",
            &source,
            "E513",
            "def helper",
        );
        for command in ["build", "dump-mono"] {
            case.reject(command, "other.hd", "other.hd", "", "E206", "");
        }

        let case = Case::new();
        case.write("main.hd", "def main = 1");
        let source = format!(
            "{}{}def helper x => x",
            "\n".repeat(padding % 3),
            " ".repeat(padding)
        );
        case.write("other.hd", &source);
        for command in ["check", "build", "dump-mono"] {
            case.reject(
                command,
                "main.hd",
                "other.hd",
                &source,
                "E513",
                "def helper",
            );
        }
    }
}

#[test]
fn every_source_file_is_checked_and_other_extensions_are_ignored() {
    let case = Case::new();
    case.write("main.hd", "def main = 1");
    std::fs::create_dir(case.0.join("nested")).unwrap();
    case.write("ignored.txt", "def main = [)");
    assert!(case.command("check", "main.hd").status.success());
    case.write("nested/broken.hd", "type Data = [)");
    case.reject(
        "check",
        "main.hd",
        "nested/broken.hd",
        "type Data = [)",
        "E100",
        "[",
    );
    std::fs::remove_file(case.0.join("nested/broken.hd")).unwrap();
    case.write("nested/helper.hd", "\n  def helper x => x");
    for command in ["check", "build", "dump-mono"] {
        case.reject(
            command,
            "main.hd",
            "nested/helper.hd",
            "\n  def helper x => x",
            "E100",
            "def helper",
        );
    }
    std::fs::remove_file(case.0.join("nested/helper.hd")).unwrap();
    case.write("unused.hd", "module S\nimplement S as M");
    case.reject(
        "check",
        "main.hd",
        "unused.hd",
        "module S\nimplement S as M",
        "E513",
        "implement",
    );
    std::fs::remove_file(case.0.join("unused.hd")).unwrap();
    case.write("broken.hd", "implement S as M\ndef broken = [)");
    case.reject(
        "build",
        "main.hd",
        "broken.hd",
        "implement S as M\ndef broken = [)",
        "E100",
        ")",
    );
}

#[test]
fn interface_files_reject_implementation_types_and_cannot_be_entries() {
    for padding in 0..2 {
        let case = Case::new();
        case.write(
            "main.hd",
            "def main = M.make 1\nimplement S as M type T = Int def make x => x",
        );
        let source = format!(
            "module S type T def make(x: Int): T\nmodule Other def use(x: {}M.T): Int",
            " ".repeat(padding)
        );
        case.write("store.hdi", &source);
        case.reject("check", "main.hd", "store.hdi", &source, "E204", "M.T");
        for command in ["check", "build", "dump-mono"] {
            let output = case.command(command, "store.hdi");
            assert!(!output.status.success());
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(stderr.contains(".hd extension"), "{stderr}");
            assert!(!stderr.contains("error["));
        }
    }
}

#[test]
fn single_file_entry_diagnostics_keep_existing_positions() {
    for padding in 0..8 {
        let case = Case::new();
        let source = format!(
            "{}{}def helper x => x + 1",
            "\n".repeat(padding),
            " ".repeat(padding)
        );
        case.write("main.hd", &source);
        let expected = hado::check_source(&source, "main.hd").err().unwrap();
        assert_eq!(expected.code.as_deref(), Some("E513"));
        let actual = case.command("check", "main.hd");
        assert_eq!(actual.stderr, format!("{expected}\n").as_bytes());
        assert_eq!(actual.status.code(), Some(1));
    }
}

#[test]
fn lexical_errors_in_later_files_precede_earlier_syntax_errors() {
    for padding in 0..2 {
        let case = Case::new();
        case.write("main.hd", "def main = [)");
        let source = format!(
            "{}{}# invalid comment",
            "\n".repeat(padding),
            " ".repeat(padding)
        );
        case.write("later.hdi", &source);
        case.reject("check", "main.hd", "later.hdi", &source, "E104", "#");
    }
}

#[test]
fn implementation_diagnostics_retain_anchors_from_each_file() {
    for padding in 0..2 {
        for (contract, body, argument, code, anchor) in [
            (
                "def run(x: Int): Bool",
                "def run x => x + 1",
                "1",
                "E602",
                "implement",
            ),
            (
                "def run(x: Int): Int",
                "def dead x => x + 2 def run x => x + 1",
                "1",
                "E514",
                "def dead",
            ),
            (
                "def run(x: Int Option): Int",
                "def run x => if x | Some(n) => n",
                "(Some(1))",
                "E402",
                "if x",
            ),
        ] {
            let case = Case::new();
            case.write("main.hd", &format!("def main = M.run {argument}"));
            case.write("contract.hdi", &format!("module S {contract}"));
            let source = format!(
                "{}{}implement S as M\n{body}",
                "\n".repeat(padding),
                " ".repeat(padding)
            );
            case.write("implementation.hd", &source);
            case.reject(
                "check",
                "main.hd",
                "implementation.hd",
                &source,
                code,
                anchor,
            );
        }
    }
}
