//! 処理系のライブラリを直接呼ぶテストを 1 本の二進に束ねる（`tests/command.rs` の対）。

#[path = "check_adts.rs"]
mod check_adts;
#[path = "check_branches.rs"]
mod check_branches;
#[path = "check_canonical.rs"]
mod check_canonical;
#[path = "check_dependencies.rs"]
mod check_dependencies;
#[path = "check_functions.rs"]
mod check_functions;
#[path = "check_modules.rs"]
mod check_modules;
#[path = "check_names.rs"]
mod check_names;
#[path = "check_records.rs"]
mod check_records;
#[path = "check_recursion.rs"]
mod check_recursion;
#[path = "check_rop.rs"]
mod check_rop;
#[path = "corpus.rs"]
mod corpus;
#[path = "emit_mono.rs"]
mod emit_mono;
#[path = "lsp_protocol.rs"]
mod lsp_protocol;
#[path = "map_internal_types.rs"]
mod map_internal_types;
#[path = "mono.rs"]
mod mono;
#[path = "named_record_order.rs"]
mod named_record_order;
#[path = "parse_corpus.rs"]
mod parse_corpus;
#[path = "postfix_close.rs"]
mod postfix_close;
#[path = "prefer_pipelines.rs"]
mod prefer_pipelines;
#[path = "prelude.rs"]
mod prelude;
#[path = "r18_checks.rs"]
mod r18_checks;
#[path = "r23_syntax.rs"]
mod r23_syntax;
#[path = "r33_shared.rs"]
mod r33_shared;
#[path = "r34_bytes.rs"]
mod r34_bytes;
#[path = "r35_array.rs"]
mod r35_array;
#[path = "r36_int.rs"]
mod r36_int;
#[path = "r45_columns.rs"]
mod r45_columns;
#[path = "r46_fixes.rs"]
mod r46_fixes;
#[path = "r47_fixes.rs"]
mod r47_fixes;
#[path = "r9_coverage.rs"]
mod r9_coverage;
#[path = "r9_grammar.rs"]
mod r9_grammar;
#[path = "r9_names.rs"]
mod r9_names;
#[path = "r9_order.rs"]
mod r9_order;
#[path = "r9_syntax.rs"]
mod r9_syntax;
#[path = "repr.rs"]
mod repr;
#[path = "runtime_abi.rs"]
mod runtime_abi;
#[path = "runtime_string.rs"]
mod runtime_string;
#[path = "tail.rs"]
mod tail;
#[path = "type_engine.rs"]
mod type_engine;
#[path = "walk.rs"]
mod walk;

/// `autotests = false` なので、どちらの束にも書かれていないファイルはテストとして走らない。
/// `tests/` 直下の `.rs` が 2 つの束のちょうど一方に書かれていることを見る。
#[test]
fn every_test_file_is_in_exactly_one_bundle() {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let bundles = ["command.rs", "compiler.rs"];
    let sources: Vec<String> = bundles
        .iter()
        .map(|b| std::fs::read_to_string(tests.join(b)).unwrap())
        .collect();
    let mut files: Vec<String> = std::fs::read_dir(&tests)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|f| f.ends_with(".rs") && !bundles.contains(&f.as_str()))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for file in &files {
        let line = format!("#[path = \"{file}\"]");
        let count: usize = sources.iter().map(|s| s.matches(&line).count()).sum();
        assert_eq!(count, 1, "{file} is declared in {count} bundles");
    }
}
