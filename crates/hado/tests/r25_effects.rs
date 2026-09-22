use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hado-r25-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, file: &str, source: &str) {
        std::fs::create_dir_all(self.0.join(file).parent().unwrap()).unwrap();
        std::fs::write(self.0.join(file), source).unwrap();
    }
    fn command(&self, command: &str, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg(command)
            .arg(self.0.join(entry))
            .output()
            .unwrap()
    }
    fn print_type(&self, entry: &str) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_hado"))
            .args(["check", "--print-type"])
            .arg(self.0.join(entry))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    }
    fn reject(&self, command: &str, entry: &str, file: &str, code: &str, offset: usize) {
        let source = std::fs::read_to_string(self.0.join(file)).unwrap();
        let before = &source[..offset];
        let output = self.command(command, entry);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        let relative = std::path::Path::new(file)
            .strip_prefix(std::path::Path::new(entry).parent().unwrap())
            .unwrap();
        let prefix = format!(
            "{}:{}:{}: error[{code}]: ",
            relative.display(),
            before.matches('\n').count() + 1,
            before.rsplit('\n').next().unwrap().chars().count() + 1
        );
        assert!(
            diagnostic.starts_with(&prefix),
            "expected {prefix}\n{diagnostic}\n{source}"
        );
        assert_eq!(diagnostic.lines().count(), 1);
    }
    fn build(&self, entry: &str) -> Output {
        self.command("build", entry)
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn random(state: &mut u64, limit: usize) -> usize {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as usize) % limit
}

fn shifted(source: &str, anchor: &str, padding: usize) -> (String, usize) {
    let offset = source.find(anchor).unwrap();
    let lead = format!("// position {padding}\n{}", "\n".repeat(padding % 3));
    (
        format!(
            "{lead}{}{}{}",
            &source[..offset],
            " ".repeat(padding),
            &source[offset..]
        ),
        lead.len() + offset + padding,
    )
}

fn perturb_rejection(case: &Case, file: &str, source: &str, code: &str, anchor: &str) {
    for padding in 0..5 {
        let (source, offset) = shifted(source, anchor, padding);
        case.write(file, &source);
        for command in ["check", "build", "dump-mono"] {
            case.reject(command, "main.hd", file, code, offset);
        }
    }
}

#[test]
fn let_question_requires_option() {
    let mut state = 0x25_03;
    for _ in 0..8 {
        let n = random(&mut state, 1000);
        let case = Case::new();
        perturb_rejection(
            &case,
            "main.hd",
            &format!("def main = let x? = {n} -> Some(x)"),
            "E303",
            &n.to_string(),
        );
    }
}

#[test]
fn do_effect_row_is_checked_after_number_defaulting() {
    let mut state = 0x25_09;
    let n = random(&mut state, 1000);
    let arg = random(&mut state, 50) + 1;
    let discarded = random(&mut state, 1000) + 1000;
    let case = Case::new();
    case.write(
        "main.hd",
        &format!("def f x => do (x / x) -> {n}\ndef main = f {arg}\n"),
    );
    assert_eq!(case.print_type("main.hd"), "Int {Error}\n");
    case.write(
        "main.hd",
        &format!("def f (x: Int): Int => do (x / x) -> {n}\ndef main = f {arg}\n"),
    );
    assert_eq!(case.print_type("main.hd"), "Int {Error}\n");
    perturb_rejection(
        &case,
        "main.hd",
        &format!("def f x => do {discarded} -> x\ndef main = f {arg}\n"),
        "E303",
        &discarded.to_string(),
    );
    case.write(
        "main.hd",
        &format!("effect Ask\nonce ask: Bool\ndef f x => do ask -> x\ndef main = f {arg}\n"),
    );
    assert_eq!(case.print_type("main.hd"), "Int {Ask}\n");
}

#[test]
fn integer_division_is_int_with_error() {
    let mut state = 0x25_05;
    let left = random(&mut state, 50) + 1;
    let right = random(&mut state, 50) + 1;
    let case = Case::new();
    case.write("main.hd", &format!("def main = {left} / {right}\n"));
    assert_eq!(case.print_type("main.hd"), "Int {Error}\n");
    case.write("main.hd", &format!("def main = {left}.0 / {right}.0\n"));
    assert_eq!(case.print_type("main.hd"), "Float\n");
}

#[test]
fn do_discards_an_error_effect() {
    let mut state = 0x25_06;
    let n = random(&mut state, 1000);
    let case = Case::new();
    case.write(
        "main.hd",
        &format!("def boom = raise {{ message = \"{n}\" }}\ndef main = do boom -> {n}\n"),
    );
    assert_eq!(case.print_type("main.hd"), "Int {Error}\n");
}

#[test]
fn integer_zero_divisor_aborts_with_division_by_zero() {
    let mut state = 0x25_08;
    let left = random(&mut state, 50) + 1;
    let case = Case::new();
    case.write("main.hd", &format!("def main = {left} % 0\n"));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = Command::new(case.0.join("main")).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert_eq!(output.stderr, b"DivisionByZero\n");
}
