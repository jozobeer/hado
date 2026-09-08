use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r22-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
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
fn path(directory: &str, name: &str) -> String { format!("{}::{name}", directory.replace('/', "::")) }
fn perturb_rejection(case: &Case, file: &str, source: &str, code: &str, anchor: &str) {
    for padding in 0..5 {
        let (source, offset) = shifted(source, anchor, padding);
        case.write(file, &source);
        for command in ["check", "build", "dump-mono"] { case.reject(command, "main.hd", file, code, offset); }
    }
}
fn generated_member(case: &Case, index: usize, directories: &[String], state: &mut u64) -> (String, usize) {
    let selected: Vec<_> = (0..3).map(|_| &directories[random(state, directories.len())]).collect();
    let names: Vec<_> = [format!("Data{index}"), format!("S{index}"), format!("M{index}")].iter().zip(&selected).map(|(name, dir)| path(dir, name)).collect();
    let value = random(state, 100);
    let added = random(state, 100);
    let constructor = path(selected[0], &format!("Value{index}"));
    case.write(&format!("{}/type{index}.hd", selected[0]), &format!("type Data{index} = | Value{index}(Int)"));
    case.write(&format!("{}/contract{index}.hdi", selected[1]), &format!("module S{index}\ndef run(value: {}): Int", names[0]));
    case.write(&format!("{}/impl{index}.hd", selected[2]), &format!("implement {} as M{index}\ndef run | {constructor}(n) => n + {added}", names[1]));
    (format!("{}.run ({constructor}({value}))", names[2]), value + added)
}
#[test]
fn generated_directory_trees_distribute_types_contracts_and_implementations() {
    let mut state = 0x22_55;
    for sample in 0..6 {
        let case = Case::new();
        let mut directories = vec![format!("branch{sample}"), format!("branch{sample}/leaf"), format!("peer{sample}")];
        for i in 0..random(&mut state, 3) {
            let parent = random(&mut state, directories.len());
            directories.push(format!("{}/extra{i}", directories[parent]));
        }
        for directory in &directories { std::fs::create_dir_all(case.0.join(directory)).unwrap(); }
        let values: Vec<_> = (0..2 + random(&mut state, 4)).map(|i| generated_member(&case, i, &directories, &mut state)).collect();
        case.write("main.hd", &format!("def main = {}", values.iter().map(|(source, _)| source.as_str()).collect::<Vec<_>>().join(" + ")));
        case.run("main.hd", values.iter().map(|(_, value)| value).sum());
    }
}
fn reference_case(contract_dir: &str) -> Case {
    let case = Case::new();
    case.write(&format!("{contract_dir}/s.hdi"), "module S\ndef run(x: Int): Int");
    case.write("main.hd", "def main = parent::child::M.run 7");
    case
}
#[test]
fn parent_and_sibling_contract_names_require_absolute_paths() {
    for contract_dir in ["parent", "parent/peer"] {
        let case = reference_case(contract_dir);
        perturb_rejection(&case, "parent/child/m.hd", "implement S as M\ndef run x => x + 3", "E204", "S");
    }
}
#[test]
fn parent_sibling_and_own_directory_absolute_paths_are_accepted() {
    for contract_dir in ["parent", "parent/peer", "parent/child"] {
        let case = reference_case(contract_dir);
        case.write("parent/child/m.hd", &format!("implement {} as M\ndef run x => x + 3", path(contract_dir, "S")));
        case.run("main.hd", 10);
    }
}
#[test]
fn own_directory_absolute_implementation_reference_is_accepted() {
    let case = Case::new();
    case.write("store/s.hdi", "module S\ndef run(x: Int): Int");
    case.write("store/m.hd", "implement S as M\ndef run x => x + 3\nimplement S as N\ndef run x => store::M.run x + 4");
    case.write("main.hd", "def main = store::N.run 7");
    case.run("main.hd", 14);
}
#[test]
fn same_names_in_distinct_directories_remain_distinct() {
    let case = Case::new();
    for (dir, offset) in [("color", 3), ("light", 11)] {
        case.write(&format!("{dir}/all.hd"), &format!("type Data = | Red(Int)\nmodule S\ndef run(value: Data): Int\nimplement S as M\ndef run | Red(n) => n + {offset}"));
    }
    case.write("main.hd", "def main = color::M.run (color::Red(7)) * 100 + light::M.run (light::Red(7))");
    case.run("main.hd", 1018);
}
#[test]
fn same_directory_duplicates_reject_the_second_declaration_in_each_space() {
    for (first, second, anchor) in [
        ("type Data = | First", "type Data = | Second", "Data"),
        ("module S", "module S", "S"),
        ("module S\nimplement S as M", "implement S as M", "M"),
        ("type First = | Shared", "type Second = | Shared", "Shared"),
    ] {
        let case = Case::new();
        case.write("main.hd", "def main = 1");
        case.write("nested/a.hd", first);
        perturb_rejection(&case, "nested/z.hd", second, "E205", anchor);
    }
}
#[test]
fn root_collisions_always_point_to_the_child_declaration_in_each_space() {
    for (root, child, anchor) in [
        ("type Data = | First", "type Data = | Second", "Data"),
        ("module S", "module S", "S"),
        ("module S\nimplement S as M", "implement S as M", "M"),
        ("type First = | Shared", "type Second = | Shared", "Shared"),
    ] {
        for root_file in ["a_root.hd", "z_root.hd"] {
            let case = Case::new();
            case.write("main.hd", "def main = 1");
            case.write(root_file, root);
            perturb_rejection(&case, "nested/child.hd", child, "E205", anchor);
        }
    }
}
#[test]
fn prelude_type_collisions_are_reported_in_the_child() {
    let case = Case::new();
    case.write("main.hd", "def main = 1");
    perturb_rejection(&case, "nested/type.hd", "type Option = | Local", "E205", "Option");
}
#[test]
fn invalid_directory_names_are_cli_failures_without_diagnostic_codes() {
    for directory in ["Upper", "9start", "bad-name", "_start"] {
        let case = Case::new();
        case.write("main.hd", "def main = 1");
        case.write(&format!("{directory}/empty.hd"), "");
        for command in ["check", "build", "dump-mono"] {
            let output = case.command(command, "main.hd");
            assert!(!output.status.success(), "accepted invalid directory {directory}");
            assert!(output.stdout.is_empty());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(!stderr.is_empty());
            assert!(!stderr.contains("error["), "{stderr}");
        }
    }
}
#[test]
fn selecting_a_child_file_makes_its_directory_the_root() {
    let case = Case::new();
    case.write("main.hd", "def main = 99");
    case.write("nested/empty.hd", "// no main\n\n");
    for command in ["check", "build", "dump-mono"] { case.reject(command, "nested/empty.hd", "nested/empty.hd", "E206", 0); }
}
#[test]
fn root_values_remain_file_local_even_when_modules_use_namespace_paths() {
    let case = Case::new();
    case.write("main.hd", "def seed = 7\ndef main = nested::M.run seed");
    case.write("nested/s.hdi", "module S\ndef run(x: Int): Int");
    perturb_rejection(&case, "nested/m.hd", "implement S as M\ndef run x => x + seed", "E204", "seed");
}
#[test]
fn root_types_and_prelude_names_are_visible_from_nested_directories() {
    let case = Case::new();
    case.write("main.hd", "type Root = | Root(Int)\ndef main = nested::deep::M.run (Root(7))");
    case.write("nested/deep/s.hdi", "module S\ndef run(value: Root): Int");
    case.write("nested/deep/m.hd", "implement S as M\ndef run | Root(n) => List.length [n, n]");
    case.run("main.hd", 2);
}
#[test]
fn parent_and_sibling_types_implementations_and_constructors_require_paths() {
    for directory in ["parent", "parent/peer"] {
        for kind in ["type", "implementation", "constructor"] {
            let case = Case::new();
            let qualified = directory.replace('/', "::");
            case.write(&format!("{directory}/all.hd"), "type Data = | Wrap(Int)\nmodule S\ndef run(value: Data): Int\nimplement S as M\ndef run | Wrap(n) => n + 3");
            case.write("main.hd", &format!("def main = parent::child::N.run ({qualified}::Wrap(7))"));
            case.write("parent/child/s.hdi", &format!("module Child\ndef run(value: {qualified}::Data): Int"));
            case.write("parent/child/n.hd", &format!("implement Child as N\ndef run | {qualified}::Wrap(n) => {qualified}::M.run ({qualified}::Wrap(n))"));
            match kind {
                "type" => perturb_rejection(&case, "parent/child/s.hdi", "module Child\ndef run(value: Data): Int", "E204", "Data"),
                "implementation" => perturb_rejection(&case, "parent/child/n.hd", "implement Child as N\ndef run x => M.run x", "E204", "M.run"),
                _ => perturb_rejection(&case, "parent/child/n.hd", &format!("implement Child as N\ndef run | Wrap(n) => {qualified}::M.run ({qualified}::Wrap(n))"), "E204", "Wrap(n)"),
            }
        }
    }
}
#[test]
fn absolute_constructor_expression_pattern_and_type_paths_are_accepted() {
    let case = Case::new();
    case.write("parent/data.hd", "type Data = | Wrap(Int)");
    case.write("parent/child/s.hdi", "module S\ndef run(value: parent::Data): Int");
    case.write("parent/child/m.hd", "implement S as M\ndef run | parent::Wrap(n) => n + 3");
    case.write("main.hd", "def main = parent::child::M.run (parent::Wrap(7))");
    case.run("main.hd", 10);
}
#[test]
fn implementation_abstract_type_paths_are_accepted_in_external_annotations() {
    let case = Case::new();
    case.write("store/s.hdi", "module S\ntype T\ndef make(x: Int): T\ndef read(x: T): Int");
    case.write("store/m.hd", "implement S as M\ntype T = Int\ndef make x => x + 3\ndef read x => x");
    case.write("main.hd", "def consume (x: store::M.T) => store::M.read x\ndef main = consume (store::M.make 7)");
    case.run("main.hd", 10);
}
#[test]
fn functor_targets_and_arguments_accept_absolute_implementation_paths() {
    let case = Case::new();
    case.write("s.hdi", "module S\ndef run(x: Int): Int");
    case.write("ids/i.hd", "implement S as I\ndef run x => x + 4");
    case.write("factory/f.hd", "implement S as Make(Param: S)\ndef run x => Param.run x + 3");
    case.write("main.hd", "def main = A.run 7\nimplement S as A = factory::Make(ids::I)");
    case.run("main.hd", 14);
}
#[test]
fn valid_directory_names_allow_digits_and_trailing_underscores() {
    let case = Case::new();
    case.write("main.hd", "def main = a1_::M.run 7");
    case.write("a1_/all.hd", "module S\ndef run(x: Int): Int\nimplement S as M\ndef run x => x + 3");
    case.run("main.hd", 10);
}
