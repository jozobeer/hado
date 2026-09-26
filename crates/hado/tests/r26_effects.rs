use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hado-r26-{}-{}",
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
    fn run(&self, entry: &str) -> Output {
        Command::new(self.0.join(entry).with_extension(""))
            .output()
            .unwrap()
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
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
fn abort_handler_runs_do_continuation_when_the_rhs_returns() {
    for (argument, expected) in [(0, "3"), (1, "9")] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => 3\nreturn value => value\ndef f x => if x == 0 => raise {{ message = \"x\" }} | x\ndef main =\n  with Catch ->\n  do f {argument} ->\n  9\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_integer_division_by_zero() {
    for op in ["/", "%"] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => 5\nreturn value => value\ndef main =\n  with Catch ->\n  7 {op} 0\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"5\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_division_inside_arithmetic_and_calls() {
    let case = Case::new();
    case.write(
        "main.hd",
        "handler Error as Catch\nabort raise _ => 5\nreturn value => value\ndef f x => x / 0\ndef main =\n  with Catch ->\n  1 + (f 4)\n",
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"5\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn abort_handler_skips_do_continuation_on_division_by_zero() {
    let case = Case::new();
    case.write(
        "main.hd",
        "handler Error as Catch\nabort raise _ => 5\nreturn value => value\ndef main =\n  with Catch ->\n  do (7 / 0) ->\n  9\n",
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"5\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn abort_handler_do_continuation_may_differ_from_the_rhs_success_type() {
    for (argument, expected) in [(0, "caught"), (1, "continued")] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => \"caught\"\nreturn value => value\ndef f x => if x == 0 => raise {{ message = \"x\" }} | x\ndef main =\n  with Catch ->\n  do f {argument} ->\n  \"continued\"\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_catches_division_outside_an_unrelated_once_handler() {
    for (helper, body) in [
        ("", "1 + (7 / 0)"),
        ("def f x => x / 0\n", "1 + (f 7)"),
        ("", "1 + (raise { message = \"x\" })"),
    ] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "handler Error as Catch\nabort raise _ => 5\nreturn value => value\neffect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\n{helper}def main =\n  with Catch ->\n  with Nop ->\n  {body}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"5\n");
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn abort_handler_outside_once_still_runs_the_once_body() {
    let case = Case::new();
    case.write(
        "main.hd",
        "handler Error as Catch\nabort raise _ => 0\nreturn value => value\neffect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef rec loop (k: Int, acc: Int): Int {Tick} =>\n  if k <= 0 => acc\n  | do tick -> loop (k - 1) (acc + 1)\ndef main =\n  with Catch ->\n  with Nop ->\n  loop 3 0\n",
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn again_handler_outside_abort_handler_forwards_callee_raise() {
    for (body, effects, value, expected) in [
        ("  do yield x -> x\n", "{Int Yield}", 7, "[7]"),
        (
            "  if x == 0 => raise { message = \"x\" }\n  | do yield x -> x\n",
            "{Int Yield, Error}",
            0,
            "[]",
        ),
        (
            "  if x == 0 => raise { message = \"x\" }\n  | do yield x -> x\n",
            "{Int Yield, Error}",
            7,
            "[7]",
        ),
    ] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\nhandler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef f (x: Int): Int {effects} =>\n{body}def main =\n  with Collect ->\n  with Catch ->\n  f {value}\n",
            collect_handler(),
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn transitive_callee_reuses_the_enclosing_answer_for_abort() {
    let case = Case::new();
    case.write("main.hd", "effect A\nabort a\neffect B\nabort b\nhandler A as OuterA\nabort a => \"outer-abort\"\nreturn value => \"outer-return\"\nhandler B as InnerB\nabort b => 0\nreturn value => value\ndef g (x: Int): Int {B} =>\n  if x == 0 => b | x\ndef f (x: Int): Int {A, B} =>\n  if x == 0 => a | g x\ndef main =\n  with OuterA ->\n  with InnerB ->\n  f 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
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
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
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
        assert_eq!(
            built.status.code(),
            Some(0),
            "{body}\n{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{body}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes(), "{body}");
        assert!(output.stderr.is_empty(), "{body}");
    }
}

#[test]
fn error_handler_arm_does_not_catch_its_own_raise() {
    let case = Case::new();
    case.write("main.hd", "handler Error as H\nabort raise _ => 1 / 0\nreturn value => value\ndef main =\n  with H ->\n  1 / 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
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
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
        assert_eq!(
            built.status.code(),
            Some(0),
            "{condition}\n{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{condition}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            format!("{expected}\n").as_bytes(),
            "{condition}"
        );
        assert!(output.stderr.is_empty(), "{condition}");
    }
}

#[test]
fn callee_case_from_a_handler_arm_escapes_that_handler() {
    let case = Case::new();
    case.write("main.hd", "handler Error as H\nabort raise _ => f 0\nreturn value => value\ndef f (x: Int): Int {Error} => x / 0\ndef main =\n  with H ->\n  1 / 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert_eq!(output.stderr, b"DivisionByZero\n");

    let case = Case::new();
    case.write("main.hd", "handler Error as G\nabort raise _ => 42\nreturn value => value\nhandler Error as H\nabort raise _ => f 0\nreturn value => value\ndef f (x: Int): Int {Error} => x / 0\ndef main =\n  with G ->\n  with H ->\n  1 / 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"42\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn mutual_recursion_forwards_a_late_raise_across_a_success_type_change() {
    let case = Case::new();
    case.write(
        "main.hd",
        "handler Error as Catch\nabort raise _ => \"caught\"\nreturn value => value\ndef rec f (x: Int): String {Error} =>\n  if x > 0 =>\n    let n = g (x - 1) ->\n    Int.to_string n\n  | raise { message = \"f\" }\ndef rec g (x: Int): Int {Error} =>\n  if x == 0 =>\n    let s = f (x - 1) ->\n    String.length s\n  | raise { message = \"g\" }\ndef main =\n  with Catch ->\n  f 1\n",
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"caught\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn arm_call_does_not_reuse_a_body_specialization() {
    let case = Case::new();
    case.write("main.hd", "effect A\nabort a\nhandler A as OuterA\nabort a => 100\nreturn value => value\nhandler Error as Catch\nabort raise _ => f 0\nreturn value => value\ndef f (x: Int): Int {A, Error}\n| x < 0 => a\n| x == 0 => raise { message = \"f\" }\n| x\ndef main =\n  with OuterA ->\n  with Catch ->\n  do f 1 -> raise { message = \"body\" }\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "{:?}", output.stdout);
    assert_eq!(output.stderr, b"f\n");
}

#[test]
fn do_binds_a_named_again_callee_with_the_caller_continuation() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}def f (x: Int): Void {{Int Yield}} => do yield x -> ()\ndef main =\n  with Collect ->\n  do f 1 -> do yield 2 -> ()\n",
        collect_handler(),
    ));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 2]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn effectful_callback_runs_under_the_handler() {
    let aborting = [
        "handler Error as Catch\nabort raise _ => []\nreturn value => value\ndef main =\n  with Catch ->\n  List.map [2, 0] fn(x => 8 / x)\n",
        "handler Error as Catch\nabort raise _ => []\nreturn value => value\ndef f (x: Int): Int {Error} => 8 / x\ndef main =\n  with Catch ->\n  List.map [2, 0] f\n",
    ];
    for source in aborting {
        let case = Case::new();
        case.write("main.hd", source);
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{source}\n{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"[]\n", "{source}");
        assert!(output.stderr.is_empty(), "{source}");
    }

    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => []\nreturn value => value\ndef main =\n  with Catch ->\n  List.map [2, 4] fn(x => 8 / x)\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"[4, 2]\n");
    assert!(output.stderr.is_empty());

    let case = Case::new();
    case.write(
        "main.hd",
        &format!(
            "{}def main =\n  with Collect ->\n  List.map [1, 2] fn(x => do yield x -> x)\n",
            collect_handler(),
        ),
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 2]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn list_filter_applies_the_predicate_from_the_head() {
    let case = Case::new();
    case.write(
        "main.hd",
        &format!(
            "{}def main =\n  with Collect ->\n  List.filter [1, 2, 3] fn(x => do yield x -> x != 2)\n",
            collect_handler(),
        ),
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 2, 3]\n");
    assert!(output.stderr.is_empty());

    // 述語が先頭から呼ばれるので、0 番目で Error が上がり、yield は [0] で止まる。
    let case = Case::new();
    case.write(
        "main.hd",
        &format!(
            "{}handler Error as Catch\nabort raise _ => []\nreturn value => value\ndef main =\n  with Collect ->\n  with Catch ->\n  List.filter [0, 1] fn(x => do yield x -> 8 / x > 0)\n",
            collect_handler(),
        ),
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[0]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn do_binds_a_local_closure_callee_with_the_caller_continuation() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}def main =\n  with Collect ->\n  let f = fn(x => do yield x -> ()) ->\n  do f 1 -> do yield 2 -> ()\n",
        collect_handler(),
    ));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 2]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn once_operation_with_a_handled_effect_is_lifted_in_argument_position() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  String.length cwd\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().parse::<i64>().is_ok(), "{stdout}");
    assert!(output.stderr.is_empty());
}

#[test]
fn late_registered_yield_composes_with_the_caller_continuation() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}def rec f (x: Int): Int {{Int Yield}} =>\n  if x == 0 => g 0\n  | do yield x -> 7\ndef rec g (x: Int): Int {{Int Yield}} => do f 1 -> 42 + x\ndef main =\n  with Collect ->\n  f 0\n",
        collect_handler().replace("return _ => []", "return value => [value]"),
    ));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 42]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn do_binds_a_branch_callee_with_the_caller_continuation() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}def main =\n  with Collect ->\n  let f = fn(x => do yield x -> ()) ->\n  let g = fn(x => do yield x -> ()) ->\n  do (if true => f | g) 1 -> do yield 2 -> ()\n",
        collect_handler(),
    ));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 2]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn effectful_top_level_value_is_lifted_in_argument_position() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef current = cwd\ndef main =\n  with Catch ->\n  String.length current\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.trim().parse::<i64>().is_ok(), "{stdout}");
    assert!(output.stderr.is_empty());
}

#[test]
fn effectful_callback_reaches_the_handler_through_prelude_folds() {
    let cases: [(&str, &str); 4] = [
        ("handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  List.fold [2, 4] 0 fn(acc, x => acc + 8 / x)\n", "6\n"),
        ("handler Error as Catch\nabort raise _ => []\nreturn value => value\ndef main =\n  with Catch ->\n  List.filter [2, 0, 4] fn(x => 8 / x > 1)\n", "[]\n"),
        ("handler Error as Catch\nabort raise _ => []\nreturn value => value\ndef main =\n  with Catch ->\n  List.map [[2, 0], [4]] fn(xs => List.map xs fn(x => 8 / x))\n", "[]\n"),
        ("handler Error as Catch\nabort raise _ => None\nreturn value => value\ndef main =\n  with Catch ->\n  Option.map (Some(0)) fn(x => 8 / x)\n", "None\n"),
    ];
    for (source, expected) in cases {
        let case = Case::new();
        case.write("main.hd", source);
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{source}\n{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{source}"
        );
        assert!(output.stderr.is_empty(), "{source}");
    }

    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => None\nreturn value => value\ndef main =\n  with Catch ->\n  Option.map (Some(4)) fn(x => 8 / x)\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"Some(2)\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn callback_passed_through_a_pure_callee_keeps_its_own_result() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef identity (f: Fn(Int => Int)): Fn(Int => Int) => f\ndef main =\n  with Catch ->\n  let f = identity fn(x => 8 / x) ->\n  f 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn do_binds_a_local_callback_passed_as_an_argument() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}def main =\n  with Collect ->\n  let cb = fn(x => do yield x -> x) ->\n  do List.map [1] cb -> do yield 2 -> ()\n",
        collect_handler(),
    ));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 2]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn do_binds_a_callback_held_in_a_record_field() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}type Box = {{ run: Fn(Int => Void {{Int Yield}}) }}\ndef main =\n  with Collect ->\n  let box = {{ run = fn(x => do yield x -> ()) }} ->\n  do box.run 1 -> do yield 2 -> ()\n",
        collect_handler(),
    ));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 2]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn an_effectful_closure_in_an_adt_field_keeps_its_outcome() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let opt = Some(fn(x => 8 / x)) ->\n  if opt | Some(f) => f 0 | None => 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn an_effectful_closure_in_an_array_slot_keeps_its_outcome() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let xs = Array.of_list [fn(x => 8 / x)] ->\n  let f = Array.get xs 0 ->\n  f 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn an_effectful_parameter_does_not_restore_the_result_of_another() {
    // f と g の論理型（outcome を外した形）が同じなので、結果型 `g` の出現を
    // f の実際の型（効果付き）へ戻してはならない。
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef select(f: Fn(Int => Int {Error}), g: Fn(Int => Int)): Fn(Int => Int) => do f 1 -> g\ndef main =\n  with Catch ->\n  let h = select fn(x => 8 / x) fn(x => x + 1) ->\n  h 1\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"2\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn an_effectful_parameter_does_not_restore_a_written_result() {
    // 引数が 1 つでも、scheme が結果型そのものに関数型を書いているときは、その
    // 出現を引数の実際の型（効果付き）へ戻してはならない。戻り値の実体は本体が
    // 組み立てる別の閉包である（`fn(x => x + 1)` は効果を持たない）。
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef replace(f: Fn(Int => Int {Error})): Fn(Int => Int) => do f 1 -> fn(x => x + 1)\ndef main =\n  with Catch ->\n  let h = replace fn(x => 8 / x) ->\n  h 1\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"2\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn an_effectful_closure_in_a_monomorphic_adt_field_keeps_its_outcome() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ntype Box = | Box(Fn(Int => Int {Error}))\ndef main =\n  with Catch ->\n  let Box(f) = Box(fn(x => 8 / x)) ->\n  f 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn a_lambda_may_return_an_effectful_lambda() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let make = fn(x => fn(y => x / y)) ->\n  let divide = make 8 ->\n  divide 0\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn a_handled_once_operation_with_a_yielding_arm_is_lifted() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}effect Fixed\nonce ask: Int {{Int Yield}}\nhandler Fixed as H\nonce ask => do yield 1 -> 4\ndef main =\n  with Collect ->\n  with H ->\n  1 + ask\n",
        collect_handler(),
    ));
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn once_tail_recursion_completes_ten_million_iterations() {
    let case = Case::new();
    case.write(
        "main.hd",
        "effect Tick\nonce tick: Void\nhandler Tick as Nop\nonce tick => ()\ndef rec loop (k: Int, acc: Int): Int {Tick} =>\n  if k <= 0 => acc\n  | do tick -> loop (k - 1) (acc + 1)\ndef main =\n  with Nop ->\n  loop 20 0\n",
    );
    let dump = case.command("dump-mono", "main.hd");
    assert_eq!(
        dump.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dump.stderr)
    );
    let ir = String::from_utf8_lossy(&dump.stdout);
    assert!(ir.contains("TailCall"), "{ir}");
    assert!(!ir.contains("Yield"), "{ir}");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"20\n");
    let small_n = 1_000_000u64;
    let large_n = 10_000_000u64;
    let small = build_once_loop(small_n);
    let large = build_once_loop(large_n);
    assert_eq!(small.run.stdout, format!("{small_n}\n").as_bytes());
    assert_eq!(large.run.stdout, format!("{large_n}\n").as_bytes());
    assert!(
        large.rss_kb * small_n < small.rss_kb * large_n,
        "once loop RSS scaled with iterations: {} KB at {small_n}, {} KB at {large_n}",
        small.rss_kb,
        large.rss_kb
    );
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
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let bin = case.0.join("main");
    let run = Command::new(&bin).output().unwrap();
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.stderr.is_empty());
    Built {
        run,
        rss_kb: max_rss_kb(&bin),
    }
}

fn max_rss_kb(bin: &std::path::Path) -> u64 {
    let output = Command::new("/usr/bin/time")
        .arg("-v")
        .arg(bin)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .find_map(|line| {
            let (label, value) = line.rsplit_once(':')?;
            label
                .contains("Maximum resident set size")
                .then(|| value.trim().parse().unwrap())
        })
        .unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(&output.stderr)))
}

fn collect_handler() -> &'static str {
    "effect A Yield\nagain yield(value: A): Void\nhandler A Yield as Collect\nagain yield value =>\n  let rest = resume () ->\n  [value, ...rest]\nreturn _ => []\n"
}

fn list_literal(values: &[i64]) -> String {
    if values.is_empty() {
        "[]".into()
    } else {
        format!(
            "[{}]",
            values
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[test]
fn generator_collects_yielded_values() {
    for values in [&[][..], &[3, 1, 4][..]] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "{}\ndef rec each (values: Int List): Void {{Int Yield}} =>\n  if values | [] => () | [head, ...tail] =>\n    do yield head ->\n    each tail\ndef main = with Collect -> each {}\n",
            collect_handler(),
            list_literal(values),
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            format!("{}\n", list_literal(values)).as_bytes()
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn again_loop_counts_each_yield() {
    let case = Case::new();
    case.write("main.hd", &format!(
        "{}\ndef rec loop (k: Int): Void {{Int Yield}} =>\n  if k <= 0 => ()\n  | do yield k -> loop (k - 1)\ndef main = List.length (with Collect -> loop 6)\n",
        collect_handler(),
    ));
    let dump = case.command("dump-mono", "main.hd");
    assert_eq!(
        dump.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dump.stderr)
    );
    let ir = String::from_utf8_lossy(&dump.stdout);
    assert!(ir.contains("MakeClosure"), "{ir}");
    assert!(ir.contains("CallClosure"), "{ir}");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"6\n");
}

#[test]
fn distinct_handlers_specialize_the_same_function_apart() {
    let case = Case::new();
    case.write(
        "main.hd",
        "effect Ask\nonce ask: Int\nhandler Ask as One\nonce ask => 2\nreturn value => value\nhandler Ask as Two\nonce ask => 3\nreturn value => value\ndef use (x: Int): Int {Ask} => ask + x\ndef main = (with One -> use 4) + (with Two -> use 5)\n",
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"14\n");
}

#[test]
fn abort_in_a_branch_skips_the_return_arm() {
    for (n, expected) in [(0, 1), (1, 0)] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as ToZero\nabort bail => 0\nreturn value => value + 1\ndef f (k: Int): Int {{Fail}} => if k > 0 => bail | k\ndef main = with ToZero -> f {n}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn abort_handler_return_arm_changes_the_with_type() {
    let entry = Case::new();
    entry.write(
        "main.hd",
        "effect Fail\nabort bail\nhandler Fail as ToOpt\nabort bail => None\nreturn value => Some(value)\ndef f (k: Int): Int {Fail} => if k > 0 => k | bail\ndef main = with ToOpt -> f 1\n",
    );
    assert_eq!(entry.print_type("main.hd"), "Int Option\n");
    for (n, expected) in [(0, "None"), (1, "Some(1)")] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as ToOpt\nabort bail => None\nreturn value => Some(value)\ndef f (k: Int): Int {{Fail}} => if k > 0 => k | bail\ndef main = with ToOpt -> f {n}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn again_arm_keeps_pure_helpers_at_their_result_type() {
    let case = Case::new();
    case.write(
        "main.hd",
        "effect A Yield\nagain yield(value: A): Void\ndef double (m: Int): Int => m * 2\nhandler A Yield as Collect\nagain yield value =>\n  let rest = resume () ->\n  [double value, ...rest]\nreturn _ => []\ndef rec loop (k: Int): Void {Int Yield} =>\n  if k <= 0 => ()\n  | do yield k -> loop (k - 1)\ndef main = with Collect -> loop 3\n",
    );
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[6, 4, 2]\n");
}

#[test]
fn return_arm_runs_when_types_already_match() {
    for (k, expected) in [(0, 0), (1, 9)] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as H\nabort bail => 0\nreturn _ => 9\ndef f (n: Int): Int {{Fail}} => if n > 0 => n | bail\ndef main = with H -> f {k}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn return_arm_does_not_rewrite_a_non_tail_branch() {
    for (k, expected) in [(0, 0), (1, 12)] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as H\nabort bail => 0\nreturn value => value + 1\ndef f (n: Int): Int {{Fail}} => let x = (if n > 0 => n | bail) -> x + 10\ndef main = with H -> f {k}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn return_arm_does_not_rewrite_a_pure_helper() {
    let entry = Case::new();
    entry.write(
        "main.hd",
        "effect Fail\nabort bail\nhandler Fail as H\nabort bail => None\nreturn value => Some(value)\ndef twice (n: Int): Int => n * 2\ndef f (n: Int): Int {Fail} => if n > 0 => twice n | bail\ndef main = with H -> f 1\n",
    );
    assert_eq!(entry.print_type("main.hd"), "Int Option\n");
    for (k, expected) in [(0, "None"), (1, "Some(2)")] {
        let case = Case::new();
        case.write("main.hd", &format!(
            "effect Fail\nabort bail\nhandler Fail as H\nabort bail => None\nreturn value => Some(value)\ndef twice (n: Int): Int => n * 2\ndef f (n: Int): Int {{Fail}} => if n > 0 => twice n | bail\ndef main = with H -> f {k}\n"
        ));
        let built = case.build("main.hd");
        assert_eq!(
            built.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = case.run("main.hd");
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    }
}

#[test]
fn an_effectful_closure_nested_in_a_monomorphic_adt_field_arg_keeps_its_outcome() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ntype Box = | Box(Fn(Int => Int {Error}) Option)\ndef main =\n  with Catch ->\n  let Box(g) = Box(Some(fn(x => 8 / x))) ->\n  if g | Some(f) => f 0 | None => -1\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn a_lambda_may_return_an_adt_holding_an_effectful_lambda() {
    let case = Case::new();
    case.write("main.hd", "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let make = fn(x => Some(fn(y => x / y))) ->\n  let holder = make 8 ->\n  if holder | Some(f) => f 0 | None => -1\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn an_operation_in_a_nested_handler_arm_does_not_reach_the_entered_handler() {
    let case = Case::new();
    case.write("main.hd", "effect A\nonce askA: Int\neffect B\nonce askB: Int\nhandler A as HA\nonce askA => with HB -> askB\nreturn value => value\nhandler B as HB\nonce askB => askA\nreturn value => value\nhandler A as HA2\nonce askA => 7\nreturn value => 0\ndef main =\n  with HA2 ->\n  with HA ->\n  askA\n");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"0\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn once_handler_arm_must_match_the_operation_result_type() {
    let source = "effect Ask\nonce ask: Int\nhandler Ask as H\nonce ask => \"hello\"\nreturn value => value\ndef main = with H -> ask\n";
    let case = Case::new();
    case.write("main.hd", source);
    case.reject(
        "check",
        "main.hd",
        "main.hd",
        "E305",
        source.find("\"hello\"").unwrap(),
    );
}

#[test]
fn once_handler_arm_must_bind_every_operation_parameter() {
    let source = "effect Ask\nonce ask(name: String): Int\nhandler Ask as H\nonce ask => 1\nreturn value => value\ndef main = with H -> ask \"x\"\n";
    let case = Case::new();
    case.write("main.hd", source);
    case.reject(
        "check",
        "main.hd",
        "main.hd",
        "E301",
        source.find("once ask =>").unwrap() + "once ".len(),
    );
}

#[test]
fn each_again_operation_of_one_effect_runs_its_own_arm() {
    let case = Case::new();
    case.write(
        "main.hd",
        "effect A\nagain ping(value: Int): Void\nagain pong(value: Int): Void\nhandler A as H\nagain ping value =>\n  let rest = resume () ->\n  [value, ...rest]\nagain pong value =>\n  let rest = resume () ->\n  [value + 100, ...rest]\nreturn _ => []\ndef main = with H -> do ping 1 -> do pong 2 -> ()\n",
    );
    let dump = case.command("dump-mono", "main.hd");
    assert_eq!(
        dump.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dump.stderr)
    );
    let ir = String::from_utf8_lossy(&dump.stdout);
    assert!(ir.contains("$Yield#1"), "{ir}");
    assert!(ir.contains("$Yield#2"), "{ir}");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"[1, 102]\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn each_abort_operation_of_one_effect_runs_its_own_arm() {
    let case = Case::new();
    case.write(
        "main.hd",
        "effect E\nabort miss\nabort bad\nhandler E as H\nabort miss => \"miss\"\nabort bad => \"bad\"\nreturn v => v\ndef f (x: Int): String {E} => if x == 0 => miss | bad\ndef main = with H -> f 1\n",
    );
    let dump = case.command("dump-mono", "main.hd");
    assert_eq!(
        dump.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&dump.stderr)
    );
    let ir = String::from_utf8_lossy(&dump.stdout);
    assert!(ir.contains("$Fail#1"), "{ir}");
    assert!(ir.contains("$Fail#2"), "{ir}");
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"bad\n");
    assert!(output.stderr.is_empty());
}

fn runs_effectful_closure_source(source: &str, expected: &str) {
    let case = Case::new();
    case.write("main.hd", source);
    let built = case.build("main.hd");
    assert_eq!(
        built.status.code(),
        Some(0),
        "{source}\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let output = case.run("main.hd");
    assert_eq!(
        output.status.code(),
        Some(0),
        "{source}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        format!("{expected}\n").as_bytes(),
        "{source}"
    );
    assert!(output.stderr.is_empty(), "{source}");
}

#[test]
fn reverse_over_effectful_closures_keeps_their_outcome() {
    runs_effectful_closure_source(
        "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let fs = List.reverse [fn(x => 8 / x), fn(x => 9 / x)] ->\n  if fs | [g, ...rest] => g 0 + List.length rest | 5\n",
        "0",
    );
}

#[test]
fn map_over_effectful_closures_keeps_their_outcome() {
    runs_effectful_closure_source(
        "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  List.map [fn(x => 8 / x), fn(x => 9 / x)] fn(f => f 3) |> List.length\n",
        "2",
    );
}

#[test]
fn array_to_list_of_effectful_closures_keeps_their_outcome() {
    runs_effectful_closure_source(
        "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let xs = Array.of_list [fn(x => 8 / x), fn(x => 9 / x)] ->\n  let fs = Array.to_list xs ->\n  List.map fs fn(f => f 2) |> List.length\n",
        "2",
    );
}

#[test]
fn array_set_of_effectful_closures_keeps_their_outcome() {
    runs_effectful_closure_source(
        "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let xs = Array.of_list [fn(x => 8 / x)] ->\n  let ys = Array.set xs 0 fn(x => x * 3 / x) ->\n  let f = Array.get ys 0 ->\n  f 5\n",
        "3",
    );
}

#[test]
fn contract_member_returning_a_closure_over_an_effectful_argument() {
    runs_effectful_closure_source(
        "handler Error as Catch\nabort raise _ => 0\nreturn value => value\ndef main =\n  with Catch ->\n  let x = Some(fn(y => 8 / y)) ->\n  let f = Maker.make x ->\n  f x\nmodule Maker\ndef A make(x: A): Fn(A => Int) Pure\nimplement Maker as Maker\ndef make _ => fn(_ => 1)\n",
        "1",
    );
}
