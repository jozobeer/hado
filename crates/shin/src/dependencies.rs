use crate::parser::{Ast, DefKind, Diagnostic, Item, Node, NodeKind, Span, TypeExpr, TypeKind};
use crate::types::Ty;
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
        Item::SigDef(_) => return None,
    })
}

pub(crate) fn diagnostics(ast: &Ast, checked: &crate::Checked, prelude_items: usize, path: &str) -> Vec<Diagnostic> {
    let collector = Collector { checked: Some(checked), constructors: constructor_owners(&ast.items) };
    let graph = collector.graph(ast);
    let used = reachable(&graph);
    ast.items.iter().skip(prelude_items).filter_map(|item| {
        let (key, span) = declaration(item)?;
        (!used.contains(&key)).then(|| Diagnostic { path: path.into(), line: span.line, col: span.col,
            code: Some("E513".into()), message: "unused top-level declaration".into() })
    }).collect()
}

fn reachable(graph: &BTreeMap<Key, References>) -> References {
    let mut used = References::new();
    let mut pending = vec![Key::Value("main".into())];
    while let Some(key) = pending.pop() {
        if used.insert(key.clone()) {
            if let Some(references) = graph.get(&key) { pending.extend(references.iter().cloned()); }
        }
    }
    used
}

fn constructor_owners(items: &[Item]) -> BTreeMap<String, Key> {
    let mut owners = BTreeMap::new();
    for item in items {
        if let Item::TypeDef(def) = item {
            if let Some(TypeExpr { kind: TypeKind::Adt(variants), .. }) = &def.ty {
                for variant in variants {
                    if let TypeKind::Name(name) | TypeKind::Apply(name, _) = &variant.kind {
                        owners.insert(name.clone(), Key::Type(def.name.clone()));
                    }
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
                Item::SigDef(_) => {}
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
        ast.items.iter().filter_map(|item| {
        let (key, _) = declaration(item)?;
        let mut references = References::new();
        self.item(item, &Scope::default(), &mut references);
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
                if let Some(signature) = &def.sig {
                    if !scope.signatures.contains(signature) { references.insert(Key::Signature(signature.clone())); }
                }
                self.items(&def.items, scope, references);
            }
        }
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
        let mut free = BTreeSet::new();
        crate::names::free_names(body, &bound, &mut free);
        references.extend(free.iter().filter(|name| name.starts_with(char::is_lowercase)).cloned().map(Key::Value));
        self.expression(body, scope, references);
    }

    fn expression(&self, node: &Node, scope: &Scope, references: &mut References) {
        if let Some(ty) = self.checked.and_then(|checked| checked.composite_types.get(&(node.span.line, node.span.col))) {
            inferred_type(ty, scope, references);
        }
        match &node.kind {
            NodeKind::Name(name) if !scope.constructors.contains(name) => {
                if let Some(owner) = self.constructors.get(name) { references.insert(owner.clone()); }
            }
            NodeKind::Field(base, _) => {
                if let NodeKind::Name(name) = &base.kind {
                    if name.starts_with(char::is_uppercase) {
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
}

fn type_name(name: &str, scope: &Scope, references: &mut References) {
    if let Some((module, _)) = name.split_once('.') {
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
        Ty::Fn(args, result) => {
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
                if let TypeKind::Apply(_, args) = &variant.kind {
                    for arg in args { type_expression(arg, scope, references); }
                }
            }
        }
    }
}
