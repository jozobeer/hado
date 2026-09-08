use crate::parser::{Diagnostic, Item, Span, TypeKind};
use std::collections::HashSet;

#[derive(Default)]
pub(crate) struct AmbiguousDeclarations {
    pub declared_types: HashSet<String>,
    pub declared_constructors: HashSet<String>,
    pub declared_modules: HashSet<String>,
    pub declared_signatures: HashSet<String>,
    pub modules: HashSet<String>,
    pub signatures: HashSet<String>,
    pub values: HashSet<String>,
    pub types: HashSet<String>,
    pub constructors: HashSet<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl AmbiguousDeclarations {
    pub fn collect(items: &[Item], path: &str) -> Self {
        let mut result = Self::default();
        let mut values = HashSet::new();
        let mut types = HashSet::new();
        let mut constructors = HashSet::new();
        let mut modules = HashSet::new();
        let mut signatures = HashSet::new();
        for item in items {
            match item {
                Item::Def(def) => record(&def.name, def.name_span, &mut values, &mut result.values, &mut result.diagnostics, path),
                Item::SigDef(def) => {
                    record(&def.name, def.name_span, &mut values, &mut result.values, &mut result.diagnostics, path);
                    let mut parameters = HashSet::new();
                    let mut repeated = HashSet::new();
                    for parameter in &def.type_params {
                        record(&parameter.name, parameter.span, &mut parameters, &mut repeated, &mut result.diagnostics, path);
                    }
                }
                Item::ModuleImpl(module) => record(&module.name, module.name_span, &mut modules, &mut result.modules, &mut result.diagnostics, path),
                Item::ModuleSig(signature) => record(&signature.name, signature.name_span, &mut signatures, &mut result.signatures, &mut result.diagnostics, path),
                Item::TypeDef(def) => {
                    record(&def.name, def.name_span, &mut types, &mut result.types, &mut result.diagnostics, path);
                    for (name, span) in constructor_names(def) {
                        record(name, span, &mut constructors, &mut result.constructors, &mut result.diagnostics, path);
                    }
                }
            }
        }
        for item in items {
            if let Item::ModuleSig(signature) = item {
                let inner = Self::collect(&signature.items, path);
                if !inner.diagnostics.is_empty() { result.signatures.insert(signature.name.clone()); }
                result.diagnostics.extend(inner.diagnostics);
            }
            if let Item::TypeDef(def) = item {
                if result.types.contains(&def.name) {
                    result.constructors.extend(constructor_names(def).into_iter().map(|(name, _)| name.clone()));
                }
            }
        }
        result.declared_modules = modules;
        result.declared_signatures = signatures;
        result.declared_types = types;
        result.declared_constructors = constructors;
        result
    }
}

fn record(name: &str, span: Span, seen: &mut HashSet<String>, ambiguous: &mut HashSet<String>, diagnostics: &mut Vec<Diagnostic>, path: &str) {
    if !seen.insert(name.into()) {
        ambiguous.insert(name.into());
        diagnostics.push(Diagnostic { path: path.into(), line: span.line, col: span.col,
            code: Some("E205".into()), message: format!("duplicate declaration {name}") });
    }
}

fn constructor_names(def: &crate::parser::TypeDef) -> Vec<(&String, Span)> {
    match def.ty.as_ref().map(|ty| &ty.kind) {
        Some(TypeKind::Adt(variants)) => variants.iter().filter_map(|variant| match &variant.kind {
            TypeKind::Name(name) | TypeKind::Apply(name, _) => Some((name, variant.span)),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    }
}
