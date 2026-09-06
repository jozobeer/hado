use crate::parser::{Ast, DefKind, Item, Node, NodeKind, Pattern, PatternKind};
use std::collections::{BTreeMap, BTreeSet};

pub fn definition_groups(ast: &Ast) -> Vec<Vec<String>> {
    let mut graph = dependency_graph(ast);
    let mut groups = Vec::new();
    while !graph.is_empty() {
        let first = graph
            .keys()
            .find(|name| {
                graph.keys().all(|dependency| {
                    !reachable(name, dependency, &graph, &mut BTreeSet::new())
                        || reachable(dependency, name, &graph, &mut BTreeSet::new())
                })
            })
            .unwrap()
            .clone();
        let group: Vec<_> = graph
            .keys()
            .filter(|name| {
                reachable(&first, name, &graph, &mut BTreeSet::new())
                    && reachable(name, &first, &graph, &mut BTreeSet::new())
            })
            .cloned()
            .collect();
        for name in &group {
            graph.remove(name);
        }
        groups.push(group);
    }
    groups
}

fn reachable(
    from: &str,
    to: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    seen: &mut BTreeSet<String>,
) -> bool {
    if from == to {
        return true;
    }
    if !seen.insert(from.into()) {
        return false;
    }
    graph
        .get(from)
        .is_some_and(|edges| edges.iter().any(|edge| reachable(edge, to, graph, seen)))
}

pub(crate) fn bound_names(pattern: &Pattern) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    bind(pattern, &mut names);
    names
}

fn bind(pattern: &Pattern, bound: &mut BTreeSet<String>) {
    match &pattern.kind {
        PatternKind::Name(name) if !name.starts_with(char::is_uppercase) => {
            bound.insert(name.clone());
        }
        PatternKind::Constructor(_, args) => {
            for arg in args {
                bind(arg, bound);
            }
        }
        PatternKind::Record(fields) => {
            for (name, pattern) in fields {
                match pattern {
                    Some(pattern) => bind(pattern, bound),
                    None => {
                        bound.insert(name.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn references(node: &Node, name: &str) -> bool {
    let mut names = BTreeSet::new();
    free_names(node, &BTreeSet::new(), &mut names);
    names.contains(name)
}

fn free_names(node: &Node, bound: &BTreeSet<String>, result: &mut BTreeSet<String>) {
    match &node.kind {
        NodeKind::Name(name) if !bound.contains(name) => {
            result.insert(name.clone());
        }
        NodeKind::Neg(value) | NodeKind::Not(value) | NodeKind::Field(value, _) => {
            free_names(value, bound, result)
        }
        NodeKind::Bin(_, a, b) | NodeKind::Pipe(a, b) => {
            free_names(a, bound, result);
            free_names(b, bound, result);
        }
        NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
            free_names(callee, bound, result);
            for arg in args {
                free_names(arg, bound, result);
            }
        }
        NodeKind::List(items) => {
            for item in items {
                free_names(item, bound, result);
            }
        }
        NodeKind::Record(fields) => {
            for (_, value) in fields {
                free_names(value, bound, result);
            }
        }
        NodeKind::Let { pat, value, body } | NodeKind::LetQ { pat, value, body } => {
            free_names(value, bound, result);
            let mut bound = bound.clone();
            bind(pat, &mut bound);
            free_names(body, &bound, result);
        }
        NodeKind::DoQ { value, body } => {
            free_names(value, bound, result);
            free_names(body, bound, result);
        }
        NodeKind::Fn { params, body } => {
            let mut bound = bound.clone();
            bound.extend(params.iter().map(|p| p.name.clone()));
            free_names(body, &bound, result);
        }
        NodeKind::BoolBranch { cond, then, els } => {
            for child in [cond, then, els] {
                free_names(child, bound, result);
            }
        }
        NodeKind::CondBranch { arms, fallback } => {
            for (condition, body) in arms {
                free_names(condition, bound, result);
                free_names(body, bound, result);
            }
            if let Some(body) = fallback {
                free_names(body, bound, result);
            }
        }
        NodeKind::PatternBranch {
            scrutinee,
            arms,
            fallback,
        } => {
            free_names(scrutinee, bound, result);
            for (pattern, body) in arms {
                let mut bound = bound.clone();
                bind(pattern, &mut bound);
                free_names(body, &bound, result);
            }
            if let Some(body) = fallback {
                free_names(body, bound, result);
            }
        }
        _ => {}
    }
}

#[derive(Default)]
struct Namespaces {
    types: BTreeSet<String>,
    modules: BTreeSet<String>,
    constructors: BTreeSet<String>,
    values: BTreeSet<String>,
    signatures: BTreeSet<String>,
}

pub fn resolve(ast: &Ast, path: &str) -> Result<(), crate::parser::Diagnostic> {
    declarations(&ast.items, path)
}

fn declarations(items: &[Item], path: &str) -> Result<(), crate::parser::Diagnostic> {
    let mut names = Namespaces::default();
    for item in items {
        match item {
            Item::Def(def) => register(&mut names.values, &def.name, def.span, path)?,
            Item::SigDef(def) => register(&mut names.values, &def.name, def.span, path)?,
            Item::TypeDef(def) => {
                register(&mut names.types, &def.name, def.span, path)?;
                if let Some(body) = &def.ty {
                    register_constructors(body, &mut names.constructors, def.span, path)?;
                }
            }
            Item::ModuleImpl(module) => {
                register(&mut names.modules, &module.name, module.span, path)?;
                declarations(&module.items, path)?;
            }
            Item::ModuleSig(signature) => {
                register(&mut names.signatures, &signature.name, signature.span, path)?;
                declarations(&signature.items, path)?;
            }
        }
    }
    Ok(())
}

fn register_constructors(
    body: &crate::parser::TypeExpr,
    names: &mut BTreeSet<String>,
    span: crate::parser::Span,
    path: &str,
) -> Result<(), crate::parser::Diagnostic> {
    use crate::parser::TypeExpr;
    match body {
        TypeExpr::Adt(variants) => {
            for variant in variants {
                register_constructors(variant, names, span, path)?;
            }
        }
        TypeExpr::Name(name) | TypeExpr::Apply(name, _) => register(names, name, span, path)?,
        _ => {}
    }
    Ok(())
}

fn register(
    names: &mut BTreeSet<String>,
    name: &str,
    span: crate::parser::Span,
    path: &str,
) -> Result<(), crate::parser::Diagnostic> {
    if !names.insert(name.into()) {
        return Err(crate::parser::Diagnostic {
            path: path.into(),
            line: span.line,
            col: span.col,
            code: None,
            message: format!("duplicate declaration {name}"),
        });
    }
    Ok(())
}

pub(crate) fn check_cycles(ast: &Ast, path: &str) -> Result<(), crate::parser::Diagnostic> {
    for group in definition_groups(ast).into_iter().filter(|group| group.len() > 1) {
        let definitions: Vec<_> = ast.items.iter().filter_map(|item| match item {
            Item::Def(def) if group.contains(&def.name) => Some(def),
            _ => None,
        }).collect();
        let violation = definitions.iter().find(|def| matches!(def.kind, DefKind::Value(_)))
            .map(|def| (*def, "E514", "recursive cycles cannot include value definitions"))
            .or_else(|| definitions.iter().find(|def| !def.rec).map(|def| (*def, "E516", "every mutually recursive function requires rec")));
        if let Some((def, code, message)) = violation {
            return Err(crate::parser::Diagnostic { path: path.into(), line: def.span.line, col: def.span.col, code: Some(code.into()), message: format!("{message}: {}", group.join(", ")) });
        }
    }
    for item in &ast.items {
        if let Item::ModuleImpl(module) = item {
            check_cycles(&Ast { items: module.items.clone(), span: module.span, branches_after_fallback: vec![] }, path)?;
        }
    }
    Ok(())
}

fn dependency_graph(ast: &Ast) -> BTreeMap<String, BTreeSet<String>> {
    let mut graph = BTreeMap::new();
    for item in &ast.items {
        if let Item::Def(def) = item {
            let mut references = BTreeSet::new();
            match &def.kind {
                DefKind::Value(body) => free_names(body, &BTreeSet::new(), &mut references),
                DefKind::Function { params, body, .. } => {
                    let bound = params.iter().map(|p| p.name.clone()).collect();
                    free_names(body, &bound, &mut references);
                }
            }
            graph.insert(def.name.clone(), references);
        }
    }
    graph
}

pub fn unused_definitions(ast: &Ast, roots: &[String]) -> Vec<String> {
    let graph = dependency_graph(ast);
    graph.keys().filter(|name| !roots.iter().any(|root| reachable(root, name, &graph, &mut BTreeSet::new()))).cloned().collect()
}
