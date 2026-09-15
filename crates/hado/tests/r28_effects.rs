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
        let once = random(&mut state, 20);
        let add = random(&mut state, 11);
        let (status, stdout) = run_main(&format!(
            "effect A\nabort a\neffect B\nonce b: Int\nhandler A as Outer\nabort a => with Inner -> b\nreturn x => x * 10\nhandler B as Inner\nonce b => {once}\nreturn x => x + {add}\ndef f (n: Int): Int {{A}} => if n > 0 => n | a\ndef main = with Outer -> f 0\n"
        ));
        assert_eq!(status, 0);
        assert_eq!(stdout, format!("{}\n", once + add));
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

fn fnv1a_odd(effect: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for byte in effect.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16777619);
    }
    hash | 1
}

fn colliding_effect_names() -> (String, String) {
    let mut seen = std::collections::HashMap::new();
    for index in 0..1_000_000u32 {
        let name = format!("E{index}");
        if let Some(previous) = seen.insert(fnv1a_odd(&name), name.clone()) {
            return (previous, name);
        }
    }
    panic!("no colliding effect names in search range");
}

#[test]
fn hashed_fail_tags_that_collided_stay_distinct() {
    let (outer, inner) = colliding_effect_names();
    assert_eq!(fnv1a_odd(&outer), fnv1a_odd(&inner));
    let (status, stdout) = run_main(&format!(
        "effect {outer}\nabort a\neffect {inner}\nabort b\nhandler {outer} as Outer\nabort a => 7\nreturn x => x + 10\nhandler {inner} as Inner\nabort b => 2\nreturn x => x + 100\ndef f (n: Int): Int {{{outer}}} => if n > 0 => n | a\ndef main = with Outer -> with Inner -> f 0\n"
    ));
    assert_eq!(status, 0);
    assert_eq!(stdout, "7\n");
}
