use crate::parser::{Ast, Diagnostic, Item, TypeDef, TypeExpr};
use std::collections::{HashMap, HashSet};

pub fn check(ast: &Ast, path: &str) -> Result<(), Diagnostic> {
    let definitions: HashMap<_, _> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::TypeDef(def) => Some((def.name.as_str(), def)),
            _ => None,
        })
        .collect();
    let edges: HashMap<_, _> = definitions
        .iter()
        .map(|(name, def)| (*name, references(def)))
        .collect();
    for (name, def) in &definitions {
        for (target, args) in &edges[name] {
            if reaches(target, name, &edges, &mut HashSet::new())
                && !same_parameters(args, &def.params)
            {
                return Err(Diagnostic {
                    path: path.into(),
                    line: def.span.line,
                    col: def.span.col,
                    code: Some("E302".into()),
                    message: "recursive type arguments must preserve declared parameters".into(),
                });
            }
        }
    }
    Ok(())
}

fn same_parameters(args: &[TypeExpr], parameters: &[String]) -> bool {
    args.len() == parameters.len()
        && args
            .iter()
            .zip(parameters)
            .all(|(arg, parameter)| matches!(arg, TypeExpr::Name(name) if name == parameter))
}

type Edges<'a> = HashMap<&'a str, Vec<(&'a str, &'a [TypeExpr])>>;

fn reaches<'a>(
    from: &'a str,
    target: &str,
    edges: &Edges<'a>,
    seen: &mut HashSet<&'a str>,
) -> bool {
    if from == target {
        return true;
    }
    if !seen.insert(from) {
        return false;
    }
    edges.get(from).is_some_and(|next| {
        next.iter()
            .any(|(name, _)| reaches(name, target, edges, seen))
    })
}

fn references(def: &TypeDef) -> Vec<(&str, &[TypeExpr])> {
    let mut result = Vec::new();
    if let Some(body) = &def.ty {
        match body {
            TypeExpr::Record(fields) => {
                for (_, ty) in fields {
                    collect(ty, &mut result);
                }
            }
            TypeExpr::Adt(variants) => {
                for variant in variants {
                    payload(variant, &mut result);
                }
            }
            variant => payload(variant, &mut result),
        }
    }
    result
}

fn payload<'a>(variant: &'a TypeExpr, result: &mut Vec<(&'a str, &'a [TypeExpr])>) {
    if let TypeExpr::Apply(_, args) = variant {
        for arg in args {
            collect(arg, result);
        }
    }
}

fn collect<'a>(ty: &'a TypeExpr, result: &mut Vec<(&'a str, &'a [TypeExpr])>) {
    match ty {
        TypeExpr::Name(name) => result.push((name, &[])),
        TypeExpr::Apply(name, args) => {
            result.push((name, args));
            for arg in args {
                collect(arg, result);
            }
        }
        TypeExpr::Group(args) | TypeExpr::Adt(args) => {
            for arg in args {
                collect(arg, result);
            }
        }
        TypeExpr::Record(fields) => {
            for (_, ty) in fields {
                collect(ty, result);
            }
        }
    }
}
