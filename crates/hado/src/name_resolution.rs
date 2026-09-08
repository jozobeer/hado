use crate::parser::{Ast, DefKind, Diagnostic, Item, Node, NodeKind, Pattern, PatternKind, Span, TypeKind};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Default)]
struct Scope {
    values: BTreeSet<String>,
    constructors: BTreeSet<String>,
    signatures: BTreeMap<String, BTreeSet<String>>,
    modules: BTreeMap<String, BTreeSet<String>>,
}

impl Scope {
    fn nested(&self, items: &[Item]) -> Self {
        let mut scope = self.clone();
        let mut signatures: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for item in items {
            match item {
                Item::Def(def) => { scope.values.insert(def.name.clone()); }
                Item::TypeDef(def) => {
                    if let Some(ty) = &def.ty {
                        if let TypeKind::Adt(variants) = &ty.kind {
                            for variant in variants {
                                if let TypeKind::Name(name) | TypeKind::Apply(name, _) = &variant.kind {
                                    scope.constructors.insert(name.clone());
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
                let members = module.sig.as_ref().and_then(|name| scope.signatures.get(name)).cloned().unwrap_or_default();
                modules.entry(module.name.clone()).or_default().extend(members);
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
        self.diagnostics.push(Diagnostic { path: self.path.into(), line: span.line, col: span.col,
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
                            local.values.extend(params.iter().map(|param| param.name.clone()));
                            body
                        }
                    };
                    self.expression(body, &local, Some(&def.name), false);
                }
                Item::ModuleImpl(module) => {
                    if let Some(name) = &module.sig {
                        if !scope.signatures.contains_key(name) { self.report(module.sig_span, "E204", name); }
                    }
                    self.items(&module.items, &scope);
                }
                _ => {}
            }
        }
    }

    fn expression(&mut self, node: &Node, scope: &Scope, current: Option<&str>, called: bool) {
        match &node.kind {
            NodeKind::Name(name) => {
                if name.starts_with("__") {
                    let trusted = self.trusted_start.is_some_and(|line| node.span.line >= line)
                        || std::path::Path::new(self.path).components().any(|part| part.as_os_str() == "stdlib");
                    let code = if !called { Some("E203") } else if !trusted { Some("E202") }
                        else if crate::runtime_abi::RuntimeFunction::from_intrinsic(name).and_then(|function| function.value_signature()).is_none() { Some("E204") }
                        else { None };
                    if let Some(code) = code { self.report(node.span, code, name); }
                } else if !scope.values.contains(name) && !scope.constructors.contains(name) {
                    self.report(node.span, if current == Some(name) { "E201" } else { "E204" }, name);
                }
            }
            NodeKind::Field(base, field) => {
                if let NodeKind::Name(name) = &base.kind {
                    if let Some(members) = scope.modules.get(name) {
                        if !members.contains(&field.name) { self.report(field.span, "E204", &field.name); }
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
                local.values.extend(params.iter().map(|param| param.name.clone()));
                self.expression(body, &local, current, false);
            }
            NodeKind::PatternBranch { scrutinee, arms, fallback } => {
                self.expression(scrutinee, scope, current, false);
                for (pat, body) in arms {
                    self.pattern(pat, scope);
                    let mut local = scope.clone();
                    local.values.extend(crate::names::bound_names(pat));
                    self.expression(body, &local, current, false);
                }
                if let Some(body) = fallback { self.expression(body, scope, current, false); }
            }
            _ => {
                for child in crate::check::children(node) { self.expression(child, scope, current, false); }
            }
        }
    }

    fn pattern(&mut self, pattern: &Pattern, scope: &Scope) {
        match &pattern.kind {
            PatternKind::Name(name) if name.starts_with(char::is_uppercase) => {
                if !scope.constructors.contains(name) { self.report(pattern.span, "E204", name); }
            }
            PatternKind::Constructor(name, args) => {
                if !scope.constructors.contains(name) { self.report(pattern.span, "E204", name); }
                for arg in args { self.pattern(arg, scope); }
            }
            PatternKind::Record(fields) => {
                for (_, nested) in fields { if let Some(nested) = nested { self.pattern(nested, scope); } }
            }
            _ => {}
        }
    }
}
