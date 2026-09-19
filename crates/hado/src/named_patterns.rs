use crate::parser::{Pattern, PatternKind};
use std::collections::HashMap;

pub(crate) fn positional(
    pattern: &Pattern,
    fields: &HashMap<String, Vec<String>>,
    resolve: &impl Fn(&str) -> String,
) -> Pattern {
    let mut result = pattern.clone();
    match &pattern.kind {
        PatternKind::NamedConstructor(name, written) => {
            let name = resolve(&name.name);
            if let Some(declared) = fields.get(&name) {
                let arguments = declared
                    .iter()
                    .map(
                        |field| match written.iter().find(|(name, _)| &name.name == field) {
                            Some((name, Some(nested))) => {
                                let _ = name;
                                positional(nested, fields, resolve)
                            }
                            Some((name, None)) => Pattern {
                                kind: PatternKind::Name(name.name.clone()),
                                span: name.span,
                            },
                            None => Pattern {
                                kind: PatternKind::Underscore,
                                span: pattern.span,
                            },
                        },
                    )
                    .collect();
                result.kind = PatternKind::Constructor(name, arguments);
            }
        }
        PatternKind::Constructor(name, arguments) => {
            result.kind = PatternKind::Constructor(
                name.clone(),
                arguments
                    .iter()
                    .map(|arg| positional(arg, fields, resolve))
                    .collect(),
            );
        }
        PatternKind::Record(written) => {
            result.kind = PatternKind::Record(
                written
                    .iter()
                    .map(|(name, nested)| {
                        (
                            name.clone(),
                            nested
                                .as_ref()
                                .map(|nested| positional(nested, fields, resolve)),
                        )
                    })
                    .collect(),
            );
        }
        _ => {}
    }
    result
}
