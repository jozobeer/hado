use hado::{check_source, parser};

fn random(state: &mut u64, limit: u64) -> usize {
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    ((*state >> 32) % limit) as usize
}

fn rejected(source: &str, code: &str, anchor: &str) {
    let error = check_source(source, "sections.hd").err().unwrap();
    let before = &source[..source.find(anchor).unwrap()];
    assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
    assert_eq!((error.line, error.col),
        (before.matches('\n').count() + 1, before.rsplit('\n').next().unwrap().chars().count() + 1),
        "{source}: {error}");
}

fn execute(source: &str, expected: i64, serial: usize) {
    let dir = std::env::temp_dir().join(format!("hado-r16-{}-{serial}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("main.hd");
    std::fs::write(&path, source).unwrap();
    let built = std::process::Command::new(env!("CARGO_BIN_EXE_hado")).arg("build").arg(&path).output().unwrap();
    assert!(built.status.success(), "{source}\n{}", String::from_utf8_lossy(&built.stderr));
    let output = std::process::Command::new(path.with_extension("")).output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
    assert!(output.stderr.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[derive(Default)]
struct Program {
    sections: Vec<String>,
    calls: Vec<String>,
    expected: i64,
}

fn generate(state: &mut u64) -> Program {
    let mut program = Program::default();
    for contract in 0..random(state, 4) + 1 {
        program.sections.push(format!("module Contract{contract}\ndef run(x: Int): Int"));
        for implementation in 0..random(state, 3) + 2 {
            let value = random(state, 100) as i64;
            let offset = random(state, 100) as i64;
            program.sections.push(format!("implement Contract{contract} as Impl{contract}_{implementation}\ndef run x => x + {offset}"));
            program.calls.push(format!("Impl{contract}_{implementation}.run {value}"));
            program.expected += value + offset;
        }
    }
    program
}

#[test]
fn generated_contracts_and_implementations_round_trip_and_execute_in_any_order() {
    let mut state = 0x160012;
    for sample in 0..24 {
        let mut program = generate(&mut state);
        for index in (1..program.sections.len()).rev() {
            let other = random(&mut state, (index + 1) as u64);
            program.sections.swap(index, other);
        }
        let source = format!("def main = {}\n{}", program.calls.join(" + "), program.sections.join("\n"));
        let ast = parser::parse_source(&source, "sections.hd").unwrap();
        assert_eq!(ast.items.len(), program.sections.len() + 1);
        let canonical = parser::canonical_program(&ast);
        let restored = parser::parse_source(&canonical, "sections.hd").unwrap();
        assert_eq!(restored.items.len(), ast.items.len());
        assert_eq!(canonical, parser::canonical_program(&restored));
        check_source(&canonical, "sections.hd").unwrap();
        execute(&source, program.expected, sample);
    }
}

#[test]
fn section_errors_preserve_namespace_and_declaration_anchors() {
    for padding in 0..8 {
        for (source, code, anchor) in [
            ("def main = 1\nmodule Unused\ndef run(x: Int): Int", "E513", "module Unused"),
            ("def main = 1\nmodule S\ndef run(x: Int): Int\nimplement S as Unused\ndef run x => x + 1", "E513", "implement S"),
            ("def main = 1\nimplement Missing as M\ndef run x => x + 1", "E204", "Missing"),
            ("def main = 1\nmodule S\ntype T = Int", "E100", "= Int"),
            ("def main = M.run 1\nmodule S\ndef run(x: Int): Int\nimplement S as M\ntype U = Int\ndef run x => x + 1", "E204", "U ="),
            ("def main = M.run 1\nmodule S\ndef run(x: Int): Bool\nimplement S as M\ndef run x => x + 1", "E602", "implement S"),
            ("def main = M.run 1\nmodule S\ndef run(x: Int): Int\nimplement S as M\ndef dead x => x + 2\ndef run x => x + 1", "E514", "def dead"),
        ] {
            let source = format!("{}{}{}", "\n".repeat(padding % 3), " ".repeat(padding), source);
            rejected(&source, code, anchor);
        }
    }
}

#[test]
fn obsolete_module_forms_are_syntax_errors_and_old_keywords_are_names() {
    for source in ["module type S = sig end", "module M : S = struct end"] {
        let error = parser::parse_source(source, "old.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"));
    }
    check_source("def sig x => x + 1\ndef struct x => sig x\ndef end x => struct x\ndef main = end 1", "names.hd").unwrap();
}

#[test]
fn abstract_types_are_opaque_and_local_data_names_do_not_escape() {
    let sections = "module S\ntype T\ndef make(x: Int): T\nimplement S as M\ntype Entry = { number: Int }\ntype T = Entry\ndef make x => { number = x }";
    for (outer, code, anchor) in [
        ("def main = (M.make 1).number", "E601", ".number"),
        ("def take (x: M.Entry): Int => 1\ndef main = take (M.make 1)", "E204", "Entry"),
        ("def take (x: S.T): Int => 1\ndef main = take (M.make 1)", "E204", "S.T"),
    ] {
        rejected(&format!("{outer}\n{sections}"), code, anchor);
    }
    for separator in [".", " . ", ".\n  "] {
        rejected(&format!("def take (x: M{separator}Entry): Int => 1\ndef main = take (M.make 1)\n{sections}"), "E204", "Entry");
    }
    rejected(&format!("def main = M.make 1\nmodule Bad\ndef consume(x: M.T): Int\n{sections}"), "E204", "M.T");
    rejected("type Entry = { number: Int }\ndef main = M.run 1\nmodule S\ndef run(x: Int): Int\nimplement S as M\ntype Entry = | Entry(Int)\ndef run x => x + 1", "E205", "Entry = |");
    rejected("def main = M.run 1\nmodule S\ntype Entry\ndef run(x: Int): Int\nimplement S as M\ntype Entry = { number: Int }\ndef run x => x + 1", "E205", "Entry = {");
}

#[test]
fn private_records_and_adts_are_scoped_per_implementation() {
    let mut state = 0x16da7a;
    for sample in 0..12 {
        let n = random(&mut state, 1000) as i64;
        let source = format!("def main = A.run {n} + B.run {n}\nmodule S\ndef run(x: Int): Int\nimplement S as A\ntype Entry = {{ number: Int }}\ndef get value => value.number\ndef run x => get {{ number = x + 1 }}\nimplement S as B\ntype Entry = | Entry(Int)\ndef get | Entry(x) => x\ndef run x => get (Entry(x + 2))");
        execute(&source, n * 2 + 3, 100 + sample);
    }
}

#[test]
fn implementation_local_data_requires_a_reachable_use() {
    for index in 0..12 {
        for body in [format!("type Dead{index} = {{ unused{index}: Int }}"), format!("type Dead{index} = | Dead{index}(Int)"), format!("type Dead{index} = | Dead{index}(Other{index}) type Other{index} = | Other{index}(Dead{index})")] {
            let source = format!("def main = M.run 1\nmodule S def run(x: Int): Int\nimplement S as M\n{}{}\ndef run x => x + 1", " ".repeat(index), body);
            rejected(&source, "E513", &format!("type Dead{index}"));
        }
        for body in [
            format!("type Used{index} = {{ value{index}: Int }} def run x => let r = {{ value{index} = x }} -> r.value{index}"),
            format!("type Used{index} = | Wrap{index}(Int) def run x => let Wrap{index}(n) = Wrap{index}(x) -> n"),
        ] {
            let source = format!("def main = M.run 1\nmodule S def run(x: Int): Int\nimplement S as M {body}");
            check_source(&source, "local-data.hd").unwrap();
        }
    }
    check_source("def main = M.get (M.make 1)\nmodule S type T def make(x: Int): T def get(value: T): Int\nimplement S as M type Inner = { value: Int } type T = Inner def make x => { value = x } def get item => item.value", "local-data.hd").unwrap();
}
