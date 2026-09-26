use hado::fixes::{fix_at, fixes, hint};

fn applied(source: &str, code: &str) -> String {
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == code)
        .expect("no fix");
    format!(
        "{}{}{}",
        &source[..fix.start],
        fix.replacement,
        &source[fix.end..]
    )
}

#[test]
fn the_sole_parameter_moves_into_the_arms() {
    let source = "def f x =>\n  if x\n  | Some(v) => v\n  | 0\ndef main = f (Some(7))\n";
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == "E515")
        .unwrap();
    assert_eq!((fix.line, fix.col), (1, 1));
    assert_eq!(fix.replacement, "");
    assert_eq!(fix.canonical, "def f | Some(v) => v | 0");
    let fixed = applied(source, "E515");
    assert_eq!(
        fixed,
        "def f\n  | Some(v) => v\n  | 0\ndef main = f (Some(7))\n"
    );
    assert!(fixes(&fixed).is_empty(), "{:?}", fixes(&fixed));
    assert_eq!(
        hint(source, "E515", 1, 1).as_deref(),
        Some("def f | Some(v) => v | 0")
    );
}

#[test]
fn the_sole_parameter_keeps_rec_and_drops_for_other_forms() {
    let recursive = "def rec f x =>\n  if x\n  | Some(v) => v\n  | 0\ndef main = f (Some(7))\n";
    assert_eq!(
        fixes(recursive)
            .into_iter()
            .find(|fix| fix.code == "E515")
            .unwrap()
            .canonical,
        "def rec f | Some(v) => v | 0"
    );
    // 腕が引数を参照する形は E515 ではないので書き換えない。
    let used = "def f x =>\n  if x\n  | Some(v) => x\n  | 0\ndef main = 0\n";
    assert!(
        fixes(used).iter().all(|fix| fix.code != "E515"),
        "{:?}",
        fixes(used)
    );
    // 型注釈のある引数は対象外。
    let typed =
        "def f (x: Int Option) =>\n  if x\n  | Some(v) => v\n  | 0\ndef main = f (Some(7))\n";
    assert!(
        fixes(typed).iter().all(|fix| fix.code != "E515"),
        "{:?}",
        fixes(typed)
    );
}

#[test]
fn an_identity_binding_is_replaced_by_its_value() {
    // §41: 束縛は値の後ろに付くので、位置は本体の `x` を指す。
    let source = "def g n => n + 1\ndef main = g 4 as x ->\n  x";
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == "E501")
        .unwrap();
    assert_eq!((fix.line, fix.col), (2, 12));
    assert_eq!(fix.replacement, "g 4");
    assert_eq!(applied(source, "E501"), "def g n => n + 1\ndef main = g 4");

    let literal = "def main = 5 as x -> x\n";
    assert_eq!(applied(literal, "E501"), "def main = 5\n");
}

#[test]
fn an_identity_binding_nested_in_a_value_keeps_its_own_arrow() {
    // 値の式の中の束縛の `->` を、外側の束縛の分離点と取り違えない。
    let source = "def main = (3 as y -> y) as x -> x";
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == "E501")
        .unwrap();
    // 内側の `as y -> y` は値の一部なので、外側の束縛の綴りだけが落ちる。
    assert_eq!(fix.replacement, "3 as y -> y)");
}

#[test]
fn a_comment_inside_e501_ranges_is_not_deleted() {
    let branch = "def f x\n  // keep\n  => if x\n  | Some(v) => v\n  | 0\ndef main = f (Some(7))\n";
    assert_eq!(fix_at(branch, "E515", 1, 1), None);
    let binding = "def main = 5 // keep\n  as x -> x\n";
    assert_eq!(fix_at(binding, "E501", 1, 12), None);
}
