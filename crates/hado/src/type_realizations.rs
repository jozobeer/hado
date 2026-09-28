use crate::parser::{Ast, Diagnostic, Item, ModuleImpl, ModuleSig, TypeDef, TypeExpr, TypeKind};
use std::collections::{HashMap, HashSet};

type Signatures<'a> = HashMap<&'a str, Option<&'a ModuleSig>>;

pub(crate) fn check(ast: &Ast, path: &str, prelude_items: usize) -> Result<(), Diagnostic> {
    let mut errors = Vec::new();
    check_items(
        &ast.items,
        &Signatures::new(),
        path,
        &mut errors,
        prelude_items,
    );
    match errors.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn check_items<'a>(
    items: &'a [Item],
    inherited: &Signatures<'a>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
    prelude_items: usize,
) {
    let mut local = Signatures::new();
    for item in items {
        if let Item::ModuleSig(signature) = item {
            local
                .entry(&signature.name)
                .and_modify(|value| *value = None)
                .or_insert(Some(signature));
        }
    }
    let mut signatures = inherited.clone();
    signatures.extend(local);
    for (index, item) in items.iter().enumerate() {
        if let Item::ModuleImpl(module) = item {
            if let Some(Some(signature)) = signatures.get(module.sig.as_str()) {
                let outer = outer_types(items, index, prelude_items);
                check_module(module, signature, &outer, path, errors);
            }
            check_items(&module.items, &signatures, path, errors, 0);
        }
    }
}

/// §49・§55: an `implement` treats the top-level types of its own unit as outer.
/// The prelude is prepended to the user's items, so an implementation in the
/// prelude must not see the user's root types; an implementation in the user's
/// program sees both the prelude and the user's types.
fn outer_types<'a>(items: &'a [Item], index: usize, prelude_items: usize) -> HashSet<&'a str> {
    let end = if index < prelude_items {
        prelude_items
    } else {
        items.len()
    };
    items[..end]
        .iter()
        .filter_map(|item| match item {
            Item::TypeDef(def) => Some(def.name.as_str()),
            _ => None,
        })
        .collect()
}

fn check_module(
    module: &ModuleImpl,
    signature: &ModuleSig,
    outer: &HashSet<&str>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let abstracts = abstract_types(signature);
    for item in &module.items {
        if let Item::TypeDef(definition) = item {
            check_realization(definition, outer, &abstracts, path, errors);
        }
    }
}

/// signature が宣言する抽象型の名前。
fn abstract_types(signature: &ModuleSig) -> HashSet<&str> {
    signature
        .items
        .iter()
        .filter_map(|item| match item {
            Item::TypeDef(definition) => Some(definition.name.as_str()),
            _ => None,
        })
        .collect()
}

/// 実装が持ち込む型定義 1 つを、外の型と signature の抽象型に突き合わせる。
/// 2 つの欠陥は種別で排他なので、当たった方をこの順で積む。
fn check_realization(
    definition: &TypeDef,
    outer: &HashSet<&str>,
    abstracts: &HashSet<&str>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let Some(ty) = &definition.ty else {
        return;
    };
    if conflicts_with_existing_type(ty, definition.name.as_str(), outer, abstracts) {
        errors.push(realization_error(
            definition,
            path,
            "E205",
            "local data type conflicts with an existing type",
        ));
    }
    if is_unknown_realization(ty, definition.name.as_str(), abstracts) {
        errors.push(realization_error(
            definition,
            path,
            "E204",
            "unknown abstract type realization",
        ));
    }
}

/// 外の型と同じ名前、または signature の抽象型と同じ名前の record を再定義している。
fn conflicts_with_existing_type(
    ty: &TypeExpr,
    name: &str,
    outer: &HashSet<&str>,
    abstracts: &HashSet<&str>,
) -> bool {
    match ty.kind {
        TypeKind::Adt(_) => outer.contains(name),
        TypeKind::Record(_) => outer.contains(name) || abstracts.contains(name),
        _ => false,
    }
}

/// signature が宣言していない抽象型を実体化しようとしている。
fn is_unknown_realization(ty: &TypeExpr, name: &str, abstracts: &HashSet<&str>) -> bool {
    matches!(ty.kind, TypeKind::Name(_) | TypeKind::Apply(..)) && !abstracts.contains(name)
}

fn realization_error(definition: &TypeDef, path: &str, code: &str, message: &str) -> Diagnostic {
    Diagnostic {
        mentions: None,
        suggestion: None,
        related: None,
        path: path.into(),
        line: definition.name_span.line,
        col: definition.name_span.col,
        end_line: definition.name_span.line,
        end_col: definition.name_span.col,
        code: Some(code.into()),
        message: message.into(),
    }
}
