use crate::parser::{Ast, Diagnostic, Item, ModuleImpl, ModuleSig, TypeKind};
use std::collections::{HashMap, HashSet};

type Signatures<'a> = HashMap<&'a str, Option<&'a ModuleSig>>;

pub(crate) fn check(ast: &Ast, path: &str) -> Result<(), Diagnostic> {
    let mut errors = Vec::new();
    check_items(&ast.items, &Signatures::new(), path, &mut errors);
    match errors.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn check_items<'a>(items: &'a [Item], inherited: &Signatures<'a>, path: &str, errors: &mut Vec<Diagnostic>) {
    let mut local = Signatures::new();
    for item in items {
        if let Item::ModuleSig(signature) = item {
            local.entry(&signature.name).and_modify(|value| *value = None).or_insert(Some(signature));
        }
    }
    let mut signatures = inherited.clone();
    signatures.extend(local);
    let outer_types: HashSet<_> = items.iter().filter_map(|item| match item { Item::TypeDef(def) => Some(def.name.as_str()), _ => None }).collect();
    for item in items {
        if let Item::ModuleImpl(module) = item {
            if let Some(Some(signature)) = signatures.get(module.sig.as_str()) {
                check_module(module, signature, &outer_types, path, errors);
            }
            check_items(&module.items, &signatures, path, errors);
        }
    }
}

fn check_module(module: &ModuleImpl, signature: &ModuleSig, outer: &HashSet<&str>, path: &str, errors: &mut Vec<Diagnostic>) {
    let abstracts: HashSet<_> = signature.items.iter().filter_map(|item| match item {
        Item::TypeDef(definition) => Some(definition.name.as_str()),
        _ => None,
    }).collect();
    for item in &module.items {
        if let Item::TypeDef(definition) = item {
            if let Some(ty) = &definition.ty {
                if matches!(ty.kind, TypeKind::Adt(_) | TypeKind::Record(_))
                    && (outer.contains(definition.name.as_str()) || (matches!(ty.kind, TypeKind::Record(_)) && abstracts.contains(definition.name.as_str()))) {
                    errors.push(Diagnostic {
                        path: path.into(), line: definition.name_span.line, col: definition.name_span.col,
                        code: Some("E205".into()), message: "local data type conflicts with an existing type".into(),
                    });
                }
                if matches!(ty.kind, TypeKind::Name(_) | TypeKind::Apply(..)) && !abstracts.contains(definition.name.as_str()) {
                    errors.push(Diagnostic {
                        path: path.into(), line: definition.name_span.line, col: definition.name_span.col,
                        code: Some("E204".into()),
                        message: "unknown abstract type realization".into(),
                    });
                }
            }
        }
    }
}
