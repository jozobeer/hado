use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

fn run_main(body: &str, expected: &[i64]) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "hado-r31-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = format!(
        "effect Y\ncapture y(value: Int): Unit\nhandler Y as Keep\ncapture y value => let rest = resume () -> [value, ...rest]\nreturn x => x\n{body}\n"
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
fn branch_result_is_bound_after_the_selected_capture() {
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
fn capture_arguments_are_evaluated_before_the_outer_operation() {
    let (a, b) = payloads();
    run_main(&format!(
        "def n u => if u == () => {b} | 0\ndef main = with Keep -> do y (n (y {a})) -> [7]"
    ), &[a, b, 7]);
}
