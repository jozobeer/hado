use crate::parser::{Diagnostic, Item, Span, TypeKind};
use std::collections::HashSet;

#[derive(Default)]
pub(crate) struct AmbiguousDeclarations {
    pub declared_types: HashSet<String>,
    pub declared_constructors: HashSet<String>,
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
        for item in items {
            match item {
                Item::Def(def) => record(&def.name, def.name_span, &mut values, &mut result.values, &mut result.diagnostics, path),
                Item::TypeDef(def) => {
                    record(&def.name, def.name_span, &mut types, &mut result.types, &mut result.diagnostics, path);
                    for (name, span) in constructor_names(def) {
                        record(name, span, &mut constructors, &mut result.constructors, &mut result.diagnostics, path);
                    }
                }
                _ => {}
            }
        }
        for item in items {
            if let Item::TypeDef(def) = item {
                if result.types.contains(&def.name) {
                    result.constructors.extend(constructor_names(def).into_iter().map(|(name, _)| name.clone()));
                }
            }
        }
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
