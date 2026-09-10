use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hado-r28-{}-{}",
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
    fn build(&self, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg("build")
            .arg(self.0.join(entry))
            .output()
            .unwrap()
    }
    fn run(&self, entry: &str) -> Output {
        Command::new(self.0.join(entry).with_extension("")).output().unwrap()
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn random(state: &mut u64, limit: usize) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*state >> 32) as usize) % limit
}

fn run_main(source: &str) -> (i32, String) {
    let case = Case::new();
    case.write("main.hd", source);
    let built = case.build("main.hd");
    assert_eq!(built.status.code(), Some(0), "{}", String::from_utf8_lossy(&built.stderr));
    let output = case.run("main.hd");
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    (
        output.status.code().unwrap(),
        String::from_utf8(output.stdout).unwrap(),
    )
}

#[test]
fn unrelated_inner_handler_does_not_catch_outer_abort() {
    let mut state = 0x28_01;
    for _ in 0..8 {
        let outer = random(&mut state, 20) + 1;
        let inner = random(&mut state, 20) + 1;
        let add = random(&mut state, 11);
        let (status, stdout) = run_main(&format!(
            "effect A\nabort a\neffect B\nabort b\nhandler A as Outer\nabort a => {outer}\nreturn x => x + {add}\nhandler B as Inner\nabort b => {inner}\nreturn x => x + 100\ndef f (n: Int): Int {{A}} => if n > 0 => n | a\ndef main = with Outer -> with Inner -> f 0\n"
        ));
        assert_eq!(status, 0);
        assert_eq!(stdout, format!("{outer}\n"));
    }
}

#[test]
fn matching_inner_handler_catches_the_abort() {
    let mut state = 0x28_02;
    for _ in 0..8 {
        let outer = random(&mut state, 20) + 1;
        let inner = random(&mut state, 20) + 1;
        let add = random(&mut state, 11);
        let (status, stdout) = run_main(&format!(
            "effect A\nabort a\nhandler A as Outer\nabort a => {outer}\nreturn x => x + {add}\nhandler A as Inner\nabort a => {inner}\nreturn x => x + 100\ndef f (n: Int): Int {{A}} => if n > 0 => n | a\ndef main = with Outer -> with Inner -> f 0\n"
        ));
        assert_eq!(status, 0);
        assert_eq!(stdout, format!("{}\n", inner + add));
    }
}

#[test]
fn with_inside_abort_arm_applies_its_return() {
    let mut state = 0x28_03;
    for _ in 0..8 {
        let reply = random(&mut state, 20);
        let add = random(&mut state, 11);
        let (status, stdout) = run_main(&format!(
            "effect A\nabort a\neffect B\nreply b: Int\nhandler A as Outer\nabort a => with Inner -> b\nreturn x => x * 10\nhandler B as Inner\nreply b => {reply}\nreturn x => x + {add}\ndef f (n: Int): Int {{A}} => if n > 0 => n | a\ndef main = with Outer -> f 0\n"
        ));
        assert_eq!(status, 0);
        assert_eq!(stdout, format!("{}\n", reply + add));
    }
}

#[test]
fn abort_arm_without_with_skips_the_return_arm() {
    let mut state = 0x28_04;
    for _ in 0..8 {
        let abort = random(&mut state, 20);
        let (status, stdout) = run_main(&format!(
            "effect A\nabort a\nhandler A as Outer\nabort a => {abort}\nreturn x => x * 10\ndef f (n: Int): Int {{A}} => if n > 0 => n | a\ndef main = with Outer -> f 0\n"
        ));
        assert_eq!(status, 0);
        assert_eq!(stdout, format!("{abort}\n"));
    }
}

#[test]
fn abort_arm_can_call_an_effectful_function() {
    let mut state = 0x28_05;
    for _ in 0..8 {
        let success = random(&mut state, 20) + 1;
        let add = random(&mut state, 11);
        let (status, stdout) = run_main(&format!(
            "effect A\nabort a\nhandler A as G\nabort a => 99\nreturn x => x + {add}\nhandler A as H\nabort a => f {success}\nreturn x => x + 10\ndef f (n: Int): Int {{A}} => if n > 0 => n | a\ndef main = with G -> with H -> f 0\n"
        ));
        assert_eq!(status, 0);
        assert_eq!(stdout, format!("{}\n", success + add));
    }
}
