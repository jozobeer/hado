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
    for item in items {
        if let Item::ModuleImpl(module) = item {
            if let Some(Some(signature)) = module.sig.as_deref().and_then(|name| signatures.get(name)) {
                check_module(module, signature, path, errors);
            }
            check_items(&module.items, &signatures, path, errors);
        }
    }
}

fn check_module(module: &ModuleImpl, signature: &ModuleSig, path: &str, errors: &mut Vec<Diagnostic>) {
    let abstracts: HashSet<_> = signature.items.iter().filter_map(|item| match item {
        Item::TypeDef(definition) => Some(definition.name.as_str()),
        _ => None,
    }).collect();
    for item in &module.items {
        if let Item::TypeDef(definition) = item {
            if let Some(ty) = &definition.ty {
                if matches!(ty.kind, TypeKind::Name(_) | TypeKind::Apply(..)) && !abstracts.contains(definition.name.as_str()) {
                    errors.push(Diagnostic {
                        path: path.into(), line: ty.span.line, col: ty.span.col,
                        code: Some("E100".into()),
                        message: "type aliases may only realize signature abstract types".into(),
                    });
                }
            }
        }
    }
}
