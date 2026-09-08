use crate::parser::{Node, Pattern, PatternKind};
use std::collections::BTreeMap;

#[derive(PartialEq)]
enum PatternShape {
    Any,
    Literal(Node),
    Constructor(String, Vec<PatternShape>),
    Record(BTreeMap<String, PatternShape>),
}

pub fn same_pattern(left: &Pattern, right: &Pattern) -> bool {
    pattern_shape(left) == pattern_shape(right)
}

pub fn covers_pattern(previous: &Pattern, next: &Pattern) -> bool {
    covers_shape(&pattern_shape(previous), &pattern_shape(next))
}

fn covers_shape(previous: &PatternShape, next: &PatternShape) -> bool {
    match (previous, next) {
        (PatternShape::Any, _) => true,
        (PatternShape::Constructor(a, left), PatternShape::Constructor(b, right)) => {
            a == b
                && left.len() == right.len()
                && left.iter().zip(right).all(|(a, b)| covers_shape(a, b))
        }
        (PatternShape::Record(left), PatternShape::Record(right)) => {
            left.iter().all(|(field, pattern)| {
                right
                    .get(field)
                    .is_some_and(|other| covers_shape(pattern, other))
            })
        }
        _ => previous == next,
    }
}

fn pattern_shape(pattern: &Pattern) -> PatternShape {
    match &pattern.kind {
        PatternKind::Name(name) if name.starts_with(char::is_uppercase) => {
            PatternShape::Constructor(name.clone(), Vec::new())
        }
        PatternKind::Name(_) | PatternKind::Underscore => PatternShape::Any,
        PatternKind::Literal(node) => PatternShape::Literal(node.as_ref().clone()),
        PatternKind::Constructor(name, args) => {
            PatternShape::Constructor(name.clone(), args.iter().map(pattern_shape).collect())
        }
        PatternKind::Record(fields) => {
            let fields = fields
                .iter()
                .filter_map(|(name, pattern)| {
                    let shape = pattern
                        .as_ref()
                        .map(pattern_shape)
                        .unwrap_or(PatternShape::Any);
                    if shape == PatternShape::Any {
                        None
                    } else {
                        Some((name.name.clone(), shape))
                    }
                })
                .collect();
            PatternShape::Record(fields)
        }
    }
}

pub fn same_expression(left: &Node, right: &Node) -> bool {
    normalized(left) == normalized(right)
}

pub fn rebuilds_pattern(pattern: &Pattern, node: &Node) -> bool {
    use crate::parser::NodeKind;
    match (&pattern.kind, &node.kind) {
        (PatternKind::Name(a), NodeKind::Name(b)) => a == b,
        (PatternKind::Literal(a), _) => same_expression(a, node),
        (PatternKind::Record(patterns), NodeKind::Record(fields)) => {
            patterns.len() == fields.len()
                && patterns.iter().all(|(name, pattern)| {
                    fields
                        .iter()
                        .find(|(field, _)| field.name == name.name)
                        .is_some_and(|(_, value)| match pattern {
                            Some(pattern) => rebuilds_pattern(pattern, value),
                            None => matches!(&value.kind, NodeKind::Name(value) if value == &name.name),
                        })
                })
        }
        (PatternKind::Constructor(name, patterns), NodeKind::Call(callee, args)) => {
            matches!(&callee.kind, NodeKind::Name(called) if called == name)
                && patterns.len() == args.len()
                && patterns
                    .iter()
                    .zip(args)
                    .all(|(p, n)| rebuilds_pattern(p, n))
        }
        _ => false,
    }
}

fn normalized(node: &Node) -> Node {
    let mut node = node.clone();
    rename(&mut node, &BTreeMap::new(), &mut 0);
    node
}

fn bind_name(name: &mut String, scope: &mut BTreeMap<String, String>, next: &mut usize) {
    let replacement = format!("${next}");
    *next += 1;
    scope.insert(name.clone(), replacement.clone());
    *name = replacement;
}

fn bind_pattern(pattern: &mut Pattern, scope: &mut BTreeMap<String, String>, next: &mut usize) {
    match &mut pattern.kind {
        PatternKind::Name(name) if !name.starts_with(char::is_uppercase) => {
            bind_name(name, scope, next)
        }
        PatternKind::Constructor(_, args) => {
            for arg in args {
                bind_pattern(arg, scope, next);
            }
        }
        PatternKind::Record(fields) => {
            fields.sort_by(|a, b| a.0.name.cmp(&b.0.name));
            for (name, pattern) in fields {
                let pattern = pattern.get_or_insert_with(|| Pattern {
                    kind: PatternKind::Name(name.name.clone()),
                    span: name.span,
                });
                bind_pattern(pattern, scope, next);
            }
        }
        _ => {}
    }
}

fn rename(node: &mut Node, scope: &BTreeMap<String, String>, next: &mut usize) {
    use crate::parser::NodeKind;
    match &mut node.kind {
        NodeKind::Name(name) => {
            if let Some(bound) = scope.get(name) {
                *name = bound.clone();
            }
        }
        NodeKind::Fn { params, body } => {
            let mut scope = scope.clone();
            for param in params {
                bind_name(&mut param.name, &mut scope, next);
            }
            rename(body, &scope, next);
        }
        NodeKind::Let { pat, value, body } | NodeKind::LetQ { pat, value, body } => {
            rename(value, scope, next);
            let mut scope = scope.clone();
            bind_pattern(pat, &mut scope, next);
            rename(body, &scope, next);
        }
        NodeKind::PatternBranch {
            scrutinee,
            arms,
            fallback,
        } => {
            rename(scrutinee, scope, next);
            for (pattern, body) in arms {
                let mut scope = scope.clone();
                bind_pattern(pattern, &mut scope, next);
                rename(body, &scope, next);
            }
            if let Some(body) = fallback {
                rename(body, scope, next);
            }
        }
        NodeKind::Neg(value) | NodeKind::Not(value) | NodeKind::Field(value, _) => {
            rename(value, scope, next)
        }
        NodeKind::Bin(_, left, right)
        | NodeKind::Pipe(left, right)
        | NodeKind::DoQ {
            value: left,
            body: right,
        } => {
            rename(left, scope, next);
            rename(right, scope, next);
        }
        NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
            rename(callee, scope, next);
            for arg in args {
                rename(arg, scope, next);
            }
        }
        NodeKind::List(items) => {
            for item in items {
                rename(item, scope, next);
            }
        }
        NodeKind::Record(fields) => {
            fields.sort_by(|a, b| a.0.name.cmp(&b.0.name));
            for (_, value) in fields {
                rename(value, scope, next);
            }
        }
        NodeKind::BoolBranch { cond, then, els } => {
            for child in [cond, then, els] {
                rename(child, scope, next);
            }
        }
        NodeKind::CondBranch { arms, fallback } => {
            for (condition, body) in arms {
                rename(condition, scope, next);
                rename(body, scope, next);
            }
            if let Some(body) = fallback {
                rename(body, scope, next);
            }
        }
        _ => {}
    }
}
