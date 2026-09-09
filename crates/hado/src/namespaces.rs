use crate::parser::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Resolution {
    pub diagnostics: Vec<Diagnostic>,
    pub error_anchors: Vec<(Span, Span)>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Space { Type, Contract, Implementation, Constructor }

type Directory = Vec<String>;
type Symbols = BTreeMap<(Directory, Space), BTreeSet<String>>;

#[derive(Clone, Default)]
struct Local {
    types: BTreeSet<String>,
    constructors: BTreeSet<String>,
    implementations: BTreeSet<String>,
    contract_types: bool,
}

struct Resolver<'a> {
    symbols: Symbols,
    directories: &'a BTreeSet<Directory>,
    sites: BTreeMap<(usize, usize), Vec<DeclaredName>>,
    renamed: BTreeMap<(usize, usize), String>,
    path: &'a str,
    result: Resolution,
}

fn qualify(directory: &[String], name: &str) -> String {
    if directory.is_empty() { name.into() } else { format!("{}::{name}", directory.join("::")) }
}

fn item_span(item: &Item) -> Span {
    match item {
        Item::Def(item) => item.span,
        Item::SigDef(item) => item.span,
        Item::TypeDef(item) => item.span,
        Item::ModuleSig(item) => item.span,
        Item::ModuleImpl(item) => item.span,
    }
}

fn declarations(item: &Item) -> Vec<(Space, &str, Span)> {
    match item {
        Item::TypeDef(def) => {
            let mut names = vec![(Space::Type, def.name.as_str(), def.name_span)];
            if let Some(TypeExpr { kind: TypeKind::Adt(constructors), .. }) = &def.ty {
                names.extend(constructors.iter().map(|ctor| (Space::Constructor, ctor.name.name.as_str(), ctor.name.span)));
            }
            names
        }
        Item::ModuleSig(module) => vec![(Space::Contract, &module.name, module.name_span)],
        Item::ModuleImpl(module) => vec![(Space::Implementation, &module.name, module.name_span)],
        _ => Vec::new(),
    }
}

pub(crate) fn resolve(
    ast: &mut Ast, files: &[(usize, Directory)], directories: &BTreeSet<Directory>,
    prelude_items: usize, path: &str,
) -> Resolution {
    let locations: Vec<_> = ast.items.iter().enumerate().map(|(index, item)| {
        if index < prelude_items { return Vec::new(); }
        files.iter().rev().find(|(offset, _)| *offset < item_span(item).line)
            .map(|(_, directory)| directory.clone()).unwrap_or_default()
    }).collect();
    let mut resolver = Resolver {
        symbols: BTreeMap::new(), directories,
        sites: ast.name_sites.iter().map(|site| ((site.start.line, site.start.col), site.parts.clone())).collect(),
        renamed: BTreeMap::new(), path,
        result: Resolution { diagnostics: Vec::new(), error_anchors: Vec::new() },
    };
    resolver.symbols.entry((Vec::new(), Space::Type)).or_default()
        .extend(crate::names::BUILTIN_TYPES.iter().map(|name| (*name).to_owned()));
    for (item, directory) in ast.items.iter().zip(&locations) {
        for (space, name, _) in declarations(item) {
            resolver.symbols.entry((directory.clone(), space)).or_default().insert(name.into());
        }
    }
    for (item, directory) in ast.items.iter_mut().zip(&locations) {
        if !directory.is_empty() {
            for (space, name, span) in declarations(item) {
                if resolver.has(&[], space, name) { resolver.error(span, "E205", "declaration conflicts with root name"); }
            }
        }
        resolver.item(item, directory, &Local::default(), true);
    }
    for (prefix, _) in &mut ast.qualified_type_anchors {
        if let Some(name) = resolver.renamed.get(&(prefix.span.line, prefix.span.col)) {
            prefix.name = name.clone();
        }
    }
    resolver.result
}

impl Resolver<'_> {
    fn has(&self, directory: &[String], space: Space, name: &str) -> bool {
        self.symbols.get(&(directory.to_vec(), space)).is_some_and(|names| names.contains(name))
    }

    fn error(&mut self, span: Span, code: &str, message: &str) {
        self.result.diagnostics.push(Diagnostic { path: self.path.into(), line: span.line, col: span.col,
            code: Some(code.into()), message: message.into() });
    }

    fn reference(&mut self, name: &mut String, span: Span, space: Space, directory: &[String], local: &Local) {
        let local_names = match space {
            Space::Type => &local.types,
            Space::Constructor => &local.constructors,
            Space::Implementation => &local.implementations,
            Space::Contract => { self.resolve_global(name, span, space, directory); return; }
        };
        if !name.contains("::") && local_names.contains(name) { return; }
        self.resolve_global(name, span, space, directory);
    }

    fn resolve_global(&mut self, name: &mut String, span: Span, space: Space, directory: &[String]) {
        if !name.contains("::") {
            if self.has(directory, space, name) { *name = qualify(directory, name); }
            else if !self.has(&[], space, name) { self.unknown(span, span); }
            return;
        }
        let parts: Vec<_> = name.split("::").collect();
        let mut prefix = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let known = if index + 1 == parts.len() { self.has(&prefix, space, part) }
                else { prefix.push((*part).into()); self.directories.contains(&prefix) };
            if !known {
                let anchor = self.sites.get(&(span.line, span.col)).and_then(|parts| parts.get(index)).map_or(span, |part| part.span);
                self.unknown(span, anchor);
                return;
            }
        }
    }

    fn unknown(&mut self, start: Span, anchor: Span) {
        self.result.error_anchors.push((start, anchor));
        self.error(anchor, "E204", "unknown namespace reference");
    }

    fn nested(&mut self, items: &[Item], directory: &[String], inherited: &Local, check_outer: bool) -> Local {
        let mut local = inherited.clone();
        for item in items {
            if let Item::TypeDef(def) = item {
                if check_outer && def.ty.as_ref().is_some_and(|ty| matches!(ty.kind, TypeKind::Adt(_) | TypeKind::Record(_)))
                    && (self.has(directory, Space::Type, &def.name) || self.has(&[], Space::Type, &def.name)) {
                    self.error(def.name_span, "E205", "local data type conflicts with a visible outer type");
                }
                local.types.insert(def.name.clone());
                if let Some(TypeExpr { kind: TypeKind::Adt(constructors), .. }) = &def.ty {
                    local.constructors.extend(constructors.iter().map(|ctor| ctor.name.name.clone()));
                }
            }
        }
        local
    }

    fn item(&mut self, item: &mut Item, directory: &[String], local: &Local, outer: bool) {
        match item {
            Item::TypeDef(def) => {
                let mut scope = local.clone();
                scope.types.extend(def.params.iter().map(|param| param.name.clone()));
                if let Some(ty) = &mut def.ty {
                    self.ty(ty, directory, &scope);
                    if outer {
                        if let TypeKind::Adt(constructors) = &mut ty.kind {
                            for ctor in constructors { ctor.name.name = qualify(directory, &ctor.name.name); }
                        }
                    }
                }
                if outer { def.name = qualify(directory, &def.name); }
            }
            Item::ModuleSig(module) => {
                let mut scope = self.nested(&module.items, directory, local, false);
                scope.contract_types = true;
                for item in &mut module.items { self.item(item, directory, &scope, false); }
                if outer { module.name = qualify(directory, &module.name); }
            }
            Item::ModuleImpl(module) => {
                self.reference(&mut module.sig, module.sig_span, Space::Contract, directory, local);
                let mut scope = self.nested(&module.items, directory, local, true);
                scope.contract_types = false;
                match &mut module.form {
                    ImplementationForm::Ordinary => {}
                    ImplementationForm::Functor(params) => {
                        for param in params {
                            self.reference(&mut param.contract, param.contract_span, Space::Contract, directory, local);
                            scope.implementations.insert(param.name.clone());
                        }
                    }
                    ImplementationForm::Application { functor, arguments } => {
                        self.reference(&mut functor.name, functor.span, Space::Implementation, directory, local);
                        for argument in arguments { self.reference(&mut argument.name, argument.span, Space::Implementation, directory, local); }
                    }
                }
                for item in &mut module.items { self.item(item, directory, &scope, false); }
                if outer { module.name = qualify(directory, &module.name); }
            }
            Item::SigDef(def) => {
                let mut scope = local.clone();
                scope.types.extend(def.type_params.iter().map(|param| param.name.clone()));
                self.annotations(&mut def.params, &mut def.ret, directory, &scope);
            }
            Item::Def(def) => match &mut def.kind {
                DefKind::Value(body) => self.node(body, directory, local),
                DefKind::Function { params, ret, body } => {
                    self.annotations(params, ret, directory, local);
                    self.node(body, directory, local);
                }
            },
        }
    }

    fn annotations(&mut self, params: &mut [Param], ret: &mut Option<TypeExpr>, directory: &[String], local: &Local) {
        for ty in params.iter_mut().filter_map(|param| param.ty.as_mut()).chain(ret.iter_mut()) { self.ty(ty, directory, local); }
    }

    fn ty(&mut self, ty: &mut TypeExpr, directory: &[String], local: &Local) {
        match &mut ty.kind {
            TypeKind::Name(name) | TypeKind::Apply(name, _) => {
                if let Some((prefix, member)) = name.split_once('.') {
                    let mut prefix = prefix.to_owned();
                    let member = member.to_owned();
                    let space = if local.contract_types { Space::Contract } else { Space::Implementation };
                    self.reference(&mut prefix, ty.name_span, space, directory, local);
                    self.renamed.insert((ty.name_span.line, ty.name_span.col), prefix.clone());
                    *name = format!("{prefix}.{member}");
                } else { self.reference(name, ty.name_span, Space::Type, directory, local); }
            }
            _ => {}
        }
        match &mut ty.kind {
            TypeKind::Apply(_, arguments) | TypeKind::Group(arguments) => {
                for ty in arguments { self.ty(ty, directory, local); }
            }
            TypeKind::Record(fields) => { for field in fields { self.ty(&mut field.ty, directory, local); } }
            TypeKind::Adt(constructors) => {
                for constructor in constructors { for ty in constructor.payload.types_mut() { self.ty(ty, directory, local); } }
            }
            TypeKind::Name(_) => {}
        }
    }

    fn pattern(&mut self, pattern: &mut Pattern, directory: &[String], local: &Local) {
        match &mut pattern.kind {
            PatternKind::Name(name) if crate::names::is_upper_reference(name) => self.reference(name, pattern.span, Space::Constructor, directory, local),
            PatternKind::Constructor(name, patterns) => {
                self.reference(name, pattern.span, Space::Constructor, directory, local);
                for pattern in patterns { self.pattern(pattern, directory, local); }
            }
            PatternKind::NamedConstructor(name, fields) => {
                self.reference(&mut name.name, name.span, Space::Constructor, directory, local);
                for (_, pattern) in fields { if let Some(pattern) = pattern { self.pattern(pattern, directory, local); } }
            }
            PatternKind::Record(fields) => {
                for (_, pattern) in fields { if let Some(pattern) = pattern { self.pattern(pattern, directory, local); } }
            }
            PatternKind::Literal(node) => self.node(node, directory, local),
            _ => {}
        }
    }

    fn node(&mut self, node: &mut Node, directory: &[String], local: &Local) {
        match &mut node.kind {
            NodeKind::Name(name) if crate::names::is_upper_reference(name) => self.reference(name, node.span, Space::Constructor, directory, local),
            NodeKind::Field(base, _) => {
                if let NodeKind::Name(name) = &mut base.kind {
                    if crate::names::is_upper_reference(name) {
                        self.reference(name, base.span, Space::Implementation, directory, local);
                        return;
                    }
                }
                self.node(base, directory, local);
            }
            NodeKind::NamedConstructor(name, fields) => {
                self.reference(&mut name.name, name.span, Space::Constructor, directory, local);
                for (_, value) in fields { self.node(value, directory, local); }
            }
            NodeKind::Record(fields) => { for (_, value) in fields { self.node(value, directory, local); } }
            NodeKind::Neg(value) | NodeKind::Not(value) => self.node(value, directory, local),
            NodeKind::Bin(_, left, right) | NodeKind::Pipe(left, right) => { self.node(left, directory, local); self.node(right, directory, local); }
            NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
                self.node(callee, directory, local);
                for arg in args { self.node(arg, directory, local); }
            }
            NodeKind::List(values, tail) => {
                for value in values { self.node(value, directory, local); }
                if let Some(tail) = tail { self.node(tail, directory, local); }
            }
            NodeKind::Let { pat, value, body } | NodeKind::LetQ { pat, value, body } => {
                self.pattern(pat, directory, local); self.node(value, directory, local); self.node(body, directory, local);
            }
            NodeKind::DoQ { value, body } => { self.node(value, directory, local); self.node(body, directory, local); }
            NodeKind::Fn { params, body } => {
                for ty in params.iter_mut().filter_map(|param| param.ty.as_mut()) { self.ty(ty, directory, local); }
                self.node(body, directory, local);
            }
            NodeKind::BoolBranch { cond, then, els } => {
                self.node(cond, directory, local); self.node(then, directory, local); self.node(els, directory, local);
            }
            NodeKind::CondBranch { arms, fallback } => {
                for (condition, body) in arms { self.node(condition, directory, local); self.node(body, directory, local); }
                if let Some(body) = fallback { self.node(body, directory, local); }
            }
            NodeKind::PatternBranch { scrutinee, arms, fallback } => {
                self.node(scrutinee, directory, local);
                for (pattern, body) in arms { self.pattern(pattern, directory, local); self.node(body, directory, local); }
                if let Some(body) = fallback { self.node(body, directory, local); }
            }
            _ => {}
        }
    }
}
