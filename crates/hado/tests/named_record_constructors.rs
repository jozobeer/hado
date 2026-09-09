use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-named-record-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, file: &str, source: &str) {
        std::fs::write(self.0.join(file), source).unwrap();
    }
    fn command(&self, command: &str, entry: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado")).arg(command).arg(self.0.join(entry)).output().unwrap()
    }
    fn run(&self, expected: &str) {
        for command in ["check", "dump-mono", "build"] {
            let output = self.command(command, "main.hd");
            assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
            assert!(output.stderr.is_empty());
        }
        let output = Command::new(self.0.join("main")).output().unwrap();
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
        let prefix = format!("{}:{}:{}: error[{code}]: ", PathBuf::from(file).strip_prefix(PathBuf::from(entry).parent().unwrap()).unwrap().display(), before.matches('\n').count() + 1, before.rsplit('\n').next().unwrap().chars().count() + 1);
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
const BASIC: &str = "type Data = Payload { left: Int; right: Int }";
fn source_case(source: &str) -> Case {
    let case = Case::new();
    case.write("main.hd", source);
    case
}
fn rejection(source: &str, code: &str, anchor: &str) {
    let case = Case::new();
    for padding in 0..5 {
        let (source, offset) = shifted(source, anchor, padding);
        case.write("main.hd", &source);
        for command in ["check", "build", "dump-mono"] { case.reject(command, "main.hd", "main.hd", code, offset); }
    }
}
#[test]
fn generated_fields_preserve_names_when_construction_order_changes() {
    let mut state = 0x120035;
    for sample in 0..8 {
        let left = random(&mut state, 100);
        let right = 100 + random(&mut state, 100);
        let fields = if sample % 2 == 0 { format!("right = {right}; left = {left}") } else { format!("left = {left}; right = {right}") };
        for pipe in ["", "| "] {
            let source = format!("type Data = {pipe}Payload {{ left: Int; right: Int }}\ndef main = let value = Payload {{ {fields} }} -> value.left * 1000 + value.right");
            source_case(&source).run(&(left * 1000 + right).to_string());
        }
    }
}
#[test]
fn generated_values_print_in_declaration_order_and_can_be_read_back() {
    let mut state = 0x510012;
    for _ in 0..5 {
        let first = random(&mut state, 100);
        let second = random(&mut state, 100);
        let expected = format!("Payload {{ left = {first}; right = {second} }}");
        source_case(&format!("{BASIC}\ndef main = Payload {{ right = {second}; left = {first} }}")).run(&expected);
        source_case(&format!("{BASIC}\ndef main = {expected}")).run(&expected);
    }
}
#[test]
fn generic_named_fields_support_different_type_arguments() {
    let mut state = 0x100012;
    for _ in 0..5 {
        let value = random(&mut state, 100);
        let source = format!("type (A, B) Pair = PairValue {{ first: A; second: B }}\ndef main = let one = PairValue {{ second = \"abc\"; first = {value} }} -> let two = PairValue {{ first = true; second = {value} }} -> one.first + String.length one.second + (if two.first => two.second | 0)");
        source_case(&source).run(&(value * 2 + 3).to_string());
    }
}
#[test]
fn identical_field_sets_do_not_make_constructor_selection_ambiguous() {
    source_case("type FirstType = First { value: Int }\ntype SecondType = Second { value: Int }\ndef main = let a = First { value = 7 } -> let b = Second { value = 11 } -> a.value * 100 + b.value").run("711");
}
#[test]
fn named_and_positional_payloads_can_share_an_adt() {
    source_case("type Choice = | Payload { left: Int; right: Int } | Other(Int) | Empty\ndef read | Payload { left; right = r } => left + r | Other(n) => n * 2 | Empty => 0\ndef main = read (Payload { right = 3; left = 7 }) + read (Other(5)) + read Empty").run("20");
}
#[test]
fn named_patterns_support_shorthand_renaming_omission_and_nesting() {
    let source = format!("{BASIC}\ntype Envelope = Wrap {{ inner: Data; extra: Int }}\ndef main = let Wrap {{ inner = Payload {{ left; right = r }}; extra }} = Wrap {{ extra = 5; inner = Payload {{ right = 3; left = 7 }} }} -> let Payload {{ left = copy }} = Payload {{ left = 11; right = 99 }} -> left + r + extra + copy");
    source_case(&source).run("26");
}
#[test]
fn missing_variant_in_a_branch_is_non_exhaustive() {
    rejection("type Choice = | Payload { left: Int } | Empty\ndef read | Payload { left } => left + 1\ndef main = read (Payload { left = 7 })", "E402", "def read");
}
#[test]
fn let_pattern_must_cover_every_constructor() {
    rejection("type Choice = | Payload { left: Int } | Empty\ndef main = let Payload { left } = Payload { left = 7 } -> left + 1", "E402", "Payload { left }");
}
#[test]
fn unknown_fields_in_construction_and_patterns_are_rejected() {
    rejection(&format!("{BASIC}\ndef main = Payload {{ left = 1; right = 2; absent = 3 }}"), "E204", "absent");
    rejection(&format!("{BASIC}\ndef main = let Payload {{ absent = n }} = Payload {{ left = 1; right = 2 }} -> n + 1"), "E204", "absent");
}
#[test]
fn duplicate_fields_in_declarations_construction_and_patterns_point_to_second_name() {
    rejection("type Data = Payload { left: Int; left: Int }\ndef main = Payload { left = 1 }", "E205", "left: Int }");
    rejection(&format!("{BASIC}\ndef main = Payload {{ left = 1; right = 2; left = 3 }}"), "E205", "left = 3");
    rejection(&format!("{BASIC}\ndef main = let Payload {{ left; left = other }} = Payload {{ left = 1; right = 2 }} -> left + other"), "E205", "left = other");
}
#[test]
fn missing_fields_and_bare_constructor_references_are_arity_errors() {
    rejection(&format!("{BASIC}\ndef main = Payload {{ left = 1 }}"), "E301", "Payload { left = 1 }");
    rejection(&format!("def main = Payload\n{BASIC}"), "E301", "Payload");
}
#[test]
fn field_type_mismatch_points_to_the_value() {
    rejection(&format!("{BASIC}\ndef main = Payload {{ right = true; left = 1 }}"), "E305", "true");
}
#[test]
fn positional_and_named_constructor_forms_cannot_be_interchanged() {
    rejection(&format!("{BASIC}\ndef main = Payload(1, 2)"), "E301", "Payload(1");
    rejection("def main = Some { value = 1 }", "E301", "Some");
    rejection(&format!("{BASIC}\ndef main = let Payload(left, right) = Payload {{ left = 1; right = 2 }} -> left + right"), "E301", "Payload(left");
}
#[test]
fn named_field_forms_require_nonempty_lists_and_no_trailing_separator() {
    for source in [
        "type Data = Payload {}\ndef main = 1",
        "type Data = Payload { left: Int; }\ndef main = 1",
        "type Data = Payload { left: Int }\ndef main = Payload {}",
        "type Data = Payload { left: Int }\ndef main = Payload { left = 1; }",
        "type Data = Payload { left: Int }\ndef main = let Payload {} = Payload { left = 1 } -> 2",
        "type Data = Payload { left: Int }\ndef main = let Payload { left; } = Payload { left = 1 } -> left + 1",
    ] {
        let offset = source.find("{}").map(|offset| offset + 1).or_else(|| source.find("; }").map(|offset| offset + 2)).unwrap();
        let case = source_case(source);
        for command in ["check", "build", "dump-mono"] { case.reject(command, "main.hd", "main.hd", "E100", offset); }
    }
}
#[test]
fn multiple_constructor_types_cannot_be_accessed_as_records() {
    rejection("def main = (choose 7).left\ntype Choice = | Payload { left: Int } | Empty\ndef choose n => Payload { left = n }", "E204", "left");
}
fn implementation_source(main: &str) -> String {
    format!("def main = {main}\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\nimplement S as M\ntype T = Payload {{ left: Int; right: Int }}\ndef make x => Payload {{ right = 3; left = x }}\ndef read value => value.left + value.right")
}
#[test]
fn constructor_backed_abstract_realizations_work_through_public_members() {
    source_case(&implementation_source("M.read (M.make 7)")).run("10");
}
#[test]
fn abstract_realizations_stay_opaque_outside_the_implementation() {
    rejection(&implementation_source("let value = M.make 7 -> value.left"), "E601", ".left");
}
#[test]
fn implementation_constructors_are_not_exported() {
    rejection(&implementation_source("Payload { left = 7; right = 3 }"), "E204", "Payload {");
}
#[test]
fn old_positional_constructor_lists_still_require_a_leading_pipe() {
    rejection("type Data = Payload(Int)\ndef main = Payload(7)", "E108", "Payload(Int)");
}
#[test]
fn functor_applications_keep_named_field_abstract_types_distinct() {
    let common = "module Id\ndef value(x: Int): Int\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\nimplement Id as I\ndef value x => x + 3\nimplement Id as J\ndef value x => x + 11\nimplement S as Make(Param: Id)\ntype T = Payload { value: Int }\ndef make x => Payload { value = Param.value x }\ndef read x => x.value\nimplement S as A = Make(I)\nimplement S as B = Make(J)";
    source_case(&format!("def main = A.read (A.make 7) * 100 + B.read (B.make 7)\n{common}")).run("1018");
    rejection(&format!("def main = A.read (B.make 7)\n{common}"), "E305", "B.make 7");
}
#[test]
fn named_constructor_printing_preserves_strings_and_nested_payloads() {
    source_case("type A Box = Payload { name: String; value: A }\ndef main = Payload { value = Some(7); name = \"hello\" }").run("Payload { name = \"hello\"; value = Some(7) }");
}

#[test]
fn bare_records_do_not_select_a_named_constructor_by_field_set() {
    rejection(&format!("{BASIC}\ndef main = {{ left = 1; right = 2 }}"), "E304", "{ left = 1");
}
#[test]
fn leading_pipe_omission_is_limited_to_one_named_constructor() {
    rejection("type Choice = Payload { left: Int } | Empty\ndef main = Payload { left = 7 }", "E108", "Payload { left: Int }");
}
fn positional_realization(main: &str) -> String {
    format!("def main = {main}\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\nimplement S as M\ntype T = | Payload(Int)\ndef make x => Payload(x)\ndef read | Payload(n) => n + 3")
}
#[test]
fn positional_constructor_can_directly_realize_an_abstract_type() {
    source_case(&positional_realization("M.read (M.make 7)")).run("10");
}
#[test]
fn positional_constructor_realization_stays_opaque_and_private() {
    rejection(&positional_realization("let value = M.make 7 -> value.left"), "E601", ".left");
    rejection(&positional_realization("Payload(7)"), "E204", "Payload(7)");
}
#[test]
fn constructorless_record_still_conflicts_with_same_named_abstract_type() {
    rejection("def main = M.read (M.make 7)\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\nimplement S as M\ntype T = { value: Int }\ndef make x => { value = x }\ndef read x => x.value", "E205", "T = {");
}
#[test]
fn payloadless_and_positional_constructors_reject_named_payloads() {
    rejection("type Data = | Guest\ndef main = Guest { value = 1 }", "E301", "Guest { value");
    rejection("def main = let Some { value = n } = Some(7) -> n", "E301", "Some { value");
}
#[test]
fn payload_constructor_cannot_be_a_bare_pattern() {
    rejection(&format!("{BASIC}\ndef main = let Payload = Payload {{ left = 7; right = 3 }} -> 1"), "E301", "Payload = Payload");
}

#[test]
fn field_inference_uses_named_constructors_and_later_call_constraints() {
    source_case("type Data = Payload { value: Int }\ndef read x => x.value + 1\ndef main = read (Payload { value = 7 })").run("8");
    source_case("type FirstType = First { value: Int }\ntype SecondType = Second { value: String }\ndef read x => x.value + 1\ndef main = read (First { value = 7 }) + String.length (Second { value = \"abc\" }).value").run("11");
}
#[test]
fn unknown_projected_field_is_a_name_error() {
    rejection(&format!("{BASIC}\ndef main = (Payload {{ left = 1; right = 2 }}).missing"), "E204", "missing");
}
#[test]
fn payload_form_checks_follow_syntax_and_name_resolution() {
    rejection(&format!("{BASIC}\ndef main = Payload(1, 2)\ndef other = Some 1"), "E106", "Some 1");
    rejection(&format!("{BASIC}\ndef main = Payload(unknown, 2)"), "E204", "unknown");
}
#[test]
fn identical_named_field_sets_do_not_unify_distinct_nominal_types() {
    let declarations = "type FirstType = First { value: Int }\ntype SecondType = Second { value: Int }\ndef take (x: FirstType): Int => x.value + 1";
    source_case(&format!("{declarations}\ndef main = take (First {{ value = 7 }}) + (Second {{ value = 11 }}).value")).run("19");
    rejection(&format!("{declarations}\ndef main = take (Second {{ value = 11 }})"), "E305", "Second { value = 11 }");
}
#[test]
fn bare_record_pattern_does_not_destructure_a_named_constructor_value() {
    rejection(&format!("{BASIC}\ndef main = let {{ left; right }} = Payload {{ left = 7; right = 3 }} -> left + right"), "E305", "{ left; right }");
}

#[test]
fn nested_unknown_field_precedes_an_earlier_field_type_error() {
    rejection("type Inner = Inside { value: Int }\ntype Data = Payload { item: Inner; number: Int }\ndef main = Payload { number = true; item = Inside { wrong = 1 } }", "E204", "wrong");
}
#[test]
fn nested_unknown_pattern_field_precedes_an_earlier_pattern_type_error() {
    rejection("type Inner = Inside { value: Int }\ntype Data = Payload { item: Inner; number: Int }\ndef read\n| Payload { number = true; item = Inside { wrong } } => wrong\ndef main = read (Payload { number = 1; item = Inside { value = 2 } })", "E204", "wrong");
}

#[test]
fn local_constructor_names_shadow_outer_payload_forms_and_restore_them() {
    for outer_named in [false, true] {
        for inner_named in [false, true] {
            let outer_type = if outer_named { "Payload { x: Int }" } else { "| Payload(Int)" };
            let outer_value = if outer_named { "Payload { x = 1 }" } else { "Payload(1)" };
            let outer_read = if outer_named { "def root value => value.x" } else { "def root | Payload(x) => x" };
            let inner_type = if inner_named { "Payload { y: Int }" } else { "| Payload(Int)" };
            let inner_value = if inner_named { "Payload { y = x }" } else { "Payload(x)" };
            let inner_read = if inner_named { "def read value => value.y" } else { "def read | Payload(y) => y" };
            source_case(&format!("type Data = {outer_type}\n{outer_read}\ndef main = root ({outer_value}) + M.read (M.make 2) + root ({outer_value})\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\nimplement S as M\ntype T = {inner_type}\ndef make x => {inner_value}\n{inner_read}")).run("4");
        }
    }
}

#[test]
fn shadowing_a_constructor_keeps_outer_named_type_projections() {
    for parameter in ["x", "(x: Root)"] {
        source_case(&format!("type Root = Payload {{ x: Int }}\ndef main = M.read_root (Payload {{ x = 5 }}) + M.read (M.make 2)\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\ndef read_root(x: Root): Int\nimplement S as M\ntype T = Payload {{ y: Int }}\ndef make x => Payload {{ y = x }}\ndef read x => x.y\ndef read_root {parameter} => x.x")).run("7");
    }
}
#[test]
fn shadowing_a_constructor_keeps_outer_type_coverage_requirements() {
    rejection("type Root = | Payload { x: Int } | Other(Int)\ndef main = M.read_root (Other(5)) + M.read (M.make 2)\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\ndef read_root(x: Root): Int\nimplement S as M\ntype T = Payload { y: Int }\ndef make x => Payload { y = x }\ndef read x => x.y\ndef read_root\n| Other(x) => x", "E402", "def read_root\n");
}

#[test]
fn contract_constraints_resolve_shadowed_types_with_identical_field_names() {
    source_case("type Root = Payload { x: Int }\ndef main = M.read_root (Payload { x = 5 }) + M.read (M.make 2)\nmodule S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int\ndef read_root(x: Root): Int\nimplement S as M\ntype T = Payload { x: Int }\ndef make x => Payload { x = x }\ndef read x => x.x\ndef read_root x => x.x").run("7");
}
