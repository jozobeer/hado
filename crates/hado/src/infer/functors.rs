use super::*;
use crate::parser::{ImplementationForm, ModuleImpl, ModuleSig};

// 前処理が functor 定義から集めた共有等式と、適用の契約名を照合するための情報。
// 等式を適用結果へ登録する前に、結果契約と各引数の契約を照合するために持つ。
struct FunctorShared {
    result_contract: String,
    parameters: Vec<(String, String)>,
    shared: HashMap<String, TypeRealization>,
}

#[derive(Clone)]
pub(super) struct Template {
    module: ModuleImpl,
    exports: HashMap<String, Scheme>,
    representations: HashMap<String, TypeRealization>,
    shared: HashMap<String, TypeRealization>,
    definitions: HashMap<String, Scheme>,
    named_constructors: HashMap<String, Vec<String>>,
    records: HashMap<String, (Vec<String>, Scheme)>,
    constructors: HashMap<String, (u32, Scheme)>,
    values: HashMap<(usize, usize), Ty>,
    composites: HashMap<(usize, usize), Ty>,
    comparisons: Vec<ComparisonConstraint>,
}

pub(super) fn duplicate_applications(ast: &Ast, path: &str) -> Vec<Diagnostic> {
    let mut seen = HashSet::new();
    ast.items
        .iter()
        .filter_map(|item| {
            let Item::ModuleImpl(module) = item else {
                return None;
            };
            let ImplementationForm::Application { functor, arguments } = &module.form else {
                return None;
            };
            let key = (
                functor.name.clone(),
                arguments
                    .iter()
                    .map(|arg| arg.name.clone())
                    .collect::<Vec<_>>(),
            );
            (!seen.insert(key)).then(|| Diagnostic {
                related: None,
                path: path.into(),
                line: module.span.line,
                col: module.span.col,
                code: Some("E604".into()),
                message: "duplicate functor application".into(),
            })
        })
        .collect()
}

fn owned<T: Clone>(entries: &HashMap<String, T>, module: &str) -> HashMap<String, T> {
    let prefix = format!("{module}.");
    entries
        .iter()
        .filter(|(name, _)| name.starts_with(&prefix))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

// 前処理が `Ty::Abstract` を作る唯一の出口。参照先の契約の arity と型引数の個数が
// 食い違う等式は登録しない。登録すると `Ty::reveal` から `TypeRealization::apply` の
// 個数 assert に到達して panic する（§47: 数が違えば E602）。登録しなければ既存の経路
// （annotation_inner の arity 照合、type_realizations の E204）が拒否する。
fn shared_abstract_type(qualified: String, arity: Option<usize>, arguments: Vec<Ty>) -> Option<Ty> {
    if arity != Some(arguments.len()) {
        return None;
    }
    Some(Ty::Abstract(qualified, arguments))
}

// reveal は名前の置き換えを再帰的に行うので、shared の等式が輪になっていると止まらない。
// 適用の写しで輪を作る等式は shared_realizations に入れない。輪そのものは
// abstract_representations に残るので check_realization_cycles が E602 で拒否する。
pub(super) fn introduces_cycle(
    base: &HashMap<String, TypeRealization>,
    additions: &HashMap<String, TypeRealization>,
) -> bool {
    let mut graph: HashMap<String, std::collections::BTreeSet<String>> = HashMap::new();
    for (name, realization) in base.iter().chain(additions.iter()) {
        let mut references = std::collections::BTreeSet::new();
        realization.body.abstract_names(&mut references);
        graph.insert(name.clone(), references);
    }
    additions
        .keys()
        .any(|name| realization_reaches(name, name, &graph, &mut HashSet::new()))
}

impl Checker<'_> {
    pub(super) fn module(
        &mut self,
        module: &ModuleImpl,
        signatures: &HashMap<String, &ModuleSig>,
    ) -> Result<(), Diagnostic> {
        match &module.form {
            ImplementationForm::Ordinary => self.ordinary_module(module, signatures),
            ImplementationForm::Functor(_) => self.functor(module, signatures),
            ImplementationForm::Application { .. } => {
                if let Err(error) = self.application(module) {
                    self.recover_inference_error(Err(error), ())?;
                    if let Some(signature) = signatures.get(&module.sig) {
                        let exports = self.module_signature_exports(module, signature)?;
                        self.modules.insert(module.name.clone(), exports);
                    }
                }
                Ok(())
            }
        }
    }

    // AST から `shared type T = body` の等式を、型と効果の登録より前に集める。
    // 実装本体の推論は行わない。等式の収集を登録より後に置くと、record schema と
    // 効果の操作型が不透明のまま登録され、消費側で E305 になる。
    pub(super) fn collect_shared_realizations(&mut self, ast: &Ast) {
        let signatures: HashMap<&str, &ModuleSig> = ast
            .items
            .iter()
            .filter_map(|item| match item {
                Item::ModuleSig(signature) => Some((signature.name.as_str(), signature)),
                _ => None,
            })
            .collect();
        let mut templates: HashMap<String, FunctorShared> = HashMap::new();
        let mut additions: HashMap<String, TypeRealization> = HashMap::new();
        for item in &ast.items {
            let Item::ModuleImpl(module) = item else {
                continue;
            };
            if self.ambiguous_modules.contains(&module.name) {
                continue;
            }
            match &module.form {
                ImplementationForm::Functor(parameters) => {
                    let internal: HashMap<String, String> = parameters
                        .iter()
                        .map(|parameter| {
                            (
                                parameter.name.clone(),
                                format!("{}${}", module.name, parameter.name),
                            )
                        })
                        .collect();
                    let contracts: HashMap<String, String> = parameters
                        .iter()
                        .map(|parameter| (parameter.name.clone(), parameter.contract.clone()))
                        .collect();
                    let shared = self.module_shared_map(module, &internal, &contracts, &signatures);
                    templates.insert(
                        module.name.clone(),
                        FunctorShared {
                            result_contract: module.sig.clone(),
                            parameters: parameters
                                .iter()
                                .map(|parameter| {
                                    (parameter.name.clone(), parameter.contract.clone())
                                })
                                .collect(),
                            shared,
                        },
                    );
                }
                ImplementationForm::Ordinary => additions.extend(self.module_shared_map(
                    module,
                    &HashMap::new(),
                    &HashMap::new(),
                    &signatures,
                )),
                ImplementationForm::Application { .. } => {}
            }
        }
        for item in &ast.items {
            let Item::ModuleImpl(module) = item else {
                continue;
            };
            let ImplementationForm::Application { functor, arguments } = &module.form else {
                continue;
            };
            if self.ambiguous_modules.contains(&module.name) {
                continue;
            }
            let Some(template) = templates.get(&functor.name) else {
                continue;
            };
            // 等式を適用結果へ登録する前に契約名を照合する。食い違う適用の等式を登録すると、
            // 適用結果と引数の実体化で抽象型の arity が食い違い `TypeRealization::apply` の
            // assert に到達する。登録しなければ application() が E602 を出す。
            if template.result_contract != module.sig {
                continue;
            }
            if template.parameters.len() != arguments.len() {
                continue;
            }
            let contracts_match =
                template
                    .parameters
                    .iter()
                    .zip(arguments)
                    .all(|((_, contract), argument)| {
                        self.source_modules
                            .get(&argument.name)
                            .filter(|actual| !matches!(actual.form, ImplementationForm::Functor(_)))
                            .is_some_and(|actual| actual.sig == *contract)
                    });
            if !contracts_match {
                continue;
            }
            let mut names = HashMap::from([(functor.name.clone(), module.name.clone())]);
            for (parameter, argument) in template.parameters.iter().zip(arguments) {
                names.insert(
                    format!("{}${}", functor.name, parameter.0),
                    argument.name.clone(),
                );
            }
            for (name, realization) in &template.shared {
                additions.insert(
                    crate::functor_instances::rename(name, &names),
                    TypeRealization {
                        parameters: realization.parameters.clone(),
                        body: realization.body.rename_modules(&names),
                    },
                );
            }
        }
        // 輪になる等式は入れない。reveal が停止しなくなるため。輪そのものは既存の
        // E602 の経路（実体化の別名解決と check_realization_cycles）が拒否する。
        for (name, realization) in additions {
            let addition = HashMap::from([(name, realization)]);
            if !introduces_cycle(&self.shared_realizations, &addition) {
                self.shared_realizations.extend(addition);
            }
        }
    }

    fn module_shared_map(
        &mut self,
        module: &ModuleImpl,
        functor_params: &HashMap<String, String>,
        functor_contracts: &HashMap<String, String>,
        signatures: &HashMap<&str, &ModuleSig>,
    ) -> HashMap<String, TypeRealization> {
        // 契約の抽象型の arity。abstract_types は functor の契約を本文処理まで持たないので、
        // 契約の署名から取る（module_abstract_parameters と同じ出所）。
        let contract_arity: HashMap<&str, usize> = signatures
            .get(module.sig.as_str())
            .map(|signature| {
                signature
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        Item::TypeDef(definition) => {
                            Some((definition.name.as_str(), definition.params.len()))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        // functor 仮引数の抽象型（本文の `M.T`）を内部名 `Make$M.T` に写し、仮引数の
        // 契約の署名から arity を取る。abstract_types は functor の契約を持たないため。
        let functor_abstracts: HashMap<String, (String, usize)> = functor_contracts
            .iter()
            .flat_map(|(name, contract)| {
                let internal = functor_params.get(name);
                let signature = signatures.get(contract.as_str());
                match (internal, signature) {
                    (Some(internal), Some(signature)) => signature
                        .items
                        .iter()
                        .filter_map(|item| match item {
                            Item::TypeDef(definition) => Some((
                                format!("{name}.{}", definition.name),
                                (
                                    format!("{internal}.{}", definition.name),
                                    definition.params.len(),
                                ),
                            )),
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                }
            })
            .collect();
        let local: HashMap<String, &crate::parser::TypeDef> = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::TypeDef(definition) => Some((definition.name.clone(), definition)),
                _ => None,
            })
            .collect();
        let mut shared = HashMap::new();
        for item in &module.items {
            let Item::TypeDef(definition) = item else {
                continue;
            };
            if !definition.shared {
                continue;
            }
            let Some(body) = &definition.ty else { continue };
            // 食い違う arity の等式を登録すると `Ty::reveal` から
            // `TypeRealization::apply` の個数 assert に到達して panic する
            // （§47: 数が違えば E602）。登録しなければ既存の E602 の経路が拒否する。
            // 契約に無い型は arity が取れず、ここで登録しない（E204 は別の経路）。
            let Some(arity) = contract_arity.get(definition.name.as_str()) else {
                continue;
            };
            if *arity != definition.params.len() {
                continue;
            }
            let qualified = format!("{}.{}", module.name, definition.name);
            let parameters: HashMap<String, Ty> = definition
                .params
                .iter()
                .map(|parameter| (parameter.name.clone(), self.inference.fresh()))
                .collect();
            let mut visiting = Vec::new();
            let Some(ty) = self.shared_body_type(
                body,
                &module.name,
                &local,
                &parameters,
                &functor_abstracts,
                &mut visiting,
            ) else {
                continue;
            };
            let ids = definition
                .params
                .iter()
                .filter_map(|parameter| match parameters.get(&parameter.name) {
                    Some(Ty::Var(id)) => Some(*id),
                    _ => None,
                })
                .collect();
            shared.insert(
                qualified,
                TypeRealization {
                    parameters: ids,
                    body: ty,
                },
            );
        }
        shared
    }

    fn shared_body_type(
        &mut self,
        expr: &TypeExpr,
        module: &str,
        local: &HashMap<String, &crate::parser::TypeDef>,
        parameters: &HashMap<String, Ty>,
        functor_abstracts: &HashMap<String, (String, usize)>,
        visiting: &mut Vec<String>,
    ) -> Option<Ty> {
        // `Fn(A, B => C {E})` は引数群（`Group`）・戻り型・効果行からなる。annotation_inner と
        // 同じ形で解く。効果行は戻り型の `effects` にある。ここを落とすと record schema と
        // 効果の操作型が登録される前に等式が確定せず、関数型の抽象型が不透明のまま残る。
        if let TypeKind::Apply(name, arguments) = &expr.kind {
            if name == "Fn" && arguments.len() == 2 {
                let inputs = match &arguments[0].kind {
                    TypeKind::Group(inputs) => inputs.as_slice(),
                    _ => &arguments[..1],
                };
                let inputs = inputs
                    .iter()
                    .map(|input| {
                        self.shared_body_type(
                            input,
                            module,
                            local,
                            parameters,
                            functor_abstracts,
                            visiting,
                        )
                    })
                    .collect::<Option<_>>()?;
                let output = self.shared_body_type(
                    &arguments[1],
                    module,
                    local,
                    parameters,
                    functor_abstracts,
                    visiting,
                )?;
                let row = self.row_annotation(&arguments[1].effects).ok()?;
                return Some(Ty::fn_with(inputs, output, row));
            }
        }
        let (name, arguments) = match &expr.kind {
            TypeKind::Name(name) => (name.as_str(), &[][..]),
            TypeKind::Apply(name, arguments) => (name.as_str(), arguments.as_slice()),
            _ => return None,
        };
        let args: Vec<Ty> = arguments
            .iter()
            .map(|argument| {
                self.shared_body_type(
                    argument,
                    module,
                    local,
                    parameters,
                    functor_abstracts,
                    visiting,
                )
            })
            .collect::<Option<_>>()?;
        // 実体化自身の型引数（`shared type A T = A List` の `A`）。
        if let Some(ty) = parameters.get(name) {
            return args.is_empty().then(|| ty.clone());
        }
        // functor 仮引数の抽象型（`shared type T = M.T` の `M.T`）。仮引数の契約の
        // 署名から取った arity と照合し、食い違えば等式を登録しない。
        if let Some((qualified, arity)) = functor_abstracts.get(name) {
            return shared_abstract_type(qualified.clone(), Some(*arity), args);
        }
        // 実装内の局所型。ADT / record は実装名で修飾し、別名は本体を展開する。
        if let Some(definition) = local.get(name) {
            if definition.params.len() != args.len() {
                return None;
            }
            let body = definition.ty.as_ref()?;
            if matches!(body.kind, TypeKind::Record(_) | TypeKind::Adt(_)) {
                return Some(Ty::Adt(format!("{module}.{name}"), args));
            }
            if visiting.iter().any(|seen| seen == name) {
                return None;
            }
            let mut substituted = parameters.clone();
            for (formal, argument) in definition.params.iter().zip(&args) {
                substituted.insert(formal.name.clone(), argument.clone());
            }
            visiting.push(name.to_owned());
            let result = self.shared_body_type(
                body,
                module,
                local,
                &substituted,
                functor_abstracts,
                visiting,
            );
            visiting.pop();
            return result;
        }
        if let Some((arity, qualified)) = self.abstract_types.get(name) {
            return shared_abstract_type(qualified.clone(), Some(*arity), args);
        }
        if self.type_arities.get(name) == Some(&args.len()) {
            return Some(Ty::Adt(name.to_owned(), args));
        }
        crate::types::builtin_type(&name)
    }

    fn functor(
        &mut self,
        module: &ModuleImpl,
        signatures: &HashMap<String, &ModuleSig>,
    ) -> Result<(), Diagnostic> {
        let saved_modules = self.modules.clone();
        let saved_abstracts = self.abstract_types.clone();
        let before_comparisons = self.comparison_constraints.len();
        let before_values: HashSet<_> = self.value_types.keys().copied().collect();
        let before_composites: HashSet<_> = self.composite_types.keys().copied().collect();
        let result = (|| {
            let ImplementationForm::Functor(parameters) = &module.form else {
                unreachable!()
            };
            for parameter in parameters {
                let signature = signatures.get(&parameter.contract).ok_or_else(|| {
                    self.error(
                        parameter.contract_span,
                        Some("E204"),
                        "unknown functor parameter contract",
                    )
                })?;
                let argument = ModuleImpl {
                    name: parameter.name.clone(),
                    name_span: parameter.name_span,
                    sig: parameter.contract.clone(),
                    sig_span: parameter.contract_span,
                    span: parameter.name_span,
                    form: ImplementationForm::Ordinary,
                    items: Vec::new(),
                };
                let exports = self.module_signature_exports(&argument, signature)?;
                let internal = format!("{}${}", module.name, parameter.name);
                let substitutions = HashMap::from([(parameter.name.clone(), internal.clone())]);
                for item in &signature.items {
                    if let Item::TypeDef(definition) = item {
                        self.abstract_types.insert(
                            format!("{}.{}", parameter.name, definition.name),
                            (
                                definition.params.len(),
                                format!("{internal}.{}", definition.name),
                            ),
                        );
                    }
                }
                self.modules.insert(
                    parameter.name.clone(),
                    exports
                        .into_iter()
                        .map(|(name, scheme)| (name, scheme.rename_modules(&substitutions)))
                        .collect(),
                );
            }
            self.ordinary_module(module, signatures)?;
            let template = Template {
                module: module.clone(),
                comparisons: self.comparison_constraints[before_comparisons..]
                    .iter()
                    .map(|constraint| ComparisonConstraint {
                        ty: self.inference.resolve(&constraint.ty),
                        code: constraint.code,
                        span: constraint.span,
                    })
                    .collect(),
                exports: self.modules.get(&module.name).cloned().unwrap_or_default(),
                representations: owned(&self.abstract_representations, &module.name),
                shared: owned(&self.shared_realizations, &module.name),
                definitions: owned(&self.module_definitions, &module.name),
                named_constructors: owned(&self.module_named_constructors, &module.name),
                records: owned(&self.module_records, &module.name),
                constructors: owned(&self.module_constructors, &module.name),
                values: self
                    .value_types
                    .iter()
                    .filter(|(span, _)| !before_values.contains(span))
                    .map(|(span, ty)| (*span, self.inference.resolve(ty)))
                    .collect(),
                composites: self
                    .composite_types
                    .iter()
                    .filter(|(span, _)| !before_composites.contains(span))
                    .map(|(span, ty)| (*span, self.inference.resolve(ty)))
                    .collect(),
            };
            self.functors.insert(module.name.clone(), template);
            Ok(())
        })();
        self.modules = saved_modules;
        self.abstract_types = saved_abstracts;
        result
    }

    fn application(&mut self, application: &ModuleImpl) -> Result<(), Diagnostic> {
        let ImplementationForm::Application { functor, arguments } = &application.form else {
            unreachable!()
        };
        let template = self
            .functors
            .get(&functor.name)
            .cloned()
            .ok_or_else(|| self.error(functor.span, Some("E204"), "unknown functor"))?;
        let ImplementationForm::Functor(parameters) = &template.module.form else {
            unreachable!()
        };
        if application.sig != template.module.sig || parameters.len() != arguments.len() {
            return Err(self.error(
                application.span,
                Some("E602"),
                "functor application contract mismatch",
            ));
        }
        let mut names = HashMap::from([(template.module.name.clone(), application.name.clone())]);
        let mut module_names = names.clone();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let actual = self
                .source_modules
                .get(&argument.name)
                .filter(|module| !matches!(module.form, ImplementationForm::Functor(_)))
                .ok_or_else(|| {
                    self.error(
                        argument.span,
                        Some("E204"),
                        "functor argument must be an implementation",
                    )
                })?;
            if actual.sig != parameter.contract {
                return Err(self.error(
                    application.span,
                    Some("E602"),
                    "functor argument contract mismatch",
                ));
            }
            names.insert(
                format!("{}${}", template.module.name, parameter.name),
                argument.name.clone(),
            );
            module_names.insert(parameter.name.clone(), argument.name.clone());
        }
        self.instantiate_template(&template, application, &names, &module_names);
        Ok(())
    }

    fn instantiate_template(
        &mut self,
        template: &Template,
        application: &ModuleImpl,
        names: &HashMap<String, String>,
        module_names: &HashMap<String, String>,
    ) {
        let offset = self.next_instance_line;
        let instance = crate::functor_instances::instantiate(
            &template.module,
            application,
            module_names,
            offset,
        );
        let ast = Ast {
            items: vec![Item::ModuleImpl(instance.clone())],
            span: instance.span,
            library: None,
            branch_anchors: Vec::new(),
            branch_spans: Vec::new(),
            equality_anchors: Vec::new(),
            qualified_type_anchors: Vec::new(),
            name_sites: Vec::new(),
            written_list_patterns: Vec::new(),
            long_condition_functions: Vec::new(),
            branches_after_fallback: Vec::new(),
        };
        self.next_instance_line = crate::functor_instances::max_line(&ast) + 1;
        self.instantiated_modules.push(instance);
        self.comparison_constraints
            .extend(
                template
                    .comparisons
                    .iter()
                    .map(|constraint| ComparisonConstraint {
                        ty: constraint.ty.rename_modules(names),
                        code: constraint.code,
                        span: constraint.span,
                    }),
            );
        // 本文の shared type T = M.T の等式を適用結果に引き継ぐ。改名した本体が
        // 引数の実装名を指すので、引数の shared 実体化へ reveal が連鎖する。
        let additions: HashMap<_, _> = template
            .shared
            .iter()
            .map(|(name, realization)| {
                (
                    crate::functor_instances::rename(name, names),
                    TypeRealization {
                        parameters: realization.parameters.clone(),
                        body: realization.body.rename_modules(names),
                    },
                )
            })
            .collect();
        if !introduces_cycle(&self.shared_realizations, &additions) {
            self.shared_realizations.extend(additions);
        }
        let exports: HashMap<_, _> = template
            .exports
            .iter()
            .map(|(name, scheme)| {
                (
                    name.clone(),
                    scheme
                        .rename_modules(names)
                        .reveal(&self.shared_realizations),
                )
            })
            .collect();
        self.modules.insert(application.name.clone(), exports);
        for (name, realization) in &template.representations {
            self.abstract_representations.insert(
                crate::functor_instances::rename(name, names),
                TypeRealization {
                    parameters: realization.parameters.clone(),
                    body: realization.body.rename_modules(names),
                },
            );
        }
        for (name, scheme) in &template.definitions {
            let revealed = scheme
                .rename_modules(names)
                .reveal(&self.shared_realizations);
            self.module_definitions
                .insert(crate::functor_instances::rename(name, names), revealed);
        }
        for (name, (fields, scheme)) in &template.records {
            let revealed = scheme
                .rename_modules(names)
                .reveal(&self.shared_realizations);
            self.module_records.insert(
                crate::functor_instances::rename(name, names),
                (fields.clone(), revealed),
            );
        }
        for (name, fields) in &template.named_constructors {
            self.module_named_constructors.insert(
                crate::functor_instances::rename(name, names),
                fields.clone(),
            );
        }
        for (name, (tag, scheme)) in &template.constructors {
            let revealed = scheme
                .rename_modules(names)
                .reveal(&self.shared_realizations);
            self.module_constructors.insert(
                crate::functor_instances::rename(name, names),
                (*tag, revealed),
            );
        }
        for ((line, col), ty) in &template.values {
            let revealed = ty.rename_modules(names).reveal(&self.shared_realizations);
            self.value_types.insert((line + offset, *col), revealed);
        }
        for ((line, col), ty) in &template.composites {
            let revealed = ty.rename_modules(names).reveal(&self.shared_realizations);
            self.composite_types.insert((line + offset, *col), revealed);
        }
    }
}
