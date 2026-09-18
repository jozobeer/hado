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
