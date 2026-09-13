use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(std::path::PathBuf);
impl Case {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r37-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("main.hd"), source).unwrap();
        Self(dir)
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado")).args(args).arg(self.0.join("main.hd")).output().unwrap()
    }
    fn prints_type(&self, expected: &str) {
        let output = self.command(&["check", "--print-type"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("{expected}\n"));
    }
    fn runs(&self, expected: i64) {
        let built = self.command(&["build"]);
        assert_eq!(built.status.code(), Some(0), "{built:?}");
        assert!(built.stderr.is_empty());
        let output = Command::new(self.0.join("main")).current_dir(&self.0).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("{expected}\n"));
    }
}
impl Drop for Case { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

#[test]
fn operation_result_rows_reach_the_call_without_effectful_handler_arms() {
    for (annotation, expected) in [("", "String"), (" {Error}", "String {Error}"), (" {Error, Bar}", "String {Bar, Error}")] {
        for (param, arm, call) in [("", "", "pick"), ("(n: Int)", " _", "pick 3")] {
            Case::new(&format!("effect Bar\nreply other: Int\neffect Foo\nreply pick{param}: Int{annotation}\nhandler Foo as H\nreply pick{arm} => 7\ndef main = Int.to_string (with H -> {call})\n")).prints_type(expected);
        }
    }
}

#[test]
fn operation_result_rows_remain_before_discharge() {
    Case::new("effect Foo\nreply pick: Int {Error}\ndef use (n: Int): Int {Foo, Error} => pick + n\ndef main = Int.to_string (use 3)\n").prints_type("String {Error, Foo}");
}

#[test]
fn value_parameters_shadow_operations_in_mono() {
    Case::new("def f (raise: Int): Int => raise\ndef main = Int.to_string (f 3)\n").runs(3);
}

#[test]
fn function_parameters_shadow_operations_in_infer_and_mono() {
    Case::new("def inc (n: Int): Int => n + 1\ndef f (raise: Fn(Int => Int)): Int => raise 3\ndef main = Int.to_string (f inc)\n").runs(4);
}

#[test]
fn let_bindings_shadow_operations() {
    Case::new("def main = Int.to_string (let raise = 5 -> raise + 1)\n").runs(6);
}

#[test]
fn user_operation_parameters_shadow_operations() {
    Case::new("effect Foo\nreply tick: Int\ndef f (tick: Int): Int => tick\ndef main = Int.to_string (f 3)\n").runs(3);
}

#[test]
fn shadowed_capture_is_a_local_let_value() {
    Case::new("effect Foo\ncapture tick(value: Int): Unit\ndef f (tick: Int): Int => let x = tick -> x + 1\ndef main = Int.to_string (f 8)\n").runs(9);
}
