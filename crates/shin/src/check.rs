use crate::parser::{Ast, DefKind, Diagnostic, Item, Node, NodeKind, Pattern, PatternKind};

pub fn check_ast(ast: &Ast, path: &str) -> Result<(), Diagnostic> {
    let mut diagnostics = Vec::new();
    for span in &ast.branches_after_fallback {
        diagnostics.push(Diagnostic {
            path: path.into(), line: span.line, col: span.col,
            code: Some("E404".into()), message: "fallback must be the last branch".into(),
        });
    }
    if let Err(error) = crate::names::check_cycles(ast, path) {
        diagnostics.push(error);
    }
    check_items(&ast.items, path, &mut diagnostics);
    match diagnostics.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn check_items(items: &[Item], path: &str, diagnostics: &mut Vec<Diagnostic>) {
    for item in items {
        match item {
            Item::Def(def) => match &def.kind {
                DefKind::Value(body) | DefKind::Function { body, .. } => check_node(body, path, diagnostics),
            },
            Item::ModuleImpl(module) => check_items(&module.items, path, diagnostics),
            _ => {}
        }
    }
}

pub(crate) fn boolean_coverage(
    scrutinee: &Node,
    arms: &[(Pattern, Node)],
    fallback: Option<&Node>,
    path: &str,
) -> Result<(), Diagnostic> {
    let exhaustive = [false, true].iter().all(|value| {
        arms.iter().any(|(pattern, _)| {
        matches!(&pattern.kind, PatternKind::Literal(node) if node.kind == NodeKind::Bool(*value))
            || matches!(&pattern.kind, PatternKind::Underscore)
            || matches!(&pattern.kind, PatternKind::Name(name) if !name.starts_with(char::is_uppercase))
    })
    });
    let (code, location, message) = match (exhaustive, fallback) {
        (false, None) => (
            "E402",
            scrutinee,
            "Boolean patterns do not cover both values",
        ),
        (true, Some(body)) => (
            "E403",
            body,
            "fallback is unreachable after both Boolean values",
        ),
        _ => return Ok(()),
    };
    Err(Diagnostic {
        path: path.into(),
        line: location.span.line,
        col: location.span.col,
        code: Some(code.into()),
        message: message.into(),
    })
}

fn check_node(node: &Node, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    for check in [check_cons_tail, check_duplicate_patterns, check_conditions,
        check_literal_fallback, check_local_redundancy, check_rewrap, check_repropagation] {
        if let Err(error) = check(node, path) {
            diagnostics.push(error);
        }
    }
    for child in children(node) {
        check_node(child, path, diagnostics);
    }
}

fn check_local_redundancy(node: &Node, path: &str) -> Result<(), Diagnostic> {
    if let NodeKind::BoolBranch { then, els, .. } = &node.kind {
        if crate::normalize::same_expression(then, els) {
            return Err(Diagnostic {
                path: path.into(),
                line: node.span.line,
                col: node.span.col,
                code: Some("E502".into()),
                message: format!(
                    "identical branch results at {}:{} and {}:{}",
                    then.span.line, then.span.col, els.span.line, els.span.col
                ),
            });
        }
    }
    if let NodeKind::Call(callee, args) = &node.kind {
        if matches!(&callee.kind, NodeKind::Name(name) if name == "Ok")
            && args.len() == 1
            && matches!(args[0].kind, NodeKind::Unit)
        {
            return Err(Diagnostic {
                path: path.into(),
                line: node.span.line,
                col: node.span.col,
                code: Some("E509".into()),
                message: "use Ok() for a Unit success value".into(),
            });
        }
    }
    if let NodeKind::PatternBranch { arms, .. } = &node.kind {
        for (pattern, _) in arms {
            if matches!(pattern.kind, PatternKind::Underscore) {
                return Err(Diagnostic {
                    path: path.into(),
                    line: pattern.span.line,
                    col: pattern.span.col,
                    code: Some("E503".into()),
                    message: "use a fallback instead of a catch-all underscore pattern".into(),
                });
            }
            if matches!(&pattern.kind, PatternKind::Name(name) if !name.starts_with(char::is_uppercase))
            {
                return Err(Diagnostic {
                    path: path.into(),
                    line: pattern.span.line,
                    col: pattern.span.col,
                    code: Some("E512".into()),
                    message: "top-level variable patterns are forbidden".into(),
                });
            }
        }
    }
    if let NodeKind::LetQ { pat, .. } = &node.kind {
        if matches!(pat.kind, PatternKind::Underscore) {
            return Err(Diagnostic {
                path: path.into(),
                line: node.span.line,
                col: node.span.col,
                code: Some("E504".into()),
                message: "use do? to discard a success value".into(),
            });
        }
    }
    if let NodeKind::Let { pat, body, .. } = &node.kind {
        if matches!(pat.kind, PatternKind::Underscore) {
            return Err(Diagnostic {
                path: path.into(),
                line: pat.span.line,
                col: pat.span.col,
                code: Some("E506".into()),
                message: "a plain let binding cannot discard its value".into(),
            });
        }
        if let (PatternKind::Name(bound), NodeKind::Name(returned)) = (&pat.kind, &body.kind) {
            if bound == returned {
                return Err(Diagnostic {
                    path: path.into(),
                    line: node.span.line,
                    col: node.span.col,
                    code: Some("E501".into()),
                    message: format!(
                        "binding {bound} is returned unchanged at {}:{}",
                        body.span.line, body.span.col
                    ),
                });
            }
        }
    }
    if let NodeKind::Let { pat, body, .. } | NodeKind::LetQ { pat, body, .. } = &node.kind {
        for name in crate::names::bound_names(pat) {
            if !crate::names::references(body, &name) {
                return Err(Diagnostic {
                    path: path.into(),
                    line: pat.span.line,
                    col: pat.span.col,
                    code: Some("E505".into()),
                    message: format!("binding {name} is unused in its continuation"),
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn adt_coverage(
    scrutinee: &Node,
    arms: &[(Pattern, Node)],
    fallback: Option<&Node>,
    constructors: &[String],
    families: &crate::coverage::Families,
    path: &str,
) -> Result<(), Diagnostic> {
    let coverage: Vec<_> = if constructors.is_empty() {
        vec![crate::coverage::record_patterns(arms, families)]
    } else {
        constructors
            .iter()
            .map(|name| crate::coverage::constructor_payloads(name, arms, families))
            .collect()
    };
    let exhaustive = coverage.iter().all(|covered| *covered == Some(true));
    let missing = coverage.contains(&Some(false));
    let (code, location, message) = match (exhaustive, fallback) {
        (false, None) if missing => ("E402", scrutinee, "not all constructor values are covered"),
        (true, Some(body)) => (
            "E403",
            body,
            "fallback is unreachable after all constructors",
        ),
        _ => return Ok(()),
    };
    Err(Diagnostic {
        path: path.into(),
        line: location.span.line,
        col: location.span.col,
        code: Some(code.into()),
        message: message.into(),
    })
}

fn check_repropagation(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::PatternBranch { arms, .. } = &node.kind else {
        return Ok(());
    };
    for (pattern, body) in arms {
        if matches!(&pattern.kind, PatternKind::Constructor(name, args)
            if name == "Error" && args.len() == 1
                && matches!(&args[0].kind, PatternKind::Name(bound) if !bound.starts_with(char::is_uppercase)))
            && crate::normalize::rebuilds_pattern(pattern, body)
        {
            return Err(Diagnostic {
                path: path.into(),
                line: body.span.line,
                col: body.span.col,
                code: Some("E508".into()),
                message: format!(
                    "failure bound at {}:{} is propagated unchanged; use let? or do?",
                    pattern.span.line, pattern.span.col
                ),
            });
        }
    }
    Ok(())
}

fn check_rewrap(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::PatternBranch {
        scrutinee,
        arms,
        fallback,
    } = &node.kind
    else {
        return Ok(());
    };
    let fallback_unchanged = fallback.as_ref().map_or(true, |body| {
        crate::normalize::same_expression(scrutinee, body)
    });
    if !arms.is_empty()
        && arms.iter().all(|(pattern, _)| !matches!(&pattern.kind,
            PatternKind::Underscore) && !matches!(&pattern.kind,
            PatternKind::Name(name) if !name.starts_with(char::is_uppercase)))
        && fallback_unchanged
        && arms
            .iter()
            .all(|(pattern, body)| crate::normalize::rebuilds_pattern(pattern, body))
    {
        return Err(Diagnostic {
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            code: Some("E507".into()),
            message: format!(
                "branches reconstruct the value at {}:{}",
                scrutinee.span.line, scrutinee.span.col
            ),
        });
    }
    Ok(())
}

fn check_literal_fallback(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::PatternBranch { arms, fallback, .. } = &node.kind else {
        return Ok(());
    };
    if fallback.is_none()
        && arms
            .iter()
            .any(|(pattern, _)| needs_literal_fallback(pattern))
    {
        return Err(Diagnostic {
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            code: Some("E402".into()),
            message: "non-Boolean literal patterns require a fallback".into(),
        });
    }
    Ok(())
}

fn needs_literal_fallback(pattern: &Pattern) -> bool {
    match &pattern.kind {
        PatternKind::Literal(value) => !matches!(value.kind, NodeKind::Bool(_)),
        PatternKind::Constructor(_, args) => args.iter().any(needs_literal_fallback),
        PatternKind::Record(fields) => fields
            .iter()
            .filter_map(|(_, p)| p.as_ref())
            .any(needs_literal_fallback),
        _ => false,
    }
}

fn check_conditions(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::CondBranch { arms, fallback } = &node.kind else {
        return Ok(());
    };
    if fallback.is_none() {
        return Err(Diagnostic {
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            code: Some("E401".into()),
            message: "condition columns require a fallback".into(),
        });
    }
    for (index, (condition, _)) in arms.iter().enumerate() {
        if let Some((previous, _)) = arms[..index].iter().find(|(previous, _)| {
            crate::normalize::same_expression(previous, condition)
                || matches!(previous.kind, NodeKind::Bool(true))
        }) {
            return Err(Diagnostic {
                path: path.into(),
                line: condition.span.line,
                col: condition.span.col,
                code: Some("E406".into()),
                message: format!(
                    "condition is redundant after {}:{}",
                    previous.span.line, previous.span.col
                ),
            });
        }
    }
    if let Some(name) = arms
        .first()
        .and_then(|(condition, _)| literal_comparison(condition))
    {
        if arms
            .iter()
            .all(|(condition, _)| literal_comparison(condition) == Some(name))
        {
            return Err(Diagnostic {
                path: path.into(),
                line: node.span.line,
                col: node.span.col,
                code: Some("E511".into()),
                message: "use literal patterns for equality tests of one variable".into(),
            });
        }
    }
    Ok(())
}

fn literal_comparison(node: &Node) -> Option<&str> {
    let NodeKind::Bin(crate::parser::BinOp::Eq, left, right) = &node.kind else {
        return None;
    };
    let NodeKind::Name(name) = &left.kind else {
        return None;
    };
    matches!(
        right.kind,
        NodeKind::Int(_)
            | NodeKind::Float(_)
            | NodeKind::Char(_)
            | NodeKind::Str(_)
            | NodeKind::Bool(_)
    )
    .then_some(name)
}

fn check_duplicate_patterns(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::PatternBranch { arms, .. } = &node.kind else {
        return Ok(());
    };
    for (index, (pattern, _)) in arms.iter().enumerate() {
        if let Some((previous, _)) = arms[..index]
            .iter()
            .find(|(previous, _)| crate::normalize::covers_pattern(previous, pattern))
        {
            let duplicate = crate::normalize::same_pattern(previous, pattern);
            return Err(Diagnostic {
                path: path.into(),
                line: pattern.span.line,
                col: pattern.span.col,
                code: Some(if duplicate { "E405" } else { "E403" }.into()),
                message: format!(
                    "pattern is covered by the arm at {}:{}",
                    previous.span.line, previous.span.col
                ),
            });
        }
    }
    Ok(())
}

fn check_cons_tail(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::Call(callee, args) = &node.kind else {
        return Ok(());
    };
    if !matches!(&callee.kind, NodeKind::Name(name) if name == "Cons") || args.len() != 2 {
        return Ok(());
    }
    let literal = matches!(&args[1].kind, NodeKind::List(_))
        || matches!(&args[1].kind, NodeKind::Name(name) if name == "Nil");
    if literal {
        return Err(Diagnostic {
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            code: Some("E510".into()),
            message: "use a List literal instead of Cons with a literal tail".into(),
        });
    }
    Ok(())
}

fn children(node: &Node) -> Vec<&Node> {
    match &node.kind {
        NodeKind::Neg(value)
        | NodeKind::Not(value)
        | NodeKind::Field(value, _)
        | NodeKind::Fn { body: value, .. } => vec![value],
        NodeKind::Bin(_, left, right) | NodeKind::Pipe(left, right) => vec![left, right],
        NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
            std::iter::once(callee.as_ref())
                .chain(args.iter())
                .collect()
        }
        NodeKind::List(items) => items.iter().collect(),
        NodeKind::Record(fields) => fields.iter().map(|(_, value)| value).collect(),
        NodeKind::Let { value, body, .. }
        | NodeKind::LetQ { value, body, .. }
        | NodeKind::DoQ { value, body } => vec![value, body],
        NodeKind::BoolBranch { cond, then, els } => vec![cond, then, els],
        NodeKind::CondBranch { arms, fallback } => arms
            .iter()
            .flat_map(|(condition, body)| [condition, body])
            .chain(fallback.iter().map(Box::as_ref))
            .collect(),
        NodeKind::PatternBranch {
            scrutinee,
            arms,
            fallback,
        } => std::iter::once(scrutinee.as_ref())
            .chain(arms.iter().map(|(_, body)| body))
            .chain(fallback.iter().map(Box::as_ref))
            .collect(),
        _ => vec![],
    }
}
