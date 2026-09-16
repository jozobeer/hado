use crate::parser::{Ast, DefKind, Diagnostic, HandlerArm, ImplementationForm, Item, Node, NodeKind, Pattern, PatternKind, Span, TypeKind};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
struct Scope {
    values: BTreeSet<String>,
    constructors: BTreeSet<String>,
    named_fields: BTreeMap<String, BTreeSet<String>>,
    signatures: BTreeMap<String, BTreeSet<String>>,
    modules: BTreeMap<String, BTreeSet<String>>,
    incomplete_modules: BTreeSet<String>,
    functors: BTreeSet<String>,
}

impl Scope {
    fn nested(&self, items: &[Item]) -> Self {
        let mut scope = self.clone();
        let mut signatures: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut local_constructors = BTreeSet::new();
        for item in items {
            match item {
                Item::Def(def) => { scope.values.insert(def.name.clone()); }
                Item::Effect(effect) => {
                    for op in &effect.ops { scope.values.insert(op.name.clone()); }
                }
                Item::TypeDef(def) => {
                    if let Some(ty) = &def.ty {
                        if let TypeKind::Adt(variants) = &ty.kind {
                            for variant in variants {
                                let name = &variant.name.name;
                                scope.constructors.insert(name.clone());
                                scope.named_fields.remove(name);
                                if local_constructors.insert(name.clone()) {
                                    if let crate::parser::ConstructorPayload::Named(fields) = &variant.payload {
                                        scope.named_fields.insert(name.clone(), fields.iter().map(|field| field.name.name.clone()).collect());
                                    }
                                }
                            }
                        }
                    }
                }
                Item::ModuleSig(sig) => {
                    signatures.entry(sig.name.clone()).or_default().extend(sig.items.iter().filter_map(|item| match item {
                        Item::SigDef(def) => Some(def.name.clone()),
                        _ => None,
                    }));
                }
                _ => {}
            }
        }
        scope.signatures.extend(signatures);
        let mut modules: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for item in items {
            if let Item::ModuleImpl(module) = item {
                if matches!(&module.form, ImplementationForm::Functor(_)) {
                    scope.functors.insert(module.name.clone());
                } else {
                    let signature = scope.signatures.get(&module.sig);
                    if signature.is_none() { scope.incomplete_modules.insert(module.name.clone()); }
                    let members = signature.cloned().unwrap_or_default();
                    modules.entry(module.name.clone()).or_default().extend(members);
                }
            }
        }
        scope.modules.extend(modules);
        scope
    }
}

pub(crate) fn diagnostics(ast: &Ast, path: &str, trusted_start: Option<usize>) -> Vec<Diagnostic> {
    let mut resolver = Resolver { path, trusted_start, diagnostics: Vec::new() };
    let mut scope = Scope::default();
    scope.values.insert("compare".into());
    resolver.items(&ast.items, &scope);
    resolver.diagnostics
}

struct Resolver<'a> {
    path: &'a str,
    trusted_start: Option<usize>,
    diagnostics: Vec<Diagnostic>,
}

impl Resolver<'_> {
    fn report(&mut self, span: Span, code: &str, name: &str) {
        self.reported(span, code, name, None);
    }

    /// §56: a duplicate names the first declaration of the same name.
    fn report_related(&mut self, span: Span, code: &str, name: &str, first: Span) {
        self.reported(span, code, name, Some(first));
    }

    fn reported(&mut self, span: Span, code: &str, name: &str, first: Option<Span>) {
        let related = first.map(|first| crate::parser::Related { path: self.path.into(), line: first.line, col: first.col });
        self.diagnostics.push(Diagnostic { related, path: self.path.into(), line: span.line, col: span.col,
            code: Some(code.into()), message: format!("invalid name reference {name}") });
    }

    fn items(&mut self, items: &[Item], inherited: &Scope) {
        let scope = inherited.nested(items);
        for item in items {
            match item {
                Item::Def(def) => {
                    let mut local = scope.clone();
                    if !def.rec { local.values.remove(&def.name); }
                    let body = match &def.kind {
                        DefKind::Value(body) => body,
                        DefKind::Function { params, body, .. } => {
                            local.values.extend(params.iter().filter(|param| param.name != "_").map(|param| param.name.clone()));
                            body
                        }
                    };
                    self.expression(body, &local, Some(&def.name), false);
                }
                Item::ModuleImpl(module) => {
                    if !scope.signatures.contains_key(&module.sig) { self.report(module.sig_span, "E204", &module.sig); }
                    let local = self.implementation_scope(module, &scope);
                    self.items(&module.items, &local);
                }
                Item::Handler(handler) => {
                    for arm in &handler.arms {
                        let mut local = scope.clone();
                        match arm {
                            HandlerArm::Operation { params, body, .. } => {
                                local.values.extend(params.iter().filter(|param| param.name != "_").map(|param| param.name.clone()));
                                self.expression(body, &local, None, false);
                            }
                            HandlerArm::Return { param, body, .. } => {
                                if param.name != "_" { local.values.insert(param.name.clone()); }
                                self.expression(body, &local, None, false);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn implementation_scope(&mut self, module: &crate::parser::ModuleImpl, scope: &Scope) -> Scope {
        let mut local = scope.clone();
        match &module.form {
            ImplementationForm::Ordinary => {}
            ImplementationForm::Functor(params) => {
                for param in params {
                    if let Some(members) = scope.signatures.get(&param.contract) {
                        local.modules.insert(param.name.clone(), members.clone());
                        local.incomplete_modules.remove(&param.name);
                        local.functors.remove(&param.name);
                    } else {
                        self.report(param.contract_span, "E204", &param.contract);
                    }
                }
            }
            ImplementationForm::Application { functor, arguments } => {
                if !scope.functors.contains(&functor.name) {
                    self.report(functor.span, "E204", &functor.name);
                }
                for argument in arguments {
                    if !scope.modules.contains_key(&argument.name) {
                        self.report(argument.span, "E204", &argument.name);
                    }
                }
            }
        }
        local
    }

    fn expression(&mut self, node: &Node, scope: &Scope, current: Option<&str>, called: bool) {
        match &node.kind {
            NodeKind::Name(name) => {
                if name.starts_with("__") {
                    let trusted = self.trusted_start.is_some_and(|line| node.span.line >= line)
                        || std::path::Path::new(self.path).components().any(|part| part.as_os_str() == "stdlib");
                    let code = if !called { Some("E203") } else if !trusted { Some("E202") }
                        else if !crate::runtime_abi::RuntimeFunction::from_intrinsic(name).is_some_and(|function| function.is_polymorphic() || function.value_signature().is_some()) { Some("E204") }
                        else { None };
                    if let Some(code) = code { self.report(node.span, code, name); }
                } else if !scope.values.contains(name) && !scope.constructors.contains(name) {
                    self.report(node.span, if current == Some(name) { "E201" } else { "E204" }, name);
                }
            }
            NodeKind::NamedConstructor(name, fields) => {
                if !scope.constructors.contains(&name.name) { self.report(name.span, "E204", &name.name); }
                for (field, value) in fields {
                    self.named_field(&name.name, field, scope);
                    self.expression(value, scope, current, false);
                }
            }
            NodeKind::Field(base, field) => {
                if let NodeKind::Name(name) = &base.kind {
                    if scope.functors.contains(name) {
                        self.report(base.span, "E204", name);
                        return;
                    }
                    if let Some(members) = scope.modules.get(name) {
                        if !scope.incomplete_modules.contains(name) && !members.contains(&field.name) { self.report(field.span, "E204", &field.name); }
                        return;
                    }
                }
                self.expression(base, scope, current, false);
            }
            NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
                self.expression(callee, scope, current, true);
                for arg in args { self.expression(arg, scope, current, false); }
            }
            NodeKind::Pipe(value, target) => {
                self.expression(value, scope, current, false);
                self.expression(target, scope, current, true);
            }
            NodeKind::Let { pat, value, body } | NodeKind::LetQ { pat, value, body } => {
                self.expression(value, scope, current, false);
                self.pattern(pat, scope);
                let mut local = scope.clone();
                local.values.extend(crate::names::bound_names(pat));
                self.expression(body, &local, current, false);
            }
            NodeKind::Fn { params, body } => {
                let mut local = scope.clone();
                local.values.extend(params.iter().filter(|param| param.name != "_").map(|param| param.name.clone()));
                self.expression(body, &local, current, false);
            }
            NodeKind::PatternBranch { scrutinees, arms, fallback } => {
                for scrutinee in scrutinees { self.expression(scrutinee, scope, current, false); }
                for (pats, body) in arms {
                    for pat in pats { self.pattern(pat, scope); }
                    let mut local = scope.clone();
                    for pat in pats { local.values.extend(crate::names::bound_names(pat)); }
                    self.expression(body, &local, current, false);
                }
                if let Some(body) = fallback { self.expression(body, scope, current, false); }
            }
            _ => {
                for child in crate::check::children(node) { self.expression(child, scope, current, false); }
            }
        }
    }

    fn named_field(&mut self, constructor: &str, field: &crate::parser::DeclaredName, scope: &Scope) {
        if scope.named_fields.get(constructor).is_some_and(|fields| !fields.contains(&field.name)) {
            self.report(field.span, "E204", &field.name);
        }
    }

    fn pattern(&mut self, pattern: &Pattern, scope: &Scope) {
        match &pattern.kind {
            PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
                if !scope.constructors.contains(name) { self.report(pattern.span, "E204", name); }
            }
            PatternKind::Constructor(name, args) => {
                if !scope.constructors.contains(name) { self.report(pattern.span, "E204", name); }
                for arg in args { self.pattern(arg, scope); }
            }
            PatternKind::NamedConstructor(name, fields) => {
                if !scope.constructors.contains(&name.name) { self.report(name.span, "E204", &name.name); }
                let mut seen: BTreeMap<String, Span> = BTreeMap::new();
                for (field, nested) in fields {
                    self.named_field(&name.name, field, scope);
                    match seen.get(&field.name) {
                        Some(&first) => self.report_related(field.span, "E205", &field.name, first),
                        None => { seen.insert(field.name.clone(), field.span); }
                    }
                    if let Some(nested) = nested { self.pattern(nested, scope); }
                }
            }
            PatternKind::Record(fields) => {
                for (_, nested) in fields { if let Some(nested) = nested { self.pattern(nested, scope); } }
            }
            _ => {}
        }
    }
}
