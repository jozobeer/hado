//! §58.3: `check --print-type=<A>.<member>` prints the type a contract declares
//! for a member, looked up by the implementation's name (`lib::Util.helper`).

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);

impl Case {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hado-print-type-members-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in files {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        Self(dir)
    }

    fn print_type(&self, name: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg("check")
            .arg(format!("--print-type={name}"))
            .arg(self.0.join("main.hd"))
            .output()
            .unwrap()
    }

    fn printed(&self, name: &str) -> String {
        let output = self.print_type(name);
        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const UTIL: &str = "library\nmodule Util\ndef helper(x: Int): Int Pure\n\nimplement Util as Util\ndef helper x => x\n";

#[test]
fn implementation_member_is_printed_by_name() {
    let case = Case::new(&[
        ("lib/util.hd", UTIL),
        ("main.hd", "def main = lib::Util.helper 3\n"),
    ]);
    let output = case.print_type("lib::Util.helper");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, b"Fn(Int => Int)\n");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn a_name_without_a_type_is_missing_not_silent() {
    let case = Case::new(&[
        ("lib/util.hd", UTIL),
        ("main.hd", "def main = lib::Util.helper 3\n"),
    ]);
    let output = case.print_type("lib::Util.nope");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .starts_with("check: no definition named lib::Util.nope in "),
        "{output:?}"
    );
}
