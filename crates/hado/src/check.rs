use crate::parser::{
    Ast, DefKind, Diagnostic, Item, Node, NodeKind, Pattern, PatternKind, TypeExpr, TypeKind,
};

pub fn check_ast(ast: &Ast, path: &str) -> Result<(), Diagnostic> {
    match check_ast_all(ast, path).into_iter().next() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// §56・§58.4（L1）: この段階の診断を**全件**、アンカー順に返す。`check_ast` は
/// 従来どおり先頭の 1 件だけを返すので、既定の CLI と拒否コーパスの判定は変わらない。
pub(crate) fn check_ast_all(ast: &Ast, path: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for span in &ast.long_condition_functions {
        diagnostics.push(Diagnostic {
            related: None,
            path: path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: Some("E517".into()),
            message: "write condition arms directly after the function parameters".into(),
        });
    }
    for span in &ast.written_list_patterns {
        diagnostics.push(Diagnostic {
            related: None,
            path: path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: Some("E516".into()),
            message: "use List pattern syntax instead of Nil or Cons".into(),
        });
    }
    for span in &ast.branches_after_fallback {
        diagnostics.push(Diagnostic {
            related: None,
            path: path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: Some("E404".into()),
            message: "fallback must be the last branch".into(),
        });
    }
    if let Err(error) = crate::names::check_cycles(ast, path) {
        diagnostics.push(error);
    }
    check_items(ast, &ast.items, &ast.items, None, path, &mut diagnostics);
    let mut diagnostics: Vec<_> = diagnostics
        .into_iter()
        .map(|error| arm_diagnostic(ast, error))
        .collect();
    diagnostics.sort_by_key(crate::diagnostic_order);
    diagnostics
}

pub(crate) fn arm_diagnostic(ast: &Ast, mut error: Diagnostic) -> Diagnostic {
    if matches!(
        error.code.as_deref(),
        Some("E403" | "E405" | "E406" | "E508")
    ) {
        if let Some((_, pipe)) = ast
            .branch_anchors
            .iter()
            .find(|(source, _)| source.line == error.line && source.col == error.col)
        {
            // The anchor moves to the `|`, and the range moves with it: §58.4
            // の終端はアンカーの要素の終端なので、`|` 1 文字分になる。
            let span = pipe.until(*pipe);
            error.set_span(span);
        }
    }
    qualified_type_diagnostic(ast, error)
}

fn qualified_type_diagnostic(ast: &Ast, mut error: Diagnostic) -> Diagnostic {
    if error.code.as_deref() != Some("E204") {
        return error;
    }
    let member = ast
        .qualified_type_anchors
        .iter()
        .find_map(|(module, member)| {
            let in_contract = ast.items.iter().any(|item| match item {
                Item::ModuleSig(signature) => signature.items.iter().any(|item| match item {
                    Item::SigDef(def) => def
                        .params
                        .iter()
                        .filter_map(|param| param.ty.as_ref())
                        .chain(def.ret.iter())
                        .any(|ty| type_has_span(ty, module.span)),
                    _ => false,
                }),
                _ => false,
            });
            let known = ast.items.iter().any(|item| match item {
                Item::ModuleSig(signature) if in_contract => signature.name == module.name,
                Item::ModuleImpl(implementation) if !in_contract => {
                    implementation.name == module.name
                        && !matches!(
                            implementation.form,
                            crate::parser::ImplementationForm::Functor(_)
                        )
                        || (!in_contract
                            && known_parameter_type_scope(
                                implementation,
                                &module.name,
                                module.span,
                                &ast.items,
                            ))
                }
                _ => false,
            });
            (module.span.line == error.line && module.span.col == error.col && known)
                .then_some(*member)
        });
    if let Some(member) = member {
        error.line = member.line;
        error.col = member.col;
    }
    error
}

fn known_parameter_type_scope(
    module: &crate::parser::ModuleImpl,
    name: &str,
    span: crate::parser::Span,
    items: &[Item],
) -> bool {
    let crate::parser::ImplementationForm::Functor(parameters) = &module.form else {
        return false;
    };
    parameters.iter().any(|parameter| {
        parameter.name == name && items.iter().any(|item|
        matches!(item, Item::ModuleSig(signature) if signature.name == parameter.contract))
    }) && module
        .items
        .iter()
        .any(|item| item_type_has_span(item, span))
}

fn item_type_has_span(item: &Item, span: crate::parser::Span) -> bool {
    match item {
        Item::TypeDef(definition) => definition
            .ty
            .as_ref()
            .is_some_and(|ty| type_has_span(ty, span)),
        Item::Def(definition) => match &definition.kind {
            DefKind::Value(body) => node_type_has_span(body, span),
            DefKind::Function { params, ret, body } => {
                params
                    .iter()
                    .filter_map(|param| param.ty.as_ref())
                    .chain(ret.iter())
                    .any(|ty| type_has_span(ty, span))
                    || node_type_has_span(body, span)
            }
        },
        _ => false,
    }
}

fn node_type_has_span(node: &Node, span: crate::parser::Span) -> bool {
    if let NodeKind::Fn { params, .. } = &node.kind {
        if params
            .iter()
            .filter_map(|param| param.ty.as_ref())
            .any(|ty| type_has_span(ty, span))
        {
            return true;
        }
    }
    children(node)
        .iter()
        .any(|child| node_type_has_span(child, span))
}

/// Whether `ty` is the type written at `span`. §58.4 で型の範囲は最後のトークン
/// まで伸びたので、同じ位置かを問うところは始まりで比べる（`M.T` の型は `M` から
/// 始まるが、範囲は `T` まで及ぶ）。
fn type_has_span(ty: &crate::parser::TypeExpr, span: crate::parser::Span) -> bool {
    if same_start(ty.span, span) {
        return true;
    }
    if let Some(crate::parser::EffectRow::Row(effects)) = &ty.effects {
        if effects.iter().any(|ty| type_has_span(ty, span)) {
            return true;
        }
    }
    match &ty.kind {
        crate::parser::TypeKind::Adt(variants) => variants.iter().any(|variant| {
            same_start(variant.name.span, span)
                || variant.payload.types().any(|ty| type_has_span(ty, span))
        }),
        crate::parser::TypeKind::Apply(_, args) | crate::parser::TypeKind::Group(args) => {
            args.iter().any(|ty| type_has_span(ty, span))
        }
        crate::parser::TypeKind::Record(fields) => {
            fields.iter().any(|field| type_has_span(&field.ty, span))
        }
        _ => false,
    }
}

/// Two spans that begin at the same line and column. A span that gained an end
/// (§58.4) still names the same element as the point span it came from, so the
/// tables that look a position up by its start keep working.
pub(crate) fn same_start(left: crate::parser::Span, right: crate::parser::Span) -> bool {
    left.line == right.line && left.col == right.col
}

fn check_items(
    ast: &Ast,
    items: &[Item],
    contracts: &[Item],
    members: Option<&[Item]>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in items {
        check_type_parameters(item, path, diagnostics);
        match item {
            Item::Def(def) => check_definition(
                ast,
                def,
                is_contract_member(members, &def.name),
                path,
                diagnostics,
            ),
            Item::SigDef(def) => {
                for ty in def
                    .params
                    .iter()
                    .filter_map(|param| param.ty.as_ref())
                    .chain(def.ret.iter())
                {
                    check_effect_rows(ty, path, diagnostics);
                }
            }
            Item::ModuleImpl(module) => check_items(
                ast,
                &module.items,
                contracts,
                contract_members(contracts, &module.sig),
                path,
                diagnostics,
            ),
            Item::ModuleSig(signature) => {
                check_items(ast, &signature.items, contracts, None, path, diagnostics)
            }
            Item::Effect(effect) => {
                for op in &effect.ops {
                    for ty in op
                        .params
                        .iter()
                        .filter_map(|param| param.ty.as_ref())
                        .chain(op.ret.iter())
                    {
                        check_effect_rows(ty, path, diagnostics);
                    }
                }
            }
            Item::Handler(handler) => {
                for ty in handler.params.iter().filter_map(|param| param.ty.as_ref()) {
                    check_effect_rows(ty, path, diagnostics);
                }
            }
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
    members.is_some_and(|items| {
        items
            .iter()
            .any(|item| matches!(item, Item::SigDef(member) if member.name == name))
    })
}

fn check_definition(
    ast: &Ast,
    def: &crate::parser::Def,
    contract_member: bool,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &def.kind {
        DefKind::Value(body) => check_node(ast, body, path, diagnostics, false),
        DefKind::Function { params, ret, body } => {
            if !contract_member {
                for param in params.iter().filter(|param| param.name == "_") {
                    diagnostics.push(Diagnostic {
                        related: None,
                        path: path.into(),
                        line: param.span.line,
                        col: param.span.col,
                        end_line: param.span.line,
                        end_col: param.span.col,
                        code: Some("E519".into()),
                        message:
                            "underscore parameters require a contract member or anonymous function"
                                .into(),
                    });
                }
            }
            for ty in params
                .iter()
                .filter_map(|param| param.ty.as_ref())
                .chain(ret.iter())
            {
                check_effect_rows(ty, path, diagnostics);
            }
            check_unused(
                params.iter().map(|param| (param.name.as_str(), param.span)),
                body,
                path,
                diagnostics,
            );
            if let Err(error) =
                check_sole_parameter_branch(params, ret.as_ref(), body, def.span, path)
            {
                diagnostics.push(error);
            }
            // §32: `def f | ...` の腕は定義と同じ位置から始まる。範囲が閉じ `)`
            // まで伸びても、直接の位置かは始まりで決まる。
            let at_start = body.span.line == def.span.line && body.span.col == def.span.col;
            check_node(ast, body, path, diagnostics, at_start);
        }
    }
}

fn check_effect_rows(ty: &TypeExpr, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(crate::parser::EffectRow::Row(effects)) = &ty.effects {
        let mut seen = std::collections::BTreeSet::new();
        for effect in effects {
            if !seen.insert(effect_key(effect)) {
                diagnostics.push(duplicate_effect(effect, path));
            }
            check_effect_rows(effect, path, diagnostics);
        }
    }
    for child in type_children(ty) {
        check_effect_rows(child, path, diagnostics);
    }
}

/// E522: 同じ効果が 1 つの row に 2 度書かれている。
fn duplicate_effect(effect: &TypeExpr, path: &str) -> Diagnostic {
    Diagnostic {
        related: None,
        path: path.into(),
        line: effect.name_span.line,
        col: effect.name_span.col,
        end_line: effect.name_span.line,
        end_col: effect.name_span.col,
        code: Some("E522".into()),
        message: "duplicate effect in row".into(),
    }
}

/// 効果の row をさらに見るために降りる先。型の形ごとの子を返す。
fn type_children(ty: &TypeExpr) -> Vec<&TypeExpr> {
    match &ty.kind {
        TypeKind::Apply(_, args) | TypeKind::Group(args) => args.iter().collect(),
        TypeKind::Record(fields) => fields.iter().map(|field| &field.ty).collect(),
        TypeKind::Adt(variants) => variants
            .iter()
            .flat_map(|variant| variant.payload.types())
            .collect(),
        TypeKind::Name(_) => Vec::new(),
    }
}

fn effect_key(ty: &TypeExpr) -> String {
    match &ty.kind {
        TypeKind::Name(name) => name.clone(),
        TypeKind::Apply(name, args) => match args.as_slice() {
            [] => name.clone(),
            [argument] => format!("{} {name}", effect_key(argument)),
            _ => format!(
                "({}) {name}",
                args.iter().map(effect_key).collect::<Vec<_>>().join(", ")
            ),
        },
        TypeKind::Group(args) => format!(
            "({})",
            args.iter().map(effect_key).collect::<Vec<_>>().join(", ")
        ),
        _ => String::new(),
    }
}

fn check_type_parameters(item: &Item, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (parameters, types): (&[_], Vec<&TypeExpr>) = match item {
        Item::TypeDef(def) if def.ty.is_some() => (&def.params, def.ty.iter().collect()),
        Item::SigDef(def) => (
            &def.type_params,
            def.params
                .iter()
                .filter_map(|param| param.ty.as_ref())
                .chain(def.ret.iter())
                .collect(),
        ),
        _ => return,
    };
    for parameter in parameters {
        if types
            .iter()
            .any(|ty| references_type_parameter(ty, &parameter.name))
        {
            continue;
        }
        diagnostics.push(Diagnostic {
            related: None,
            path: path.into(),
            line: parameter.span.line,
            col: parameter.span.col,
            end_line: parameter.span.line,
            end_col: parameter.span.col,
            code: Some("E518".into()),
            message: format!("type parameter {} is unused", parameter.name),
        });
    }
}

fn references_type_parameter(ty: &TypeExpr, parameter: &str) -> bool {
    let in_kind = match &ty.kind {
        TypeKind::Name(name) => name == parameter,
        TypeKind::Apply(name, arguments) => {
            name == parameter
                || arguments
                    .iter()
                    .any(|ty| references_type_parameter(ty, parameter))
        }
        TypeKind::Group(arguments) => arguments
            .iter()
            .any(|ty| references_type_parameter(ty, parameter)),
        TypeKind::Record(fields) => fields
            .iter()
            .any(|field| references_type_parameter(&field.ty, parameter)),
        TypeKind::Adt(variants) => variants
            .iter()
            .any(|variant| constructor_uses_type_parameter(variant, parameter)),
    };
    in_kind
        || matches!(&ty.effects, Some(crate::parser::EffectRow::Row(effects)) if effects.iter().any(|ty| references_type_parameter(ty, parameter)))
}

fn constructor_uses_type_parameter(
    variant: &crate::parser::ConstructorDecl,
    parameter: &str,
) -> bool {
    variant
        .payload
        .types()
        .any(|ty| references_type_parameter(ty, parameter))
}

fn check_unused<'a>(
    bindings: impl IntoIterator<Item = (&'a str, crate::parser::Span)>,
    body: &Node,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (name, span) in bindings {
        if matches!(name, "$argument" | "_") || crate::names::references(body, name) {
            continue;
        }
        diagnostics.push(Diagnostic {
            related: None,
            path: path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: Some("E505".into()),
            message: format!("binding {name} is unused"),
        });
    }
}

/// `def f x => if x | ...` where the sole parameter is only scrutinized:
/// the canonical form is `def f | ...` (§31, E515). The checker and the
/// fix-it (§57) share this predicate so a rewrite cannot drift from the code.
pub(crate) fn sole_parameter_branch(
    params: &[crate::parser::Param],
    ret: Option<&crate::parser::TypeExpr>,
    body: &Node,
) -> bool {
    let [parameter] = params else {
        return false;
    };
    if parameter.ty.is_some() || ret.is_some() || parameter.name == "$argument" {
        return false;
    }
    let NodeKind::PatternBranch {
        scrutinees,
        arms,
        fallback,
    } = &body.kind
    else {
        return false;
    };
    let [scrutinee] = scrutinees.as_slice() else {
        return false;
    };
    if !matches!(&scrutinee.kind, NodeKind::Name(name) if name == &parameter.name) {
        return false;
    }
    let referenced_in_arm = arms.iter().any(|(patterns, body)| {
        let bound: std::collections::BTreeSet<_> = patterns
            .iter()
            .flat_map(crate::names::bound_names)
            .collect();
        !bound.contains(&parameter.name) && crate::names::references(body, &parameter.name)
    });
    !(referenced_in_arm
        || fallback
            .as_ref()
            .is_some_and(|body| crate::names::references(body, &parameter.name)))
}

fn check_sole_parameter_branch(
    params: &[crate::parser::Param],
    ret: Option<&crate::parser::TypeExpr>,
    body: &Node,
    span: crate::parser::Span,
    path: &str,
) -> Result<(), Diagnostic> {
    if !sole_parameter_branch(params, ret, body) {
        return Ok(());
    }
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: span.line,
        col: span.col,
        end_line: span.line,
        end_col: span.col,
        code: Some("E515".into()),
        message: "use pattern arms directly for the sole parameter".into(),
    })
}

pub(crate) fn matrix_coverage(
    branch_span: crate::parser::Span,
    rows: &[Vec<Pattern>],
    fallback: Option<&Node>,
    families: &crate::coverage::Families,
    path: &str,
) -> Result<(), Diagnostic> {
    let (code, location, message) = match (crate::coverage::covers_rows(rows, families), fallback) {
        (Some(false), None) => (
            "E402",
            branch_span,
            "not all constructor values are covered",
        ),
        (Some(true), Some(body)) => (
            "E403",
            body.span,
            "fallback is unreachable after all constructors",
        ),
        _ => return Ok(()),
    };
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: location.line,
        col: location.col,
        end_line: location.line,
        end_col: location.col,
        code: Some(code.into()),
        message: message.into(),
    })
}

/// §24: a Boolean `if` written as pattern arms, or a single condition arm.
/// §58.4: the anchor is the branch itself, so the range runs from its start to
/// its last arm; `node.span` may reach further (the next `|`, the enclosing `)`),
/// so the extent the parser recorded for the branch is used instead.
fn check_boolean_form(
    ast: &Ast,
    node: &Node,
    path: &str,
    direct_condition: bool,
) -> Result<(), Diagnostic> {
    let redundant = match &node.kind {
        NodeKind::PatternBranch { scrutinees, arms, .. } => {
            scrutinees.len() == 1
                && !arms.is_empty()
                && arms.iter().all(|(patterns, _)| {
                    matches!(
                        patterns.as_slice(),
                        [pattern]
                            if matches!(&pattern.kind, PatternKind::Literal(value) if matches!(value.kind, NodeKind::Bool(_)))
                    )
                })
        }
        NodeKind::CondBranch { arms, .. } => arms.len() == 1 && !direct_condition,
        _ => false,
    };
    if !redundant {
        return Ok(());
    }
    let span = branch_extent(ast, node).unwrap_or(node.span);
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: span.line,
        col: span.col,
        end_line: span.end_line,
        end_col: span.end_col,
        code: Some("E520".into()),
        message: "use a Boolean branch instead of Boolean pattern arms or a single condition arm"
            .into(),
    })
}

fn check_node(
    ast: &Ast,
    node: &Node,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    direct_condition: bool,
) {
    if let Err(error) = check_boolean_form(ast, node, path, direct_condition) {
        diagnostics.push(error);
    }
    check_fn_node(node, path, diagnostics);
    check_bindings(node, path, diagnostics);
    for check in NODE_CHECKS {
        if let Err(error) = check(node, path) {
            diagnostics.push(error);
        }
    }
    for check in BRANCH_CHECKS {
        if let Err(error) = check(ast, node, path) {
            diagnostics.push(error);
        }
    }
    for child in children(node) {
        check_node(
            ast,
            child,
            path,
            diagnostics,
            condition_is_direct(node, child),
        );
    }
}

/// ノード 1 つを見る検査。
type NodeCheck = fn(&Node, &str) -> Result<(), Diagnostic>;

/// ノードごとの検査。順序は診断の出る順に効くので、並びを変えない。
/// `check_rewrap` だけは枝の範囲を要するので `NODE_CHECKS` の外で走る。
const NODE_CHECKS: [NodeCheck; 6] = [
    check_cons_tail,
    check_duplicate_patterns,
    check_conditions,
    check_literal_fallback,
    check_local_redundancy,
    check_repropagation,
];

/// 枝の範囲を見る検査。順序は `NODE_CHECKS` の `check_local_redundancy` の直後。
type BranchCheck = fn(&Ast, &Node, &str) -> Result<(), Diagnostic>;

const BRANCH_CHECKS: [BranchCheck; 1] = [check_rewrap];

/// `fn` の引数についての検査。使われない束縛・腕の形・型に載る効果を見る。
fn check_fn_node(node: &Node, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    let NodeKind::Fn { params, body } = &node.kind else {
        return;
    };
    check_unused(
        params.iter().map(|param| (param.name.as_str(), param.span)),
        body,
        path,
        diagnostics,
    );
    if let Err(error) = check_sole_parameter_branch(params, None, body, node.span, path) {
        diagnostics.push(error);
    }
    for ty in params.iter().filter_map(|param| param.ty.as_ref()) {
        check_effect_rows(ty, path, diagnostics);
    }
}

/// 束縛を導入するノードで、その束縛が本文で使われているかを見る。
fn check_bindings(node: &Node, path: &str, diagnostics: &mut Vec<Diagnostic>) {
    match &node.kind {
        NodeKind::Let { pat, body, .. } | NodeKind::LetQ { pat, body, .. } => {
            check_unused(crate::names::bindings(pat), body, path, diagnostics);
        }
        NodeKind::PatternBranch { arms, .. } => {
            for (patterns, body) in arms {
                for pattern in patterns {
                    check_unused(crate::names::bindings(pattern), body, path, diagnostics);
                }
            }
        }
        _ => {}
    }
}

/// 子を条件の直接の位置として見るか。`fn` の本体と、`case` の腕の `if` が
/// 直接の位置にあたる。§58.4 で `fn` の範囲は閉じ `)` まで伸びたが、本体が
/// 直接の位置かは始まりで決まる（`fn(| ...)` の腕は `fn` と同じ位置から始まる）。
fn condition_is_direct(node: &Node, child: &Node) -> bool {
    let arm_condition = matches!(node.kind, NodeKind::PatternBranch { .. })
        && matches!(child.kind, NodeKind::CondBranch { .. });
    let fn_body_at_start = matches!(node.kind, NodeKind::Fn { .. })
        && child.span.line == node.span.line
        && child.span.col == node.span.col;
    arm_condition || fn_body_at_start
}

fn check_local_redundancy(node: &Node, path: &str) -> Result<(), Diagnostic> {
    for check in [
        identical_branches,
        catch_all_rows,
        discarded_binding,
        returned_binding,
    ] {
        check(node, path)?;
    }
    Ok(())
}

/// E502: `if` の 2 つの腕が同じ式。
fn identical_branches(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::BoolBranch { then, els, .. } = &node.kind else {
        return Ok(());
    };
    if !crate::normalize::same_expression(then, els) {
        return Ok(());
    }
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: node.span.line,
        col: node.span.col,
        end_line: node.span.line,
        end_col: node.span.col,
        code: Some("E502".into()),
        message: format!(
            "identical branch results at {}:{} and {}:{}",
            then.span.line, then.span.col, els.span.line, els.span.col
        ),
    })
}

/// `case` の腕の pattern の並びについての検査。
fn catch_all_rows(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::PatternBranch { arms, .. } = &node.kind else {
        return Ok(());
    };
    for (patterns, _) in arms {
        catch_all_row(patterns, path)?;
    }
    Ok(())
}

/// E504: `let _? =` は `do` を使うべき位置。
fn discarded_binding(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::LetQ { pat, .. } = &node.kind else {
        return Ok(());
    };
    if !matches!(pat.kind, PatternKind::Underscore) {
        return Ok(());
    }
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: pat.span.line,
        col: pat.span.col,
        end_line: pat.span.line,
        end_col: pat.span.col,
        code: Some("E504".into()),
        message: "use do to discard a success value".into(),
    })
}

/// E506: `let _ =` は値を捨てられない。E501: `let x = ... x` は束縛が素通し。
fn returned_binding(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::Let { pat, body, .. } = &node.kind else {
        return Ok(());
    };
    if matches!(pat.kind, PatternKind::Underscore) {
        return Err(Diagnostic {
            related: None,
            path: path.into(),
            line: pat.span.line,
            col: pat.span.col,
            end_line: pat.span.line,
            end_col: pat.span.col,
            code: Some("E506".into()),
            message: "a plain let binding cannot discard its value".into(),
        });
    }
    let (PatternKind::Name(bound), NodeKind::Name(returned)) = (&pat.kind, &body.kind) else {
        return Ok(());
    };
    if bound != returned {
        return Ok(());
    }
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: node.span.line,
        col: node.span.col,
        end_line: node.span.line,
        end_col: node.span.col,
        code: Some("E501".into()),
        message: format!(
            "binding {bound} is returned unchanged at {}:{}",
            body.span.line, body.span.col
        ),
    })
}

fn is_wildcard_pattern(pattern: &Pattern) -> bool {
    matches!(pattern.kind, PatternKind::Underscore)
}

fn is_variable_pattern(pattern: &Pattern) -> bool {
    matches!(&pattern.kind, PatternKind::Name(name) if !crate::names::is_upper_reference(name))
}

fn catch_all_row(patterns: &[Pattern], path: &str) -> Result<(), Diagnostic> {
    if !patterns
        .iter()
        .all(|pattern| is_wildcard_pattern(pattern) || is_variable_pattern(pattern))
    {
        return Ok(());
    }
    let (code, pattern, message) = if patterns.iter().all(is_wildcard_pattern) {
        let Some(pattern) = patterns.first() else {
            return Ok(());
        };
        (
            "E503",
            pattern,
            "use a fallback instead of a catch-all underscore pattern",
        )
    } else {
        let Some(pattern) = patterns.iter().find(|pattern| is_variable_pattern(pattern)) else {
            return Ok(());
        };
        ("E512", pattern, "top-level variable patterns are forbidden")
    };
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: pattern.span.line,
        col: pattern.span.col,
        end_line: pattern.span.line,
        end_col: pattern.span.col,
        code: Some(code.into()),
        message: message.into(),
    })
}

/// §36 / §57: the single `None` arm of a single-scrutinee branch rebuilds the
/// absence it binds. The checker (E508) and the local rewrite (§57) share this
/// predicate, so a rewrite cannot drift from the reported code.
pub(crate) fn repropagated_arm(node: &Node) -> Option<(&Pattern, &Node)> {
    let NodeKind::PatternBranch {
        scrutinees, arms, ..
    } = &node.kind
    else {
        return None;
    };
    if scrutinees.len() != 1 {
        return None;
    }
    for (patterns, body) in arms {
        let [pattern] = patterns.as_slice() else {
            continue;
        };
        if matches!(&pattern.kind, PatternKind::Name(name) if name == "None")
            && crate::normalize::rebuilds_pattern(pattern, body)
        {
            return Some((pattern, body));
        }
    }
    None
}

fn check_repropagation(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let Some((pattern, body)) = repropagated_arm(node) else {
        return Ok(());
    };
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: body.span.line,
        col: body.span.col,
        end_line: body.span.line,
        end_col: body.span.col,
        code: Some("E508".into()),
        message: format!(
            "absence bound at {}:{} is propagated unchanged; use let p? = or do",
            pattern.span.line, pattern.span.col
        ),
    })
}

/// §45 / §57: every arm of a single-scrutinee branch rebuilds its own pattern,
/// and any fallback is the scrutinee unchanged. The checker (E507) and the local
/// rewrite (§57) share this predicate, so a rewrite cannot drift from the code.
pub(crate) fn rewrap_branch(node: &Node) -> Option<&Node> {
    let NodeKind::PatternBranch {
        scrutinees,
        arms,
        fallback,
    } = &node.kind
    else {
        return None;
    };
    let [scrutinee] = scrutinees.as_slice() else {
        return None;
    };
    let fallback_unchanged = fallback
        .as_ref()
        .is_none_or(|body| crate::normalize::same_expression(scrutinee, body));
    if !arms.is_empty()
        && arms.iter().all(|(patterns, _)| matches!(patterns.as_slice(), [pattern]
            if !matches!(&pattern.kind, PatternKind::Underscore)
                && !matches!(&pattern.kind, PatternKind::Name(name) if !crate::names::is_upper_reference(name))))
        && fallback_unchanged
        && arms.iter().all(|(patterns, body)| {
            matches!(patterns.as_slice(), [pattern] if crate::normalize::rebuilds_pattern(pattern, body))
        })
    {
        return Some(scrutinee);
    }
    None
}

/// The extent of a branch node as the parser recorded it (§57 keeps the whole
/// branch there for the local rewrites), when one is recorded.
fn branch_extent(ast: &Ast, node: &Node) -> Option<crate::parser::Span> {
    ast.branch_spans
        .iter()
        .find(|(start, _, _)| start.line == node.span.line && start.col == node.span.col)
        .map(|(_, _, end)| node.span.until(*end))
}

/// §45, §58.4: the anchor is the whole `if`, so the range runs from the `if`
/// to the last arm. §57 rewrites that same extent to the scrutinee.
fn check_rewrap(ast: &Ast, node: &Node, path: &str) -> Result<(), Diagnostic> {
    let Some(scrutinee) = rewrap_branch(node) else {
        return Ok(());
    };
    let span = branch_extent(ast, node).unwrap_or(node.span);
    Err(Diagnostic {
        related: None,
        path: path.into(),
        line: span.line,
        col: span.col,
        end_line: span.end_line,
        end_col: span.end_col,
        code: Some("E507".into()),
        message: format!(
            "branches reconstruct the value at {}:{}",
            scrutinee.span.line, scrutinee.span.col
        ),
    })
}

fn check_literal_fallback(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::PatternBranch { arms, fallback, .. } = &node.kind else {
        return Ok(());
    };
    if fallback.is_none()
        && arms
            .iter()
            .any(|(patterns, _)| patterns.iter().any(needs_literal_fallback))
    {
        return Err(Diagnostic {
            related: None,
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            end_line: node.span.line,
            end_col: node.span.col,
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
            related: None,
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            end_line: node.span.line,
            end_col: node.span.col,
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
                related: None,
                path: path.into(),
                line: condition.span.line,
                col: condition.span.col,
                end_line: condition.span.line,
                end_col: condition.span.col,
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
                related: None,
                path: path.into(),
                line: node.span.line,
                col: node.span.col,
                end_line: node.span.line,
                end_col: node.span.col,
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

fn covers_row(previous: &[Pattern], next: &[Pattern]) -> bool {
    previous.len() == next.len()
        && previous
            .iter()
            .zip(next)
            .all(|(left, right)| crate::normalize::covers_pattern(left, right))
}

fn same_row(previous: &[Pattern], next: &[Pattern]) -> bool {
    previous.len() == next.len()
        && previous
            .iter()
            .zip(next)
            .all(|(left, right)| crate::normalize::same_pattern(left, right))
}

fn check_duplicate_patterns(node: &Node, path: &str) -> Result<(), Diagnostic> {
    let NodeKind::PatternBranch { arms, .. } = &node.kind else {
        return Ok(());
    };
    for (index, (patterns, _)) in arms.iter().enumerate() {
        if let Some(previous) = arms[..index]
            .iter()
            .find_map(|(previous, _)| covers_row(previous, patterns).then_some(previous))
        {
            let Some(pattern) = patterns.first() else {
                continue;
            };
            let Some(previous_pattern) = previous.first() else {
                continue;
            };
            return Err(Diagnostic {
                related: None,
                path: path.into(),
                line: pattern.span.line,
                col: pattern.span.col,
                end_line: pattern.span.line,
                end_col: pattern.span.col,
                code: Some(
                    if same_row(previous, patterns) {
                        "E405"
                    } else {
                        "E403"
                    }
                    .into(),
                ),
                message: format!(
                    "pattern is covered by the arm at {}:{}",
                    previous_pattern.span.line, previous_pattern.span.col
                ),
            });
        }
    }
    Ok(())
}

fn check_cons_tail(node: &Node, path: &str) -> Result<(), Diagnostic> {
    if matches!(&node.kind, NodeKind::Name(name) if matches!(name.as_str(), "Nil" | "Cons")) {
        return Err(Diagnostic {
            related: None,
            path: path.into(),
            line: node.span.line,
            col: node.span.col,
            end_line: node.span.line,
            end_col: node.span.col,
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
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
            fields.iter().map(|(_, value)| value).collect()
        }
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
            scrutinees,
            arms,
            fallback,
        } => scrutinees
            .iter()
            .chain(arms.iter().map(|(_, body)| body))
            .chain(fallback.iter().map(Box::as_ref))
            .collect(),
        NodeKind::With { args, body, .. } => {
            args.iter().chain(std::iter::once(body.as_ref())).collect()
        }
        _ => vec![],
    }
}
