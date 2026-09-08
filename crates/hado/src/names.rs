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

pub(crate) fn bindings(pattern: &Pattern) -> Vec<(&str, crate::parser::Span)> {
    match &pattern.kind {
        PatternKind::Name(name) if !name.starts_with(char::is_uppercase) => vec![(name, pattern.span)],
        PatternKind::Constructor(_, args) => args.iter().flat_map(bindings).collect(),
        PatternKind::Record(fields) => fields.iter().flat_map(|(name, nested)| match nested {
            Some(nested) => bindings(nested),
            None => vec![(name.name.as_str(), name.span)],
        }).collect(),
        _ => Vec::new(),
    }
}

fn bind(pattern: &Pattern, bound: &mut BTreeSet<String>) {
    bound.extend(bindings(pattern).into_iter().map(|(name, _)| name.to_owned()));
}

pub(crate) fn references(node: &Node, name: &str) -> bool {
    let mut names = BTreeSet::new();
    free_names(node, &BTreeSet::new(), &mut names);
    names.contains(name)
}

pub(crate) fn free_names(node: &Node, bound: &BTreeSet<String>, result: &mut BTreeSet<String>) {
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
        NodeKind::List(items, tail) => {
            for item in items.iter().chain(tail.iter().map(Box::as_ref)) {
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
            bound.extend(params.iter().filter(|p| p.name != "_").map(|p| p.name.clone()));
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
    resolve_with_prelude(ast, path, None)
}

pub(crate) fn resolve_with_prelude(ast: &Ast, path: &str, trusted_start: Option<usize>) -> Result<(), crate::parser::Diagnostic> {
    let mut diagnostics = crate::name_resolution::diagnostics(ast, path, trusted_start);
    if let Err(error) = crate::type_realizations::check(ast, path) { diagnostics.push(error); }
    if let Err(error) = declarations(&ast.items, path) { diagnostics.push(error); }
    if let Err(error) = entry_form(ast, path) { diagnostics.push(error); }
    if let Err(error) = check_cycles(ast, path) { diagnostics.push(error); }
    record_field_duplicates(&ast.items, path, &mut diagnostics);
    declaration_types(&ast.items, &BTreeSet::new(), &BTreeMap::new(), path, &mut diagnostics);
    match diagnostics.into_iter().map(|error| crate::check::arm_diagnostic(ast, error)).min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn record_field_duplicates(items: &[Item], path: &str, diagnostics: &mut Vec<crate::parser::Diagnostic>) {
    for item in items {
        match item {
            Item::Def(def) => {
                let (DefKind::Value(body) | DefKind::Function { body, .. }) = &def.kind;
                expression_field_duplicates(body, path, diagnostics);
            }
            Item::ModuleImpl(module) => record_field_duplicates(&module.items, path, diagnostics),
            _ => {}
        }
    }
}

fn expression_field_duplicates(node: &Node, path: &str, diagnostics: &mut Vec<crate::parser::Diagnostic>) {
    if let NodeKind::Record(fields) = &node.kind {
        let mut names = BTreeSet::new();
        for (name, _) in fields {
            if let Err(error) = register(&mut names, &name.name, name.span, path) {
                diagnostics.push(error);
            }
        }
    }
    for child in crate::check::children(node) {
        expression_field_duplicates(child, path, diagnostics);
    }
}

fn declaration_types(
    items: &[Item], inherited: &BTreeSet<String>, inherited_signatures: &BTreeMap<String, Vec<String>>, path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut types = inherited.clone();
    types.extend(["Int", "Float", "Bool", "Char", "String", "Unit", "Fn"].map(str::to_owned));
    types.extend(items.iter().filter_map(|item| match item {
        Item::TypeDef(def) => Some(def.name.clone()),
        _ => None,
    }));
    let mut signatures = inherited_signatures.clone();
    let mut local_signatures: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for item in items {
        if let Item::ModuleSig(signature) = item {
            local_signatures.entry(signature.name.clone()).or_default().extend(signature.items.iter().filter_map(|item| match item {
                Item::TypeDef(def) => Some(def.name.clone()),
                _ => None,
            }));
        }
    }
    signatures.extend(local_signatures.into_iter().map(|(name, types)| (name, types.into_iter().collect())));
    for item in items {
        if let Item::ModuleImpl(module) = item {
            let prefix = format!("{}.", module.name);
            types.retain(|name| !name.starts_with(&prefix));
        }
    }
    for item in items {
        if let Item::ModuleImpl(module) = item {
            if let Some(exported) = signatures.get(&module.sig) {
                types.extend(exported.iter().map(|name| format!("{}.{name}", module.name)));
            }
        }
    }
    for item in items {
        match item {
            Item::ModuleSig(signature) => {
                let mut contract_types: BTreeSet<_> = types.iter().filter(|name| !name.contains('.')).cloned().collect();
                for (contract, members) in &signatures {
                    contract_types.extend(members.iter().map(|member| format!("{contract}.{member}")));
                }
                declaration_types(&signature.items, &contract_types, &signatures, path, diagnostics);
            }
            Item::ModuleImpl(module) => declaration_types(&module.items, &types, &signatures, path, diagnostics),
            Item::SigDef(def) => {
                let mut local = types.clone();
                local.extend(def.type_params.iter().filter(|param| param.name != "_").map(|param| param.name.clone()));
                for ty in def.params.iter().filter_map(|param| param.ty.as_ref()).chain(def.ret.iter()) {
                    resolve_type_names(ty, &local, path, diagnostics);
                }
            }
            Item::TypeDef(def) => {
                let mut local = types.clone();
                local.extend(def.params.iter().filter(|param| param.name != "_").map(|param| param.name.clone()));
                if let Some(ty) = &def.ty { resolve_type_names(ty, &local, path, diagnostics); }
            }
            Item::Def(def) => {
                let body = match &def.kind {
                    DefKind::Value(body) => body,
                    DefKind::Function { params, ret, body } => {
                        for ty in params.iter().filter_map(|param| param.ty.as_ref()).chain(ret.iter()) {
                            resolve_type_names(ty, &types, path, diagnostics);
                        }
                        body
                    }
                };
                expression_type_names(body, &types, path, diagnostics);
            }
        }
    }
}

fn expression_type_names(
    node: &Node, types: &BTreeSet<String>, path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    if let NodeKind::Fn { params, .. } = &node.kind {
        for ty in params.iter().filter_map(|param| param.ty.as_ref()) {
            resolve_type_names(ty, types, path, diagnostics);
        }
    }
    for child in crate::check::children(node) { expression_type_names(child, types, path, diagnostics); }
}

fn resolve_type_names(
    ty: &crate::parser::TypeExpr, types: &BTreeSet<String>, path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    use crate::parser::TypeKind;
    match &ty.kind {
        TypeKind::Name(name) | TypeKind::Apply(name, _) => {
            if !types.contains(name) {
                diagnostics.push(crate::parser::Diagnostic {
                    path: path.into(), line: ty.span.line, col: ty.span.col,
                    code: Some("E204".into()), message: format!("unknown type {name}"),
                });
            }
            if let TypeKind::Apply(_, args) = &ty.kind {
                for arg in args { resolve_type_names(arg, types, path, diagnostics); }
            }
        }
        TypeKind::Group(args) => {
            for arg in args { resolve_type_names(arg, types, path, diagnostics); }
        }
        TypeKind::Record(fields) => {
            for field in fields { resolve_type_names(&field.ty, types, path, diagnostics); }
        }
        TypeKind::Adt(variants) => {
            for variant in variants {
                if let TypeKind::Apply(_, payloads) = &variant.kind {
                    for payload in payloads { resolve_type_names(payload, types, path, diagnostics); }
                }
            }
        }
    }
}

fn entry_form(ast: &Ast, path: &str) -> Result<(), crate::parser::Diagnostic> {
    let main = ast.items.iter().find_map(|item| match item {
        Item::Def(def) if def.name == "main" => Some(def),
        _ => None,
    });
    if main.is_some_and(|def| matches!(def.kind, DefKind::Value(_))) { return Ok(()); }
    let span = main.map_or(crate::parser::Span { line: 1, col: 1 }, |def| def.span);
    Err(crate::parser::Diagnostic { path: path.into(), line: span.line, col: span.col,
        code: Some("E206".into()), message: "main must be a value definition".into() })
}

fn declarations(items: &[Item], path: &str) -> Result<(), crate::parser::Diagnostic> {
    let mut names = Namespaces::default();
    for item in items {
        match item {
            Item::Def(def) => register(&mut names.values, &def.name, def.name_span, path)?,
            Item::SigDef(def) => {
                register(&mut names.values, &def.name, def.name_span, path)?;
                register_parameters(&def.type_params, path)?;
            }
            Item::TypeDef(def) => {
                register(&mut names.types, &def.name, def.name_span, path)?;
                register_parameters(&def.params, path)?;
                if let Some(body) = &def.ty {
                    register_type_members(body, &mut names.constructors, path)?;
                }
            }
            Item::ModuleImpl(module) => {
                register(&mut names.modules, &module.name, module.name_span, path)?;
                declarations(&module.items, path)?;
            }
            Item::ModuleSig(signature) => {
                register(&mut names.signatures, &signature.name, signature.name_span, path)?;
                declarations(&signature.items, path)?;
            }
        }
    }
    Ok(())
}

fn register_parameters(parameters: &[crate::parser::DeclaredName], path: &str) -> Result<(), crate::parser::Diagnostic> {
    let mut names = BTreeSet::new();
    for parameter in parameters { register(&mut names, &parameter.name, parameter.span, path)?; }
    Ok(())
}

fn register_type_members(
    body: &crate::parser::TypeExpr,
    names: &mut BTreeSet<String>,
    path: &str,
) -> Result<(), crate::parser::Diagnostic> {
    use crate::parser::TypeKind;
    match &body.kind {
        TypeKind::Adt(variants) => {
            for variant in variants {
                if let TypeKind::Name(name) | TypeKind::Apply(name, _) = &variant.kind {
                    register(names, name, variant.span, path)?;
                }
            }
        }
        TypeKind::Record(fields) => {
            let mut names = BTreeSet::new();
            for field in fields { register(&mut names, &field.name.name, field.name.span, path)?; }
        }
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
            code: Some("E205".into()),
            message: format!("duplicate declaration {name}"),
        });
    }
    Ok(())
}

pub(crate) fn check_cycles(ast: &Ast, path: &str) -> Result<(), crate::parser::Diagnostic> {
    let mut diagnostics = Vec::new();
    for group in definition_groups(ast).into_iter().filter(|group| group.len() > 1) {
        for item in &ast.items {
            let Item::Def(def) = item else { continue };
            if !group.contains(&def.name) { continue; }
            let violation = if matches!(def.kind, DefKind::Value(_)) {
                Some(("E207", "recursive cycles cannot include value definitions"))
            } else if !def.rec {
                Some(("E208", "every mutually recursive function requires rec"))
            } else { None };
            if let Some((code, message)) = violation {
                diagnostics.push(crate::parser::Diagnostic { path: path.into(), line: def.span.line, col: def.span.col,
                    code: Some(code.into()), message: format!("{message}: {}", group.join(", ")) });
            }
        }
    }
    for item in &ast.items {
        if let Item::ModuleImpl(module) = item {
            if let Err(error) = check_cycles(&Ast { items: module.items.clone(), span: module.span, branches_after_fallback: vec![], branch_anchors: vec![], written_list_patterns: vec![], long_condition_functions: vec![], equality_anchors: vec![], qualified_type_anchors: vec![] }, path) {
                diagnostics.push(error);
            }
        }
    }
    match diagnostics.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn dependency_graph(ast: &Ast) -> BTreeMap<String, BTreeSet<String>> {
    let mut graph = BTreeMap::new();
    for item in &ast.items {
        if let Item::Def(def) = item {
            let mut references = BTreeSet::new();
            match &def.kind {
                DefKind::Value(body) => free_names(body, &BTreeSet::new(), &mut references),
                DefKind::Function { params, body, .. } => {
                    let bound = params.iter().filter(|p| p.name != "_").map(|p| p.name.clone()).collect();
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
