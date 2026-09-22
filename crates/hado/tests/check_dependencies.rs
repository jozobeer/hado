use hado::{names::definition_groups, parser::parse_source};

#[test]
fn dependencies_precede_users_and_cycles_stay_together() {
    let ast = parse_source("def main = f 1\ndef f x => g x\ndef g x => x", "t").unwrap();
    assert_eq!(
        definition_groups(&ast),
        vec![vec!["g"], vec!["f"], vec!["main"]]
    );
    let ast = parse_source(
        "def main = f 1\ndef rec f x => g x\ndef rec g x => f x",
        "t",
    )
    .unwrap();
    assert_eq!(definition_groups(&ast), vec![vec!["f", "g"], vec!["main"]]);
}

#[test]
fn local_names_do_not_create_definition_dependencies() {
    let ast = parse_source("def f g => g\ndef g f => f\ndef main = let f = 1 -> f", "t").unwrap();
    assert_eq!(
        definition_groups(&ast),
        vec![vec!["f"], vec!["g"], vec!["main"]]
    );
}

#[test]
fn dependencies_of_any_cycle_member_come_first() {
    let ast = parse_source(
        "def rec a x => b x\ndef rec b x => a (z x)\ndef z x => x",
        "t",
    )
    .unwrap();
    assert_eq!(definition_groups(&ast), vec![vec!["z"], vec!["a", "b"]]);
}

#[test]
fn unused_definitions_follow_reachable_dependencies() {
    let ast = hado::parser::parse_source(
        "def used x => helper x\ndef helper x => x\ndef dead x => x",
        "t",
    )
    .unwrap();
    assert_eq!(
        hado::names::unused_definitions(&ast, &["used".into()]),
        vec!["dead"]
    );
    let ast =
        hado::parser::parse_source("def used helper => helper\ndef helper x => x", "t").unwrap();
    assert_eq!(
        hado::names::unused_definitions(&ast, &["used".into()]),
        vec!["helper"]
    );
}
