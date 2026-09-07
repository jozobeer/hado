use crate::parser::{Ast, BinOp, DefKind, Diagnostic, Item, Node, NodeKind, Span};
use crate::parser::{FieldName, Param, Pattern, PatternKind, TypeExpr, TypeKind};
use crate::types::{ComparisonConstraint, Inference, Scheme, Ty, TypeError};
use crate::Checked;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct Checker<'a> {
    aliases: HashMap<String, TypeExpr>,
    ambiguous_modules: HashSet<String>,
    ambiguous_signatures: HashSet<String>,
    ambiguous_types: HashSet<String>,
    ambiguous_constructors: HashSet<String>,
    abstract_representations: HashMap<String, Ty>,
    deferred: Vec<Diagnostic>,
    module_constructors: HashMap<String, (u32, Scheme)>,
    module_records: HashMap<String, (Vec<String>, Scheme)>,
    trusted_start: Option<usize>,
    module_path: Vec<String>,
    module_definitions: HashMap<String, Scheme>,
    composite_types: HashMap<(usize, usize), Ty>,
    value_types: HashMap<(usize, usize), Ty>,
    path: &'a str,
    current_definition: Option<String>,
    inference: Inference,
    values: HashMap<String, Scheme>,
    numeric_constraints: Vec<(Ty, Span)>,
    divisions: Vec<(BinOp, Ty, Ty, Span)>,
    type_arities: HashMap<String, usize>,
    constructor_tags: HashMap<String, u32>,
    constructors: HashMap<String, Scheme>,
    records: HashMap<String, (Vec<String>, Scheme)>,
    modules: HashMap<String, HashMap<String, Scheme>>,
    abstract_types: HashMap<String, (usize, String)>,
    projections: Vec<(Ty, FieldName, Ty, Span)>,
    comparison_constraints: Vec<ComparisonConstraint>,
    equality_anchors: HashMap<(usize, usize, usize, usize), Span>,
    recursive_calls: HashMap<u32, Vec<(Vec<Ty>, Span)>>,
}

pub fn check_ast(ast: &Ast, path: &str) -> Result<Checked, Diagnostic> {
    check_ast_with_prelude(ast, path, None)
}

pub(crate) fn check_ast_with_prelude(ast: &Ast, path: &str, trusted_start: Option<usize>) -> Result<Checked, Diagnostic> {
    let (checked, deferred) = infer_with_diagnostics(ast, path, trusted_start)?;
    match deferred.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(checked),
    }
}

pub(crate) fn infer_with_diagnostics(ast: &Ast, path: &str, trusted_start: Option<usize>) -> Result<(Checked, Vec<Diagnostic>), Diagnostic> {
    let mut checker = Checker {
        aliases: HashMap::new(),
        ambiguous_modules: HashSet::new(),
        ambiguous_signatures: HashSet::new(),
        ambiguous_types: HashSet::new(),
        ambiguous_constructors: HashSet::new(),
        abstract_representations: HashMap::new(),
        deferred: Vec::new(),
        module_constructors: HashMap::new(),
        module_records: HashMap::new(),
        trusted_start,
        module_path: Vec::new(),
        module_definitions: HashMap::new(),
        composite_types: HashMap::new(),
        value_types: HashMap::new(),
        path,
        current_definition: None,
        inference: Inference::default(),
        values: HashMap::new(),
        numeric_constraints: Vec::new(),
        divisions: Vec::new(),
        type_arities: HashMap::new(),
        constructor_tags: HashMap::new(),
        constructors: HashMap::new(),
        records: HashMap::new(),
        modules: HashMap::new(),
        abstract_types: HashMap::new(),
        projections: Vec::new(),
        comparison_constraints: Vec::new(),
        equality_anchors: ast.equality_anchors.iter().map(|(left, right, operator)|
            ((left.line, left.col, right.line, right.col), *operator)).collect(),
        recursive_calls: HashMap::new(),
    };
    let mut definitions = HashMap::new();
    let inferred = (|| -> Result<Ty, Diagnostic> {
        let comparable = checker.inference.fresh();
        let compare = Ty::Fn(vec![comparable.clone(), comparable.clone()], Box::new(Ty::Int));
        checker.values.insert(
            "compare".into(),
            checker.inference.generalize_constrained(&compare, &[], &[ComparisonConstraint { ty: comparable, code: "E704", span: None }]),
        );
        let ambiguous_values = checker.prepare_ambiguous_declarations(&ast.items);
        for item in &ast.items {
            if let Item::TypeDef(def) = item {
                if !checker.ambiguous_types.contains(&def.name) {
                    checker.type_arities.insert(def.name.clone(), def.params.len());
                }
            }
        }
        let signatures: HashMap<_, _> = ast
            .items
            .iter()
            .filter_map(|item| match item {
                Item::ModuleSig(sig) => Some((sig.name.clone(), sig)),
                _ => None,
            })
            .collect();
        for item in &ast.items {
            if let Item::ModuleImpl(module) = item {
                if let Some(signature) = module.sig.as_ref().and_then(|name| signatures.get(name)) {
                    checker.module_abstract_parameters(module, signature);
                }
            }
        }
        let violations = crate::type_cycles::violations(ast, path);
        for item in &ast.items {
            if let Item::TypeDef(def) = item {
                let rejected: Vec<_> = violations.iter().filter(|violation| violation.definition == def.name)
                    .map(|violation| violation.reference).collect();
                checker.register_type(def, &rejected)?;
            }
        }
        checker.deferred.extend(violations.into_iter().map(|violation| violation.diagnostic));
        for item in &ast.items {
            match item {
                Item::Def(def) => {
                    definitions.entry(def.name.clone()).or_insert(def);
                }
                Item::TypeDef(_) | Item::ModuleSig(_) | Item::ModuleImpl(_) => {}
                _ => return Err(checker.error(ast.span, None, "item inference is not implemented")),
            }
        }
        for group in crate::dependencies::inference_groups(ast) {
            let mut functions = Vec::new();
            for item in group {
                match item {
                    Item::ModuleImpl(module) => checker.module(module, &signatures)?,
                    Item::Def(def) if !ambiguous_values.contains(&def.name) => functions.push(def.name.clone()),
                    _ => {}
                }
            }
            if !functions.is_empty() { checker.definition_group(&functions, &definitions)?; }
        }
        checker.check_ambiguous_definitions(&ast.items, &ambiguous_values);
        checker.check_ambiguous_modules(&ast.items, &signatures);
        checker.resolve_projections(true)?;
        for constraint in &checker.comparison_constraints {
            if checker.contains_function(&checker.inference.resolve(&constraint.ty), &mut Vec::new()) {
                checker.deferred.push(checker.error(constraint.span.expect("comparison use has an origin"), Some(constraint.code), "values containing functions cannot be compared"));
            }
        }
        let scheme = checker
            .values
            .get("main")
            .cloned()
            .ok_or_else(|| checker.error(ast.span, Some("E206"), "main is not defined"))?;
        let main = checker.inference.instantiate(&scheme);
        if !checker.inference.resolve(&main).is_monomorphic() {
            checker.deferred.push(checker.error(definitions["main"].span, Some("E701"), "main must have a monomorphic type"));
        }
        if checker.contains_function(&checker.inference.resolve(&main), &mut Vec::new()) {
            checker.deferred.push(checker.error(definitions["main"].span, Some("E702"), "main cannot return a value containing a function"));
        }
        Ok(main)
    })();
    let main = match inferred {
        Ok(main) => main,
        Err(error) => {
            checker.deferred.push(error);
            return Err(checker.deferred.into_iter().min_by_key(crate::diagnostic_order).unwrap());
        }
    };
    Ok((Checked {
        abstract_representations: checker.abstract_representations,
        module_constructors: checker.module_constructors,
        module_records: checker.module_records,
        module_definitions: checker.module_definitions.clone(),
        modules: checker.modules.clone(),
        constructor_tags: checker.constructor_tags.clone(),
        constructors: checker.constructors.clone(),
        records: checker.records.clone(),
        composite_types: checker.composite_types.iter().map(|(span, ty)| (*span, checker.inference.resolve(ty))).collect(),
        value_types: checker.value_types.iter().map(|(span, ty)| (*span, checker.inference.resolve(ty))).collect(),
        main: checker.inference.resolve(&main),
        definitions: definitions.keys().map(|name| (name.clone(), checker.values[name].clone())).collect(),
    }, checker.deferred))
}

impl Checker<'_> {
    fn prepare_ambiguous_declarations(&mut self, items: &[Item]) -> HashSet<String> {
        self.prepare_module_ambiguities(items);
        let ambiguous = crate::ambiguous_declarations::AmbiguousDeclarations::collect(items, self.path);
        for name in &ambiguous.declared_types {
            self.ambiguous_types.remove(name);
            self.type_arities.remove(name);
            self.aliases.remove(name);
            self.records.remove(name);
        }
        for name in &ambiguous.declared_constructors {
            if self.ambiguous_constructors.remove(name) {
                self.constructors.remove(name);
                self.constructor_tags.remove(name);
            }
        }
        self.ambiguous_types.extend(ambiguous.types);
        self.ambiguous_constructors.extend(ambiguous.constructors);
        for name in &ambiguous.values {
            self.values.insert(name.clone(), Scheme::monomorphic(Ty::Error));
        }
        for name in &self.ambiguous_constructors {
            self.constructors.insert(name.clone(), Scheme::monomorphic(Ty::Error));
        }
        self.deferred.extend(ambiguous.diagnostics);
        ambiguous.values
    }

    fn prepare_module_ambiguities(&mut self, items: &[Item]) {
        let ambiguous = crate::ambiguous_declarations::AmbiguousDeclarations::collect(items, self.path);
        self.ambiguous_signatures.retain(|name| !ambiguous.declared_signatures.contains(name));
        self.ambiguous_signatures.extend(ambiguous.signatures);
        self.ambiguous_modules.retain(|name| !ambiguous.declared_modules.contains(name));
        self.ambiguous_modules.extend(ambiguous.modules);
        for item in items {
            if let Item::ModuleImpl(module) = item {
                if module.sig.as_ref().is_some_and(|name| self.ambiguous_signatures.contains(name)) {
                    self.ambiguous_modules.insert(module.name.clone());
                }
            }
        }
    }

    fn check_ambiguous_definitions(&mut self, items: &[Item], ambiguous: &HashSet<String>) {
        for item in items {
            let Item::Def(def) = item else { continue };
            if !ambiguous.contains(&def.name) { continue; }
            let saved = self.clone();
            self.current_definition = Some(def.name.clone());
            let result = self.definition(def);
            let mut diagnostics = self.deferred.split_off(saved.deferred.len());
            if let Err(error) = result { diagnostics.push(error); }
            *self = saved;
            self.deferred.extend(diagnostics);
        }
    }

    fn check_ambiguous_modules(&mut self, items: &[Item], signatures: &HashMap<String, &crate::parser::ModuleSig>) {
        for item in items {
            let Item::ModuleImpl(module) = item else { continue };
            if !self.ambiguous_modules.contains(&module.name) { continue; }
            let saved = self.clone();
            self.module_path.push(module.name.clone());
            let ast = Ast { items: module.items.clone(), span: module.span,
                branches_after_fallback: Vec::new(), branch_anchors: Vec::new(), equality_anchors: Vec::new() };
            let result = self.infer_module_items(&ast, signatures);
            let mut diagnostics = self.deferred.split_off(saved.deferred.len());
            if let Err(error) = result { diagnostics.push(error); }
            *self = saved;
            self.deferred.extend(diagnostics);
        }
    }

    fn module(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<(), Diagnostic> {
        if self.ambiguous_modules.contains(&module.name) { return Ok(()); }
        let name = module
            .sig
            .as_ref()
            .ok_or_else(|| self.error(module.span, Some("E113"), "module requires a signature"))?;
        let signature = signatures
            .get(name)
            .ok_or_else(|| self.error(module.sig_span, Some("E204"), "unknown signature"))?;
        let saved = self.clone();
        let saved_value_spans: std::collections::HashSet<_> = self.value_types.keys().copied().collect();
        let saved_composite_spans: std::collections::HashSet<_> = self.composite_types.keys().copied().collect();
        self.module_path.push(module.name.clone());
        let exports = match self.module_body(module, signature, signatures) {
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
        {
            let type_names: HashMap<_, _> = module.items.iter().filter_map(|item| match item {
                Item::TypeDef(definition) => Some((definition.name.clone(), format!("{}.{}", self.module_path.join("."), definition.name))),
                _ => None,
            }).collect();
            let prefix = format!("{}.", self.module_path.join("."));
            for (name, representation) in &mut self.abstract_representations {
                if name.starts_with(&prefix) {
                    *representation = representation.qualify_data_types(&type_names);
                }
            }
            for (name, scheme) in &mut self.module_definitions {
                if name.starts_with(&prefix) {
                    *scheme = scheme.qualify_data_types(&type_names);
                }
            }
            for (span, ty) in &mut self.value_types {
                if !saved_value_spans.contains(span) {
                    *ty = self.inference.resolve(ty).qualify_data_types(&type_names);
                }
            }
            for (span, ty) in &mut self.composite_types {
                if !saved_composite_spans.contains(span) {
                    *ty = self.inference.resolve(ty).qualify_data_types(&type_names);
                }
            }
            for item in &module.items {
                if let Item::TypeDef(definition) = item {
                    if matches!(definition.ty.as_ref().map(|ty| &ty.kind), Some(TypeKind::Record(_))) {
                        if let Some(record) = self.records.get(&definition.name) {
                            self.module_records.insert(format!("{}.{}", self.module_path.join("."), definition.name), (record.0.clone(), record.1.qualify_data_types(&type_names)));
                        }
                    } else {
                        for (name, scheme) in &self.constructors {
                            let result = match scheme.body() { Ty::Fn(_, result) => result.as_ref(), ty => ty };
                            if matches!(result, Ty::Adt(type_name, _) if type_name == &definition.name) {
                                self.module_constructors.insert(format!("{}.{}", self.module_path.join("."), name), (self.constructor_tags[name], scheme.qualify_data_types(&type_names)));
                            }
                        }
                    }
                }
            }
        }
        self.module_path.pop();
        self.modules = saved.modules;
        self.abstract_types = saved.abstract_types;
        self.type_arities = saved.type_arities;
        self.aliases = saved.aliases;
        self.ambiguous_types = saved.ambiguous_types;
        self.ambiguous_modules = saved.ambiguous_modules;
        self.ambiguous_signatures = saved.ambiguous_signatures;
        self.ambiguous_constructors = saved.ambiguous_constructors;
        self.records = saved.records;
        self.constructors = saved.constructors;
        self.constructor_tags = saved.constructor_tags;
        self.values = saved.values;
        self.modules.insert(module.name.clone(), exports);
        self.module_abstract_parameters(module, signature);
        Ok(())
    }

    fn module_abstract_parameters(
        &mut self, module: &crate::parser::ModuleImpl, signature: &crate::parser::ModuleSig,
    ) -> HashMap<String, Ty> {
        if self.ambiguous_modules.contains(&module.name) { return HashMap::new(); }
        let mut parameters = HashMap::new();
        for item in &signature.items {
            if let Item::TypeDef(def) = item {
                let local = format!("{}.{}", module.name, def.name);
                let qualified = if self.module_path.is_empty() { local.clone() } else { format!("{}.{}", self.module_path.join("."), local) };
                self.abstract_types.insert(local, (def.params.len(), qualified.clone()));
                parameters.insert(def.name.clone(), Ty::Abstract(qualified, vec![]));
            }
        }
        parameters
    }

    fn module_signature_exports(
        &mut self, module: &crate::parser::ModuleImpl, signature: &crate::parser::ModuleSig,
    ) -> Result<HashMap<String, Scheme>, Diagnostic> {
        let abstracts = self.module_abstract_parameters(module, signature);
        let mut exports = HashMap::new();
        for item in &signature.items {
            if let Item::SigDef(member) = item {
                let mut parameters = abstracts.clone();
                for parameter in &member.type_params {
                    parameters.insert(parameter.name.clone(), self.inference.fresh());
                }
                let ty = self.signature_type(member, &parameters)?;
                exports.insert(member.name.clone(), self.inference.generalize(&ty, &[]));
            }
        }
        Ok(exports)
    }

    fn infer_module_items<'a>(
        &mut self, ast: &'a Ast, signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<(HashMap<String, &'a crate::parser::Def>, HashSet<String>), Diagnostic> {
        crate::type_cycles::check(ast, self.path)?;
        self.prepare_module_ambiguities(&ast.items);
        let mut signatures = signatures.clone();
        for item in &ast.items {
            if let Item::ModuleSig(signature) = item {
                signatures.insert(signature.name.clone(), signature);
            }
        }
        for item in &ast.items {
            if let Item::ModuleImpl(inner) = item {
                self.module(inner, &signatures)?;
            }
        }
        let ambiguous_values = self.prepare_ambiguous_declarations(&ast.items);
        let mut definitions = HashMap::new();
        for item in &ast.items {
            if let Item::TypeDef(def) = item {
                if self.ambiguous_types.contains(&def.name) { continue; }
                self.type_arities.insert(def.name.clone(), def.params.len());
                self.aliases.remove(&def.name);
                if let Some(ty) = &def.ty {
                    if matches!(ty.kind, TypeKind::Name(_) | TypeKind::Apply(_, _)) {
                        self.aliases.insert(def.name.clone(), ty.clone());
                    }
                }
            }
        }
        for item in &ast.items {
            match item {
                Item::TypeDef(def) => self.register_type(def, &[])?,
                Item::Def(def) => {
                    definitions.entry(def.name.clone()).or_insert(def);
                }
                Item::ModuleImpl(_) | Item::ModuleSig(_) => {}
                _ => {
                    return Err(self.error(
                        ast.span,
                        None,
                        "unexpected signature member in module",
                    ))
                }
            }
        }
        for group in crate::names::definition_groups(ast) {
            let group: Vec<_> = group.into_iter().filter(|name| !ambiguous_values.contains(name)).collect();
            if !group.is_empty() { self.definition_group(&group, &definitions)?; }
        }
        self.check_ambiguous_definitions(&ast.items, &ambiguous_values);
        for name in definitions.keys() {
            self.module_definitions.insert(format!("{}.{}", self.module_path.join("."), name), self.values[name].clone());
        }
        self.check_ambiguous_modules(&ast.items, &signatures);
        Ok((definitions, ambiguous_values))
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
            branches_after_fallback: Vec::new(),
            branch_anchors: Vec::new(),
            equality_anchors: Vec::new(),
        };
        let (definitions, ambiguous_values) = self.infer_module_items(&ast, signatures)?;
        let mut abstract_names = HashMap::new();
        for item in &signature.items {
            if let Item::TypeDef(def) = item {
                if !ast.items.iter().any(|item| matches!(item, Item::TypeDef(actual) if actual.name == def.name && actual.params.len() == def.params.len())) {
                    return Err(self.error(module.span, Some("E602"), "missing signature type"));
                }
                if def.ty.is_some() {
                    return Err(self.error(
                        def.span,
                        None,
                        "manifest signature types are not implemented",
                    ));
                }
                let qualified = format!("{}.{}", self.module_path.join("."), def.name);
                let reference = TypeExpr { kind: TypeKind::Name(def.name.clone()), span: def.name_span };
                let representation = self.annotation(&reference, def.name_span)?;
                self.abstract_representations.insert(qualified.clone(), representation);
                abstract_names.insert(def.name.clone(), qualified);
            }
        }
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
            if !definitions.contains_key(&declaration.name) {
                return Err(self.error(module.span, Some("E602"), "missing signature member"));
            }
            let scheme = self.values[&declaration.name].clone();
            let (mut actual, constraints) = self.inference.instantiate_constrained(&scheme);
            if !ambiguous_values.contains(&declaration.name) && self.inference.resolve(&actual).has_error() {
                let known = self.invalid_definition_type(definitions[&declaration.name]);
                actual = self.inference.freshen_errors(&known);
            }
            self.record_comparisons(constraints, declaration.span);
            let parameters: HashMap<_, _> = declaration.type_params.iter().map(|name| (name.name.clone(), self.inference.fresh())).collect();
            let expected = self.signature_type(declaration, &parameters)?;
            self.inference
                .unify(&actual, &expected)
                .map_err(|_| self.error(module.span, Some("E602"), "signature type mismatch"))?;
            let mut resolved_parameters = Vec::new();
            for parameter in parameters.values() {
                let resolved = self.inference.resolve(parameter);
                if !matches!(resolved, Ty::Var(_)) || resolved_parameters.contains(&resolved) {
                    return Err(self.error(module.span, Some("E602"), "signature implementation is not sufficiently polymorphic"));
                }
                resolved_parameters.push(resolved);
            }
            let mut public_parameters: HashMap<_, _> = abstract_names.iter().map(|(name, public)|
                (name.clone(), Ty::Abstract(public.clone(), vec![]))).collect();
            public_parameters.extend(parameters);
            let public_type = self.signature_type(declaration, &public_parameters)?;
            exports.insert(declaration.name.clone(), self.inference.generalize_constrained(&public_type, &[], &self.comparison_constraints));
        }
        let roots = exports.keys().cloned().collect::<Vec<_>>();
        if let Some(name) = crate::names::unused_definitions(&ast, &roots).first() {
            self.deferred.push(self.error(definitions[name].span, Some("E514"), &format!("unused module definition {name}")));
        }
        Ok(exports)
    }

    fn signature_type(&self, declaration: &crate::parser::SigDef, parameters: &HashMap<String, Ty>) -> Result<Ty, Diagnostic> {
        let args = declaration
            .params
            .iter()
            .map(|param| {
                let annotation = param.ty.as_ref().ok_or_else(|| {
                    self.error(param.span, None, "signature parameter requires a type")
                })?;
                self.payload_type(annotation, parameters, param.span)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = declaration.ret.as_ref().ok_or_else(|| {
            self.error(declaration.span, None, "signature requires a return type")
        })?;
        Ok(Ty::Fn(
            args,
            Box::new(self.payload_type(result, parameters, declaration.span)?),
        ))
    }

    fn definition_group(
        &mut self,
        group: &[String],
        definitions: &HashMap<String, &crate::parser::Def>,
    ) -> Result<(), Diagnostic> {
        let inherited_projections: std::collections::HashSet<_> = self.projections.iter()
            .map(|(_, field, _, _)| (field.dot.line, field.dot.col)).collect();
        let placeholders: HashMap<_, _> = group
            .iter()
            .map(|name| (name.clone(), self.inference.fresh()))
            .collect();
        for (name, ty) in &placeholders {
            if definitions[name].rec {
                let Ty::Var(id) = ty else { unreachable!("fresh recursive placeholder") };
                self.recursive_calls.insert(*id, Vec::new());
            }
            self.values
                .insert(name.clone(), Scheme::monomorphic(ty.clone()));
        }
        let mut invalid_group = false;
        for name in group {
            let def = definitions[name];
            self.current_definition = Some(name.clone());
            let removed = if !def.rec {
                self.values.remove(name)
            } else {
                None
            };
            let result = self.check_definition(def, &placeholders[name]);
            if let Some(scheme) = removed {
                self.values.insert(name.clone(), scheme);
            }
            let invalid = self.recover_inference_error(result.map(|()| false), true)?;
            invalid_group |= invalid;
            if invalid {
                self.values.insert(name.clone(), Scheme::monomorphic(self.invalid_definition_type(def)));
            }
        }
        self.current_definition = None;
        let deferred_start = self.deferred.len();
        let result = self.default_numbers();
        invalid_group |= self.recover_inference_error(result.map(|()| false), true)?;
        let result = self.resolve_projections(false);
        invalid_group |= self.recover_inference_error(result.map(|()| false), true)?;
        invalid_group |= self.deferred[deferred_start..].iter().any(|error| crate::diagnostic_order(error).0 == 3);
        if invalid_group {
            self.projections.retain(|(_, field, _, _)| inherited_projections.contains(&(field.dot.line, field.dot.col)));
            for name in group {
                self.values.insert(name.clone(), Scheme::monomorphic(self.invalid_definition_type(definitions[name])));
            }
            return Ok(());
        }
        let environment: Vec<_> = self
            .projections
            .iter()
            .flat_map(|(input, _, output, _)| [input.clone(), output.clone()])
            .collect();
        for (name, ty) in placeholders {
            self.values
                .insert(name, self.inference.generalize_constrained(&ty, &environment, &self.comparison_constraints));
        }
        Ok(())
    }

    fn check_definition(&mut self, def: &crate::parser::Def, placeholder: &Ty) -> Result<(), Diagnostic> {
        let ty = self.definition(def)?;
        if def.rec { self.check_recursive_arguments(placeholder, &ty, def.span)?; }
        self.unify(placeholder, &ty, def.span)
    }

    fn invalid_definition_type(&self, def: &crate::parser::Def) -> Ty {
        let (params, ret) = match &def.kind {
            DefKind::Function { params, ret, .. } => (params, ret.as_ref()),
            DefKind::Value(Node { kind: NodeKind::Fn { params, .. }, .. }) => (params, None),
            DefKind::Value(_) => return Ty::Error,
        };
        let args = params.iter().map(|param| param.ty.as_ref()
            .and_then(|ty| self.annotation(ty, param.span).ok()).unwrap_or(Ty::Error)).collect();
        let result = ret.and_then(|ty| self.annotation(ty, def.span).ok()).unwrap_or(Ty::Error);
        Ty::Fn(args, Box::new(result))
    }

    fn check_recursive_arguments(&self, calls: &Ty, definition: &Ty, span: Span) -> Result<(), Diagnostic> {
        let recursive_calls = match calls {
            Ty::Var(id) => self.recursive_calls.get(id),
            _ => None,
        };
        if let (Some(calls), Ty::Fn(parameters, _)) = (recursive_calls, self.inference.resolve(definition)) {
            let first = calls.iter().filter(|(arguments, _)| self.recursive_arguments_occur(arguments, &parameters))
                .map(|(_, span)| *span).min_by_key(|span| (span.line, span.col));
            if let Some(span) = first {
                return Err(self.error(span, Some("E307"), "recursive calls change the argument type"));
            }
        }
        if let (Ty::Fn(calls, _), Ty::Fn(parameters, _)) = (self.inference.resolve(calls), self.inference.resolve(definition)) {
            if self.recursive_arguments_occur(&calls, &parameters) {
                return Err(self.error(span, Some("E307"), "recursive calls change the argument type"));
            }
        }
        Ok(())
    }

    fn recursive_arguments_occur(&self, arguments: &[Ty], parameters: &[Ty]) -> bool {
        let mut inference = self.inference.clone();
        arguments.iter().zip(parameters).any(|(argument, parameter)| inference.unify(argument, parameter) == Err(TypeError::Occurs))
    }

    fn definition(&mut self, def: &crate::parser::Def) -> Result<Ty, Diagnostic> {
        match &def.kind {
            DefKind::Value(body) => self.expression(body),
            DefKind::Function { params, ret, body } => {
                let ty = self.function(params, body)?;
                if let (Some(annotation), Ty::Fn(_, result)) = (ret, &ty) {
                    let expected = self.annotation(annotation, def.span)?;
                    self.unify(result, &expected, def.span)?;
                }
                Ok(ty)
            }
        }
    }

    fn register_type(&mut self, def: &crate::parser::TypeDef, rejected: &[Span]) -> Result<(), Diagnostic> {
        if self.ambiguous_types.contains(&def.name) { return Ok(()); }
        if self.aliases.contains_key(&def.name) { return Ok(()); }
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
                .map(|field| self.registered_payload_type(&field.ty, &parameters, def.span, rejected))
                .collect::<Result<Vec<_>, _>>()?;
            let scheme = self
                .inference
                .generalize(&Ty::Fn(payload, Box::new(result)), &[]);
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
            _ => std::slice::from_ref(body),
        };
        for (tag, variant) in variants.iter().enumerate() {
            self.register_constructor(variant, &parameters, &result, def.span, rejected)?;
            let name = match &variant.kind {
                TypeKind::Name(name) | TypeKind::Apply(name, _) => name,
                _ => unreachable!("registered constructor"),
            };
            let tag = u32::try_from(tag).map_err(|_| self.error(def.span, None, "too many constructors"))?;
            self.constructor_tags.insert(name.clone(), tag);
        }
        Ok(())
    }

    fn register_constructor(
        &mut self,
        variant: &TypeExpr,
        parameters: &HashMap<String, Ty>,
        result: &Ty,
        span: Span,
        rejected: &[Span],
    ) -> Result<(), Diagnostic> {
        let (name, payload) = match &variant.kind {
            TypeKind::Name(name) => (name, Vec::new()),
            TypeKind::Apply(name, args) => (
                name,
                args.iter()
                    .map(|arg| self.registered_payload_type(arg, parameters, span, rejected))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            _ => return Err(self.error(span, None, "record definitions are not implemented")),
        };
        let ty = if payload.is_empty() {
            result.clone()
        } else {
            Ty::Fn(payload, Box::new(result.clone()))
        };
        if self.ambiguous_constructors.contains(name) { return Ok(()); }
        let scheme = self.inference.generalize(&ty, &[]);
        if self.constructors.insert(name.clone(), scheme).is_some() {
            return Err(self.error(span, None, "duplicate constructor"));
        }
        Ok(())
    }

    fn registered_payload_type(
        &mut self, expr: &TypeExpr, parameters: &HashMap<String, Ty>, span: Span, rejected: &[Span],
    ) -> Result<Ty, Diagnostic> {
        let result = self.recovering_payload_type(expr, parameters, span, rejected);
        self.recover_inference_error(result, Ty::Error)
    }

    fn payload_type(
        &self,
        expr: &TypeExpr,
        parameters: &HashMap<String, Ty>,
        span: Span,
    ) -> Result<Ty, Diagnostic> {
        self.recovering_payload_type(expr, parameters, span, &[])
    }

    fn recovering_payload_type(
        &self, expr: &TypeExpr, parameters: &HashMap<String, Ty>, span: Span, rejected: &[Span],
    ) -> Result<Ty, Diagnostic> {
        if rejected.iter().any(|reference| reference.line == expr.span.line && reference.col == expr.span.col) {
            return Ok(Ty::Error);
        }
        match &expr.kind {
            TypeKind::Apply(name, args) if name == "Fn" && args.len() == 2 => {
                let inputs = match &args[0].kind { TypeKind::Group(inputs) => inputs.as_slice(), _ => std::slice::from_ref(&args[0]) };
                Ok(Ty::Fn(inputs.iter().map(|ty| self.recovering_payload_type(ty, parameters, span, rejected)).collect::<Result<_, _>>()?, Box::new(self.recovering_payload_type(&args[1], parameters, span, rejected)?)))
            }
            TypeKind::Name(name) if parameters.contains_key(name) => Ok(parameters[name].clone()),
            TypeKind::Name(name) if self.aliases.contains_key(name) => self.annotation(expr, span),
            TypeKind::Name(name) if self.type_arities.get(name) == Some(&0) => {
                Ok(Ty::Adt(name.clone(), vec![]))
            }
            TypeKind::Apply(name, args) if self.type_arities.get(name) == Some(&args.len()) => {
                let args = args
                    .iter()
                    .map(|arg| self.recovering_payload_type(arg, parameters, span, rejected))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Ty::Adt(name.clone(), args))
            }
            _ => self.annotation(expr, span),
        }
    }

    fn error(&self, span: Span, code: Option<&str>, message: &str) -> Diagnostic {
        Diagnostic {
            path: self.path.into(),
            line: span.line,
            col: span.col,
            code: code.map(str::to_owned),
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
            self.error(span, Some(code), &format!("cannot unify {a} with {b}"))
        })
    }

    fn recover_inference_error<T>(&mut self, result: Result<T, Diagnostic>, fallback: T) -> Result<T, Diagnostic> {
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
        if matches!(node.kind, NodeKind::Apply(..) | NodeKind::Call(..) | NodeKind::Pipe(..) | NodeKind::List(..) | NodeKind::Record(..)) {
            self.composite_types.insert((node.span.line, node.span.col), ty.clone());
        }
        Ok(ty)
    }

    fn expression_inner(&mut self, node: &Node) -> Result<Ty, Diagnostic> {
        match &node.kind {
            NodeKind::Int(_) => Ok(Ty::Int),
            NodeKind::Float(_) => Ok(Ty::Float),
            NodeKind::Bool(_) => Ok(Ty::Bool),
            NodeKind::Char(_) => Ok(Ty::Char),
            NodeKind::Str(_) => Ok(Ty::String),
            NodeKind::Unit => Ok(Ty::Unit),
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
                let scheme = self
                    .values
                    .get(name)
                    .cloned()
                    .ok_or_else(|| {
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
                let (ty, constraints) = self.inference.instantiate_constrained(&scheme);
                self.value_types.insert((node.span.line, node.span.col), ty.clone());
                self.record_comparisons(constraints, node.span);
                Ok(ty)
            }
            NodeKind::Fn { params, body } => {
                let ty = self.function(params, body)?;
                self.value_types.insert((node.span.line, node.span.col), ty.clone());
                Ok(ty)
            },
            NodeKind::List(items) => self.list(items),
            NodeKind::Record(fields) => self.record(fields, node.span),
            NodeKind::Field(value, field) => {
                let ty = self.project(value, field)?;
                self.value_types.insert((node.span.line, node.span.col), ty.clone());
                Ok(ty)
            },
            NodeKind::PatternBranch {
                scrutinee,
                arms,
                fallback,
            } => self.pattern_branch(scrutinee, arms, fallback.as_deref(), node.span),
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
                self.branch_result(els, &result)?;
                Ok(self.inference.resolve(&result))
            }
            NodeKind::CondBranch { arms, fallback } => self.conditions(arms, fallback.as_deref()),
        }
    }

    fn record(&mut self, fields: &[(crate::parser::DeclaredName, Node)], span: Span) -> Result<Ty, Diagnostic> {
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
        let Ty::Fn(payload, result) = self.inference.instantiate(scheme) else {
            unreachable!()
        };
        for (name, ty) in names.iter().zip(&payload) {
            let value = &fields.iter().find(|(field, _)| &field.name == name).unwrap().1;
            self.branch_result(value, ty)?;
        }
        Ok(self.inference.resolve(&result))
    }

    fn project(&mut self, value: &Node, field: &FieldName) -> Result<Ty, Diagnostic> {
        if let NodeKind::Name(name) = &value.kind {
            if self.ambiguous_modules.contains(name) { return Ok(Ty::Error); }
            if let Some(exports) = self.modules.get(name) {
                let scheme = exports.get(&field.name).cloned().ok_or_else(|| {
                    self.error(field.span, Some("E204"), "module member is not exported")
                })?;
                let (ty, constraints) = self.inference.instantiate_constrained(&scheme);
                self.record_comparisons(constraints, field.span);
                return Ok(ty);
            }
        }
        let ty = self.expression(value)?;
        self.project_type(&ty, field, value.span)
    }

    fn resolve_projections(&mut self, final_pass: bool) -> Result<(), Diagnostic> {
        for (input, field, output, receiver) in std::mem::take(&mut self.projections) {
            if matches!(self.inference.resolve(&input), Ty::Var(_)) {
                if final_pass {
                    return Err(self.error(field.dot, None, "record type could not be inferred"));
                }
                self.projections.push((input, field, output, receiver));
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
            .filter(|(names, _)| names.iter().any(|name| name == field))
            .map(|(_, scheme)| scheme.clone())
            .collect();
        if candidates.len() != 1 {
            return Ok(());
        }
        let Ty::Fn(_, result) = self.inference.instantiate(&candidates[0]) else {
            unreachable!()
        };
        self.unify(ty, &result, span)
    }

    fn project_type(&mut self, ty: &Ty, field: &FieldName, receiver: Span) -> Result<Ty, Diagnostic> {
        if self.inference.resolve(ty) == Ty::Error { return Ok(Ty::Error); }
        if matches!(self.inference.resolve(ty), Ty::Abstract(_, _)) {
            return Err(self.error(field.dot, Some("E601"), "abstract type has no accessible fields"));
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
        let (names, scheme) = self
            .records
            .get(&name)
            .cloned()
            .ok_or_else(|| self.error(receiver, Some("E305"), "field access requires a record"))?;
        let index = names
            .iter()
            .position(|name| name == &field.name)
            .ok_or_else(|| self.error(field.span, Some("E204"), "unknown field"))?;
        let Ty::Fn(payload, result) = self.inference.instantiate(&scheme) else {
            unreachable!()
        };
        self.unify(ty, &result, field.dot)?;
        Ok(self.inference.resolve(&payload[index]))
    }

    fn list(&mut self, items: &[Node]) -> Result<Ty, Diagnostic> {
        let element = self.inference.fresh();
        for item in items {
            self.branch_result(item, &element)?;
        }
        Ok(Ty::Adt(
            "List".into(),
            vec![self.inference.resolve(&element)],
        ))
    }

    fn pattern_branch(
        &mut self,
        scrutinee: &Node,
        arms: &[(Pattern, Node)],
        fallback: Option<&Node>,
        branch_span: Span,
    ) -> Result<Ty, Diagnostic> {
        let input = self.expression(scrutinee)?;
        let result = self.inference.fresh();
        for (pattern, body) in arms {
            let saved = self.values.clone();
            let checked = self
                .pattern(pattern, &input)
                .and_then(|()| self.branch_result(body, &result));
            self.values = saved;
            checked?;
        }
        if let Some(body) = fallback {
            self.branch_result(body, &result)?;
        }
        if self.inference.resolve(&input) == Ty::Bool {
            if let Err(error) = crate::check::boolean_coverage(branch_span, arms, fallback, self.path) {
                self.deferred.push(error);
            }
        }
        if let Ty::Adt(name, _) = self.inference.resolve(&input) {
            let groups = self.constructor_groups();
            let constructors = groups.get(&name).cloned().unwrap_or_default().into_iter().map(|(name, _)| name).collect::<Vec<_>>();
            let families = groups.values().flat_map(|group| group.iter().map(|(name, _)| (name.clone(), group.clone()))).collect();
            if let Err(error) = crate::check::adt_coverage(branch_span, arms, fallback, &constructors, &families, self.path) {
                self.deferred.push(error);
            }
        }
        Ok(self.inference.resolve(&result))
    }

    fn pattern(&mut self, pattern: &Pattern, expected: &Ty) -> Result<(), Diagnostic> {
        match &pattern.kind {
            PatternKind::Underscore => Ok(()),
            PatternKind::Name(name) if name.starts_with(char::is_uppercase) => {
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
        }
    }

    fn record_pattern(
        &mut self,
        fields: &[(crate::parser::DeclaredName, Option<Pattern>)],
        expected: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if matches!(self.inference.resolve(expected), Ty::Var(_)) {
            let candidates: Vec<_> = self.records.values().filter(|(names, _)|
                fields.iter().all(|(field, _)| names.contains(&field.name))).map(|(_, scheme)| scheme.clone()).collect();
            let [scheme] = candidates.as_slice() else {
                return Err(self.error(span, Some("E304"), "record pattern fields must identify one nominal type"));
            };
            let Ty::Fn(_, result) = self.inference.instantiate(scheme) else { unreachable!() };
            self.unify(expected, &result, span)?;
        }
        for (name, pattern) in fields {
            let field = FieldName { name: name.name.clone(), dot: span, span: name.span };
            let ty = self.project_type(expected, &field, span)?;
            match pattern {
                Some(pattern) => self.pattern(pattern, &ty)?,
                None => {
                    self.values.insert(name.name.clone(), Scheme::monomorphic(ty));
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
        let scheme = self
            .constructors
            .get(name)
            .cloned()
            .ok_or_else(|| self.error(span, Some("E204"), "unknown constructor"))?;
        let ty = self.inference.instantiate(&scheme);
        if ty == Ty::Error {
            for pattern in args { self.pattern(pattern, &Ty::Error)?; }
            return Ok(());
        }
        match ty {
            Ty::Fn(payload, result) => {
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
        let ty = self.expression(node)?;
        self.unify(&ty, &Ty::Bool, node.span)
    }

    fn branch_result(&mut self, node: &Node, expected: &Ty) -> Result<(), Diagnostic> {
        let ty = self.expression(node)?;
        self.unify(&ty, expected, node.span)
    }

    fn conditions(
        &mut self,
        arms: &[(Node, Node)],
        fallback: Option<&Node>,
    ) -> Result<Ty, Diagnostic> {
        let result = self.inference.fresh();
        for (condition, body) in arms {
            self.condition(condition)?;
            self.branch_result(body, &result)?;
        }
        if let Some(body) = fallback {
            self.branch_result(body, &result)?;
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
        let mut args = Vec::new();
        for param in params {
            let ty = match &param.ty {
                Some(annotation) => self.annotation(annotation, param.span)?,
                None => self.inference.fresh(),
            };
            self.values
                .insert(param.name.clone(), Scheme::monomorphic(ty.clone()));
            args.push(ty);
        }
        let result = self.expression(body)?;
        Ok(Ty::Fn(args, Box::new(result)))
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
                if !self.trusted_start.is_some_and(|line| node.span.line >= line) && !std::path::Path::new(self.path)
                    .components()
                    .any(|part| part.as_os_str() == "stdlib")
                {
                    return Err(self.error(
                        node.span,
                        Some("E202"),
                        "intrinsics are restricted to stdlib",
                    ));
                }
                let function = crate::runtime_abi::RuntimeFunction::from_intrinsic(name)
                    .ok_or_else(|| self.error(node.span, Some("E204"), "unknown intrinsic"))?;
                let signature = function.value_signature().ok_or_else(|| self.error(node.span, Some("E204"), "intrinsic has no value signature"))?;
                return Ok(Ty::Fn(signature.parameters.iter().map(Ty::from).collect(), Box::new(Ty::from(&signature.result))));
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
        let args = args
            .iter()
            .map(|arg| self.expression(arg))
            .collect::<Result<Vec<_>, _>>()?;
        let recursive = if let Ty::Var(id) = &function {
            self.recursive_calls.get_mut(id).map(|calls| calls.push((args.clone(), span))).is_some()
        } else { false };
        if self.inference.resolve(&function) == Ty::Error { return Ok(Ty::Error); }
        let result = self.inference.fresh();
        self.unify(&function, &Ty::Fn(args, Box::new(result.clone())), span).map_err(|mut error| {
            if recursive && error.code.as_deref() == Some("E306") {
                error.code = Some("E307".into());
                error.message = "recursive calls change the argument type".into();
            }
            error
        })?;
        Ok(self.inference.resolve(&result))
    }

    fn construct(&mut self, name: &str, args: &[Node], span: Span) -> Result<Ty, Diagnostic> {
        let scheme = self.constructors[name].clone();
        if scheme.body() == &Ty::Error {
            for argument in args { self.expression(argument)?; }
            return Ok(Ty::Error);
        }
        // Constructor schemes describe payloads internally; source expressions yield only data.
        let (payloads, result) = match self.inference.instantiate(&scheme) {
            Ty::Fn(payloads, result) => (payloads, *result),
            result => (Vec::new(), result),
        };
        if args.len() != payloads.len() {
            return Err(self.error(span, Some("E301"), "constructor arity mismatch"));
        }
        for (arg, payload) in args.iter().zip(payloads) {
            let actual = self.expression(arg)?;
            self.unify(&actual, &payload, span)?;
        }
        let result = self.inference.resolve(&result);
        self.value_types.insert((span.line, span.col), result.clone());
        Ok(result)
    }

    fn propagation(
        &mut self,
        pattern: Option<&Pattern>,
        value: &Node,
        body: &Node,
    ) -> Result<Ty, Diagnostic> {
        let container = self.expression(value)?;
        if self.inference.resolve(&container) == Ty::Error {
            let saved = self.values.clone();
            let checked = self.propagation_body(pattern, &Ty::Error, body);
            self.values = saved;
            checked?;
            return Ok(Ty::Error);
        }
        if matches!(self.inference.resolve(&container), Ty::Var(_)) {
            let payload = self.inference.fresh();
            let saved = self.values.clone();
            let checked = self.propagation_body(pattern, &payload, body);
            self.values = saved;
            let result = checked?;
            let (_, input) = self.propagation_types(&result, value.span)?;
            let Ty::Adt(name, mut args) = input else { unreachable!() };
            args[0] = payload;
            self.unify(&container, &Ty::Adt(name, args), value.span)?;
            return Ok(self.inference.resolve(&result));
        }
        let (payload, result) =
            self.propagation_types(&container, value.span)?;
        let saved = self.values.clone();
        let checked = self.propagation_body(pattern, &payload, body);
        self.values = saved;
        let body_type = checked?;
        self.unify(&body_type, &result, body.span)?;
        Ok(self.inference.resolve(&result))
    }

    fn constructor_groups(&self) -> HashMap<String, Vec<(String, usize)>> {
        let mut groups: HashMap<String, Vec<(String, usize)>> = HashMap::new();
        for (name, scheme) in &self.constructors {
            let (result, arity) = match scheme.body() {
                Ty::Fn(args, result) => (result.as_ref(), args.len()),
                ty => (ty, 0),
            };
            if let Ty::Adt(result, _) = result {
                groups.entry(result.clone()).or_default().push((name.clone(), arity));
            }
        }
        groups
    }

    fn check_irrefutable(&mut self, pattern: &Pattern) {
        let groups = self.constructor_groups();
        let families = groups.values().flat_map(|group|
            group.iter().map(|(name, _)| (name.clone(), group.clone()))).collect();
        if crate::coverage::covers_pattern(pattern, &families) != Some(true) {
            self.deferred.push(self.error(pattern.span, Some("E402"), "binding pattern does not cover its type"));
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

    fn propagation_types(
        &mut self,
        container: &Ty,
        span: Span,
    ) -> Result<(Ty, Ty), Diagnostic> {
        match self.inference.resolve(container) {
            Ty::Adt(name, args) if name == "Option" && args.len() == 1 => {
                let output = self.inference.fresh();
                Ok((args[0].clone(), Ty::Adt(name, vec![output])))
            }
            Ty::Adt(name, args) if name == "Result" && args.len() == 2 => {
                let output = self.inference.fresh();
                Ok((
                    args[0].clone(),
                    Ty::Adt(name, vec![output, args[1].clone()]),
                ))
            }
            _ => Err(self.error(
                span,
                Some("E303"),
                "propagation requires Option or Result",
            )),
        }
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

    fn annotation(&self, annotation: &TypeExpr, span: Span) -> Result<Ty, Diagnostic> {
        self.annotation_inner(annotation, span, &mut Vec::new())
    }

    fn annotation_inner(&self, annotation: &TypeExpr, span: Span, visiting: &mut Vec<String>) -> Result<Ty, Diagnostic> {
        if matches!(&annotation.kind, TypeKind::Name(name) | TypeKind::Apply(name, _) if name.split_once('.').is_some_and(|(module, _)| self.ambiguous_modules.contains(module))) {
            return Ok(Ty::Error);
        }
        if matches!(&annotation.kind, TypeKind::Name(name) | TypeKind::Apply(name, _) if self.ambiguous_types.contains(name)) {
            return Ok(Ty::Error);
        }
        if let TypeKind::Name(name) = &annotation.kind {
            if let Some(target) = self.aliases.get(name) {
                if visiting.contains(name) {
                    return Err(self.error(annotation.span, None, "cyclic abstract type instantiation has no specified diagnostic"));
                }
                visiting.push(name.clone());
                let result = self.annotation_inner(target, target.span, visiting);
                visiting.pop();
                return result;
            }
        }
        match &annotation.kind {
            TypeKind::Name(name) if self.abstract_types.get(name).is_some_and(|(arity, _)| *arity == 0) => {
                Ok(Ty::Abstract(self.abstract_types[name].1.clone(), vec![]))
            }
            TypeKind::Apply(name, args) if self.abstract_types.get(name).is_some_and(|(arity, _)| *arity == args.len()) => {
                let args = args
                    .iter()
                    .map(|arg| self.annotation_inner(arg, span, visiting))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Ty::Abstract(self.abstract_types[name].1.clone(), args))
            }
            TypeKind::Apply(name, args) if name == "Fn" && args.len() == 2 => {
                let inputs = match &args[0].kind {
                    TypeKind::Group(inputs) => inputs.as_slice(),
                    _ => std::slice::from_ref(&args[0]),
                };
                let inputs = inputs
                    .iter()
                    .map(|input| self.annotation_inner(input, span, visiting))
                    .collect::<Result<Vec<_>, _>>()?;
                let output = self.annotation_inner(&args[1], span, visiting)?;
                Ok(Ty::Fn(inputs, Box::new(output)))
            }
            TypeKind::Name(name) if self.type_arities.get(name) == Some(&0) => {
                Ok(Ty::Adt(name.clone(), vec![]))
            }
            TypeKind::Apply(name, args) if self.type_arities.get(name) == Some(&args.len()) => {
                let args = args
                    .iter()
                    .map(|arg| self.annotation_inner(arg, span, visiting))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Ty::Adt(name.clone(), args))
            }
            TypeKind::Name(name) => match name.as_str() {
                "Int" => Ok(Ty::Int),
                "Float" => Ok(Ty::Float),
                "Bool" => Ok(Ty::Bool),
                "Char" => Ok(Ty::Char),
                "String" => Ok(Ty::String),
                "Unit" => Ok(Ty::Unit),
                _ => Err(self.error(annotation.span, Some("E204"), &format!("unknown type {name}"))),
            },
            _ => Err(self.error(span, None, "type annotation is not implemented")),
        }
    }

    fn default_numbers(&mut self) -> Result<(), Diagnostic> {
        for (op, operand, result, span) in std::mem::take(&mut self.divisions) {
            let checked = self.default_division(op, &operand, &result, span);
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

    fn default_division(&mut self, op: BinOp, operand: &Ty, result: &Ty, span: Span) -> Result<(), Diagnostic> {
        if matches!(self.inference.resolve(operand), Ty::Var(_)) {
            self.unify(operand, &Ty::Int, span)?;
        }
        let operand = self.inference.resolve(operand);
        let expected = self.numeric_binary(op, &operand, span)?;
        self.unify(result, &expected, span)
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
                let span = self.equality_anchors.get(&(a.span.line, a.span.col, b.span.line, b.span.col)).copied().unwrap_or(a.span);
                self.comparison_constraints.push(ComparisonConstraint { ty: left.clone(), code: "E703", span: Some(span) });
                Ok(Ty::Bool)
            }
            BinOp::And | BinOp::Or => {
                self.unify(&left, &Ty::Bool, a.span)?;
                Ok(Ty::Bool)
            }
            _ => self.numeric_binary(op, &left, a.span),
        }
    }

    fn record_comparisons(&mut self, mut constraints: Vec<ComparisonConstraint>, span: Span) {
        for constraint in &mut constraints { constraint.span.get_or_insert(span); }
        self.comparison_constraints.extend(constraints);
    }

    fn contains_function(&self, ty: &Ty, visited: &mut Vec<Ty>) -> bool {
        if matches!(ty, Ty::Fn(..)) { return true; }
        if visited.contains(ty) { return false; }
        visited.push(ty.clone());
        if let Ty::Abstract(name, _) = ty {
            return self.abstract_representations.get(name).is_some_and(|representation|
                self.contains_function(representation, visited));
        }
        let Ty::Adt(name, _) = ty else { return false; };
        self.constructors.values()
            .chain(self.records.values().map(|(_, scheme)| scheme))
            .chain(self.module_constructors.values().map(|(_, scheme)| scheme))
            .chain(self.module_records.values().map(|(_, scheme)| scheme))
            .any(|scheme| {
            let mut inference = self.inference.clone();
            let instantiated = inference.instantiate(scheme);
            let Ty::Fn(payload, result) = instantiated else { return false; };
            if !matches!(&*result, Ty::Adt(result, _) if result == name) || inference.unify(&result, ty).is_err() { return false; }
            payload.iter().any(|field| self.contains_function(&inference.resolve(field), visited))
        })
    }

    fn numeric_binary(&mut self, op: BinOp, ty: &Ty, span: Span) -> Result<Ty, Diagnostic> {
        self.numeric(ty, span)?;
        let ty = self.inference.resolve(ty);
        if matches!(op, BinOp::Div | BinOp::Mod) && matches!(ty, Ty::Var(_)) {
            let result = self.inference.fresh();
            self.divisions.push((op, ty, result.clone(), span));
            return Ok(result);
        }
        match op {
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => Ok(Ty::Bool),
            BinOp::Mod if ty == Ty::Float => {
                Err(self.error(span, Some("E305"), "Float has no remainder operator"))
            }
            BinOp::Div | BinOp::Mod if ty == Ty::Int => Ok(Ty::Adt(
                "Result".into(),
                vec![Ty::Int, Ty::Adt("DivisionError".into(), vec![])],
            )),
            _ => Ok(ty.clone()),
        }
    }
}
