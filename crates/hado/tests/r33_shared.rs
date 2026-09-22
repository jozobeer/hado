use hado::check_source;
use hado::parser::{self, parse_source, Item, TypeDef};
use hado::types::Ty;

fn parse(source: &str) -> parser::Ast {
    parse_source(source, "generated.hd").unwrap_or_else(|error| panic!("{error}\n{source}"))
}

fn location(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    (
        before.matches('\n').count() + 1,
        before.rsplit('\n').next().unwrap().chars().count() + 1,
    )
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
            let offset = shifted
                .find(needle)
                .unwrap_or_else(|| panic!("needle {needle}\n{shifted}"));
            let error = parse_source(&shifted, "generated.hd").expect_err(&shifted);
            assert_eq!(error.code.as_deref(), Some(code), "{error}\n{shifted}");
            assert_eq!(
                (error.line, error.col),
                location(&shifted, offset),
                "{error}\n{shifted}"
            );
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

const MONOID: &str =
    "module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure";

#[test]
fn unmarked_realization_is_not_shared() {
    let source = "module Monoid\ntype T\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\n\nimplement Monoid as StringMonoid\ntype T = String\ndef combine left _ => left\ndef of value => value";
    let ast = parse(source);
    assert!(!type_def(&ast.items, "T").shared);
}

#[test]
fn shared_prefixes_a_parameterized_realization() {
    let source = "module Stack\ntype A T\ndef A push(stack: A T, value: A): A T\n\nimplement Stack as IntStack\nshared type A T = A List\ndef push stack value => [value, ...stack]";
    let ast = parse(source);
    let def = type_def(&ast.items, "T");
    assert!(def.shared);
    assert_eq!(
        def.params
            .iter()
            .map(|name| name.name.as_str())
            .collect::<Vec<_>>(),
        vec!["A"]
    );
    parser::canonical_program(&ast);
}

#[test]
fn shared_outside_an_implementation_is_a_syntax_error() {
    reject("shared type T = Int\n\ndef main = 1\n", "E100", "shared");
    reject(
        "module Monoid\nshared type T\ndef of(value: String): T Pure\n",
        "E100",
        "shared",
    );
}

#[test]
fn shared_def_is_a_syntax_error() {
    let source = format!("{MONOID}\n\nimplement Monoid as StringMonoid\nshared def f = 1\ndef combine left _ => left\ndef of value => value");
    reject(&source, "E100", "def f");
}

#[test]
fn shared_with_a_constructor_body_is_a_syntax_error() {
    let adt = format!("{MONOID}\n\nimplement Monoid as Monad\ntype T = Void\ndef combine left _ => left\ndef of value => value\nshared type T = | A | B");
    reject(&adt, "E100", "| A");
    let record = format!("{MONOID}\n\nimplement Monoid as Monad\ntype T = Void\ndef combine left _ => left\ndef of value => value\nshared type T = {{ x: Int }}");
    reject(&record, "E100", "{ x");
}

const UNSHARED: &str = "implement Monoid as StringMonoid\ntype T = String\ndef combine left _ => left\ndef of value => value";

// --- 工程 1: shared の等式が「解けた型への置き換え」になる ---

#[test]
fn unshared_implementation_keeps_the_abstract_type() {
    // 印なしの実体化は register_module_realization の 1 分岐で不透明に登録される。
    // 契約の型・局所型・値の消費経路（本体の適用・constructor payload・pattern・
    // field 射影・named constructor・関数型）を変えても同じ抽象型のまま検査が止まる。
    let applied = format!(
        "def main = StringMonoid.combine (StringMonoid.of \"a\") \"b\"\n{MONOID}\n\n{UNSHARED}"
    );
    let payload = "def main = Box(\"hi\")\ntype Box = | Box(Store.T)\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = String\ndef make name => name";
    let pattern = "def main = read (Box(Store.make \"hi\"))\ndef read | Box(value) => String.concat value \"!\"\ntype Box = | Box(Store.T)\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = String\ndef make name => name";
    let projection = "def main = String.concat ({ name = Store.make \"hi\" }).name \"!\"\ntype Box = { name: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = String\ndef make name => name";
    let named = format!("{NAMED}\ntype T = String\ndef make name => name");
    let function = "def main = read ({ value = Store.make \"hi\" })\ndef read box => box.value \"!\"\ntype Box = { value: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype T = Fn(String => String)\ndef make name => fn(suffix => String.concat name suffix)";
    let local_record = "def main = name (Store.make \"hi\")\ndef name (t: Store.T) => t.name\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\ntype Hidden = { name: String }\ntype T = Hidden\ndef make name => { name = name }";
    for (source, code, member) in [
        (applied.as_str(), "E305", Some("StringMonoid.T")),
        (payload, "E305", Some("Store.T")),
        (pattern, "E305", Some("Store.T")),
        (projection, "E305", Some("Store.T")),
        (named.as_str(), "E305", Some("Store.T")),
        (function, "E305", None),
        (local_record, "E601", None),
    ] {
        let error = check_source(source, "t")
            .err()
            .expect("an unmarked realization stays opaque");
        assert_eq!(error.code.as_deref(), Some(code), "{source}: {error}");
        if let Some(member) = member {
            assert!(
                error.message.contains(member),
                "{source}: {}",
                error.message
            );
        }
    }
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
    assert_eq!(
        check_source(&direct, "t").err().unwrap().code.as_deref(),
        Some("E602")
    );
    let mutual = "def main = StringMonoid.of \"a\"\nmodule Monoid\ntype T\ntype U\ndef combine(left: T, right: T): T Pure\ndef of(value: String): T Pure\n\nimplement Monoid as StringMonoid\nshared type T = U\nshared type U = T\ndef combine left _ => left\ndef of value => value";
    assert_eq!(
        check_source(mutual, "t").err().unwrap().code.as_deref(),
        Some("E602")
    );
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
fn shared_functor_application_type_resolves_to_the_argument() {
    let source = format!("def main = squash (StringFold.fold [\"a\", \"b\"])\ndef squash (x: StringFold.T): String => x\n{}", string_fold("shared "));
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn a_shared_functor_application_type_is_known_before_its_owner_is_processed() {
    // 適用結果 StringFold.T を参照する record schema は、適用 StringFold の処理より前に
    // 登録される。前処理が適用結果の等式を先に置かないと field が不透明のまま E305 に
    // なる。functor の契約 arity を署名から取って照合しないと、前処理の登録が落ちる。
    let source = format!(
        "def main = read ({{ value = StringFold.fold [\"a\"] }})\ndef read box => String.concat box.value \"!\"\ntype Box = {{ value: StringFold.T }}\n{FOLD_CONTRACTS}\n\n{STRING_MONOID}\n\n{}\nimplement Fold as StringFold = MakeFold(StringMonoid)",
        make_fold("shared "));
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn unmarked_functor_equation_keeps_the_application_type_distinct() {
    let source = format!("def main = StringFold.fold [\"a\"]\n{}", string_fold(""));
    let error = check_source(&source, "t")
        .err()
        .expect("an unmarked functor equation stays opaque");
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
    let error = check_source(&positive, "t")
        .err()
        .expect("returning the opaque argument type must be E705");
    assert_eq!(error.code.as_deref(), Some("E705"), "{error}");
    let negative = format!("def main = OpaqueFold.fold [\"a\"]\n{contracts}\n\n{opaque}\n\n{functor}\nimplement Fold as OpaqueFold = MakeFold(OpaqueMonoid)");
    let error = check_source(&negative, "t")
        .err()
        .expect("the argument's own type must still reject a String");
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
    let error = check_source(source, "t")
        .err()
        .expect("unmarked applications must keep distinct types");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn a_parameter_type_stays_opaque_inside_the_functor_body() {
    let source = "module Input\ntype T\ndef make(x: Int): T Pure\n\nmodule Output\ntype T\ndef run(x: Int): Int Pure\n\nimplement Input as Concrete\nshared type T = Int\ndef make x => x\n\nimplement Output as Make(Arg: Input)\nshared type T = Arg.T\ndef run x => Arg.make x + 1\n\nimplement Output as Applied = Make(Concrete)";
    let error = check_source(source, "t")
        .err()
        .expect("the parameter type must stay opaque in the body");
    assert_eq!(error.code.as_deref(), Some("E305"), "{error}");
}

#[test]
fn cyclic_shared_functor_applications_are_rejected() {
    let source = "def main = A.read (A.make 7)\nmodule Id\ntype T\ndef make(n: Int): T Pure\ndef read(value: T): Int Pure\n\nimplement Id as I\nshared type T = Int\ndef make n => n\ndef read value => value\n\nimplement Id as Make(Arg: Id)\nshared type T = Arg.T\ndef make n => Arg.make n\ndef read value => Arg.read value\n\nimplement Id as A = Make(B)\nimplement Id as B = Make(A)";
    let error = check_source(source, "t")
        .err()
        .expect("a shared application cycle must be rejected, not crash");
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
fn local_types_of_two_implementations_stay_distinct() {
    let source = "def main = A.read (B.make 7)\nmodule Store\ntype T\ndef make(id: Int): T Pure\ndef read(value: T): Int Pure\n\nimplement Store as A\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id\n\nimplement Store as B\ntype Hidden = | Wrap(Int)\nshared type T = Hidden\ndef make id => Wrap(id)\ndef read | Wrap(id) => id";
    let error = check_source(source, "t")
        .err()
        .expect("two implementations must keep distinct local types");
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

// --- 工程 1 の修正 2: パターン payload と field 射影への shared の展開 ---

#[test]
fn shared_equation_reaches_a_pattern_payload() {
    let source = "def main = read (Box(Store.make \"hi\"))\ndef read | Box(value) => String.concat value \"!\"\ntype Box = | Box(Store.T)\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\nshared type T = String\ndef make name => name";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

#[test]
fn shared_equation_reaches_a_field_projection() {
    let source = "def main = String.concat ({ name = Store.make \"hi\" }).name \"!\"\ntype Box = { name: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store\nshared type T = String\ndef make name => name";
    let checked = check_source(source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
}

// --- 工程 1 の修正 3: named constructor の構築とパターンへの shared の展開 ---

const NAMED: &str = "def main = read (Box { value = Store.make \"hi\" })\ndef read\n| Box { value } => String.concat value \"!\"\ntype Box = | Box { value: Store.T }\nmodule Store\ntype T\ndef make(name: String): T Pure\n\nimplement Store as Store";

#[test]
fn shared_equation_reaches_a_named_constructor_payload() {
    let source = format!("{NAMED}\nshared type T = String\ndef make name => name");
    let checked = check_source(&source, "t").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(checked.main, Ty::String);
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
    let error = check_source(source, "t")
        .err()
        .expect("a wider realization must be rejected, not crash");
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
}

#[test]
fn a_shared_realization_with_fewer_type_arguments_than_the_contract_is_rejected() {
    // 契約 type A T（型引数 1）に shared type T = Int（型引数 0）。実体化の方が少ない形。
    let source = "module Stack\ntype A T\ndef A empty: A T Pure\n\nimplement Stack as Stack\nshared type T = Int\ndef empty = 1";
    let error = check_source(source, "t")
        .err()
        .expect("a narrower realization must be rejected, not crash");
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
}

// --- 工程 1 の修正 8: 前処理が作る抽象型の arity と functor 適用の契約 ---

#[test]
fn a_shared_realization_whose_rhs_under_applies_an_abstract_type_is_rejected() {
    // 契約 Stack の T は型引数 1。右辺 Stack.T を型引数 0 で書いた等式を前処理が
    // 登録すると、`Ty::reveal` から `TypeRealization::apply` の個数 assert で panic する。
    // 登録せず、既存の経路（annotation_inner の arity 照合）が E204 で拒否すること。
    let source = "type Box = { value: Alias.T }\nmodule Stack\ntype A T\ndef A empty: A T Pure\nmodule Alias\ntype T\ndef touch: Void Pure\nimplement Stack as Stack\nshared type A T = A List\ndef empty = []\nimplement Alias as Alias\nshared type T = Stack.T\ndef touch = ()";
    let error = check_source(source, "t")
        .err()
        .expect("an under-applied rhs must be rejected, not crash");
    assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
}

#[test]
fn a_shared_realization_whose_rhs_over_applies_an_abstract_type_is_rejected() {
    // 右辺 (Int, String) Stack.T は型引数 2。Stack.T の arity 1 と食い違う。同じく
    // 登録せず、annotation_inner が型注釈として拒否すること（コード無しの型注釈エラー）。
    let source = "type Box = { value: Alias.T }\nmodule Stack\ntype A T\ndef A empty: A T Pure\nmodule Alias\ntype T\ndef touch: Void Pure\nimplement Stack as Stack\nshared type A T = A List\ndef empty = []\nimplement Alias as Alias\nshared type T = (Int, String) Stack.T\ndef touch = ()";
    let error = check_source(source, "t")
        .err()
        .expect("an over-applied rhs must be rejected, not crash");
    assert!(error.code.is_none(), "{error}");
}

#[test]
fn a_functor_shared_realization_whose_rhs_under_applies_a_parameter_is_rejected() {
    // functor 本文の `shared type T = M.T`。契約 Stack の T は型引数 1 だが M.T を
    // 型引数 0 で参照している。仮引数の契約の署名から arity を取り、食い違う等式を
    // 登録しないこと（登録すると apply の assert で panic する）。
    let source = "type Box = { value: Out.T }\nmodule Stack\ntype A T\ndef A empty: A T Pure\nmodule Alias\ntype T\ndef touch: Void Pure\nimplement Stack as Base\nshared type A T = A List\ndef empty = []\nimplement Alias as Make(M: Stack)\nshared type T = M.T\ndef touch = ()\nimplement Alias as Out = Make(Base)";
    let error = check_source(source, "t")
        .err()
        .expect("a parameter-dropping rhs must be rejected, not crash");
    assert_eq!(error.code.as_deref(), Some("E204"), "{error}");
}

#[test]
fn a_functor_application_with_a_mismatched_result_contract_is_rejected() {
    // functor Make の結果契約は Source。適用の見出しは Target。等式を登録する前に
    // 結果契約を照合しないと、Out.T の arity が契約間で食い違い、apply の assert で
    // panic する。登録せず application() が E602 を出すこと。
    let source = "type Box = { value: Int Out.T }\nmodule Source\ntype T\ndef make(x: String): T Pure\nmodule Target\ntype A T\ndef A empty: A T Pure\nimplement Source as Base\nshared type T = String\ndef make x => x\nimplement Source as Make(M: Source)\nshared type T = M.T\ndef make x => M.make x\nimplement Target as Out = Make(Base)";
    let error = check_source(source, "t")
        .err()
        .expect("a result contract mismatch must be E602, not a crash");
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
}

#[test]
fn a_functor_application_with_a_mismatched_argument_contract_is_rejected() {
    // 仮引数 M の契約は Source。引数 Base の契約は Target。等式を登録する前に
    // 各引数の契約を照合しないと、Out.T と Base.T の arity が食い違い panic する。
    let source = "type Box = { value: Out.T }\nmodule Source\ntype T\ndef make(x: String): T Pure\nmodule Target\ntype A T\ndef A empty: A T Pure\nimplement Target as Base\nshared type A T = A List\ndef empty = []\nimplement Source as Make(M: Source)\nshared type T = M.T\ndef make x => M.make x\nimplement Source as Out = Make(Base)";
    let error = check_source(source, "t")
        .err()
        .expect("an argument contract mismatch must be E602, not a crash");
    assert_eq!(error.code.as_deref(), Some("E602"), "{error}");
}
