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
        "effect Ask\nonce ask: String\ndef main = ask\n",
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
        case.write("main.hd", &format!("effect Ask\nonce ask: String\ndef f x => if true => ask | x\ndef main = f \"{n}\"\n"));
        assert_eq!(case.print_type("main.hd"), "String {Ask}\n");
        let output = case.command("check", "main.hd");
        assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(String::from_utf8_lossy(&output.stderr).contains("error[E706]"), "{}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn once_evaluates_the_handler_arm_in_place() {
    let mut state = 0x26_02;
    for _ in 0..8 {
        let token = format!("t{}", random(&mut state, 100_000));
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Ask\nonce ask: String\nhandler Ask as Fixed\nonce ask => \"{token}\"\ndef main =\n  with Fixed ->\n  ask\n"
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
fn abort_handler_skips_do_continuation_when_the_rhs_raises() {
    let mut state = 0x26_30;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let continued = caught + 1 + random(&mut state, 1000);
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\ndef f x => if x == 0 => raise {{ message = \"x\" }} | x\ndef main =\n  with Catch ->\n  do f 0 ->\n  {continued}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{caught}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_runs_do_continuation_when_the_rhs_returns() {
    let mut state = 0x26_31;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let continued = caught + 1 + random(&mut state, 1000);
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\ndef f x => if x == 0 => raise {{ message = \"x\" }} | x\ndef main =\n  with Catch ->\n  do f 1 ->\n  {continued}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{continued}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_integer_division_by_zero() {
    let mut state = 0x26_32;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let left = random(&mut state, 50) + 1;
        let op = if random(&mut state, 2) == 0 { "/" } else { "%" };
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\ndef main =\n  with Catch ->\n  {left} {op} 0\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{caught}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_division_inside_arithmetic_and_calls() {
    let mut state = 0x26_33;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let left = random(&mut state, 50) + 1;
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\ndef f x => x / 0\ndef main =\n  with Catch ->\n  1 + (f {left})\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{caught}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_skips_do_continuation_on_division_by_zero() {
    let mut state = 0x26_34;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let continued = caught + 1 + random(&mut state, 1000);
        let left = random(&mut state, 50) + 1;
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\ndef main =\n  with Catch ->\n  do ({left} / 0) ->\n  {continued}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{caught}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_do_continuation_may_differ_from_the_rhs_success_type() {
    let mut state = 0x26_35;
    for _ in 0..8 {
        let caught = format!("c{}", random(&mut state, 1000));
        let continued = format!("d{}", random(&mut state, 1000));
        let x = random(&mut state, 2);
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => \"{caught}\"\nreturn value => value\ndef f x => if x == 0 => raise {{ message = \"x\" }} | x\ndef main =\n  with Catch ->\n  do f {x} ->\n  \"{continued}\"\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = if x == 0 { &caught } else { &continued };
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_division_outside_an_unrelated_once_handler() {
    let mut state = 0x26_36;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let left = random(&mut state, 50) + 1;
        let op = if random(&mut state, 2) == 0 { "/" } else { "%" };
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\neffect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef main =\n  with Catch ->\n  with Nop ->\n  1 + ({left} {op} 0)\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{caught}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_division_in_a_callee_outside_an_unrelated_once_handler() {
    let mut state = 0x26_37;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let left = random(&mut state, 50) + 1;
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\neffect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef f x => x / 0\ndef main =\n  with Catch ->\n  with Nop ->\n  1 + (f {left})\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{caught}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_raise_outside_an_unrelated_once_handler() {
    let mut state = 0x26_38;
    for _ in 0..8 {
        let caught = random(&mut state, 1000);
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => {caught}\nreturn value => value\neffect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef main =\n  with Catch ->\n  with Nop ->\n  1 + (raise {{ message = \"x\" }})\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{caught}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_outside_once_still_runs_the_once_body() {
    let mut state = 0x26_39;
    for _ in 0..8 {
        let n = random(&mut state, 20) + 1;
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => 0\nreturn value => value\neffect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef rec loop (k: Int, acc: Int): Int {{Tick}} =>\n  if k <= 0 => acc\n  | do tick -> loop (k - 1) (acc + 1)\ndef main =\n  with Catch ->\n  with Nop ->\n  loop {n} 0\n"
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
fn again_handler_outside_abort_handler_collects_callee_yields() {
    let mut state = 0x26_3a;
    for _ in 0..8 {
        let yielded = random(&mut state, 50) as i64;
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\nhandler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef f (x: Int): Int {{Int Yield}} =>\n  do yield x -> x\ndef main =\n  with Collect ->\n  with Catch ->\n  f {yielded}\n",
            collect_handler(),
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("[{}]\n", yielded).as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn again_handler_outside_abort_handler_forwards_callee_raise() {
    let mut state = 0x26_3b;
    for _ in 0..8 {
        let fails = random(&mut state, 2) == 0;
        let yielded = random(&mut state, 50) + 1;
        let value = if fails { 0 } else { yielded };
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\nhandler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef f (x: Int): Int {{Int Yield, Error}} =>\n  if x == 0 => raise {{ message = \"x\" }}\n  | do yield x -> x\ndef main =\n  with Collect ->\n  with Catch ->\n  f {value}\n",
            collect_handler(),
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = if fails { "[]".to_string() } else { format!("[{yielded}]") };
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn transitive_callee_reuses_the_enclosing_answer_for_abort() {
    let case = Case::new();
    case.write("main.hd", "effect A\nabort a\neffect B\nabort b\nhandler A as OuterA\nabort a => \"outer-abort\"\nreturn value => \"outer-return\"\nhandler B as InnerB\nabort b => 0\nreturn value => value\ndef g (x: Int): Int {B} =>\n  if x == 0 => b | x\ndef f (x: Int): Int {A, B} =>\n  if x == 0 => a | g x\ndef main =\n  with OuterA ->\n  with InnerB ->\n  f 0\n");
    let built = case.build("main.hd");
    assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, b"outer-abort\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn transitive_callee_reuses_the_enclosing_answer_for_again() {
    for (value, expected) in [(0, "outer-abort"), (3, "outer-return")] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect A\nabort a\neffect B YieldB\nagain yieldB(value: Int): Void\nhandler A as OuterA\nabort a => \"outer-abort\"\nreturn value => \"outer-return\"\nhandler B YieldB as CollectB\nagain yieldB value =>\n  let rest = resume () ->\n  rest\nreturn _ => 0\ndef g (x: Int): Int {{Int YieldB}} =>\n  do yieldB x -> x\ndef f (x: Int): Int {{A, Int YieldB}} =>\n  if x == 0 => a | g x\ndef main =\n  with OuterA ->\n  with CollectB ->\n  f {value}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn nested_again_handlers_with_different_answers_align() {
    for (value, expected) in [(0, "[0]"), (3, "[]")] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\neffect B YieldB\nagain yieldB(value: B): Void\nhandler B YieldB as CollectB\nagain yieldB value =>\n  let rest = resume () ->\n  rest\nreturn _ => 0\ndef f (x: Int): Int {{Int Yield, Int YieldB}} =>\n  if x == 0 => do yield x -> x\n  | do yieldB x -> x\ndef main =\n  with Collect ->\n  with CollectB ->\n  f {value}\n",
            collect_handler(),
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn division_inside_lambda_under_handler_reaches_the_abort_arm() {
    for (body, expected) in [
        ("fn(x => 8 / x) 4", 2),
        ("fn(x => 8 / x) 0", 99),
        ("let f = fn(x => 8 / x) -> f 0", 99),
    ] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => 99\nreturn value => value\ndef main =\n  with Catch ->\n  {body}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{body}\n{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{body}\n{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes(), "{body}");
        assert!(output.stderr.is_empty(), "{body}");
    }
}

#[test]
fn error_handler_arm_does_not_catch_its_own_raise() {
    let case = Case::new();
    case.write("main.hd", "handler Error as H\nabort raise _ => 1 / 0\nreturn value => value\ndef main =\n  with H ->\n  1 / 0\n");
    let built = case.build("main.hd");
    assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert_eq!(output.stderr, b"DivisionByZero\n");
}

#[test]
fn error_handler_arm_raise_reaches_the_outer_handler() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Outer\nabort raise _ => 42\nreturn value => value\nhandler Error as Inner\nabort raise _ => 1 / 0\nreturn value => value\ndef main =\n  with Outer ->\n  with Inner ->\n  1 / 0\n");
    let built = case.build("main.hd");
    assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn direct_abort_and_yield_agree_on_the_innermost_answer() {
    for (condition, expected) in [("true", "[1]"), ("false", "[]")] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\nhandler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Collect ->\n  with Catch ->\n  if {condition} =>\n    do yield 1 -> 1\n  | raise {{ message = \"b\" }}\n",
            collect_handler(),
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{condition}\n{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{condition}\n{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes(), "{condition}");
        assert!(output.stderr.is_empty(), "{condition}");
    }
}

#[test]
fn once_tail_recursion_completes_ten_million_iterations() {
    let mut state = 0x26_05;
    for _ in 0..4 {
        let n = random(&mut state, 40) + 10;
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef rec loop (k: Int, acc: Int): Int {{Tick}} =>\n  if k <= 0 => acc\n  | do tick -> loop (k - 1) (acc + 1)\ndef main =\n  with Nop ->\n  loop {n} 0\n"
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
    let small_n = 1_000_000u64;
    let large_n = 10_000_000u64;
    let small = build_once_loop(small_n);
    let large = build_once_loop(large_n);
    assert_eq!(small.run.stdout, format!("{small_n}\n").as_bytes());
    assert_eq!(large.run.stdout, format!("{large_n}\n").as_bytes());
    assert!(large.rss_kb * small_n < small.rss_kb * large_n,
        "once loop RSS scaled with iterations: {} KB at {small_n}, {} KB at {large_n}",
        small.rss_kb, large.rss_kb);
}

struct Built {
    run: Output,
    rss_kb: u64,
}

fn build_once_loop(n: u64) -> Built {
    let case = Case::new();
    case.write("main.hd", &format!(
        "effect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef rec loop (k: Int, acc: Int): Int {{Tick}} =>\n  if k <= 0 => acc\n  | do tick -> loop (k - 1) (acc + 1)\ndef main =\n  with Nop ->\n  loop {n} 0\n"
    ));
    let built = case.build("main.hd");
    assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
    let bin = case.0.join("main");
    let run = Command::new(&bin).output().unwrap();
    assert_eq!(run.status.code(), Some(0), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(run.stderr.is_empty());
    Built { run, rss_kb: max_rss_kb(&bin) }
}

fn max_rss_kb(bin: &std::path::Path) -> u64 {
    let output = Command::new("/usr/bin/time").arg("-v").arg(bin).output().unwrap();
    String::from_utf8_lossy(&output.stderr).lines().find_map(|line| {
        let (label, value) = line.rsplit_once(':')?;
        label.contains("Maximum resident set size").then(|| value.trim().parse().unwrap())
    }).unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(&output.stderr)))
}

fn collect_handler() -> &'static str {
    "effect A Yield\nagain yield(value: A): Void\nhandler A Yield as Collect\nagain yield value =>\n  let rest = resume () ->\n  [value, ...rest]\nreturn _ => []\n"
}

fn list_literal(values: &[i64]) -> String {
    if values.is_empty() { "[]".into() } else {
        format!("[{}]", values.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "))
    }
}

#[test]
fn generator_collects_yielded_values() {
    let mut state = 0x26_06;
    for _ in 0..8 {
        let len = random(&mut state, 6);
        let values: Vec<i64> = (0..len).map(|_| random(&mut state, 50) as i64).collect();
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\ndef rec each (values: Int List): Void {{Int Yield}} =>\n  if values | [] => () | [head, ...tail] =>\n    do yield head ->\n    each tail\ndef main = with Collect -> each {}\n",
            collect_handler(),
            list_literal(&values),
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{}\n", list_literal(&values)).as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn again_loop_counts_each_yield() {
    let mut state = 0x26_07;
    for _ in 0..4 {
        let n = random(&mut state, 30) + 5;
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\ndef rec loop (k: Int): Void {{Int Yield}} =>\n  if k <= 0 => ()\n  | do yield k -> loop (k - 1)\ndef main = List.length (with Collect -> loop {n})\n",
            collect_handler(),
        ));
        let dump = case.command("dump-mono", "main.hd");
        assert_eq!(dump.status.code(), Some(0), "{}", String::from_utf8_lossy(&dump.stderr));
        let ir = String::from_utf8_lossy(&dump.stdout);
        assert!(ir.contains("MakeClosure"), "{ir}");
        assert!(ir.contains("CallClosure"), "{ir}");
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{n}\n").as_bytes());
    }
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}\ndef rec loop (n: Int): Void {{Int Yield}} =>\n  if n <= 0 => ()\n  | do yield n -> loop (n - 1)\ndef main = List.length (with Collect -> loop 10000)\n",
        collect_handler(),
    ));
    let built = case.build("main.hd");
    assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, b"10000\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn distinct_handlers_specialize_the_same_function_apart() {
    let mut state = 0x26_11;
    for _ in 0..8 {
        let one = random(&mut state, 20) + 1;
        let two = random(&mut state, 20) + 1;
        let left = random(&mut state, 20);
        let right = random(&mut state, 20);
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Ask\nonce ask: Int\nhandler Ask as One\nonce ask => {one}\nreturn value => value\nhandler Ask as Two\nonce ask => {two}\nreturn value => value\ndef use (x: Int): Int {{Ask}} => ask + x\ndef main = (with One -> use {left}) + (with Two -> use {right})\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{}\n", (one + left) + (two + right)).as_bytes());
    }
}

#[test]
fn abort_in_a_branch_skips_the_return_arm() {
    let mut state = 0x26_12;
    for _ in 0..8 {
        let n = random(&mut state, 11);
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as ToZero\nabort bail => 0\nreturn value => value + 1\ndef f (k: Int): Int {{Fail}} => if k > 0 => bail | k\ndef main = with ToZero -> f {n}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = if n > 0 { 0 } else { n + 1 };
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn abort_handler_return_arm_changes_the_with_type() {
    let mut state = 0x26_13;
    for _ in 0..8 {
        let n = random(&mut state, 11);
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as ToOpt\nabort bail => None\nreturn value => Some(value)\ndef f (k: Int): Int {{Fail}} => if k > 0 => k | bail\ndef main = with ToOpt -> f {n}\n"
        ));
        assert_eq!(case.print_type("main.hd"), "Int Option\n");
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = if n > 0 { format!("Some({n})\n") } else { "None\n".into() };
        assert_eq!(output.stdout, expected.as_bytes());
    }
}

#[test]
fn handler_arm_can_call_a_top_level_function() {
    let mut state = 0x26_14;
    for _ in 0..8 {
        let k = random(&mut state, 20) + 1;
        let x = random(&mut state, 20);
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Ask\nonce ask: Int\ndef double (n: Int): Int => n * 2\nhandler Ask as Fixed\nonce ask => double {k}\nreturn value => value\ndef use (y: Int): Int {{Ask}} => ask + y\ndef main = with Fixed -> use {x}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{}\n", k * 2 + x).as_bytes());
    }
}

#[test]
fn again_arm_keeps_pure_helpers_at_their_result_type() {
    let mut state = 0x26_15;
    for _ in 0..8 {
        let n = random(&mut state, 6) + 1;
        let doubled: Vec<i64> = (1..=n as i64).rev().map(|k| k * 2).collect();
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect A Yield\nagain yield(value: A): Void\ndef double (m: Int): Int => m * 2\nhandler A Yield as Collect\nagain yield value =>\n  let rest = resume () ->\n  [double value, ...rest]\nreturn _ => []\ndef rec loop (k: Int): Void {{Int Yield}} =>\n  if k <= 0 => ()\n  | do yield k -> loop (k - 1)\ndef main = with Collect -> loop {n}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, format!("{}\n", list_literal(&doubled)).as_bytes());
    }
}

#[test]
fn return_arm_runs_when_types_already_match() {
    let mut state = 0x26_21;
    for _ in 0..8 {
        let k = random(&mut state, 11);
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as H\nabort bail => 0\nreturn _ => 9\ndef f (n: Int): Int {{Fail}} => if n > 0 => n | bail\ndef main = with H -> f {k}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = if k > 0 { 9 } else { 0 };
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn return_arm_does_not_rewrite_a_non_tail_branch() {
    let mut state = 0x26_22;
    for _ in 0..8 {
        let k = random(&mut state, 11);
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as H\nabort bail => 0\nreturn value => value + 1\ndef f (n: Int): Int {{Fail}} => let x = (if n > 0 => n | bail) -> x + 10\ndef main = with H -> f {k}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = if k > 0 { k + 10 + 1 } else { 0 };
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn return_arm_does_not_rewrite_a_pure_helper() {
    let mut state = 0x26_23;
    for _ in 0..8 {
        let k = random(&mut state, 11);
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as H\nabort bail => None\nreturn value => Some(value)\ndef twice (n: Int): Int => n * 2\ndef f (n: Int): Int {{Fail}} => if n > 0 => twice n | bail\ndef main = with H -> f {k}\n"
        ));
        assert_eq!(case.print_type("main.hd"), "Int Option\n");
        let built = case.build("main.hd");
        assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
        let output = case.run("main.hd");
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let expected = if k > 0 { format!("Some({})\n", k * 2) } else { "None\n".into() };
        assert_eq!(output.stdout, expected.as_bytes());
    }
}
