mod functors;
mod named;
use crate::parser::{Ast, BinOp, DefKind, Diagnostic, Item, Node, NodeKind, Span};
use crate::parser::{
    DeclaredName, EffectRow, FieldName, HandlerArm, HandlerDecl, OpKind, Param, Pattern,
    PatternKind, TypeExpr, TypeKind,
};
use crate::types::{
    ComparisonConstraint, Inference, Row, RowConstraint, Scheme, Ty, TypeError, TypeRealization,
};
use crate::Checked;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct Checker<'a> {
    functors: HashMap<String, functors::Template>,
    source_modules: HashMap<String, crate::parser::ModuleImpl>,
    instantiated_modules: Vec<crate::parser::ModuleImpl>,
    next_instance_line: usize,
    contract_types: crate::contract_types::Contracts,
    aliases: HashMap<String, (Vec<crate::parser::DeclaredName>, TypeExpr, Span)>,
    ambiguous_modules: HashSet<String>,
    ambiguous_signatures: HashSet<String>,
    ambiguous_types: HashSet<String>,
    ambiguous_constructors: HashSet<String>,
    abstract_representations: HashMap<String, TypeRealization>,
    shared_realizations: HashMap<String, TypeRealization>,
    deferred: Vec<Diagnostic>,
    module_constructors: HashMap<String, (u32, Scheme)>,
    module_named_constructors: HashMap<String, Vec<String>>,
    named_constructors: HashMap<String, Vec<String>>,
    module_records: HashMap<String, (Vec<String>, Scheme)>,
    trusted_start: Option<usize>,
    module_path: Vec<String>,
    module_definitions: HashMap<String, Scheme>,
    composite_types: HashMap<(usize, usize), Ty>,
    value_types: HashMap<(usize, usize), Ty>,
    /// 各 `with` の本体と handler の腕を推論している間に出た節点の型を、
    /// `with` の位置ごとに控える（`crate::WithTypes`）。同じ節点が答え型の
    /// 違う複数の `with` の下に現れても、mono が `with` ごとの型で腕を
    /// lower できるようにする。
    with_types: HashMap<(usize, usize), crate::WithTypes>,
    active_withs: Vec<(usize, usize)>,
    path: &'a str,
    current_definition: Option<String>,
    inference: Inference,
    values: HashMap<String, Scheme>,
    numeric_constraints: Vec<(Ty, Span)>,
    divisions: Vec<(BinOp, Ty, Ty, Span, Row)>,
    type_arities: HashMap<String, usize>,
    constructor_tags: HashMap<String, u32>,
    constructors: HashMap<String, Scheme>,
    records: HashMap<String, (Vec<String>, Scheme)>,
    modules: HashMap<String, HashMap<String, Scheme>>,
    contracts: HashMap<String, HashMap<String, Scheme>>,
    abstract_types: HashMap<String, (usize, String)>,
    projections: Vec<(Ty, FieldName, Ty, Span)>,
    comparison_constraints: Vec<ComparisonConstraint>,
    row_constraints: Vec<RowConstraint>,
    equality_anchors: HashMap<(usize, usize, usize, usize), Span>,
    recursive_calls: HashMap<u32, Vec<(Vec<Ty>, Span)>>,
    eval_effects: Row,
    main_effects: Row,
    effects: HashMap<String, Vec<(OpKind, String)>>,
    prelude_effects: HashSet<String>,
    handlers: HashMap<String, String>,
    handler_decls: HashMap<String, HandlerDecl>,
    operations: HashMap<String, OpKind>,
    default_operations: HashMap<String, String>,
    operation_result_rows: HashMap<String, Row>,
    resume_type: Option<Ty>,
}

pub fn check_ast(ast: &Ast, path: &str) -> Result<Checked, Diagnostic> {
    check_ast_with_prelude(ast, path, None)
}

pub(crate) fn check_ast_with_prelude(
    ast: &Ast,
    path: &str,
    trusted_start: Option<usize>,
) -> Result<Checked, Diagnostic> {
    let inspection = if crate::has_main_definition(ast) {
        crate::Inspection::Entry
    } else {
        crate::Inspection::Library
    };
    let (checked, deferred) = infer_with_diagnostics(ast, path, trusted_start, inspection, 0)?;
    match deferred.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(checked),
    }
}

pub(crate) fn infer_with_diagnostics(
    ast: &Ast,
    path: &str,
    trusted_start: Option<usize>,
    inspection: crate::Inspection,
    prelude_items: usize,
) -> Result<(Checked, Vec<Diagnostic>), Diagnostic> {
    let contracts = crate::contract_types::Contracts::collect(&ast.items);
    let contract_diagnostics = contracts.diagnostics(ast, path);
    let mut checker = Checker {
        functors: HashMap::new(),
        source_modules: ast
            .items
            .iter()
            .filter_map(|item| match item {
                Item::ModuleImpl(module) => Some((module.name.clone(), module.clone())),
                _ => None,
            })
            .collect(),
        instantiated_modules: Vec::new(),
        next_instance_line: crate::functor_instances::max_line(ast) + 1,
        contract_types: contracts,
        aliases: HashMap::new(),
        ambiguous_modules: HashSet::new(),
        ambiguous_signatures: HashSet::new(),
        ambiguous_types: HashSet::new(),
        ambiguous_constructors: HashSet::new(),
        abstract_representations: HashMap::new(),
        shared_realizations: HashMap::new(),
        deferred: contract_diagnostics,
        module_constructors: HashMap::new(),
        module_records: HashMap::new(),
        module_named_constructors: HashMap::new(),
        named_constructors: HashMap::new(),
        trusted_start,
        module_path: Vec::new(),
        module_definitions: HashMap::new(),
        composite_types: HashMap::new(),
        value_types: HashMap::new(),
        with_types: HashMap::new(),
        active_withs: Vec::new(),
        path,
        current_definition: None,
        inference: Inference::default(),
        values: HashMap::new(),
        numeric_constraints: Vec::new(),
        divisions: Vec::new(),
        type_arities: HashMap::from([(crate::mono::ARRAY.into(), 1)]),
        constructor_tags: HashMap::new(),
        constructors: HashMap::new(),
        records: HashMap::new(),
        modules: HashMap::new(),
        contracts: HashMap::new(),
        abstract_types: HashMap::new(),
        projections: Vec::new(),
        comparison_constraints: Vec::new(),
        row_constraints: Vec::new(),
        equality_anchors: ast
            .equality_anchors
            .iter()
            .map(|(left, right, operator)| {
                ((left.line, left.col, right.line, right.col), *operator)
            })
            .collect(),
        recursive_calls: HashMap::new(),
        eval_effects: Row::pure(),
        main_effects: Row::pure(),
        effects: HashMap::new(),
        prelude_effects: ast.items[..prelude_items]
            .iter()
            .filter_map(|item| match item {
                Item::Effect(effect) => Some(effect_print_name(&effect.name)),
                _ => None,
            })
            .collect(),
        handlers: HashMap::new(),
        handler_decls: HashMap::new(),
        operations: HashMap::new(),
        default_operations: HashMap::new(),
        operation_result_rows: HashMap::new(),
        resume_type: None,
    };
    checker
        .deferred
        .extend(functors::duplicate_applications(ast, path));
    let mut definitions = HashMap::new();
    let inferred = checker.infer_program(ast, &mut definitions, inspection, prelude_items);
    let main = match inferred {
        Ok(main) => main,
        Err(error) => {
            checker.deferred.push(error);
            return Err(checker
                .deferred
                .into_iter()
                .min_by_key(crate::diagnostic_order)
                .unwrap());
        }
    };
    Ok((
        Checked {
            instantiated_modules: checker.instantiated_modules,
            abstract_representations: checker.abstract_representations,
            module_constructors: checker.module_constructors,
            module_records: checker.module_records,
            module_named_constructors: checker.module_named_constructors,
            named_constructors: checker.named_constructors.clone(),
            module_definitions: checker.module_definitions.clone(),
            modules: checker.modules.clone(),
            contracts: checker.contracts.clone(),
            constructor_tags: checker.constructor_tags.clone(),
            constructors: checker.constructors.clone(),
            records: checker.records.clone(),
            composite_types: checker
                .composite_types
                .iter()
                .map(|(span, ty)| (*span, checker.inference.resolve(ty)))
                .collect(),
            value_types: checker
                .value_types
                .iter()
                .map(|(span, ty)| (*span, checker.inference.resolve(ty)))
                .collect(),
            with_types: checker
                .with_types
                .iter()
                .map(|(span, types)| {
                    let resolve = |map: &HashMap<(usize, usize), Ty>| {
                        map.iter()
                            .map(|(span, ty)| (*span, checker.inference.resolve(ty)))
                            .collect()
                    };
                    (
                        *span,
                        crate::WithTypes {
                            values: resolve(&types.values),
                            composites: resolve(&types.composites),
                        },
                    )
                })
                .collect(),
            main: checker.inference.resolve(&main),
            effects: checker.inference.resolve_row(&checker.main_effects),
            definitions: definitions
                .keys()
                .map(|name| (name.clone(), checker.values[name].clone()))
                .collect(),
            operations: checker.operations.clone(),
            default_operations: checker.default_operations.clone(),
        },
        checker.deferred,
    ))
}

/// モジュールシグネチャを名前で引ける表に集める。
fn collect_signatures(items: &[Item]) -> HashMap<String, &crate::parser::ModuleSig> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::ModuleSig(sig) => Some((sig.name.clone(), sig)),
            _ => None,
        })
        .collect()
}

/// シグネチャの型定義に対応する実装側の定義を、名前と引数の個数で引く。
fn find_actual_type<'a>(
    items: &'a [Item],
    def: &crate::parser::TypeDef,
) -> Option<&'a crate::parser::TypeDef> {
    items.iter().find_map(|item| match item {
        Item::TypeDef(actual)
            if actual.name == def.name && actual.params.len() == def.params.len() =>
        {
            Some(actual)
        }
        _ => None,
    })
}

impl Checker<'_> {
    fn install_compare(&mut self) {
        let comparable = self.inference.fresh();
        let compare = Ty::fn_of(vec![comparable.clone(), comparable.clone()], Ty::Int);
        self.values.insert(
            "compare".into(),
            self.inference.generalize_constrained(
                &compare,
                &[],
                &[ComparisonConstraint {
                    ty: comparable,
                    code: "E704",
                    span: None,
                }],
                &[],
            ),
        );
    }

    fn register_type_arities(&mut self, items: &[Item]) {
        for item in items {
            if let Item::TypeDef(def) = item {
                if !self.ambiguous_types.contains(&def.name) {
                    self.type_arities.insert(def.name.clone(), def.params.len());
                }
            }
        }
    }

    fn register_signature_abstract_parameters(
        &mut self,
        items: &[Item],
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) {
        for item in items {
            if let Item::ModuleImpl(module) = item {
                if !matches!(module.form, crate::parser::ImplementationForm::Functor(_)) {
                    if let Some(signature) = signatures.get(&module.sig) {
                        self.module_abstract_parameters(module, signature);
                    }
                }
            }
        }
    }

    fn register_types(&mut self, ast: &Ast) -> Result<(), Diagnostic> {
        let violations = crate::type_cycles::violations(ast, self.path);
        for item in &ast.items {
            if let Item::TypeDef(def) = item {
                let rejected: Vec<_> = violations
                    .iter()
                    .filter(|violation| violation.definition == def.name)
                    .map(|violation| violation.reference)
                    .collect();
                self.register_type(def, &rejected)?;
            }
        }
        self.deferred
            .extend(violations.into_iter().map(|violation| violation.diagnostic));
        Ok(())
    }

    fn collect_definitions<'a>(
        &self,
        ast: &'a Ast,
        definitions: &mut HashMap<String, &'a crate::parser::Def>,
    ) -> Result<(), Diagnostic> {
        for item in &ast.items {
            match item {
                Item::Def(def) => {
                    definitions.entry(def.name.clone()).or_insert(def);
                }
                Item::TypeDef(_)
                | Item::ModuleSig(_)
                | Item::ModuleImpl(_)
                | Item::Effect(_)
                | Item::Handler(_) => {}
                _ => return Err(self.error(ast.span, None, "item inference is not implemented")),
            }
        }
        Ok(())
    }

    fn infer_definition_groups(
        &mut self,
        ast: &Ast,
        definitions: &HashMap<String, &crate::parser::Def>,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
        ambiguous_values: &HashSet<String>,
    ) -> Result<(), Diagnostic> {
        for group in crate::dependencies::inference_groups(ast) {
            let mut functions = Vec::new();
            for item in group {
                match item {
                    Item::ModuleImpl(module) => self.module(module, signatures)?,
                    Item::Def(def) if !ambiguous_values.contains(&def.name) => {
                        functions.push(def.name.clone())
                    }
                    _ => {}
                }
            }
            if !functions.is_empty() {
                self.definition_group(&functions, definitions)?;
            }
        }
        Ok(())
    }

    fn check_comparison_constraints(&mut self) {
        for constraint in &self.comparison_constraints {
            if self.contains_function(&self.inference.resolve(&constraint.ty), &mut Vec::new()) {
                self.deferred.push(self.error(
                    constraint.span.expect("comparison use has an origin"),
                    Some(constraint.code),
                    "values containing functions cannot be compared",
                ));
            }
        }
    }

    fn check_row_constraints(&mut self) {
        for constraint in &self.row_constraints {
            let violated = if constraint.code == "E303" {
                !self
                    .inference
                    .resolve_row(&constraint.row)
                    .has_known_effects()
            } else {
                !self
                    .inference
                    .row_within(&constraint.row, &constraint.bound)
            };
            if !violated {
                continue;
            }
            let message = match constraint.code {
                "E308" => "effects are not allowed in branch conditions",
                "E303" => "`e ->` requires an effect",
                _ => "effect row mismatch",
            };
            self.deferred.push(self.error(
                constraint.span.expect("row constraint has an origin"),
                Some(constraint.code),
                message,
            ));
        }
    }

    fn finish_main(
        &mut self,
        ast: &Ast,
        definitions: &HashMap<String, &crate::parser::Def>,
        inspection: crate::Inspection,
    ) -> Result<Ty, Diagnostic> {
        // A library has no entry: skip the `main` type rules (E206 stays in
        // `entry_form`, and E701/E702/E705/E706 need a `main` to apply).
        if inspection == crate::Inspection::Library {
            return Ok(Ty::Void);
        }
        let scheme = self
            .values
            .get("main")
            .cloned()
            .ok_or_else(|| self.error(ast.span, Some("E206"), "main is not defined"))?;
        let main = self.inference.instantiate(&scheme);
        if let Ty::Var(_) = self.inference.resolve(&main) {
            // raise's result is polymorphic; leaving it open makes main E701.
            let _ = self.inference.unify(&main, &Ty::Void);
        }
        self.inference.close_rows(&main);
        self.main_effects = self.inference.close_row(&self.main_effects);
        let main = self.reveal_shared(&self.inference.resolve(&main));
        self.check_main_type(definitions, &main);
        Ok(main)
    }

    fn check_main_type(&mut self, definitions: &HashMap<String, &crate::parser::Def>, main: &Ty) {
        if !main.is_monomorphic() {
            self.deferred.push(self.error(
                definitions["main"].span,
                Some("E701"),
                "main must have a monomorphic type",
            ));
        }
        if self.contains_function(main, &mut Vec::new()) {
            self.deferred.push(self.error(
                definitions["main"].span,
                Some("E702"),
                "main cannot return a value containing a function",
            ));
        }
        if self.contains_abstract(main, &mut Vec::new()) {
            self.deferred.push(self.error(
                definitions["main"].span,
                Some("E705"),
                "main cannot return a value containing an abstract type",
            ));
        }
        if self.main_effects.has_user_effect(&self.prelude_effects) {
            self.deferred.push(self.error(
                definitions["main"].span,
                Some("E706"),
                "main cannot have a user-defined effect",
            ));
        }
    }

    fn infer_program<'a>(
        &mut self,
        ast: &'a Ast,
        definitions: &mut HashMap<String, &'a crate::parser::Def>,
        inspection: crate::Inspection,
        prelude_items: usize,
    ) -> Result<Ty, Diagnostic> {
        self.install_compare();
        let ambiguous_values = self.prepare_ambiguous_declarations(&ast.items);
        self.register_type_arities(&ast.items);
        let signatures = collect_signatures(&ast.items);
        self.register_signature_abstract_parameters(&ast.items, &signatures);
        // 等式は型と効果の登録より前に集める。登録時に record schema と効果の操作型を
        // 解けた型で作るため（26 節「等式の収集は型と効果の登録より前に置く」）。
        self.collect_shared_realizations(ast);
        self.register_types(ast)?;
        self.collect_definitions(ast, &mut *definitions)?;
        self.register_effects(&ast.items, prelude_items)?;
        self.infer_definition_groups(ast, definitions, &signatures, &ambiguous_values)?;
        self.check_operation_defaults(&ast.items[..prelude_items])?;
        self.check_ambiguous_definitions(&ast.items, &ambiguous_values);
        self.check_ambiguous_modules(&ast.items, &signatures);
        self.resolve_projections(true)?;
        self.check_realization_cycles(&ast.items);
        self.check_comparison_constraints();
        self.check_row_constraints();
        self.collect_contract_exports(&signatures);
        self.finish_main(ast, definitions, inspection)
    }

    fn prepare_ambiguous_declarations(&mut self, items: &[Item]) -> HashSet<String> {
        self.prepare_module_ambiguities(items);
        let ambiguous =
            crate::ambiguous_declarations::AmbiguousDeclarations::collect(items, self.path);
        for name in &ambiguous.declared_types {
            self.ambiguous_types.remove(name);
            self.type_arities.remove(name);
            self.aliases.remove(name);
            self.records.remove(name);
        }
        self.hide_declared_constructors(&ambiguous.declared_constructors);
        self.ambiguous_types.extend(ambiguous.types);
        self.ambiguous_constructors.extend(ambiguous.constructors);
        for name in &ambiguous.values {
            self.values
                .insert(name.clone(), Scheme::monomorphic(Ty::Error));
        }
        for name in &self.ambiguous_constructors {
            self.constructors
                .insert(name.clone(), Scheme::monomorphic(Ty::Error));
        }
        self.deferred.extend(ambiguous.diagnostics);
        ambiguous.values
    }

    /// 曖昧な宣言で隠す側のコンストラクタを `$outer$` 名へ退避する。
    fn hide_declared_constructors(&mut self, names: &HashSet<String>) {
        for name in names {
            self.ambiguous_constructors.remove(name);
            let hidden = format!("{}$outer${name}", self.module_path.join("."));
            if let Some(scheme) = self.constructors.remove(name) {
                self.constructors.insert(hidden.clone(), scheme);
            }
            if let Some(tag) = self.constructor_tags.remove(name) {
                self.constructor_tags.insert(hidden.clone(), tag);
            }
            if let Some(fields) = self.named_constructors.remove(name) {
                self.named_constructors.insert(hidden, fields);
            }
        }
    }

    fn prepare_module_ambiguities(&mut self, items: &[Item]) {
        let ambiguous =
            crate::ambiguous_declarations::AmbiguousDeclarations::collect(items, self.path);
        self.ambiguous_signatures
            .retain(|name| !ambiguous.declared_signatures.contains(name));
        self.ambiguous_signatures.extend(ambiguous.signatures);
        self.ambiguous_modules
            .retain(|name| !ambiguous.declared_modules.contains(name));
        self.ambiguous_modules.extend(ambiguous.modules);
        for item in items {
            if let Item::ModuleImpl(module) = item {
                if self.ambiguous_signatures.contains(&module.sig) {
                    self.ambiguous_modules.insert(module.name.clone());
                }
            }
        }
    }

    fn check_ambiguous_definitions(&mut self, items: &[Item], ambiguous: &HashSet<String>) {
        for item in items {
            let Item::Def(def) = item else { continue };
            if !ambiguous.contains(&def.name) {
                continue;
            }
            let saved = self.clone();
            self.current_definition = Some(def.name.clone());
            let result = self.definition(def);
            let mut diagnostics = self.deferred.split_off(saved.deferred.len());
            if let Err(error) = result {
                diagnostics.push(error);
            }
            *self = saved;
            self.deferred.extend(diagnostics);
        }
    }

    fn check_ambiguous_modules(
        &mut self,
        items: &[Item],
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) {
        for item in items {
            let Item::ModuleImpl(module) = item else {
                continue;
            };
            if !self.ambiguous_modules.contains(&module.name) {
                continue;
            }
            let saved = self.clone();
            self.module_path.push(module.name.clone());
            let ast = Ast {
                items: module.items.clone(),
                span: module.span,
                library: None,
                branches_after_fallback: Vec::new(),
                branch_anchors: Vec::new(),
                branch_spans: Vec::new(),
                written_list_patterns: Vec::new(),
                long_condition_functions: Vec::new(),
                equality_anchors: Vec::new(),
                qualified_type_anchors: Vec::new(),
                name_sites: Vec::new(),
            };
            let result = self.infer_module_items(&ast, signatures);
            let mut diagnostics = self.deferred.split_off(saved.deferred.len());
            if let Err(error) = result {
                diagnostics.push(error);
            }
            *self = saved;
            self.deferred.extend(diagnostics);
        }
    }

    fn ordinary_module(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<(), Diagnostic> {
        if self.ambiguous_modules.contains(&module.name) {
            return Ok(());
        }
        let name = &module.sig;
        let signature = signatures
            .get(name)
            .ok_or_else(|| self.error(module.sig_span, Some("E204"), "unknown signature"))?;
        let saved = self.clone();
        let saved_value_spans: std::collections::HashSet<_> =
            self.value_types.keys().copied().collect();
        let saved_composite_spans: std::collections::HashSet<_> =
            self.composite_types.keys().copied().collect();
        let saved_with_spans: std::collections::HashSet<_> =
            self.with_types.keys().copied().collect();
        self.module_path.push(module.name.clone());
        let mut exports = match self.module_body(module, signature, signatures) {
            Ok(exports) => exports,
            Err(error) => {
                let diagnostics = self.deferred.split_off(saved.deferred.len());
                *self = saved;
                self.deferred.extend(diagnostics);
                self.recover_inference_error(Err(error), ())?;
                let exports = self.module_signature_exports(module, signature)?;
                self.modules.insert(module.name.clone(), exports);
                return Ok(());
            }
        };
        self.qualify_module_exports(
            module,
            &mut exports,
            &saved_value_spans,
            &saved_composite_spans,
            &saved_with_spans,
        );
        self.module_path.pop();
        self.modules = saved.modules;
        self.abstract_types = saved.abstract_types;
        self.type_arities = saved.type_arities;
        self.aliases = saved.aliases;
        self.ambiguous_types = saved.ambiguous_types;
        self.ambiguous_modules = saved.ambiguous_modules;
        self.ambiguous_signatures = saved.ambiguous_signatures;
        self.ambiguous_constructors = saved.ambiguous_constructors;
        self.named_constructors = saved.named_constructors;
        self.records = saved.records;
        self.constructors = saved.constructors;
        self.constructor_tags = saved.constructor_tags;
        self.values = saved.values;
        self.modules.insert(module.name.clone(), exports);
        self.module_abstract_parameters(module, signature);
        Ok(())
    }

    fn qualify_module_exports(
        &mut self,
        module: &crate::parser::ModuleImpl,
        exports: &mut HashMap<String, Scheme>,
        saved_value_spans: &HashSet<(usize, usize)>,
        saved_composite_spans: &HashSet<(usize, usize)>,
        saved_with_spans: &HashSet<(usize, usize)>,
    ) {
        let type_names: HashMap<_, _> = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::TypeDef(definition) => Some((
                    definition.name.clone(),
                    format!("{}.{}", self.module_path.join("."), definition.name),
                )),
                _ => None,
            })
            .collect();
        self.qualify_module_representations(&type_names);
        self.qualify_module_public_types(exports, &type_names);
        self.qualify_module_value_types(
            &type_names,
            saved_value_spans,
            saved_composite_spans,
            saved_with_spans,
        );
        self.qualify_module_type_definitions(module, &type_names);
    }

    fn qualify_module_representations(&mut self, type_names: &HashMap<String, String>) {
        let prefix = format!("{}.", self.module_path.join("."));
        for (name, representation) in &mut self.abstract_representations {
            if name.starts_with(&prefix) {
                representation.body = representation.body.qualify_data_types(type_names);
            }
        }
        // shared の本体も abstract_representations と同じく局所型名を実装名で
        // 修飾する。これを落とすと別実装の同名局所型が同じ型に混同される。
        for (name, representation) in &mut self.shared_realizations {
            if name.starts_with(&prefix) {
                representation.body = representation.body.qualify_data_types(type_names);
            }
        }
    }

    fn qualify_module_public_types(
        &mut self,
        exports: &mut HashMap<String, Scheme>,
        type_names: &HashMap<String, String>,
    ) {
        let prefix = format!("{}.", self.module_path.join("."));
        // 公開面にも同じ修飾を通す。shared の本体と公開面が食い違うと、
        // 実装の member 呼び出しで型が一致しなくなる。
        for scheme in exports.values_mut() {
            *scheme = scheme
                .resolved(&self.inference)
                .qualify_data_types(type_names);
        }
        for (name, scheme) in &mut self.module_definitions {
            if name.starts_with(&prefix) {
                *scheme = scheme
                    .resolved(&self.inference)
                    .qualify_data_types(type_names);
            }
        }
    }

    fn qualify_module_value_types(
        &mut self,
        type_names: &HashMap<String, String>,
        saved_value_spans: &HashSet<(usize, usize)>,
        saved_composite_spans: &HashSet<(usize, usize)>,
        saved_with_spans: &HashSet<(usize, usize)>,
    ) {
        for (span, ty) in &mut self.value_types {
            if !saved_value_spans.contains(span) {
                *ty = self.inference.resolve(ty).qualify_data_types(type_names);
            }
        }
        for (span, ty) in &mut self.composite_types {
            if !saved_composite_spans.contains(span) {
                *ty = self.inference.resolve(ty).qualify_data_types(type_names);
            }
        }
        self.qualify_module_with_types(type_names, saved_with_spans);
    }

    // `with` ごとに控えた型も、module の本体で推論した分だけ局所型名を修飾する。
    fn qualify_module_with_types(
        &mut self,
        type_names: &HashMap<String, String>,
        saved_with_spans: &HashSet<(usize, usize)>,
    ) {
        for (with_span, types) in &mut self.with_types {
            if saved_with_spans.contains(with_span) {
                continue;
            }
            for ty in types
                .values
                .values_mut()
                .chain(types.composites.values_mut())
            {
                *ty = self.inference.resolve(ty).qualify_data_types(type_names);
            }
        }
    }

    fn qualify_module_type_definitions(
        &mut self,
        module: &crate::parser::ModuleImpl,
        type_names: &HashMap<String, String>,
    ) {
        for item in &module.items {
            if let Item::TypeDef(definition) = item {
                self.qualify_module_definition(definition, type_names);
            }
        }
    }

    fn qualify_module_definition(
        &mut self,
        definition: &crate::parser::TypeDef,
        type_names: &HashMap<String, String>,
    ) {
        if matches!(
            definition.ty.as_ref().map(|ty| &ty.kind),
            Some(TypeKind::Record(_))
        ) {
            if let Some(record) = self.records.get(&definition.name) {
                self.module_records.insert(
                    format!("{}.{}", self.module_path.join("."), definition.name),
                    (record.0.clone(), record.1.qualify_data_types(type_names)),
                );
            }
            return;
        }
        self.qualify_module_constructors(definition, type_names);
    }

    fn qualify_module_constructors(
        &mut self,
        definition: &crate::parser::TypeDef,
        type_names: &HashMap<String, String>,
    ) {
        for (name, scheme) in &self.constructors {
            let result = match scheme.body() {
                Ty::Fn(_, result, _) => result.as_ref(),
                ty => ty,
            };
            if !matches!(result, Ty::Adt(type_name, _) if type_name == &definition.name) {
                continue;
            }
            if let Some(fields) = self.named_constructors.get(name) {
                self.module_named_constructors.insert(
                    format!("{}.{}", self.module_path.join("."), name),
                    fields.clone(),
                );
            }
            self.module_constructors.insert(
                format!("{}.{}", self.module_path.join("."), name),
                (
                    self.constructor_tags[name],
                    scheme.qualify_data_types(type_names),
                ),
            );
        }
    }

    fn module_abstract_parameters(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signature: &crate::parser::ModuleSig,
    ) -> HashMap<String, Ty> {
        if self.ambiguous_modules.contains(&module.name) {
            return HashMap::new();
        }
        let mut parameters = HashMap::new();
        for item in &signature.items {
            if let Item::TypeDef(def) = item {
                let local = format!("{}.{}", module.name, def.name);
                let qualified = if self.module_path.is_empty() {
                    local.clone()
                } else {
                    format!("{}.{}", self.module_path.join("."), local)
                };
                self.abstract_types
                    .insert(local, (def.params.len(), qualified.clone()));
                parameters.insert(def.name.clone(), Ty::Abstract(qualified, vec![]));
            }
        }
        parameters
    }

    fn module_signature_exports(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signature: &crate::parser::ModuleSig,
    ) -> Result<HashMap<String, Scheme>, Diagnostic> {
        let abstracts = self.module_abstract_parameters(module, signature);
        let mut exports = HashMap::new();
        for item in &signature.items {
            if let Item::SigDef(member) = item {
                let mut parameters = abstracts.clone();
                for parameter in &member.type_params {
                    parameters.insert(parameter.name.clone(), self.inference.fresh());
                }
                let ty = self.signature_type(member, &parameters, true)?;
                exports.insert(member.name.clone(), self.inference.generalize(&ty, &[]));
            }
        }
        Ok(exports)
    }

    /// §58.3: the type each contract declares for its members, spelled from the
    /// contract alone. `modules` holds the same declarations as one
    /// implementation instantiates them (`Mem.T`, `shared` revealed), so a
    /// contract with no implementation, or with several, has no entry there.
    /// Here the contract's own abstract types read `Store.T`, and the result is
    /// the same whichever implementations the program has (§58.5: what a
    /// contract's hover shows does not depend on the entry that read it). A
    /// member whose declaration does not resolve is left out; its diagnostic
    /// belongs to the stages that already own it, so nothing is reported here.
    fn collect_contract_exports(
        &mut self,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) {
        for (name, signature) in signatures {
            if self.ambiguous_signatures.contains(name) {
                continue;
            }
            let exports = self.contract_exports(signature);
            self.contracts.insert(name.clone(), exports);
        }
    }

    fn contract_exports(
        &mut self,
        signature: &crate::parser::ModuleSig,
    ) -> HashMap<String, Scheme> {
        // The arity check in `annotation_inner` reads `abstract_types`, so the
        // contract's own types are registered for the duration and taken back
        // after: an annotation outside the contract must not resolve `Store.T`
        // through here.
        let saved_abstract_types = self.abstract_types.clone();
        let mut abstracts = HashMap::new();
        for item in &signature.items {
            if let Item::TypeDef(def) = item {
                let qualified = format!("{}.{}", signature.name, def.name);
                self.abstract_types
                    .insert(qualified.clone(), (def.params.len(), qualified.clone()));
                abstracts.insert(def.name.clone(), Ty::Abstract(qualified, vec![]));
            }
        }
        let mut exports = HashMap::new();
        for item in &signature.items {
            let Item::SigDef(member) = item else { continue };
            let mut parameters = abstracts.clone();
            for parameter in &member.type_params {
                parameters.insert(parameter.name.clone(), self.inference.fresh());
            }
            if let Ok(ty) = self.signature_type(member, &parameters, true) {
                exports.insert(member.name.clone(), self.inference.generalize(&ty, &[]));
            }
        }
        self.abstract_types = saved_abstract_types;
        exports
    }

    fn infer_module_items<'a>(
        &mut self,
        ast: &'a Ast,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<(HashMap<String, &'a crate::parser::Def>, HashSet<String>), Diagnostic> {
        crate::type_cycles::check(ast, self.path)?;
        self.prepare_module_ambiguities(&ast.items);
        let mut signatures = signatures.clone();
        for item in &ast.items {
            if let Item::ModuleSig(signature) = item {
                signatures.insert(signature.name.clone(), signature);
            }
        }
        self.infer_nested_modules(&ast.items, &signatures)?;
        let ambiguous_values = self.prepare_ambiguous_declarations(&ast.items);
        self.register_local_type_names(ast);
        let mut definitions = HashMap::new();
        self.register_module_items(ast, &mut definitions)?;
        self.infer_module_definition_groups(ast, &definitions, &ambiguous_values)?;
        self.check_ambiguous_definitions(&ast.items, &ambiguous_values);
        for name in definitions.keys() {
            self.module_definitions.insert(
                format!("{}.{}", self.module_path.join("."), name),
                self.values[name].clone(),
            );
        }
        self.check_ambiguous_modules(&ast.items, &signatures);
        Ok((definitions, ambiguous_values))
    }

    fn infer_nested_modules(
        &mut self,
        items: &[Item],
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<(), Diagnostic> {
        for item in items {
            if let Item::ModuleImpl(inner) = item {
                self.module(inner, signatures)?;
            }
        }
        Ok(())
    }

    fn register_local_type_names(&mut self, ast: &Ast) {
        for item in &ast.items {
            let Item::TypeDef(def) = item else { continue };
            if !self.ambiguous_types.contains(&def.name) {
                self.register_local_type_name(def, ast.span);
            }
        }
    }

    fn register_local_type_name(&mut self, def: &crate::parser::TypeDef, span: Span) {
        self.type_arities.insert(def.name.clone(), def.params.len());
        self.aliases.remove(&def.name);
        let Some(ty) = &def.ty else { return };
        if matches!(ty.kind, TypeKind::Name(_) | TypeKind::Apply(_, _)) {
            self.aliases
                .insert(def.name.clone(), (def.params.clone(), ty.clone(), span));
        }
    }

    fn register_module_items<'a>(
        &mut self,
        ast: &'a Ast,
        definitions: &mut HashMap<String, &'a crate::parser::Def>,
    ) -> Result<(), Diagnostic> {
        for item in &ast.items {
            match item {
                Item::TypeDef(def) => self.register_type(def, &[])?,
                Item::Def(def) => {
                    definitions.entry(def.name.clone()).or_insert(def);
                }
                Item::ModuleImpl(_) | Item::ModuleSig(_) | Item::Effect(_) | Item::Handler(_) => {}
                _ => {
                    return Err(self.error(ast.span, None, "unexpected signature member in module"))
                }
            }
        }
        Ok(())
    }

    fn infer_module_definition_groups(
        &mut self,
        ast: &Ast,
        definitions: &HashMap<String, &crate::parser::Def>,
        ambiguous_values: &HashSet<String>,
    ) -> Result<(), Diagnostic> {
        for group in crate::names::definition_groups(ast) {
            let group: Vec<_> = group
                .into_iter()
                .filter(|name| !ambiguous_values.contains(name))
                .collect();
            if !group.is_empty() {
                self.definition_group(&group, definitions)?;
            }
        }
        Ok(())
    }

    fn module_body(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signature: &crate::parser::ModuleSig,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<HashMap<String, Scheme>, Diagnostic> {
        let ast = Ast {
            items: module.items.clone(),
            span: module.span,
            library: None,
            branches_after_fallback: Vec::new(),
            branch_anchors: Vec::new(),
            branch_spans: Vec::new(),
            written_list_patterns: Vec::new(),
            long_condition_functions: Vec::new(),
            equality_anchors: Vec::new(),
            qualified_type_anchors: Vec::new(),
            name_sites: Vec::new(),
        };
        let (definitions, ambiguous_values) = self.infer_module_items(&ast, signatures)?;
        let abstract_names = self.register_module_realizations(&ast, module, signature)?;
        let exports = self.collect_module_exports(
            module,
            signature,
            &definitions,
            &ambiguous_values,
            &abstract_names,
        )?;
        self.resolve_projections(false)?;
        let roots = exports.keys().cloned().collect::<Vec<_>>();
        if let Some(name) = crate::names::unused_definitions(&ast, &roots).first() {
            self.deferred.push(self.error(
                definitions[name].span,
                Some("E514"),
                &format!("unused module definition {name}"),
            ));
        }
        Ok(exports)
    }

    fn register_module_realizations(
        &mut self,
        ast: &Ast,
        module: &crate::parser::ModuleImpl,
        signature: &crate::parser::ModuleSig,
    ) -> Result<HashMap<String, String>, Diagnostic> {
        let mut abstract_names = HashMap::new();
        for item in &signature.items {
            let Item::TypeDef(def) = item else { continue };
            let actual = find_actual_type(&ast.items, def);
            let Some(actual) = actual else {
                return Err(self.error_related(
                    module.span,
                    Some("E602"),
                    "missing signature type",
                    Some(def.name_span),
                ));
            };
            if def.ty.is_some() {
                return Err(self.error(
                    def.span,
                    None,
                    "manifest signature types are not implemented",
                ));
            }
            let qualified = self.register_module_realization(def, actual)?;
            abstract_names.insert(def.name.clone(), qualified);
        }
        Ok(abstract_names)
    }

    fn register_module_realization(
        &mut self,
        def: &crate::parser::TypeDef,
        actual: &crate::parser::TypeDef,
    ) -> Result<String, Diagnostic> {
        let qualified = format!("{}.{}", self.module_path.join("."), def.name);
        let parameters: HashMap<_, _> = def
            .params
            .iter()
            .map(|p| (p.name.clone(), self.inference.fresh()))
            .collect();
        let arguments: Vec<_> = def
            .params
            .iter()
            .map(|p| TypeExpr {
                kind: TypeKind::Name(p.name.clone()),
                span: p.span,
                name_span: p.span,
                effects: None,
            })
            .collect();
        let kind = if arguments.is_empty() {
            TypeKind::Name(def.name.clone())
        } else {
            TypeKind::Apply(def.name.clone(), arguments)
        };
        let reference = TypeExpr {
            kind,
            span: def.name_span,
            name_span: def.name_span,
            effects: None,
        };
        let body = self.payload_type(&reference, &parameters, def.name_span)?;
        let parameters = def
            .params
            .iter()
            .map(|p| match parameters[&p.name] {
                Ty::Var(id) => id,
                _ => unreachable!(),
            })
            .collect();
        let realization = TypeRealization { parameters, body };
        if actual.shared {
            self.shared_realizations
                .insert(qualified.clone(), realization.clone());
        }
        self.abstract_representations
            .insert(qualified.clone(), realization);
        Ok(qualified)
    }

    fn collect_module_exports(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signature: &crate::parser::ModuleSig,
        definitions: &HashMap<String, &crate::parser::Def>,
        ambiguous_values: &HashSet<String>,
        abstract_names: &HashMap<String, String>,
    ) -> Result<HashMap<String, Scheme>, Diagnostic> {
        let mut exports = HashMap::new();
        for item in &signature.items {
            if matches!(item, Item::TypeDef(_)) {
                continue;
            }
            let Item::SigDef(declaration) = item else {
                return Err(self.error(
                    signature.span,
                    None,
                    "signature type members are not implemented",
                ));
            };
            let (name, scheme) = self.module_export(
                module,
                declaration,
                definitions,
                ambiguous_values,
                abstract_names,
            )?;
            exports.insert(name, scheme);
        }
        Ok(exports)
    }

    fn module_export(
        &mut self,
        module: &crate::parser::ModuleImpl,
        declaration: &crate::parser::SigDef,
        definitions: &HashMap<String, &crate::parser::Def>,
        ambiguous_values: &HashSet<String>,
        abstract_names: &HashMap<String, String>,
    ) -> Result<(String, Scheme), Diagnostic> {
        if !definitions.contains_key(&declaration.name) {
            return Err(self.error_related(
                module.span,
                Some("E602"),
                "missing signature member",
                Some(declaration.name_span),
            ));
        }
        if matches!(definitions[&declaration.name].kind, DefKind::Value(_))
            != declaration.params.is_empty()
        {
            return Err(self.error_related(
                module.span,
                Some("E602"),
                "signature member requires the declared definition form",
                Some(declaration.name_span),
            ));
        }
        let scheme = self.values[&declaration.name].clone();
        let (mut actual, constraints, rows) = self.inference.instantiate_constrained(&scheme);
        if !ambiguous_values.contains(&declaration.name)
            && self.inference.resolve(&actual).has_error()
        {
            let known = self.invalid_definition_type(definitions[&declaration.name]);
            actual = self.inference.freshen_errors(&known);
        }
        self.record_comparisons(constraints, declaration.span);
        self.record_row_constraints(rows, declaration.span);
        let parameters: HashMap<_, _> = declaration
            .type_params
            .iter()
            .map(|name| (name.name.clone(), self.inference.fresh()))
            .collect();
        let expected = self.signature_type(declaration, &parameters, false)?;
        self.inference.unify(&actual, &expected).map_err(|_| {
            self.error_related(
                module.span,
                Some("E602"),
                "signature type mismatch",
                Some(declaration.name_span),
            )
        })?;
        self.check_signature_polymorphism(&parameters, module, declaration)?;
        let mut public_parameters: HashMap<_, _> = abstract_names
            .iter()
            .map(|(name, public)| (name.clone(), Ty::Abstract(public.clone(), vec![])))
            .collect();
        public_parameters.extend(parameters);
        let public_type = self.signature_type(declaration, &public_parameters, true)?;
        let resolved = self.inference.resolve(&expected);
        let _ = self.inference.unify(&public_type, &resolved);
        let scheme = self.inference.generalize_constrained(
            &public_type,
            &[],
            &self.comparison_constraints,
            &self.row_constraints,
        );
        Ok((declaration.name.clone(), scheme))
    }

    fn check_signature_polymorphism(
        &self,
        parameters: &HashMap<String, Ty>,
        module: &crate::parser::ModuleImpl,
        declaration: &crate::parser::SigDef,
    ) -> Result<(), Diagnostic> {
        let mut resolved_parameters = Vec::new();
        for parameter in parameters.values() {
            let resolved = self.inference.resolve(parameter);
            if !matches!(resolved, Ty::Var(_)) || resolved_parameters.contains(&resolved) {
                return Err(self.error_related(
                    module.span,
                    Some("E602"),
                    "signature implementation is not sufficiently polymorphic",
                    Some(declaration.name_span),
                ));
            }
            resolved_parameters.push(resolved);
        }
        Ok(())
    }

    fn signature_type(
        &mut self,
        declaration: &crate::parser::SigDef,
        parameters: &HashMap<String, Ty>,
        public: bool,
    ) -> Result<Ty, Diagnostic> {
        let mut bindings: HashMap<_, _> = self
            .contract_types
            .types
            .iter()
            .map(|(name, implementation)| {
                let ty = implementation
                    .as_ref()
                    .map_or(Ty::Error, |name| Ty::Abstract(name.clone(), vec![]));
                (name.clone(), ty)
            })
            .collect();
        bindings.extend(parameters.clone());
        let parameters = &bindings;
        let mut args = Vec::new();
        for param in &declaration.params {
            let annotation = param.ty.as_ref().ok_or_else(|| {
                self.error(param.span, None, "signature parameter requires a type")
            })?;
            args.push(self.payload_type(annotation, parameters, param.span)?);
        }
        let result_ty = declaration.ret.as_ref().ok_or_else(|| {
            self.error(declaration.span, None, "signature requires a return type")
        })?;
        let result = self.payload_type(result_ty, parameters, declaration.span)?;
        let row = self.row_annotation(&result_ty.effects)?;
        let ty = Ty::fn_with(args, result, row);
        if public {
            return Ok(self.reveal_shared(&ty));
        }
        let prefix = format!("{}.", self.module_path.join("."));
        let local = self
            .abstract_representations
            .iter()
            .filter(|(name, _)| name.starts_with(&prefix))
            .map(|(name, representation)| (name.clone(), representation.clone()))
            .collect();
        Ok(self.reveal_shared(&ty.reveal(&local)))
    }

    fn reveal_shared(&self, ty: &Ty) -> Ty {
        ty.reveal(&self.shared_realizations)
    }

    fn definition_group(
        &mut self,
        group: &[String],
        definitions: &HashMap<String, &crate::parser::Def>,
    ) -> Result<(), Diagnostic> {
        let inherited_projections: std::collections::HashSet<_> = self
            .projections
            .iter()
            .map(|(_, field, _, _)| (field.dot.line, field.dot.col))
            .collect();
        let placeholders: HashMap<_, _> = group
            .iter()
            .map(|name| (name.clone(), self.inference.fresh()))
            .collect();
        self.install_group_placeholders(definitions, &placeholders);
        let mut invalid_group = self.check_group_definitions(group, definitions, &placeholders)?;
        let deferred_start = self.deferred.len();
        let result = self.default_numbers();
        invalid_group |= self.recover_inference_error(result.map(|()| false), true)?;
        let result = self.resolve_projections(false);
        invalid_group |= self.recover_inference_error(result.map(|()| false), true)?;
        invalid_group |= self.deferred[deferred_start..]
            .iter()
            .any(|error| crate::diagnostic_order(error).0 == 3);
        if invalid_group {
            self.invalidate_group(group, definitions, &inherited_projections);
            return Ok(());
        }
        self.generalize_group(placeholders);
        Ok(())
    }

    fn install_group_placeholders(
        &mut self,
        definitions: &HashMap<String, &crate::parser::Def>,
        placeholders: &HashMap<String, Ty>,
    ) {
        for (name, ty) in placeholders {
            if definitions[name].rec {
                let Ty::Var(id) = ty else {
                    unreachable!("fresh recursive placeholder")
                };
                self.recursive_calls.insert(*id, Vec::new());
            }
            self.values
                .insert(name.clone(), Scheme::monomorphic(ty.clone()));
        }
    }

    fn check_group_definitions(
        &mut self,
        group: &[String],
        definitions: &HashMap<String, &crate::parser::Def>,
        placeholders: &HashMap<String, Ty>,
    ) -> Result<bool, Diagnostic> {
        let mut invalid_group = false;
        for name in group {
            invalid_group |=
                self.check_group_definition(name, definitions[name], &placeholders[name])?;
        }
        // 定義の走査が終わったら、どの定義を推論中かという印を落とす。
        self.current_definition = None;
        Ok(invalid_group)
    }

    fn check_group_definition(
        &mut self,
        name: &str,
        def: &crate::parser::Def,
        placeholder: &Ty,
    ) -> Result<bool, Diagnostic> {
        self.current_definition = Some(name.to_string());
        let removed = if !def.rec {
            self.values.remove(name)
        } else {
            None
        };
        let result = self.check_definition(def, placeholder);
        if let Some(scheme) = removed {
            self.values.insert(name.to_string(), scheme);
        }
        let invalid = self.recover_inference_error(result.map(|()| false), true)?;
        if invalid {
            let ty = self.invalid_definition_type(def);
            self.values
                .insert(name.to_string(), Scheme::monomorphic(ty));
        }
        Ok(invalid)
    }

    fn invalidate_group(
        &mut self,
        group: &[String],
        definitions: &HashMap<String, &crate::parser::Def>,
        inherited_projections: &std::collections::HashSet<(usize, usize)>,
    ) {
        self.projections.retain(|(_, field, _, _)| {
            inherited_projections.contains(&(field.dot.line, field.dot.col))
        });
        for name in group {
            let ty = self.invalid_definition_type(definitions[name]);
            self.values.insert(name.clone(), Scheme::monomorphic(ty));
        }
    }

    fn generalize_group(&mut self, placeholders: HashMap<String, Ty>) {
        let environment: Vec<_> = self
            .projections
            .iter()
            .flat_map(|(input, _, output, _)| [input.clone(), output.clone()])
            .collect();
        for (name, ty) in placeholders {
            self.values.insert(
                name,
                self.inference.generalize_constrained(
                    &ty,
                    &environment,
                    &self.comparison_constraints,
                    &self.row_constraints,
                ),
            );
        }
    }

    fn check_definition(
        &mut self,
        def: &crate::parser::Def,
        placeholder: &Ty,
    ) -> Result<(), Diagnostic> {
        let ty = self.definition(def)?;
        if def.rec {
            self.check_recursive_arguments(placeholder, &ty, def.span)?;
        }
        self.unify(placeholder, &ty, def.span)
    }

    fn invalid_definition_type(&mut self, def: &crate::parser::Def) -> Ty {
        let (params, ret) = match &def.kind {
            DefKind::Function { params, ret, .. } => (params, ret.as_ref()),
            DefKind::Value(Node {
                kind: NodeKind::Fn { params, .. },
                ..
            }) => (params, None),
            DefKind::Value(_) => return Ty::Error,
        };
        let args = params
            .iter()
            .map(|param| {
                param
                    .ty
                    .as_ref()
                    .and_then(|ty| self.annotation(ty, param.span).ok())
                    .unwrap_or(Ty::Error)
            })
            .collect();
        let result = ret
            .and_then(|ty| self.annotation(ty, def.span).ok())
            .unwrap_or(Ty::Error);
        Ty::fn_of(args, result)
    }

    fn check_recursive_arguments(
        &self,
        calls: &Ty,
        definition: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let recursive_calls = match calls {
            Ty::Var(id) => self.recursive_calls.get(id),
            _ => None,
        };
        if let (Some(calls), Ty::Fn(parameters, _, _)) =
            (recursive_calls, self.inference.resolve(definition))
        {
            let first = calls
                .iter()
                .filter(|(arguments, _)| self.recursive_arguments_occur(arguments, &parameters))
                .map(|(_, span)| *span)
                .min_by_key(|span| (span.line, span.col));
            if let Some(span) = first {
                return Err(self.error(
                    span,
                    Some("E307"),
                    "recursive calls change the argument type",
                ));
            }
        }
        if let (Ty::Fn(calls, _, _), Ty::Fn(parameters, _, _)) = (
            self.inference.resolve(calls),
            self.inference.resolve(definition),
        ) {
            if self.recursive_arguments_occur(&calls, &parameters) {
                return Err(self.error(
                    span,
                    Some("E307"),
                    "recursive calls change the argument type",
                ));
            }
        }
        Ok(())
    }

    fn recursive_arguments_occur(&self, arguments: &[Ty], parameters: &[Ty]) -> bool {
        let mut inference = self.inference.clone();
        arguments
            .iter()
            .zip(parameters)
            .any(|(argument, parameter)| {
                inference.unify(argument, parameter) == Err(TypeError::Occurs)
            })
    }

    fn definition(&mut self, def: &crate::parser::Def) -> Result<Ty, Diagnostic> {
        match &def.kind {
            DefKind::Value(body) => {
                let (ty, row) = self.isolate_effects(|this| this.expression(body))?;
                if def.name == "main" || def.name.ends_with("$main") {
                    self.main_effects = row;
                    Ok(ty)
                } else {
                    Ok(Ty::fn_with(vec![], ty, row))
                }
            }
            DefKind::Function { params, ret, body } => {
                let ty = self.function(params, body)?;
                let Some(annotation) = ret else {
                    return Ok(ty);
                };
                let Ty::Fn(args, result, inferred) = ty else {
                    return Ok(ty);
                };
                let expected = self.annotation(annotation, def.span)?;
                self.expect(&expected, &result, def.span, Some(annotation.span))?;
                let expected_row = self.row_annotation(&annotation.effects)?;
                self.contain_row(&inferred, &expected_row, def.span)?;
                let row = if annotation.effects.is_some() {
                    expected_row
                } else {
                    *inferred
                };
                Ok(Ty::fn_with(args, *result, row))
            }
        }
    }

    fn register_type(
        &mut self,
        def: &crate::parser::TypeDef,
        rejected: &[Span],
    ) -> Result<(), Diagnostic> {
        if self.ambiguous_types.contains(&def.name) {
            return Ok(());
        }
        if self.aliases.contains_key(&def.name) {
            return Ok(());
        }
        let parameters: HashMap<_, _> = def
            .params
            .iter()
            .map(|name| (name.name.clone(), self.inference.fresh()))
            .collect();
        let result = Ty::Adt(
            def.name.clone(),
            def.params
                .iter()
                .map(|name| parameters[&name.name].clone())
                .collect(),
        );
        let body = def
            .ty
            .as_ref()
            .ok_or_else(|| self.error(def.span, None, "abstract type outside signature"))?;
        if let TypeKind::Record(fields) = &body.kind {
            let payload = fields
                .iter()
                .map(|field| {
                    self.registered_payload_type(&field.ty, &parameters, def.span, rejected)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let scheme = self.inference.generalize(&Ty::fn_of(payload, result), &[]);
            self.records.insert(
                def.name.clone(),
                (
                    fields.iter().map(|field| field.name.name.clone()).collect(),
                    scheme,
                ),
            );
            return Ok(());
        }
        let variants = match &body.kind {
            TypeKind::Adt(variants) => variants.as_slice(),
            _ => return Err(self.error(def.span, None, "record definitions are not implemented")),
        };
        for (tag, variant) in variants.iter().enumerate() {
            self.register_constructor(variant, &parameters, &result, def.span, rejected)?;
            let name = &variant.name.name;
            let tag = u32::try_from(tag)
                .map_err(|_| self.error(def.span, None, "too many constructors"))?;
            self.constructor_tags.insert(name.clone(), tag);
        }
        Ok(())
    }

    fn register_constructor(
        &mut self,
        variant: &crate::parser::ConstructorDecl,
        parameters: &HashMap<String, Ty>,
        result: &Ty,
        span: Span,
        rejected: &[Span],
    ) -> Result<(), Diagnostic> {
        let name = &variant.name.name;
        if let crate::parser::ConstructorPayload::Named(fields) = &variant.payload {
            self.named_constructors.insert(
                name.clone(),
                fields.iter().map(|field| field.name.name.clone()).collect(),
            );
        }
        let payload = variant
            .payload
            .types()
            .map(|arg| self.registered_payload_type(arg, parameters, span, rejected))
            .collect::<Result<Vec<_>, _>>()?;
        let ty = if payload.is_empty() {
            result.clone()
        } else {
            Ty::fn_of(payload, result.clone())
        };
        if self.ambiguous_constructors.contains(name) {
            return Ok(());
        }
        let scheme = self.inference.generalize(&ty, &[]);
        if self.constructors.insert(name.clone(), scheme).is_some() {
            return Err(self.error(span, None, "duplicate constructor"));
        }
        Ok(())
    }

    fn registered_payload_type(
        &mut self,
        expr: &TypeExpr,
        parameters: &HashMap<String, Ty>,
        span: Span,
        rejected: &[Span],
    ) -> Result<Ty, Diagnostic> {
        let result = self.recovering_payload_type(expr, parameters, span, rejected);
        self.recover_inference_error(result, Ty::Error)
    }

    fn payload_type(
        &mut self,
        expr: &TypeExpr,
        parameters: &HashMap<String, Ty>,
        span: Span,
    ) -> Result<Ty, Diagnostic> {
        self.recovering_payload_type(expr, parameters, span, &[])
    }

    fn recovering_payload_type(
        &mut self,
        expr: &TypeExpr,
        parameters: &HashMap<String, Ty>,
        span: Span,
        rejected: &[Span],
    ) -> Result<Ty, Diagnostic> {
        if rejected
            .iter()
            .any(|reference| reference.line == expr.span.line && reference.col == expr.span.col)
        {
            return Ok(Ty::Error);
        }
        self.annotation_inner(expr, span, parameters, rejected, &mut Vec::new())
    }

    fn error(&self, span: Span, code: Option<&str>, message: &str) -> Diagnostic {
        self.error_related(span, code, message, None)
    }

    /// §56: `first` is the contract's declaration of the member the report is
    /// about (a `.hdi` file may hold it; the print path localizes the position).
    fn error_related(
        &self,
        span: Span,
        code: Option<&str>,
        message: &str,
        first: Option<Span>,
    ) -> Diagnostic {
        let related = first.map(|first| {
            Box::new(crate::parser::Related {
                path: self.path.into(),
                line: first.line,
                col: first.col,
            })
        });
        Diagnostic {
            mentions: None,
            suggestion: None,
            related,
            path: self.path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: code.map(Into::into),
            message: message.into(),
        }
    }

    fn unify(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), Diagnostic> {
        self.inference.unify(a, b).map_err(|error| {
            let code = match error {
                TypeError::Mismatch => "E305",
                TypeError::Arity => "E301",
                TypeError::Occurs => "E306",
            };
            let (left, right) = crate::types::readable_pair(a, b);
            self.error(
                span,
                Some(code),
                &format!("cannot unify {left} with {right}"),
            )
        })
    }

    fn recover_inference_error<T>(
        &mut self,
        result: Result<T, Diagnostic>,
        fallback: T,
    ) -> Result<T, Diagnostic> {
        match result {
            Err(error) if matches!(crate::diagnostic_order(&error).0, 2 | 3) => {
                self.deferred.push(error);
                Ok(fallback)
            }
            result => result,
        }
    }

    fn expression(&mut self, node: &Node) -> Result<Ty, Diagnostic> {
        let result = self.expression_inner(node);
        let ty = self.recover_inference_error(result, Ty::Error)?;
        if matches!(
            node.kind,
            NodeKind::Apply(..)
                | NodeKind::Call(..)
                | NodeKind::Pipe(..)
                | NodeKind::List(..)
                | NodeKind::Record(..)
                | NodeKind::NamedConstructor(..)
                | NodeKind::With { .. }
        ) {
            let key = (node.span.line, node.span.col);
            self.composite_types.insert(key, ty.clone());
            self.note_type(key, &ty, true);
        }
        Ok(ty)
    }

    // span 単位の型は最後に推論した `with` のもので上書きされる。今いる
    // `with`（入れ子なら全部）にも同じ型を控え、mono が差し替えられるようにする。
    fn note_type(&mut self, span: (usize, usize), ty: &Ty, composite: bool) {
        for key in self.active_withs.clone() {
            let types = self.with_types.entry(key).or_default();
            if composite {
                types.composites.insert(span, ty.clone());
            } else {
                types.values.insert(span, ty.clone());
            }
        }
    }

    fn expression_inner(&mut self, node: &Node) -> Result<Ty, Diagnostic> {
        match &node.kind {
            NodeKind::Int(_) => Ok(Ty::Int),
            NodeKind::Float(_) => Ok(Ty::Float),
            NodeKind::Bool(_) => Ok(Ty::Bool),
            NodeKind::Char(_) => Ok(Ty::Char),
            NodeKind::Str(_) => Ok(Ty::String),
            NodeKind::Void => Ok(Ty::Void),
            NodeKind::Bin(op, a, b) => self.binary(*op, a, b),
            NodeKind::Not(value) => {
                let ty = self.expression(value)?;
                self.unify(&ty, &Ty::Bool, node.span)?;
                Ok(Ty::Bool)
            }
            NodeKind::Neg(value) => {
                let ty = self.expression(value)?;
                self.numeric(&ty, node.span)?;
                Ok(ty)
            }
            NodeKind::Name(name) => {
                if self.constructors.contains_key(name) {
                    return self.construct(name, &[], node.span);
                }
                if name.starts_with("__") {
                    return Err(self.error(
                        node.span,
                        Some("E203"),
                        "intrinsics cannot be used as values",
                    ));
                }
                let scheme = self.values.get(name).cloned().ok_or_else(|| {
                    self.error(
                        node.span,
                        Some(if self.current_definition.as_deref() == Some(name) {
                            "E201"
                        } else {
                            "E204"
                        }),
                        &format!("unbound name {name}"),
                    )
                })?;
                let (ty, constraints, rows) = self.inference.instantiate_constrained(&scheme);
                let ty = self.unwrap_operation(name, ty);
                let ty = self.unwrap_thunk(ty);
                let key = (node.span.line, node.span.col);
                self.value_types.insert(key, ty.clone());
                self.note_type(key, &ty, false);
                self.record_comparisons(constraints, node.span);
                self.record_row_constraints(rows, node.span);
                Ok(ty)
            }
            NodeKind::Fn { params, body } => {
                let ty = self.function(params, body)?;
                let key = (node.span.line, node.span.col);
                self.value_types.insert(key, ty.clone());
                self.note_type(key, &ty, false);
                Ok(ty)
            }
            NodeKind::List(items, tail) => self.list(items, tail.as_deref()),
            NodeKind::Record(fields) => self.record(fields, node.span),
            NodeKind::NamedConstructor(name, fields) => self.named_construct(name, fields),
            NodeKind::Field(value, field) => {
                let ty = self.project(value, field)?;
                let key = (node.span.line, node.span.col);
                self.value_types.insert(key, ty.clone());
                self.note_type(key, &ty, false);
                Ok(ty)
            }
            NodeKind::PatternBranch {
                scrutinees,
                arms,
                fallback,
            } => self.pattern_branch(scrutinees, arms, fallback.as_deref(), node.span),
            NodeKind::Pipe(value, target) => self.pipe(value, target),
            NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
                self.apply(callee, args, node.span)
            }
            NodeKind::Let { pat, value, body } => self.binding(pat, value, body),
            NodeKind::LetQ { pat, value, body } => self.propagation(Some(pat), value, body),
            NodeKind::DoQ { value, body } => self.propagation(None, value, body),
            NodeKind::BoolBranch { cond, then, els } => {
                self.condition(cond)?;
                let result = self.expression(then)?;
                self.branch_result_expect(els, &result, Some(then.span))?;
                Ok(self.inference.resolve(&result))
            }
            NodeKind::CondBranch { arms, fallback } => self.conditions(arms, fallback.as_deref()),
            NodeKind::With {
                handler,
                args,
                body,
            } => self.with_handler(node.span, handler, args, body),
            NodeKind::Resume => Ok(self
                .resume_type
                .clone()
                .unwrap_or_else(|| self.inference.fresh())),
        }
    }

    fn record(
        &mut self,
        fields: &[(crate::parser::DeclaredName, Node)],
        span: Span,
    ) -> Result<Ty, Diagnostic> {
        let candidates: Vec<_> = self
            .records
            .values()
            .filter(|(names, _)| {
                names.len() == fields.len()
                    && names
                        .iter()
                        .all(|name| fields.iter().any(|(field, _)| &field.name == name))
            })
            .cloned()
            .collect();
        if candidates.len() != 1 {
            return Err(self.error(
                span,
                Some("E304"),
                "record fields must identify one nominal type",
            ));
        }
        let (names, scheme) = &candidates[0];
        let Ty::Fn(payload, result, _) = self.inference.instantiate(scheme) else {
            unreachable!()
        };
        for (name, ty) in names.iter().zip(&payload) {
            let value = &fields
                .iter()
                .find(|(field, _)| &field.name == name)
                .unwrap()
                .1;
            self.branch_result_expect(value, ty, None)?;
        }
        Ok(self.inference.resolve(&result))
    }

    fn project(&mut self, value: &Node, field: &FieldName) -> Result<Ty, Diagnostic> {
        if let NodeKind::Name(name) = &value.kind {
            if self.ambiguous_modules.contains(name) {
                return Ok(Ty::Error);
            }
            if let Some(exports) = self.modules.get(name) {
                let scheme = exports.get(&field.name).cloned().ok_or_else(|| {
                    self.error(field.span, Some("E204"), "module member is not exported")
                })?;
                let (ty, constraints, rows) = self.inference.instantiate_constrained(&scheme);
                self.record_comparisons(constraints, field.span);
                self.record_row_constraints(rows, field.span);
                return Ok(self.unwrap_thunk(ty));
            }
        }
        let ty = self.expression(value)?;
        self.project_type(&ty, field, value.span)
    }

    fn resolve_projections(&mut self, final_pass: bool) -> Result<(), Diagnostic> {
        for (input, field, output, receiver) in std::mem::take(&mut self.projections) {
            if matches!(self.inference.resolve(&input), Ty::Var(_)) {
                if final_pass {
                    self.deferred.push(self.error(
                        receiver,
                        Some("E304"),
                        "record type could not be inferred",
                    ));
                } else {
                    self.projections.push((input, field, output, receiver));
                }
            } else {
                let checked = self.project_type(&input, &field, receiver);
                let actual = self.recover_inference_error(checked, Ty::Error)?;
                let checked = self.unify(&output, &actual, field.dot);
                self.recover_inference_error(checked, ())?;
            }
        }
        Ok(())
    }

    fn infer_record_from_field(
        &mut self,
        ty: &Ty,
        field: &str,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !matches!(self.inference.resolve(ty), Ty::Var(_)) {
            return Ok(());
        }
        let candidates: Vec<_> = self
            .records
            .values()
            .cloned()
            .chain(self.named_projection_schemas())
            .filter(|(names, _)| names.iter().any(|name| name == field))
            .map(|(_, scheme)| scheme.clone())
            .collect();
        if candidates.len() != 1 {
            return Ok(());
        }
        let Ty::Fn(_, result, _) = self.inference.instantiate(&candidates[0]) else {
            unreachable!()
        };
        self.unify(ty, &result, span)
    }

    fn project_type(
        &mut self,
        ty: &Ty,
        field: &FieldName,
        receiver: Span,
    ) -> Result<Ty, Diagnostic> {
        if self.inference.resolve(ty) == Ty::Error {
            return Ok(Ty::Error);
        }
        if matches!(self.inference.resolve(ty), Ty::Abstract(_, _)) {
            return Err(self.error(
                field.dot,
                Some("E601"),
                "abstract type has no accessible fields",
            ));
        }
        self.infer_record_from_field(ty, &field.name, field.dot)?;
        if matches!(self.inference.resolve(ty), Ty::Var(_)) {
            let result = self.inference.fresh();
            self.projections
                .push((ty.clone(), field.clone(), result.clone(), receiver));
            return Ok(result);
        }
        let Ty::Adt(name, _) = self.inference.resolve(ty) else {
            return Err(self.error(receiver, Some("E305"), "field access requires a record"));
        };
        let schema = self
            .records
            .get(&name)
            .cloned()
            .or_else(|| self.module_records.get(&name).cloned())
            .or_else(|| self.named_projection_schema(&name));
        let (names, scheme) = schema.ok_or_else(|| {
            let code = if self.has_named_variant(&name) {
                "E204"
            } else {
                "E305"
            };
            self.error(
                if code == "E204" { field.span } else { receiver },
                Some(code),
                "field access requires a single record constructor",
            )
        })?;
        let index = names
            .iter()
            .position(|name| name == &field.name)
            .ok_or_else(|| self.error(field.span, Some("E204"), "unknown field"))?;
        let Ty::Fn(payload, result, _) = self.inference.instantiate(&scheme) else {
            unreachable!()
        };
        self.unify(ty, &result, field.dot)?;
        Ok(self.inference.resolve(&payload[index]))
    }

    fn list(&mut self, items: &[Node], tail: Option<&Node>) -> Result<Ty, Diagnostic> {
        let element = self.inference.fresh();
        for item in items {
            self.branch_result(item, &element)?;
        }
        let list = Ty::Adt("List".into(), vec![element]);
        if let Some(tail) = tail {
            self.branch_result(tail, &list)?;
        }
        Ok(self.inference.resolve(&list))
    }

    fn pattern_branch(
        &mut self,
        scrutinees: &[Node],
        arms: &[(Vec<Pattern>, Node)],
        fallback: Option<&Node>,
        branch_span: Span,
    ) -> Result<Ty, Diagnostic> {
        let inputs: Vec<Ty> = scrutinees
            .iter()
            .map(|scrutinee| self.expression(scrutinee))
            .collect::<Result<_, _>>()?;
        let result = self.inference.fresh();
        for (patterns, body) in arms {
            let saved = self.values.clone();
            let checked = patterns
                .iter()
                .zip(&inputs)
                .try_for_each(|(pattern, input)| self.pattern(pattern, input))
                .and_then(|()| self.branch_result(body, &result));
            self.values = saved;
            checked?;
        }
        if let Some(body) = fallback {
            self.branch_result(body, &result)?;
        }
        let groups = self.constructor_groups();
        let families = groups
            .values()
            .flat_map(|group| group.iter().map(|(name, _)| (name.clone(), group.clone())))
            .collect();
        let rows: Vec<Vec<_>> = arms
            .iter()
            .map(|(patterns, _)| {
                patterns
                    .iter()
                    .map(|pattern| {
                        crate::named_patterns::positional(
                            pattern,
                            &self.named_constructors,
                            &str::to_owned,
                        )
                    })
                    .collect()
            })
            .collect();
        if let Err(error) =
            crate::check::matrix_coverage(branch_span, &rows, fallback, &families, self.path)
        {
            self.deferred.push(error);
        }
        Ok(self.inference.resolve(&result))
    }

    fn pattern(&mut self, pattern: &Pattern, expected: &Ty) -> Result<(), Diagnostic> {
        match &pattern.kind {
            PatternKind::Underscore => Ok(()),
            PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
                self.constructor_pattern(name, &[], expected, pattern.span)
            }
            PatternKind::Name(name) => {
                self.values
                    .insert(name.clone(), Scheme::monomorphic(expected.clone()));
                Ok(())
            }
            PatternKind::Literal(node) => self.branch_result(node, expected),
            PatternKind::Constructor(name, args) => {
                self.constructor_pattern(name, args, expected, pattern.span)
            }
            PatternKind::Record(fields) => self.record_pattern(fields, expected, pattern.span),
            PatternKind::NamedConstructor(name, fields) => {
                self.named_pattern(name, fields, expected)
            }
        }
    }

    fn record_pattern(
        &mut self,
        fields: &[(crate::parser::DeclaredName, Option<Pattern>)],
        expected: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if matches!(self.inference.resolve(expected), Ty::Adt(ref name, _) if self.has_named_variant(name))
        {
            return Err(self.error(
                span,
                Some("E305"),
                "record pattern requires a constructor name",
            ));
        }
        if matches!(self.inference.resolve(expected), Ty::Var(_)) {
            let candidates: Vec<_> = self
                .records
                .values()
                .filter(|(names, _)| fields.iter().all(|(field, _)| names.contains(&field.name)))
                .map(|(_, scheme)| scheme.clone())
                .collect();
            let [scheme] = candidates.as_slice() else {
                return Err(self.error(
                    span,
                    Some("E304"),
                    "record pattern fields must identify one nominal type",
                ));
            };
            let Ty::Fn(_, result, _) = self.inference.instantiate(scheme) else {
                unreachable!()
            };
            self.unify(expected, &result, span)?;
        }
        for (name, pattern) in fields {
            let field = FieldName {
                name: name.name.clone(),
                dot: span,
                span: name.span,
            };
            let ty = self.project_type(expected, &field, span)?;
            match pattern {
                Some(pattern) => self.pattern(pattern, &ty)?,
                None => {
                    self.values
                        .insert(name.name.clone(), Scheme::monomorphic(ty));
                }
            }
        }
        Ok(())
    }

    fn constructor_pattern(
        &mut self,
        name: &str,
        args: &[Pattern],
        expected: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if self.named_constructors.contains_key(name) {
            return Err(self.error(
                span,
                Some("E301"),
                "constructor pattern requires named fields",
            ));
        }
        let scheme = self
            .constructors
            .get(name)
            .cloned()
            .ok_or_else(|| self.error(span, Some("E204"), "unknown constructor"))?;
        let ty = self.inference.instantiate(&scheme);
        if ty == Ty::Error {
            for pattern in args {
                self.pattern(pattern, &Ty::Error)?;
            }
            return Ok(());
        }
        match ty {
            Ty::Fn(payload, result, _) => {
                self.unify(&result, expected, span)?;
                self.pattern_arguments(args, &payload, span)
            }
            ty if args.is_empty() => self.unify(&ty, expected, span),
            _ => Err(self.error(span, Some("E301"), "constructor pattern arity mismatch")),
        }
    }

    fn pattern_arguments(
        &mut self,
        args: &[Pattern],
        payload: &[Ty],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if args.len() != payload.len() {
            return Err(self.error(span, Some("E301"), "constructor pattern arity mismatch"));
        }
        for (pattern, ty) in args.iter().zip(payload) {
            self.pattern(pattern, ty)?;
        }
        Ok(())
    }

    fn condition(&mut self, node: &Node) -> Result<(), Diagnostic> {
        let saved = self.eval_effects.clone();
        self.eval_effects = self.inference.fresh_row();
        let ty = self.expression(node);
        let row = self.inference.resolve_row(&self.eval_effects);
        self.eval_effects = saved;
        let ty = ty?;
        self.expect(&Ty::Bool, &ty, node.span, None)?;
        if row.has_known_effects() {
            return Err(self.error(
                node.span,
                Some("E308"),
                "effects are not allowed in branch conditions",
            ));
        }
        self.row_constraints.push(RowConstraint {
            row,
            bound: Row::pure(),
            code: "E308",
            span: Some(node.span),
        });
        Ok(())
    }

    fn branch_result(&mut self, node: &Node, expected: &Ty) -> Result<(), Diagnostic> {
        let ty = self.expression(node)?;
        self.unify(&ty, expected, node.span)
    }

    /// §15.1: at a site whose expectation is determined, the message names the
    /// pair instead of the two types as equals. `expected_at` is where the
    /// program spells the expectation out, for the related position (§56): the
    /// declared return type, or the first arm of a conditional.
    fn expect(
        &mut self,
        expected: &Ty,
        found: &Ty,
        span: Span,
        expected_at: Option<Span>,
    ) -> Result<(), Diagnostic> {
        self.inference.unify(expected, found).map_err(|error| {
            let code = match error {
                TypeError::Mismatch => "E305",
                TypeError::Arity => "E301",
                TypeError::Occurs => "E306",
            };
            let (expected, found) = crate::types::readable_pair(expected, found);
            let related = expected_at.filter(|at| at.line != span.line || at.col != span.col);
            self.error_related(
                span,
                Some(code),
                &format!("expected {expected}, found {found}"),
                related,
            )
        })
    }

    fn branch_result_expect(
        &mut self,
        node: &Node,
        expected: &Ty,
        expected_at: Option<Span>,
    ) -> Result<(), Diagnostic> {
        let ty = self.expression(node)?;
        self.expect(expected, &ty, node.span, expected_at)
    }

    fn conditions(
        &mut self,
        arms: &[(Node, Node)],
        fallback: Option<&Node>,
    ) -> Result<Ty, Diagnostic> {
        let result = self.inference.fresh();
        let first = arms.first().map(|(_, body)| body.span);
        for (condition, body) in arms {
            self.condition(condition)?;
            self.branch_result_expect(body, &result, first)?;
        }
        if let Some(body) = fallback {
            self.branch_result_expect(body, &result, first)?;
        }
        Ok(self.inference.resolve(&result))
    }

    fn function(&mut self, params: &[Param], body: &Node) -> Result<Ty, Diagnostic> {
        let saved = self.values.clone();
        let result = self.function_body(params, body);
        self.values = saved;
        result
    }

    fn function_body(&mut self, params: &[Param], body: &Node) -> Result<Ty, Diagnostic> {
        let saved_effects = self.eval_effects.clone();
        self.eval_effects = self.inference.fresh_row();
        let mut args = Vec::new();
        for param in params {
            let ty = match &param.ty {
                Some(annotation) => self.annotation(annotation, param.span)?,
                None => self.inference.fresh(),
            };
            if param.name != "_" {
                self.values
                    .insert(param.name.clone(), Scheme::monomorphic(ty.clone()));
            }
            args.push(ty);
        }
        let result = self.expression(body);
        let row = self.inference.resolve_row(&self.eval_effects);
        self.eval_effects = saved_effects;
        Ok(Ty::fn_with(args, result?, row))
    }

    fn pipe(&mut self, value: &Node, target: &Node) -> Result<Ty, Diagnostic> {
        let (callee, mut args) = match &target.kind {
            NodeKind::Apply(callee, args) => (callee.as_ref(), args.clone()),
            _ => (target, Vec::new()),
        };
        args.insert(0, value.clone());
        self.apply(callee, &args, value.span)
    }

    fn callee(&mut self, node: &Node) -> Result<Ty, Diagnostic> {
        if let NodeKind::Name(name) = &node.kind {
            if name.starts_with("__") {
                if !self
                    .trusted_start
                    .is_some_and(|line| node.span.line >= line)
                    && !crate::is_stdlib_source(self.path)
                {
                    return Err(self.error(
                        node.span,
                        Some("E202"),
                        "intrinsics are restricted to stdlib",
                    ));
                }
                let function = crate::runtime_abi::RuntimeFunction::from_intrinsic(name)
                    .ok_or_else(|| self.error(node.span, Some("E204"), "unknown intrinsic"))?;
                if function.is_polymorphic() {
                    use crate::runtime_abi::RuntimeFunction;
                    let element = self.inference.fresh();
                    let array = Ty::Adt(crate::mono::ARRAY.into(), vec![element.clone()]);
                    return Ok(match function {
                        RuntimeFunction::ArrayLength => Ty::fn_of(vec![array], Ty::Int),
                        RuntimeFunction::ArrayGet => Ty::fn_of(vec![array, Ty::Int], element),
                        RuntimeFunction::ArrayAlloc => Ty::fn_of(vec![Ty::Int], array),
                        RuntimeFunction::ArrayFillUnchecked => {
                            Ty::fn_of(vec![array.clone(), Ty::Int, element], array)
                        }
                        _ => unreachable!(),
                    });
                }
                let signature = function.value_signature().ok_or_else(|| {
                    self.error(node.span, Some("E204"), "intrinsic has no value signature")
                })?;
                return Ok(Ty::fn_of(
                    signature.parameters.iter().map(Ty::from).collect(),
                    Ty::from(&signature.result),
                ));
            }
        }
        self.expression(node)
    }

    fn apply(&mut self, callee: &Node, args: &[Node], span: Span) -> Result<Ty, Diagnostic> {
        if let NodeKind::Name(name) = &callee.kind {
            if self.constructors.contains_key(name) {
                return self.construct(name, args, callee.span);
            }
        }
        let function = self.callee(callee)?;
        let argument_types = args
            .iter()
            .map(|arg| self.expression(arg))
            .collect::<Result<Vec<_>, _>>()?;
        let recursive = if let Ty::Var(id) = &function {
            self.recursive_calls
                .get_mut(id)
                .map(|calls| calls.push((argument_types.clone(), span)))
                .is_some()
        } else {
            false
        };
        if self.inference.resolve(&function) == Ty::Error {
            return Ok(Ty::Error);
        }
        let result = self.inference.fresh();
        let row = self.inference.fresh_row();
        // §20: a definition that omits its row gets an implicit effect variable,
        // so a call to a recursive definition must not tie that definition's row
        // to the effects of this one call. The call's effects are their own row,
        // and they reach the definition's row through the body's row, which is
        // what the definition unifies with at the end of its group.
        let callee_row = if recursive {
            self.inference.fresh_row()
        } else {
            row.clone()
        };
        self.unify(
            &function,
            &Ty::fn_with(argument_types.clone(), result.clone(), callee_row),
            span,
        )
        .map_err(|mut error| {
            if recursive && error.code.as_deref() == Some("E306") {
                error.code = Some("E307".into());
                error.message = "recursive calls change the argument type".into();
            } else if matches!(error.code.as_deref(), Some("E305" | "E306")) {
                let (anchor, pair) =
                    self.application_mismatch(&function, &argument_types, args, callee.span);
                error.line = anchor.line;
                error.col = anchor.col;
                // §15.1: at an argument the roles are certain, so the message
                // names the pair instead of the two function types.
                if error.code.as_deref() == Some("E305") {
                    if let Some((parameter, argument)) = pair {
                        let (expected, found) = crate::types::readable_pair(&parameter, &argument);
                        error.message = format!("expected {expected}, found {found}");
                    }
                }
            }
            error
        })?;
        self.add_effects(&row);
        Ok(self.inference.resolve(&result))
    }

    /// §15.1: the argument a mismatch is at, with its parameter's type and its
    /// own type, so the message can name that pair instead of the two function
    /// types. The probe is a copy of the inference, and the pair is resolved
    /// before the unification it is about is attempted, so a failed
    /// unification's partial state is never printed.
    fn application_mismatch(
        &self,
        function: &Ty,
        types: &[Ty],
        args: &[Node],
        fallback: Span,
    ) -> (Span, Option<(Ty, Ty)>) {
        let mut probe = self.inference.clone();
        let parameters: Vec<_> = types.iter().map(|_| probe.fresh()).collect();
        let result = probe.fresh();
        let row = probe.fresh_row();
        if probe
            .unify(function, &Ty::fn_with(parameters.clone(), result, row))
            .is_err()
        {
            return (fallback, None);
        }
        for ((parameter, actual), argument) in parameters.iter().zip(types).zip(args) {
            let expected = probe.resolve(parameter);
            let found = probe.resolve(actual);
            if probe.unify(&expected, &found).is_err() {
                return (argument.span, Some((expected, found)));
            }
        }
        (fallback, None)
    }

    fn construct(&mut self, name: &str, args: &[Node], span: Span) -> Result<Ty, Diagnostic> {
        if self.named_constructors.contains_key(name) {
            for arg in args {
                self.expression(arg)?;
            }
            return Err(self.error(span, Some("E301"), "constructor requires named fields"));
        }
        let scheme = self.constructors[name].clone();
        if scheme.body() == &Ty::Error {
            for argument in args {
                self.expression(argument)?;
            }
            return Ok(Ty::Error);
        }
        // Constructor schemes describe payloads internally; source expressions yield only data.
        let (payloads, result) = match self.inference.instantiate(&scheme) {
            Ty::Fn(payloads, result, _) => (payloads, result),
            result => (Vec::new(), Box::new(result)),
        };
        if args.len() != payloads.len() {
            return Err(self.error(span, Some("E301"), "constructor arity mismatch"));
        }
        for (arg, payload) in args.iter().zip(payloads) {
            let actual = self.expression(arg)?;
            self.unify(&actual, &payload, span).map_err(|mut error| {
                if matches!(error.code.as_deref(), Some("E305" | "E306")) {
                    error.line = arg.span.line;
                    error.col = arg.span.col;
                }
                error
            })?;
        }
        let result = self.inference.resolve(&result);
        let key = (span.line, span.col);
        self.value_types.insert(key, result.clone());
        self.note_type(key, &result, false);
        Ok(result)
    }

    fn propagation(
        &mut self,
        pattern: Option<&Pattern>,
        value: &Node,
        body: &Node,
    ) -> Result<Ty, Diagnostic> {
        let saved_effects = self.eval_effects.clone();
        self.eval_effects = self.inference.fresh_row();
        let container = self.expression(value);
        let rhs_row = self.inference.resolve_row(&self.eval_effects);
        self.eval_effects = saved_effects;
        let container = container?;
        self.add_effects(&rhs_row);
        if self.inference.resolve(&container) == Ty::Error {
            let saved = self.values.clone();
            let checked = self.propagation_body(pattern, &Ty::Error, body);
            self.values = saved;
            checked?;
            return Ok(Ty::Error);
        }
        if pattern.is_some() {
            return Err(self.error(
                value.span,
                Some("E303"),
                "`let p? =` is removed; absence is an effect (bind with `e as p ->`)",
            ));
        }
        if self.option_container(&container) {
            return Err(self.error(value.span, Some("E303"), "`e ->` requires an effect"));
        }
        if !rhs_row.has_known_effects() {
            self.row_constraints.push(RowConstraint {
                row: rhs_row,
                bound: Row::pure(),
                code: "E303",
                span: Some(value.span),
            });
        }
        self.expression(body)
    }

    fn option_container(&self, ty: &Ty) -> bool {
        matches!(self.inference.resolve(ty), Ty::Adt(name, args) if name == "Option" && args.len() == 1)
    }

    fn constructor_groups(&self) -> HashMap<String, Vec<(String, usize)>> {
        let mut groups: HashMap<String, Vec<(String, usize)>> = HashMap::new();
        for (name, scheme) in &self.constructors {
            let (result, arity) = match scheme.body() {
                Ty::Fn(args, result, _) => (result.as_ref(), args.len()),
                ty => (ty, 0),
            };
            if let Ty::Adt(result, _) = result {
                groups
                    .entry(result.clone())
                    .or_default()
                    .push((name.clone(), arity));
            }
        }
        groups
    }

    fn check_irrefutable(&mut self, pattern: &Pattern) {
        let groups = self.constructor_groups();
        let families = groups
            .values()
            .flat_map(|group| group.iter().map(|(name, _)| (name.clone(), group.clone())))
            .collect();
        let expanded =
            crate::named_patterns::positional(pattern, &self.named_constructors, &str::to_owned);
        if crate::coverage::covers_pattern(&expanded, &families) != Some(true) {
            self.deferred.push(self.error(
                pattern.span,
                Some("E402"),
                "binding pattern does not cover its type",
            ));
        }
    }

    fn propagation_body(
        &mut self,
        pattern: Option<&Pattern>,
        payload: &Ty,
        body: &Node,
    ) -> Result<Ty, Diagnostic> {
        if let Some(pattern) = pattern {
            self.pattern(pattern, payload)?;
            self.check_irrefutable(pattern);
        }
        self.expression(body)
    }

    fn binding(&mut self, pat: &Pattern, value: &Node, body: &Node) -> Result<Ty, Diagnostic> {
        let ty = self.expression(value)?;
        let saved = self.values.clone();
        let result = self.pattern(pat, &ty).and_then(|()| {
            self.check_irrefutable(pat);
            self.expression(body)
        });
        self.values = saved;
        result
    }

    fn annotation(&mut self, annotation: &TypeExpr, span: Span) -> Result<Ty, Diagnostic> {
        self.annotation_inner(annotation, span, &HashMap::new(), &[], &mut Vec::new())
    }

    fn annotation_inner(
        &mut self,
        annotation: &TypeExpr,
        span: Span,
        parameters: &HashMap<String, Ty>,
        rejected: &[Span],
        visiting: &mut Vec<String>,
    ) -> Result<Ty, Diagnostic> {
        if rejected.iter().any(|reference| {
            reference.line == annotation.span.line && reference.col == annotation.span.col
        }) {
            return Ok(Ty::Error);
        }
        let (name, arguments) = match &annotation.kind {
            TypeKind::Name(name) => (name, &[][..]),
            TypeKind::Apply(name, args) => (name, args.as_slice()),
            _ => return Err(self.error(span, None, "type annotation is not implemented")),
        };
        if name
            .split_once('.')
            .is_some_and(|(module, _)| self.ambiguous_modules.contains(module))
            || self.ambiguous_types.contains(name)
        {
            return Ok(Ty::Error);
        }
        if name == "Fn" && arguments.len() == 2 {
            let inputs = match &arguments[0].kind {
                TypeKind::Group(inputs) => inputs.as_slice(),
                _ => &arguments[..1],
            };
            let inputs = inputs
                .iter()
                .map(|input| self.annotation_inner(input, span, parameters, rejected, visiting))
                .collect::<Result<_, _>>()?;
            let output =
                self.annotation_inner(&arguments[1], span, parameters, rejected, visiting)?;
            let row = self.row_annotation(&arguments[1].effects)?;
            return Ok(Ty::fn_with(inputs, output, row));
        }
        let args = arguments
            .iter()
            .map(|arg| self.annotation_inner(arg, span, parameters, rejected, visiting))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(ty) = parameters.get(name) {
            return match ty {
                Ty::Abstract(public, _) => {
                    if self
                        .abstract_types
                        .values()
                        .any(|(arity, name)| name == public && *arity != args.len())
                    {
                        return Err(self.error(
                            annotation.span,
                            None,
                            "type argument count differs",
                        ));
                    }
                    Ok(Ty::Abstract(public.clone(), args))
                }
                Ty::Error => Ok(Ty::Error),
                _ if args.is_empty() => Ok(ty.clone()),
                _ => Err(self.error(span, None, "type parameter cannot take arguments")),
            };
        }
        let prefix = format!("{}.", self.module_path.join("."));
        let alias_name = name.strip_prefix(&prefix).unwrap_or(name);
        if let Some((formal, target, module)) = self.aliases.get(alias_name).cloned() {
            if formal.len() != args.len() {
                return Err(self.error(span, None, "type argument count differs"));
            }
            if visiting.iter().any(|name| name == alias_name) {
                return Err(self.error(module, Some("E602"), "cyclic abstract type instantiation"));
            }
            let substituted = formal
                .iter()
                .zip(args)
                .map(|(formal, arg)| (formal.name.clone(), arg))
                .collect();
            visiting.push(alias_name.to_owned());
            let result =
                self.annotation_inner(&target, target.span, &substituted, rejected, visiting);
            visiting.pop();
            return result;
        }
        if let Some((arity, qualified)) = self.abstract_types.get(name) {
            if *arity == args.len() {
                return Ok(self.reveal_shared(&Ty::Abstract(qualified.clone(), args)));
            }
        }
        if self.type_arities.get(name) == Some(&args.len()) {
            return Ok(Ty::Adt(name.clone(), args));
        }
        if !args.is_empty() {
            return Err(self.error(span, None, "type annotation is not implemented"));
        }
        crate::types::builtin_type(name).ok_or_else(|| {
            self.error(
                annotation.name_span,
                Some("E204"),
                &format!("unknown type {name}"),
            )
        })
    }

    fn default_numbers(&mut self) -> Result<(), Diagnostic> {
        for (op, operand, result, span, site) in std::mem::take(&mut self.divisions) {
            let checked = self.default_division(op, &operand, &result, span, &site);
            self.recover_inference_error(checked, ())?;
        }
        for (ty, span) in std::mem::take(&mut self.numeric_constraints) {
            let checked = match self.inference.resolve(&ty) {
                Ty::Var(_) => self.unify(&ty, &Ty::Int, span),
                Ty::Int | Ty::Float | Ty::Error => Ok(()),
                _ => Err(self.error(span, Some("E305"), "expected Int or Float")),
            };
            self.recover_inference_error(checked, ())?;
        }
        Ok(())
    }

    fn default_division(
        &mut self,
        op: BinOp,
        operand: &Ty,
        result: &Ty,
        span: Span,
        site: &Row,
    ) -> Result<(), Diagnostic> {
        if matches!(self.inference.resolve(operand), Ty::Var(_)) {
            self.unify(operand, &Ty::Int, span)?;
        }
        let operand = self.inference.resolve(operand);
        if matches!(op, BinOp::Div | BinOp::Mod) && operand == Ty::Int {
            self.add_error_effect(site);
        }
        let expected = self.numeric_binary(op, &operand, span)?;
        self.unify(result, &expected, span)
    }

    fn error_row() -> Row {
        Row::Closed(vec![Ty::Adt("Error".into(), vec![])])
    }

    fn add_error_effect(&mut self, site: &Row) {
        match self.inference.resolve_row(site) {
            Row::Var(id) | Row::Open(id, _) => {
                let _ = self.inference.unify_row(&Row::Var(id), &Self::error_row());
            }
            Row::Closed(_) => {}
        }
    }

    fn numeric(&mut self, ty: &Ty, span: Span) -> Result<(), Diagnostic> {
        match self.inference.resolve(ty) {
            Ty::Int | Ty::Float | Ty::Error => Ok(()),
            Ty::Var(_) => {
                self.numeric_constraints.push((ty.clone(), span));
                Ok(())
            }
            _ => Err(self.error(span, Some("E305"), "expected Int or Float")),
        }
    }

    fn binary(&mut self, op: BinOp, a: &Node, b: &Node) -> Result<Ty, Diagnostic> {
        let left = self.expression(a)?;
        let right = self.expression(b)?;
        self.unify(&left, &right, a.span)?;
        match op {
            BinOp::Eq | BinOp::Ne => {
                let span = self
                    .equality_anchors
                    .get(&(a.span.line, a.span.col, b.span.line, b.span.col))
                    .copied()
                    .unwrap_or(a.span);
                self.comparison_constraints.push(ComparisonConstraint {
                    ty: left.clone(),
                    code: "E703",
                    span: Some(span),
                });
                Ok(Ty::Bool)
            }
            BinOp::And | BinOp::Or => {
                self.unify(&left, &Ty::Bool, a.span)?;
                Ok(Ty::Bool)
            }
            _ => {
                let result = self.numeric_binary(op, &left, a.span)?;
                if matches!(op, BinOp::Div | BinOp::Mod) && self.inference.resolve(&left) == Ty::Int
                {
                    self.add_effects(&Self::error_row());
                }
                Ok(result)
            }
        }
    }

    fn record_comparisons(&mut self, mut constraints: Vec<ComparisonConstraint>, span: Span) {
        for constraint in &mut constraints {
            constraint.span.get_or_insert(span);
        }
        self.comparison_constraints.extend(constraints);
    }

    fn record_row_constraints(&mut self, mut constraints: Vec<RowConstraint>, span: Span) {
        for constraint in &mut constraints {
            constraint.span.get_or_insert(span);
        }
        self.row_constraints.extend(constraints);
    }

    fn check_realization_cycles(&mut self, items: &[Item]) {
        let graph: HashMap<_, _> = self
            .abstract_representations
            .iter()
            .map(|(name, representation)| {
                let mut references = std::collections::BTreeSet::new();
                representation.body.abstract_names(&mut references);
                (name.clone(), references)
            })
            .collect();
        for item in items {
            let Item::ModuleImpl(module) = item else {
                continue;
            };
            let prefix = format!("{}.", module.name);
            let cyclic = graph
                .keys()
                .filter(|name| name.starts_with(&prefix))
                .any(|name| realization_reaches(name, name, &graph, &mut HashSet::new()));
            if cyclic {
                self.deferred.push(self.error(
                    module.span,
                    Some("E602"),
                    "cyclic abstract type instantiation",
                ));
            }
        }
    }

    fn contains_abstract(&self, ty: &Ty, visited: &mut Vec<Ty>) -> bool {
        if matches!(ty, Ty::Abstract(..)) {
            return true;
        }
        if visited.contains(ty) {
            return false;
        }
        visited.push(ty.clone());
        match ty {
            Ty::Fn(args, result, _) => {
                return args.iter().any(|arg| self.contains_abstract(arg, visited))
                    || self.contains_abstract(result, visited)
            }
            Ty::Adt(_, args) if args.iter().any(|arg| self.contains_abstract(arg, visited)) => {
                return true
            }
            _ => {}
        }
        let Ty::Adt(name, _) = ty else {
            return false;
        };
        self.constructors
            .values()
            .chain(self.records.values().map(|(_, scheme)| scheme))
            .chain(self.module_constructors.values().map(|(_, scheme)| scheme))
            .chain(self.module_records.values().map(|(_, scheme)| scheme))
            .any(|scheme| {
                let mut inference = self.inference.clone();
                let instantiated = inference.instantiate(scheme);
                let Ty::Fn(payload, result, _) = instantiated else {
                    return false;
                };
                if !matches!(&*result, Ty::Adt(result, _) if result == name)
                    || inference.unify(&result, ty).is_err()
                {
                    return false;
                }
                payload.iter().any(|field| {
                    self.contains_abstract(&self.reveal_shared(&inference.resolve(field)), visited)
                })
            })
    }

    fn contains_function(&self, ty: &Ty, visited: &mut Vec<Ty>) -> bool {
        if matches!(ty, Ty::Fn(..)) {
            return true;
        }
        if visited.contains(ty) {
            return false;
        }
        visited.push(ty.clone());
        match ty {
            Ty::Adt(_, args) if args.iter().any(|arg| self.contains_function(arg, visited)) => {
                return true
            }
            Ty::Abstract(name, args) => {
                return self
                    .abstract_representations
                    .get(name)
                    .is_some_and(|representation| {
                        self.contains_function(&representation.apply(args), visited)
                    })
            }
            _ => {}
        }
        let Ty::Adt(name, _) = ty else {
            return false;
        };
        self.constructors
            .values()
            .chain(self.records.values().map(|(_, scheme)| scheme))
            .chain(self.module_constructors.values().map(|(_, scheme)| scheme))
            .chain(self.module_records.values().map(|(_, scheme)| scheme))
            .any(|scheme| {
                let mut inference = self.inference.clone();
                let instantiated = inference.instantiate(scheme);
                let Ty::Fn(payload, result, _) = instantiated else {
                    return false;
                };
                if !matches!(&*result, Ty::Adt(result, _) if result == name)
                    || inference.unify(&result, ty).is_err()
                {
                    return false;
                }
                payload.iter().any(|field| {
                    self.contains_function(&self.reveal_shared(&inference.resolve(field)), visited)
                })
            })
    }

    fn numeric_binary(&mut self, op: BinOp, ty: &Ty, span: Span) -> Result<Ty, Diagnostic> {
        self.numeric(ty, span)?;
        let ty = self.inference.resolve(ty);
        if matches!(op, BinOp::Div | BinOp::Mod) && matches!(ty, Ty::Var(_)) {
            let result = self.inference.fresh();
            self.divisions
                .push((op, ty, result.clone(), span, self.eval_effects.clone()));
            return Ok(result);
        }
        match op {
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => Ok(Ty::Bool),
            BinOp::Mod if ty == Ty::Float => {
                Err(self.error(span, Some("E305"), "Float has no remainder operator"))
            }
            BinOp::Div | BinOp::Mod if ty == Ty::Int => Ok(Ty::Int),
            _ => Ok(ty.clone()),
        }
    }

    fn check_operation_defaults(&mut self, items: &[Item]) -> Result<(), Diagnostic> {
        for item in items {
            let Item::Effect(effect) = item else { continue };
            for op in &effect.ops {
                if op.kind != OpKind::Once {
                    continue;
                }
                let definition = &self.default_operations[&op.name];
                let scheme = self.module_definitions.get(definition).ok_or_else(|| {
                    self.error(
                        op.name_span,
                        Some("E305"),
                        "default operation requires a definition",
                    )
                })?;
                let actual = self.inference.instantiate(scheme);
                let Ty::Fn(args, result, row) = actual else {
                    return Err(self.error(
                        op.name_span,
                        Some("E305"),
                        "default operation requires a definition",
                    ));
                };
                let wrapped = self.inference.instantiate(&self.values[&op.name]);
                let Ty::Fn(mut payload, _, _) = wrapped else {
                    unreachable!("operation scheme")
                };
                let expected = payload.remove(0);
                let expected = if op.params.is_empty() {
                    Ty::fn_of(vec![], expected)
                } else {
                    expected
                };
                self.unify(&Ty::fn_of(args, *result), &expected, op.name_span)?;
                if !self
                    .inference
                    .row_within(&row, &self.operation_result_rows[&op.name])
                {
                    return Err(self.error(
                        op.name_span,
                        Some("E305"),
                        "default operation exceeds declared effects",
                    ));
                }
            }
        }
        Ok(())
    }

    fn register_effects(&mut self, items: &[Item], prelude_items: usize) -> Result<(), Diagnostic> {
        for item in items {
            let Item::Effect(effect) = item else { continue };
            self.register_effect(effect)?;
        }
        self.register_prelude_defaults(items, prelude_items)?;
        self.register_handlers(items);
        Ok(())
    }

    fn register_effect(&mut self, effect: &crate::parser::EffectDecl) -> Result<(), Diagnostic> {
        let parameters: HashMap<_, _> = effect
            .type_params
            .iter()
            .map(|name| (name.name.clone(), self.inference.fresh()))
            .collect();
        let template = Ty::Adt(
            effect.name.clone(),
            effect
                .type_params
                .iter()
                .map(|name| parameters[&name.name].clone())
                .collect(),
        );
        let mut ops = Vec::new();
        for op in &effect.ops {
            ops.push(self.register_effect_operation(&parameters, &template, op)?);
        }
        self.effects.insert(effect.name.clone(), ops);
        Ok(())
    }

    fn register_effect_operation(
        &mut self,
        parameters: &HashMap<String, Ty>,
        template: &Ty,
        op: &crate::parser::EffectOp,
    ) -> Result<(OpKind, String), Diagnostic> {
        let mut args = Vec::new();
        for param in &op.params {
            let annotation = param.ty.as_ref().ok_or_else(|| {
                self.error(param.span, None, "operation parameter requires a type")
            })?;
            args.push(self.payload_type(annotation, parameters, param.span)?);
        }
        let result = match &op.ret {
            Some(ty) => self.payload_type(ty, parameters, op.span)?,
            None if op.kind == OpKind::Abort => self.inference.fresh(),
            None => Ty::Void,
        };
        let result_row = match &op.ret {
            Some(ty) if ty.effects.is_some() => self.row_annotation(&ty.effects)?,
            _ => Row::pure(),
        };
        self.operation_result_rows
            .insert(op.name.clone(), result_row);
        let op_ty = if args.is_empty() {
            result
        } else {
            Ty::fn_of(args, result)
        };
        let scheme = self
            .inference
            .generalize(&Ty::fn_of(vec![op_ty], template.clone()), &[]);
        self.values.insert(op.name.clone(), scheme);
        self.operations.insert(op.name.clone(), op.kind);
        Ok((op.kind, op.name.clone()))
    }

    fn register_prelude_defaults(
        &mut self,
        items: &[Item],
        prelude_items: usize,
    ) -> Result<(), Diagnostic> {
        for item in &items[..prelude_items] {
            let Item::Effect(effect) = item else { continue };
            for op in effect.ops.iter().filter(|op| op.kind == OpKind::Once) {
                self.register_prelude_default(items, prelude_items, &effect.name, op)?;
            }
        }
        Ok(())
    }

    fn register_prelude_default(
        &mut self,
        items: &[Item],
        prelude_items: usize,
        effect_name: &str,
        op: &crate::parser::EffectOp,
    ) -> Result<(), Diagnostic> {
        let module_name = format!("{}Default", effect_name);
        let module = items[..prelude_items].iter().find_map(|item| match item {
            Item::ModuleImpl(module) if module.name == module_name => Some(module),
            _ => None,
        });
        let definition = module
            .and_then(|module| {
                module.items.iter().find_map(|item| match item {
                    Item::Def(def) if def.name == op.name => {
                        Some(format!("{}.{}", module.name, def.name))
                    }
                    _ => None,
                })
            })
            .ok_or_else(|| {
                self.error(
                    op.name_span,
                    Some("E204"),
                    "prelude once has no default definition",
                )
            })?;
        self.default_operations.insert(op.name.clone(), definition);
        Ok(())
    }

    fn register_handlers(&mut self, items: &[Item]) {
        for item in items {
            let Item::Handler(handler) = item else {
                continue;
            };
            self.register_handler(handler);
        }
    }

    fn register_handler(&mut self, handler: &crate::parser::HandlerDecl) {
        let expected = self
            .effects
            .get(&handler.effect)
            .cloned()
            .unwrap_or_default();
        let written: Vec<_> = handler
            .arms
            .iter()
            .filter_map(|arm| match arm {
                HandlerArm::Operation { kind, name, .. } => Some((*kind, name.clone())),
                _ => None,
            })
            .collect();
        if written.len() != expected.len() || written.iter().any(|op| !expected.contains(op)) {
            self.deferred.push(self.error(
                handler.span,
                Some("E605"),
                "handler operations do not match the effect",
            ));
        }
        self.handlers
            .insert(handler.name.clone(), effect_print_name(&handler.effect));
        self.handler_decls
            .insert(handler.name.clone(), handler.clone());
    }

    fn isolate_effects<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<(T, Row), Diagnostic> {
        let saved = self.eval_effects.clone();
        self.eval_effects = self.inference.fresh_row();
        let result = f(self);
        let row = self.inference.resolve_row(&self.eval_effects);
        self.eval_effects = saved;
        Ok((result?, row))
    }

    fn add_effects(&mut self, row: &Row) {
        self.eval_effects = self.inference.union_row(&self.eval_effects, row);
    }

    fn contain_row(&mut self, inner: &Row, outer: &Row, span: Span) -> Result<(), Diagnostic> {
        let inner = self.inference.resolve_row(inner);
        let outer = self.inference.resolve_row(outer);
        self.inference
            .row_contained(&inner, &outer)
            .map_err(|_| self.error(span, Some("E305"), "effect row mismatch"))?;
        if let (Row::Open(id, _), Row::Closed(_)) = (&inner, &outer) {
            self.row_constraints.push(RowConstraint {
                row: Row::Var(*id),
                bound: outer,
                code: "E305",
                span: Some(span),
            });
        }
        Ok(())
    }

    fn row_annotation(&mut self, effects: &Option<EffectRow>) -> Result<Row, Diagnostic> {
        match effects {
            None => Ok(self.inference.fresh_row()),
            Some(EffectRow::Pure) => Ok(Row::pure()),
            Some(EffectRow::Row(labels)) => {
                let mut row = Vec::new();
                for ty in labels {
                    row.push(self.effect_label(ty)?);
                }
                Ok(Row::Closed(row))
            }
        }
    }

    fn effect_label(&mut self, ty: &TypeExpr) -> Result<Ty, Diagnostic> {
        match &ty.kind {
            TypeKind::Name(name) => Ok(Ty::Adt(effect_print_name(name), vec![])),
            TypeKind::Apply(name, args) => {
                let args = args
                    .iter()
                    .map(|arg| self.annotation(arg, arg.span))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Ty::Adt(name.clone(), args))
            }
            _ => Err(self.error(ty.name_span, Some("E204"), "unknown effect")),
        }
    }

    fn unwrap_thunk(&mut self, ty: Ty) -> Ty {
        match ty {
            Ty::Fn(args, result, row) if args.is_empty() => {
                self.add_effects(&row);
                *result
            }
            ty => ty,
        }
    }

    fn with_handler(
        &mut self,
        span: Span,
        handler: &DeclaredName,
        args: &[Node],
        body: &Node,
    ) -> Result<Ty, Diagnostic> {
        // 本体と腕の型をこの `with` の位置ごとに控える。mono が腕を lower
        // するとき、同じ節点を答え型の違う別の `with` が上書きしていても、
        // この `with` の答え型で具体化できる。
        self.active_withs.push((span.line, span.col));
        let result = self.with_handler_body(handler, args, body);
        self.active_withs.pop();
        result
    }

    fn with_handler_body(
        &mut self,
        handler: &DeclaredName,
        args: &[Node],
        body: &Node,
    ) -> Result<Ty, Diagnostic> {
        let mut arg_tys = Vec::new();
        for arg in args {
            arg_tys.push(self.expression(arg)?);
        }
        let (ty, row) = self.isolate_effects(|this| this.expression(body))?;
        let Some(effect) = self.handlers.get(&handler.name).cloned() else {
            return Err(self.error(
                handler.span,
                Some("E204"),
                &format!("unbound name {}", handler.name),
            ));
        };
        self.add_effects(&row.without_effect(&effect));
        let Some(decl) = self.handler_decls.get(&handler.name).cloned() else {
            return Ok(ty);
        };
        self.handler_answer(&decl, &ty, &row, handler.span, &arg_tys, args)
    }

    fn handler_answer(
        &mut self,
        handler: &HandlerDecl,
        body: &Ty,
        row: &Row,
        span: Span,
        arg_tys: &[Ty],
        args: &[Node],
    ) -> Result<Ty, Diagnostic> {
        let answer = self.inference.fresh();
        let saved = self.values.clone();
        let result = self
            .bind_handler_params(handler, arg_tys, args, span)
            .and_then(|()| self.infer_handler_arms(handler, body, row, &answer, span));
        self.values = saved;
        result?;
        Ok(self.inference.resolve(&answer))
    }

    /// §38: `with` の実引数を handler の宣言型と照合し、値引数を全ての腕の
    /// スコープに入れる。個数は E301、型は E305（アンカーは食い違った実引数）。
    fn bind_handler_params(
        &mut self,
        handler: &HandlerDecl,
        arg_tys: &[Ty],
        args: &[Node],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if handler.params.len() != arg_tys.len() {
            return Err(self.error(span, Some("E301"), "handler argument arity mismatch"));
        }
        for ((param, arg_ty), arg) in handler.params.iter().zip(arg_tys).zip(args) {
            let declared = match &param.ty {
                Some(annotation) => self.annotation(annotation, param.span)?,
                None => self.inference.fresh(),
            };
            self.unify(&declared, arg_ty, arg.span)?;
            if param.name != "_" {
                self.values
                    .insert(param.name.clone(), Scheme::monomorphic(declared));
            }
        }
        Ok(())
    }

    fn infer_handler_arms(
        &mut self,
        handler: &HandlerDecl,
        body: &Ty,
        row: &Row,
        answer: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let mut saw_return = false;
        for arm in &handler.arms {
            match arm {
                HandlerArm::Return {
                    param,
                    body: ret,
                    span: arm_span,
                } => {
                    saw_return = true;
                    self.bind_param(param, body);
                    let ty = self.expression(ret)?;
                    self.unify(&ty, answer, *arm_span)?;
                }
                HandlerArm::Operation {
                    kind: OpKind::Again,
                    name,
                    params,
                    body: ret,
                    span: arm_span,
                    ..
                } => {
                    let (types, result) = self.operation_payload(name, row)?;
                    self.bind_operation_params(params, &types, *arm_span)?;
                    let resume = Ty::fn_with(vec![result], answer.clone(), Row::pure());
                    let previous = self.resume_type.replace(resume);
                    let ty = self.expression(ret);
                    self.resume_type = previous;
                    self.unify(&ty?, answer, *arm_span)?;
                }
                HandlerArm::Operation {
                    kind: OpKind::Abort,
                    name,
                    params,
                    body: ret,
                    span: arm_span,
                    ..
                } => {
                    if let Ok((types, _)) = self.operation_payload(name, row) {
                        self.bind_operation_params(params, &types, *arm_span)?;
                    }
                    let ty = self.expression(ret)?;
                    self.unify(&ty, answer, *arm_span)?;
                }
                HandlerArm::Operation {
                    name,
                    params,
                    body: ret,
                    span: arm_span,
                    ..
                } => {
                    if let Ok((types, result)) = self.operation_payload(name, row) {
                        self.bind_operation_params(params, &types, *arm_span)?;
                        let ty = self.expression(ret)?;
                        self.unify(&ty, &result, ret.span)?;
                    } else {
                        let _ = self.expression(ret)?;
                    }
                }
            }
        }
        if !saw_return {
            self.unify(body, answer, span)?;
        }
        Ok(())
    }

    fn bind_param(&mut self, param: &Param, ty: &Ty) {
        if param.name != "_" {
            self.values
                .insert(param.name.clone(), Scheme::monomorphic(ty.clone()));
        }
    }

    fn bind_operation_params(
        &mut self,
        params: &[Param],
        types: &[Ty],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if params.len() != types.len() {
            return Err(self.error(span, Some("E301"), "handler arm arity mismatch"));
        }
        for (param, ty) in params.iter().zip(types) {
            self.bind_param(param, ty);
        }
        Ok(())
    }

    fn operation_payload(&mut self, name: &str, row: &Row) -> Result<(Vec<Ty>, Ty), Diagnostic> {
        let scheme = self.values.get(name).cloned().ok_or_else(|| {
            self.error(
                Span::point(1, 1),
                Some("E204"),
                &format!("unbound name {name}"),
            )
        })?;
        let (ty, _, _) = self.inference.instantiate_constrained(&scheme);
        let Ty::Fn(mut args, effect, _) = ty else {
            return Ok((Vec::new(), ty));
        };
        let op_ty = args.pop().unwrap_or(*effect.clone());
        let _ = self
            .inference
            .row_contained(&Row::Closed(vec![*effect]), row);
        Ok(match op_ty {
            Ty::Fn(params, result, _) => (params, *result),
            result => (Vec::new(), result),
        })
    }

    fn unwrap_operation(&mut self, name: &str, ty: Ty) -> Ty {
        if !self.operations.contains_key(name) {
            return ty;
        }
        if !matches!(&ty, Ty::Fn(_, effect, _) if
            matches!(&**effect, Ty::Adt(name, _) if self.effects.contains_key(name)))
        {
            return ty;
        }
        let Ty::Fn(mut args, effect, _) = ty else {
            return ty;
        };
        let Some(op_ty) = args.pop() else {
            return Ty::fn_of(vec![], *effect);
        };
        let row = self.inference.union_row(
            &Row::Closed(vec![*effect]),
            &self.operation_result_rows[name],
        );
        match op_ty {
            Ty::Fn(params, result, _) => Ty::fn_with(params, *result, row),
            result => {
                self.add_effects(&row);
                result
            }
        }
    }
}

fn effect_print_name(name: &str) -> String {
    name.rsplit_once('$')
        .map_or(name, |(_, name)| name)
        .to_string()
}

fn realization_reaches(
    current: &str,
    target: &str,
    graph: &HashMap<String, std::collections::BTreeSet<String>>,
    visited: &mut HashSet<String>,
) -> bool {
    if !visited.insert(current.into()) {
        return false;
    }
    graph.get(current).is_some_and(|references| {
        references
            .iter()
            .any(|next| next == target || realization_reaches(next, target, graph, visited))
    })
}
