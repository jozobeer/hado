use hado::parser::{self, parse_source, Diagnostic, Item, TypeDef};
use hado::types::Ty;
use hado::check_source;
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
fn shared_to_a_known_type_is_accepted() {
    let case = Case::new();
    let source = format!("def main = StringMonoid.of \"a\"\n\n{MONOID}\n\n{IMPLEMENTATION}");
    case.write("main.hd", &source);
    assert!(case.check("main.hd").is_ok(), "a shared realization must let main be typed");
}

const UNSHARED: &str = "implement Monoid as StringMonoid\ntype T = String\ndef combine left _ => left\ndef of value => value";

// --- 工程 1: shared の等式が「解けた型への置き換え」になる ---

#[test]
fn shared_equation_reaches_the_public_interface() {
    let source = format!("def main = StringMonoid.combine (StringMonoid.of \"a\") \"b\"\n{MONOID}\n\n{IMPLEMENTATION}");
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unshared_implementation_keeps_the_abstract_type() {
    let source = format!("def main = StringMonoid.combine (StringMonoid.of \"a\") \"b\"\n{MONOID}\n\n{UNSHARED}");
    let error = check_source(&source, "t").err().expect("an unmarked realization stays opaque");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert!(error.message.contains("StringMonoid.T"), "{}", error.message);
}

#[test]
fn shared_equation_relaxes_e705() {
    let source = "def main = Store.find 1\nmodule Store\ntype T\ndef find(id: Int): T Pure\n\nimplement Store as Store\nshared type T = Int\ndef find _ => 1";
    assert_eq!(check_source(source, "t").unwrap_or_else(|error| panic!("{error}")).main, Ty::Int);
}

#[test]
fn unshared_realization_keeps_e705() {
    let source = "def main = Store.find 1\nmodule Store\ntype T\ndef find(id: Int): T Pure\n\nimplement Store as Store\ntype T = Int\ndef find _ => 1";
    let error = check_source(source, "t").err().expect("an unmarked realization keeps E705");
    assert_eq!(error.code.as_deref(), Some("E705"), "{error}");
}

#[test]
fn shared_record_realization_allows_field_access() {
    let source = "def main = describe (UserStore.make \"n\")\ndef describe (t: UserStore.T) => t.name\ntype User = { name: String }\n\nmodule UserStore\ntype T\ndef make(name: String): T Pure\n\nimplement UserStore as UserStore\nshared type T = User\ndef make name => { name = name }";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unshared_record_realization_rejects_field_access() {
    let source = "def main = describe (UserStore.make \"n\")\ndef describe (t: UserStore.T) => t.name\ntype User = { name: String }\n\nmodule UserStore\ntype T\ndef make(name: String): T Pure\n\nimplement UserStore as UserStore\ntype T = User\ndef make name => { name = name }";
    let error = check_source(source, "t").err().expect("an unmarked realization keeps E601");
    assert_eq!(error.code.as_deref(), Some("E601"), "{error}");
}

#[test]
fn shared_parameterized_equation_transforms_arguments() {
    let source = "def main = Stack.push [1] 2\nmodule Stack\ntype A T\ndef A push(stack: A T, value: A): A T Pure\ndef A empty: A T Pure\n\nimplement Stack as Stack\nshared type A T = A List\ndef push stack value => [value, ...stack]\ndef empty = []";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::Adt("List".into(), vec![Ty::Int]));
}

#[test]
fn shared_realization_cycles_are_rejected() {
    let direct = format!("def main = StringMonoid.of \"a\"\n{MONOID}\n\nimplement Monoid as StringMonoid\nshared type T = T\ndef combine left _ => left\ndef of value => value");
    assert_eq!(check_source(&direct, "t").err().unwrap().code.as_deref(), Some("E602"));
    let mutual = "def main = StringMonoid.of \"a\"\nmodule Monoid\ntype T\ntype U\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\n\nimplement Monoid as StringMonoid\nshared type T = U\nshared type U = T\ndef combine left _ => left\ndef of value => value";
    assert_eq!(check_source(mutual, "t").err().unwrap().code.as_deref(), Some("E602"));
}

// --- 工程 2: functor の shared type T = M.T が適用結果に引き継がれる ---

const FOLD_CONTRACTS: &str = "module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\ndef empty: T Pure\ndef show(value: T): String Pure\n\nmodule Fold\ntype T\ndef fold(values: T List): T Pure";
const STRING_MONOID: &str = "implement Monoid as StringMonoid\nshared type T = String\ndef combine left right => String.concat left right\ndef of value => value\ndef empty = \"\"\ndef show value => value";

fn make_fold(shared: &str) -> String {
    format!("implement Fold as MakeFold(M: Monoid)\n{shared}type T = M.T\ndef fold values => List.fold values M.empty M.combine")
}
fn string_fold(shared: &str) -> String {
    format!("{FOLD_CONTRACTS}\n\n{STRING_MONOID}\n\n{}\nimplement Fold as StringFold = MakeFold(StringMonoid)", make_fold(shared))
}

#[test]
fn shared_functor_equation_reaches_the_application_result() {
    let source = format!("def main = StringFold.fold [\"a\", \"b\"]\n{}", string_fold("shared "));
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn shared_functor_equation_follows_the_arguments_members() {
    let source = format!("def main = StringFold.fold [StringMonoid.of \"a\", StringMonoid.of \"b\"]\n{}", string_fold("shared "));
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn shared_functor_application_type_resolves_to_the_argument() {
    let source = format!("def main = squash (StringFold.fold [\"a\", \"b\"])\ndef squash (x: StringFold.T): String => x\n{}", string_fold("shared "));
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unmarked_functor_equation_keeps_the_application_type_distinct() {
    let source = format!("def main = StringFold.fold [\"a\"]\n{}", string_fold(""));
    let error = check_source(&source, "t").err().expect("an unmarked functor equation stays opaque");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn a_shared_functor_follows_an_unmarked_argument() {
    let contracts = FOLD_CONTRACTS;
    let functor = make_fold("shared ");
    let opaque = "implement Monoid as OpaqueMonoid\ntype T = String\ndef combine left right => String.concat left right\ndef of value => value\ndef empty = \"\"\ndef show value => value";
    // 印なしの引数の T は不透明なので main からは返せない（E705）。fold が
    // OpaqueMonoid.T を受け取れていることは、E305 が出ないことで測る。
    let positive = format!("def main = OpaqueFold.fold [OpaqueMonoid.of \"a\"]\n{contracts}\n\n{opaque}\n\n{functor}\nimplement Fold as OpaqueFold = MakeFold(OpaqueMonoid)");
    let error = check_source(&positive, "t").err().expect("returning the opaque argument type must be E705");
    assert_eq!(error.code.as_deref(), Some("E705"), "{error}");
    let negative = format!("def main = OpaqueFold.fold [\"a\"]\n{contracts}\n\n{opaque}\n\n{functor}\nimplement Fold as OpaqueFold = MakeFold(OpaqueMonoid)");
    let error = check_source(&negative, "t").err().expect("the argument's own type must still reject a String");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn an_opaque_functor_result_matches_the_arguments_type() {
    // OpaqueFold.T と OpaqueMonoid.T が同一であることを、両者の間の注釈付き関数
    // squash を挟んでも通ることで測る。適用で等式を登録し落とすと、2 つの抽象型が
    // 揃わず squash の注釈で E305 になる。公開関数 show で String に戻して型を測る。
    let opaque = "implement Monoid as OpaqueMonoid\ntype T = String\ndef combine left right => String.concat left right\ndef of value => value\ndef empty = \"\"\ndef show value => value";
    let source = format!("def main = OpaqueMonoid.show (squash (OpaqueFold.fold [OpaqueMonoid.of \"a\"]))\ndef squash (x: OpaqueFold.T): OpaqueMonoid.T => x\n{}\n\n{opaque}\n\n{}\nimplement Fold as OpaqueFold = MakeFold(OpaqueMonoid)", FOLD_CONTRACTS, make_fold("shared "));
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn two_unmarked_applications_of_the_same_functor_are_distinct() {
    let source = "def main = A.read (B.make 7)\nmodule Id\ntype T\ndef make(x: Int): T Pure\ndef read(x: T): Int Pure\n\nmodule Box\ntype T\ndef make(x: Int): T Pure\ndef read(x: T): Int Pure\n\nimplement Id as I\ntype T = Int\ndef make x => x\ndef read x => x\n\nimplement Id as J\ntype T = Int\ndef make x => x\ndef read x => x\n\nimplement Box as Make(Arg: Id)\ntype T = Arg.T\ndef make x => Arg.make x\ndef read x => Arg.read x\n\nimplement Box as A = Make(I)\nimplement Box as B = Make(J)";
    let error = check_source(source, "t").err().expect("unmarked applications must keep distinct types");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn a_parameter_type_stays_opaque_inside_the_functor_body() {
    let source = "module Input\ntype T\ndef make(x: Int): T Pure\n\nmodule Output\ntype T\ndef run(x: Int): Int Pure\n\nimplement Input as Concrete\nshared type T = Int\ndef make x => x\n\nimplement Output as Make(Arg: Input)\nshared type T = Arg.T\ndef run x => Arg.make x + 1\n\nimplement Output as Applied = Make(Concrete)";
    let error = check_source(source, "t").err().expect("the parameter type must stay opaque in the body");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn cyclic_shared_functor_applications_are_rejected() {
    let source = "def main = A.read (A.make 7)\nmodule Id\ntype T\ndef make(n: Int): T Pure\ndef read(value: T): Int Pure\n\nimplement Id as I\nshared type T = Int\ndef make n => n\ndef read value => value\n\nimplement Id as Make(Arg: Id)\nshared type T = Arg.T\ndef make n => Arg.make n\ndef read value => Arg.read value\n\nimplement Id as A = Make(B)\nimplement Id as B = Make(A)";
    let error = check_source(source, "t").err().expect("a shared application cycle must be rejected, not crash");
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
}

// --- 工程 1 の修正: ADT payload・別実装の局所型・局所 record ---

#[test]
fn shared_equation_reaches_a_constructor_payload() {
    let source = "def main = Box(\"hi\")\ntype Box = | Box(Store.T)\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\nshared type T = String\ndef make name => name";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::Adt("Box".into(), vec![]));
}

#[test]
fn unshared_constructor_payload_stays_abstract() {
    let source = "def main = Box(\"hi\")\ntype Box = | Box(Store.T)\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = String\ndef make name => name";
    let error = check_source(source, "t").err().expect("an unmarked payload stays opaque");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert!(error.message.contains("Store.T"), "{}", error.message);
}

#[test]
fn local_types_of_two_implementations_stay_distinct() {
    let source = "def main = A.read (B.make 7)\nmodule Store\ntype T\ndef make(id: Int): T Pure\ndef read(value: T): Int Pure\n\nimplement Store as A\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id\n\nimplement Store as B\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id";
    let error = check_source(source, "t").err().expect("two implementations must keep distinct local types");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert!(error.message.contains("A.Hidden"), "{}", error.message);
    assert!(error.message.contains("B.Hidden"), "{}", error.message);
}

#[test]
fn a_local_type_can_be_used_through_its_own_implementation() {
    let source = "def main = A.read (A.make 7)\nmodule Store\ntype T\ndef make(id: Int): T Pure\ndef read(value: T): Int Pure\n\nimplement Store as A\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::Int);
}

#[test]
fn shared_local_record_allows_field_access() {
    let source = "def main = name (Store.make \"hi\")\ndef name (t: Store.T) => t.name\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype Hidden = { name: String }\nshared type T = Hidden\ndef make name => { name = name }";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unshared_local_record_rejects_field_access() {
    let source = "def main = name (Store.make \"hi\")\ndef name (t: Store.T) => t.name\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype Hidden = { name: String }\ntype T = Hidden\ndef make name => { name = name }";
    let error = check_source(source, "t").err().expect("an unmarked local record keeps E601");
    assert_eq!(error.code.as_deref(), Some("E601"), "{error}");
}

// --- 工程 1 の修正 2: パターン payload と field 射影への shared の展開 ---

#[test]
fn shared_equation_reaches_a_pattern_payload() {
    let source = "def main = read (Box(Store.make \"hi\"))\ndef read | Box(value) => String.concat value \"!\"\ntype Box = | Box(Store.T)\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\nshared type T = String\ndef make name => name";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unshared_pattern_payload_stays_abstract() {
    let source = "def main = read (Box(Store.make \"hi\"))\ndef read | Box(value) => String.concat value \"!\"\ntype Box = | Box(Store.T)\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = String\ndef make name => name";
    let error = check_source(source, "t").err().expect("an unmarked pattern payload stays opaque");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert!(error.message.contains("Store.T"), "{}", error.message);
}

#[test]
fn shared_equation_reaches_a_field_projection() {
    let source = "def main = String.concat ({ name = Store.make \"hi\" }).name \"!\"\ntype Box = { name: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\nshared type T = String\ndef make name => name";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unshared_field_projection_stays_abstract() {
    let source = "def main = String.concat ({ name = Store.make \"hi\" }).name \"!\"\ntype Box = { name: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = String\ndef make name => name";
    let error = check_source(source, "t").err().expect("an unmarked field projection stays opaque");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert!(error.message.contains("Store.T"), "{}", error.message);
}

// --- 工程 1 の修正 3: named constructor の構築とパターンへの shared の展開 ---

const NAMED: &str = "def main = read (Box { value = Store.make \"hi\" })\ndef read\n| Box { value } => String.concat value \"!\"\ntype Box = | Box { value: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store";

#[test]
fn shared_equation_reaches_a_named_constructor_payload() {
    let source = format!("{NAMED}\nshared type T = String\ndef make name => name");
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unshared_named_constructor_payload_stays_abstract() {
    let source = format!("{NAMED}\ntype T = String\ndef make name => name");
    let error = check_source(&source, "t").err().expect("an unmarked named payload stays opaque");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
    assert!(error.message.contains("Store.T"), "{}", error.message);
}

// --- 工程 1 の修正 5: 関数型を本体とする shared の等式 ---

#[test]
fn shared_function_realization_reaches_a_record_schema() {
    // record schema の field の型が関数型の Store.T のとき、record 登録より前に等式が
    // 解けていないと field は不透明のままになり、適用が E305 で落ちる。
    let source = "def main = read ({ value = Store.make \"hi\" })\ndef read box => box.value \"!\"\ntype Box = { value: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\nshared type T = Fn(String => String)\ndef make name => fn(suffix => String.concat name suffix)";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unshared_function_realization_stays_abstract() {
    let source = "def main = read ({ value = Store.make \"hi\" })\ndef read box => box.value \"!\"\ntype Box = { value: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = Fn(String => String)\ndef make name => fn(suffix => String.concat name suffix)";
    let error = check_source(source, "t").err().expect("an unmarked function realization stays opaque");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn shared_function_realization_resolves_an_effect_row() {
    let source = "def main = read ({ value = Store.make \"n\" })\ndef read box => box.value \"!\"\ntype Box = { value: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\nshared type T = Fn(String => String {Error})\ndef go (suffix: String): String {Error} => String.concat \"hi\" (parse suffix)\ndef parse (input: String): String {Error} => if input == \"\" => raise { message = \"empty\" } | input\ndef make _ => go";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

// --- 工程 1 の修正 7: 契約と実体化の型引数の個数の照合 ---

#[test]
fn a_shared_realization_with_more_type_arguments_than_the_contract_is_rejected() {
    // 契約 type T（型引数 0）に shared type A T = A List（型引数 1）。前処理が等式を
    // 登録すると reveal の apply の assert で panic する。E602 で拒否すること。
    let source = "def main = Store.make \"hi\"\nmodule Store\ntype T\ndef make(x: String): T Pure\n\nimplement Store as Store\nshared type A T = A List\ndef make x => [x]";
    let error = check_source(source, "t").err().expect("a wider realization must be rejected, not crash");
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
}

#[test]
fn a_shared_realization_with_fewer_type_arguments_than_the_contract_is_rejected() {
    // 契約 type A T（型引数 1）に shared type T = Int（型引数 0）。実体化の方が少ない形。
    let source = "module Stack\ntype A T\ndef A empty: A T Pure\n\nimplement Stack as Stack\nshared type T = Int\ndef empty = 1";
    let error = check_source(source, "t").err().expect("a narrower realization must be rejected, not crash");
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
}

#[test]
fn a_shared_realization_with_a_matching_type_argument_count_is_accepted() {
    let source = "def main = Stack.push [1] 2\nmodule Stack\ntype A T\ndef A push(stack: A T, value: A): A T Pure\ndef A empty: A T Pure\n\nimplement Stack as Stack\nshared type A T = A List\ndef push stack value => [value, ...stack]\ndef empty = []";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::Adt("List".into(), vec![Ty::Int]));
}

