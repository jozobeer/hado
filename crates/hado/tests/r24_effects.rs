use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r24-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
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

fn letters(state: &mut u64, count: usize) -> Vec<String> {
    let mut names = Vec::new();
    while names.len() < count {
        let name = format!("E{}", char::from(b'a' + random(state, 26) as u8));
        if !names.contains(&name) { names.push(name); }
    }
    names
}

#[test]
fn print_type_orders_effect_rows_and_writes_them_back() {
    let mut state = 0x24_01;
    for _ in 0..8 {
        let mut names = letters(&mut state, 3);
        let mut written = names.clone();
        for index in (1..written.len()).rev() {
            written.swap(index, random(&mut state, index + 1));
        }
        names.sort();
        let case = Case::new();
        let mut source = String::new();
        for name in &names {
            source.push_str(&format!("effect {name}\nreply {}op: Unit\n", name.to_lowercase()));
        }
        source.push_str(&format!("def f (x: Int): Int {{{}}} => x\ndef main = f 1\n", written.join(", ")));
        case.write("main.hd", &source);
        let printed = case.print_type("main.hd");
        let expected = format!("Int {{{}}}\n", names.join(", "));
        assert_eq!(printed, expected, "{source}");
        let mut restored = String::new();
        for name in &names {
            restored.push_str(&format!("effect {name}\nreply {}op: Unit\n", name.to_lowercase()));
        }
        restored.push_str(&format!("def g (x: Int): {} => x\ndef main = g 1\n", printed.trim()));
        case.write("main.hd", &restored);
        assert_eq!(case.print_type("main.hd"), expected, "{restored}");
    }
}

#[test]
fn omitted_pure_and_written_rows_are_distinct() {
    let case = Case::new();
    case.write("main.hd", "effect Ask\nreply ask: String\ndef f x => if true => ask | x\ndef main = f \"a\"\n");
    assert_eq!(case.print_type("main.hd"), "String {Ask}\n");
    case.write("main.hd", "effect Ask\nreply ask: String\ndef f (x: String): String Pure => ask\ndef main = 1\n");
    let output = case.command("check", "main.hd");
    assert_eq!(output.status.code(), Some(1));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("error[E305]"), "{diagnostic}");
    case.write("main.hd", "effect Ask\nreply ask: String\neffect Io\nreply tick: String\ndef f (x: String): String {Ask} => tick\ndef main = 1\n");
    let output = case.command("check", "main.hd");
    assert_eq!(output.status.code(), Some(1));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("error[E305]"), "{diagnostic}");
}

#[test]
fn list_map_propagates_argument_effects() {
    let case = Case::new();
    case.write("main.hd", "effect Ask\nreply ask: Bool\ndef fetch (b: Bool): Bool {Ask} => b\ndef main = List.length (List.map [true] fetch)\n");
    assert_eq!(case.print_type("main.hd"), "Int {Ask}\n");
}

#[test]
fn effect_diagnostics_follow_source_positions() {
    let case = Case::new();
    perturb_rejection(&case, "main.hd", "effect Ask\nreply ask: Bool\ndef fetch (b: Bool): Bool {Ask} => b\ndef main = if fetch true => 1 | 0\n", "E308", "fetch true");
    perturb_rejection(&case, "main.hd", "effect Ask\nreply ask: String\ndef f (x: Int): Int {Ask, Ask} => x\ndef main = f 1\n", "E522", "Ask} =>");
    perturb_rejection(&case, "main.hd", "effect Ask\nreply ask: String\nreply extra: Int\nhandler Ask as H\nreply ask => \"x\"\ndef main = 1\n", "E605", "handler");
}

#[test]
fn operations_and_handlers_stay_in_their_directory() {
    let case = Case::new();
    case.write("fx/ask.hd", "effect Ask\nreply ask: String\n");
    case.write("main.hd", "def main = ask\n");
    case.reject("check", "main.hd", "main.hd", "E204", "def main = ".len());
    let case = Case::new();
    case.write("fx/ask.hd", "effect Ask\nreply ask: String\n");
    case.write("other/h.hd", "handler Ask as H\nreply ask => \"x\"\n");
    case.write("main.hd", "def main = 1\n");
    case.reject("check", "main.hd", "other/h.hd", "E204", "handler ".len());
}

#[test]
fn value_definitions_keep_evaluation_effects() {
    let case = Case::new();
    case.write("main.hd", "effect Ask\nreply ask: String\ndef cached = ask\ndef main = cached\n");
    assert_eq!(case.print_type("main.hd"), "String {Ask}\n");
}

#[test]
fn instantiated_condition_effects_are_e308() {
    let case = Case::new();
    case.write("main.hd", "effect Ask\nreply ask: Bool\ndef peek (b: Bool): Bool {Ask} => b\ndef choose f => if f true => 1 | 0\ndef main = choose peek\n");
    let output = case.command("check", "main.hd");
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("error[E308]"), "{diagnostic}");
}

#[test]
fn higher_order_union_keeps_argument_row_open() {
    let case = Case::new();
    case.write("main.hd", "effect Ask\nreply ask: Bool\neffect Io\nreply tick: Unit\ndef tickit (x: Int): Int {Io} => x\ndef apply2 f x => tickit (f x)\ndef peek (b: Int): Int {Ask} => b\ndef main = apply2 peek 1\n");
    assert_eq!(case.print_type("main.hd"), "Int {Ask, Io}\n");
}

#[test]
fn operation_labels_use_unified_arguments() {
    let case = Case::new();
    case.write("main.hd", "effect A Yield\ncapture yield(value: A): Unit\ndef emit x => yield x\ndef main = emit 1\n");
    let printed = case.print_type("main.hd");
    assert_eq!(printed, "Unit {Int Yield}\n");
    case.write("main.hd", "effect A Yield\ncapture yield(value: A): Unit\ndef emit (x: Int): Unit {Int Yield} => yield x\ndef main = emit 1\n");
    assert_eq!(case.print_type("main.hd"), "Unit {Int Yield}\n");
}

#[test]
fn handler_for_a_root_effect_in_another_directory_is_e204() {
    let case = Case::new();
    case.write("main.hd", "effect Ask\nreply ask: String\ndef caller (x: Int): String {Ask} => ask\ndef main =\n  with sub::Fixed ->\n  caller 1\n");
    case.write("sub/fixed.hd", "handler Ask as Fixed\nreply ask => \"there\"\nreturn value => value\n");
    case.reject("check", "main.hd", "sub/fixed.hd", "E204", "handler ".len());
}

#[test]
fn unannotated_union_keeps_argument_and_body_rows() {
    let case = Case::new();
    case.write("main.hd", "effect Ask\nreply ask: Bool\neffect Io\nreply tick: Unit\ndef tickit (x: Int): Int {Io} => x\ndef apply2 f x => tickit (f x)\ndef peek (b: Int): Int {Ask} => b\ndef main = apply2 peek 1\n");
    assert_eq!(case.print_type("main.hd"), "Int {Ask, Io}\n");
}

#[test]
fn annotated_row_is_an_upper_bound_for_argument_rows() {
    let case = Case::new();
    case.write("main.hd", "effect Io\nreply tick: Unit\ndef tickit (x: Int): Int {Io} => x\ndef apply2 f x: Int {Io} => tickit (f x)\ndef main = apply2 tickit 1\n");
    assert_eq!(case.print_type("main.hd"), "Int {Io}\n");
}

#[test]
fn annotated_row_rejects_argument_effects_outside_the_bound() {
    let case = Case::new();
    case.write("main.hd", "effect Io\nreply tick: Unit\neffect Ask\nreply ask: Bool\ndef tickit (x: Int): Int {Io} => x\ndef apply2 f x: Int {Io} => tickit (f x)\ndef peek (n: Int): Int {Ask} => n\ndef main = apply2 peek 1\n");
    let output = case.command("check", "main.hd");
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("error[E305]"), "{diagnostic}");
}

#[test]
fn argument_rows_are_generalized_per_call() {
    let case = Case::new();
    case.write("main.hd", "effect Io\nreply tick: Unit\neffect Ask\nreply ask: Bool\ndef tickit (x: Int): Int {Io} => x\ndef g f x: Int {Ask, Io} => tickit (f x)\ndef peek (n: Int): Int {Ask} => n\ndef main = g peek 1 + g tickit 1\n");
    assert_eq!(case.print_type("main.hd"), "Int {Ask, Io}\n");
}

#[test]
fn pure_signature_rejects_effectful_value_members() {
    let case = Case::new();
    case.write("store.hdi", "module Store\ndef value: Int Pure\n");
    case.write("impl.hd", "effect Ask\nreply ask: Int\nimplement Store as Real\ndef value = ask\n");
    case.write("main.hd", "def main = Real.value\n");
    let output = case.command("check", "main.hd");
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("error[E602]") || diagnostic.contains("error[E305]"), "{diagnostic}");
}

#[test]
fn matching_value_member_rows_are_kept() {
    let case = Case::new();
    case.write("store.hdi", "module Store\ndef value: Int {Ask}\n");
    case.write("impl.hd", "effect Ask\nreply ask: Int\nimplement Store as Real\ndef value = ask\n");
    case.write("main.hd", "def main = Real.value\n");
    assert_eq!(case.print_type("main.hd"), "Int {Ask}\n");
}

#[test]
fn unannotated_argument_accepts_matching_effect_arguments() {
    let case = Case::new();
    case.write("main.hd", "effect Io\nreply tick: Unit\neffect A Yield\ncapture yield(value: A): Unit\ndef tickit (x: Int): Int {Io} => x\ndef g f x: Int {Io, Int Yield} => tickit (f x)\ndef emit (n: Int): Int {Int Yield} => n\ndef main = g emit 1\n");
    assert_eq!(case.print_type("main.hd"), "Int {Int Yield, Io}\n");
}

#[test]
fn unannotated_argument_rejects_mismatched_effect_arguments() {
    let case = Case::new();
    case.write("main.hd", "effect Io\nreply tick: Unit\neffect A Yield\ncapture yield(value: A): Unit\ndef tickit (x: Int): Int {Io} => x\ndef g f x: Int {Io, Int Yield} => tickit (f x)\ndef semit (n: Int): Int {String Yield} => n\ndef main = g semit 1\n");
    let output = case.command("check", "main.hd");
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("error[E305]"), "{diagnostic}");
}

#[test]
fn annotated_argument_accepts_matching_effect_arguments() {
    let case = Case::new();
    case.write("main.hd", "effect A Yield\ncapture yield(value: A): Unit\ndef emit (n: Int): Int {Int Yield} => n\ndef g (f: Fn(Int => Int {Int Yield}), x: Int): Int {Int Yield} => f x\ndef main = g emit 1\n");
    assert_eq!(case.print_type("main.hd"), "Int {Int Yield}\n");
}

#[test]
fn annotated_argument_rejects_mismatched_effect_arguments() {
    let case = Case::new();
    case.write("main.hd", "effect A Yield\ncapture yield(value: A): Unit\ndef semit (n: Int): Int {String Yield} => n\ndef g (f: Fn(Int => Int {Int Yield}), x: Int): Int {Int Yield} => f x\ndef main = g semit 1\n");
    let output = case.command("check", "main.hd");
    assert_eq!(output.status.code(), Some(1), "{}", String::from_utf8_lossy(&output.stderr));
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(diagnostic.contains("error[E305]"), "{diagnostic}");
}
