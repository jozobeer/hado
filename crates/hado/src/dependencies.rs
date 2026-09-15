use crate::parser::{Ast, DefKind, Diagnostic, HandlerArm, HandlerDecl, Item, Node, NodeKind, Span, TypeExpr, TypeKind};
use crate::types::Ty;
use crate::parser::ImplementationForm;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key { Value(String), Type(String), Signature(String), Module(String) }

type References = BTreeSet<Key>;

fn declaration(item: &Item) -> Option<(Key, Span)> {
    Some(match item {
        Item::Def(d) => (Key::Value(d.name.clone()), d.span),
        Item::TypeDef(d) => (Key::Type(d.name.clone()), d.span),
        Item::ModuleSig(d) => (Key::Signature(d.name.clone()), d.span),
        Item::ModuleImpl(d) => (Key::Module(d.name.clone()), d.span),
        Item::SigDef(_) | Item::Effect(_) | Item::Handler(_) => return None,
    })
}

pub(crate) fn diagnostics(ast: &Ast, checked: &crate::Checked, prelude_items: usize, other_entry_roots: &[String], inspection: crate::Inspection, path: &str) -> Vec<Diagnostic> {
    let collector = Collector { checked: Some(checked), constructors: constructor_owners(&ast.items) };
    let graph = collector.graph(ast);
    let mut used = References::new();
    match inspection {
        crate::Inspection::Entry => {
            used.extend(transitive_references(&Key::Value("main".into()), &graph));
            for name in other_entry_roots {
                used.extend(transitive_references(&Key::Value(name.clone()), &graph));
            }
        }
        // A2: the public surface is the root. Outer `def`s are not roots, and
        // prelude items keep their `.skip(prelude_items)` exemption (§56).
        crate::Inspection::Library => {
            for item in &ast.items {
                let key = match item {
                    Item::ModuleImpl(module) => Key::Module(module.name.clone()),
                    Item::ModuleSig(contract) => Key::Signature(contract.name.clone()),
                    Item::TypeDef(def) => Key::Type(def.name.clone()),
                    _ => continue,
                };
                used.extend(transitive_references(&key, &graph));
            }
        }
    }
    let scope = Scope::default();
    let mut handler_references = References::new();
    for item in &ast.items {
        if let Item::Handler(handler) = item {
            collector.handler(handler, &scope, &mut handler_references);
        }
    }
    // A handler is part of the program, so what its arms reach is used just like
    // what `main` or a module reaches: close it transitively (§40).
    for key in handler_references {
        used.extend(transitive_references(&key, &graph));
    }
    let mut diagnostics: Vec<_> = ast.items.iter().skip(prelude_items).filter_map(|item| {
        let (key, span) = declaration(item)?;
        let referenced = match &key {
            Key::Signature(_) if inspection == crate::Inspection::Library => true,
            Key::Signature(name) => ast.items.iter().any(|item|
                matches!(item, Item::ModuleImpl(module) if &module.sig == name && !matches!(module.form, ImplementationForm::Functor(_)))),
            _ => used.contains(&key),
        };
        (!referenced).then(|| Diagnostic { path: path.into(), line: span.line, col: span.col,
            code: Some("E513".into()), message: "unused top-level declaration".into() })
    }).collect();
    for item in ast.items.iter().skip(prelude_items) {
        if let Item::ModuleImpl(module) = item {
            diagnostics.extend(local_data_diagnostics(ast, module, checked, path));
            if let ImplementationForm::Functor(parameters) = &module.form {
                let mut references = References::new();
                collector.items(&module.items, &Scope::default(), &mut references);
                for parameter in parameters {
                    if !references.contains(&Key::Module(parameter.name.clone())) {
                        diagnostics.push(Diagnostic { path: path.into(), line: parameter.name_span.line,
                            col: parameter.name_span.col, code: Some("E521".into()),
                            message: "unused functor parameter".into() });
                    }
                }
            }
        }
    }
    diagnostics
}

fn local_data_diagnostics(ast: &Ast, module: &crate::parser::ModuleImpl, checked: &crate::Checked, path: &str) -> Vec<Diagnostic> {
    let collector = Collector { checked: Some(checked), constructors: constructor_owners(&module.items) };
    let scope = Scope { local_module: Some(module.name.clone()), ..Scope::default() };
    let graph = collector.graph_items(&module.items, &scope);
    let mut used = References::new();
    for item in &ast.items {
        if let Item::ModuleSig(contract) = item {
            if contract.name != module.sig { continue; }
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
    module.items.iter().filter_map(|item| {
        let Item::TypeDef(def) = item else { return None; };
        if !matches!(def.ty.as_ref().map(|ty| &ty.kind), Some(TypeKind::Record(_) | TypeKind::Adt(_)))
            || used.contains(&Key::Type(def.name.clone())) { return None; }
        Some(Diagnostic { path: path.into(), line: def.span.line, col: def.span.col,
            code: Some("E513".into()), message: "unused implementation-local type".into() })
    }).collect()
}

pub(crate) fn inference_groups(ast: &Ast) -> Vec<Vec<&Item>> {
    let collector = Collector { checked: None, constructors: constructor_owners(&ast.items) };
    let mut graph = collector.graph(ast);
    let keys: Vec<_> = ast.items.iter().filter_map(|item| declaration(item).map(|(key, _)| key)).collect();
    let mut groups = Vec::new();
    while !graph.is_empty() {
        let closure: BTreeMap<_, _> = graph.keys().map(|key| (key.clone(), transitive_references(key, &graph))).collect();
        let first = keys.iter().filter(|key| graph.contains_key(*key)).find(|key| {
            graph.keys().all(|dependency| !closure[*key].contains(dependency) || closure[dependency].contains(*key))
        }).expect("a finite dependency graph has a sink component");
        let component: References = graph.keys().filter(|key| closure[first].contains(*key) && closure[*key].contains(first)).cloned().collect();
        let group = ast.items.iter().filter(|item| declaration(item).is_some_and(|(key, _)| component.contains(&key))).collect();
        for key in component { graph.remove(&key); }
        groups.push(group);
    }
    groups
}

fn transitive_references(start: &Key, graph: &BTreeMap<Key, References>) -> References {
    let mut visited = References::new();
    let mut pending = vec![start.clone()];
    while let Some(key) = pending.pop() {
        if visited.insert(key.clone()) {
            if let Some(next) = graph.get(&key) { pending.extend(next.iter().cloned()); }
        }
    }
    visited
}

fn constructor_owners(items: &[Item]) -> BTreeMap<String, Key> {
    let mut owners = BTreeMap::new();
    for item in items {
        if let Item::TypeDef(def) = item {
            if let Some(TypeExpr { kind: TypeKind::Adt(variants), .. }) = &def.ty {
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
                Item::Def(d) => { scope.values.insert(d.name.clone()); }
                Item::TypeDef(d) => { scope.types.insert(d.name.clone()); }
                Item::ModuleSig(d) => { scope.signatures.insert(d.name.clone()); }
                Item::ModuleImpl(d) => { scope.modules.insert(d.name.clone()); }
                Item::SigDef(_) | Item::Effect(_) | Item::Handler(_) => {}
            }
        }
        scope.constructors.extend(constructor_owners(items).into_keys());
        scope
    }
}

struct Collector<'a> {
    checked: Option<&'a crate::Checked>,
    constructors: BTreeMap<String, Key>,
}

impl Collector<'_> {
    fn graph(&self, ast: &Ast) -> BTreeMap<Key, References> {
        let mut graph = self.graph_items(&ast.items, &Scope::default());
        let contracts = crate::contract_types::Contracts::collect(&ast.items);
        for (key, references) in &mut graph {
            if matches!(key, Key::Signature(_)) {
                *references = references.iter().map(|reference| match reference {
                    Key::Module(name) => contracts.implementation(name).map_or_else(|| reference.clone(), |implementation| Key::Module(implementation.into())),
                    _ => reference.clone(),
                }).collect();
            }
        }
        graph
    }

    fn graph_items(&self, items: &[Item], scope: &Scope) -> BTreeMap<Key, References> {
        items.iter().filter_map(|item| {
            let (key, _) = declaration(item)?;
            let mut references = References::new();
            self.item(item, scope, &mut references);
            Some((key, references))
        }).collect()
    }

    fn item(&self, item: &Item, scope: &Scope, references: &mut References) {
        match item {
            Item::Def(def) => self.definition(&def.kind, scope, references),
            Item::TypeDef(def) => {
                let mut scope = scope.clone();
                scope.types.extend(def.params.iter().map(|p| p.name.clone()));
                if let Some(ty) = &def.ty { type_expression(ty, &scope, references); }
            }
            Item::SigDef(def) => {
                let mut scope = scope.clone();
                scope.types.extend(def.type_params.iter().map(|p| p.name.clone()));
                for param in &def.params { if let Some(ty) = &param.ty { type_expression(ty, &scope, references); } }
                if let Some(ty) = &def.ret { type_expression(ty, &scope, references); }
            }
            Item::ModuleSig(def) => self.items(&def.items, scope, references),
            Item::ModuleImpl(def) => {
                if !scope.signatures.contains(&def.sig) { references.insert(Key::Signature(def.sig.clone())); }
                let mut body_scope = scope.clone();
                match &def.form {
                    ImplementationForm::Ordinary => {}
                    ImplementationForm::Functor(parameters) => {
                        for parameter in parameters {
                            body_scope.modules.insert(parameter.name.clone());
                            references.insert(Key::Signature(parameter.contract.clone()));
                        }
                    }
                    ImplementationForm::Application { functor, arguments } => {
                        references.insert(Key::Module(functor.name.clone()));
                        references.extend(arguments.iter().map(|argument| Key::Module(argument.name.clone())));
                    }
                }
                self.items(&def.items, &body_scope, references);
            }
            Item::Effect(_) => {}
            Item::Handler(handler) => self.handler(handler, scope, references),
        }
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

    fn body(&self, body: &Node, scope: &Scope, bound: &BTreeSet<String>, references: &mut References) {
        let mut free = BTreeSet::new();
        crate::names::free_names(body, bound, &mut free);
        references.extend(free.iter().filter(|name| crate::names::is_value_reference(name)).cloned().map(Key::Value));
        self.expression(body, scope, references);
    }

    fn items(&self, items: &[Item], scope: &Scope, references: &mut References) {
        let nested = scope.nested(items);
        for item in items { self.item(item, &nested, references); }
    }

    fn definition(&self, def: &DefKind, scope: &Scope, references: &mut References) {
        let mut bound = scope.values.clone();
        let body = match def {
            DefKind::Value(body) => body,
            DefKind::Function { params, ret, body } => {
                for param in params {
                    bound.insert(param.name.clone());
                    if let Some(ty) = &param.ty { type_expression(ty, scope, references); }
                }
                if let Some(ty) = ret { type_expression(ty, scope, references); }
                body
            }
        };
        self.body(body, scope, &bound, references);
    }

    fn expression(&self, node: &Node, scope: &Scope, references: &mut References) {
        if let Some(ty) = self.checked.and_then(|checked| checked.composite_types.get(&(node.span.line, node.span.col))) {
            inferred_type(ty, scope, references);
        }
        match &node.kind {
            NodeKind::Name(name) if !scope.constructors.contains(name) => {
                if let Some(owner) = self.constructors.get(name) { references.insert(owner.clone()); }
            }
            NodeKind::NamedConstructor(name, _) if !scope.constructors.contains(&name.name) => {
                if let Some(owner) = self.constructors.get(&name.name) { references.insert(owner.clone()); }
            }
            NodeKind::Let { pat, .. } | NodeKind::LetQ { pat, .. } => self.pattern(pat, scope, references),
            NodeKind::PatternBranch { arms, .. } => {
                for (pattern, _) in arms { self.pattern(pattern, scope, references); }
            }
            NodeKind::Field(base, _) => {
                if let NodeKind::Name(name) = &base.kind {
                    if crate::names::is_upper_reference(name) {
                        if !scope.modules.contains(name) { references.insert(Key::Module(name.clone())); }
                        return;
                    }
                }
            }
            NodeKind::Fn { params, .. } => {
                for param in params { if let Some(ty) = &param.ty { type_expression(ty, scope, references); } }
            }
            _ => {}
        }
        for child in crate::check::children(node) { self.expression(child, scope, references); }
    }
    fn pattern(&self, pattern: &crate::parser::Pattern, scope: &Scope, references: &mut References) {
        use crate::parser::PatternKind;
        let name = match &pattern.kind {
            PatternKind::Name(name) | PatternKind::Constructor(name, _) => Some(name.as_str()),
            PatternKind::NamedConstructor(name, _) => Some(name.name.as_str()),
            _ => None,
        };
        if let Some(name) = name.filter(|name| !scope.constructors.contains(*name)) {
            if let Some(owner) = self.constructors.get(name) { references.insert(owner.clone()); }
        }
        match &pattern.kind {
            PatternKind::Constructor(_, args) => {
                for arg in args { self.pattern(arg, scope, references); }
            }
            PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => {
                for nested in fields.iter().filter_map(|(_, nested)| nested.as_ref()) { self.pattern(nested, scope, references); }
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
        if !scope.modules.contains(module) { references.insert(Key::Module(module.into())); }
    } else if !scope.types.contains(name) {
        references.insert(Key::Type(name.into()));
    }
}

fn inferred_type(ty: &Ty, scope: &Scope, references: &mut References) {
    match ty {
        Ty::Adt(name, args) | Ty::Abstract(name, args) => {
            type_name(name, scope, references);
            for arg in args { inferred_type(arg, scope, references); }
        }
        Ty::Fn(args, result, _) => {
            for arg in args { inferred_type(arg, scope, references); }
            inferred_type(result, scope, references);
        }
        _ => {}
    }
}

fn type_expression(ty: &TypeExpr, scope: &Scope, references: &mut References) {
    match &ty.kind {
        TypeKind::Name(name) => type_name(name, scope, references),
        TypeKind::Apply(name, args) => {
            type_name(name, scope, references);
            for arg in args { type_expression(arg, scope, references); }
        }
        TypeKind::Group(args) => { for arg in args { type_expression(arg, scope, references); } }
        TypeKind::Record(fields) => { for field in fields { type_expression(&field.ty, scope, references); } }
        TypeKind::Adt(variants) => {
            for variant in variants {
                for arg in variant.payload.types() { type_expression(arg, scope, references); }
            }
        }
    }
    if let Some(crate::parser::EffectRow::Row(effects)) = &ty.effects {
        for ty in effects { type_expression(ty, scope, references); }
    }
}
