use hado::fixes::{fix_at, fixes, hint};

fn applied(source: &str, index: usize) -> String {
    let fix = &fixes(source)[index];
    format!(
        "{}{}{}",
        &source[..fix.start],
        fix.replacement,
        &source[fix.end..]
    )
}

#[test]
fn condition_arms_move_next_to_the_parameters() {
    let source = "def f x => if\n  | x > 0 => 1\n  | 0\ndef main = f 1\n";
    let list = fixes(source);
    assert_eq!(list.len(), 1, "{list:?}");
    let fix = &list[0];
    assert_eq!(fix.code, "E517");
    assert_eq!((fix.line, fix.col), (1, 1));
    assert!(
        fix.canonical.starts_with("def f x | ("),
        "{}",
        fix.canonical
    );
    let fixed = applied(source, 0);
    assert_eq!(fixed, "def f x\n  | x > 0 => 1\n  | 0\ndef main = f 1\n");
    // 直した形はもう冗長でない（適用は 1 回で止まる）。
    assert!(fixes(&fixed).is_empty(), "{:?}", fixes(&fixed));
}

#[test]
fn nested_application_inside_cons_keeps_call_boundaries() {
    // `(add 1) 2` は 2 回の適用。正準形が括弧を落とすと `add 1 2` になり、
    // 1 回の 2 引数呼び出しへ意味が変わる（§36）。E510 の書き換えはその
    // 正準形をソースへ書く。
    let source = "def add x => fn(y => x + y)\ndef main = Cons((add 1) 2, Nil)\n";
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == "E510" && fix.replacement.contains("add"))
        .expect("E510");
    assert_eq!(fix.replacement, "[(add 1) 2]");
    let (formatted, _) = hado::fixes::format(source)
        .expect("settled")
        .expect("rewritten");
    assert!(
        formatted.contains("(add 1) 2"),
        "fmt changed the nested application: {formatted}"
    );
    let restored = hado::parser::parse_source(&formatted, "<fmt>").expect("formatted parses");
    let main = restored.main().expect("main");
    let hado::parser::NodeKind::List(items, None) = &main.kind else {
        panic!("formatted main is not a list: {formatted}");
    };
    let [item] = items.as_slice() else {
        panic!("formatted list has {} items: {formatted}", items.len());
    };
    let hado::parser::NodeKind::Apply(callee, _) = &item.kind else {
        panic!("formatted element is not an application: {formatted}");
    };
    assert!(
        matches!(callee.kind, hado::parser::NodeKind::Apply(..)),
        "formatted program lost the nested application: {formatted}"
    );
}

#[test]
fn nested_application_in_a_boolean_arm_keeps_call_boundaries() {
    let source =
        "def add x => fn(y => x + y)\ndef main = if true | true => (add 1) 2 | false => 0\n";
    let fix = fixes(source)
        .into_iter()
        .find(|fix| fix.code == "E520")
        .expect("E520");
    assert!(fix.replacement.contains("(add 1) 2"), "{}", fix.replacement);
    assert!(!fix.replacement.contains("add 1 2"), "{}", fix.replacement);
}

#[test]
fn written_list_forms_become_literals() {
    let source = "def main = Cons(1, Cons(2, Nil))\n";
    let list = fixes(source);
    // 入れ子の編集は外側に飲まれる（重なる編集は最も外側だけが残る）。
    assert_eq!(list.len(), 1, "{list:?}");
    let fix = &list[0];
    assert_eq!((fix.code, fix.replacement.as_str()), ("E510", "[1, 2]"));
    let fixed = applied(source, 0);
    assert_eq!(fixed, "def main = [1, 2]\n");
    assert!(fixes(&fixed).is_empty());

    let pattern =
        "def f (l: Int List): Int =>\n  if l\n  | Cons(x, Nil) => x\n  | 0\ndef main = f [7]\n";
    let fix = fixes(pattern)
        .into_iter()
        .find(|fix| fix.code == "E516")
        .unwrap();
    assert_eq!((fix.replacement.as_str(), fix.line, fix.col), ("[x]", 3, 5));
    let fixed = applied(pattern, 0);
    assert!(fixed.contains("| [x] => x"), "{fixed}");
    assert!(fixes(&fixed).is_empty());
}

#[test]
fn a_comment_inside_the_range_is_never_deleted() {
    let source = "def main = Cons(1, // keep\n  Nil)\n";
    // 外側の書き換えはコメントを含むので出ない。報告されるアンカーも外側である。
    assert_eq!(fix_at(source, "E510", 1, 12), None);
    // 内側の `Nil` は別の位置なので編集は出るが、`--fix` は報告されたアンカー
    // と一致するものだけを当てるので、ファイルは変わらない（§57）。
    let inner = fixes(source);
    assert_eq!(inner.len(), 1, "{inner:?}");
    assert!(inner[0].start > source.find("//").unwrap());
}

#[test]
fn spans_map_through_multibyte_text() {
    let source = "// 日本語のコメント\ndef main = Cons(1, Nil)\n";
    let fix = fix_at(source, "E510", 2, 12).unwrap();
    assert_eq!(fix.replacement, "[1]");
    assert_eq!(hint(source, "E510", 2, 12).as_deref(), Some("[1]"));
    assert_eq!(fix_at(source, "E516", 2, 12), None);
    assert_eq!(fix_at(source, "E510", 1, 1), None);
}
