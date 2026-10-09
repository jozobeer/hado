//! §47・§49・§55: `implement` の中の ADT と record はその実装だけの局所
//! 名前空間にある。prelude は利用者の根の前に連結されるので、prelude の実装は
//! 利用者の根の型を見ない。利用者が prelude の実装の内部型と同じ名前を根で
//! 宣言しても E205 にならない。

use hado::check_source;

/// 利用者が根で宣言した型が、prelude の実装が内部に置く型と同じ名前でも
/// E205 にならない。`T` は契約の抽象型の実体化、`MapColor` と `MapStep` は
/// 契約に載らない内部型である。
#[test]
fn prelude_implementation_types_do_not_conflict_with_root_types() {
    let source = "\
type T = { t: Int }
type MapColor = { color: Int }
type MapStep = { step: Int }
def useT (value: T) => value.t
def useColor (value: MapColor) => value.color
def useState (value: MapStep) => value.step
def main = useT { t = 1 } + useColor { color = 2 } + useState { step = 3 }
";
    let result = check_source(source, "main.hd");
    assert!(result.is_ok(), "{:?}", result.err());
}

/// §47・§49・E402: `implement` の本文の ADT はその実装だけの局所的な型である。
/// 根の `type T = | TA | TB` の constructor が prelude の実装の `type (K, V) T`
/// と同じ family にまとまると、`MapLeaf`・`MapNode` だけを覆う実装の腕が
/// 網羅でなくなり E402 になる。名前が重ならなければ E402 にならない（対照）。
#[test]
fn root_variant_types_do_not_mix_with_prelude_implementation_types() {
    let source = "\
type T = | TA | TB
type MapColor = | CA | CB
type MapStep = | SA | SB
def useT (value: T): Int => if value | TA => 1 | TB => 2
def useColor (value: MapColor): Int => if value | CA => 1 | CB => 2
def useState (value: MapStep): Int => if value | SA => 1 | SB => 2
def main = useT TA + useColor CA + useState SA
";
    let result = check_source(source, "main.hd");
    assert!(result.is_ok(), "{:?}", result.err());
}

/// §47・§49: `implement` の本文の名前付き constructor の field は、その本文の
/// 中の `Ty::Adt` の名前で引ける。網羅性の family を宣言元の綴り（`Store.T`）で
/// まとめても、field の schema の引き先は本文の型の名前のままにする（`tests/
/// accept/functor/main.hd` と同じ形。ここでは prelude の内部型と名前が重なら
/// ない形で測る）。family の綴りを schema の引き先に流用すると E305 になる。
#[test]
fn a_functor_named_field_type_keeps_its_field_schema() {
    let source = "\
module Store
type T
def create (name: String): T
def describe (user: T): String
/module
implement Store as StoreImpl
type T = | Payload { name: String }
def create name => Payload { name = name }
def describe user => user.name
/implement
def main = StoreImpl.describe (StoreImpl.create \"x\")
";
    let result = check_source(source, "main.hd");
    assert!(result.is_ok(), "{:?}", result.err());
}

/// §47: 検査は緩めない。利用者の実装が外の型と同名の ADT を持つときは、
/// いまも E205 である。
#[test]
fn a_user_implementation_type_still_conflicts_with_an_outer_type() {
    let source = "\
type Entry = { number: Int }
def main = M.run 1
module S
def run(x: Int): Int
/module
implement S as M
type Entry = | Entry(Int)
def run x => x + 1
/implement
";
    let error = check_source(source, "main.hd").err().expect("E205");
    assert_eq!(error.code.as_deref(), Some("E205"));
}
