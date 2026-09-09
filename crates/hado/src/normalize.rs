use crate::parser::{Node, Pattern, PatternKind};
use std::collections::BTreeMap;

#[derive(PartialEq)]
enum PatternShape {
    Any,
    Literal(Node),
    Constructor(String, Vec<PatternShape>),
    Record(BTreeMap<String, PatternShape>),
    NamedConstructor(String, BTreeMap<String, PatternShape>),
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
        (PatternShape::NamedConstructor(a, left), PatternShape::NamedConstructor(b, right)) => {
            a == b && left.iter().all(|(field, pattern)| right.get(field).is_some_and(|other| covers_shape(pattern, other)))
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
        PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
            PatternShape::Constructor(name.clone(), Vec::new())
        }
        PatternKind::Name(_) | PatternKind::Underscore => PatternShape::Any,
        PatternKind::Literal(node) => PatternShape::Literal(node.as_ref().clone()),
        PatternKind::Constructor(name, args) => {
            PatternShape::Constructor(name.clone(), args.iter().map(pattern_shape).collect())
        }
        PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => {
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
            if let PatternKind::NamedConstructor(name, _) = &pattern.kind {
                PatternShape::NamedConstructor(name.name.clone(), fields)
            } else { PatternShape::Record(fields) }
        }
    }
}

pub fn same_expression(left: &Node, right: &Node) -> bool {
    normalized(left) == normalized(right)
}

pub fn rebuilds_pattern(pattern: &Pattern, node: &Node) -> bool {
    use crate::parser::NodeKind;
    match (&pattern.kind, &node.kind) {
        (_, NodeKind::List(items, tail)) => rebuilds_list(pattern, items, tail.as_deref()),
        (PatternKind::Name(a), NodeKind::Name(b)) => a == b,
        (PatternKind::Literal(a), _) => same_expression(a, node),
        (PatternKind::NamedConstructor(pattern_name, patterns), NodeKind::NamedConstructor(value_name, fields)) => {
            pattern_name.name == value_name.name && rebuilds_fields(patterns, fields)
        }
        (PatternKind::Record(patterns), NodeKind::Record(fields)) => rebuilds_fields(patterns, fields),
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

fn rebuilds_fields(patterns: &[(crate::parser::DeclaredName, Option<Pattern>)], fields: &[(crate::parser::DeclaredName, Node)]) -> bool {
    patterns.len() == fields.len() && patterns.iter().all(|(name, pattern)| {
        fields.iter().find(|(field, _)| field.name == name.name).is_some_and(|(_, value)| match pattern {
            Some(pattern) => rebuilds_pattern(pattern, value),
            None => matches!(&value.kind, crate::parser::NodeKind::Name(value) if value == &name.name),
        })
    })
}

fn rebuilds_list(pattern: &Pattern, items: &[Node], tail: Option<&Node>) -> bool {
    let Some((head, rest)) = items.split_first() else {
        return match tail {
            Some(tail) => rebuilds_pattern(pattern, tail),
            None => matches!(&pattern.kind, PatternKind::Name(name) if name == "Nil"),
        };
    };
    let PatternKind::Constructor(name, parts) = &pattern.kind else { return false; };
    name == "Cons" && parts.len() == 2
        && rebuilds_pattern(&parts[0], head) && rebuilds_list(&parts[1], rest, tail)
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
        PatternKind::Name(name) if !crate::names::is_upper_reference(name) => {
            bind_name(name, scope, next)
        }
        PatternKind::Constructor(_, args) => {
            for arg in args {
                bind_pattern(arg, scope, next);
            }
        }
        PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => {
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
        NodeKind::List(items, tail) => {
            for item in items.iter_mut().chain(tail.iter_mut().map(Box::as_mut)) {
                rename(item, scope, next);
            }
        }
        NodeKind::NamedConstructor(_, fields) => {
            for (_, value) in fields { rename(value, scope, next); }
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
