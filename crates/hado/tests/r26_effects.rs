use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r26-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, file: &str, source: &str) {
        std::fs::create_dir_all(self.0.join(file).parent().unwrap()).unwrap();
        std::fs::write(self.0.join(file), source).unwrap();
    }
    fn command(&self, command: &str, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado")).arg(command).arg(self.0.join(entry)).output().unwrap()
    }
    fn print_type(&self, entry: &str) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_hado")).args(["check", "--print-type"]).arg(self.0.join(entry)).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stderr.is_empty());
        String::from_utf8(output.stdout).unwrap()
    }
    fn reject(&self, command: &str, entry: &str, file: &str, code: &str, offset: usize) {
        let source = std::fs::read_to_string(self.0.join(file)).unwrap();
        let before = &source[..offset];
        let output = self.command(command, entry);
        assert_eq!(output.status.code(), Some(1), "{source}\n{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        let relative = std::path::Path::new(file).strip_prefix(std::path::Path::new(entry).parent().unwrap()).unwrap();
        let prefix = format!("{}:{}:{}: error[{code}]: ", relative.display(), before.matches('\n').count() + 1, before.rsplit('\n').next().unwrap().chars().count() + 1);
        assert!(diagnostic.starts_with(&prefix), "expected {prefix}\n{diagnostic}\n{source}");
        assert_eq!(diagnostic.lines().count(), 1);
    }
    fn build(&self, entry: &str) -> Output {
        self.command("build", entry)
    }
    fn run(&self, entry: &str) -> Output {
        Command::new(self.0.join(entry).with_extension("")).output().unwrap()
    }
}
impl Drop for Case { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

fn random(state: &mut u64, limit: usize) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*state >> 32) as usize) % limit
}

fn shifted(source: &str, anchor: &str, padding: usize) -> (String, usize) {
    let offset = source.find(anchor).unwrap();
    let lead = format!("// position {padding}\n{}", "\n".repeat(padding % 3));
    (format!("{lead}{}{}{}", &source[..offset], " ".repeat(padding), &source[offset..]), lead.len() + offset + padding)
}

fn perturb_rejection(case: &Case, file: &str, source: &str, code: &str, anchor: &str) {
    for padding in 0..5 {
        let (source, offset) = shifted(source, anchor, padding);
        case.write(file, &source);
        for command in ["check", "build", "dump-mono"] { case.reject(command, "main.hd", file, code, offset); }
    }
}

#[test]
fn main_user_effect_is_e706_on_def_main() {
    let case = Case::new();
    perturb_rejection(
        &case,
        "main.hd",
        "effect Ask\nreply ask: String\ndef main = ask\n",
        "E706",
        "def main",
    );
}

#[test]
fn print_type_still_shows_user_effects_on_main() {
    let mut state = 0x26_01;
    for _ in 0..8 {
        let n = random(&mut state, 1000);
        let case = Case::new();
        case.write("main.hd", &format!("effect Ask\nreply ask: String\ndef f x => if true => ask | x\ndef main = f \"{n}\"\n"));
        assert_eq!(case.print_type("main.hd"), "String {Ask}\n");
        let output = case.command("check", "main.hd");
        assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stderr).contains("error[E706]"), "{}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn reply_evaluates_the_handler_arm_in_place() {
    let mut state = 0x26_02;
    for _ in 0..8 {
        let token = format!("t{}", random(&mut state, 100_000));
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Ask\nreply ask: String\nhandler Ask as Fixed\nreply ask => \"{token}\"\ndef main =\n  with Fixed ->\n  ask\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, format!("{token}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn unhandled_abort_matches_result_stderr_and_status() {
    let mut state = 0x26_03;
    for _ in 0..8 {
        let token = format!("e{}", random(&mut state, 100_000));
        let case = Case::new();
        case.write("main.hd", &format!("def main = raise {{ message = \"{token}\" }}\n"));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty(), "{:?}", output.stdout);
        assert_eq!(output.stderr, format!("{token}\n").as_bytes());
    }
}

#[test]
fn abort_handler_turns_raise_into_a_value() {
    let mut state = 0x26_04;
    for _ in 0..8 {
        let n = random(&mut state, 1000);
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as ToZero\nabort raise _ => {n}\nreturn value => value\ndef main =\n  with ToZero ->\n  raise {{ message = \"caught\" }}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{n}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn reply_tail_recursion_completes_ten_million_iterations() {
    let mut state = 0x26_05;
    for _ in 0..4 {
        let n = random(&mut state, 40) + 10;
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Tick\nreply tick: Unit\nhandler Tick as Nop\nreply tick => ()\ndef rec loop (k: Int, acc: Int): Int {{Tick}} =>\n  if k <= 0 => acc\n  | do tick -> loop (k - 1) (acc + 1)\ndef main =\n  with Nop ->\n  loop {n} 0\n"
        ));
        let dump = case.command("dump-mono", "main.hd");
        assert_eq!(dump.status.code(), Some(0), "{}", String::from_utf8_lossy(&dump.stderr));
        let ir = String::from_utf8_lossy(&dump.stdout);
        assert!(ir.contains("TailCall"), "{ir}");
        assert!(!ir.contains("Yield"), "{ir}");
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{n}\n").as_bytes());
    }
    let case = Case::new();
    case.write("main.hd", "effect Tick\nreply tick: Unit\nhandler Tick as Nop\nreply tick => ()\ndef rec loop (n: Int, acc: Int): Int {Tick} =>\n  if n <= 0 => acc\n  | do tick -> loop (n - 1) (acc + 1)\ndef main =\n  with Nop ->\n  loop 10000000 0\n");
    let built = case.build("main.hd");
    assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, b"10000000\n");
    assert!(output.stderr.is_empty());
}
