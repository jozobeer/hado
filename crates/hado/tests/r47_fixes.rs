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
    let source = "def g n => n + 1\ndef main =\n  let x = g 4 -> x\n";
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == "E501")
        .unwrap();
    assert_eq!((fix.line, fix.col), (3, 3));
    assert_eq!(fix.replacement, "g 4");
    assert_eq!(
        applied(source, "E501"),
        "def g n => n + 1\ndef main =\n  g 4\n"
    );

    let literal = "def main = let x = 5 -> x\n";
    assert_eq!(applied(literal, "E501"), "def main = 5\n");
}

#[test]
fn the_nested_let_separator_is_the_body_arrow() {
    // 値の式の `->` を分離点と取り違えない。
    let source = "def main =\n  let x = let y = 1 -> y -> x\n";
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == "E501")
        .unwrap();
    assert_eq!(fix.replacement, "let y = 1 -> y");
    let once = applied(source, "E501");
    assert_eq!(once, "def main =\n  let y = 1 -> y\n");
    assert_eq!(applied(&once, "E501"), "def main =\n  1\n");
}

#[test]
fn a_comment_inside_e515_and_e501_ranges_is_not_deleted() {
    let branch = "def f x\n  // keep\n  => if x\n  | Some(v) => v\n  | 0\ndef main = f (Some(7))\n";
    assert_eq!(fix_at(branch, "E515", 1, 1), None);
    let binding = "def main =\n  let x = 5 // keep\n    -> x\n";
    assert_eq!(fix_at(binding, "E501", 2, 3), None);
}
