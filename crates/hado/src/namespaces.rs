use crate::parser::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct Resolution {
    pub diagnostics: Vec<Diagnostic>,
    pub error_anchors: Vec<(Span, Span)>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Space {
    Type,
    Contract,
    Implementation,
    Constructor,
}

type Directory = Vec<String>;
/// §56: each name keeps the position of its first declaration. The prelude and
/// the builtin types have no position in the user's source, so they carry a
/// zero line and never become a related position.
type Symbols = BTreeMap<(Directory, Space), BTreeMap<String, Span>>;

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
    // §55: the prelude is placed at `[]`, but a library must still see it while
    // the user root `[]` stays invisible, so it keeps its own table.
    prelude: BTreeMap<Space, BTreeSet<String>>,
    prelude_operations: BTreeSet<String>,
    // §51, §52: the root that the current item resolves paths from.
    root: Directory,
    sites: BTreeMap<(usize, usize), Vec<DeclaredName>>,
    renamed: BTreeMap<(usize, usize), String>,
    operations: BTreeMap<Directory, BTreeSet<String>>,
    path: &'a str,
    result: Resolution,
}

/// §52: the deepest library root that contains `directory`, or the user root
/// `[]` when the item sits outside every library.
fn innermost_library_root(directory: &[String], libraries: &BTreeSet<Directory>) -> Directory {
    libraries
        .iter()
        .filter(|library| directory.starts_with(library.as_slice()))
        .max_by_key(|library| library.len())
        .cloned()
        .unwrap_or_default()
}

fn qualify(directory: &[String], name: &str) -> String {
    if directory.is_empty() {
        name.into()
    } else {
        format!("{}::{name}", directory.join("::"))
    }
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
            if let Some(TypeExpr {
                kind: TypeKind::Adt(constructors),
                ..
            }) = &def.ty
            {
                names.extend(
                    constructors
                        .iter()
                        .map(|ctor| (Space::Constructor, ctor.name.name.as_str(), ctor.name.span)),
                );
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
    if directory.is_empty() {
        name.into()
    } else {
        format!("{}${name}", directory.join("$"))
    }
}

fn file_value(name: &str) -> Option<&str> {
    name.split_once('$').and_then(|(prefix, rest)| {
        prefix
            .strip_prefix("file")
            .and_then(|index| index.chars().all(|ch| ch.is_ascii_digit()).then_some(rest))
    })
}

pub(crate) fn resolve(
    ast: &mut Ast,
    files: &[(usize, Directory)],
    directories: &BTreeSet<Directory>,
    libraries: &BTreeSet<Directory>,
    prelude_items: usize,
    path: &str,
) -> Resolution {
    let locations: Vec<_> = ast
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            if index < prelude_items {
                return Vec::new();
            }
            files
                .iter()
                .rev()
                .find(|(offset, _)| *offset < item_span(item).line)
                .map(|(_, directory)| directory.clone())
                .unwrap_or_default()
        })
        .collect();
    let roots: Vec<_> = locations
        .iter()
        .map(|directory| innermost_library_root(directory, libraries))
        .collect();
    let mut resolver = Resolver {
        symbols: BTreeMap::new(),
        directories,
        prelude: BTreeMap::new(),
        prelude_operations: BTreeSet::new(),
        root: Vec::new(),
        sites: ast
            .name_sites
            .iter()
            .map(|site| ((site.start.line, site.start.col), site.parts.clone()))
            .collect(),
        renamed: BTreeMap::new(),
        operations: BTreeMap::new(),
        path,
        result: Resolution {
            diagnostics: Vec::new(),
            error_anchors: Vec::new(),
        },
    };
    resolver.register_builtin_types();
    resolver.register_symbols(&ast.items, &locations, prelude_items);
    resolver.resolve_items(&mut ast.items, &locations, &roots, prelude_items);
    for (prefix, _) in &mut ast.qualified_type_anchors {
        if let Some(name) = resolver.renamed.get(&(prefix.span.line, prefix.span.col)) {
            prefix.name = name.clone();
        }
    }
    resolver.result
}

impl Resolver<'_> {
    fn register_builtin_types(&mut self) {
        self.symbols
            .entry((Vec::new(), Space::Type))
            .or_default()
            .extend(
                crate::names::BUILTIN_TYPES
                    .iter()
                    .map(|name| ((*name).to_owned(), Span::point(0, 0))),
            );
        self.prelude.entry(Space::Type).or_default().extend(
            crate::names::BUILTIN_TYPES
                .iter()
                .map(|name| (*name).to_owned()),
        );
    }

    fn register_symbols(&mut self, items: &[Item], locations: &[Directory], prelude_items: usize) {
        for (index, (item, directory)) in items.iter().zip(locations).enumerate() {
            let in_prelude = index < prelude_items;
            self.register_item_declarations(item, directory, in_prelude);
            self.register_item_effects(item, directory, in_prelude);
        }
    }

    fn register_item_declarations(&mut self, item: &Item, directory: &[String], in_prelude: bool) {
        for (space, name, span) in declarations(item) {
            // §56: only the user's declarations carry a position; the prelude is
            // synthesized outside the user's source.
            let span = if in_prelude { Span::point(0, 0) } else { span };
            self.symbols
                .entry((directory.to_vec(), space))
                .or_default()
                .entry(name.into())
                .or_insert(span);
            if in_prelude {
                self.prelude.entry(space).or_default().insert(name.into());
            }
        }
    }

    fn register_item_effects(&mut self, item: &Item, directory: &[String], in_prelude: bool) {
        if let Item::Effect(effect) = item {
            self.operations
                .entry(directory.to_vec())
                .or_default()
                .extend(effect.ops.iter().map(|op| op.name.clone()));
            if in_prelude {
                self.prelude_operations
                    .extend(effect.ops.iter().map(|op| op.name.clone()));
            }
        }
    }

    fn resolve_items(
        &mut self,
        items: &mut [Item],
        locations: &[Directory],
        roots: &[Directory],
        prelude_items: usize,
    ) {
        for (index, ((item, directory), root)) in
            items.iter_mut().zip(locations).zip(roots).enumerate()
        {
            self.root = root.clone();
            if index >= prelude_items {
                self.check_declaration_conflicts(item, directory);
            }
            self.item(item, directory, &Local::default(), true);
        }
    }

    /// §51, §52: a declaration conflicts with the names visible unqualified from
    /// its directory. The prelude is visible even at a library root, so its names
    /// are checked everywhere. The root's own names are compared only from a
    /// subdirectory: at the root the declaration is the root.
    fn check_declaration_conflicts(&mut self, item: &Item, directory: &[String]) {
        self.check_prelude_operation_declarations(item, directory);
        for (space, name, span) in declarations(item) {
            self.check_declaration_conflict(directory, space, name, span);
        }
    }

    fn check_declaration_conflict(
        &mut self,
        directory: &[String],
        space: Space,
        name: &str,
        span: Span,
    ) {
        let conflicts_with_root = directory != self.root && self.has(&self.root, space, name);
        if !conflicts_with_root && !self.has_prelude(space, name) {
            return;
        }
        // §56: the root's declaration is the related position. A name that only
        // the prelude has (a builtin type) has none.
        let first = if conflicts_with_root {
            self.site(&self.root, space, name)
        } else {
            None
        };
        self.error_at(span, "E205", "declaration conflicts with root name", first);
    }

    fn check_prelude_operation_declarations(&mut self, item: &Item, directory: &[String]) {
        match item {
            Item::Def(def) => {
                let bare = file_value(&def.name).unwrap_or(&def.name);
                if self.prelude_operations.contains(bare) {
                    self.error(
                        def.name_span,
                        "E205",
                        "declaration conflicts with prelude operation",
                    );
                }
            }
            Item::ModuleImpl(module) => {
                for item in &module.items {
                    self.check_prelude_operation_declarations(item, directory);
                }
            }
            Item::Effect(effect) if !directory.is_empty() => {
                for op in &effect.ops {
                    if self.prelude_operations.contains(&op.name) {
                        self.error(
                            op.name_span,
                            "E205",
                            "declaration conflicts with prelude operation",
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn has(&self, directory: &[String], space: Space, name: &str) -> bool {
        self.symbols
            .get(&(directory.to_vec(), space))
            .is_some_and(|names| names.contains_key(name))
    }

    /// §56: the first declaration of `name`, or `None` when it has no position
    /// in the user's source (the builtin types and the prelude).
    fn site(&self, directory: &[String], space: Space, name: &str) -> Option<Span> {
        self.symbols
            .get(&(directory.to_vec(), space))
            .and_then(|names| names.get(name))
            .copied()
            .filter(|span| span.line != 0)
    }

    fn has_prelude(&self, space: Space, name: &str) -> bool {
        self.prelude
            .get(&space)
            .is_some_and(|names| names.contains(name))
    }

    fn error(&mut self, span: Span, code: &str, message: &str) {
        self.error_at(span, code, message, None);
    }

    /// §56: `first` is the first declaration of the name the report is about.
    fn error_at(&mut self, span: Span, code: &str, message: &str, first: Option<Span>) {
        let related = first.filter(|first| first.line != 0).map(|first| {
            Box::new(crate::parser::Related {
                path: self.path.into(),
                line: first.line,
                col: first.col,
            })
        });
        self.result.diagnostics.push(Diagnostic {
            suggestion: None,
            related,
            path: self.path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: Some(code.into()),
            message: message.into(),
        });
    }

    fn reference(
        &mut self,
        name: &mut String,
        span: Span,
        space: Space,
        directory: &[String],
        local: &Local,
    ) {
        let local_names = match space {
            Space::Type => &local.types,
            Space::Constructor => &local.constructors,
            Space::Implementation => &local.implementations,
            Space::Contract => {
                self.resolve_global(name, span, space, directory);
                return;
            }
        };
        if !name.contains("::") && local_names.contains(name) {
            return;
        }
        self.resolve_global(name, span, space, directory);
    }

    fn resolve_global(
        &mut self,
        name: &mut String,
        span: Span,
        space: Space,
        directory: &[String],
    ) {
        // §51, §52: unqualified names reach the own directory, then the item's
        // root (the innermost library root, or the user root outside a library).
        // The prelude stays visible from everywhere (§55) while the user root
        // does not reach into a library.
        if !name.contains("::") {
            if self.has(directory, space, name) {
                *name = qualify(directory, name);
            } else if self.has(&self.root, space, name) {
                *name = qualify(&self.root, name);
            } else if !self.has_prelude(space, name) {
                self.unknown(span, span);
            }
            return;
        }
        // Absolute paths are read from the root: the innermost library root
        // inside a library, the user root outside one.
        let parts: Vec<_> = name.split("::").collect();
        let mut prefix = self.root.clone();
        for (index, part) in parts.iter().enumerate() {
            let known = if index + 1 == parts.len() {
                self.has(&prefix, space, part)
            } else {
                prefix.push((*part).into());
                self.directories.contains(&prefix)
            };
            if !known {
                let anchor = self
                    .sites
                    .get(&(span.line, span.col))
                    .and_then(|parts| parts.get(index))
                    .map_or(span, |part| part.span);
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

    fn nested(
        &mut self,
        items: &[Item],
        directory: &[String],
        inherited: &Local,
        check_outer: bool,
    ) -> Local {
        let mut local = inherited.clone();
        for item in items {
            if let Item::TypeDef(def) = item {
                if check_outer
                    && def
                        .ty
                        .as_ref()
                        .is_some_and(|ty| matches!(ty.kind, TypeKind::Adt(_) | TypeKind::Record(_)))
                    && (self.has(directory, Space::Type, &def.name)
                        || self.has(&self.root, Space::Type, &def.name)
                        || self.has_prelude(Space::Type, &def.name))
                {
                    let first = self
                        .site(directory, Space::Type, &def.name)
                        .or_else(|| self.site(&self.root, Space::Type, &def.name));
                    self.error_at(
                        def.name_span,
                        "E205",
                        "local data type conflicts with a visible outer type",
                        first,
                    );
                }
                local.types.insert(def.name.clone());
                if let Some(TypeExpr {
                    kind: TypeKind::Adt(constructors),
                    ..
                }) = &def.ty
                {
                    local
                        .constructors
                        .extend(constructors.iter().map(|ctor| ctor.name.name.clone()));
                }
            }
        }
        local
    }

    fn item(&mut self, item: &mut Item, directory: &[String], local: &Local, outer: bool) {
        match item {
            Item::TypeDef(def) => self.item_type_def(def, directory, local, outer),
            Item::ModuleSig(module) => self.item_module_sig(module, directory, local, outer),
            Item::ModuleImpl(module) => self.item_module_impl(module, directory, local, outer),
            Item::SigDef(def) => self.item_sig_def(def, directory, local),
            Item::Def(def) => self.item_def(def, directory, local),
            Item::Effect(effect) => self.item_effect(effect, directory, local, outer),
            Item::Handler(handler) => self.item_handler(handler, directory, local, outer),
        }
    }

    fn item_type_def(
        &mut self,
        def: &mut TypeDef,
        directory: &[String],
        local: &Local,
        outer: bool,
    ) {
        let mut scope = local.clone();
        scope
            .types
            .extend(def.params.iter().map(|param| param.name.clone()));
        if let Some(ty) = &mut def.ty {
            self.ty(ty, directory, &scope);
            if outer {
                if let TypeKind::Adt(constructors) = &mut ty.kind {
                    for ctor in constructors {
                        ctor.name.name = qualify(directory, &ctor.name.name);
                    }
                }
            }
        }
        if outer {
            def.name = qualify(directory, &def.name);
        }
    }

    fn item_module_sig(
        &mut self,
        module: &mut ModuleSig,
        directory: &[String],
        local: &Local,
        outer: bool,
    ) {
        let mut scope = self.nested(&module.items, directory, local, false);
        scope.contract_types = true;
        for item in &mut module.items {
            self.item(item, directory, &scope, false);
        }
        if outer {
            module.name = qualify(directory, &module.name);
        }
    }

    fn item_module_impl(
        &mut self,
        module: &mut ModuleImpl,
        directory: &[String],
        local: &Local,
        outer: bool,
    ) {
        self.reference(
            &mut module.sig,
            module.sig_span,
            Space::Contract,
            directory,
            local,
        );
        let mut scope = self.nested(&module.items, directory, local, true);
        scope.contract_types = false;
        match &mut module.form {
            ImplementationForm::Ordinary => {}
            ImplementationForm::Functor(params) => {
                for param in params {
                    self.reference(
                        &mut param.contract,
                        param.contract_span,
                        Space::Contract,
                        directory,
                        local,
                    );
                    scope.implementations.insert(param.name.clone());
                }
            }
            ImplementationForm::Application { functor, arguments } => {
                self.reference(
                    &mut functor.name,
                    functor.span,
                    Space::Implementation,
                    directory,
                    local,
                );
                for argument in arguments {
                    self.reference(
                        &mut argument.name,
                        argument.span,
                        Space::Implementation,
                        directory,
                        local,
                    );
                }
            }
        }
        for item in &mut module.items {
            self.item(item, directory, &scope, false);
        }
        if outer {
            module.name = qualify(directory, &module.name);
        }
    }

    fn item_sig_def(&mut self, def: &mut SigDef, directory: &[String], local: &Local) {
        let mut scope = local.clone();
        scope
            .types
            .extend(def.type_params.iter().map(|param| param.name.clone()));
        self.annotations(&mut def.params, &mut def.ret, directory, &scope);
    }

    fn item_def(&mut self, def: &mut Def, directory: &[String], local: &Local) {
        match &mut def.kind {
            DefKind::Value(body) => self.node(body, directory, local),
            DefKind::Function { params, ret, body } => {
                self.annotations(params, ret, directory, local);
                self.node(body, directory, local);
            }
        }
    }

    fn item_effect(
        &mut self,
        effect: &mut EffectDecl,
        directory: &[String],
        local: &Local,
        outer: bool,
    ) {
        let mut scope = local.clone();
        scope
            .types
            .extend(effect.type_params.iter().map(|param| param.name.clone()));
        for op in &mut effect.ops {
            for ty in op
                .params
                .iter_mut()
                .filter_map(|param| param.ty.as_mut())
                .chain(op.ret.iter_mut())
            {
                self.ty(ty, directory, &scope);
            }
            op.name = operation_key(directory, &op.name);
        }
        if outer {
            effect.name = qualify(directory, &effect.name);
        }
    }

    fn item_handler(
        &mut self,
        handler: &mut HandlerDecl,
        directory: &[String],
        local: &Local,
        outer: bool,
    ) {
        let mut scope = local.clone();
        scope
            .types
            .extend(handler.type_params.iter().map(|param| param.name.clone()));
        self.reference(
            &mut handler.effect,
            handler.effect_span,
            Space::Contract,
            directory,
            local,
        );
        let prelude_effect = self.has_prelude(Space::Contract, &handler.effect);
        if !prelude_effect && declared_directory(&handler.effect) != directory {
            self.unknown(handler.effect_span, handler.effect_span);
        }
        self.annotations(&mut handler.params, &mut None, directory, &scope);
        for arm in &mut handler.arms {
            self.bind_handler_arm(arm, directory, local, &scope, prelude_effect);
        }
        if outer {
            handler.name = qualify(directory, &handler.name);
        }
    }

    fn bind_handler_arm(
        &mut self,
        arm: &mut HandlerArm,
        directory: &[String],
        local: &Local,
        scope: &Local,
        prelude_effect: bool,
    ) {
        match arm {
            HandlerArm::Operation {
                name, params, body, ..
            } => {
                if !prelude_effect {
                    *name = operation_key(directory, name);
                }
                for ty in params.iter_mut().filter_map(|param| param.ty.as_mut()) {
                    self.ty(ty, directory, scope);
                }
                self.node(body, directory, local);
            }
            HandlerArm::Return { param, body, .. } => {
                if let Some(ty) = &mut param.ty {
                    self.ty(ty, directory, scope);
                }
                self.node(body, directory, local);
            }
        }
    }

    fn annotations(
        &mut self,
        params: &mut [Param],
        ret: &mut Option<TypeExpr>,
        directory: &[String],
        local: &Local,
    ) {
        for ty in params
            .iter_mut()
            .filter_map(|param| param.ty.as_mut())
            .chain(ret.iter_mut())
        {
            self.ty(ty, directory, local);
        }
    }

    fn ty(&mut self, ty: &mut TypeExpr, directory: &[String], local: &Local) {
        self.resolve_type_name(ty, directory, local);
        self.visit_type_children(ty, directory, local);
        if let Some(crate::parser::EffectRow::Row(effects)) = &mut ty.effects {
            for ty in effects {
                self.effect_label(ty, directory, local);
            }
        }
    }

    fn resolve_type_name(&mut self, ty: &mut TypeExpr, directory: &[String], local: &Local) {
        match &mut ty.kind {
            TypeKind::Name(name) | TypeKind::Apply(name, _) => {
                if let Some((prefix, member)) = name.split_once('.') {
                    let mut prefix = prefix.to_owned();
                    let member = member.to_owned();
                    let space = if local.contract_types {
                        Space::Contract
                    } else {
                        Space::Implementation
                    };
                    self.reference(&mut prefix, ty.name_span, space, directory, local);
                    self.renamed
                        .insert((ty.name_span.line, ty.name_span.col), prefix.clone());
                    *name = format!("{prefix}.{member}");
                } else {
                    self.reference(name, ty.name_span, Space::Type, directory, local);
                }
            }
            _ => {}
        }
    }

    fn visit_type_children(&mut self, ty: &mut TypeExpr, directory: &[String], local: &Local) {
        match &mut ty.kind {
            TypeKind::Apply(_, arguments) | TypeKind::Group(arguments) => {
                for ty in arguments {
                    self.ty(ty, directory, local);
                }
            }
            TypeKind::Record(fields) => {
                for field in fields {
                    self.ty(&mut field.ty, directory, local);
                }
            }
            TypeKind::Adt(constructors) => {
                for constructor in constructors {
                    for ty in constructor.payload.types_mut() {
                        self.ty(ty, directory, local);
                    }
                }
            }
            TypeKind::Name(_) => {}
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
                for ty in arguments {
                    self.ty(ty, directory, local);
                }
            }
            _ => {}
        }
    }

    fn pattern(&mut self, pattern: &mut Pattern, directory: &[String], local: &Local) {
        match &mut pattern.kind {
            PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
                self.reference(name, pattern.span, Space::Constructor, directory, local)
            }
            PatternKind::Constructor(name, patterns) => {
                self.reference(name, pattern.span, Space::Constructor, directory, local);
                for pattern in patterns {
                    self.pattern(pattern, directory, local);
                }
            }
            PatternKind::NamedConstructor(name, fields) => {
                self.reference(
                    &mut name.name,
                    name.span,
                    Space::Constructor,
                    directory,
                    local,
                );
                self.bind_field_patterns(fields, directory, local);
            }
            PatternKind::Record(fields) => self.bind_field_patterns(fields, directory, local),
            PatternKind::Literal(node) => self.node(node, directory, local),
            _ => {}
        }
    }

    fn bind_field_patterns(
        &mut self,
        fields: &mut [(DeclaredName, Option<Pattern>)],
        directory: &[String],
        local: &Local,
    ) {
        for (_, pattern) in fields {
            if let Some(pattern) = pattern {
                self.pattern(pattern, directory, local);
            }
        }
    }

    fn operation(&mut self, name: &mut String, directory: &[String]) {
        let bare = file_value(name).unwrap_or(name.as_str());
        if self.prelude_operations.contains(bare) {
            *name = bare.to_owned();
        } else if self
            .operations
            .get(directory)
            .is_some_and(|names| names.contains(bare))
        {
            *name = operation_key(directory, bare);
        }
    }

    fn node(&mut self, node: &mut Node, directory: &[String], local: &Local) {
        match &mut node.kind {
            NodeKind::Name(name) if crate::names::is_upper_reference(name) => {
                self.reference(name, node.span, Space::Constructor, directory, local)
            }
            NodeKind::Name(name) => self.operation(name, directory),
            NodeKind::Field(base, _) => self.node_field(base, directory, local),
            NodeKind::NamedConstructor(name, fields) => {
                self.reference(
                    &mut name.name,
                    name.span,
                    Space::Constructor,
                    directory,
                    local,
                );
                self.node_fields(fields, directory, local);
            }
            NodeKind::Record(fields) => self.node_fields(fields, directory, local),
            NodeKind::Neg(value) | NodeKind::Not(value) => self.node(value, directory, local),
            NodeKind::Bin(_, left, right) | NodeKind::Pipe(left, right) => {
                self.node(left, directory, local);
                self.node(right, directory, local);
            }
            NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
                self.node(callee, directory, local);
                self.node_arguments(args, directory, local);
            }
            NodeKind::List(values, tail) => self.node_list(values, tail, directory, local),
            NodeKind::Let { pat, value, body } | NodeKind::LetQ { pat, value, body } => {
                self.pattern(pat, directory, local);
                self.node(value, directory, local);
                self.node(body, directory, local);
            }
            NodeKind::DoQ { value, body } => {
                self.node(value, directory, local);
                self.node(body, directory, local);
            }
            NodeKind::Fn { params, body } => self.node_fn(params, body, directory, local),
            NodeKind::BoolBranch { cond, then, els } => {
                self.node(cond, directory, local);
                self.node(then, directory, local);
                self.node(els, directory, local);
            }
            NodeKind::CondBranch { arms, fallback } => {
                self.node_cond_branch(arms, fallback, directory, local);
            }
            NodeKind::PatternBranch {
                scrutinees,
                arms,
                fallback,
            } => self.node_pattern_branch(scrutinees, arms, fallback, directory, local),
            NodeKind::With {
                handler,
                args,
                body,
            } => self.node_with(handler, args, body, directory, local),
            _ => {}
        }
    }

    fn node_field(&mut self, base: &mut Node, directory: &[String], local: &Local) {
        if let NodeKind::Name(name) = &mut base.kind {
            if crate::names::is_upper_reference(name) {
                self.reference(name, base.span, Space::Implementation, directory, local);
                return;
            }
        }
        self.node(base, directory, local);
    }

    fn node_fields(
        &mut self,
        fields: &mut [(DeclaredName, Node)],
        directory: &[String],
        local: &Local,
    ) {
        for (_, value) in fields {
            self.node(value, directory, local);
        }
    }

    fn node_arguments(&mut self, args: &mut [Node], directory: &[String], local: &Local) {
        for arg in args {
            self.node(arg, directory, local);
        }
    }

    fn node_list(
        &mut self,
        values: &mut [Node],
        tail: &mut Option<Box<Node>>,
        directory: &[String],
        local: &Local,
    ) {
        for value in values {
            self.node(value, directory, local);
        }
        if let Some(tail) = tail {
            self.node(tail, directory, local);
        }
    }

    fn node_fn(
        &mut self,
        params: &mut [Param],
        body: &mut Node,
        directory: &[String],
        local: &Local,
    ) {
        for ty in params.iter_mut().filter_map(|param| param.ty.as_mut()) {
            self.ty(ty, directory, local);
        }
        self.node(body, directory, local);
    }

    fn node_cond_branch(
        &mut self,
        arms: &mut [(Node, Node)],
        fallback: &mut Option<Box<Node>>,
        directory: &[String],
        local: &Local,
    ) {
        for (condition, body) in arms {
            self.node(condition, directory, local);
            self.node(body, directory, local);
        }
        if let Some(body) = fallback {
            self.node(body, directory, local);
        }
    }

    fn node_pattern_branch(
        &mut self,
        scrutinees: &mut [Node],
        arms: &mut [(Vec<Pattern>, Node)],
        fallback: &mut Option<Box<Node>>,
        directory: &[String],
        local: &Local,
    ) {
        for scrutinee in scrutinees {
            self.node(scrutinee, directory, local);
        }
        for (patterns, body) in arms {
            for pattern in patterns {
                self.pattern(pattern, directory, local);
            }
            self.node(body, directory, local);
        }
        if let Some(body) = fallback {
            self.node(body, directory, local);
        }
    }

    fn node_with(
        &mut self,
        handler: &mut DeclaredName,
        args: &mut [Node],
        body: &mut Node,
        directory: &[String],
        local: &Local,
    ) {
        self.reference(
            &mut handler.name,
            handler.span,
            Space::Implementation,
            directory,
            local,
        );
        for arg in args {
            self.node(arg, directory, local);
        }
        self.node(body, directory, local);
    }
}
