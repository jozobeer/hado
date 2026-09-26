//! §57・§58.1 の正準形の印字器。
//!
//! `e as p ->`・`e ->` の束縛と、`if` の閉じ（本体に `->` があれば `/if`、
//! 無ければ続きがあるときだけ `if(...)`）で、構文木を 1 通りに綴る。
//!
//! 綴りは構文木から決まる。`if` の本体（腕と fallback）に束縛が 1 つでも
//! あれば `/if` で閉じ、無ければ「続きがある位置」では `if(...)`、末尾では
//! 閉じない。`fn` も同じで、本体に束縛があれば `fn x => … /fn`、無ければ
//! 今の `fn(x => …)` のまま。`module` と `implement` は `/module`・
//! `/implement` で閉じ、`def` は閉じない。

use super::source_print::{literal, parameters, return_type, type_parameters, type_source};
use super::*;

/// 1 ファイル分の綴り。`library` 見出しのあとに項目を並べる。
pub fn program(ast: &Ast) -> String {
    let mut out = String::new();
    if ast.library.is_some() {
        out.push_str("library\n");
    }
    out.push_str(&items_text(&ast.items));
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// 式 1 つを、末尾の位置として 1 行で綴る。正準形の案内（`canonical form:`）に使う。
pub fn node_text(node: &Node) -> String {
    expr(node, 2, true).replace('\n', " ")
}

/// 構文木の中に `->` の束縛（`Let`・`LetQ`・`DoQ`）があるか。
pub(crate) fn contains_continuation(node: &Node) -> bool {
    match &node.kind {
        NodeKind::Let { .. } | NodeKind::LetQ { .. } | NodeKind::DoQ { .. } => true,
        NodeKind::Neg(c) | NodeKind::Not(c) => contains_continuation(c),
        NodeKind::Bin(_, l, r) | NodeKind::Pipe(l, r) => {
            contains_continuation(l) || contains_continuation(r)
        }
        NodeKind::Apply(f, args) | NodeKind::Call(f, args) => {
            contains_continuation(f) || args.iter().any(contains_continuation)
        }
        NodeKind::Field(l, _) => contains_continuation(l),
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
            fields.iter().any(|(_, value)| contains_continuation(value))
        }
        NodeKind::List(items, rest) => {
            items.iter().any(contains_continuation)
                || rest.as_deref().is_some_and(contains_continuation)
        }
        NodeKind::Fn { body, .. } | NodeKind::With { body, .. } => contains_continuation(body),
        NodeKind::BoolBranch { then, els, .. } => {
            contains_continuation(then) || contains_continuation(els)
        }
        NodeKind::CondBranch { arms, fallback } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        NodeKind::PatternBranch { arms, fallback, .. } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        _ => false,
    }
}

/// `if` の本体（腕と fallback）に `->` があるか。Issue の規則の判定そのもの。
pub(crate) fn branch_has_arrow(node: &Node) -> bool {
    match &node.kind {
        NodeKind::BoolBranch { then, els, .. } => {
            contains_continuation(then) || contains_continuation(els)
        }
        NodeKind::CondBranch { arms, fallback } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        NodeKind::PatternBranch { arms, fallback, .. } => {
            arms.iter().any(|(_, body)| contains_continuation(body))
                || fallback.as_deref().is_some_and(contains_continuation)
        }
        _ => contains_continuation(node),
    }
}

fn items_text(items: &[Item]) -> String {
    items.iter().map(item_text).collect::<Vec<_>>().join("\n")
}

pub(crate) fn item_text(item: &Item) -> String {
    match item {
        Item::Def(def) => def_text(def),
        Item::SigDef(def) => format!(
            "def {}{}{}{}",
            type_parameters(&def.type_params),
            def.name,
            parameters(&def.params),
            return_type(def.ret.as_ref())
        ),
        Item::TypeDef(def) => {
            let body = def
                .ty
                .as_ref()
                .map(|ty| format!(" = {}", type_source(ty)))
                .unwrap_or_default();
            let shared = if def.shared { "shared " } else { "" };
            format!(
                "{shared}type {}{}{body}",
                type_parameters(&def.params),
                def.name
            )
        }
        Item::ModuleSig(module) => format!(
            "module {}\n{}\n/module",
            module.name,
            items_text(&module.items)
        ),
        Item::ModuleImpl(module) => implementation_text(module),
        Item::Effect(effect) => effect_text(effect),
        Item::Handler(handler) => handler_text(handler),
    }
}

fn def_text(def: &Def) -> String {
    let rec = if def.rec { "rec " } else { "" };
    let tail = match &def.kind {
        DefKind::Value(body) => format!(" = {}", expr(body, 2, true)),
        DefKind::Function { params, ret, body } => {
            if let NodeKind::CondBranch { arms, fallback } = &body.kind {
                format!(
                    "{}{}{}",
                    parameters(params),
                    return_type(ret.as_ref()),
                    cond_arms(arms, fallback.as_deref(), 2, true)
                )
            } else if is_arm_function(params, body) {
                arm_arms(body, 2, true)
            } else {
                format!(
                    "{}{} => {}",
                    parameters(params),
                    return_type(ret.as_ref()),
                    expr(body, 2, true)
                )
            }
        }
    };
    format!("def {rec}{}{tail}", def.name)
}

fn is_arm_function(params: &[Param], body: &Node) -> bool {
    params.len() == 1
        && params[0].name == "$argument"
        && matches!(body.kind, NodeKind::PatternBranch { .. })
}

/// `def f | …` と `fn(| …)` の腕の並び。scrutinee は書かない。
fn arm_arms(body: &Node, indent: usize, tail: bool) -> String {
    let NodeKind::PatternBranch { arms, fallback, .. } = &body.kind else {
        return expr(body, indent, tail);
    };
    pattern_arms(arms, fallback.as_deref(), indent, tail)
}

fn pattern_arms(
    arms: &[(Vec<Pattern>, Node)],
    fallback: Option<&Node>,
    indent: usize,
    tail: bool,
) -> String {
    let mut out = String::new();
    for (patterns, value) in arms {
        out.push_str(&format!(
            " | {} => {}",
            comma_patterns(patterns),
            arm_value(value, indent, tail)
        ));
    }
    if let Some(fallback) = fallback {
        out.push_str(&format!(" | {}", arm_value(fallback, indent, tail)));
    }
    out
}

/// 腕の本体。本体が `->` を含むなら括弧で包む。包まないと、後ろに続く
/// `| 次の腕` が本体の内側の分岐の腕として読まれる（`| A => 2 as y -> y | B => 3`
/// の `| B` は内側の fallback になる）。
fn arm_value(value: &Node, indent: usize, tail: bool) -> String {
    let text = expr(value, indent, tail);
    if contains_continuation(value) {
        format!("({text})")
    } else {
        text
    }
}

fn cond_arms(arms: &[(Node, Node)], fallback: Option<&Node>, indent: usize, tail: bool) -> String {
    let mut out = String::new();
    for (condition, value) in arms {
        out.push_str(&format!(
            " | {} => {}",
            condition_text(condition, indent),
            expr(value, indent, tail)
        ));
    }
    if let Some(fallback) = fallback {
        out.push_str(&format!(" | {}", expr(fallback, indent, tail)));
    }
    out
}

fn implementation_text(module: &ModuleImpl) -> String {
    let header = format!("implement {} as {}", module.sig, module.name);
    match &module.form {
        ImplementationForm::Ordinary => {
            format!("{header}\n{}\n/implement", items_text(&module.items))
        }
        ImplementationForm::Functor(parameters) => {
            let parameters = parameters
                .iter()
                .map(|parameter| format!("{}: {}", parameter.name, parameter.contract))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{header}({parameters})\n{}\n/implement",
                items_text(&module.items)
            )
        }
        ImplementationForm::Application { functor, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| argument.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{header} = {}({arguments})", functor.name)
        }
    }
}

fn effect_text(effect: &EffectDecl) -> String {
    let ops = effect
        .ops
        .iter()
        .map(effect_op_text)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "effect {}{}{}",
        type_parameters(&effect.type_params),
        effect.name,
        if ops.is_empty() {
            String::new()
        } else {
            format!("\n{ops}")
        }
    )
}

fn effect_op_text(op: &EffectOp) -> String {
    let params = if op.params.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            op.params
                .iter()
                .map(|param| {
                    format!(
                        "{}: {}",
                        param.name,
                        type_source(param.ty.as_ref().expect("effect parameter"))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    format!(
        "{} {}{}{}",
        op.kind.as_str(),
        op.name,
        params,
        return_type(op.ret.as_ref())
    )
}

fn handler_text(handler: &HandlerDecl) -> String {
    let params = if handler.params.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            handler
                .params
                .iter()
                .map(|param| {
                    format!(
                        "{}: {}",
                        param.name,
                        type_source(param.ty.as_ref().expect("handler parameter"))
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let arms = handler
        .arms
        .iter()
        .map(handler_arm_text)
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "handler {}{} as {}{}{}",
        type_parameters(&handler.type_params),
        handler.effect,
        handler.name,
        params,
        if arms.is_empty() {
            String::new()
        } else {
            format!("\n{arms}")
        }
    )
}

fn handler_arm_text(arm: &HandlerArm) -> String {
    match arm {
        HandlerArm::Operation {
            kind,
            name,
            params,
            body,
            ..
        } => {
            let params = params
                .iter()
                .map(|param| format!(" {}", param.name))
                .collect::<Vec<_>>()
                .join("");
            format!(
                "{} {}{} => {}",
                kind.as_str(),
                name,
                params,
                expr(body, 2, true)
            )
        }
        HandlerArm::Return { param, body, .. } => {
            format!("return {} => {}", param.name, expr(body, 2, true))
        }
    }
}

/// `tail` は、その位置に続きが無いこと（束縛や分岐を閉じずに置けること）。
fn expr(node: &Node, indent: usize, tail: bool) -> String {
    expr_prec(node, indent, tail, 0)
}

/// 演算子の強さ（§7）。大きいほど強く結合する。
fn bin_prec(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 2,
        BinOp::And => 3,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne => 4,
        BinOp::Add | BinOp::Sub => 5,
        BinOp::Mul | BinOp::Div | BinOp::Mod => 6,
    }
}

fn is_comparison(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
    )
}

/// `min` より弱い演算子は括弧で包む。
fn expr_prec(node: &Node, indent: usize, tail: bool, min: u8) -> String {
    match &node.kind {
        NodeKind::Int(n) => n.to_string(),
        NodeKind::Float(x) => float_text(*x),
        NodeKind::Char(c) => literal(&c.to_string(), '\''),
        NodeKind::Str(s) => literal(s, '"'),
        NodeKind::Bool(b) => b.to_string(),
        NodeKind::Void => "()".to_string(),
        NodeKind::Name(name) => name.clone(),
        // `- - x` を `--x` と綴ると、字句が `--` をコメントの印と読んで E104。
        // 入れ子の `-` の被演算子は括弧で包む（§7）。
        NodeKind::Neg(c) => {
            let inner = expr_prec(c, indent, false, 7);
            let text = if matches!(c.kind, NodeKind::Neg(_)) {
                format!("-({inner})")
            } else {
                format!("-{inner}")
            };
            wrap(text, 7, min)
        }
        NodeKind::Not(c) => wrap(format!("not {}", expr_prec(c, indent, false, 7)), 7, min),
        NodeKind::Bin(op, l, r) => {
            let prec = bin_prec(*op);
            let (left_min, right_min) = if is_comparison(*op) {
                (prec + 1, prec + 1)
            } else {
                (prec, prec + 1)
            };
            let text = format!(
                "{} {} {}",
                expr_prec(l, indent, false, left_min),
                op.as_str(),
                expr_prec(r, indent, false, right_min)
            );
            wrap(text, prec, min)
        }
        NodeKind::Pipe(l, r) => wrap(
            format!(
                "{} |> {}",
                expr_prec(l, indent, false, 1),
                expr_prec(r, indent, false, 2)
            ),
            1,
            min,
        ),
        NodeKind::Apply(f, args) => {
            let mut out = expr_prec(f, indent, false, 8);
            for argument in args {
                out.push(' ');
                if matches!(argument.kind, NodeKind::Apply(..))
                    || matches!(&argument.kind, NodeKind::Name(name) if crate::names::is_upper_reference(name))
                {
                    out.push_str(&format!("({})", expr(argument, indent, false)));
                } else {
                    out.push_str(&expr_prec(argument, indent, false, 9));
                }
            }
            out
        }
        NodeKind::Call(f, args) => format!(
            "{}({})",
            expr_prec(f, indent, false, 9),
            args.iter()
                .map(|a| enclosed(a, indent))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        NodeKind::Field(l, r) => {
            let base = if matches!(l.kind, NodeKind::Int(_)) {
                format!("({})", expr(l, indent, false))
            } else {
                expr_prec(l, indent, false, 9)
            };
            format!("{base}.{}", r.name)
        }
        NodeKind::Record(fields) => format!("{{ {} }}", fields_text(fields, indent)),
        NodeKind::NamedConstructor(name, fields) => {
            format!("{} {{ {} }}", name.name, fields_text(fields, indent))
        }
        NodeKind::List(items, rest) => {
            let mut parts = items
                .iter()
                .map(|item| enclosed(item, indent))
                .collect::<Vec<_>>();
            if let Some(rest) = rest {
                parts.push(format!("...{}", enclosed(rest, indent)));
            }
            format!("[{}]", parts.join(", "))
        }
        NodeKind::Let { pat, value, body } => {
            binding(format!(" as {pat}"), value, body, indent, tail)
        }
        NodeKind::LetQ { pat, value, body } => {
            binding(format!(" as {pat}?"), value, body, indent, tail)
        }
        NodeKind::DoQ { value, body } => binding(String::new(), value, body, indent, tail),
        NodeKind::Fn { params, body } => fn_text(params, body, indent),
        NodeKind::BoolBranch { .. }
        | NodeKind::CondBranch { .. }
        | NodeKind::PatternBranch { .. } => branch_text(node, indent, tail),
        NodeKind::With {
            handler,
            args,
            body,
        } => {
            let text = format!(
                "with {}{} -> {}",
                handler.name,
                with_args(args, indent),
                expr(body, indent, true)
            );
            if tail {
                text
            } else {
                format!("({text})")
            }
        }
        NodeKind::Resume => "resume".to_string(),
    }
}

/// 区切り（`,`・`;`）か閉じ括弧（`)`・`]`・`}`）が後ろに来る位置。そこで中の構文も
/// 閉じるので、末尾と同じく印を付けない。
/// 区切りの前の式。`if` は閉じの印を付けず、区切り（`,`・`)` など）に閉じさせる。
/// `fn` は常に閉じるので、区切りの前でも閉じた綴りになる。
fn enclosed(node: &Node, indent: usize) -> String {
    expr(node, indent, true)
}

/// 並置の引数に置く式。区切りの前と同じ扱いにする（§12.1）。
fn single_argument(node: &Node, indent: usize) -> String {
    match &node.kind {
        NodeKind::BoolBranch { .. }
        | NodeKind::CondBranch { .. }
        | NodeKind::PatternBranch { .. } => enclosed(node, indent),
        _ => expr(node, indent, false),
    }
}

fn wrap(text: String, prec: u8, min: u8) -> String {
    if prec < min {
        format!("({text})")
    } else {
        text
    }
}

fn binding(suffix: String, value: &Node, body: &Node, indent: usize, tail: bool) -> String {
    let head = format!("{}{suffix} ->", expr(value, indent, false));
    let pad = " ".repeat(indent);
    let body = expr(body, indent, true);
    if tail {
        format!("{head}\n{pad}{body}")
    } else {
        format!("({head}\n{pad}{body})")
    }
}

fn branch_text(node: &Node, indent: usize, tail: bool) -> String {
    let inner = branch_inner(node, indent);
    let pad = " ".repeat(indent);
    if branch_has_arrow(node) {
        if tail {
            format!("if {inner}\n{pad}/if")
        } else {
            format!("(if {inner}\n{pad}/if)")
        }
    } else if tail {
        format!("if {inner}")
    } else {
        format!("if({inner})")
    }
}

fn branch_inner(node: &Node, indent: usize) -> String {
    match &node.kind {
        NodeKind::BoolBranch { cond, then, els } => format!(
            "{} => {} | {}",
            condition_text(cond, indent),
            expr(then, indent, true),
            expr(els, indent, true)
        ),
        NodeKind::CondBranch { arms, fallback } => {
            cond_arms(arms, fallback.as_deref(), indent, true)
        }
        NodeKind::PatternBranch {
            scrutinees,
            arms,
            fallback,
        } => {
            let mut out = scrutinees
                .iter()
                .map(|scrutinee| condition_text(scrutinee, indent))
                .collect::<Vec<_>>()
                .join(", ");
            for (patterns, body) in arms {
                out.push_str(&format!(
                    " | {} => {}",
                    comma_patterns(patterns),
                    expr(body, indent, true)
                ));
            }
            if let Some(fallback) = fallback {
                out.push_str(&format!(" | {}", expr(fallback, indent, true)));
            }
            out
        }
        _ => expr(node, indent, false),
    }
}

/// `if` の条件と scrutinee。先頭が `(` でも、`)` までの最上位に `=>` が無いので
/// `if(` の閉じの形とは読まれない（パーサの `if_paren_closes`）。
fn condition_text(node: &Node, indent: usize) -> String {
    expr(node, indent, false)
}

fn fn_text(params: &[Param], body: &Node, indent: usize) -> String {
    if let NodeKind::CondBranch { arms, fallback } = &body.kind {
        return format!(
            "fn({}{})",
            lambda_parameters(params),
            cond_arms(arms, fallback.as_deref(), indent + 2, false)
        );
    }
    if is_arm_function(params, body) {
        return format!("fn({})", arm_arms(body, indent + 2, false));
    }
    if contains_continuation(body) {
        let pad = " ".repeat(indent);
        format!(
            "fn {} =>\n{}{}\n{pad}/fn",
            lambda_parameters(params),
            " ".repeat(indent + 2),
            expr(body, indent + 2, true)
        )
    } else {
        format!(
            "fn({} => {})",
            lambda_parameters(params),
            expr(body, indent, true)
        )
    }
}

fn lambda_parameters(params: &[Param]) -> String {
    params
        .iter()
        .map(|param| match &param.ty {
            Some(ty) => format!("{}: {}", param.name, type_source(ty)),
            None => param.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn with_args(args: &[Node], indent: usize) -> String {
    if args.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            args.iter()
                .map(|arg| single_argument(arg, indent))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn fields_text(fields: &[(DeclaredName, Node)], indent: usize) -> String {
    fields
        .iter()
        .map(|(field, value)| format!("{} = {}", field.name, enclosed(value, indent)))
        .collect::<Vec<_>>()
        .join("; ")
}

fn comma_patterns(patterns: &[Pattern]) -> String {
    patterns
        .iter()
        .map(|pattern| pattern.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 整数値でも `1.0` のように浮動小数の見た目を保つ。
fn float_text(x: f64) -> String {
    let text = x.to_string();
    if text.contains('.') {
        text
    } else {
        format!("{text}.0")
    }
}
