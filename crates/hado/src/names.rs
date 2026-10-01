use crate::parser::{Ast, DefKind, Item, Node, NodeKind, Pattern, PatternKind};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const BUILTIN_TYPES: &[&str] = &[
    "Int", "Float", "Bool", "Char", "String", "Bytes", "Array", "Void", "Fn",
];

pub(crate) fn is_upper_reference(name: &str) -> bool {
    name.rsplit("::")
        .next()
        .is_some_and(|name| name.starts_with(char::is_uppercase))
}

pub(crate) fn is_value_reference(name: &str) -> bool {
    !name.contains("::") && name.starts_with(char::is_lowercase)
}

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
        PatternKind::Name(name) if !crate::names::is_upper_reference(name) => {
            vec![(name, pattern.span)]
        }
        PatternKind::Constructor(_, args) => args.iter().flat_map(bindings).collect(),
        PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => fields
            .iter()
            .flat_map(|(name, nested)| match nested {
                Some(nested) => bindings(nested),
                None => vec![(name.name.as_str(), name.span)],
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn bind(pattern: &Pattern, bound: &mut BTreeSet<String>) {
    bound.extend(
        bindings(pattern)
            .into_iter()
            .map(|(name, _)| name.to_owned()),
    );
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
            free_names_all(args, bound, result);
        }
        NodeKind::List(items, tail) => {
            free_names_all(
                items.iter().chain(tail.iter().map(Box::as_ref)),
                bound,
                result,
            );
        }
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
            free_names_all(fields.iter().map(|(_, value)| value), bound, result);
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
            bound.extend(
                params
                    .iter()
                    .filter(|p| p.name != "_")
                    .map(|p| p.name.clone()),
            );
            free_names(body, &bound, result);
        }
        NodeKind::BoolBranch { cond, then, els } => {
            for child in [cond, then, els] {
                free_names(child, bound, result);
            }
        }
        NodeKind::CondBranch { arms, fallback } => {
            free_cond_branch(arms, fallback, bound, result);
        }
        NodeKind::PatternBranch {
            scrutinees,
            arms,
            fallback,
        } => {
            free_pattern_branch(scrutinees, arms, fallback, bound, result);
        }
        NodeKind::With { args, body, .. } => {
            free_names_all(args, bound, result);
            free_names(body, bound, result);
        }
        _ => {}
    }
}

fn free_names_all<'a>(
    nodes: impl IntoIterator<Item = &'a Node>,
    bound: &BTreeSet<String>,
    result: &mut BTreeSet<String>,
) {
    for node in nodes {
        free_names(node, bound, result);
    }
}

fn free_cond_branch(
    arms: &[(Node, Node)],
    fallback: &Option<Box<Node>>,
    bound: &BTreeSet<String>,
    result: &mut BTreeSet<String>,
) {
    for (condition, body) in arms {
        free_names(condition, bound, result);
        free_names(body, bound, result);
    }
    if let Some(body) = fallback {
        free_names(body, bound, result);
    }
}

fn free_pattern_branch(
    scrutinees: &[Node],
    arms: &[(Vec<Pattern>, Node)],
    fallback: &Option<Box<Node>>,
    bound: &BTreeSet<String>,
    result: &mut BTreeSet<String>,
) {
    free_names_all(scrutinees, bound, result);
    for (patterns, body) in arms {
        let mut bound = bound.clone();
        for pattern in patterns {
            bind(pattern, &mut bound);
        }
        free_names(body, &bound, result);
    }
    if let Some(body) = fallback {
        free_names(body, bound, result);
    }
}

/// §56: the tables keep the first declaration's position, so a duplicate can
/// name it as its related position.
type Declared = BTreeMap<String, crate::parser::Span>;

#[derive(Default)]
struct Namespaces {
    types: Declared,
    modules: Declared,
    constructors: Declared,
    values: Declared,
    signatures: Declared,
    effects: Declared,
}

pub fn resolve(ast: &Ast, path: &str) -> Result<(), crate::parser::Diagnostic> {
    let inspection = if crate::has_main_definition(ast) {
        crate::Inspection::Entry
    } else {
        crate::Inspection::Library
    };
    resolve_with_prelude(ast, path, None, inspection, 0)
}

pub(crate) fn resolve_with_prelude(
    ast: &Ast,
    path: &str,
    trusted_start: Option<usize>,
    inspection: crate::Inspection,
    prelude_items: usize,
) -> Result<(), crate::parser::Diagnostic> {
    let mut diagnostics = crate::name_resolution::diagnostics(ast, path, trusted_start);
    if let Err(error) = crate::type_realizations::check(ast, path, prelude_items) {
        diagnostics.push(error);
    }
    if let Err(error) = declarations(&ast.items, path) {
        diagnostics.push(error);
    }
    if let Err(error) = entry_form(ast, path, inspection) {
        diagnostics.push(error);
    }
    if let Err(error) = check_cycles(ast, path) {
        diagnostics.push(error);
    }
    record_field_duplicates(&ast.items, path, &mut diagnostics);
    declaration_types(
        &ast.items,
        &BTreeSet::new(),
        &BTreeMap::new(),
        &BTreeSet::new(),
        path,
        &mut diagnostics,
    );
    match diagnostics
        .into_iter()
        .map(|error| crate::check::arm_diagnostic(ast, error))
        .min_by_key(crate::diagnostic_order)
    {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn record_field_duplicates(
    items: &[Item],
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
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

fn expression_field_duplicates(
    node: &Node,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    if let NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) = &node.kind {
        let mut names = Declared::new();
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
    items: &[Item],
    inherited: &BTreeSet<String>,
    inherited_signatures: &BTreeMap<String, Vec<String>>,
    inherited_effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut types = visible_types(items, inherited);
    let effects = visible_effects(items, inherited_effects);
    let signatures = visible_signatures(items, inherited_signatures);
    hide_shadowed_module_types(items, &mut types);
    export_signature_types(items, &signatures, &mut types);
    for item in items {
        declaration_item(item, &types, &effects, &signatures, path, diagnostics);
    }
}

fn visible_types(items: &[Item], inherited: &BTreeSet<String>) -> BTreeSet<String> {
    let mut types = inherited.clone();
    types.extend(BUILTIN_TYPES.iter().map(|name| (*name).to_owned()));
    types.extend(items.iter().filter_map(|item| match item {
        Item::TypeDef(def) => Some(def.name.clone()),
        _ => None,
    }));
    types
}

fn visible_effects(items: &[Item], inherited: &BTreeSet<String>) -> BTreeSet<String> {
    let mut effects = inherited.clone();
    effects.extend(items.iter().filter_map(|item| match item {
        Item::Effect(effect) => Some(effect.name.clone()),
        _ => None,
    }));
    effects
}

fn visible_signatures(
    items: &[Item],
    inherited: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut signatures = inherited.clone();
    let mut local: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for item in items {
        if let Item::ModuleSig(signature) = item {
            local.entry(signature.name.clone()).or_default().extend(
                signature.items.iter().filter_map(|item| match item {
                    Item::TypeDef(def) => Some(def.name.clone()),
                    _ => None,
                }),
            );
        }
    }
    signatures.extend(
        local
            .into_iter()
            .map(|(name, types)| (name, types.into_iter().collect())),
    );
    signatures
}

fn hide_shadowed_module_types(items: &[Item], types: &mut BTreeSet<String>) {
    for item in items {
        if let Item::ModuleImpl(module) = item {
            let prefix = format!("{}.", module.name);
            types.retain(|name| !name.starts_with(&prefix));
        }
    }
}

fn export_signature_types(
    items: &[Item],
    signatures: &BTreeMap<String, Vec<String>>,
    types: &mut BTreeSet<String>,
) {
    for item in items {
        if let Item::ModuleImpl(module) = item {
            if !matches!(&module.form, crate::parser::ImplementationForm::Functor(_)) {
                if let Some(exported) = signatures.get(&module.sig) {
                    types.extend(
                        exported
                            .iter()
                            .map(|name| format!("{}.{name}", module.name)),
                    );
                }
            }
        }
    }
}

fn declaration_item(
    item: &Item,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    signatures: &BTreeMap<String, Vec<String>>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    match item {
        Item::ModuleSig(signature) => {
            declaration_module_sig(signature, types, effects, signatures, path, diagnostics)
        }
        Item::ModuleImpl(module) => {
            declaration_module_impl(module, types, effects, signatures, path, diagnostics)
        }
        Item::SigDef(def) => declaration_sig_def(def, types, effects, path, diagnostics),
        Item::TypeDef(def) => declaration_type_def(def, types, effects, path, diagnostics),
        Item::Def(def) => declaration_def(def, types, effects, path, diagnostics),
        Item::Effect(effect) => declaration_effect(effect, types, effects, path, diagnostics),
        Item::Handler(handler) => declaration_handler(handler, types, effects, path, diagnostics),
    }
}

fn declaration_module_sig(
    signature: &crate::parser::ModuleSig,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    signatures: &BTreeMap<String, Vec<String>>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut contract_types: BTreeSet<_> = types
        .iter()
        .filter(|name| !name.contains('.'))
        .cloned()
        .collect();
    for (contract, members) in signatures {
        contract_types.extend(members.iter().map(|member| format!("{contract}.{member}")));
    }
    declaration_types(
        &signature.items,
        &contract_types,
        signatures,
        effects,
        path,
        diagnostics,
    );
}

fn declaration_module_impl(
    module: &crate::parser::ModuleImpl,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    signatures: &BTreeMap<String, Vec<String>>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut local = types.clone();
    if let crate::parser::ImplementationForm::Functor(params) = &module.form {
        for param in params {
            let prefix = format!("{}.", param.name);
            local.retain(|name| !name.starts_with(&prefix));
            if let Some(exported) = signatures.get(&param.contract) {
                local.extend(exported.iter().map(|name| format!("{}.{name}", param.name)));
            }
        }
    }
    declaration_types(
        &module.items,
        &local,
        signatures,
        effects,
        path,
        diagnostics,
    );
}

fn declaration_sig_def(
    def: &crate::parser::SigDef,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut local = types.clone();
    local.extend(
        def.type_params
            .iter()
            .filter(|param| param.name != "_")
            .map(|param| param.name.clone()),
    );
    for ty in def
        .params
        .iter()
        .filter_map(|param| param.ty.as_ref())
        .chain(def.ret.iter())
    {
        resolve_type_names(ty, &local, effects, path, diagnostics);
    }
}

fn declaration_type_def(
    def: &crate::parser::TypeDef,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut local = types.clone();
    local.extend(
        def.params
            .iter()
            .filter(|param| param.name != "_")
            .map(|param| param.name.clone()),
    );
    if let Some(ty) = &def.ty {
        resolve_type_names(ty, &local, effects, path, diagnostics);
    }
}

fn declaration_def(
    def: &crate::parser::Def,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let body = match &def.kind {
        DefKind::Value(body) => body,
        DefKind::Function { params, ret, body } => {
            for ty in params
                .iter()
                .filter_map(|param| param.ty.as_ref())
                .chain(ret.iter())
            {
                resolve_type_names(ty, types, effects, path, diagnostics);
            }
            body
        }
    };
    expression_type_names(body, types, effects, path, diagnostics);
}

fn declaration_effect(
    effect: &crate::parser::EffectDecl,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut local = types.clone();
    local.extend(
        effect
            .type_params
            .iter()
            .filter(|param| param.name != "_")
            .map(|param| param.name.clone()),
    );
    for op in &effect.ops {
        for ty in op
            .params
            .iter()
            .filter_map(|param| param.ty.as_ref())
            .chain(op.ret.iter())
        {
            resolve_type_names(ty, &local, effects, path, diagnostics);
        }
    }
}

fn declaration_handler(
    handler: &crate::parser::HandlerDecl,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    let mut local = types.clone();
    local.extend(
        handler
            .type_params
            .iter()
            .filter(|param| param.name != "_")
            .map(|param| param.name.clone()),
    );
    for ty in handler.params.iter().filter_map(|param| param.ty.as_ref()) {
        resolve_type_names(ty, &local, effects, path, diagnostics);
    }
    for arm in &handler.arms {
        match arm {
            crate::parser::HandlerArm::Operation { params, body, .. } => {
                for ty in params.iter().filter_map(|param| param.ty.as_ref()) {
                    resolve_type_names(ty, &local, effects, path, diagnostics);
                }
                expression_type_names(body, &local, effects, path, diagnostics);
            }
            crate::parser::HandlerArm::Return { param, body, .. } => {
                if let Some(ty) = &param.ty {
                    resolve_type_names(ty, &local, effects, path, diagnostics);
                }
                expression_type_names(body, &local, effects, path, diagnostics);
            }
        }
    }
}

fn expression_type_names(
    node: &Node,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    if let NodeKind::Fn { params, .. } = &node.kind {
        for ty in params.iter().filter_map(|param| param.ty.as_ref()) {
            resolve_type_names(ty, types, effects, path, diagnostics);
        }
    }
    for child in crate::check::children(node) {
        expression_type_names(child, types, effects, path, diagnostics);
    }
}

fn resolve_type_names(
    ty: &crate::parser::TypeExpr,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    use crate::parser::TypeKind;
    if let TypeKind::Name(name) | TypeKind::Apply(name, _) = &ty.kind {
        if !types.contains(name) {
            diagnostics.push(crate::parser::Diagnostic {
                mentions: None,
                suggestion: None,
                related: None,
                path: path.into(),
                line: ty.name_span.line,
                col: ty.name_span.col,
                end_line: ty.name_span.line,
                end_col: ty.name_span.col,
                code: Some("E204".into()),
                message: format!("unknown type {name}"),
            });
        }
    }
    resolve_type_children(ty, types, effects, path, diagnostics);
    if let Some(crate::parser::EffectRow::Row(labels)) = &ty.effects {
        for label in labels {
            resolve_effect_label(label, types, effects, path, diagnostics);
        }
    }
}

fn resolve_type_children(
    ty: &crate::parser::TypeExpr,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    use crate::parser::TypeKind;
    match &ty.kind {
        TypeKind::Apply(_, args) | TypeKind::Group(args) => {
            for arg in args {
                resolve_type_names(arg, types, effects, path, diagnostics);
            }
        }
        TypeKind::Record(fields) => {
            for field in fields {
                resolve_type_names(&field.ty, types, effects, path, diagnostics);
            }
        }
        TypeKind::Adt(variants) => {
            for variant in variants {
                for payload in variant.payload.types() {
                    resolve_type_names(payload, types, effects, path, diagnostics);
                }
            }
        }
        TypeKind::Name(_) => {}
    }
}

fn resolve_effect_label(
    ty: &crate::parser::TypeExpr,
    types: &BTreeSet<String>,
    effects: &BTreeSet<String>,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    use crate::parser::TypeKind;
    match &ty.kind {
        TypeKind::Name(name) | TypeKind::Apply(name, _) => {
            if !effects.contains(name) {
                diagnostics.push(crate::parser::Diagnostic {
                    mentions: None,
                    suggestion: None,
                    related: None,
                    path: path.into(),
                    line: ty.name_span.line,
                    col: ty.name_span.col,
                    end_line: ty.name_span.line,
                    end_col: ty.name_span.col,
                    code: Some("E204".into()),
                    message: format!("unknown effect {name}"),
                });
            }
            if let TypeKind::Apply(_, args) = &ty.kind {
                for arg in args {
                    resolve_type_names(arg, types, effects, path, diagnostics);
                }
            }
        }
        _ => {}
    }
}

fn entry_form(
    ast: &Ast,
    path: &str,
    inspection: crate::Inspection,
) -> Result<(), crate::parser::Diagnostic> {
    let main = ast.items.iter().find_map(|item| match item {
        Item::Def(def) if def.name == "main" => Some(def),
        _ => None,
    });
    if main.is_some_and(|def| matches!(def.kind, DefKind::Value(_))) {
        return Ok(());
    }
    // A library inspection has no entry by definition (§52); an argument-taking
    // `main` is still E206 (§10).
    if inspection == crate::Inspection::Library && main.is_none() {
        return Ok(());
    }
    let span = main.map_or(crate::parser::Span::point(1, 1), |def| def.span);
    Err(crate::parser::Diagnostic {
        mentions: None,
        suggestion: None,
        related: None,
        path: path.into(),
        line: span.line,
        col: span.col,
        end_line: span.line,
        end_col: span.col,
        code: Some("E206".into()),
        message: "main must be a value definition".into(),
    })
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
                if let crate::parser::ImplementationForm::Functor(params) = &module.form {
                    let mut parameters = Declared::new();
                    for param in params {
                        register(&mut parameters, &param.name, param.name_span, path)?;
                    }
                }
                declarations(&module.items, path)?;
            }
            Item::ModuleSig(signature) => {
                register(
                    &mut names.signatures,
                    &signature.name,
                    signature.name_span,
                    path,
                )?;
                declarations(&signature.items, path)?;
            }
            Item::Effect(effect) => {
                register(&mut names.effects, &effect.name, effect.name_span, path)?;
                register_parameters(&effect.type_params, path)?;
                for op in &effect.ops {
                    register(&mut names.values, &op.name, op.name_span, path)?;
                }
            }
            Item::Handler(handler) => {
                register(&mut names.modules, &handler.name, handler.name_span, path)?;
                register_parameters(&handler.type_params, path)?;
            }
        }
    }
    Ok(())
}

fn register_parameters(
    parameters: &[crate::parser::DeclaredName],
    path: &str,
) -> Result<(), crate::parser::Diagnostic> {
    let mut names = Declared::new();
    for parameter in parameters {
        register(&mut names, &parameter.name, parameter.span, path)?;
    }
    Ok(())
}

fn register_type_members(
    body: &crate::parser::TypeExpr,
    names: &mut Declared,
    path: &str,
) -> Result<(), crate::parser::Diagnostic> {
    use crate::parser::TypeKind;
    match &body.kind {
        TypeKind::Adt(variants) => {
            for variant in variants {
                register(names, &variant.name.name, variant.name.span, path)?;
                if let crate::parser::ConstructorPayload::Named(fields) = &variant.payload {
                    let mut fields_seen = Declared::new();
                    for field in fields {
                        register(&mut fields_seen, &field.name.name, field.name.span, path)?;
                    }
                }
            }
        }
        TypeKind::Record(fields) => {
            let mut names = Declared::new();
            for field in fields {
                register(&mut names, &field.name.name, field.name.span, path)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn register(
    names: &mut Declared,
    name: &str,
    span: crate::parser::Span,
    path: &str,
) -> Result<(), crate::parser::Diagnostic> {
    if let Some(&first) = names.get(name) {
        return Err(crate::parser::Diagnostic {
            mentions: None,
            suggestion: None,
            related: Some(Box::new(crate::parser::Related {
                path: path.into(),
                line: first.line,
                col: first.col,
            })),
            path: path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: Some("E205".into()),
            message: format!("duplicate declaration {name}"),
        });
    }
    names.insert(name.into(), span);
    Ok(())
}

pub(crate) fn check_cycles(ast: &Ast, path: &str) -> Result<(), crate::parser::Diagnostic> {
    let mut diagnostics = Vec::new();
    for group in definition_groups(ast)
        .into_iter()
        .filter(|group| group.len() > 1)
    {
        collect_cycle_violations(&ast.items, &group, path, &mut diagnostics);
    }
    collect_module_cycle_violations(ast, path, &mut diagnostics);
    match diagnostics.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn cycle_violation(def: &crate::parser::Def) -> Option<(&'static str, &'static str)> {
    if matches!(def.kind, DefKind::Value(_)) {
        Some(("E207", "recursive cycles cannot include value definitions"))
    } else if !def.rec {
        Some(("E208", "every mutually recursive function requires rec"))
    } else {
        None
    }
}

fn collect_cycle_violations(
    items: &[Item],
    group: &[String],
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    for item in items {
        let Item::Def(def) = item else { continue };
        if !group.contains(&def.name) {
            continue;
        }
        if let Some((code, message)) = cycle_violation(def) {
            diagnostics.push(crate::parser::Diagnostic {
                mentions: None,
                suggestion: None,
                related: None,
                path: path.into(),
                line: def.span.line,
                col: def.span.col,
                end_line: def.span.line,
                end_col: def.span.col,
                code: Some(code.into()),
                message: format!("{message}: {}", group.join(", ")),
            });
        }
    }
}

fn collect_module_cycle_violations(
    ast: &Ast,
    path: &str,
    diagnostics: &mut Vec<crate::parser::Diagnostic>,
) {
    for item in &ast.items {
        if let Item::ModuleImpl(module) = item {
            if let Err(error) = check_cycles(
                &Ast {
                    items: module.items.clone(),
                    span: module.span,
                    library: None,
                    branches_after_fallback: vec![],
                    branch_anchors: vec![],
                    branch_spans: vec![],
                    written_list_patterns: vec![],
                    long_condition_functions: vec![],
                    equality_anchors: vec![],
                    qualified_type_anchors: vec![],
                    name_sites: vec![],
                },
                path,
            ) {
                diagnostics.push(error);
            }
        }
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
                    let bound = params
                        .iter()
                        .filter(|p| p.name != "_")
                        .map(|p| p.name.clone())
                        .collect();
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
    graph
        .keys()
        .filter(|name| {
            !roots
                .iter()
                .any(|root| reachable(root, name, &graph, &mut BTreeSet::new()))
        })
        .cloned()
        .collect()
}
