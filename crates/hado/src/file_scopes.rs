use std::collections::BTreeSet;

use crate::parser::{Ast, DefKind, HandlerArm, HandlerDecl, Item, Node, NodeKind, Param, Pattern};

pub(crate) fn isolate(ast: &mut Ast, file_index: usize) -> Vec<String> {
    let scope = FileScope {
        index: file_index,
        outer: definition_names(&ast.items),
    };
    let roots = if file_index != 0 && scope.outer.contains("main") {
        vec![scope.internal_name("main")]
    } else {
        Vec::new()
    };
    for item in &mut ast.items {
        match item {
            Item::Def(definition) => {
                definition.name = scope.internal_name(&definition.name);
                scope.definition(&mut definition.kind, &BTreeSet::new());
            }
            Item::Handler(handler) => scope.handler(handler, &BTreeSet::new()),
            _ => scope.module(item, &BTreeSet::new()),
        }
    }
    roots
}

fn definition_names(items: &[Item]) -> BTreeSet<String> {
    items.iter().filter_map(|item| match item {
        Item::Def(definition) => Some(definition.name.clone()),
        _ => None,
    }).collect()
}

struct FileScope {
    index: usize,
    outer: BTreeSet<String>,
}

impl FileScope {
    fn internal_name(&self, name: &str) -> String {
        if self.index == 0 && name == "main" {
            name.into()
        } else {
            format!("file{}${name}", self.index)
        }
    }

    fn reference(&self, name: &mut String, bound: &BTreeSet<String>) {
        if !crate::names::is_value_reference(name) || bound.contains(name) {
            return;
        }
        if name == "compare" && !self.outer.contains(name) {
            return;
        }
        *name = self.internal_name(name);
    }

    fn module(&self, item: &mut Item, inherited: &BTreeSet<String>) {
        let items = match item {
            Item::ModuleImpl(module) => &mut module.items,
            _ => return,
        };
        let mut bound = inherited.clone();
        bound.extend(definition_names(items));
        for item in items {
            match item {
                Item::Def(definition) => self.definition(&mut definition.kind, &bound),
                _ => self.module(item, &bound),
            }
        }
    }

    fn handler(&self, handler: &mut HandlerDecl, inherited: &BTreeSet<String>) {
        for arm in &mut handler.arms {
            match arm {
                HandlerArm::Operation { params, body, .. } => self.function(params, body, inherited),
                HandlerArm::Return { param, body, .. } => self.function(std::slice::from_ref(param), body, inherited),
            }
        }
    }

    fn definition(&self, definition: &mut DefKind, bound: &BTreeSet<String>) {
        match definition {
            DefKind::Value(body) => self.expression(body, bound),
            DefKind::Function { params, body, .. } => self.function(params, body, bound),
        }
    }

    fn function(&self, params: &[Param], body: &mut Node, bound: &BTreeSet<String>) {
        let mut bound = bound.clone();
        bound.extend(params.iter().map(|param| param.name.clone()));
        self.expression(body, &bound);
    }

    fn pattern_body(&self, pattern: &Pattern, body: &mut Node, bound: &BTreeSet<String>) {
        let mut bound = bound.clone();
        bound.extend(crate::names::bound_names(pattern));
        self.expression(body, &bound);
    }

    fn expression(&self, node: &mut Node, bound: &BTreeSet<String>) {
        match &mut node.kind {
            NodeKind::Name(name) => self.reference(name, bound),
            NodeKind::Fn { params, body } => self.function(params, body, bound),
            NodeKind::Let { pat, value, body } | NodeKind::LetQ { pat, value, body } => {
                self.expression(value, bound);
                self.pattern_body(pat, body, bound);
            }
            NodeKind::PatternBranch { scrutinee, arms, fallback } => {
                self.expression(scrutinee, bound);
                for (pattern, body) in arms {
                    self.pattern_body(pattern, body, bound);
                }
                self.optional(fallback, bound);
            }
            _ => self.unbound_children(node, bound),
        }
    }

    fn optional(&self, node: &mut Option<Box<Node>>, bound: &BTreeSet<String>) {
        if let Some(node) = node { self.expression(node, bound); }
    }

    fn unbound_children(&self, node: &mut Node, bound: &BTreeSet<String>) {
        match &mut node.kind {
            NodeKind::Neg(value) | NodeKind::Not(value) | NodeKind::Field(value, _) => self.expression(value, bound),
            NodeKind::Bin(_, left, right) | NodeKind::Pipe(left, right) => {
                self.expression(left, bound);
                self.expression(right, bound);
            }
            NodeKind::DoQ { value, body } => {
                self.expression(value, bound);
                self.expression(body, bound);
            }
            NodeKind::Apply(callee, arguments) | NodeKind::Call(callee, arguments) => {
                self.expression(callee, bound);
                for argument in arguments { self.expression(argument, bound); }
            }
            _ => self.aggregate_children(node, bound),
        }
    }

    fn aggregate_children(&self, node: &mut Node, bound: &BTreeSet<String>) {
        match &mut node.kind {
            NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
                for (_, value) in fields { self.expression(value, bound); }
            }
            NodeKind::List(values, tail) => {
                for value in values { self.expression(value, bound); }
                self.optional(tail, bound);
            }
            NodeKind::BoolBranch { cond, then, els } => {
                self.expression(cond, bound);
                self.expression(then, bound);
                self.expression(els, bound);
            }
            NodeKind::CondBranch { arms, fallback } => {
                for (condition, body) in arms {
                    self.expression(condition, bound);
                    self.expression(body, bound);
                }
                self.optional(fallback, bound);
            }
            NodeKind::Int(_) | NodeKind::Float(_) | NodeKind::Char(_) | NodeKind::Str(_) | NodeKind::Bool(_) | NodeKind::Unit | NodeKind::Resume => {}
            NodeKind::With { args, body, .. } => {
                for arg in args { self.expression(arg, bound); }
                self.expression(body, bound);
            }
            _ => unreachable!("scoped expressions and scalar children are handled first"),
        }
    }
}
