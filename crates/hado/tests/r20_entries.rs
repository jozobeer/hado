use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r20-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
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
fn contract(case: &Case) { case.write("calc.hdi", "module Calc\ndef apply(x: Int): Int"); }

fn entry_source(index: usize, refs: &[usize], constants: &[usize], state: &mut u64) -> (String, usize) {
    let seed = 1 + random(state, 99);
    let bias = index * 100_000 + random(state, 99);
    let calls: Vec<_> = refs.iter().map(|j| format!("Calc{j}.apply seed")).collect();
    let expected = bias + refs.iter().map(|j| seed + constants[*j]).sum::<usize>();
    (format!("def seed = {seed}\ndef local x => x + {bias}\ndef main = local ({})", calls.join(" + ")), expected)
}
fn generated_project(state: &mut u64, count: usize) {
    let case = Case::new();
    let constants: Vec<_> = (0..count + 1 + random(state, 3)).map(|_| random(state, 100)).collect();
    contract(&case);
    for (j, value) in constants.iter().enumerate().rev() {
        case.write(&format!("impl{j}.hd"), &format!("implement Calc as Calc{j}\ndef apply x => x + {value}"));
    }
    let mut owners: Vec<_> = (0..count).collect();
    for i in (1..count).rev() { let j = random(state, i + 1); owners.swap(i, j); }
    let mut entries = Vec::new();
    for i in 0..count {
        // Implementation zero is shared; each other implementation has exactly one owner.
        let refs: Vec<_> = (0..constants.len()).filter(|j| *j == 0 || owners[(*j - 1) % count] == i).collect();
        let (source, expected) = entry_source(i, &refs, &constants, state);
        let file = format!("{}entry{i}.hd", if i % 2 == 0 { "z_" } else { "a_" });
        case.write(&file, &source);
        entries.push((file, expected));
    }
    for (file, expected) in entries { case.run(&file, expected); }
}
#[test]
fn generated_entries_select_their_own_main_and_keep_other_entry_implementations_used() {
    let mut state = 0x20_0123;
    for _ in 0..4 { let count = 2 + random(&mut state, 3); generated_project(&mut state, count); }
}
#[test]
fn another_files_outer_value_is_not_visible_even_when_that_file_is_selected() {
    for padding in 0..5 {
        let case = Case::new();
        let (source, offset) = shifted("def main = foreign", "foreign", padding);
        case.write("a.hd", &source);
        case.write("b.hd", "def foreign = 37\ndef main = foreign");
        for entry in ["a.hd", "b.hd"] { case.reject("check", entry, "a.hd", "E204", offset); }
    }
}
#[test]
fn sections_only_see_outer_values_from_their_own_file() {
    for padding in 0..5 {
        let case = Case::new();
        contract(&case);
        case.write("entry.hd", "def seed = 7\ndef main = M.apply seed");
        let (source, offset) = shifted("implement Calc as M\ndef apply x => x + seed", "seed", padding);
        case.write("impl.hd", &source);
        case.reject("check", "entry.hd", "impl.hd", "E204", offset);
    }
}
#[test]
fn root_file_without_main_can_use_its_outer_helper_in_its_section() {
    let mut state = 0x20513;
    for _ in 0..4 {
        let case = Case::new();
        let value = random(&mut state, 100);
        let added = random(&mut state, 100);
        contract(&case);
        case.write("entry.hd", &format!("def main = M.apply {value}"));
        case.write("support.hd", &format!("def helper x => x + {added}\nimplement Calc as M\ndef apply x => helper x"));
        case.run("entry.hd", value + added);
    }
}
#[test]
fn unused_outer_helper_is_not_rescued_by_another_files_same_name() {
    for padding in 0..5 {
        let case = Case::new();
        contract(&case);
        case.write("entry.hd", "def helper x => x + 4\ndef main = M.apply (helper 3)");
        let (source, offset) = shifted("def helper x => x + 3\nimplement Calc as M\ndef apply x => x", "def helper", padding);
        case.write("support.hd", &source);
        case.reject("check", "entry.hd", "support.hd", "E513", offset);
    }
}
#[test]
fn nonselected_main_is_exempt_but_its_unused_helpers_are_not() {
    for padding in 0..5 {
        let case = Case::new();
        case.write("entry.hd", "def main = 1");
        let (source, offset) = shifted("def unused x => x\ndef main = 2", "def unused", padding);
        case.write("other.hd", &source);
        case.reject("check", "entry.hd", "other.hd", "E513", offset);
    }
}
#[test]
fn nonselected_main_body_is_still_checked() {
    for padding in 0..5 {
        let case = Case::new();
        case.write("entry.hd", "def main = 1");
        let (source, offset) = shifted("def main = missing", "missing", padding);
        case.write("other.hd", &source);
        case.reject("check", "entry.hd", "other.hd", "E204", offset);
    }
}
#[test]
fn missing_selected_main_reports_selected_file_one_one_for_all_commands() {
    for padding in 0..5 {
        let case = Case::new();
        case.write("a.hd", "def main = 1");
        case.write("z.hd", "def main = 2");
        case.write("empty.hd", &format!("// no entry\n{}", "\n".repeat(padding)));
        for command in ["check", "build", "dump-mono"] { case.reject(command, "empty.hd", "empty.hd", "E206", 0); }
    }
}
#[test]
fn used_support_file_cannot_fall_back_to_sibling_main() {
    let case = Case::new();
    contract(&case);
    case.write("entry.hd", "def main = M.apply 7");
    case.write("support.hd", "def helper x => x + 3\nimplement Calc as M\ndef apply x => helper x");
    for command in ["check", "build", "dump-mono"] { case.reject(command, "support.hd", "support.hd", "E206", 0); }
}
#[test]
fn file_isolation_preserves_parameter_let_lambda_and_pattern_shadowing() {
    let mut state = 0x20_55;
    for _ in 0..4 {
        let case = Case::new();
        let mut expected = Vec::new();
        for i in 0..2 {
            let outer = 1 + random(&mut state, 50);
            let inner = 1 + random(&mut state, 50);
            let source = format!("def seed = {outer}\ndef local seed => seed + 1\ndef main = seed + local {inner} + (let seed = {inner} -> seed + 3) + (fn(seed => seed + 2)) {inner} + (if Some({inner}) | Some(seed) => seed + 4 | None => 0)");
            let control = Case::new();
            control.write("single.hd", &source);
            control.run("single.hd", outer + 4 * inner + 10);
            case.write(&format!("entry{i}.hd"), &source);
            expected.push(outer + 4 * inner + 10);
        }
        for (i, value) in expected.into_iter().enumerate() { case.run(&format!("entry{i}.hd"), value); }
    }
}
#[test]
fn same_file_value_duplicates_still_reject_at_second_name() {
    for padding in 0..5 {
        let case = Case::new();
        let (second, offset) = shifted("def value = 2", "value", padding);
        let first = "def value = 1\n";
        case.write("entry.hd", &format!("{first}{second}\ndef main = value"));
        case.reject("check", "entry.hd", "entry.hd", "E205", first.len() + offset);
    }
}
#[test]
fn shared_type_duplicates_follow_selected_then_byte_path_order() {
    for selected in ["z.hd", "a.hd"] {
        let case = Case::new();
        case.write("z.hd", "def main = 1\ntype Data = | First");
        case.write("a.hd", "def main = 2\ntype Data = | Second");
        let second = if selected == "z.hd" { "a.hd" } else { "z.hd" };
        let source = std::fs::read_to_string(case.0.join(second)).unwrap();
        case.reject("check", selected, second, "E205", source.find("Data").unwrap());
    }
}
#[test]
fn same_stage_errors_follow_selected_then_byte_path_order() {
    for padding in 0..5 {
        let case = Case::new();
        let (a, a_offset) = shifted("def main = absentA", "absentA", padding);
        let (z, z_offset) = shifted("def main = absentZ", "absentZ", 4 - padding);
        case.write("a.hd", &a);
        case.write("z.hd", &z);
        case.write("middle.hd", "def main = 3");
        case.reject("check", "a.hd", "a.hd", "E204", a_offset);
        case.reject("check", "z.hd", "z.hd", "E204", z_offset);
        case.reject("check", "middle.hd", "a.hd", "E204", a_offset);
    }
}
#[test]
fn name_resolution_precedes_unused_outer_value_checks() {
    let case = Case::new();
    case.write("entry.hd", "def main = missing");
    case.write("support.hd", "def unused = 1");
    case.reject("check", "entry.hd", "entry.hd", "E204", "def main = ".len());
}
#[test]
fn subdirectory_outer_def_is_rejected_at_its_original_position() {
    let case = Case::new();
    case.write("entry.hd", "def main = 1");
    std::fs::create_dir(case.0.join("nested")).unwrap();
    let (source, offset) = shifted("def helper x => x", "def", 3);
    case.write("nested/helper.hd", &source);
    case.reject("check", "entry.hd", "nested/helper.hd", "E100", offset);
}

#[test]
fn single_entry_generator_control() {
    generated_project(&mut 0x20_0123, 1);
}
