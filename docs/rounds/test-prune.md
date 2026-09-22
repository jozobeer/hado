# テストの重複・過剰分割を削る（test-prune）

開始 2026-09-22。着手時の main は `b5d9a29`。

## 手順書

### 背景

845 本のテストを精査し、実装の写し・モックの自己確認・型や標準ライブラリの保証の再確認・
他のテストやコーパスとの重複・内部依存・過剰なケース分割を洗い出した。判定は
keep 613 / delete 68 / merge 35 / shrink 129。delete の 68 本は別のエージェントが本文と
重複先を開いて再判定し、CONFIRMED 65 / REFUTED 3 だった。

本ラウンドは CONFIRMED 65 の削除、merge 35、shrink 129 を適用する。判定の基準は
「現実的な欠陥を見逃すか」で、テストの本数や網羅率は目的にしない。コーパス
（`tests/accept`・`tests/reject`・`stdlib/`）は変えない。

### 決定

- **決定 1: REFUTED 3 本は削らない。** `r25_effects.rs` の `let_question_requires_option`
  （重複先は `check_source` だけで、`let?` の check / build / dump-mono 経路が失われる）、
  `r35_array.rs` の `array_public_members_match_specification`（門番 `verify-m7.sh` は
  §55 の member 集合しか見ず、spec の署名ブロックと `stdlib/array.hd` の完全一致を
  代替しない）、`prefer_pipelines.rs` の
  `distributed_hado_sources_start_pipelines_from_applications`（実配布 `.hd` を製品
  parser に通しており mock-self ではない。R78/R79 の pipeline 規約を執行する唯一の箇所）。
- **決定 2: 統合は畳む先にケースを足してから元を消す。** 覆う欠陥の集合を減らさない。
- **決定 3: 縮小はケース数・反復回数・乗数だけを減らす。** 検査内容は変えない。
- **決定 4: `r28_effects.rs` が空になるので、門番 `verify-r28.sh` の 1 件を accept コーパスの
  実走に変える。** 28-a〜28-c の保証は `tests/accept/nested_unrelated_abort`・
  `nested_matching_abort`・`abort_arm_*` が持ち、corpus 判定が実走で照合する。空の
  `--test r28_effects` を回すと門番が緑のまま何も見なくなるため、緩めない。
- **決定 5: コーパスと `tests/corpus.sha256` は変えない。**

### 成果物

- `crates/hado/tests/` と `crates/hado/src/lexer.rs` の `#[cfg(test)]` から、
  delete 65・merge 35・shrink 129 を適用した差分。
- `scripts/verify-r28.sh`: 空になる `r28_effects` の代わりに accept コーパスを実走する 1 件。
- `docs/rounds/test-prune.md`: 本ファイル。

### 工程

1. 判定の適用（担当ファイルごとに分けて並行。整形と門番は統括が直列に回す）。
2. `cargo fmt --all`。
3. `cargo clippy --offline --all-targets -- -D warnings`。
4. `cargo test --offline`。
5. 触った門番、quick 層、full 層。
6. 本ファイルの実測を書いてコミットし、PR にする。

### 判定

- `cargo test --offline` が緑。
- `bash scripts/run-gates.sh --tier quick|core|full` が `red: none`。
- `bash scripts/lint-docs.sh` が緑。

### 許容される赤

なし。削除・統合・縮小は振る舞いを変えないので、全層が緑であるべきである。

### スコープ外

- 残した 613 本の書き換え。
- REFUTED 3 本が担っている保証の、門番への移植（`verify-r34.sh` の生成器検査、
  `verify-m7.sh` の spec 署名一致、`prefer_pipelines` の規約）。
- `verify-r28.sh` を `verify-m1-corpus.sh` へ統合する後片付け。

### 中断の条件

- 削除したテストの重複先が実際には覆っておらず、門番が赤になった場合は、その 1 本を
  戻し、他の担当を続けて報告する。
- 統合で畳む先のケースが元の検査を覆えない場合は、統合せずに残す。

### 適用した判定

REFUTED で残した 3 本は含まない。`（行）` は適用前の位置。

- `crates/hado/src/lexer.rs`
  - delete 1: `lex_e104_hash_comment`（692-696）
- `crates/hado/tests/check_adts.rs`
  - shrink 2: `recursive_type_cycles_preserve_parameters`（59-76）, `main_requires_a_monomorphic_type`（105-118）
- `crates/hado/tests/check_canonical.rs`
  - delete 7: `discarded_railway_binding_requires_do`（22-33）, `top_level_variable_patterns_are_rejected_but_nested_bindings_are_allowed`（35-44）, `written_list_constructors_require_list_syntax`（46-63）, `catch_all_underscore_requires_fallback_syntax`（237-250）, `plain_let_cannot_discard_a_value`（294-310）, `adt_branches_cover_constructor_alternatives`（439-462）, `recursive_list_coverage_terminates_and_finds_missing_lengths`（575-591）
  - merge 1: `identical_boolean_branches_are_rejected_structurally`（65-79）
- `crates/hado/tests/check_cli.rs`
  - merge 1: `every_reject_preserves_code_and_column_after_header_line_insertion`（268-321）
  - shrink 1: `dump_mono_exposes_typed_arithmetic`（174-193）
- `crates/hado/tests/check_dependencies.rs`
  - delete 1: `unused_top_level_definitions_are_rejected`（56-68）
- `crates/hado/tests/check_functions.rs`
  - delete 1: `function_values_cannot_be_compared_for_equality`（136-143）
  - shrink 2: `compare_rejects_function_values_and_containers`（185-194）, `main_cannot_return_a_function_or_function_container`（266-275）
- `crates/hado/tests/check_infer.rs`
  - shrink 1: `generated_type_round_trip_twenty`（76-103）
- `crates/hado/tests/check_modules.rs`
  - delete 2: `signature_matching_rejects_missing_and_mismatched_values`（23-30）, `unused_private_module_definition_is_rejected`（100-107）
  - merge 2: `checked_modules_retain_internal_constructor_tags_and_schemes`（127-135）, `internal_record_schemas_use_qualified_result_types`（137-157）
- `crates/hado/tests/check_recursion.rs`
  - delete 1: `mutual_cycles_reject_values_and_require_rec`（38-56）
  - shrink 2: `self_reference_requires_rec_but_shadowing_does_not`（14-23）, `type_changing_recursion_reports_poly_recursion`（58-78）
- `crates/hado/tests/check_rop.rs`
  - shrink 1: `do_of_a_pure_value_is_rejected`（24-42）
- `crates/hado/tests/emit_mono.rs`
  - delete 3: `mono_object_links_and_executes_recursive_calls`（92-109）, `tail_calls_survive_arity_changes_and_captured_indirection`（151-167）, `string_main_prints_utf8_and_newline`（453-461）
  - merge 1: `constructor_switch_values_flow_into_following_arithmetic`（196-210）
  - shrink 13: `closures_execute_direct_values_and_captured_environments`（111-138）, `integer_division_handles_zero_and_wrapping_boundary`（212-244）, `nested_literal_patterns_merge_values_without_reaching_impossible_paths`（246-264）, `string_equality_compares_contents_across_static_objects`（287-303）, `derived_string_comparison_executes_lexicographic_order`（305-326）, `recursive_adt_equality_compares_tags_and_payloads`（345-363）, `record_compare_uses_declaration_order`（365-379）, `adt_compare_uses_constructor_order_and_recursive_payloads`（381-401）, `char_main_encodes_unicode_scalars_as_utf8`（480-492）, `float_main_passes_f64_values_to_the_runtime`（494-505）, `integer_to_string_preserves_signed_boundaries`（507-527）, `float_to_string_preserves_numeric_values`（529-549）, `char_to_string_preserves_utf8`（551-563）
- `crates/hado/tests/lsp_hover_contract.rs`
  - merge 1: `hover_on_a_contract_opened_alone_prints_the_declared_type`（91-104）
- `crates/hado/tests/lsp_load_errors.rs`
  - delete 1: `an_excluded_document_is_not_blamed_for_the_roots_refusal`（216-237）
  - merge 2: `invalid_namespace_directory_is_published_instead_of_an_empty_list`（113-127）, `a_contract_opened_alone_sees_the_refusal_of_its_candidates`（129-144）
- `crates/hado/tests/lsp_protocol.rs`
  - merge 1: `units_count_scalars_in_each_encoding`（17-39）
- `crates/hado/tests/mono.rs`
  - merge 7: `mono_types_preserve_concrete_data_and_closure_signatures`（14-24）, `nested_nullary_patterns_check_both_tags`（629-639）, `deep_constructor_patterns_guard_payload_bindings`（662-673）, `divergent_function_values_keep_the_inferred_result_type`（767-773）, `scalar_compare_derivations_have_distinct_signatures`（865-885）, `dump_mono_includes_the_generated_main_printer`（926-931）, `parenthesized_constructor_calls_lower_with_payload_types`（963-971）
- `crates/hado/tests/named_record_constructors.rs`
  - shrink 1: `generated_values_print_in_declaration_order_and_can_be_read_back`（128-141）
- `crates/hado/tests/prelude.rs`
  - shrink 2: `scalar_prelude_wrappers_check_and_execute`（1-43）, `generic_prelude_operations_execute`（61-90）
- `crates/hado/tests/r11_comments.rs`
  - shrink 1: `generated_comment_markers_preserve_tokens_or_report_first_character`（3-30）
- `crates/hado/tests/r12_lists.rs`
  - shrink 4: `generated_lists_round_trip_with_canonical_patterns`（64-77）, `list_coverage_uses_lengths_and_nested_rest_patterns`（79-108）, `written_list_constructors_report_original_name_positions`（110-147）, `spread_expressions_execute_with_independently_computed_output`（195-220）
- `crates/hado/tests/r13_conditions.rs`
  - shrink 1: `generated_functions_reject_long_form_and_execute_short_form`（22-100）
- `crates/hado/tests/r14_rails.rs`
  - shrink 4: `generated_patterns_round_trip_with_an_independent_question_token`（22-44）, `generated_rail_errors_preserve_stage_and_pattern_anchors`（65-97）, `generated_chains_execute_success_and_each_failure_path`（131-164）, `generated_option_chains_propagate_none_at_each_step`（166-183）
- `crates/hado/tests/r16_sections.rs`
  - delete 1: `private_records_and_adts_are_scoped_per_implementation`（164-172）
  - shrink 3: `generated_contracts_and_implementations_round_trip_and_execute_in_any_order`（72-95）, `section_errors_preserve_namespace_and_declaration_anchors`（97-113）, `implementation_local_data_requires_a_reachable_use`（174-190）
- `crates/hado/tests/r17_files.rs`
  - shrink 6: `generated_programs_share_types_and_contracts_across_files`（66-116）, `duplicate_names_follow_entry_then_byte_path_order`（118-158）, `file_role_errors_keep_original_paths_and_positions`（160-230）, `interface_files_reject_implementation_types_and_cannot_be_entries`（282-304）, `lexical_errors_in_later_files_precede_earlier_syntax_errors`（324-337）, `implementation_diagnostics_retain_anchors_from_each_file`（339-384）
- `crates/hado/tests/r18_checks.rs`
  - merge 1: `boolean_forms_are_redundant_without_loading_the_prelude`（3-15）
- `crates/hado/tests/r18_spelling.rs`
  - shrink 5: `printable_main_types_match_independently_generated_types`（161-179）, `old_spellings_and_comparison_chains_reject_at_the_token`（210-242）, `lambda_arguments_and_arm_bodies_execute_without_consuming_arms`（244-259）, `boolean_alternative_forms_and_position_perturbations`（261-298）, `nested_boolean_patterns_and_direct_condition_functions_remain_valid`（300-308）
- `crates/hado/tests/r19_contracts.rs`
  - delete 1: `generated_field_oracles_match_concrete_runtime_controls`（404-413）
  - merge 1: `contract_errors_share_type_stage_and_precede_unused_type_variables`（388-402）
  - shrink 8: `generated_abstract_arities_and_value_members_execute`（161-174）, `contract_arity_mismatches_are_e602`（176-196）, `value_member_implementation_form_is_distinct_from_function_definition`（198-211）, `underscores_preserve_argument_positions_and_named_unused_arguments_reject`（221-237）, `contract_qualified_types_require_exactly_one_implementation`（278-294）, `unused_type_variables_use_type_references_not_constructor_names`（296-318）, `main_abstract_types_are_rejected_inside_fixed_and_generic_payloads`（320-343）, `parameterized_contract_references_keep_qualified_name_anchors`（415-424）
- `crates/hado/tests/r20_entries.rs`
  - merge 1: `single_entry_generator_control`（370-373）
  - shrink 2: `generated_entries_select_their_own_main_and_keep_other_entry_implementations_used`（162-169）, `file_isolation_preserves_parameter_let_lambda_and_pattern_shadowing`（285-305）
- `crates/hado/tests/r21_functors.rs`
  - delete 5: `minimal_application_is_accepted`（198-201）, `application_types_are_distinct_even_with_identical_representations`（202-217）, `both_distinct_application_types_can_be_consumed_by_their_own_members`（218-231）, `interface_files_cannot_contain_functor_definitions`（339-344）, `application_headers_cannot_have_bodies`（345-355）
  - shrink 5: `application_argument_requires_its_position_contract`（247-262）, `functor_body_must_satisfy_its_result_contract`（378-383）, `unused_application_is_rejected`（384-395）, `parameter_types_remain_opaque_even_when_actual_representations_support_the_operation`（418-452）, `diagnostic_types_use_source_parameter_names`（599-638）
- `crates/hado/tests/r22_namespaces.rs`
  - merge 1: `root_values_remain_file_local_even_when_modules_use_namespace_paths`（357-369）
  - shrink 6: `generated_directory_trees_distribute_types_contracts_and_implementations`（162-195）, `parent_and_sibling_contract_names_require_absolute_paths`（205-217）, `same_directory_duplicates_reject_the_second_declaration_in_each_space`（255-268）, `root_collisions_always_point_to_the_child_declaration_in_each_space`（269-284）, `parent_and_sibling_types_implementations_and_constructors_require_paths`（384-407）, `missing_namespace_segments_and_terminal_names_have_exact_independent_anchors`（484-523）
- `crates/hado/tests/r24_effects.rs`
  - delete 1: `higher_order_union_keeps_argument_row_open`（243-248）
  - shrink 1: `print_type_orders_effect_rows_and_writes_them_back`（119-159）
- `crates/hado/tests/r25_effects.rs`
  - delete 2: `do_requires_an_effect_or_option`（127-141）, `unhandled_raise_prints_message_on_stderr`（204-226）
  - shrink 4: `do_effect_row_is_checked_after_number_defaulting`（143-174）, `integer_division_is_int_with_error`（176-188）, `do_discards_an_error_effect`（190-202）, `integer_zero_divisor_aborts_with_division_by_zero`（228-248）
- `crates/hado/tests/r26_effects.rs`
  - delete 5: `print_type_still_shows_user_effects_on_main`（128-149）, `once_evaluates_the_handler_arm_in_place`（151-172）, `unhandled_abort_matches_result_stderr_and_status`（174-196）, `abort_handler_turns_raise_into_a_value`（198-224）, `handler_arm_can_call_a_top_level_function`（1624-1650）
  - merge 4: `abort_handler_skips_do_continuation_when_the_rhs_raises`（226-253）, `abort_handler_catches_division_in_a_callee_outside_an_unrelated_once_handler`（434-461）, `abort_handler_catches_raise_outside_an_unrelated_once_handler`（463-489）, `again_handler_outside_abort_handler_collects_callee_yields`（519-546）
  - shrink 18: `abort_handler_runs_do_continuation_when_the_rhs_returns`（255-282）, `abort_handler_catches_integer_division_by_zero`（284-312）, `abort_handler_catches_division_inside_arithmetic_and_calls`（314-341）, `abort_handler_skips_do_continuation_on_division_by_zero`（343-371）, `abort_handler_do_continuation_may_differ_from_the_rhs_success_type`（373-402）, `abort_handler_catches_division_outside_an_unrelated_once_handler`（404-432）, `abort_handler_outside_once_still_runs_the_once_body`（491-517）, `again_handler_outside_abort_handler_forwards_callee_raise`（548-582）, `once_tail_recursion_completes_ten_million_iterations`（1320-1367）, `generator_collects_yielded_values`（1437-1469）, `again_loop_counts_each_yield`（1471-1528）, `distinct_handlers_specialize_the_same_function_apart`（1530-1561）, `abort_in_a_branch_skips_the_return_arm`（1563-1589）, `abort_handler_return_arm_changes_the_with_type`（1591-1622）, `again_arm_keeps_pure_helpers_at_their_result_type`（1652-1681）, `return_arm_runs_when_types_already_match`（1683-1709）, `return_arm_does_not_rewrite_a_non_tail_branch`（1711-1737）, `return_arm_does_not_rewrite_a_pure_helper`（1739-1770）
- `crates/hado/tests/r28_effects.rs`
  - delete 6: `unrelated_inner_handler_does_not_catch_outer_abort`（65-78）, `matching_inner_handler_catches_the_abort`（80-93）, `with_inside_abort_arm_applies_its_return`（95-107）, `abort_arm_without_with_skips_the_return_arm`（109-120）, `abort_arm_can_call_an_effectful_function`（122-134）, `hashed_fail_tags_that_collided_stay_distinct`（156-165）
- `crates/hado/tests/r31_effects.rs`
  - delete 3: `non_unit_again_result_is_bound`（149-158）, `different_payload_types_compose`（166-178）, `different_payload_types_forward_through_resume`（180-192）
- `crates/hado/tests/r32_library.rs`
  - merge 1: `library_span_is_the_first_declaration`（93-97）
- `crates/hado/tests/r33_shared.rs`
  - delete 11: `shared_realization_is_recorded`（93-99）, `shared_survives_the_canonical_round_trip`（108-116）, `shared_to_a_type_absent_from_the_contract_is_unbound`（158-172）, `shared_to_a_known_type_is_accepted`（174-183）, `shared_equation_reaches_the_public_interface`（189-194）, `shared_equation_relaxes_e705`（212-221）, `unshared_realization_keeps_e705`（223-230）, `shared_record_realization_allows_field_access`（232-237）, `unshared_record_realization_rejects_field_access`（239-246）, `shared_functor_equation_reaches_the_application_result`（281-289）, `shared_functor_equation_follows_the_arguments_members`（291-299）
  - merge 6: `unshared_constructor_payload_stays_abstract`（395-403）, `unshared_local_record_rejects_field_access`（430-437）, `unshared_pattern_payload_stays_abstract`（448-456）, `unshared_field_projection_stays_abstract`（465-473）, `unshared_named_constructor_payload_stays_abstract`（486-494）, `unshared_function_realization_stays_abstract`（507-514）
- `crates/hado/tests/r34_bytes.rs`
  - delete 4: `bytes_type_round_trips_between_tys_and_mono`（5-9）, `utf8_case_count_is_seed_independent`（95-125）, `string_probe_classes_cover_all_utf8_widths`（127-210）, `string_generator_rejects_surrogates_before_generating_probes`（212-244）
- `crates/hado/tests/r45_columns.rs`
  - delete 2: `unary_coverage_is_unchanged`（90-103）, `column_bindings_are_visible_in_the_lowered_body`（352-371）
  - merge 1: `two_column_three_row_matrix_specializes_left_to_right`（296-313）
  - shrink 2: `wildcard_on_one_column_covers_that_constructor_product`（53-88）, `rewrap_repropagate_and_sole_parameter_stay_unary`（263-290）
- `crates/hado/tests/r9_coverage.rs`
  - delete 1: `variable_arm_is_not_decomposition_and_reconstruction`（67-81）
  - merge 1: `unresolved_receivers_keep_diagnostic_stage_and_position_order`（30-41）
  - shrink 6: `unresolved_field_receivers_report_e304_at_the_reference_start`（3-17）, `catch_all_patterns_are_exhaustive_before_redundancy_checks`（43-65）, `let_patterns_must_cover_the_entire_payload_type`（112-135）, `do_requires_a_rail_and_records_require_a_candidate`（137-159）, `record_patterns_resolve_the_whole_field_set`（192-204）, `discarded_rail_binding_points_to_the_underscore`（206-215）
- `crates/hado/tests/r9_module_types.rs`
  - shrink 4: `cyclic_abstract_aliases_report_signature_mismatch_at_the_module`（19-57）, `native_execution_uses_the_concrete_representation`（109-158）, `outer_data_can_reference_later_implementations_in_any_section_order`（217-241）, `native_data_payloads_use_module_abstract_representations`（276-326）
- `crates/hado/tests/r9_names.rs`
  - shrink 2: `diagnostic_numbers_follow_the_current_specification`（35-49）, `constructors_require_all_payloads_and_are_not_function_values`（51-84）
- `crates/hado/tests/r9_order.rs`
  - merge 1: `duplicate_signature_members_do_not_choose_a_public_type`（816-827）
  - shrink 9: `types_precede_redundancy`（86-92）, `remainder_is_int_only`（105-110）, `all_type_references_resolve_before_expression_type_errors`（173-192）, `unresolved_values_and_intrinsics_precede_type_errors`（228-242）, `recursive_type_errors_do_not_hide_earlier_independent_type_errors`（467-486）, `recursive_type_recovery_preserves_known_payloads_and_breaks_growing_cycles`（488-502）, `indirect_polymorphic_recursion_points_to_its_definition`（568-601）, `field_name_errors_are_ordered_with_unresolved_value_names`（645-666）, `independent_fields_precede_later_duplicate_declarations`（688-707）
- `crates/hado/tests/r9_print.rs`
  - shrink 2: `generated_lists_print_source_not_constructor_chains`（3-56）, `nested_text_is_quoted_but_outer_result_payload_is_raw`（76-144）
- `crates/hado/tests/r9_syntax.rs`
  - shrink 7: `generated_syntax_diagnostics`（50-75）, `whitespace_does_not_change_calls`（77-91）, `arm_functions_and_negative_patterns_parse`（93-105）, `type_definition_contexts_are_distinct`（120-137）, `identifier_namespaces_follow_the_grammar`（211-231）, `integer_field_receivers_remain_separate_from_float_tokens`（258-268）, `implementation_aliases_only_realize_declared_abstract_types`（270-293）
- `crates/hado/tests/r9_unused.rs`
  - shrink 3: `every_user_declaration_kind_is_checked_for_reachability`（3-32）, `unused_module_is_reported_even_when_its_signature_is_used`（48-56）, `definitions_and_modules_follow_dependencies_in_any_source_order`（58-85）
- `crates/hado/tests/repr.rs`
  - shrink 1: `string_literals_encode_byte_lengths_in_target_order`（153-171）
- `crates/hado/tests/unit/parser.rs`
  - delete 6: `defs_modules_types`（118-125）, `fn_type_group`（149-169）, `e102_rec_on_value`（283-287）, `e105_no_tuple`（289-293）, `e106_constructor_call_form`（304-308）, `e107_operator_overloading`（310-314）
  - merge 1: `parens_override_precedence`（46-49）

## 系譜

| 節 | 規範文の要約 | 段階 | コード | 固定するもの |
|---|---|---|---|---|
| （テスト運用） | テストは製品の欠陥を見逃さない範囲で最小に保つ | 工程 | 成功 | 本ファイルの実測 |
| （テスト運用） | 受理の振る舞いはコーパスが実走で照合する | 実行 | 成功 | `verify-m1-corpus.sh`・`tests/accept/*/main.hd` |
| （テスト運用） | 28-a〜28-c の入れ子ハンドラの保証は accept コーパスが持つ | 実行 | 成功 | `verify-r28.sh` |
| （テスト運用） | R78/R79 の pipeline 規約は `prefer_pipelines.rs` が執行する | 構文 | 成功 | `crates/hado/tests/prefer_pipelines.rs` |

## 実測

削除 65・統合 35・縮小 129 を適用した木で測る。テストの本数は 845 → 745
（delete 65 と merge 35 のぶん減り、REFUTED で残した 3 本は据え置き）。

```
$ cargo fmt --all
$ cargo clippy --offline --all-targets -- -D warnings; echo exit=$?
exit=0
$ cargo test --offline 2>&1 | grep -E '^test result:' | awk '{s+=$4; f+=$6} END {print "passed="s" failed="f}'
passed=745 failed=0
$ bash scripts/run-gates.sh --tier quick target/gates-quick 2>&1 | tail -1
TALLY: 38/38 passed; red: none
$ bash scripts/run-gates.sh --tier core target/gates-core 2>&1 | tail -1
TALLY: 602/602 passed; red: none
$ bash scripts/run-gates.sh --tier full target/gates-full 2>&1 | tail -1
TALLY: 3788/3788 passed; red: none
$ bash scripts/corpus-lock.sh --check; echo exit=$?
exit=0
```

静的照合: `#[test]` の実数が、期待（keep 613 + shrink 129 + REFUTED で残す 3 = 745）と
69 ファイルすべてで一致した。`r28_effects.rs` は全 6 テストを削ったのでファイルごと消え、
`verify-r28.sh` の 1 件は `cargo test --test corpus` に、`verify-r44.sh` の対象からは
`--test r28_effects` を外した。実装の途中で見つけた 2 点は次のとおり。

- `do_effect_row_is_checked_after_number_defaulting` の縮小が `do <int>` の E303 を
  check / build / dump-mono × 5 字下げで見る 1 ケースを落としていた。`do_requires_an_effect_or_option`
  の削除を検証役が CONFIRMED した根拠がこのケースだったので、1 ケースだけ戻した（8→1 の縮小は維持）。
- `for x in [1]` の形は clippy の `single-element-loop` で赤になる。横断で確認し、残りは無い。
