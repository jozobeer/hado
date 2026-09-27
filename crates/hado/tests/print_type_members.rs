//! §58.3: `check --print-type=<A>.<member>` prints the type a contract declares
//! for a member, looked up by the implementation's name (`lib::Util.helper`) or
//! by the contract's own name (`Store.find`), whether or not an implementation
//! exists. §58.5 hover on a `.hdi` takes the same lookup.

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

const UTIL: &str = "library\nmodule Util\ndef helper (x: Int): Int Pure\n/module\nimplement Util as Util\ndef helper x => x\n/implement\n";

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

const STORE: &str =
    "module Store\ntype T\ntype A Box\ndef A find (id: Int, x: A): T Option\ndef wrap (x: Int): Int Box\n/module\n";
const MEM: &str =
    "implement Store as Mem\ntype T = Int\ntype A Box = A List\ndef find id _ => Some(id)\ndef wrap x => [x]\n/implement\n";
const DISK: &str =
    "implement Store as Disk\ntype T = Bool\ntype A Box = A List\ndef find _ _ => None\ndef wrap x => [x]\n/implement\n";

// A contract with no implementation still declares a type for each member
// (§58.5: a `.hdi` opened on its own has a hover). The entry has no `main`, so
// it is checked as a library (§52) and the contract is its public surface.
#[test]
fn contract_member_without_an_implementation() {
    let case = Case::new(&[
        (
            "store.hdi",
            "type T = | A | B\nmodule M\ndef good (x: Int): T\n/module\n",
        ),
        ("main.hd", "def use (x: T) => x\n"),
    ]);
    assert_eq!(case.printed("M.good"), "Fn(Int => T {A})\n");
}

// The contract's own abstract types are spelled by the contract's name, since
// no implementation is there to name them.
#[test]
fn contract_abstract_types_are_spelled_by_the_contract() {
    let case = Case::new(&[("store.hdi", STORE), ("main.hd", "def main = ()\n")]);
    assert_eq!(
        case.printed("Store.find"),
        "Fn(Int, A => Store.T Option {B})\n"
    );
    assert_eq!(case.printed("Store.wrap"), "Fn(Int => Int Store.Box {A})\n");
}

// Beside an implementation of another name, the two spellings stay apart: the
// implementation's name gives the instantiated view, the contract's its own.
#[test]
fn contract_and_implementation_names_are_looked_up_apart() {
    let case = Case::new(&[
        ("store.hdi", STORE),
        ("mem.hd", MEM),
        (
            "main.hd",
            "def main = if Mem.find 1 2 | Some(_) => Mem.wrap 1 | Mem.wrap 2\n",
        ),
    ]);
    assert_eq!(case.printed("Mem.find"), "Fn(Int, A => Mem.T Option {B})\n");
    assert_eq!(
        case.printed("Store.find"),
        "Fn(Int, A => Store.T Option {B})\n"
    );
    assert_eq!(case.printed("Mem.wrap"), "Fn(Int => Int Mem.Box {A})\n");
    assert_eq!(case.printed("Store.wrap"), "Fn(Int => Int Store.Box {A})\n");
}

// §47 allows several implementations; the contract's own view names neither.
#[test]
fn contract_member_with_two_implementations() {
    let case = Case::new(&[
        ("store.hdi", STORE),
        ("mem.hd", MEM),
        ("disk.hd", DISK),
        (
            "main.hd",
            "def main = if Mem.find 1 2 | Some(_) => Disk.wrap 1 | Disk.wrap 2\n",
        ),
    ]);
    assert_eq!(case.printed("Mem.find"), "Fn(Int, A => Mem.T Option {B})\n");
    assert_eq!(
        case.printed("Disk.find"),
        "Fn(Int, A => Disk.T Option {B})\n"
    );
    assert_eq!(
        case.printed("Store.find"),
        "Fn(Int, A => Store.T Option {B})\n"
    );
}

// §51: a contract in a subdirectory is keyed by its path, as the rest of the
// program spells it.
#[test]
fn nested_contract_member_is_keyed_by_its_path() {
    let case = Case::new(&[
        (
            "api/store.hdi",
            "type T = | A | B\nmodule M\ndef good (x: Int): T\n/module\n",
        ),
        ("main.hd", "def use (x: api::T) => x\n"),
    ]);
    assert_eq!(case.printed("api::M.good"), "Fn(Int => api::T {A})\n");
    assert_eq!(case.printed("M.good"), "");
}
