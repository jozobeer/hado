use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r21-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, file: &str, source: &str) {
        std::fs::write(self.0.join(file), source).unwrap();
    }
    fn command(&self, command: &str, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado")).arg(command).arg(self.0.join(entry)).output().unwrap()
    }
    fn run(&self, entry: &str, expected: usize) {
        let output = Command::new(env!("CARGO_BIN_EXE_hado")).args(["check", "--print-type"]).arg(self.0.join(entry)).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, b"Int\n");
        assert!(output.stderr.is_empty());
        let output = self.command("build", entry);
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stderr.is_empty());
        let output = Command::new(self.0.join(entry).with_extension("")).output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
    fn reject(&self, command: &str, entry: &str, file: &str, code: &str, offset: usize) {
        let source = std::fs::read_to_string(self.0.join(file)).unwrap();
        let before = &source[..offset];
        let output = self.command(command, entry);
        assert_eq!(output.status.code(), Some(1), "{source}\n{}", String::from_utf8_lossy(&output.stderr));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        let prefix = format!("{}:{}:{}: error[{code}]: ", self.0.join(file).display(), before.matches('\n').count() + 1, before.rsplit('\n').next().unwrap().chars().count() + 1);
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
const CONTRACTS: &str = "module Id\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\nmodule Box\ntype T\ndef make(x: Int): T\ndef read(x: T): Int";
fn base() -> Case {
    let case = Case::new();
    case.write("contracts.hdi", CONTRACTS);
    case.write("ids.hd", "implement Id as I\ntype T = Int\ndef make x => x + 3\ndef read x => x");
    case.write("functor.hd", &functor(1));
    case.write("apply.hd", "implement Box as A = Make(I)");
    case.write("main.hd", "def main = A.read (A.make 7)");
    case
}
fn functor(arity: usize) -> String {
    let params: Vec<_> = (0..arity).map(|i| format!("P{i}: Id")).collect();
    let fields: Vec<_> = (0..arity).map(|i| format!("slot{i}: P{i}.T")).collect();
    let values: Vec<_> = (0..arity).map(|i| format!("slot{i} = P{i}.make x")).collect();
    let reads: Vec<_> = (0..arity).map(|i| format!("(P{i}.read value.slot{i}) * {}", i + 1)).collect();
    format!("implement Box as Make({})\ntype T = {{ {} }}\ndef make x => {{ {} }}\ndef read value => {}", params.join(", "), fields.join("; "), values.join("; "), reads.join(" + "))
}
fn reject_shifted(case: &Case, file: &str, source: &str, code: &str, anchor: &str) {
    for padding in 0..5 {
        let (source, offset) = shifted(source, anchor, padding);
        case.write(file, &source);
        for command in ["check", "build", "dump-mono"] { case.reject(command, "main.hd", file, code, offset); }
    }
}
fn generated_application(case: &Case, app: usize, arity: usize, state: &mut u64) -> (String, usize) {
    let value = random(state, 100);
    let mut expected = 0;
    let mut args = Vec::new();
    for parameter in 0..arity {
        let added = random(state, 100);
        let name = format!("I{app}_{parameter}");
        case.write(&format!("id{app}_{parameter}.hd"), &format!("implement Id as {name}\ntype T = Int\ndef make x => x + {added}\ndef read x => x"));
        args.push(name);
        expected += (value + added) * (parameter + 1);
    }
    case.write(&format!("apply{app}.hd"), &format!("implement Box as A{app} = Make({})", args.join(", ")));
    (format!("A{app}.read (A{app}.make {value})"), expected)
}
#[test]
fn generated_arities_and_applications_have_independent_arithmetic_results() {
    let mut state = 0x21_56;
    for sample in 0..6 {
        let case = Case::new();
        let arity = if sample < 3 { sample + 1 } else { 1 + random(&mut state, 3) };
        let applications = 2 + random(&mut state, 3);
        case.write("contracts.hdi", CONTRACTS);
        case.write("functor.hd", &functor(arity));
        let values: Vec<_> = (0..applications).map(|app| generated_application(&case, app, arity, &mut state)).collect();
        case.write("main.hd", &format!("def main = {}", values.iter().map(|(call, _)| call.as_str()).collect::<Vec<_>>().join(" + ")));
        case.run("main.hd", values.iter().map(|(_, value)| *value).sum());
    }
}
#[test]
fn minimal_application_is_accepted() { base().run("main.hd", 10); }
#[test]
fn application_types_are_distinct_even_with_identical_representations() {
    let case = base();
    case.write("ids.hd", "implement Id as I\ntype T = Int\ndef make x => x + 3\ndef read x => x\nimplement Id as J\ntype T = Int\ndef make x => x + 3\ndef read x => x");
    case.write("apply.hd", "implement Box as A = Make(I)\nimplement Box as B = Make(J)");
    reject_shifted(&case, "main.hd", "def main = A.read (B.make 7)", "E305", "B.make 7");
}
#[test]
fn both_distinct_application_types_can_be_consumed_by_their_own_members() {
    let case = base();
    case.write("ids.hd", "implement Id as I\ntype T = Int\ndef make x => x + 3\ndef read x => x\nimplement Id as J\ntype T = Int\ndef make x => x + 3\ndef read x => x");
    case.write("apply.hd", "implement Box as A = Make(I)\nimplement Box as B = Make(J)");
    case.write("main.hd", "def main = A.read (A.make 7) + B.read (B.make 8)");
    case.run("main.hd", 21);
}
#[test]
fn application_header_requires_the_functors_contract() {
    let case = base();
    case.write("contracts.hdi", &format!("{CONTRACTS}\nmodule Other\ntype T\ndef make(x: Int): T\ndef read(x: T): Int"));
    reject_shifted(&case, "apply.hd", "implement Other as A = Make(I)", "E602", "implement");
}
#[test]
fn application_argument_requires_its_position_contract() {
    let case = base();
    case.write("contracts.hdi", &format!("{CONTRACTS}\nmodule Other\ndef flag: Bool"));
    case.write("ids.hd", "implement Other as I\ndef flag = true");
    reject_shifted(&case, "apply.hd", "implement Box as A = Make(I)", "E602", "implement");
}
#[test]
fn a_functor_cannot_be_an_application_argument() {
    let case = base();
    case.write("ids.hd", "");
    reject_shifted(&case, "apply.hd", "implement Box as A = Make(Make)", "E204", "Make)");
}
#[test]
fn repeated_argument_lists_report_the_second_implement() {
    let case = base();
    case.write("main.hd", "def main = A.read (A.make 7) + B.read (B.make 8)");
    case.write("apply.hd", "implement Box as A = Make(I)");
    reject_shifted(&case, "z_duplicate.hd", "implement Box as B = Make(I)", "E604", "implement");
}
#[test]
fn reversed_argument_lists_are_distinct_and_preserve_order() {
    let case = base();
    case.write("functor.hd", &functor(2));
    case.write("ids.hd", "implement Id as I\ntype T = Int\ndef make x => x + 3\ndef read x => x\nimplement Id as J\ntype T = Int\ndef make x => x + 11\ndef read x => x");
    case.write("apply.hd", "implement Box as A = Make(I, J)\nimplement Box as B = Make(J, I)");
    case.write("main.hd", "def main = A.read (A.make 7) * 100 + B.read (B.make 7)");
    case.run("main.hd", ((7 + 3) + 2 * (7 + 11)) * 100 + (7 + 11) + 2 * (7 + 3));
}
#[test]
fn unused_functor_is_rejected_even_when_its_contract_has_an_implementation() {
    let case = base();
    case.write("apply.hd", "implement Box as A\ntype T = Int\ndef make x => x\ndef read x => x");
    case.write("main.hd", "def main = A.read (A.make 7) + I.read (I.make 1)");
    reject_shifted(&case, "functor.hd", &functor(1), "E513", "implement");
}
#[test]
fn unused_parameter_reports_its_name_under_position_perturbations() {
    let case = base();
    let source = "implement Box as Make(P0: Id)\ntype T = Int\ndef make x => x\ndef read x => x";
    reject_shifted(&case, "functor.hd", source, "E521", "P0");
}
#[test]
fn functor_members_are_not_directly_visible() {
    let case = base();
    reject_shifted(&case, "main.hd", "def main = A.read (Make.make 7)", "E204", "Make.make");
}
#[test]
fn interface_files_cannot_contain_functor_definitions() {
    let case = base();
    case.write("functor.hd", "");
    reject_shifted(&case, "functor.hdi", &functor(1), "E100", "implement");
}
#[test]
fn application_headers_cannot_have_bodies() {
    let case = base();
    reject_shifted(&case, "apply.hd", "implement Box as A = Make(I)\ndef extra = 1", "E100", "def extra");
}
#[test]
fn contract_qualified_type_counts_the_application_but_not_the_functor() {
    let case = base();
    case.write("consumer.hdi", "module Consumer\ndef consume(x: Box.T): Int");
    case.write("consumer.hd", "implement Consumer as C\ndef consume x => A.read x");
    case.write("main.hd", "def main = C.consume (A.make 7)");
    case.run("main.hd", 10);
}
#[test]
fn an_application_result_can_be_passed_to_another_functor() {
    let case = base();
    case.write("wrap.hd", "implement Box as Wrap(Source: Box)\ntype T = { wrapped: Source.T }\ndef make x => { wrapped = Source.make x }\ndef read value => Source.read value.wrapped + 5");
    case.write("outer.hd", "implement Box as B = Wrap(A)");
    case.write("main.hd", "def main = B.read (B.make 7)");
    case.run("main.hd", 15);
}
#[test]
fn functor_body_must_satisfy_its_result_contract() {
    let case = base();
    let source = functor(1).replace("(P0.read value.slot0) * 1", "P0.read value.slot0 > 0");
    reject_shifted(&case, "functor.hd", &source, "E602", "implement");
}
#[test]
fn unused_application_is_rejected() {
    let case = base();
    case.write("main.hd", "def main = I.read (I.make 7)");
    reject_shifted(&case, "apply.hd", "implement Box as A = Make(I)", "E513", "implement");
}
#[test]
fn two_application_results_make_contract_qualified_type_ambiguous() {
    let case = base();
    case.write("ids.hd", "implement Id as I\ntype T = Int\ndef make x => x + 3\ndef read x => x\nimplement Id as J\ntype T = Int\ndef make x => x + 11\ndef read x => x");
    case.write("apply.hd", "implement Box as A = Make(I)\nimplement Box as B = Make(J)");
    case.write("consumer.hd", "implement Consumer as C\ndef consume _ => 1");
    case.write("main.hd", "def main = A.read (A.make 7) + B.read (B.make 8)");
    reject_shifted(&case, "consumer.hdi", "module Consumer\ndef consume(x: Box.T): Int", "E603", "Box.T");
}

#[test]
fn parameter_types_remain_opaque_even_when_actual_representations_support_the_operation() {
    for (representation, make, body, code) in [
        ("type T = Int", "x", "Arg.make x + 1", "E305"),
        ("type Payload = { number: Int }\ntype T = Payload", "{ number = x }", "(Arg.make x).number", "E601"),
    ] {
        let case = Case::new();
        case.write("contracts.hdi", "module Input\ntype T\ndef make(x: Int): T\nmodule Output\ndef run(x: Int): Int");
        case.write("argument.hd", &format!("implement Input as Concrete\n{representation}\ndef make x => {make}"));
        let source = format!("implement Output as Make(Arg: Input)\ndef run x => {body}");
        case.write("functor.hd", &source);
        case.write("apply.hd", "implement Output as Applied = Make(Concrete)");
        case.write("main.hd", "def main = Applied.run 7");
        for command in ["check", "build", "dump-mono"] {
            let anchor = if code == "E601" { source.find(".number") } else { source.find("Arg.make") }.unwrap();
            case.reject(command, "main.hd", "functor.hd", code, anchor);
        }
    }
}
