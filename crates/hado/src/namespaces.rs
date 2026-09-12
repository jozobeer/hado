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
    // §49: the prelude is placed at `[]`, but a library must still see it while
    // the user root `[]` stays invisible, so it keeps its own table.
    prelude: BTreeMap<Space, BTreeSet<String>>,
    // §55, §59: the root that the current item resolves paths from.
    root: Directory,
    sites: BTreeMap<(usize, usize), Vec<DeclaredName>>,
    renamed: BTreeMap<(usize, usize), String>,
    operations: BTreeMap<Directory, BTreeSet<String>>,
    path: &'a str,
    result: Resolution,
}

/// §59: the deepest library root that contains `directory`, or the user root
/// `[]` when the item sits outside every library.
fn innermost_library_root(directory: &[String], libraries: &BTreeSet<Directory>) -> Directory {
    libraries.iter().filter(|library| directory.starts_with(library.as_slice()))
        .max_by_key(|library| library.len()).cloned().unwrap_or_default()
}

fn qualify(directory: &[String], name: &str) -> String {
    if directory.is_empty() { name.into() } else { format!("{}::{name}", directory.join("::")) }
}

fn declared_directory(name: &str) -> Directory {
    match name.rsplit_once("::") {
        Some((prefix, _)) => prefix.split("::").map(String::from).collect(),
        None => Vec::new(),
    }
}

fn item_span(item: &Item) -> Span {
    match item {
        Item::Def(item) => item.span,
        Item::SigDef(item) => item.span,
        Item::TypeDef(item) => item.span,
        Item::ModuleSig(item) => item.span,
        Item::ModuleImpl(item) => item.span,
        Item::Effect(item) => item.span,
        Item::Handler(item) => item.span,
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
        Item::Effect(effect) => vec![(Space::Contract, &effect.name, effect.name_span)],
        Item::Handler(handler) => vec![(Space::Implementation, &handler.name, handler.name_span)],
        _ => Vec::new(),
    }
}

fn operation_key(directory: &[String], name: &str) -> String {
    if directory.is_empty() { name.into() } else { format!("{}${name}", directory.join("$")) }
}

fn file_value(name: &str) -> Option<&str> {
    name.split_once('$').and_then(|(prefix, rest)| prefix.strip_prefix("file").and_then(|index| {
        index.chars().all(|ch| ch.is_ascii_digit()).then_some(rest)
    }))
}

pub(crate) fn resolve(
    ast: &mut Ast, files: &[(usize, Directory)], directories: &BTreeSet<Directory>,
    libraries: &BTreeSet<Directory>, prelude_items: usize, path: &str,
) -> Resolution {
    let locations: Vec<_> = ast.items.iter().enumerate().map(|(index, item)| {
        if index < prelude_items { return Vec::new(); }
        files.iter().rev().find(|(offset, _)| *offset < item_span(item).line)
            .map(|(_, directory)| directory.clone()).unwrap_or_default()
    }).collect();
    let roots: Vec<_> = locations.iter().map(|directory| innermost_library_root(directory, libraries)).collect();
    let mut resolver = Resolver {
        symbols: BTreeMap::new(), directories,
        prelude: BTreeMap::new(), root: Vec::new(),
        sites: ast.name_sites.iter().map(|site| ((site.start.line, site.start.col), site.parts.clone())).collect(),
        renamed: BTreeMap::new(), operations: BTreeMap::new(), path,
        result: Resolution { diagnostics: Vec::new(), error_anchors: Vec::new() },
    };
    resolver.symbols.entry((Vec::new(), Space::Type)).or_default()
        .extend(crate::names::BUILTIN_TYPES.iter().map(|name| (*name).to_owned()));
    resolver.prelude.entry(Space::Type).or_default()
        .extend(crate::names::BUILTIN_TYPES.iter().map(|name| (*name).to_owned()));
    for (index, (item, directory)) in ast.items.iter().zip(&locations).enumerate() {
        for (space, name, _) in declarations(item) {
            resolver.symbols.entry((directory.clone(), space)).or_default().insert(name.into());
            if index < prelude_items { resolver.prelude.entry(space).or_default().insert(name.into()); }
        }
        if let Item::Effect(effect) = item {
            resolver.operations.entry(directory.clone()).or_default()
                .extend(effect.ops.iter().map(|op| op.name.clone()));
        }
    }
    for ((item, directory), root) in ast.items.iter_mut().zip(&locations).zip(&roots) {
        resolver.root = root.clone();
        // §59: a declaration in a library subdirectory conflicts with the
        // library root and the prelude, not with the user root.
        if directory != root {
            for (space, name, span) in declarations(item) {
                if resolver.has(&resolver.root, space, name) || resolver.has_prelude(space, name) {
                    resolver.error(span, "E205", "declaration conflicts with root name");
                }
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

    fn has_prelude(&self, space: Space, name: &str) -> bool {
        self.prelude.get(&space).is_some_and(|names| names.contains(name))
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
        // §55, §59: unqualified names reach the own directory, then the item's
        // root (the innermost library root, or the user root outside a library).
        // The prelude stays visible from everywhere (§49) while the user root
        // does not reach into a library.
        if !name.contains("::") {
            if self.has(directory, space, name) { *name = qualify(directory, name); }
            else if self.has(&self.root, space, name) { *name = qualify(&self.root, name); }
            else if !self.has_prelude(space, name) { self.unknown(span, span); }
            return;
        }
        // Absolute paths are read from the root: the innermost library root
        // inside a library, the user root outside one.
        let parts: Vec<_> = name.split("::").collect();
        let mut prefix = self.root.clone();
        for (index, part) in parts.iter().enumerate() {
            let known = if index + 1 == parts.len() { self.has(&prefix, space, part) }
                else { prefix.push((*part).into()); self.directories.contains(&prefix) };
            if !known {
                let anchor = self.sites.get(&(span.line, span.col)).and_then(|parts| parts.get(index)).map_or(span, |part| part.span);
                self.unknown(span, anchor);
                return;
            }
        }
        // The declaration table is indexed by the path from the user root, so a
        // path written from a library root becomes absolute for the rest of the
        // pipeline. Outside a library the root is `[]`, a no-op.
        *name = qualify(&self.root, name);
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
                    && (self.has(directory, Space::Type, &def.name)
                        || self.has(&self.root, Space::Type, &def.name)
                        || self.has_prelude(Space::Type, &def.name)) {
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
            Item::Effect(effect) => {
                let mut scope = local.clone();
                scope.types.extend(effect.type_params.iter().map(|param| param.name.clone()));
                for op in &mut effect.ops {
                    for ty in op.params.iter_mut().filter_map(|param| param.ty.as_mut()).chain(op.ret.iter_mut()) {
                        self.ty(ty, directory, &scope);
                    }
                    op.name = operation_key(directory, &op.name);
                }
                if outer { effect.name = qualify(directory, &effect.name); }
            }
            Item::Handler(handler) => {
                let mut scope = local.clone();
                scope.types.extend(handler.type_params.iter().map(|param| param.name.clone()));
                self.reference(&mut handler.effect, handler.effect_span, Space::Contract, directory, local);
                if declared_directory(&handler.effect) != directory {
                    self.unknown(handler.effect_span, handler.effect_span);
                }
                self.annotations(&mut handler.params, &mut None, directory, &scope);
                for arm in &mut handler.arms {
                    match arm {
                        HandlerArm::Operation { name, params, body, .. } => {
                            *name = operation_key(directory, name);
                            for ty in params.iter_mut().filter_map(|param| param.ty.as_mut()) { self.ty(ty, directory, &scope); }
                            self.node(body, directory, local);
                        }
                        HandlerArm::Return { param, body, .. } => {
                            if let Some(ty) = &mut param.ty { self.ty(ty, directory, &scope); }
                            self.node(body, directory, local);
                        }
                    }
                }
                if outer { handler.name = qualify(directory, &handler.name); }
            }
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
        if let Some(crate::parser::EffectRow::Row(effects)) = &mut ty.effects {
            for ty in effects { self.effect_label(ty, directory, local); }
        }
    }

    fn effect_label(&mut self, ty: &mut TypeExpr, directory: &[String], local: &Local) {
        match &mut ty.kind {
            TypeKind::Name(name) | TypeKind::Apply(name, _) => {
                self.reference(name, ty.name_span, Space::Contract, directory, local);
            }
            _ => {}
        }
        match &mut ty.kind {
            TypeKind::Apply(_, arguments) | TypeKind::Group(arguments) => {
                for ty in arguments { self.ty(ty, directory, local); }
            }
            _ => {}
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

    fn operation(&mut self, name: &mut String, directory: &[String]) {
        let bare = file_value(name).unwrap_or(name.as_str());
        if self.operations.get(directory).is_some_and(|names| names.contains(bare)) {
            *name = operation_key(directory, bare);
        }
    }

    fn node(&mut self, node: &mut Node, directory: &[String], local: &Local) {
        match &mut node.kind {
            NodeKind::Name(name) if crate::names::is_upper_reference(name) => self.reference(name, node.span, Space::Constructor, directory, local),
            NodeKind::Name(name) => self.operation(name, directory),
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
            NodeKind::With { handler, args, body } => {
                self.reference(&mut handler.name, handler.span, Space::Implementation, directory, local);
                for arg in args { self.node(arg, directory, local); }
                self.node(body, directory, local);
            }
            _ => {}
        }
    }
}
