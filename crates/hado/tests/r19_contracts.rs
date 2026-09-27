use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "hado-r19-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("main.hd"), source).unwrap();
        Self(dir)
    }
    fn command(&self, command: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado"))
            .arg(command)
            .arg(self.0.join("main.hd"))
            .output()
            .unwrap()
    }
    fn run(&self, expected: &str) {
        let output = self.command("build");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let output = Command::new(self.0.join("main")).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
    fn rejects(&self, source: &str, code: &str, offset: usize, command: &str) {
        let prefix = &source[..offset];
        let output = self.command(command);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{source}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8(output.stderr).unwrap();
        let location = format!(
            "{}:{}:{}: error[{code}]: ",
            "main.hd",
            prefix.matches('\n').count() + 1,
            prefix.rsplit('\n').next().unwrap().chars().count() + 1
        );
        assert!(
            diagnostic.starts_with(&location),
            "expected {location}\n{diagnostic}\n{source}"
        );
        assert_eq!(diagnostic.lines().count(), 1);
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn random(state: &mut u64, limit: usize) -> usize {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as usize) % limit
}
fn applied(args: &[String], name: &str) -> String {
    match args {
        [] => name.to_owned(),
        [arg] => format!("{arg} {name}"),
        _ => format!("({}) {name}", args.join(", ")),
    }
}
fn rejected(source: &str, code: &str, anchor: &str) {
    let offset = source.find(anchor).unwrap();
    Case::new(source).rejects(source, code, offset, "check");
}
fn perturbed(source: &str, code: &str, anchor: &str) {
    let offset = source.find(anchor).unwrap();
    for padding in 0..5 {
        let lead = format!(
            "// generated position {padding}\n{}",
            "\n".repeat(padding % 3)
        );
        let shifted = format!(
            "{lead}{}{}{}",
            &source[..offset],
            " ".repeat(padding),
            &source[offset..]
        );
        Case::new(&shifted).rejects(&shifted, code, lead.len() + offset + padding, "check");
    }
}

fn generated_store(arity: usize, seed: i64, offset: i64) -> (String, i64) {
    let mut source = store_header(arity, seed);
    let (filled, reads, expected) = generated_fields(arity.max(1), seed, offset);
    source.push_str(&format!(
        "\ndef empty = {{ {} }}\ndef fill seed => {{ {filled} }}\ndef read value => {reads}\n/implement",
        (0..arity.max(1))
            .map(|i| format!("slot{i} = None"))
            .collect::<Vec<_>>()
            .join("; ")
    ));
    (source, expected)
}
fn store_header(arity: usize, seed: i64) -> String {
    let vars: Vec<_> = (0..arity).map(|i| format!("A{i}")).collect();
    let types: Vec<_> = (0..arity)
        .map(|i| ["Int", "Bool", "String"][i % 3].into())
        .collect();
    let fields: Vec<_> = (0..arity.max(1))
        .map(|i| {
            format!(
                "slot{i}: {} Option",
                vars.get(i).map(String::as_str).unwrap_or("Int")
            )
        })
        .collect();
    format!("type {} = {{ {} }}\ndef main = M.read M.empty + M.read (M.fill ({seed}))\nmodule Store\ntype {}\ndef {}: {}\ndef fill (seed: Int): {}\ndef read (value: {}): Int\n/module\nimplement Store as M\ntype {} = {}", applied(&vars, "Payload"), fields.join("; "), applied(&vars, "T"), applied(&vars, "empty"), applied(&vars, "T"), applied(&types, "T"), applied(&types, "T"), applied(&vars, "T"), applied(&vars, "Payload"))
}
fn generated_fields(count: usize, seed: i64, offset: i64) -> (String, String, i64) {
    let mut filled = Vec::new();
    let mut reads = Vec::new();
    let mut expected = 0;
    for i in 0..count {
        match i % 3 {
            0 => {
                filled.push(format!("slot{i} = Some(seed + {offset})"));
                reads.push(format!("(Option.unwrap_or value.slot{i} 0)"));
                expected += seed + offset;
            }
            1 => {
                filled.push(format!("slot{i} = Some(seed > {offset})"));
                reads.push(format!(
                    "(if Option.unwrap_or value.slot{i} false => {} | 0)",
                    offset + 1
                ));
                if seed > offset {
                    expected += offset + 1;
                }
            }
            _ => {
                filled.push(format!("slot{i} = Some(Int.to_string (seed + {offset}))"));
                reads.push(format!(
                    "(String.length (Option.unwrap_or value.slot{i} \"\"))"
                ));
                expected += (seed + offset).to_string().chars().count() as i64;
            }
        }
    }
    (filled.join("; "), reads.join(" + "), expected)
}

#[test]
fn generated_abstract_arities_and_value_members_execute() {
    let mut state = 0x19_a11;
    for arity in 0..5 {
        let seed = random(&mut state, 100) as i64 + 1;
        let offset = random(&mut state, 30) as i64;
        let (source, expected) = generated_store(arity, seed, offset);
        Case::new(&source).run(&expected.to_string());
    }
    let source = "def main = M.count_int M.empty + M.count_bool M.empty\nmodule S\ntype A T\ndef A empty: A T\ndef count_int (value: Int T): Int\ndef count_bool (value: Bool T): Int\n/module\nimplement S as M\ntype A T = A List\ndef empty = []\ndef count_int value => List.length value\ndef count_bool value => List.length value\n/implement";
    Case::new(source).run("0");
}

#[test]
fn contract_arity_mismatches_are_e602() {
    for (arity, actual) in [(1usize, 0usize), (1, 2)] {
        let declared: Vec<_> = (0..arity).map(|i| format!("A{i}")).collect();
        let realized: Vec<_> = (0..actual).map(|i| format!("B{i}")).collect();
        let fields = if realized.is_empty() {
            "value: Int".into()
        } else {
            realized
                .iter()
                .enumerate()
                .map(|(i, name)| format!("field{i}: {name} Option"))
                .collect::<Vec<_>>()
                .join("; ")
        };
        let source = format!("type {} = {{ {fields} }}\ndef main = M.run 1\nmodule S\ntype {}\ndef run (x: Int): Int\n/module\nimplement S as M\ntype {} = {}\ndef run x => x\n/implement", applied(&realized, "Payload"), applied(&declared, "T"), applied(&realized, "T"), applied(&realized, "Payload"));
        if actual == 0 {
            perturbed(&source, "E602", "implement");
        } else {
            rejected(&source, "E602", "implement");
        }
    }
}

#[test]
fn value_member_implementation_form_is_distinct_from_function_definition() {
    for (signature, implementation) in [
        ("def value: Int", "def value x => x"),
        ("def operation: Fn(Int => Int)", "def operation x => x + 1"),
    ] {
        let source =
            format!("def main = 1\nmodule S\n{signature}\n/module\nimplement S as M\n{implementation}\n/implement");
        rejected(&source, "E602", "implement");
    }
    Case::new("def main = M.operation 41\nmodule S\ndef operation: Fn(Int => Int)\n/module\nimplement S as M\ndef operation = fn(x => x + 1)\n/implement").run("42");
}

fn argument_program(place: usize, arguments: &str, invocation: &str) -> String {
    match place {
        0 => format!("def main = M.run {invocation}\nmodule S\ndef run (first: Int, second: Int, third: Int): Int\n/module\nimplement S as M\ndef run {arguments} => used + 7\n/implement"),
        1 => format!("def main = fn({arguments} => used + 7) {invocation}"),
        2 => format!("def run {arguments} => used + 7\ndef main = run {invocation}"),
        _ => format!("def main = M.run {invocation}\nmodule S\ndef run (first: Int, second: Int, third: Int): Int\n/module\nimplement S as M\ndef helper {arguments} => used + 7\ndef run x y z => helper x y z\n/implement"),
    }
}
#[test]
fn underscores_preserve_argument_positions_and_named_unused_arguments_reject() {
    for typed in [false, true] {
        for place in 0..4 {
            let args = argument_names(0, typed, place);
            let source = argument_program(place, &args, "11 23 37");
            match place {
                0 | 1 => Case::new(&source).run("18"),
                2 => rejected(&source, "E519", "_"),
                _ => perturbed(&source, "E519", "_"),
            }
        }
    }
    for place in [0, 3] {
        let args = argument_names(0, false, place);
        named_unused_rejects(&argument_program(place, &args, "11 23 37"));
    }
}

fn argument_names(position: usize, typed: bool, place: usize) -> String {
    let names: Vec<_> = (0..3)
        .map(|i| if i == position { "used" } else { "_" })
        .collect();
    let args = if typed {
        names
            .iter()
            .map(|name| format!("{name}: Int"))
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        names.join(if place == 1 { ", " } else { " " })
    };
    if typed && place != 1 {
        format!("({args})")
    } else {
        args
    }
}
fn named_unused_rejects(source: &str) {
    let source = source
        .replacen('_', "unused", 1)
        .replace('_', "other_unused");
    Case::new(&source).rejects(&source, "E505", source.find("unused").unwrap(), "check");
}

fn contract_count_program(count: usize) -> String {
    let calls = (0..count)
        .map(|i| format!("(Id{i}.get (Id{i}.make {}))", i + 2))
        .collect::<Vec<_>>();
    let mut source = format!("def main = C.answer{}\nmodule X\ntype T\ndef make (x: Int): T\ndef get (x: T): Int\n/module\nmodule Consumer\ndef answer: Int\ndef use (x: X.T): Int\n/module\n", calls.iter().map(|call| format!(" + {call}")).collect::<String>());
    for i in 0..count {
        source.push_str(&format!(
            "implement X as Id{i}\ntype T = Int\ndef make x => x\ndef get x => x\n/implement\n"
        ));
    }
    source.push_str("implement Consumer as C\ndef answer = 9\ndef use _ => 1\n/implement");
    source
}
#[test]
fn contract_qualified_types_require_exactly_one_implementation() {
    for count in 0..3 {
        let source = contract_count_program(count);
        match count {
            1 => Case::new(&source).run("11"),
            0 => perturbed(&source, "E603", "X.T"),
            _ => rejected(&source, "E603", "X.T"),
        }
    }
    let source = "def main = C.use (Id.make 42)\nmodule X\ntype T\ndef make (x: Int): T\ndef get (x: T): Int\n/module\nmodule Consumer\ndef use (x: X.T): Int\n/module\nimplement X as Id\ntype T = Int\ndef make x => x\ndef get x => x\n/implement\nimplement Consumer as C\ndef use x => Id.get x\n/implement";
    Case::new(source).run("42");
    rejected(&source.replace("x: X.T", "x: Id.T"), "E204", "Id.T");
    rejected(&source.replace("x: X.T", "x: X.Missing"), "E204", "Missing");
    Case::new(&source.replace("as Id", "as X").replace("Id.", "X.")).run("42");
    Case::new("def main = M.read (M.make 7)\nmodule S\ntype T\ndef make (n: Int): S.T\ndef read (v: S.T): Int\n/module\nimplement S as M\ntype T = Int\ndef make n => n\ndef read v => v\n/implement").run("7");
}

#[test]
fn unused_type_variables_use_type_references_not_constructor_names() {
    for (index, source) in [
        "type A Box = | Wrap(Int)\ndef read (value: Int Box): Int => value as Wrap(n) ->\n  n\ndef main = read Wrap(7)",
        "type A Box = { value: Int }\ndef read(value: Int Box): Int => value.value\ndef main = read { value = 7 }",
        "type A Box = | A(Int)\ndef read (value: Int Box): Int => value as A(n) ->\n  n\ndef main = read A(7)",
        "def main = M.run 7\nmodule S\ndef A run (x: Int): Int\n/module\nimplement S as M\ndef run x => x\n/implement",
        "def main = M.run 7\nmodule S\ndef run (x: Int): Int\n/module\nimplement S as M\ntype A Box = | Wrap(Int)\ndef run x => Wrap(x) as Wrap(n) ->\n  n\n/implement",
        "def main = M.run 7\nmodule S\ndef run (x: Int): Int\n/module\nimplement S as M\ntype A Box = { value: Int }\ndef run x => { value = x } as box ->\n  box.value\n/implement",
        "def main = M.read (M.make 7)\nmodule S\ntype A T\ndef make (x: Int): Int T\ndef read (x: Int T): Int\n/module\nimplement S as M\ntype A T = Int\ndef make x => x\ndef read x => x\n/implement",
    ]
    .into_iter()
    .enumerate()
    {
        let anchor = if source.contains("type A T =") { "A T =" } else if source.contains("def A run") { "A run" } else { "A Box" };
        if index == 0 {
            perturbed(source, "E518", anchor);
        } else {
            rejected(source, "E518", anchor);
        }
    }
    Case::new("type A Box = | Wrap(A)\ndef read | Wrap(n) => n\ndef main = read (Wrap(7))")
        .run("7");
    for parameters in ["(A, B)", "(B, A)", "(B, A, C)"] {
        let source = format!(
            "type {parameters} Box = | Wrap(A)\ndef read | Wrap(n) => n\ndef main = read (Wrap(7))"
        );
        rejected(&source, "E518", "B");
    }
}

#[test]
fn main_abstract_types_are_rejected_inside_fixed_and_generic_payloads() {
    let suffix = "module S\ntype T\ndef make (x: Int): T\ndef get (x: T): Int\n/module\nimplement S as M\ntype T = Int\ndef make x => x\ndef get x => x\n/implement";
    for (index, (prefix, main)) in [
        ("", "M.make 7"),
        ("", "Some(M.make 7)"),
        ("", "[M.make 7]"),
        ("type Box = { value: M.T }\n", "{ value = M.make 7 }"),
        ("type Box = | Wrap(M.T)\n", "Wrap(M.make 7)"),
        ("type A Box = | Wrap(A)\n", "Wrap(M.make 7)"),
        (
            "type Chain = | Last(M.T) | Next(Chain)\n",
            "Next(Last(M.make 7))",
        ),
    ]
    .iter()
    .enumerate()
    {
        let source = format!("{prefix}def main = {main}{suffix}");
        if index == 0 {
            let case = Case::new(&source);
            for command in ["check", "build", "dump-mono"] {
                case.rejects(&source, "E705", source.find("def main").unwrap(), command);
                assert!(!case.0.join("main").exists());
            }
        } else {
            rejected(&source, "E705", "def main");
        }
    }
    Case::new(&format!("def main = M.get (M.make 7){suffix}")).run("7");
}

#[test]
fn new_diagnostics_keep_stage_and_source_order() {
    for (earlier, later, code, anchor) in [
        ("def run _ => 1", "def broken = missing", "E204", "missing"),
        (
            "def run _ => 1",
            "def broken = 1 + true",
            "E305",
            "1 + true",
        ),
        (
            "def run _ => 1",
            "module S\ndef x: Int\n/module\nimplement S as M\ndef x = true\n/implement",
            "E602",
            "implement",
        ),
    ] {
        let source = format!("{earlier}\ndef main = run 1\n{later}");
        rejected(&source, code, anchor);
    }
    for (first, second, code, anchor) in [
        ("def left _ => 1", "def right unused => 2", "E519", "_"),
        ("def right unused => 2", "def left _ => 1", "E505", "unused"),
        (
            "type A Box = | Wrap(Int)",
            "def left _ => 1",
            "E518",
            "A Box",
        ),
        ("def left _ => 1", "type A Box = | Wrap(Int)", "E519", "_"),
    ] {
        let main = if first.contains("Box") || second.contains("Box") {
            "def read (value: Int Box): Int => value as Wrap(n) ->\n  n\ndef main = left 1 + read Wrap(2)"
        } else {
            "def main = (left 1) + (right 2)"
        };
        let source = format!("{first}\n{second}\n{main}");
        rejected(&source, code, anchor);
    }
    let source = "def main = M.make (bad 1)\ndef bad _ => 7\nmodule S\ntype T\ndef make (x: Int): T\n/module\nimplement S as M\ntype T = Int\ndef make x => x\n/implement";
    rejected(source, "E519", "_");
    let mismatch =
        "module Broken\ndef run (x: Int): Int\n/module\nimplement Broken as Bad\ndef run x => true\n/implement";
    let unresolved = "module X\ntype T\n/module\nmodule Consumer\ndef answer: Int\ndef use (x: X.T): Int\n/module\nimplement Consumer as C\ndef answer = 1\ndef use _ => 1\n/implement";
    rejected(
        &format!("def main = (Bad.run 1) + C.answer\n{unresolved}\n{mismatch}"),
        "E603",
        "X.T",
    );
}

#[test]
fn parameterized_contract_references_keep_qualified_name_anchors() {
    for (index, annotation) in ["Int X.T", "Fn(Int X.T => Int)", "(Int X.T, Bool) Pair"]
        .into_iter()
        .enumerate()
    {
        let source = format!("type (A, B) Pair = | Hold(A, B)\ndef main = C.answer\nmodule X\ntype A T\n/module\nmodule Consumer\ndef answer: Int\ndef use (value: {annotation}): Int\n/module\nimplement Consumer as C\ndef answer = 1\ndef use _ => 1\n/implement");
        if index == 1 {
            perturbed(&source, "E603", "X.T");
        } else {
            rejected(&source, "E603", "X.T");
        }
        rejected(&source.replace("X.T", "X.Missing"), "E204", "Missing");
    }
    let source = "def main = C.use (Id.make 7)\nmodule X\ntype A T\ndef make (n: Int): Int T\ndef get (value: Int T): Int\n/module\nmodule Consumer\ndef use (value: Int X.T): Int\n/module\nimplement X as Id\ntype A T = A List\ndef make n => [n]\ndef get value => List.length value\n/implement\nimplement Consumer as C\ndef use value => Id.get value\n/implement";
    Case::new(source).run("1");
}

#[test]
fn qualified_realization_cycles_report_e602_without_overflowing() {
    for (variables, applied, representation) in [("", "", "M.T"), ("A ", "Int ", "A M.T")] {
        let source = format!("def main = M.get (M.make 1)\nmodule S\ntype {variables}T\ndef make (n: Int): {applied}T\ndef get (v: {applied}T): Int\n/module\nimplement S as M\ntype {variables}T = {representation}\ndef make n => n\ndef get v => v\n/implement");
        rejected(&source, "E602", "implement");
    }
    let source = "def main = A.read (None)\nmodule S\ntype T\ndef read (value: T Option): Int\n/module\nimplement S as A\ntype T = B.T\ndef read _ => 1\n/implement\nimplement S as B\ntype T = A.T\ndef read _ => 2\n/implement";
    rejected(source, "E602", "implement");
}
