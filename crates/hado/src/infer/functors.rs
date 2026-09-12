use super::*;
use crate::parser::{ImplementationForm, ModuleImpl, ModuleSig};

#[derive(Clone)]
pub(super) struct Template {
    module: ModuleImpl,
    exports: HashMap<String, Scheme>,
    representations: HashMap<String, TypeRealization>,
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
    ast.items.iter().filter_map(|item| {
        let Item::ModuleImpl(module) = item else { return None; };
        let ImplementationForm::Application { functor, arguments } = &module.form else { return None; };
        let key = (functor.name.clone(), arguments.iter().map(|arg| arg.name.clone()).collect::<Vec<_>>());
        (!seen.insert(key)).then(|| Diagnostic { path: path.into(), line: module.span.line,
            col: module.span.col, code: Some("E604".into()), message: "duplicate functor application".into() })
    }).collect()
}

fn owned<T: Clone>(entries: &HashMap<String, T>, module: &str) -> HashMap<String, T> {
    let prefix = format!("{module}.");
    entries.iter().filter(|(name, _)| name.starts_with(&prefix)).map(|(name, value)| (name.clone(), value.clone())).collect()
}

impl Checker<'_> {
    pub(super) fn module(&mut self, module: &ModuleImpl, signatures: &HashMap<String, &ModuleSig>) -> Result<(), Diagnostic> {
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
            },
        }
    }

    fn functor(&mut self, module: &ModuleImpl, signatures: &HashMap<String, &ModuleSig>) -> Result<(), Diagnostic> {
        let saved_modules = self.modules.clone();
        let saved_abstracts = self.abstract_types.clone();
        let before_comparisons = self.comparison_constraints.len();
        let before_values: HashSet<_> = self.value_types.keys().copied().collect();
        let before_composites: HashSet<_> = self.composite_types.keys().copied().collect();
        let result = (|| {
            let ImplementationForm::Functor(parameters) = &module.form else { unreachable!() };
            for parameter in parameters {
                let signature = signatures.get(&parameter.contract)
                    .ok_or_else(|| self.error(parameter.contract_span, Some("E204"), "unknown functor parameter contract"))?;
                let argument = ModuleImpl { name: parameter.name.clone(), name_span: parameter.name_span,
                    sig: parameter.contract.clone(), sig_span: parameter.contract_span, span: parameter.name_span,
                    form: ImplementationForm::Ordinary, items: Vec::new() };
                let exports = self.module_signature_exports(&argument, signature)?;
                let internal = format!("{}${}", module.name, parameter.name);
                let substitutions = HashMap::from([(parameter.name.clone(), internal.clone())]);
                for item in &signature.items {
                    if let Item::TypeDef(definition) = item {
                        self.abstract_types.insert(format!("{}.{}", parameter.name, definition.name),
                            (definition.params.len(), format!("{internal}.{}", definition.name)));
                    }
                }
                self.modules.insert(parameter.name.clone(), exports.into_iter()
                    .map(|(name, scheme)| (name, scheme.rename_modules(&substitutions))).collect());
            }
            self.ordinary_module(module, signatures)?;
            let template = Template {
                module: module.clone(),
                comparisons: self.comparison_constraints[before_comparisons..].iter().map(|constraint| ComparisonConstraint { ty: self.inference.resolve(&constraint.ty), code: constraint.code, span: constraint.span }).collect(),
                exports: self.modules.get(&module.name).cloned().unwrap_or_default(),
                representations: owned(&self.abstract_representations, &module.name),
                definitions: owned(&self.module_definitions, &module.name),
                named_constructors: owned(&self.module_named_constructors, &module.name),
                records: owned(&self.module_records, &module.name),
                constructors: owned(&self.module_constructors, &module.name),
                values: self.value_types.iter().filter(|(span, _)| !before_values.contains(span)).map(|(span, ty)| (*span, self.inference.resolve(ty))).collect(),
                composites: self.composite_types.iter().filter(|(span, _)| !before_composites.contains(span)).map(|(span, ty)| (*span, self.inference.resolve(ty))).collect(),
            };
            self.functors.insert(module.name.clone(), template);
            Ok(())
        })();
        self.modules = saved_modules;
        self.abstract_types = saved_abstracts;
        result
    }

    fn application(&mut self, application: &ModuleImpl) -> Result<(), Diagnostic> {
        let ImplementationForm::Application { functor, arguments } = &application.form else { unreachable!() };
        let template = self.functors.get(&functor.name).cloned()
            .ok_or_else(|| self.error(functor.span, Some("E204"), "unknown functor"))?;
        let ImplementationForm::Functor(parameters) = &template.module.form else { unreachable!() };
        if application.sig != template.module.sig || parameters.len() != arguments.len() {
            return Err(self.error(application.span, Some("E602"), "functor application contract mismatch"));
        }
        let mut names = HashMap::from([(template.module.name.clone(), application.name.clone())]);
        let mut module_names = names.clone();
        for (parameter, argument) in parameters.iter().zip(arguments) {
            let actual = self.source_modules.get(&argument.name)
                .filter(|module| !matches!(module.form, ImplementationForm::Functor(_)))
                .ok_or_else(|| self.error(argument.span, Some("E204"), "functor argument must be an implementation"))?;
            if actual.sig != parameter.contract {
                return Err(self.error(application.span, Some("E602"), "functor argument contract mismatch"));
            }
            names.insert(format!("{}${}", template.module.name, parameter.name), argument.name.clone());
            module_names.insert(parameter.name.clone(), argument.name.clone());
        }
        self.instantiate_template(&template, application, &names, &module_names);
        Ok(())
    }

    fn instantiate_template(&mut self, template: &Template, application: &ModuleImpl, names: &HashMap<String, String>, module_names: &HashMap<String, String>) {
        let offset = self.next_instance_line;
        let instance = crate::functor_instances::instantiate(&template.module, application, module_names, offset);
        let ast = Ast { items: vec![Item::ModuleImpl(instance.clone())], span: instance.span, library: None,
            branch_anchors: Vec::new(), equality_anchors: Vec::new(), qualified_type_anchors: Vec::new(), name_sites: Vec::new(),
            written_list_patterns: Vec::new(), long_condition_functions: Vec::new(), branches_after_fallback: Vec::new() };
        self.next_instance_line = crate::functor_instances::max_line(&ast) + 1;
        self.instantiated_modules.push(instance);
        self.comparison_constraints.extend(template.comparisons.iter().map(|constraint| ComparisonConstraint {
            ty: constraint.ty.rename_modules(names), code: constraint.code, span: constraint.span }));
        self.modules.insert(application.name.clone(), template.exports.iter().map(|(name, scheme)| (name.clone(), scheme.rename_modules(names))).collect());
        for (name, realization) in &template.representations {
            self.abstract_representations.insert(crate::functor_instances::rename(name, names), TypeRealization {
                parameters: realization.parameters.clone(), body: realization.body.rename_modules(names) });
        }
        for (name, scheme) in &template.definitions {
            self.module_definitions.insert(crate::functor_instances::rename(name, names), scheme.rename_modules(names));
        }
        for (name, (fields, scheme)) in &template.records {
            self.module_records.insert(crate::functor_instances::rename(name, names), (fields.clone(), scheme.rename_modules(names)));
        }
        for (name, fields) in &template.named_constructors {
            self.module_named_constructors.insert(crate::functor_instances::rename(name, names), fields.clone());
        }
        for (name, (tag, scheme)) in &template.constructors {
            self.module_constructors.insert(crate::functor_instances::rename(name, names), (*tag, scheme.rename_modules(names)));
        }
        for ((line, col), ty) in &template.values {
            self.value_types.insert((line + offset, *col), ty.rename_modules(names));
        }
        for ((line, col), ty) in &template.composites {
            self.composite_types.insert((line + offset, *col), ty.rename_modules(names));
        }
    }
}
