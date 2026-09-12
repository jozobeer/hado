use hado::parser::{parse_source, Item};

fn parse(source: &str) -> hado::parser::Ast {
    parse_source(source, "generated.hd").unwrap_or_else(|error| panic!("{error}\n{source}"))
}

#[test]
fn library_is_accepted_and_recorded() {
    let ast = parse("library");
    assert_eq!(ast.library.map(|span| (span.line, span.col)), Some((1, 1)));
}

#[test]
fn library_span_is_the_first_declaration() {
    let ast = parse("// header\n\nlibrary\n");
    assert_eq!(ast.library.map(|span| (span.line, span.col)), Some((3, 1)));
}

#[test]
fn library_may_be_followed_by_outer_items_and_sections() {
    for source in [
        "library\ndef main = 1",
        "library\ntype T = | Case",
        "library\nmodule M\ndef f(x: Int): Int",
        "library\neffect E\nabort fail",
    ] {
        assert!(parse(source).library.is_some(), "{source}");
    }
}

#[test]
fn repeated_library_is_left_to_the_assembler() {
    // §59: 同一ファイルの 2 つ目は名前解決の E205 であって、構文エラーではない。
    assert!(parse("library\nlibrary").library.is_some());
}

#[test]
fn def_main_is_still_an_item_next_to_library() {
    let ast = parse("library\ndef main = 1");
    assert!(ast.items.iter().any(|item| matches!(item, Item::Def(def) if def.name == "main")));
}

#[test]
fn library_and_shared_are_not_names() {
    for source in ["def library = 1", "def shared = 1", "type shared = | A"] {
        let error = parse_source(source, "generated.hd").expect_err(source);
        assert_eq!(error.code.as_deref(), Some("E100"), "{source}: {error}");
    }
}
