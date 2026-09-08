use super::*;

pub(super) fn list_pattern(pattern: &Pattern) -> Option<String> {
    match &pattern.kind {
        PatternKind::Name(name) if name == "Nil" => return Some("[]".into()),
        PatternKind::Constructor(name, args) if name == "Nil" && args.is_empty() => return Some("[]".into()),
        PatternKind::Constructor(name, args) if name == "Cons" && args.len() == 2 => {}
        _ => return None,
    }
    let mut parts = Vec::new();
    let mut rest = pattern;
    while let PatternKind::Constructor(name, args) = &rest.kind {
        if name != "Cons" || args.len() != 2 { break; }
        parts.push(args[0].to_string());
        rest = &args[1];
    }
    if !matches!(&rest.kind, PatternKind::Name(name) if name == "Nil") {
        parts.push(format!("...{rest}"));
    }
    Some(format!("[{}]", parts.join(", ")))
}

pub fn canonical_program(ast: &Ast) -> String {
    items(&ast.items)
}

fn items(values: &[Item]) -> String {
    values.iter().map(item).collect::<Vec<_>>().join("\n")
}

fn item(value: &Item) -> String {
    match value {
        Item::Def(def) => {
            let rec = if def.rec { "rec " } else { "" };
            let tail = match &def.kind {
                DefKind::Value(body) => format!(" = {}", canonical(body)),
                DefKind::Function { params, ret, body } => {
                    function_tail(params, ret.as_ref(), body)
                }
            };
            format!("def {rec}{}{tail}", def.name)
        }
        Item::TypeDef(def) => {
            let body = def
                .ty
                .as_ref()
                .map(|ty| format!(" = {}", type_source(ty)))
                .unwrap_or_default();
            format!("type {}{}{body}", def.name, type_parameters(&def.params))
        }
        Item::SigDef(def) => format!(
            "def {}{}{}{}",
            def.name,
            type_parameters(&def.type_params),
            parameters(&def.params),
            return_type(def.ret.as_ref())
        ),
        Item::ModuleSig(module) => format!(
            "module type {} = sig\n{}\nend",
            module.name,
            items(&module.items)
        ),
        Item::ModuleImpl(module) => format!(
            "module {} : {} = struct\n{}\nend",
            module.name,
            module.sig.as_deref().unwrap_or_default(),
            items(&module.items)
        ),
    }
}

fn type_parameters(names: &[DeclaredName]) -> String {
    if names.is_empty() {
        String::new()
    } else {
        format!("<{}>", names.iter().map(|name| name.name.as_str()).collect::<Vec<_>>().join(", "))
    }
}

pub(super) fn function_tail(params: &[Param], ret: Option<&TypeExpr>, body: &Node) -> String {
    if params.len() == 1 && params[0].name == "$argument" {
        if let NodeKind::PatternBranch { arms, fallback, .. } = &body.kind {
            let mut output = String::new();
            for (pattern, value) in arms {
                output.push_str(&format!(" | {pattern} => {}", canonical(value)));
            }
            if let Some(value) = fallback {
                output.push_str(&format!(" | {}", canonical(value)));
            }
            return output;
        }
    }
    format!(
        "{}{} => {}",
        parameters(params),
        return_type(ret),
        canonical(body)
    )
}

fn parameters(params: &[Param]) -> String {
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
    match &ty.kind {
        TypeKind::Name(name) => name.clone(),
        TypeKind::Apply(name, args) => format!("{name}<{}>", type_list(args)),
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
            .map(|variant| match &variant.kind {
                TypeKind::Apply(name, args) => format!("| {name}({})", type_list(args)),
                TypeKind::Name(name) => format!("| {name}"),
                _ => unreachable!("constructor definition"),
            })
            .collect::<Vec<_>>()
            .join(" "),
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
