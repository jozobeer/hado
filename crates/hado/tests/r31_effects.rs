use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn run_main(body: &str, expected: &[i64]) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "hado-r31-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = format!(
        "effect Y\nagain y(value: Int): Unit\nhandler Y as Keep\nagain y value => let rest = resume () -> [value, ...rest]\nreturn x => x\n{body}\n"
    );
    std::fs::write(dir.join("main.hd"), &source).unwrap();
    let built = Command::new(env!("CARGO_BIN_EXE_hado"))
        .arg("build").arg(dir.join("main.hd")).output().unwrap();
    let output = built.status.success().then(|| Command::new(dir.join("main")).output().unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(built.status.success(), "{source}\n{}", String::from_utf8_lossy(&built.stderr));
    let output = output.unwrap();
    assert!(output.status.success(), "{source}\n{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("{expected:?}\n"), "{source}");
}

fn payloads() -> (i64, i64) {
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let a = i64::from(seed % 100_000);
    (a, a + 1)
}

#[test]
fn branch_result_is_bound_after_the_selected_again() {
    let (a, b) = payloads();
    run_main(&format!(
        "def f u => if u == () => [7] | []\ndef main = with Keep -> let u = if true => y {a} | y {b} -> f u"
    ), &[a, 7]);
}

#[test]
fn short_circuit_takes_the_right_branch_only_when_needed() {
    let (a, b) = payloads();
    run_main(&format!(
        "def g u => if u == () => true | false\ndef main = with Keep -> let b = false && g (y {a}) -> if b => [] | [{b}]"
    ), &[b]);
    run_main(&format!(
        "def g u => if u == () => true | false\ndef main = with Keep -> let b = true && g (y {a}) -> if b => [{b}] | []"
    ), &[a, b]);
    run_main(&format!(
        "def g u => if u == () => false | true\ndef main = with Keep -> let b = false || g (y {a}) -> if b => [] | [{b}]"
    ), &[a, b]);
}

#[test]
fn compound_do_keeps_the_following_computation() {
    let (a, b) = payloads();
    run_main(&format!(
        "def g (u: Unit) => u\ndef main = with Keep -> do g (y {a}) -> [{b}]"
    ), &[a, b]);
}

#[test]
fn pattern_scrutinee_is_evaluated_before_its_arm() {
    let (a, b) = payloads();
    run_main(&format!(
        "type Box = | Empty | Full(Unit)\ndef main = with Keep -> if Full(y {a}) | Full(_) => [{b}] | Empty => []"
    ), &[a, b]);
}

#[test]
fn again_arguments_are_evaluated_before_the_outer_operation() {
    let (a, b) = payloads();
    run_main(&format!(
        "def n u => if u == () => {b} | 0\ndef main = with Keep -> do y (n (y {a})) -> [7]"
    ), &[a, b, 7]);
}

struct Case(std::path::PathBuf);

impl Case {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hado-r31-types-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("main.hd"), source).unwrap();
        Self(dir)
    }

    fn run_main(&self, expected: &str) {
        let built = Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg("build").arg(self.0.join("main.hd")).output().unwrap();
        assert!(built.status.success(), "{}", String::from_utf8_lossy(&built.stderr));
        let output = Command::new(self.0.join("main")).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("{expected}\n"));
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn random(state: &mut u64) -> u64 {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (*state >> 32) % 100_000 + 1
}

#[test]
fn non_unit_again_result_is_bound() {
    let mut state = 0x31_08;
    for _ in 0..4 {
        let seed = random(&mut state);
        Case::new(&format!(
            "effect Ask\nagain ask(seed: Int): Int\nhandler Ask as Twice\nagain ask seed => resume (seed * 2)\nreturn x => x\ndef main = with Twice -> let v = ask {seed} -> [v]\n"
        )).run_main(&format!("[{}]", seed * 2));
    }
}

fn mixed_source(body: &str, prefix: &str) -> String {
    format!(
        "effect S\nagain s(value: String): Unit\neffect Y\nagain y(value: Int): Unit\nhandler S as Names\nagain s value => let rest = resume () -> [value, ...rest]\nreturn x => x\nhandler Y as Collect\nagain y value => let rest = resume () -> [String.concat \"{prefix}\" (Int.to_string value), ...rest]\nreturn _ => []\ndef main = with Names -> with Collect -> {body}\n"
    )
}

#[test]
fn different_payload_types_compose() {
    let mut state = 0x31_09;
    for _ in 0..4 {
        let number = random(&mut state);
        let text = format!("s{}", random(&mut state));
        Case::new(&mixed_source(&format!("do s {text:?} -> do y {number} -> ()"), "n"))
            .run_main(&format!("[{text:?}, \"n{number}\"]"));
    }
}

#[test]
fn different_payload_types_forward_through_resume() {
    let mut state = 0x31_0a;
    for _ in 0..4 {
        let number = random(&mut state);
        let text = format!("s{}", random(&mut state));
        Case::new(&mixed_source(&format!("do y {number} -> do s {text:?} -> ()"), ""))
            .run_main(&format!("[{text:?}, \"{number}\"]"));
    }
}
