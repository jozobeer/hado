use crate::parser::{Ast, DefKind, Diagnostic, Item, Node, NodeKind, Pattern, PatternKind, TypeExpr, TypeKind};

pub fn check_ast(ast: &Ast, path: &str) -> Result<(), Diagnostic> {
    let mut diagnostics = Vec::new();
    for span in &ast.long_condition_functions {
        diagnostics.push(Diagnostic {
            path: path.into(), line: span.line, col: span.col,
            code: Some("E517".into()), message: "write condition arms directly after the function parameters".into(),
        });
    }
    for span in &ast.written_list_patterns {
        diagnostics.push(Diagnostic {
            path: path.into(), line: span.line, col: span.col,
            code: Some("E516".into()), message: "use List pattern syntax instead of Nil or Cons".into(),
        });
    }
    for span in &ast.branches_after_fallback {
        diagnostics.push(Diagnostic {
            path: path.into(), line: span.line, col: span.col,
            code: Some("E404".into()), message: "fallback must be the last branch".into(),
        });
    }
    if let Err(error) = crate::names::check_cycles(ast, path) {
        diagnostics.push(error);
    }
    check_items(&ast.items, &ast.items, None, path, &mut diagnostics);
    match diagnostics.into_iter().map(|error| arm_diagnostic(ast, error)).min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(crate) fn arm_diagnostic(ast: &Ast, mut error: Diagnostic) -> Diagnostic {
    if matches!(error.code.as_deref(), Some("E403" | "E405" | "E406" | "E508")) {
        if let Some((_, pipe)) = ast.branch_anchors.iter().find(|(source, _)| source.line == error.line && source.col == error.col) {
            error.line = pipe.line;
            error.col = pipe.col;
        }
    }
    qualified_type_diagnostic(ast, error)
}

fn qualified_type_diagnostic(ast: &Ast, mut error: Diagnostic) -> Diagnostic {
    if error.code.as_deref() != Some("E204") { return error; }
    let member = ast.qualified_type_anchors.iter().find_map(|(module, member)| {
        let in_contract = ast.items.iter().any(|item| match item {
            Item::ModuleSig(signature) => signature.items.iter().any(|item| match item {
                Item::SigDef(def) => def.params.iter().filter_map(|param| param.ty.as_ref()).chain(def.ret.iter())
                    .any(|ty| type_has_span(ty, module.span)),
                _ => false,
            }),
            _ => false,
        });
        let known = ast.items.iter().any(|item| match item {
            Item::ModuleSig(signature) if in_contract => signature.name == module.name,
            Item::ModuleImpl(implementation) if !in_contract => implementation.name == module.name
                && !matches!(implementation.form, crate::parser::ImplementationForm::Functor(_))
                || (!in_contract && known_parameter_type_scope(implementation, &module.name, module.span, &ast.items)),
            _ => false,
        });
        (module.span.line == error.line && module.span.col == error.col && known).then_some(*member)
    });
    if let Some(member) = member {
        error.line = member.line;
        error.col = member.col;
    }
    error
}

fn known_parameter_type_scope(module: &crate::parser::ModuleImpl, name: &str, span: crate::parser::Span, items: &[Item]) -> bool {
    let crate::parser::ImplementationForm::Functor(parameters) = &module.form else { return false; };
    parameters.iter().any(|parameter| parameter.name == name && items.iter().any(|item|
        matches!(item, Item::ModuleSig(signature) if signature.name == parameter.contract)))
        && module.items.iter().any(|item| item_type_has_span(item, span))
}

fn item_type_has_span(item: &Item, span: crate::parser::Span) -> bool {
    match item {
        Item::TypeDef(definition) => definition.ty.as_ref().is_some_and(|ty| type_has_span(ty, span)),
        Item::Def(definition) => match &definition.kind {
            DefKind::Value(body) => node_type_has_span(body, span),
            DefKind::Function { params, ret, body } => params.iter().filter_map(|param| param.ty.as_ref()).chain(ret.iter())
                .any(|ty| type_has_span(ty, span)) || node_type_has_span(body, span),
        },
        _ => false,
    }
}

fn node_type_has_span(node: &Node, span: crate::parser::Span) -> bool {
    if let NodeKind::Fn { params, .. } = &node.kind {
        if params.iter().filter_map(|param| param.ty.as_ref()).any(|ty| type_has_span(ty, span)) { return true; }
    }
    children(node).iter().any(|child| node_type_has_span(child, span))
}

fn type_has_span(ty: &crate::parser::TypeExpr, span: crate::parser::Span) -> bool {
    if ty.span == span { return true; }
    if let Some(crate::parser::EffectRow::Row(effects)) = &ty.effects {
        if effects.iter().any(|ty| type_has_span(ty, span)) { return true; }
    }
    match &ty.kind {
        crate::parser::TypeKind::Adt(variants) => variants.iter().any(|variant| variant.name.span == span || variant.payload.types().any(|ty| type_has_span(ty, span))),
        crate::parser::TypeKind::Apply(_, args) | crate::parser::TypeKind::Group(args) => args.iter().any(|ty| type_has_span(ty, span)),
        crate::parser::TypeKind::Record(fields) => fields.iter().any(|field| type_has_span(&field.ty, span)),
        _ => false,
    }
}

fn check_items(
    items: &[Item], contracts: &[Item], members: Option<&[Item]>,
    path: &str, diagnostics: &mut Vec<Diagnostic>,
) {
    for item in items {
        check_type_parameters(item, path, diagnostics);
        match item {
            Item::Def(def) => check_definition(def, is_contract_member(members, &def.name), path, diagnostics),
            Item::ModuleImpl(module) => check_items(&module.items, contracts, contract_members(contracts, &module.sig), path, diagnostics),
            Item::ModuleSig(signature) => check_items(&signature.items, contracts, None, path, diagnostics),
            _ => {}
        }
    }
}

fn contract_members<'a>(items: &'a [Item], name: &str) -> Option<&'a [Item]> {
    items.iter().find_map(|item| match item {
        Item::ModuleSig(signature) if signature.name == name => Some(signature.items.as_slice()),
        _ => None,
    })
}

fn is_contract_member(members: Option<&[Item]>, name: &str) -> bool {
    members.is_some_and(|items| items.iter().any(|item| matches!(item, Item::SigDef(member) if member.name == name)))
}

fn check_definition(def: &crate::parser::Def, contract_member: bool, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    match &def.kind {
        DefKind::Value(body) => check_node(body, path, diagnostics, false),
        DefKind::Function { params, ret, body } => {
            if !contract_member {
                for param in params.iter().filter(|param| param.name == "_") {
                    diagnostics.push(Diagnostic { path: path.into(), line: param.span.line, col: param.span.col,
                        code: Some("E519".into()), message: "underscore parameters require a contract member or anonymous function".into() });
                }
            }
            check_unused(params.iter().map(|param| (param.name.as_str(), param.span)), body, path, diagnostics);
            if let Err(error) = check_sole_parameter_branch(params, ret.as_ref(), body, def.span, path) {
                diagnostics.push(error);
            }
            check_node(body, path, diagnostics, body.span == def.span);
        }
    }
}

fn check_type_parameters(item: &Item, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (parameters, types): (&[_], Vec<&TypeExpr>) = match item {
        Item::TypeDef(def) if def.ty.is_some() => (&def.params, def.ty.iter().collect()),
        Item::SigDef(def) => (&def.type_params, def.params.iter().filter_map(|param| param.ty.as_ref()).chain(def.ret.iter()).collect()),
        _ => return,
    };
    for parameter in parameters {
        if types.iter().any(|ty| references_type_parameter(ty, &parameter.name)) { continue; }
        diagnostics.push(Diagnostic { path: path.into(), line: parameter.span.line, col: parameter.span.col,
            code: Some("E518".into()), message: format!("type parameter {} is unused", parameter.name) });
    }
}

fn references_type_parameter(ty: &TypeExpr, parameter: &str) -> bool {
    let in_kind = match &ty.kind {
        TypeKind::Name(name) => name == parameter,
        TypeKind::Apply(name, arguments) => name == parameter || arguments.iter().any(|ty| references_type_parameter(ty, parameter)),
        TypeKind::Group(arguments) => arguments.iter().any(|ty| references_type_parameter(ty, parameter)),
        TypeKind::Record(fields) => fields.iter().any(|field| references_type_parameter(&field.ty, parameter)),
        TypeKind::Adt(variants) => variants.iter().any(|variant| constructor_uses_type_parameter(variant, parameter)),
    };
    in_kind || matches!(&ty.effects, Some(crate::parser::EffectRow::Row(effects)) if effects.iter().any(|ty| references_type_parameter(ty, parameter)))
}

fn constructor_uses_type_parameter(variant: &crate::parser::ConstructorDecl, parameter: &str) -> bool {
    variant.payload.types().any(|ty| references_type_parameter(ty, parameter))
}

fn check_unused<'a>(
    bindings: impl IntoIterator<Item = (&'a str, crate::parser::Span)>, body: &Node,
    path: &str, diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, span) in bindings {
        if matches!(name, "$argument" | "_") || crate::names::references(body, name) { continue; }
        diagnostics.push(Diagnostic { path: path.into(), line: span.line, col: span.col,
            code: Some("E505".into()), message: format!("binding {name} is unused") });
    }
}

fn check_sole_parameter_branch(
    params: &[crate::parser::Param], ret: Option<&crate::parser::TypeExpr>,
    body: &Node, span: crate::parser::Span, path: &str,
) -> Result<(), Diagnostic> {
    let [parameter] = params else { return Ok(()); };
    if parameter.ty.is_some() || ret.is_some() || parameter.name == "$argument" { return Ok(()); }
    let NodeKind::PatternBranch { scrutinee, arms, fallback } = &body.kind else { return Ok(()); };
    if !matches!(&scrutinee.kind, NodeKind::Name(name) if name == &parameter.name) { return Ok(()); }
    let referenced_in_arm = arms.iter().any(|(pattern, body)|
        !crate::names::bound_names(pattern).contains(&parameter.name)
            && crate::names::references(body, &parameter.name));
    if referenced_in_arm || fallback.as_ref().is_some_and(|body| crate::names::references(body, &parameter.name)) {
        return Ok(());
    }
    Err(Diagnostic { path: path.into(), line: span.line, col: span.col,
        code: Some("E515".into()), message: "use pattern arms directly for the sole parameter".into() })
}

pub(crate) fn boolean_coverage(
    branch_span: crate::parser::Span,
    arms: &[(Pattern, Node)],
    fallback: Option<&Node>,
    path: &str,
) -> Result<(), Diagnostic> {
    let exhaustive = [false, true].iter().all(|value| {
        arms.iter().any(|(pattern, _)| {
        matches!(&pattern.kind, PatternKind::Literal(node) if node.kind == NodeKind::Bool(*value))
            || matches!(&pattern.kind, PatternKind::Underscore)
            || matches!(&pattern.kind, PatternKind::Name(name) if !crate::names::is_upper_reference(name))
    })
    });
    let (code, location, message) = match (exhaustive, fallback) {
        (false, None) => (
            "E402",
            branch_span,
            "Boolean patterns do not cover both values",
        ),
        (true, Some(body)) => (
            "E403",
            body.span,
            "fallback is unreachable after both Boolean values",
        ),
        _ => return Ok(()),
    };
    Err(Diagnostic {
        path: path.into(),
        line: location.line,
        col: location.col,
        code: Some(code.into()),
        message: message.into(),
    })
}

fn check_boolean_form(node: &Node, path: &str, direct_condition: bool) -> Result<(), Diagnostic> {
    let redundant = match &node.kind {
        NodeKind::PatternBranch { arms, .. } => !arms.is_empty() && arms.iter().all(|(pattern, _)|
            matches!(&pattern.kind, PatternKind::Literal(value) if matches!(value.kind, NodeKind::Bool(_)))),
        NodeKind::CondBranch { arms, .. } => arms.len() == 1 && !direct_condition,
        _ => false,
    };
    if !redundant { return Ok(()); }
    Err(Diagnostic { path: path.into(), line: node.span.line, col: node.span.col,
        code: Some("E520".into()), message: "use a Boolean branch instead of Boolean pattern arms or a single condition arm".into() })
}

fn check_node(node: &Node, path: &str, diagnostics: &mut Vec<Diagnostic>, direct_condition: bool) {
    if let Err(error) = check_boolean_form(node, path, direct_condition) {
        diagnostics.push(error);
    }
    if let NodeKind::Fn { params, body } = &node.kind {
        check_unused(params.iter().map(|param| (param.name.as_str(), param.span)), body, path, diagnostics);
        if let Err(error) = check_sole_parameter_branch(params, None, body, node.span, path) {
            diagnostics.push(error);
        }
    }
    match &node.kind {
        NodeKind::Let { pat, body, .. } | NodeKind::LetQ { pat, body, .. } => {
            check_unused(crate::names::bindings(pat), body, path, diagnostics);
        }
        NodeKind::PatternBranch { arms, .. } => {
            for (pattern, body) in arms {
                check_unused(crate::names::bindings(pattern), body, path, diagnostics);
            }
        }
        _ => {}
    }
    for check in [check_cons_tail, check_duplicate_patterns, check_conditions,
        check_literal_fallback, check_local_redundancy, check_rewrap, check_repropagation] {
        if let Err(error) = check(node, path) {
            diagnostics.push(error);
        }
    }
    for child in children(node) {
        check_node(child, path, diagnostics, matches!(node.kind, NodeKind::Fn { .. }) && child.span == node.span);
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
            if matches!(&pattern.kind, PatternKind::Name(name) if !crate::names::is_upper_reference(name))
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
                line: pat.span.line,
                col: pat.span.col,
                code: Some("E504".into()),
                message: "use do to discard a success value".into(),
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
    Ok(())
}

pub(crate) fn adt_coverage(
    branch_span: crate::parser::Span,
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
        (false, None) if missing => ("E402", branch_span, "not all constructor values are covered"),
        (true, Some(body)) => (
            "E403",
            body.span,
            "fallback is unreachable after all constructors",
        ),
        _ => return Ok(()),
    };
    Err(Diagnostic {
        path: path.into(),
        line: location.line,
        col: location.col,
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
                && matches!(&args[0].kind, PatternKind::Name(bound) if !crate::names::is_upper_reference(bound)))
            && crate::normalize::rebuilds_pattern(pattern, body)
        {
            return Err(Diagnostic {
                path: path.into(),
                line: body.span.line,
                col: body.span.col,
                code: Some("E508".into()),
                message: format!(
                    "failure bound at {}:{} is propagated unchanged; use let p? = or do",
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
            PatternKind::Name(name) if !crate::names::is_upper_reference(name)))
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
        PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => fields
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
    if matches!(&node.kind, NodeKind::Name(name) if matches!(name.as_str(), "Nil" | "Cons")) {
        return Err(Diagnostic {
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            code: Some("E510".into()),
            message: "use List literal syntax instead of Nil or Cons".into(),
        });
    }
    Ok(())
}

pub(crate) fn children(node: &Node) -> Vec<&Node> {
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
        NodeKind::List(items, tail) => items.iter().chain(tail.iter().map(Box::as_ref)).collect(),
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => fields.iter().map(|(_, value)| value).collect(),
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
        NodeKind::With { args, body, .. } => args.iter().chain(std::iter::once(body.as_ref())).collect(),
        _ => vec![],
    }
}
