use crate::parser::ImplementationForm;
use crate::parser::{
    Ast, ConstructorDecl, DefKind, Diagnostic, HandlerArm, HandlerDecl, Item, Node, NodeKind,
    Param, Pattern, Span, TypeExpr, TypeField, TypeKind,
};
use crate::types::Ty;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Value(String),
    Type(String),
    Signature(String),
    Module(String),
    Effect(String),
    Handler(String),
}

type References = BTreeSet<Key>;

/// 宣言から、それが参照する宣言への辺。`Collector::graph` が組み立てる。
type Graph = BTreeMap<Key, References>;

fn declaration(item: &Item) -> Option<(Key, Span)> {
    Some(match item {
        Item::Def(d) => (Key::Value(d.name.clone()), d.span),
        Item::TypeDef(d) => (Key::Type(d.name.clone()), d.span),
        Item::ModuleSig(d) => (Key::Signature(d.name.clone()), d.span),
        Item::ModuleImpl(d) => (Key::Module(d.name.clone()), d.span),
        Item::Effect(d) => (Key::Effect(d.name.clone()), d.span),
        Item::Handler(d) => (Key::Handler(d.name.clone()), d.span),
        Item::SigDef(_) => return None,
    })
}

pub(crate) fn diagnostics(
    ast: &Ast,
    checked: &crate::Checked,
    prelude_items: usize,
    other_entry_roots: &[String],
    inspection: crate::Inspection,
    path: &str,
) -> Vec<Diagnostic> {
    let collector = Collector::new(Some(checked), &ast.items);
    let graph = collector.graph(ast);
    let used = roots(ast, other_entry_roots, inspection, &graph);
    let used = close_over_handlers(ast, &collector, &graph, used);
    let mut diagnostics = unused_declarations(ast, prelude_items, inspection, &used, path);
    diagnostics.extend(module_diagnostics(
        ast,
        checked,
        &collector,
        prelude_items,
        path,
    ));
    diagnostics
}

/// The roots of the reference graph. A1 walks from `main` and the other entry
/// roots; A2 makes the public surface the root, so outer `def`s are not roots
/// and prelude items keep their `.skip(prelude_items)` exemption (§55).
fn roots(
    ast: &Ast,
    other_entry_roots: &[String],
    inspection: crate::Inspection,
    graph: &Graph,
) -> References {
    let mut used = References::new();
    match inspection {
        crate::Inspection::Entry => {
            used.extend(transitive_references(&Key::Value("main".into()), graph));
            for name in other_entry_roots {
                used.extend(transitive_references(&Key::Value(name.clone()), graph));
            }
        }
        crate::Inspection::Library => {
            for item in &ast.items {
                let key = match item {
                    Item::ModuleImpl(module) => Key::Module(module.name.clone()),
                    Item::ModuleSig(contract) => Key::Signature(contract.name.clone()),
                    Item::TypeDef(def) => Key::Type(def.name.clone()),
                    Item::Effect(effect) => Key::Effect(effect.name.clone()),
                    Item::Handler(handler) => Key::Handler(handler.name.clone()),
                    _ => continue,
                };
                used.extend(transitive_references(&key, graph));
            }
        }
    }
    used
}

/// A handler is part of the program, so its head names an effect that is used,
/// and what its arms reach is used just like what `main` or a module reaches:
/// close it transitively (§38・§39).
fn close_over_handlers(
    ast: &Ast,
    collector: &Collector,
    graph: &Graph,
    mut used: References,
) -> References {
    let scope = Scope::default();
    let mut handler_references = References::new();
    for item in &ast.items {
        if let Item::Handler(handler) = item {
            handler_references.insert(Key::Effect(handler.effect.clone()));
            collector.handler(handler, &scope, &mut handler_references);
        }
    }
    for key in handler_references {
        used.extend(transitive_references(&key, graph));
    }
    used
}

/// E513: a top-level declaration that nothing reaches.
fn unused_declarations(
    ast: &Ast,
    prelude_items: usize,
    inspection: crate::Inspection,
    used: &References,
    path: &str,
) -> Vec<Diagnostic> {
    ast.items
        .iter()
        .skip(prelude_items)
        .filter_map(|item| {
            let (key, span) = declaration(item)?;
            let referenced = match &key {
                Key::Signature(_) if inspection == crate::Inspection::Library => true,
                Key::Signature(name) => ast.items.iter().any(|item| {
                    matches!(item, Item::ModuleImpl(module) if &module.sig == name
                        && !matches!(module.form, ImplementationForm::Functor(_)))
                }),
                _ => used.contains(&key),
            };
            (!referenced).then(|| Diagnostic {
                mentions: None,
                suggestion: None,
                related: None,
                path: path.into(),
                line: span.line,
                col: span.col,
                end_line: span.line,
                end_col: span.col,
                code: Some("E513".into()),
                message: "unused top-level declaration".into(),
            })
        })
        .collect()
}

/// E521: a functor parameter that the body never names, plus each module's
/// own local data diagnostics.
fn module_diagnostics(
    ast: &Ast,
    checked: &crate::Checked,
    collector: &Collector,
    prelude_items: usize,
    path: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for item in ast.items.iter().skip(prelude_items) {
        let Item::ModuleImpl(module) = item else {
            continue;
        };
        diagnostics.extend(local_data_diagnostics(ast, module, checked, path));
        let ImplementationForm::Functor(parameters) = &module.form else {
            continue;
        };
        let mut references = References::new();
        collector.items(&module.items, &Scope::default(), &mut references);
        for parameter in parameters {
            if references.contains(&Key::Module(parameter.name.clone())) {
                continue;
            }
            diagnostics.push(Diagnostic {
                mentions: None,
                suggestion: None,
                related: None,
                path: path.into(),
                line: parameter.name_span.line,
                col: parameter.name_span.col,
                end_line: parameter.name_span.line,
                end_col: parameter.name_span.col,
                code: Some("E521".into()),
                message: "unused functor parameter".into(),
            });
        }
    }
    diagnostics
}

fn local_data_diagnostics(
    ast: &Ast,
    module: &crate::parser::ModuleImpl,
    checked: &crate::Checked,
    path: &str,
) -> Vec<Diagnostic> {
    let collector = Collector::new(Some(checked), &module.items);
    let scope = Scope {
        local_module: Some(module.name.clone()),
        ..Scope::default()
    };
    let graph = collector.graph_items(&module.items, &scope);
    let mut used = References::new();
    for item in &ast.items {
        if let Item::ModuleSig(contract) = item {
            if contract.name != module.sig {
                continue;
            }
            for member in &contract.items {
                let key = match member {
                    Item::SigDef(def) => Key::Value(def.name.clone()),
                    Item::TypeDef(def) => Key::Type(def.name.clone()),
                    _ => continue,
                };
                used.extend(transitive_references(&key, &graph));
            }
        }
    }
    module
        .items
        .iter()
        .filter_map(|item| {
            let Item::TypeDef(def) = item else {
                return None;
            };
            if !matches!(
                def.ty.as_ref().map(|ty| &ty.kind),
                Some(TypeKind::Record(_) | TypeKind::Adt(_))
            ) || used.contains(&Key::Type(def.name.clone()))
            {
                return None;
            }
            Some(Diagnostic {
                mentions: None,
                suggestion: None,
                related: None,
                path: path.into(),
                line: def.span.line,
                col: def.span.col,
                end_line: def.span.line,
                end_col: def.span.col,
                code: Some("E513".into()),
                message: "unused implementation-local type".into(),
            })
        })
        .collect()
}

pub(crate) fn inference_groups(ast: &Ast) -> Vec<Vec<&Item>> {
    let collector = Collector::new(None, &ast.items);
    let mut graph = collector.graph(ast);
    // §37 / §38: the unused judgment needs the effects and the handlers, but
    // the inference order does not (55 節 決定 8).
    graph.retain(|key, _| !matches!(key, Key::Effect(_) | Key::Handler(_)));
    let keys: Vec<_> = ast
        .items
        .iter()
        .filter_map(|item| declaration(item).map(|(key, _)| key))
        .collect();
    let mut groups = Vec::new();
    while !graph.is_empty() {
        let closure: BTreeMap<_, _> = graph
            .keys()
            .map(|key| (key.clone(), transitive_references(key, &graph)))
            .collect();
        let first = keys
            .iter()
            .filter(|key| graph.contains_key(*key))
            .find(|key| {
                graph.keys().all(|dependency| {
                    !closure[*key].contains(dependency) || closure[dependency].contains(*key)
                })
            })
            .expect("a finite dependency graph has a sink component");
        let component: References = graph
            .keys()
            .filter(|key| closure[first].contains(*key) && closure[*key].contains(first))
            .cloned()
            .collect();
        let group = ast
            .items
            .iter()
            .filter(|item| declaration(item).is_some_and(|(key, _)| component.contains(&key)))
            .collect();
        for key in component {
            graph.remove(&key);
        }
        groups.push(group);
    }
    groups
}

fn transitive_references(start: &Key, graph: &BTreeMap<Key, References>) -> References {
    let mut visited = References::new();
    let mut pending = vec![start.clone()];
    while let Some(key) = pending.pop() {
        if visited.insert(key.clone()) {
            if let Some(next) = graph.get(&key) {
                pending.extend(next.iter().cloned());
            }
        }
    }
    visited
}

fn constructor_owners(items: &[Item]) -> BTreeMap<String, Key> {
    let mut owners = BTreeMap::new();
    for item in items {
        if let Item::TypeDef(def) = item {
            if let Some(TypeExpr {
                kind: TypeKind::Adt(variants),
                ..
            }) = &def.ty
            {
                for variant in variants {
                    owners.insert(variant.name.name.clone(), Key::Type(def.name.clone()));
                }
            }
        }
    }
    owners
}

#[derive(Clone, Default)]
struct Scope {
    values: BTreeSet<String>,
    types: BTreeSet<String>,
    modules: BTreeSet<String>,
    signatures: BTreeSet<String>,
    constructors: BTreeSet<String>,
    local_module: Option<String>,
}

impl Scope {
    fn nested(&self, items: &[Item]) -> Self {
        let mut scope = self.clone();
        for item in items {
            match item {
                Item::Def(d) => {
                    scope.values.insert(d.name.clone());
                }
                Item::TypeDef(d) => {
                    scope.types.insert(d.name.clone());
                }
                Item::ModuleSig(d) => {
                    scope.signatures.insert(d.name.clone());
                }
                Item::ModuleImpl(d) => {
                    scope.modules.insert(d.name.clone());
                }
                Item::SigDef(_) | Item::Effect(_) | Item::Handler(_) => {}
            }
        }
        scope
            .constructors
            .extend(constructor_owners(items).into_keys());
        scope
    }
}

struct Collector<'a> {
    checked: Option<&'a crate::Checked>,
    constructors: BTreeMap<String, Key>,
    /// §37: the declared effects. A row that names one uses it.
    effects: BTreeSet<String>,
    /// §37: an operation name to the effect that declares it. A call site spells
    /// the name, so a reference to it is a reference to the effect.
    operations: BTreeMap<String, Key>,
    /// §38: a handler name to its declaration, for the names `with` spells.
    handlers: BTreeMap<String, Key>,
}

impl<'a> Collector<'a> {
    fn new(checked: Option<&'a crate::Checked>, items: &[Item]) -> Self {
        let mut effects = BTreeSet::new();
        let mut operations = BTreeMap::new();
        let mut handlers = BTreeMap::new();
        for item in items {
            match item {
                Item::Effect(effect) => {
                    effects.insert(effect.name.clone());
                    for op in &effect.ops {
                        operations.insert(op.name.clone(), Key::Effect(effect.name.clone()));
                    }
                }
                Item::Handler(handler) => {
                    handlers.insert(handler.name.clone(), Key::Handler(handler.name.clone()));
                }
                _ => {}
            }
        }
        Collector {
            checked,
            constructors: constructor_owners(items),
            effects,
            operations,
            handlers,
        }
    }

    fn graph(&self, ast: &Ast) -> BTreeMap<Key, References> {
        let mut graph = self.graph_items(&ast.items, &Scope::default());
        // §37: a name that denotes a declared effect is a use of that effect
        // wherever it appears — an effect row, a parameter's type, a signature.
        for references in graph.values_mut() {
            let effects: Vec<Key> = references
                .iter()
                .filter_map(|reference| match reference {
                    Key::Type(name) if self.effects.contains(name) => {
                        Some(Key::Effect(name.clone()))
                    }
                    _ => None,
                })
                .collect();
            references.extend(effects);
        }
        let contracts = crate::contract_types::Contracts::collect(&ast.items);
        for (key, references) in &mut graph {
            if matches!(key, Key::Signature(_)) {
                *references = references
                    .iter()
                    .map(|reference| match reference {
                        Key::Module(name) => contracts.implementation(name).map_or_else(
                            || reference.clone(),
                            |implementation| Key::Module(implementation.into()),
                        ),
                        _ => reference.clone(),
                    })
                    .collect();
            }
        }
        graph
    }

    fn graph_items(&self, items: &[Item], scope: &Scope) -> BTreeMap<Key, References> {
        items
            .iter()
            .filter_map(|item| {
                let (key, _) = declaration(item)?;
                let mut references = References::new();
                self.item(item, scope, &mut references);
                Some((key, references))
            })
            .collect()
    }

    fn item(&self, item: &Item, scope: &Scope, references: &mut References) {
        match item {
            Item::Def(def) => self.definition(&def.kind, scope, references),
            Item::TypeDef(def) => self.type_definition(def, scope, references),
            Item::SigDef(def) => self.signature_definition(def, scope, references),
            Item::ModuleSig(def) => self.items(&def.items, scope, references),
            Item::ModuleImpl(def) => self.module_implementation(def, scope, references),
            Item::Effect(effect) => self.effect_definition(effect, scope, references),
            Item::Handler(handler) => self.handler_definition(handler, scope, references),
        }
    }

    fn type_definition(
        &self,
        def: &crate::parser::TypeDef,
        scope: &Scope,
        references: &mut References,
    ) {
        let scope = extend_types(scope, def.params.iter().map(|p| p.name.clone()));
        if let Some(ty) = &def.ty {
            type_expression(ty, &scope, references);
        }
    }

    fn signature_definition(
        &self,
        def: &crate::parser::SigDef,
        scope: &Scope,
        references: &mut References,
    ) {
        let scope = extend_types(scope, def.type_params.iter().map(|p| p.name.clone()));
        for param in &def.params {
            if let Some(ty) = &param.ty {
                type_expression(ty, &scope, references);
            }
        }
        if let Some(ty) = &def.ret {
            type_expression(ty, &scope, references);
        }
    }

    fn module_implementation(
        &self,
        def: &crate::parser::ModuleImpl,
        scope: &Scope,
        references: &mut References,
    ) {
        if !scope.signatures.contains(&def.sig) {
            references.insert(Key::Signature(def.sig.clone()));
        }
        let body_scope = extend_signature_scope(scope, &def.form, references);
        self.items(&def.items, &body_scope, references);
    }

    fn effect_definition(
        &self,
        effect: &crate::parser::EffectDecl,
        scope: &Scope,
        references: &mut References,
    ) {
        // §37 / 55 節 決定 4: what the operations' types name is used as
        // soon as the effect itself is.
        let scope = extend_types(scope, effect.type_params.iter().map(|p| p.name.clone()));
        for op in &effect.ops {
            for param in &op.params {
                if let Some(ty) = &param.ty {
                    type_expression(ty, &scope, references);
                }
            }
            if let Some(ty) = &op.ret {
                type_expression(ty, &scope, references);
            }
        }
    }

    /// §38: the head names the effect it handles, so the effect is used. 値引数の
    /// 型はこの head の scope で歩き、本体の腕は `handler` に渡す。
    fn handler_definition(
        &self,
        handler: &HandlerDecl,
        scope: &Scope,
        references: &mut References,
    ) {
        let scope = extend_types(scope, handler.type_params.iter().map(|p| p.name.clone()));
        references.insert(Key::Effect(handler.effect.clone()));
        for param in &handler.params {
            if let Some(ty) = &param.ty {
                type_expression(ty, &scope, references);
            }
        }
        self.handler(handler, &scope, references);
    }

    fn handler(&self, handler: &HandlerDecl, scope: &Scope, references: &mut References) {
        for arm in &handler.arms {
            match arm {
                HandlerArm::Operation { params, body, .. } => {
                    let mut bound = scope.values.clone();
                    bound.extend(params.iter().map(|param| param.name.clone()));
                    self.body(body, scope, &bound, references);
                }
                HandlerArm::Return { param, body, .. } => {
                    let mut bound = scope.values.clone();
                    bound.insert(param.name.clone());
                    self.body(body, scope, &bound, references);
                }
            }
        }
    }

    fn body(
        &self,
        body: &Node,
        scope: &Scope,
        bound: &BTreeSet<String>,
        references: &mut References,
    ) {
        let mut free = BTreeSet::new();
        crate::names::free_names(body, bound, &mut free);
        references.extend(
            free.iter()
                .filter(|name| crate::names::is_value_reference(name))
                .cloned()
                .map(Key::Value),
        );
        self.expression(body, scope, references);
    }

    fn items(&self, items: &[Item], scope: &Scope, references: &mut References) {
        let nested = scope.nested(items);
        for item in items {
            self.item(item, &nested, references);
        }
    }

    fn definition(&self, def: &DefKind, scope: &Scope, references: &mut References) {
        let mut bound = scope.values.clone();
        let body = match def {
            DefKind::Value(body) => body,
            DefKind::Function { params, ret, body } => {
                for param in params {
                    bound.insert(param.name.clone());
                    if let Some(ty) = &param.ty {
                        type_expression(ty, scope, references);
                    }
                }
                if let Some(ty) = ret {
                    type_expression(ty, scope, references);
                }
                body
            }
        };
        self.body(body, scope, &bound, references);
    }

    /// 名前がこの scope で束縛されていなければ、その名前が指す宣言を辺に足す。
    /// 構築子・操作・handler のどれも「裸の名前で呼ばれる」ので判断を 1 か所に置く。
    fn reference_unless_shadowed(
        &self,
        name: &str,
        scope: &Scope,
        table: &BTreeMap<String, Key>,
        references: &mut References,
    ) {
        if scope.constructors.contains(name) {
            return;
        }
        if let Some(owner) = table.get(name) {
            references.insert(owner.clone());
        }
    }

    fn expression(&self, node: &Node, scope: &Scope, references: &mut References) {
        self.record_inferred_type(node, scope, references);
        if self.insert_module_field_edge(node, scope, references) {
            return;
        }
        self.expression_kind(node, scope, references);
        for child in crate::check::children(node) {
            self.expression(child, scope, references);
        }
    }

    /// 推論済みの型が付いていれば、その型が指す宣言への辺を足す。
    fn record_inferred_type(&self, node: &Node, scope: &Scope, references: &mut References) {
        let Some(ty) = self.checked.and_then(|checked| {
            checked
                .composite_types
                .get(&(node.span.line, node.span.col))
        }) else {
            return;
        };
        inferred_type(ty, scope, references);
    }

    /// `M.field` の `M` が大文字の名前なら子は歩かない（`M` の綴りはここで
    /// 使い切っている）。外の module を指すときだけその module への辺を足す。
    fn insert_module_field_edge(
        &self,
        node: &Node,
        scope: &Scope,
        references: &mut References,
    ) -> bool {
        let NodeKind::Field(base, _) = &node.kind else {
            return false;
        };
        let NodeKind::Name(name) = &base.kind else {
            return false;
        };
        if !crate::names::is_upper_reference(name) {
            return false;
        }
        if !scope.modules.contains(name) {
            references.insert(Key::Module(name.clone()));
        }
        true
    }

    /// 腕の本体はその腕の名前を持つ関数に置く。`match` は振り分けだけを持つ。
    fn expression_kind(&self, node: &Node, scope: &Scope, references: &mut References) {
        match &node.kind {
            NodeKind::Name(name) => self.expression_name(name, scope, references),
            NodeKind::With { handler, .. } => self.expression_with(&handler.name, references),
            NodeKind::NamedConstructor(name, _) => {
                self.reference_unless_shadowed(&name.name, scope, &self.constructors, references);
            }
            NodeKind::Let { pat, .. } | NodeKind::LetQ { pat, .. } => {
                self.pattern(pat, scope, references)
            }
            NodeKind::PatternBranch { arms, .. } => {
                self.expression_pattern_branch(arms, scope, references)
            }
            NodeKind::Fn { params, .. } => self.expression_fn(params, scope, references),
            _ => {}
        }
    }

    fn expression_name(&self, name: &str, scope: &Scope, references: &mut References) {
        self.reference_unless_shadowed(name, scope, &self.constructors, references);
        // §37: an operation is called by its bare name.
        if let Some(effect) = self.operations.get(name) {
            references.insert(effect.clone());
        }
    }

    fn expression_with(&self, handler: &str, references: &mut References) {
        if let Some(key) = self.handlers.get(handler) {
            references.insert(key.clone());
        }
    }

    fn expression_pattern_branch(
        &self,
        arms: &[(Vec<Pattern>, Node)],
        scope: &Scope,
        references: &mut References,
    ) {
        for (patterns, _) in arms {
            for pattern in patterns {
                self.pattern(pattern, scope, references);
            }
        }
    }

    fn expression_fn(&self, params: &[Param], scope: &Scope, references: &mut References) {
        for param in params {
            if let Some(ty) = &param.ty {
                type_expression(ty, scope, references);
            }
        }
    }
    fn pattern(
        &self,
        pattern: &crate::parser::Pattern,
        scope: &Scope,
        references: &mut References,
    ) {
        use crate::parser::PatternKind;
        let name = match &pattern.kind {
            PatternKind::Name(name) | PatternKind::Constructor(name, _) => Some(name.as_str()),
            PatternKind::NamedConstructor(name, _) => Some(name.name.as_str()),
            _ => None,
        };
        if let Some(name) = name {
            self.reference_unless_shadowed(name, scope, &self.constructors, references);
        }
        match &pattern.kind {
            PatternKind::Constructor(_, args) => {
                for arg in args {
                    self.pattern(arg, scope, references);
                }
            }
            PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => {
                for nested in fields.iter().filter_map(|(_, nested)| nested.as_ref()) {
                    self.pattern(nested, scope, references);
                }
            }
            _ => {}
        }
    }
}

fn type_name(name: &str, scope: &Scope, references: &mut References) {
    if let Some((module, member)) = name.split_once('.') {
        if scope.local_module.as_deref() == Some(module) {
            references.insert(Key::Type(member.into()));
            return;
        }
        if !scope.modules.contains(module) {
            references.insert(Key::Module(module.into()));
        }
    } else if !scope.types.contains(name) {
        references.insert(Key::Type(name.into()));
    }
}

/// 型パラメータを scope に足した写しを返す。型の本体を歩くときの前準備。
fn extend_types(scope: &Scope, params: impl Iterator<Item = String>) -> Scope {
    let mut scope = scope.clone();
    scope.types.extend(params);
    scope
}

/// `module M : SIG = struct ... end` の本体を歩く scope。functor のパラメータは
/// module として見え、その contract が使われる。application は functor と実引数を
/// module として使う。
fn extend_signature_scope(
    scope: &Scope,
    form: &ImplementationForm,
    references: &mut References,
) -> Scope {
    let mut body_scope = scope.clone();
    match form {
        ImplementationForm::Ordinary => {}
        ImplementationForm::Functor(parameters) => {
            for parameter in parameters {
                body_scope.modules.insert(parameter.name.clone());
                references.insert(Key::Signature(parameter.contract.clone()));
            }
        }
        ImplementationForm::Application { functor, arguments } => {
            references.insert(Key::Module(functor.name.clone()));
            references.extend(
                arguments
                    .iter()
                    .map(|argument| Key::Module(argument.name.clone())),
            );
        }
    }
    body_scope
}

/// 参照を集める。`Collector` の `expression` から呼ばれる。
fn inferred_type(ty: &Ty, scope: &Scope, references: &mut References) {
    match ty {
        Ty::Adt(name, args) | Ty::Abstract(name, args) => {
            type_name(name, scope, references);
            for arg in args {
                inferred_type(arg, scope, references);
            }
        }
        Ty::Fn(args, result, _) => {
            for arg in args {
                inferred_type(arg, scope, references);
            }
            inferred_type(result, scope, references);
        }
        _ => {}
    }
}

fn type_expression(ty: &TypeExpr, scope: &Scope, references: &mut References) {
    type_expression_kind(&ty.kind, scope, references);
    collect_effect_row(&ty.effects, scope, references);
}

/// 型の本体は腕ごとの関数に置く。`match` は振り分けだけを持つ。
fn type_expression_kind(kind: &TypeKind, scope: &Scope, references: &mut References) {
    match kind {
        TypeKind::Name(name) => type_name(name, scope, references),
        TypeKind::Apply(name, args) => apply_type_arguments(name, args, scope, references),
        TypeKind::Group(args) => type_expression_children(args, scope, references),
        TypeKind::Record(fields) => record_field_types(fields, scope, references),
        TypeKind::Adt(variants) => adt_variant_types(variants, scope, references),
    }
}

fn apply_type_arguments(name: &str, args: &[TypeExpr], scope: &Scope, references: &mut References) {
    type_name(name, scope, references);
    type_expression_children(args, scope, references);
}

fn type_expression_children(args: &[TypeExpr], scope: &Scope, references: &mut References) {
    for arg in args {
        type_expression(arg, scope, references);
    }
}

fn record_field_types(fields: &[TypeField], scope: &Scope, references: &mut References) {
    for field in fields {
        type_expression(&field.ty, scope, references);
    }
}

fn adt_variant_types(variants: &[ConstructorDecl], scope: &Scope, references: &mut References) {
    for variant in variants {
        for arg in variant.payload.types() {
            type_expression(arg, scope, references);
        }
    }
}

fn collect_effect_row(
    effects: &Option<crate::parser::EffectRow>,
    scope: &Scope,
    references: &mut References,
) {
    if let Some(crate::parser::EffectRow::Row(effects)) = effects {
        for ty in effects {
            type_expression(ty, scope, references);
        }
    }
}
