use hado::parser;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Case(PathBuf);
impl Case {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hado-r18-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("main.hd"), source).unwrap();
        Self(path)
    }
    fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_hado"))
            .args(args)
            .arg(self.0.join("main.hd"))
            .output()
            .unwrap()
    }
    fn run(&self, expected: i64) {
        let output = self.command(&["build"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(self.0.join("main")).output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{expected}\n").as_bytes());
        assert!(output.stderr.is_empty());
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
fn reject(source: &str, code: &str, anchor_offset: usize) {
    let before = &source[..anchor_offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().chars().count() + 1;
    let case = Case::new(source);
    let output = case.command(&["check"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "{source}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let prefix = format!("{}:{line}:{column}: error[{code}]:", "main.hd");
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with(&prefix),
        "expected {prefix}\n{}\n{source}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[derive(Clone)]
enum Type {
    Atom(&'static str),
    Apply(&'static str, Vec<Type>),
    Function(Vec<Type>, Box<Type>),
}
impl Type {
    fn render(&self) -> String {
        match self {
            Self::Atom(name) => (*name).into(),
            Self::Apply(name, args) if args.len() == 1 => format!("{} {name}", args[0].render()),
            Self::Apply(name, args) => format!("({}) {name}", render_types(args)),
            Self::Function(args, ret) => format!("Fn({} => {})", render_types(args), ret.render()),
        }
    }
}
fn render_types(types: &[Type]) -> String {
    types
        .iter()
        .map(Type::render)
        .collect::<Vec<_>>()
        .join(", ")
}
fn generated_type(state: &mut u64, depth: usize) -> Type {
    if depth == 0 {
        return Type::Atom(["Int", "Bool", "String", "Y.T"][random(state, 4)]);
    }
    match random(state, 4) {
        0 => Type::Apply("List", vec![generated_type(state, depth - 1)]),
        1 => Type::Apply("Option", vec![generated_type(state, depth - 1)]),
        2 => Type::Apply(
            "Result",
            vec![
                generated_type(state, depth - 1),
                generated_type(state, depth - 1),
            ],
        ),
        _ => Type::Function(
            vec![
                generated_type(state, depth - 1),
                generated_type(state, depth - 1),
            ],
            Box::new(generated_type(state, depth - 1)),
        ),
    }
}

#[test]
fn generated_types_roundtrip_in_canonical_form() {
    let mut state = 0x18719;
    for _ in 0..32 {
        let ty = Type::Function(
            vec![generated_type(&mut state, 2), Type::Atom("Y.T")],
            Box::new(Type::Apply(
                "Result",
                vec![
                    generated_type(&mut state, 2),
                    Type::Function(vec![Type::Atom("Int")], Box::new(Type::Atom("Int"))),
                ],
            )),
        )
        .render();
        let definition = format!("def probe (x: {ty}): {ty} => x");
        let ast = parser::parse_source(&definition, "generated.hd")
            .unwrap_or_else(|e| panic!("{e}\n{definition}"));
        let printed = parser::canonical_program(&ast);
        let canonical = format!("{definition}\n");
        assert_eq!(printed, canonical);
        let restored = parser::parse_source(&printed, "restored.hd").unwrap();
        assert_eq!(parser::canonical_program(&restored), canonical);
    }
}

fn printable_value(state: &mut u64, depth: usize) -> (Type, String) {
    if depth == 0 {
        return match random(state, 3) {
            0 => (Type::Atom("Int"), random(state, 1000).to_string()),
            1 => (Type::Atom("Bool"), (random(state, 2) == 0).to_string()),
            _ => (
                Type::Atom("String"),
                format!("\"word{}\"", random(state, 1000)),
            ),
        };
    }
    let (inner, value) = printable_value(state, depth - 1);
    match random(state, 2) {
        0 => (Type::Apply("List", vec![inner]), format!("[{value}]")),
        _ => (Type::Apply("Option", vec![inner]), format!("Some({value})")),
    }
}

#[test]
fn printable_main_types_match_independently_generated_types() {
    let mut state = 0x18_c11;
    for _ in 0..8 {
        let (ty, value) = printable_value(&mut state, 3);
        let ty = ty.render();
        let case = Case::new(&format!(
            "def probe(x: {ty}): {ty} => x\ndef main = probe ({value})"
        ));
        let output = case.command(&["check", "--print-type"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, format!("{ty}\n").as_bytes());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn lambda_parameter_annotations_cannot_be_mixed() {
    for source in [
        "def main = fn(x: Int, y => x + y)",
        "def main = fn(x, y: Int => x + y)",
    ] {
        let error = parser::parse_source(source, "mixed.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"));
    }
}

#[test]
fn empty_arm_columns_point_at_the_first_pipe() {
    for source in [
        "def main = if | 5",
        "def main = fn(| 5)",
        "def f x | 5",
        "def f | 5",
    ] {
        let error = parser::parse_source(source, "empty.hd").unwrap_err();
        assert_eq!(error.code.as_deref(), Some("E100"));
        assert_eq!(
            (error.line, error.col),
            (1, source.find('|').unwrap() + 1),
            "{source}"
        );
    }
}

#[test]
fn old_spellings_and_comparison_chains_reject_at_the_token() {
    for (source, anchor) in [
        ("def main = 1 < 2 < 3", "< 3"),
        (
            "def id(x: Option<Int>): Option<Int> => x\ndef main = id (Some(1))",
            "<",
        ),
        (
            "def id(x: Fn<Int, Int>): Fn<Int, Int> => x\ndef main = id",
            "<",
        ),
        (
            "type Option<T> = | Nothing | Just(T)\ndef main = Just(1)",
            "<",
        ),
        ("def f<A>(x: A): A => x\ndef main = f 1", "<"),
        ("def main = fn x => x", "fn"),
        (
            "def f(x: (Int) List): Int List => x\ndef main = f [1]",
            "(Int)",
        ),
    ] {
        for padding in 0..2 {
            let source = format!(
                "{}{}{source}",
                "\n".repeat(padding),
                " ".repeat(padding * 2)
            );
            reject(&source, "E100", source.find(anchor).unwrap());
        }
    }
}

#[test]
fn lambda_arguments_and_arm_bodies_execute_without_consuming_arms() {
    let mut state = 0x18_f00d;
    for _ in 0..4 {
        let a = random(&mut state, 100) as i64;
        let b = a + 1 + random(&mut state, 100) as i64;
        let source = format!("def apply f x y => f x y\ndef choose | Some(n) => fn(x, y => x + y + n) | None => fn(x, y => x - y)\ndef main = (apply fn(x, y => x + y) {a} {b}) + (apply (choose (Some({a}))) {a} {b}) + (apply (choose None) {a} {b})");
        Case::new(&source).run((a + b) + (a + b + a) + (a - b));
        let source = format!("type Choice = | First | Second\ndef apply f x => f x\ndef main = (apply fn(| First => {a} | Second => {b}) First) + (apply fn(x | x > 0 => x | 0) {b})");
        Case::new(&source).run(a + b);
    }
    for body in ["fn(| First => 1 | Second => 2)", "fn(x | x > 0 => x | 0)"] {
        let source = format!("type Choice = | First | Second\ndef choose | Some(n) => {body} | None => fn(x => x)\ndef main = choose None");
        reject(&source, "E101", source.find("fn").unwrap());
    }
}

#[test]
fn boolean_alternative_forms_and_position_perturbations() {
    for (source, anchor) in [
        (
            "def f(b: Bool) => if b | true => 1 | false => 0\ndef main = f true",
            "if",
        ),
        (
            "def f(b: Bool) => if b | true => 1 | 0\ndef main = f true",
            "if",
        ),
        (
            "def positive x => x > 0\ndef main = if | positive 1 => 1 | 0",
            "if",
        ),
        ("def main = if | true => 1 | 0", "if"),
        ("def f | true => 1 | false => 0\ndef main = f true", "def f"),
        (
            "def apply f x => f x\ndef main = apply fn(| true => 1 | false => 0) true",
            "fn",
        ),
    ] {
        for padding in 0..3 {
            let source = format!(
                "{}{}",
                "\n".repeat(padding % 3),
                source.replace('\n', &format!("\n{}", " ".repeat(padding)))
            );
            let offset = source.find(anchor).unwrap();
            let source = format!(
                "{}{}{}",
                &source[..offset],
                " ".repeat(padding * 2),
                &source[offset..]
            );
            reject(&source, "E520", offset + padding * 2);
        }
    }
}

#[test]
fn nested_boolean_patterns_and_direct_condition_functions_remain_valid() {
    let mut state = 0x18_520;
    for _ in 0..4 {
        let value = 1 + random(&mut state, 100) as i64;
        let source = format!("def nested | Some(true) => {value} | Some(false) => 0 | None => -1\ndef direct x | x > 0 => x | 0\ndef main = (nested (Some(true))) + (direct {value})");
        Case::new(&source).run(value * 2);
    }
}
