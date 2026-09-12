use hado::parser::{self, parse_source, Diagnostic, Item, TypeDef};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn parse(source: &str) -> parser::Ast {
    parse_source(source, "generated.hd").unwrap_or_else(|error| panic!("{error}\n{source}"))
}

fn location(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    (before.matches('\n').count() + 1, before.rsplit('\n').next().unwrap().chars().count() + 1)
}

// 診断の行:列は探針の行頭の空白と先頭の空行から計算する（定数で書かない）。
fn reject(source: &str, code: &str, needle: &str) {
    for blank in 0..3usize {
        for pad in 0..4usize {
            let mut shifted = "\n".repeat(blank);
            for line in source.split_inclusive('\n') {
                shifted.push_str(&" ".repeat(pad));
                shifted.push_str(line);
            }
            let offset = shifted.find(needle).unwrap_or_else(|| panic!("needle {needle}\n{shifted}"));
            let error = parse_source(&shifted, "generated.hd").expect_err(&shifted);
            assert_eq!(error.code.as_deref(), Some(code), "{error}\n{shifted}");
            assert_eq!((error.line, error.col), location(&shifted, offset), "{error}\n{shifted}");
        }
    }
}

fn find_type<'a>(items: &'a [Item], name: &str) -> Option<&'a TypeDef> {
    for item in items {
        match item {
            Item::TypeDef(def) if def.name == name => return Some(def),
            Item::ModuleImpl(module) => {
                if let Some(def) = find_type(&module.items, name) {
                    return Some(def);
                }
            }
            _ => {}
        }
    }
    None
}

fn type_def<'a>(items: &'a [Item], name: &str) -> &'a TypeDef {
    find_type(items, name).unwrap_or_else(|| panic!("type {name} not found"))
}

struct Case(PathBuf);
impl Case {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!("hado-r33-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn write(&self, file: &str, source: &str) {
        std::fs::write(self.0.join(file), source).unwrap();
    }
    fn check(&self, entry: &str) -> Result<(), Diagnostic> {
        hado::project::Project::load(&self.0.join(entry)).unwrap().check().map(|_| ())
    }
}
impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const MONOID: &str = "module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure";
const IMPLEMENTATION: &str = "implement Monoid as StringMonoid\nshared type T = String\ndef combine left _ => left\ndef of value => value";

#[test]
fn shared_realization_is_recorded() {
    let ast = parse(&format!("{MONOID}\n\n{IMPLEMENTATION}"));
    let def = type_def(&ast.items, "T");
    assert!(def.shared);
    assert!(def.ty.is_some());
}

#[test]
fn unmarked_realization_is_not_shared() {
    let source = "module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\n\nimplement Monoid as StringMonoid\ntype T = String\ndef combine left _ => left\ndef of value => value";
    let ast = parse(source);
    assert!(!type_def(&ast.items, "T").shared);
}

#[test]
fn shared_survives_the_canonical_round_trip() {
    let source = format!("{MONOID}\n\n{IMPLEMENTATION}");
    let ast = parse(&source);
    let canonical = parser::canonical_program(&ast);
    let restored = parse(&canonical);
    assert!(type_def(&restored.items, "T").shared, "{canonical}");
    assert_eq!(canonical, parser::canonical_program(&restored));
}

#[test]
fn shared_prefixes_a_parameterized_realization() {
    let source = "module Stack\ntype A T\ndef A push(stack: A T, value: A): A T\n\nimplement Stack as IntStack\nshared type A T = A List\ndef push stack value => [value, ...stack]";
    let ast = parse(source);
    let def = type_def(&ast.items, "T");
    assert!(def.shared);
    assert_eq!(def.params.iter().map(|name| name.name.as_str()).collect::<Vec<_>>(), vec!["A"]);
    parser::canonical_program(&ast);
}

#[test]
fn shared_outside_an_implementation_is_a_syntax_error() {
    reject("shared type T = Int\n\ndef main = 1\n", "E100", "shared");
    reject("module Monoid\nshared type T\ndef of(value: String): T Pure\n", "E100", "shared");
}

#[test]
fn shared_def_is_a_syntax_error() {
    let source = format!("{MONOID}\n\nimplement Monoid as StringMonoid\nshared def f = 1\ndef combine left _ => left\ndef of value => value");
    reject(&source, "E100", "def f");
}

#[test]
fn shared_with_a_constructor_body_is_a_syntax_error() {
    let adt = format!("{MONOID}\n\nimplement Monoid as Monad\ntype T = Unit\ndef combine left _ => left\ndef of value => value\nshared type T = | A | B");
    reject(&adt, "E100", "| A");
    let record = format!("{MONOID}\n\nimplement Monoid as Monad\ntype T = Unit\ndef combine left _ => left\ndef of value => value\nshared type T = {{ x: Int }}");
    reject(&record, "E100", "{ x");
}

#[test]
fn shared_to_a_type_absent_from_the_contract_is_unbound() {
    let case = Case::new();
    let source = format!("def main = StringMonoid.of \"a\"\n\n{MONOID}\n\nimplement Monoid as StringMonoid\nshared type T = String\nshared type U = Int\ndef combine left _ => left\ndef of value => value");
    case.write("main.hd", &source);
    let error = case.check("main.hd").expect_err("shared to an unknown type must be E204");
    assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
    let (line, col) = location(&source, source.find("shared type U").unwrap() + "shared type ".len());
    assert_eq!((error.line, error.col), (line, col), "{error}");
}

#[test]
fn shared_to_a_known_type_does_not_trigger_unbound() {
    let case = Case::new();
    let source = format!("def main = StringMonoid.of \"a\"\n\n{MONOID}\n\n{IMPLEMENTATION}");
    case.write("main.hd", &source);
    let error = case.check("main.hd").expect_err("phase 0 leaves the abstract type opaque");
    assert_ne!(error.code.as_deref(), Some("E204"), "{error}");
}
