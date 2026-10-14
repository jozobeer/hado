//! `hado` のコマンドを子プロセスで起動して測るテストを 1 本の二進に束ねる。
//! 二進ごとのリンクを省くため、`tests/` のファイルは束ねた二進の module として組む
//! （`Cargo.toml` の `autotests = false`、docs/rounds/ci-shards.md）。
//! `CARGO_BIN_EXE_hado` を使うファイルはここに、使わないファイルは `tests/compiler.rs` に置く。

#[path = "build_cli.rs"]
mod build_cli;
#[path = "check_cli.rs"]
mod check_cli;
#[path = "check_infer.rs"]
mod check_infer;
#[path = "exit_code.rs"]
mod exit_code;
#[path = "fmt_layout.rs"]
mod fmt_layout;
#[path = "localize_message_positions.rs"]
mod localize_message_positions;
#[path = "lsp_hover_contract.rs"]
mod lsp_hover_contract;
#[path = "lsp_load_errors.rs"]
mod lsp_load_errors;
#[path = "named_record_constructors.rs"]
mod named_record_constructors;
#[path = "print_type_members.rs"]
mod print_type_members;
#[path = "r11_comments.rs"]
mod r11_comments;
#[path = "r12_lists.rs"]
mod r12_lists;
#[path = "r13_conditions.rs"]
mod r13_conditions;
#[path = "r14_rails.rs"]
mod r14_rails;
#[path = "r16_sections.rs"]
mod r16_sections;
#[path = "r17_files.rs"]
mod r17_files;
#[path = "r18_spelling.rs"]
mod r18_spelling;
#[path = "r19_contracts.rs"]
mod r19_contracts;
#[path = "r20_entries.rs"]
mod r20_entries;
#[path = "r21_functors.rs"]
mod r21_functors;
#[path = "r22_namespaces.rs"]
mod r22_namespaces;
#[path = "r24_effects.rs"]
mod r24_effects;
#[path = "r25_effects.rs"]
mod r25_effects;
#[path = "r26_effects.rs"]
mod r26_effects;
#[path = "r31_effects.rs"]
mod r31_effects;
#[path = "r32_library.rs"]
mod r32_library;
#[path = "r37_world.rs"]
mod r37_world;
#[path = "r9_module_types.rs"]
mod r9_module_types;
#[path = "r9_print.rs"]
mod r9_print;
#[path = "r9_unused.rs"]
mod r9_unused;
#[path = "repl.rs"]
mod repl;
#[path = "run_cli.rs"]
mod run_cli;
