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
