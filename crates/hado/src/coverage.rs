use crate::parser::{Node, NodeKind, Pattern, PatternKind};
use std::collections::HashMap;

pub(crate) type Families = HashMap<String, Vec<(String, usize)>>;

#[derive(Clone)]
enum Cell {
    Any,
    Constructor(String, Vec<Cell>),
    Record(std::collections::BTreeMap<String, Cell>),
    Unknown,
}

pub(crate) fn covers_pattern(pattern: &Pattern, families: &Families) -> Option<bool> {
    covers(&[vec![cell(pattern)]], families)
}

pub(crate) fn constructor_payloads(
    name: &str,
    arms: &[(Pattern, Node)],
    families: &Families,
) -> Option<bool> {
    if arms.iter().any(|(pattern, _)| matches!(cell(pattern), Cell::Any)) {
        return Some(true);
    }
    let rows: Vec<_> = arms
        .iter()
        .filter_map(|(pattern, _)| match cell(pattern) {
            Cell::Constructor(n, args) if n == name => Some(args),
            _ => None,
        })
        .collect();
    covers(&rows, families)
}

pub(crate) fn record_patterns(arms: &[(Pattern, Node)], families: &Families) -> Option<bool> {
    covers(
        &arms
            .iter()
            .map(|(pattern, _)| vec![cell(pattern)])
            .collect::<Vec<_>>(),
        families,
    )
}

fn cell(pattern: &Pattern) -> Cell {
    match &pattern.kind {
        PatternKind::Underscore => Cell::Any,
        PatternKind::Name(name) if !name.starts_with(char::is_uppercase) => Cell::Any,
        PatternKind::Name(name) => Cell::Constructor(name.clone(), vec![]),
        PatternKind::Constructor(name, args) => {
            Cell::Constructor(name.clone(), args.iter().map(cell).collect())
        }
        PatternKind::Literal(node) => match node.kind {
            NodeKind::Bool(value) => Cell::Constructor(value.to_string(), vec![]),
            _ => Cell::Unknown,
        },
        PatternKind::NamedConstructor(..) => Cell::Unknown,
        PatternKind::Record(fields) => Cell::Record(
            fields
                .iter()
                .map(|(name, pattern)| {
                    (
                        name.name.clone(),
                        pattern.as_ref().map(cell).unwrap_or(Cell::Any),
                    )
                })
                .collect(),
        ),
    }
}

fn covers(rows: &[Vec<Cell>], families: &Families) -> Option<bool> {
    if rows.is_empty() {
        return Some(false);
    }
    if rows
        .iter()
        .any(|row| row.iter().all(|cell| matches!(cell, Cell::Any)))
    {
        return Some(true);
    }
    if rows.iter().any(|row| matches!(row[0], Cell::Record(_))) {
        return covers_records(rows, families);
    }
    let head = rows.iter().find_map(|row| match &row[0] {
        Cell::Constructor(name, _) => Some(name),
        _ => None,
    });
    let Some(name) = head else {
        if rows.iter().any(|row| matches!(row[0], Cell::Unknown)) {
            return None;
        }
        return covers(
            &rows.iter().map(|row| row[1..].to_vec()).collect::<Vec<_>>(),
            families,
        );
    };
    let alternatives = if name == "true" || name == "false" {
        vec![("true".into(), 0), ("false".into(), 0)]
    } else {
        families.get(name)?.clone()
    };
    let mut unknown = false;
    for (name, arity) in alternatives {
        let specialized: Vec<_> = rows
            .iter()
            .filter_map(|row| specialize(row, &name, arity))
            .collect();
        match covers(&specialized, families) {
            Some(false) => return Some(false),
            None => unknown = true,
            Some(true) => {}
        }
    }
    if unknown {
        None
    } else {
        Some(true)
    }
}

fn specialize(row: &[Cell], name: &str, arity: usize) -> Option<Vec<Cell>> {
    let mut result = match &row[0] {
        Cell::Any => vec![Cell::Any; arity],
        Cell::Constructor(found, args) if found == name => args.clone(),
        _ => return None,
    };
    result.extend_from_slice(&row[1..]);
    Some(result)
}

fn covers_records(rows: &[Vec<Cell>], families: &Families) -> Option<bool> {
    let fields: std::collections::BTreeSet<_> = rows
        .iter()
        .flat_map(|row| match &row[0] {
            Cell::Record(fields) => fields.keys().cloned().collect::<Vec<_>>(),
            _ => vec![],
        })
        .collect();
    let expanded: Vec<_> = rows
        .iter()
        .map(|row| {
            let mut result: Vec<_> = fields
                .iter()
                .map(|name| match &row[0] {
                    Cell::Record(fields) => fields.get(name).cloned().unwrap_or(Cell::Any),
                    Cell::Any => Cell::Any,
                    _ => Cell::Unknown,
                })
                .collect();
            result.extend_from_slice(&row[1..]);
            result
        })
        .collect();
    covers(&expanded, families)
}
