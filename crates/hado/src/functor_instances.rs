use std::cell::Cell;
use std::collections::HashMap;

use crate::parser::*;

pub(crate) fn rename(name: &str, modules: &HashMap<String, String>) -> String {
    if let Some(replacement) = modules.get(name) { return replacement.clone(); }
    if let Some((module, member)) = name.split_once('.') {
        if let Some(replacement) = modules.get(module) {
            return format!("{replacement}.{member}");
        }
    }
    name.into()
}

pub(crate) fn instantiate(
    template: &ModuleImpl,
    application: &ModuleImpl,
    modules: &HashMap<String, String>,
    line_offset: usize,
) -> ModuleImpl {
    let mut result = application.clone();
    result.form = ImplementationForm::Ordinary;
    result.items = template.items.clone();
    let instance = Instance { modules, line_offset, maximum: Cell::new(0) };
    for item in &mut result.items { instance.item(item); }
    result
}

pub(crate) fn max_line(ast: &Ast) -> usize {
    let mut ast = ast.clone();
    let modules = HashMap::new();
    let instance = Instance { modules: &modules, line_offset: 0, maximum: Cell::new(ast.span.line) };
    for item in &mut ast.items { instance.item(item); }
    for span in ast.branches_after_fallback.iter_mut()
        .chain(&mut ast.written_list_patterns).chain(&mut ast.long_condition_functions) {
        instance.span(span);
    }
    for (first, second) in &mut ast.branch_anchors {
        instance.span(first);
        instance.span(second);
    }
    for (start, pipe, after) in &mut ast.branch_spans {
        instance.span(start);
        instance.span(pipe);
        instance.span(after);
    }
    for (first, second, operator) in &mut ast.equality_anchors {
        instance.span(first);
        instance.span(second);
        instance.span(operator);
    }
    for (name, member) in &mut ast.qualified_type_anchors {
        instance.declared(name);
        instance.span(member);
    }
    for site in &mut ast.name_sites {
        instance.span(&mut site.start);
        for part in &mut site.parts { instance.declared(part); }
    }
    instance.maximum.get()
}

struct Instance<'a> {
    modules: &'a HashMap<String, String>,
    line_offset: usize,
    maximum: Cell<usize>,
}

impl Instance<'_> {
    fn span(&self, span: &mut Span) {
        span.line += self.line_offset;
        self.maximum.set(self.maximum.get().max(span.line));
    }

    fn declared(&self, name: &mut DeclaredName) { self.span(&mut name.span); }

    fn qualified_type(&self, name: &mut String) {
        if name.contains('.') { *name = rename(name, self.modules); }
    }

    fn parameter(&self, parameter: &mut Param) {
        self.span(&mut parameter.span);
        if let Some(ty) = &mut parameter.ty { self.ty(ty); }
    }

    fn item(&self, item: &mut Item) {
        match item {
            Item::Def(definition) => {
                self.span(&mut definition.span);
                self.span(&mut definition.name_span);
                self.definition(&mut definition.kind);
            }
            Item::TypeDef(definition) => {
                self.span(&mut definition.span);
                self.span(&mut definition.name_span);
                for parameter in &mut definition.params { self.declared(parameter); }
                if let Some(ty) = &mut definition.ty { self.ty(ty); }
            }
            Item::SigDef(definition) => {
                self.span(&mut definition.span);
                self.span(&mut definition.name_span);
                for parameter in &mut definition.type_params { self.declared(parameter); }
                for parameter in &mut definition.params { self.parameter(parameter); }
                if let Some(ty) = &mut definition.ret { self.ty(ty); }
            }
            Item::ModuleSig(module) => {
                self.span(&mut module.span);
                self.span(&mut module.name_span);
                for item in &mut module.items { self.item(item); }
            }
            Item::ModuleImpl(module) => self.module(module),
            Item::Effect(_) | Item::Handler(_) => {}
        }
    }

    fn module(&self, module: &mut ModuleImpl) {
        self.span(&mut module.span);
        self.span(&mut module.name_span);
        self.span(&mut module.sig_span);
        match &mut module.form {
            ImplementationForm::Ordinary => {}
            ImplementationForm::Functor(parameters) => {
                for parameter in parameters {
                    self.span(&mut parameter.name_span);
                    self.span(&mut parameter.contract_span);
                }
            }
            ImplementationForm::Application { functor, arguments } => {
                self.declared(functor);
                functor.name = rename(&functor.name, self.modules);
                for argument in arguments {
                    self.declared(argument);
                    argument.name = rename(&argument.name, self.modules);
                }
            }
        }
        for item in &mut module.items { self.item(item); }
    }

    fn definition(&self, definition: &mut DefKind) {
        match definition {
            DefKind::Value(body) => self.node(body),
            DefKind::Function { params, ret, body } => {
                for parameter in params { self.parameter(parameter); }
                if let Some(ty) = ret { self.ty(ty); }
                self.node(body);
            }
        }
    }

    fn ty(&self, ty: &mut TypeExpr) {
        self.span(&mut ty.span);
        self.span(&mut ty.name_span);
        match &mut ty.kind {
            TypeKind::Name(name) => self.qualified_type(name),
            TypeKind::Apply(name, arguments) => {
                self.qualified_type(name);
                for argument in arguments { self.ty(argument); }
            }
            TypeKind::Adt(variants) => {
                for variant in variants {
                    self.declared(&mut variant.name);
                    if let crate::parser::ConstructorPayload::Named(fields) = &mut variant.payload {
                        for field in fields { self.declared(&mut field.name); }
                    }
                    for ty in variant.payload.types_mut() { self.ty(ty); }
                }
            }
            TypeKind::Group(types) => {
                for ty in types { self.ty(ty); }
            }
            TypeKind::Record(fields) => {
                for field in fields {
                    self.declared(&mut field.name);
                    self.ty(&mut field.ty);
                }
            }
        }
        if let Some(crate::parser::EffectRow::Row(effects)) = &mut ty.effects {
            for ty in effects { self.ty(ty); }
        }
    }

    fn pattern(&self, pattern: &mut Pattern) {
        self.span(&mut pattern.span);
        match &mut pattern.kind {
            PatternKind::Constructor(_, arguments) => {
                for argument in arguments { self.pattern(argument); }
            }
            PatternKind::NamedConstructor(name, fields) => {
                self.declared(name);
                for (name, pattern) in fields {
                    self.declared(name);
                    if let Some(pattern) = pattern { self.pattern(pattern); }
                }
            }
            PatternKind::Record(fields) => {
                for (name, pattern) in fields {
                    self.declared(name);
                    if let Some(pattern) = pattern { self.pattern(pattern); }
                }
            }
            PatternKind::Literal(node) => self.node(node),
            PatternKind::Name(_) | PatternKind::Underscore => {}
        }
    }

    fn optional_node(&self, node: &mut Option<Box<Node>>) {
        if let Some(node) = node { self.node(node); }
    }

    fn node(&self, node: &mut Node) {
        self.span(&mut node.span);
        match &mut node.kind {
            NodeKind::Field(base, field) => {
                if let NodeKind::Name(name) = &mut base.kind {
                    *name = rename(name, self.modules);
                }
                self.span(&mut field.span);
                self.span(&mut field.dot);
                self.node(base);
            }
            NodeKind::Fn { params, body } => {
                for parameter in params { self.parameter(parameter); }
                self.node(body);
            }
            NodeKind::Let { pat, value, body } | NodeKind::LetQ { pat, value, body } => {
                self.pattern(pat);
                self.node(value);
                self.node(body);
            }
            NodeKind::PatternBranch { scrutinees, arms, fallback } => {
                for scrutinee in scrutinees { self.node(scrutinee); }
                for (patterns, body) in arms {
                    for pattern in patterns { self.pattern(pattern); }
                    self.node(body);
                }
                self.optional_node(fallback);
            }
            _ => self.scalar_children(node),
        }
    }

    fn scalar_children(&self, node: &mut Node) {
        match &mut node.kind {
            NodeKind::Neg(value) | NodeKind::Not(value) => self.node(value),
            NodeKind::Bin(_, left, right) | NodeKind::Pipe(left, right) => {
                self.node(left);
                self.node(right);
            }
            NodeKind::DoQ { value, body } => {
                self.node(value);
                self.node(body);
            }
            NodeKind::Apply(callee, arguments) | NodeKind::Call(callee, arguments) => {
                self.node(callee);
                for argument in arguments { self.node(argument); }
            }
            _ => self.aggregate_children(node),
        }
    }

    fn aggregate_children(&self, node: &mut Node) {
        match &mut node.kind {
            NodeKind::NamedConstructor(name, fields) => {
                self.declared(name);
                for (name, value) in fields { self.declared(name); self.node(value); }
            }
            NodeKind::Record(fields) => {
                for (name, value) in fields {
                    self.declared(name);
                    self.node(value);
                }
            }
            NodeKind::List(values, tail) => {
                for value in values { self.node(value); }
                self.optional_node(tail);
            }
            NodeKind::BoolBranch { cond, then, els } => {
                self.node(cond);
                self.node(then);
                self.node(els);
            }
            NodeKind::CondBranch { arms, fallback } => {
                for (condition, body) in arms {
                    self.node(condition);
                    self.node(body);
                }
                self.optional_node(fallback);
            }
            NodeKind::Int(_) | NodeKind::Float(_) | NodeKind::Char(_) | NodeKind::Str(_) | NodeKind::Bool(_) | NodeKind::Unit | NodeKind::Name(_) | NodeKind::Resume => {}
            NodeKind::With { args, body, .. } => {
                for arg in args { self.node(arg); }
                self.node(body);
            }
            _ => unreachable!("binding and scalar children are handled first"),
        }
    }
}
