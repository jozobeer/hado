use super::*;

pub(crate) fn item_source(value: &Item) -> String {
    item(value)
}

/// The `def f | ...` form of a `def` whose sole parameter is only scrutinized
/// (§31, E515). The parameter is renamed to `$argument`, which is what the
/// `def f | ...` surface form parses to, so the ordinary printer writes the
/// 引数を 1 つ持つ `def` を、§31 / §32 の腕の形に直した綴りを返す。
/// その 1 引数の名前を、ソースに書けない `$argument` に変える。
pub(crate) fn arm_function(def: &Def) -> String {
    let mut lifted = def.clone();
    if let DefKind::Function { params, .. } = &mut lifted.kind {
        rename_to_implicit_argument(params);
    }
    item(&Item::Def(lifted))
}

/// 引数がちょうど 1 つなら、その名前を `$argument` にする。
fn rename_to_implicit_argument(params: &mut [crate::parser::Param]) {
    if let [parameter] = params {
        parameter.name = "$argument".to_string();
    }
}

/// pattern が `Cons` / `Nil` の並びで書かれていれば `[a, ...b]` の綴りを返す。
/// そうでなければ `None`（呼び側が元の綴りを使う）。
pub(crate) fn list_pattern(pattern: &Pattern) -> Option<String> {
    if is_nil_pattern(pattern) {
        return Some("[]".into());
    }
    cons_parts(pattern)?;
    let (parts, rest) = cons_elements(pattern);
    Some(list_text(parts, rest))
}

/// 綴りが `Nil` である pattern。`Nil` の名前と、引数の無い `Constructor` の両方。
fn is_nil_pattern(pattern: &Pattern) -> bool {
    match &pattern.kind {
        PatternKind::Name(name) => name == "Nil",
        PatternKind::Constructor(name, args) => name == "Nil" && args.is_empty(),
        _ => false,
    }
}

/// `Cons` の並びを先頭から辿り、要素の綴りと、`Nil` で終わらない残りを返す。
fn cons_elements(pattern: &Pattern) -> (Vec<String>, &Pattern) {
    let mut parts = Vec::new();
    let mut rest = pattern;
    while let Some((head, tail)) = cons_parts(rest) {
        parts.push(head.to_string());
        rest = tail;
    }
    (parts, rest)
}

/// `[...]` の本文。末尾が裸の `Nil` でなければ、残りを `...rest` として足す。
/// 引数の無い `Constructor("Nil")` は先頭の `[]` 判定だけが拾う（元の綴りを保つ）。
fn list_text(mut parts: Vec<String>, rest: &Pattern) -> String {
    if !matches!(&rest.kind, PatternKind::Name(name) if name == "Nil") {
        parts.push(format!("...{rest}"));
    }
    format!("[{}]", parts.join(", "))
}

/// `Cons(head, tail)` ならその 2 つを返す。それ以外は `None`。
fn cons_parts(pattern: &Pattern) -> Option<(&Pattern, &Pattern)> {
    let PatternKind::Constructor(name, args) = &pattern.kind else {
        return None;
    };
    if name != "Cons" || args.len() != 2 {
        return None;
    }
    Some((&args[0], &args[1]))
}

pub fn canonical_program(ast: &Ast) -> String {
    items(&ast.items)
}

fn items(values: &[Item]) -> String {
    values.iter().map(item).collect::<Vec<_>>().join("\n")
}

/// `def` の綴り。`rec` の印と、値定義・関数定義それぞれの綴りを組む。
fn definition(def: &Def) -> String {
    let rec = if def.rec { "rec " } else { "" };
    let tail = match &def.kind {
        DefKind::Value(body) => format!(" = {}", canonical(body)),
        DefKind::Function { params, ret, body } => function_tail(params, ret.as_ref(), body),
    };
    format!("def {rec}{}{tail}", def.name)
}

/// `effect` の綴り。操作が無ければ見出しだけを書く。
fn effect_declaration(effect: &crate::parser::EffectDecl) -> String {
    let ops = effect
        .ops
        .iter()
        .map(effect_op)
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

/// `handler` の綴り。引数と腕が無ければ見出しだけを書く。
fn handler_declaration(handler: &crate::parser::HandlerDecl) -> String {
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
        .map(handler_arm)
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

fn item(value: &Item) -> String {
    match value {
        Item::Def(def) => definition(def),
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
        Item::SigDef(def) => format!(
            "def {}{}{}{}",
            type_parameters(&def.type_params),
            def.name,
            parameters(&def.params),
            return_type(def.ret.as_ref())
        ),
        Item::ModuleSig(module) => format!("module {}\n{}", module.name, items(&module.items)),
        Item::ModuleImpl(module) => implementation(module),
        Item::Effect(effect) => effect_declaration(effect),
        Item::Handler(handler) => handler_declaration(handler),
    }
}

fn effect_op(op: &EffectOp) -> String {
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

fn handler_arm(arm: &HandlerArm) -> String {
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
                canonical(body)
            )
        }
        HandlerArm::Return { param, body, .. } => {
            format!("return {} => {}", param.name, canonical(body))
        }
    }
}

fn implementation(module: &ModuleImpl) -> String {
    let header = format!("implement {} as {}", module.sig, module.name);
    match &module.form {
        ImplementationForm::Ordinary => format!("{header}\n{}", items(&module.items)),
        ImplementationForm::Functor(parameters) => {
            let parameters = parameters
                .iter()
                .map(|parameter| format!("{}: {}", parameter.name, parameter.contract))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{header}({parameters})\n{}", items(&module.items))
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

fn type_parameters(names: &[DeclaredName]) -> String {
    let names = names
        .iter()
        .map(|name| name.name.as_str())
        .collect::<Vec<_>>();
    match names.as_slice() {
        [] => String::new(),
        [name] => format!("{name} "),
        _ => format!("({}) ", names.join(", ")),
    }
}

pub(super) fn lambda_tail(params: &[Param], body: &Node) -> String {
    let arguments = params
        .iter()
        .map(|param| match &param.ty {
            Some(ty) => format!("{}: {}", param.name, type_source(ty)),
            None => param.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    function_tail_with_parameters(params, None, body, &arguments)
        .trim_start()
        .into()
}

pub(super) fn function_tail(params: &[Param], ret: Option<&TypeExpr>, body: &Node) -> String {
    function_tail_with_parameters(params, ret, body, &parameters(params))
}

fn function_tail_with_parameters(
    params: &[Param],
    ret: Option<&TypeExpr>,
    body: &Node,
    arguments: &str,
) -> String {
    if params.len() == 1 && params[0].name == "$argument" {
        if let NodeKind::PatternBranch { arms, fallback, .. } = &body.kind {
            let mut output = String::new();
            for (patterns, value) in arms {
                output.push_str(&format!(
                    " | {} => {}",
                    comma_patterns(patterns),
                    pattern_arm_body(value)
                ));
            }
            if let Some(value) = fallback {
                output.push_str(&format!(" | {}", canonical(value)));
            }
            return output;
        }
    }
    if let NodeKind::CondBranch { arms, fallback } = &body.kind {
        let mut output = format!("{}{}", arguments, return_type(ret));
        for (condition, value) in arms {
            output.push_str(&format!(
                " | {} => {}",
                canonical(condition),
                canonical(value)
            ));
        }
        if let Some(value) = fallback {
            output.push_str(&format!(" | {}", canonical(value)));
        }
        return output;
    }
    format!("{}{} => {}", arguments, return_type(ret), canonical(body))
}

fn parameters(params: &[Param]) -> String {
    if params.is_empty() {
        return String::new();
    }
    if params.iter().all(|param| param.ty.is_none()) {
        return format!(
            " {}",
            params
                .iter()
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    let fields = params
        .iter()
        .map(|param| {
            format!(
                "{}: {}",
                param.name,
                type_source(param.ty.as_ref().expect("typed parameter group"))
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(" ({fields})")
}

fn return_type(ty: Option<&TypeExpr>) -> String {
    ty.map(|ty| format!(": {}", type_source(ty)))
        .unwrap_or_default()
}

fn type_source(ty: &TypeExpr) -> String {
    let core = match &ty.kind {
        TypeKind::Name(name) => name.clone(),
        TypeKind::Apply(name, args) if name == "Fn" => {
            let inputs = match &args[0].kind {
                TypeKind::Group(inputs) => type_list(inputs),
                _ => type_source(&args[0]),
            };
            format!("Fn({inputs} => {})", type_source(&args[1]))
        }
        TypeKind::Apply(name, args) if args.len() == 1 => {
            format!("{} {name}", type_source(&args[0]))
        }
        TypeKind::Apply(name, args) => format!("({}) {name}", type_list(args)),
        TypeKind::Group(args) => format!("({})", type_list(args)),
        TypeKind::Record(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|field| format!("{}: {}", field.name.name, type_source(&field.ty)))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        TypeKind::Adt(variants) => variants
            .iter()
            .map(|variant| {
                let name = &variant.name.name;
                match &variant.payload {
                    crate::parser::ConstructorPayload::Positional(args) => {
                        format!("| {name}({})", type_list(args))
                    }
                    crate::parser::ConstructorPayload::Nullary => format!("| {name}"),
                    crate::parser::ConstructorPayload::Named(fields) => format!(
                        "| {name} {{ {} }}",
                        fields
                            .iter()
                            .map(|field| format!("{}: {}", field.name.name, type_source(&field.ty)))
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    };
    match &ty.effects {
        None => core,
        Some(EffectRow::Pure) => format!("{core} Pure"),
        Some(EffectRow::Row(effects)) => format!("{core} {{{}}}", type_list(effects)),
    }
}

fn type_list(types: &[TypeExpr]) -> String {
    types.iter().map(type_source).collect::<Vec<_>>().join(", ")
}

pub(super) fn literal(text: &str, delimiter: char) -> String {
    let mut result = delimiter.to_string();
    for ch in text.chars() {
        match ch {
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\\' => result.push_str("\\\\"),
            ch if ch == delimiter => {
                result.push('\\');
                result.push(ch);
            }
            ch => result.push(ch),
        }
    }
    result.push(delimiter);
    result
}
