use crate::parser::{Ast, Diagnostic, Item, Span, TypeDef, TypeExpr, TypeKind};
use std::collections::{HashMap, HashSet};

pub fn check(ast: &Ast, path: &str) -> Result<(), Diagnostic> {
    match violations(ast, path)
        .into_iter()
        .map(|violation| violation.diagnostic)
        .min_by_key(crate::diagnostic_order)
    {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(crate) struct Violation {
    pub definition: String,
    pub reference: Span,
    pub diagnostic: Diagnostic,
}

pub(crate) fn violations(ast: &Ast, path: &str) -> Vec<Violation> {
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
    let mut diagnostics = Vec::new();
    for (name, def) in &definitions {
        for (target, args, reference) in &edges[name] {
            if reaches(target, name, &edges, &mut HashSet::new()) {
                if let Some(span) = changed_argument(args, &def.params, *reference) {
                    diagnostics.push(Violation {
                        definition: (*name).into(),
                        reference: *reference,
                        diagnostic: Diagnostic {
                            related: None,
                            path: path.into(),
                            line: span.line,
                            col: span.col,
                            code: Some("E302".into()),
                            message: "recursive type arguments must preserve declared parameters"
                                .into(),
                        },
                    });
                }
            }
        }
    }
    diagnostics
}

fn changed_argument(
    args: &[TypeExpr],
    parameters: &[crate::parser::DeclaredName],
    reference: Span,
) -> Option<Span> {
    for (index, arg) in args.iter().enumerate() {
        if !parameters
            .get(index)
            .is_some_and(|param| matches!(&arg.kind, TypeKind::Name(name) if name == &param.name))
        {
            return Some(arg.span);
        }
    }
    (args.len() != parameters.len()).then_some(reference)
}

type Edges<'a> = HashMap<&'a str, Vec<(&'a str, &'a [TypeExpr], Span)>>;

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
            .any(|(name, _, _)| reaches(name, target, edges, seen))
    })
}

fn references(def: &TypeDef) -> Vec<(&str, &[TypeExpr], Span)> {
    let mut result = Vec::new();
    if let Some(body) = &def.ty {
        match &body.kind {
            TypeKind::Record(fields) => {
                for field in fields {
                    collect(&field.ty, &mut result);
                }
            }
            TypeKind::Adt(variants) => {
                for variant in variants {
                    for ty in variant.payload.types() {
                        collect(ty, &mut result);
                    }
                }
            }
            TypeKind::Apply(_, args) => {
                for arg in args {
                    collect(arg, &mut result);
                }
            }
            _ => {}
        }
    }
    result
}

fn collect<'a>(ty: &'a TypeExpr, result: &mut Vec<(&'a str, &'a [TypeExpr], Span)>) {
    match &ty.kind {
        TypeKind::Name(name) => result.push((name, &[], ty.span)),
        TypeKind::Apply(name, args) => {
            result.push((name, args, ty.span));
            for arg in args {
                collect(arg, result);
            }
        }
        TypeKind::Adt(variants) => {
            for variant in variants {
                for ty in variant.payload.types() {
                    collect(ty, result);
                }
            }
        }
        TypeKind::Group(args) => {
            for arg in args {
                collect(arg, result);
            }
        }
        TypeKind::Record(fields) => {
            for field in fields {
                collect(&field.ty, result);
            }
        }
    }
}
