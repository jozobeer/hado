use hado::parser::{self, parse_source};

fn spec() -> &'static str {
    include_str!("../../../docs/spec.md")
}

fn spec_between(start: &str, end: &str) -> &'static str {
    let text = spec();
    let from = text.find(start).unwrap_or_else(|| panic!("missing {start}"));
    let rest = &text[from..];
    let until = rest[start.len()..].find(end).map(|index| start.len() + index).unwrap_or(rest.len());
    &rest[..until]
}

fn spec_to_end(start: &str) -> &'static str {
    let text = spec();
    let from = text.find(start).unwrap_or_else(|| panic!("missing {start}"));
    &text[from..]
}

fn ocaml_blocks(section: &str) -> Vec<&str> {
    section
        .split("```ocaml")
        .skip(1)
        .map(|block| block.split("```").next().unwrap().trim())
        .filter(|block| !block.is_empty())
        .collect()
}

fn location(source: &str, offset: usize) -> (usize, usize) {
    let before = &source[..offset];
    (before.matches('\n').count() + 1, before.rsplit('\n').next().unwrap().chars().count() + 1)
}

fn shifted(source: &str, offset: usize, padding: usize) -> (String, usize) {
    (format!("{}{}{}", &source[..offset], " ".repeat(padding), &source[offset..]), offset + padding)
}

fn reject(source: &str, code: &str, offset: usize) {
    for padding in 0..8 {
        let (source, offset) = shifted(source, offset, padding);
        let error = parse_source(&source, "generated.hd").expect_err(&source);
        assert_eq!(error.code.as_deref(), Some(code), "{error}\n{source}");
        assert_eq!((error.line, error.col), location(&source, offset), "{error}\n{source}");
    }
}

fn abort_return_colon(source: &str) -> usize {
    let start = source.find("abort ").unwrap();
    let after = &source[start..];
    if let Some(close) = after.find(')') {
        start + close + after[close..].find(':').expect("abort return colon")
    } else {
        start + after.find(':').expect("abort return colon")
    }
}

fn roundtrip(source: &str) {
    let ast = parse_source(source, "generated.hd").unwrap_or_else(|error| panic!("{error}\n{source}"));
    let canonical = parser::canonical_program(&ast);
    let restored = parse_source(&canonical, "canonical.hd").unwrap_or_else(|error| panic!("{error}\n{source}\n{canonical}"));
    assert_eq!(canonical, parser::canonical_program(&restored), "{source}\n{canonical}");
}

fn spec_keywords() -> Vec<String> {
    let line = spec_to_end("## 付録C ")
        .lines()
        .find(|line| line.contains("キーワードは"))
        .expect("keyword sentence");
    line.split('`')
        .enumerate()
        .filter(|(index, _)| index % 2 == 1)
        .map(|(_, word)| word.to_string())
        .filter(|word| word.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic()))
        .collect()
}

#[test]
fn handler_type_var_before_a_namespaced_effect_parses() {
    parse_source("handler A ns::Yield as H", "generated.hd").unwrap();
}

#[test]
fn spec_effect_and_handler_examples_parse() {
    for block in ocaml_blocks(spec_between("## 37.", "## 38.")).into_iter()
        .chain(ocaml_blocks(spec_between("## 38.", "## 39.")))
    {
        parse_source(block, "spec-example.hd").unwrap_or_else(|error| panic!("{error}\n{block}"));
    }
}

#[test]
fn abort_return_type_is_e123_on_the_colon() {
    for source in [
        "def main = 1\neffect E\nabort stop: Unit",
        "def main = 1\neffect E\nabort raise(error: Exception): Unit",
    ] {
        reject(source, "E123", abort_return_colon(source));
    }
}

#[test]
fn resume_outside_again_is_e124_on_resume() {
    for source in [
        "def main = resume",
        "effect Ask\nonce ask: String\nhandler Ask as Fixed\nonce ask => resume",
        "effect Ask\nabort stop\nhandler Ask as Fixed\nabort stop => resume",
        "effect Ask\nonce ask: String\nhandler Ask as Fixed\nreturn value => resume",
    ] {
        reject(source, "E124", source.find("resume").unwrap());
    }
}

#[test]
fn again_arm_may_contain_resume() {
    for source in [
        "effect A Yield\nagain yield(value: A): Unit\nhandler A Yield as H\nagain yield value => resume",
        "effect A Yield\nagain yield(value: A): Unit\nhandler A Yield as H\nagain yield value => let rest = resume () -> rest",
        "effect A Yield\nagain yield(value: A): Unit\nhandler A Yield as H\nagain yield value => fn(x => resume)",
    ] {
        parse_source(source, "generated.hd").unwrap_or_else(|error| panic!("{error}\n{source}"));
    }
}

#[test]
fn once_and_again_may_write_a_result() {
    for source in [
        "effect Ask\nonce ask: String",
        "effect A Yield\nagain yield(value: A): Unit",
        "effect Ask\nonce ask",
        "effect E\nabort raise(error: Exception)",
    ] {
        parse_source(source, "generated.hd").unwrap_or_else(|error| panic!("{error}\n{source}"));
    }
}

#[test]
fn effect_rows_roundtrip_in_result_position() {
    for source in [
        "def f (x: Int): Int Pure => x",
        "def f (x: Int): Int {Io, Error} => x",
        "def f (x: Fn(A => B {E})): Int => 1",
        "module M\ndef run(x: Int): Int {Io}",
    ] {
        roundtrip(source);
    }
}

#[test]
fn keywords_cannot_be_names() {
    for keyword in spec_keywords() {
        let source = if keyword.starts_with(char::is_uppercase) {
            format!("type {keyword} = | Case")
        } else {
            format!("def {keyword} = 1")
        };
        let error = parse_source(&source, "generated.hd").expect_err(&source);
        assert_eq!(error.code.as_deref(), Some("E100"), "{keyword}: {error}");
    }
}

#[test]
fn with_is_not_an_arm_body() {
    let source = "def main = if true => with H -> 1 | 2";
    reject(source, "E100", source.find("with").unwrap());
}
