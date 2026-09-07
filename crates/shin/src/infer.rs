use crate::parser::{Ast, BinOp, DefKind, Diagnostic, Item, Node, NodeKind, Span};
use crate::parser::{Param, Pattern, PatternKind, TypeExpr, TypeKind};
use crate::types::{Inference, Scheme, Ty, TypeError};
use crate::Checked;
use std::collections::HashMap;

struct Checker<'a> {
    aliases: HashMap<String, TypeExpr>,
    abstract_representations: HashMap<String, Ty>,
    deferred: Vec<Diagnostic>,
    module_constructors: HashMap<String, (u32, Scheme)>,
    module_records: HashMap<String, (Vec<String>, Scheme)>,
    trusted_start: Option<usize>,
    module_path: Vec<String>,
    module_definitions: HashMap<String, Scheme>,
    composite_types: HashMap<(usize, usize), Ty>,
    value_types: HashMap<(usize, usize), Ty>,
    path: &'a str,
    current_definition: Option<String>,
    inference: Inference,
    values: HashMap<String, Scheme>,
    numeric_constraints: Vec<(Ty, Span)>,
    divisions: Vec<(BinOp, Ty, Ty, Span)>,
    type_arities: HashMap<String, usize>,
    constructor_tags: HashMap<String, u32>,
    constructors: HashMap<String, Scheme>,
    records: HashMap<String, (Vec<String>, Scheme)>,
    modules: HashMap<String, HashMap<String, Scheme>>,
    abstract_types: HashMap<String, (usize, String)>,
    projections: Vec<(Ty, String, Ty, Span)>,
    comparison_constraints: Vec<(Ty, Span, &'static str)>,
}

pub fn check_ast(ast: &Ast, path: &str) -> Result<Checked, Diagnostic> {
    check_ast_with_prelude(ast, path, None)
}

pub(crate) fn check_ast_with_prelude(ast: &Ast, path: &str, trusted_start: Option<usize>) -> Result<Checked, Diagnostic> {
    let (checked, deferred) = infer_with_diagnostics(ast, path, trusted_start)?;
    match deferred.into_iter().min_by_key(crate::diagnostic_order) {
        Some(error) => Err(error),
        None => Ok(checked),
    }
}

pub(crate) fn infer_with_diagnostics(ast: &Ast, path: &str, trusted_start: Option<usize>) -> Result<(Checked, Vec<Diagnostic>), Diagnostic> {
    let mut checker = Checker {
        aliases: HashMap::new(),
        abstract_representations: HashMap::new(),
        deferred: Vec::new(),
        module_constructors: HashMap::new(),
        module_records: HashMap::new(),
        trusted_start,
        module_path: Vec::new(),
        module_definitions: HashMap::new(),
        composite_types: HashMap::new(),
        value_types: HashMap::new(),
        path,
        current_definition: None,
        inference: Inference::default(),
        values: HashMap::new(),
        numeric_constraints: Vec::new(),
        divisions: Vec::new(),
        type_arities: HashMap::new(),
        constructor_tags: HashMap::new(),
        constructors: HashMap::new(),
        records: HashMap::new(),
        modules: HashMap::new(),
        abstract_types: HashMap::new(),
        projections: Vec::new(),
        comparison_constraints: Vec::new(),
    };
    let comparable = checker.inference.fresh();
    let compare = Ty::Fn(vec![comparable.clone(), comparable.clone()], Box::new(Ty::Int));
    checker.values.insert(
        "compare".into(),
        checker.inference.generalize_constrained(&compare, &[], &[(comparable, "E704")]),
    );
    for item in &ast.items {
        if let Item::TypeDef(def) = item {
            if checker
                .type_arities
                .insert(def.name.clone(), def.params.len())
                .is_some()
            {
                return Err(checker.error(def.span, None, "duplicate type"));
            }
        }
    }
    for item in &ast.items {
        if let Item::TypeDef(def) = item {
            checker.register_type(def)?;
        }
    }
    let signatures: HashMap<_, _> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::ModuleSig(sig) => Some((sig.name.clone(), sig)),
            _ => None,
        })
        .collect();
    let mut definitions = HashMap::new();
    for item in &ast.items {
        match item {
            Item::Def(def) => {
                if definitions.insert(def.name.clone(), def).is_some() {
                    return Err(checker.error(def.span, None, "duplicate definition"));
                }
            }
            Item::TypeDef(_) | Item::ModuleSig(_) | Item::ModuleImpl(_) => {}
            _ => return Err(checker.error(ast.span, None, "item inference is not implemented")),
        }
    }
    for group in crate::dependencies::inference_groups(ast) {
        let mut functions = Vec::new();
        for item in group {
            match item {
                Item::ModuleImpl(module) => checker.module(module, &signatures)?,
                Item::Def(def) => functions.push(def.name.clone()),
                _ => {}
            }
        }
        if !functions.is_empty() { checker.definition_group(&functions, &definitions)?; }
    }
    checker.resolve_projections(true)?;
    for (ty, span, code) in &checker.comparison_constraints {
        if checker.contains_function(&checker.inference.resolve(ty), &mut Vec::new()) {
            checker.deferred.push(checker.error(*span, Some(code), "values containing functions cannot be compared"));
        }
    }
    let scheme = checker
        .values
        .get("main")
        .cloned()
        .ok_or_else(|| checker.error(ast.span, Some("E206"), "main is not defined"))?;
    let main = checker.inference.instantiate(&scheme);
    if !checker.inference.resolve(&main).is_monomorphic() {
        checker.deferred.push(checker.error(definitions["main"].span, Some("E701"), "main must have a monomorphic type"));
    }
    if checker.contains_function(&checker.inference.resolve(&main), &mut Vec::new()) {
        checker.deferred.push(checker.error(definitions["main"].span, Some("E702"), "main cannot return a value containing a function"));
    }
    Ok((Checked {
        abstract_representations: checker.abstract_representations,
        module_constructors: checker.module_constructors,
        module_records: checker.module_records,
        module_definitions: checker.module_definitions.clone(),
        modules: checker.modules.clone(),
        constructor_tags: checker.constructor_tags.clone(),
        constructors: checker.constructors.clone(),
        records: checker.records.clone(),
        composite_types: checker.composite_types.iter().map(|(span, ty)| (*span, checker.inference.resolve(ty))).collect(),
        value_types: checker.value_types.iter().map(|(span, ty)| (*span, checker.inference.resolve(ty))).collect(),
        main: checker.inference.resolve(&main),
        definitions: definitions.keys().map(|name| (name.clone(), checker.values[name].clone())).collect(),
    }, checker.deferred))
}

impl Checker<'_> {
    fn module(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<(), Diagnostic> {
        let name = module
            .sig
            .as_ref()
            .ok_or_else(|| self.error(module.span, Some("E113"), "module requires a signature"))?;
        let signature = signatures
            .get(name)
            .ok_or_else(|| self.error(module.span, Some("E204"), "unknown signature"))?;
        let saved = self.values.clone();
        let saved_types = self.type_arities.clone();
        let saved_aliases = self.aliases.clone();
        let saved_records = self.records.clone();
        let saved_constructors = self.constructors.clone();
        let saved_tags = self.constructor_tags.clone();
        let saved_modules = self.modules.clone();
        let saved_abstracts = self.abstract_types.clone();
        let saved_value_spans: std::collections::HashSet<_> = self.value_types.keys().copied().collect();
        let saved_composite_spans: std::collections::HashSet<_> = self.composite_types.keys().copied().collect();
        self.module_path.push(module.name.clone());
        let result = self.module_body(module, signature, signatures);
        if result.is_ok() {
            let type_names: HashMap<_, _> = module.items.iter().filter_map(|item| match item {
                Item::TypeDef(definition) => Some((definition.name.clone(), format!("{}.{}", self.module_path.join("."), definition.name))),
                _ => None,
            }).collect();
            let prefix = format!("{}.", self.module_path.join("."));
            for (name, representation) in &mut self.abstract_representations {
                if name.starts_with(&prefix) {
                    *representation = representation.qualify_data_types(&type_names);
                }
            }
            for (name, scheme) in &mut self.module_definitions {
                if name.starts_with(&prefix) {
                    *scheme = scheme.qualify_data_types(&type_names);
                }
            }
            for (span, ty) in &mut self.value_types {
                if !saved_value_spans.contains(span) {
                    *ty = self.inference.resolve(ty).qualify_data_types(&type_names);
                }
            }
            for (span, ty) in &mut self.composite_types {
                if !saved_composite_spans.contains(span) {
                    *ty = self.inference.resolve(ty).qualify_data_types(&type_names);
                }
            }
            for item in &module.items {
                if let Item::TypeDef(definition) = item {
                    if matches!(definition.ty.as_ref().map(|ty| &ty.kind), Some(TypeKind::Record(_))) {
                        if let Some(record) = self.records.get(&definition.name) {
                            self.module_records.insert(format!("{}.{}", self.module_path.join("."), definition.name), (record.0.clone(), record.1.qualify_data_types(&type_names)));
                        }
                    } else {
                        for (name, scheme) in &self.constructors {
                            let result = match scheme.body() { Ty::Fn(_, result) => result.as_ref(), ty => ty };
                            if matches!(result, Ty::Adt(type_name, _) if type_name == &definition.name) {
                                self.module_constructors.insert(format!("{}.{}", self.module_path.join("."), name), (self.constructor_tags[name], scheme.qualify_data_types(&type_names)));
                            }
                        }
                    }
                }
            }
        }
        self.module_path.pop();
        self.modules = saved_modules;
        self.abstract_types = saved_abstracts;
        self.type_arities = saved_types;
        self.aliases = saved_aliases;
        self.records = saved_records;
        self.constructors = saved_constructors;
        self.constructor_tags = saved_tags;
        self.values = saved;
        self.modules.insert(module.name.clone(), result?);
        for item in &signature.items {
            if let Item::TypeDef(def) = item {
                let local = format!("{}.{}", module.name, def.name);
                let qualified = if self.module_path.is_empty() { local.clone() } else { format!("{}.{}", self.module_path.join("."), local) };
                self.abstract_types.insert(local, (def.params.len(), qualified));
            }
        }
        Ok(())
    }

    fn module_body(
        &mut self,
        module: &crate::parser::ModuleImpl,
        signature: &crate::parser::ModuleSig,
        signatures: &HashMap<String, &crate::parser::ModuleSig>,
    ) -> Result<HashMap<String, Scheme>, Diagnostic> {
        let ast = Ast {
            items: module.items.clone(),
            span: module.span,
            branches_after_fallback: Vec::new(),
            branch_anchors: Vec::new(),
        };
        crate::type_cycles::check(&ast, self.path)?;
        let mut signatures = signatures.clone();
        for item in &ast.items {
            if let Item::ModuleSig(signature) = item {
                signatures.insert(signature.name.clone(), signature);
            }
        }
        for item in &ast.items {
            if let Item::ModuleImpl(inner) = item {
                self.module(inner, &signatures)?;
            }
        }
        let mut definitions = HashMap::new();
        let mut local_types = std::collections::HashSet::new();
        for item in &ast.items {
            if let Item::TypeDef(def) = item {
                if !local_types.insert(def.name.clone()) {
                    return Err(self.error(def.span, None, "duplicate module type"));
                }
                self.type_arities.insert(def.name.clone(), def.params.len());
                self.aliases.remove(&def.name);
                if let Some(ty) = &def.ty {
                    if matches!(ty.kind, TypeKind::Name(_) | TypeKind::Apply(_, _)) {
                        self.aliases.insert(def.name.clone(), ty.clone());
                    }
                }
            }
        }
        for item in &ast.items {
            match item {
                Item::TypeDef(def) => self.register_type(def)?,
                Item::Def(def) => {
                    if definitions.insert(def.name.clone(), def).is_some() {
                        return Err(self.error(def.span, None, "duplicate module definition"));
                    }
                }
                Item::ModuleImpl(_) | Item::ModuleSig(_) => {}
                _ => {
                    return Err(self.error(
                        module.span,
                        None,
                        "unexpected signature member in module",
                    ))
                }
            }
        }
        for group in crate::names::definition_groups(&ast) {
            self.definition_group(&group, &definitions)?;
        }
        for name in definitions.keys() {
            self.module_definitions.insert(format!("{}.{}", self.module_path.join("."), name), self.values[name].clone());
        }
        let mut abstract_names = HashMap::new();
        for item in &signature.items {
            if let Item::TypeDef(def) = item {
                if !ast.items.iter().any(|item| matches!(item, Item::TypeDef(actual) if actual.name == def.name && actual.params.len() == def.params.len())) {
                    return Err(self.error(module.span, Some("E602"), "missing signature type"));
                }
                if def.ty.is_some() {
                    return Err(self.error(
                        def.span,
                        None,
                        "manifest signature types are not implemented",
                    ));
                }
                let qualified = format!("{}.{}", self.module_path.join("."), def.name);
                let reference = TypeExpr { kind: TypeKind::Name(def.name.clone()), span: def.name_span };
                let representation = self.annotation(&reference, def.name_span)?;
                self.abstract_representations.insert(qualified.clone(), representation);
                abstract_names.insert(def.name.clone(), qualified);
            }
        }
        let mut exports = HashMap::new();
        for item in &signature.items {
            if matches!(item, Item::TypeDef(_)) {
                continue;
            }
            let Item::SigDef(declaration) = item else {
                return Err(self.error(
                    signature.span,
                    None,
                    "signature type members are not implemented",
                ));
            };
            if !definitions.contains_key(&declaration.name) {
                return Err(self.error(module.span, Some("E602"), "missing signature member"));
            }
            let scheme = self.values[&declaration.name].clone();
            let (actual, constraints) = self.inference.instantiate_constrained(&scheme);
            self.comparison_constraints.extend(constraints.into_iter().map(|(ty, code)| (ty, declaration.span, code)));
            let parameters: HashMap<_, _> = declaration.type_params.iter().map(|name| (name.name.clone(), self.inference.fresh())).collect();
            let expected = self.signature_type(declaration, &parameters)?;
            self.inference
                .unify(&actual, &expected)
                .map_err(|_| self.error(module.span, Some("E602"), "signature type mismatch"))?;
            let mut resolved_parameters = Vec::new();
            for parameter in parameters.values() {
                let resolved = self.inference.resolve(parameter);
                if !matches!(resolved, Ty::Var(_)) || resolved_parameters.contains(&resolved) {
                    return Err(self.error(module.span, Some("E602"), "signature implementation is not sufficiently polymorphic"));
                }
                resolved_parameters.push(resolved);
            }
            let mut public_parameters: HashMap<_, _> = abstract_names.iter().map(|(name, public)|
                (name.clone(), Ty::Abstract(public.clone(), vec![]))).collect();
            public_parameters.extend(parameters);
            let public_type = self.signature_type(declaration, &public_parameters)?;
            exports.insert(declaration.name.clone(), self.inference.generalize(&public_type, &[]));
        }
        let roots = exports.keys().cloned().collect::<Vec<_>>();
        if let Some(name) = crate::names::unused_definitions(&ast, &roots).first() {
            self.deferred.push(self.error(definitions[name].span, Some("E514"), &format!("unused module definition {name}")));
        }
        Ok(exports)
    }

    fn signature_type(&self, declaration: &crate::parser::SigDef, parameters: &HashMap<String, Ty>) -> Result<Ty, Diagnostic> {
        let args = declaration
            .params
            .iter()
            .map(|param| {
                let annotation = param.ty.as_ref().ok_or_else(|| {
                    self.error(param.span, None, "signature parameter requires a type")
                })?;
                self.payload_type(annotation, parameters, param.span)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let result = declaration.ret.as_ref().ok_or_else(|| {
            self.error(declaration.span, None, "signature requires a return type")
        })?;
        Ok(Ty::Fn(
            args,
            Box::new(self.payload_type(result, parameters, declaration.span)?),
        ))
    }

    fn definition_group(
        &mut self,
        group: &[String],
        definitions: &HashMap<String, &crate::parser::Def>,
    ) -> Result<(), Diagnostic> {
        let placeholders: HashMap<_, _> = group
            .iter()
            .map(|name| (name.clone(), self.inference.fresh()))
            .collect();
        for (name, ty) in &placeholders {
            self.values
                .insert(name.clone(), Scheme::monomorphic(ty.clone()));
        }
        for name in group {
            let def = definitions[name];
            self.current_definition = Some(name.clone());
            let removed = if !def.rec {
                self.values.remove(name)
            } else {
                None
            };
            let ty = self.definition(def)?;
            if let Some(scheme) = removed {
                self.values.insert(name.clone(), scheme);
            }
            if def.rec {
                self.check_recursive_arguments(&placeholders[name], &ty, def.span)?;
            }
            self.unify(&placeholders[name], &ty, def.span)?;
        }
        self.current_definition = None;
        self.default_numbers()?;
        self.resolve_projections(false)?;
        let environment: Vec<_> = self
            .projections
            .iter()
            .flat_map(|(input, _, output, _)| [input.clone(), output.clone()])
            .collect();
        for (name, ty) in placeholders {
            let constraints = self.comparison_constraints.iter().map(|(ty, _, code)| (ty.clone(), *code)).collect::<Vec<_>>();
            self.values
                .insert(name, self.inference.generalize_constrained(&ty, &environment, &constraints));
        }
        Ok(())
    }

    fn check_recursive_arguments(&self, calls: &Ty, definition: &Ty, span: Span) -> Result<(), Diagnostic> {
        let mut inference = self.inference.clone();
        if let (Ty::Fn(calls, _), Ty::Fn(parameters, _)) = (inference.resolve(calls), inference.resolve(definition)) {
            for (call, parameter) in calls.iter().zip(&parameters) {
                if inference.unify(call, parameter) == Err(TypeError::Occurs) {
                    return Err(self.error(span, Some("E307"), "recursive calls change the argument type"));
                }
            }
        }
        Ok(())
    }

    fn definition(&mut self, def: &crate::parser::Def) -> Result<Ty, Diagnostic> {
        match &def.kind {
            DefKind::Value(body) => self.expression(body),
            DefKind::Function { params, ret, body } => {
                let ty = self.function(params, body)?;
                if let (Some(annotation), Ty::Fn(_, result)) = (ret, &ty) {
                    let expected = self.annotation(annotation, def.span)?;
                    self.unify(result, &expected, def.span)?;
                }
                Ok(ty)
            }
        }
    }

    fn register_type(&mut self, def: &crate::parser::TypeDef) -> Result<(), Diagnostic> {
        if self.aliases.contains_key(&def.name) { return Ok(()); }
        let parameters: HashMap<_, _> = def
            .params
            .iter()
            .map(|name| (name.name.clone(), self.inference.fresh()))
            .collect();
        let result = Ty::Adt(
            def.name.clone(),
            def.params
                .iter()
                .map(|name| parameters[&name.name].clone())
                .collect(),
        );
        let body = def
            .ty
            .as_ref()
            .ok_or_else(|| self.error(def.span, None, "abstract type outside signature"))?;
        if let TypeKind::Record(fields) = &body.kind {
            let payload = fields
                .iter()
                .map(|field| self.payload_type(&field.ty, &parameters, def.span))
                .collect::<Result<Vec<_>, _>>()?;
            let scheme = self
                .inference
                .generalize(&Ty::Fn(payload, Box::new(result)), &[]);
            self.records.insert(
                def.name.clone(),
                (
                    fields.iter().map(|field| field.name.name.clone()).collect(),
                    scheme,
                ),
            );
            return Ok(());
        }
        let variants = match &body.kind {
            TypeKind::Adt(variants) => variants.as_slice(),
            _ => std::slice::from_ref(body),
        };
        for (tag, variant) in variants.iter().enumerate() {
            self.register_constructor(variant, &parameters, &result, def.span)?;
            let name = match &variant.kind {
                TypeKind::Name(name) | TypeKind::Apply(name, _) => name,
                _ => unreachable!("registered constructor"),
            };
            let tag = u32::try_from(tag).map_err(|_| self.error(def.span, None, "too many constructors"))?;
            self.constructor_tags.insert(name.clone(), tag);
        }
        Ok(())
    }

    fn register_constructor(
        &mut self,
        variant: &TypeExpr,
        parameters: &HashMap<String, Ty>,
        result: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let (name, payload) = match &variant.kind {
            TypeKind::Name(name) => (name, Vec::new()),
            TypeKind::Apply(name, args) => (
                name,
                args.iter()
                    .map(|arg| self.payload_type(arg, parameters, span))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            _ => return Err(self.error(span, None, "record definitions are not implemented")),
        };
        let ty = if payload.is_empty() {
            result.clone()
        } else {
            Ty::Fn(payload, Box::new(result.clone()))
        };
        let scheme = self.inference.generalize(&ty, &[]);
        if self.constructors.insert(name.clone(), scheme).is_some() {
            return Err(self.error(span, None, "duplicate constructor"));
        }
        Ok(())
    }

    fn payload_type(
        &self,
        expr: &TypeExpr,
        parameters: &HashMap<String, Ty>,
        span: Span,
    ) -> Result<Ty, Diagnostic> {
        match &expr.kind {
            TypeKind::Apply(name, args) if name == "Fn" && args.len() == 2 => {
                let inputs = match &args[0].kind { TypeKind::Group(inputs) => inputs.as_slice(), _ => std::slice::from_ref(&args[0]) };
                Ok(Ty::Fn(inputs.iter().map(|ty| self.payload_type(ty, parameters, span)).collect::<Result<_, _>>()?, Box::new(self.payload_type(&args[1], parameters, span)?)))
            }
            TypeKind::Name(name) if parameters.contains_key(name) => Ok(parameters[name].clone()),
            TypeKind::Name(name) if self.aliases.contains_key(name) => self.annotation(expr, span),
            TypeKind::Name(name) if self.type_arities.get(name) == Some(&0) => {
                Ok(Ty::Adt(name.clone(), vec![]))
            }
            TypeKind::Apply(name, args) if self.type_arities.get(name) == Some(&args.len()) => {
                let args = args
                    .iter()
                    .map(|arg| self.payload_type(arg, parameters, span))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Ty::Adt(name.clone(), args))
            }
            _ => self.annotation(expr, span),
        }
    }

    fn error(&self, span: Span, code: Option<&str>, message: &str) -> Diagnostic {
        Diagnostic {
            path: self.path.into(),
            line: span.line,
            col: span.col,
            code: code.map(str::to_owned),
            message: message.into(),
        }
    }

    fn unify(&mut self, a: &Ty, b: &Ty, span: Span) -> Result<(), Diagnostic> {
        self.inference.unify(a, b).map_err(|error| {
            let code = match error {
                TypeError::Mismatch => "E305",
                TypeError::Arity => "E301",
                TypeError::Occurs => "E306",
            };
            self.error(span, Some(code), &format!("cannot unify {a} with {b}"))
        })
    }

    fn expression(&mut self, node: &Node) -> Result<Ty, Diagnostic> {
        let ty = self.expression_inner(node)?;
        if matches!(node.kind, NodeKind::Apply(..) | NodeKind::Call(..) | NodeKind::Pipe(..) | NodeKind::List(..) | NodeKind::Record(..)) {
            self.composite_types.insert((node.span.line, node.span.col), ty.clone());
        }
        Ok(ty)
    }

    fn expression_inner(&mut self, node: &Node) -> Result<Ty, Diagnostic> {
        match &node.kind {
            NodeKind::Int(_) => Ok(Ty::Int),
            NodeKind::Float(_) => Ok(Ty::Float),
            NodeKind::Bool(_) => Ok(Ty::Bool),
            NodeKind::Char(_) => Ok(Ty::Char),
            NodeKind::Str(_) => Ok(Ty::String),
            NodeKind::Unit => Ok(Ty::Unit),
            NodeKind::Bin(op, a, b) => self.binary(*op, a, b),
            NodeKind::Not(value) => {
                let ty = self.expression(value)?;
                self.unify(&ty, &Ty::Bool, node.span)?;
                Ok(Ty::Bool)
            }
            NodeKind::Neg(value) => {
                let ty = self.expression(value)?;
                self.numeric(&ty, node.span)?;
                Ok(ty)
            }
            NodeKind::Name(name) => {
                if self.constructors.contains_key(name) {
                    return self.construct(name, &[], node.span);
                }
                if name.starts_with("__") {
                    return Err(self.error(
                        node.span,
                        Some("E203"),
                        "intrinsics cannot be used as values",
                    ));
                }
                let scheme = self
                    .values
                    .get(name)
                    .cloned()
                    .ok_or_else(|| {
                        self.error(
                            node.span,
                            Some(if self.current_definition.as_deref() == Some(name) {
                                "E201"
                            } else {
                                "E204"
                            }),
                            &format!("unbound name {name}"),
                        )
                    })?;
                let (ty, constraints) = self.inference.instantiate_constrained(&scheme);
                self.value_types.insert((node.span.line, node.span.col), ty.clone());
                self.comparison_constraints.extend(constraints.into_iter().map(|(ty, code)| (ty, node.span, code)));
                Ok(ty)
            }
            NodeKind::Fn { params, body } => {
                let ty = self.function(params, body)?;
                self.value_types.insert((node.span.line, node.span.col), ty.clone());
                Ok(ty)
            },
            NodeKind::List(items) => self.list(items),
            NodeKind::Record(fields) => self.record(fields, node.span),
            NodeKind::Field(value, field) => {
                let ty = self.project(value, &field.name, field.dot)?;
                self.value_types.insert((node.span.line, node.span.col), ty.clone());
                Ok(ty)
            },
            NodeKind::PatternBranch {
                scrutinee,
                arms,
                fallback,
            } => self.pattern_branch(scrutinee, arms, fallback.as_deref(), node.span),
            NodeKind::Pipe(value, target) => self.pipe(value, target),
            NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
                self.apply(callee, args, node.span)
            }
            NodeKind::Let { pat, value, body } => self.binding(pat, value, body),
            NodeKind::LetQ { pat, value, body } => self.propagation(Some(pat), value, body),
            NodeKind::DoQ { value, body } => self.propagation(None, value, body),
            NodeKind::BoolBranch { cond, then, els } => {
                self.condition(cond)?;
                let result = self.expression(then)?;
                self.branch_result(els, &result)?;
                Ok(self.inference.resolve(&result))
            }
            NodeKind::CondBranch { arms, fallback } => self.conditions(arms, fallback.as_deref()),
        }
    }

    fn record(&mut self, fields: &[(crate::parser::DeclaredName, Node)], span: Span) -> Result<Ty, Diagnostic> {
        let candidates: Vec<_> = self
            .records
            .values()
            .filter(|(names, _)| {
                names.len() == fields.len()
                    && names
                        .iter()
                        .all(|name| fields.iter().any(|(field, _)| &field.name == name))
            })
            .cloned()
            .collect();
        if candidates.len() != 1 {
            return Err(self.error(
                span,
                Some("E304"),
                "record fields must identify one nominal type",
            ));
        }
        let (names, scheme) = &candidates[0];
        let Ty::Fn(payload, result) = self.inference.instantiate(scheme) else {
            unreachable!()
        };
        for (name, ty) in names.iter().zip(&payload) {
            let value = &fields.iter().find(|(field, _)| &field.name == name).unwrap().1;
            self.branch_result(value, ty)?;
        }
        Ok(self.inference.resolve(&result))
    }

    fn project(&mut self, value: &Node, field: &str, dot: Span) -> Result<Ty, Diagnostic> {
        if let NodeKind::Name(name) = &value.kind {
            if let Some(exports) = self.modules.get(name) {
                let scheme = exports.get(field).cloned().ok_or_else(|| {
                    self.error(value.span, Some("E204"), "module member is not exported")
                })?;
                return Ok(self.inference.instantiate(&scheme));
            }
        }
        let ty = self.expression(value)?;
        self.project_type(&ty, field, dot)
    }

    fn resolve_projections(&mut self, final_pass: bool) -> Result<(), Diagnostic> {
        for (input, field, output, span) in std::mem::take(&mut self.projections) {
            if matches!(self.inference.resolve(&input), Ty::Var(_)) {
                if final_pass {
                    return Err(self.error(span, None, "record type could not be inferred"));
                }
                self.projections.push((input, field, output, span));
            } else {
                let actual = self.project_type(&input, &field, span)?;
                self.unify(&output, &actual, span)?;
            }
        }
        Ok(())
    }

    fn infer_record_from_field(
        &mut self,
        ty: &Ty,
        field: &str,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if !matches!(self.inference.resolve(ty), Ty::Var(_)) {
            return Ok(());
        }
        let candidates: Vec<_> = self
            .records
            .values()
            .filter(|(names, _)| names.iter().any(|name| name == field))
            .map(|(_, scheme)| scheme.clone())
            .collect();
        if candidates.len() != 1 {
            return Ok(());
        }
        let Ty::Fn(_, result) = self.inference.instantiate(&candidates[0]) else {
            unreachable!()
        };
        self.unify(ty, &result, span)
    }

    fn project_type(&mut self, ty: &Ty, field: &str, span: Span) -> Result<Ty, Diagnostic> {
        if matches!(self.inference.resolve(ty), Ty::Abstract(_, _)) {
            return Err(self.error(span, Some("E601"), "abstract type has no accessible fields"));
        }
        self.infer_record_from_field(ty, field, span)?;
        if matches!(self.inference.resolve(ty), Ty::Var(_)) {
            let result = self.inference.fresh();
            self.projections
                .push((ty.clone(), field.into(), result.clone(), span));
            return Ok(result);
        }
        let Ty::Adt(name, _) = self.inference.resolve(ty) else {
            return Err(self.error(span, None, "record type is not known"));
        };
        let (names, scheme) = self
            .records
            .get(&name)
            .cloned()
            .ok_or_else(|| self.error(span, None, "expected record"))?;
        let index = names
            .iter()
            .position(|name| name == field)
            .ok_or_else(|| self.error(span, Some("E204"), "unknown field"))?;
        let Ty::Fn(payload, result) = self.inference.instantiate(&scheme) else {
            unreachable!()
        };
        self.unify(ty, &result, span)?;
        Ok(self.inference.resolve(&payload[index]))
    }

    fn list(&mut self, items: &[Node]) -> Result<Ty, Diagnostic> {
        let element = self.inference.fresh();
        for item in items {
            self.branch_result(item, &element)?;
        }
        Ok(Ty::Adt(
            "List".into(),
            vec![self.inference.resolve(&element)],
        ))
    }

    fn pattern_branch(
        &mut self,
        scrutinee: &Node,
        arms: &[(Pattern, Node)],
        fallback: Option<&Node>,
        branch_span: Span,
    ) -> Result<Ty, Diagnostic> {
        let input = self.expression(scrutinee)?;
        let result = self.inference.fresh();
        for (pattern, body) in arms {
            let saved = self.values.clone();
            let checked = self
                .pattern(pattern, &input)
                .and_then(|()| self.branch_result(body, &result));
            self.values = saved;
            checked?;
        }
        if let Some(body) = fallback {
            self.branch_result(body, &result)?;
        }
        if self.inference.resolve(&input) == Ty::Bool {
            if let Err(error) = crate::check::boolean_coverage(branch_span, arms, fallback, self.path) {
                self.deferred.push(error);
            }
        }
        if let Ty::Adt(name, _) = self.inference.resolve(&input) {
            let groups = self.constructor_groups();
            let constructors = groups.get(&name).cloned().unwrap_or_default().into_iter().map(|(name, _)| name).collect::<Vec<_>>();
            let families = groups.values().flat_map(|group| group.iter().map(|(name, _)| (name.clone(), group.clone()))).collect();
            if let Err(error) = crate::check::adt_coverage(branch_span, arms, fallback, &constructors, &families, self.path) {
                self.deferred.push(error);
            }
        }
        Ok(self.inference.resolve(&result))
    }

    fn pattern(&mut self, pattern: &Pattern, expected: &Ty) -> Result<(), Diagnostic> {
        match &pattern.kind {
            PatternKind::Underscore => Ok(()),
            PatternKind::Name(name) if name.starts_with(char::is_uppercase) => {
                self.constructor_pattern(name, &[], expected, pattern.span)
            }
            PatternKind::Name(name) => {
                self.values
                    .insert(name.clone(), Scheme::monomorphic(expected.clone()));
                Ok(())
            }
            PatternKind::Literal(node) => self.branch_result(node, expected),
            PatternKind::Constructor(name, args) => {
                self.constructor_pattern(name, args, expected, pattern.span)
            }
            PatternKind::Record(fields) => self.record_pattern(fields, expected, pattern.span),
        }
    }

    fn record_pattern(
        &mut self,
        fields: &[(String, Option<Pattern>)],
        expected: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if matches!(self.inference.resolve(expected), Ty::Var(_)) {
            let candidates: Vec<_> = self.records.values().filter(|(names, _)|
                fields.iter().all(|(field, _)| names.contains(field))).map(|(_, scheme)| scheme.clone()).collect();
            let [scheme] = candidates.as_slice() else {
                return Err(self.error(span, Some("E304"), "record pattern fields must identify one nominal type"));
            };
            let Ty::Fn(_, result) = self.inference.instantiate(scheme) else { unreachable!() };
            self.unify(expected, &result, span)?;
        }
        for (name, pattern) in fields {
            let ty = self.project_type(expected, name, span)?;
            match pattern {
                Some(pattern) => self.pattern(pattern, &ty)?,
                None => {
                    self.values.insert(name.clone(), Scheme::monomorphic(ty));
                }
            }
        }
        Ok(())
    }

    fn constructor_pattern(
        &mut self,
        name: &str,
        args: &[Pattern],
        expected: &Ty,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let scheme = self
            .constructors
            .get(name)
            .cloned()
            .ok_or_else(|| self.error(span, Some("E204"), "unknown constructor"))?;
        let ty = self.inference.instantiate(&scheme);
        match ty {
            Ty::Fn(payload, result) => {
                self.unify(&result, expected, span)?;
                self.pattern_arguments(args, &payload, span)
            }
            ty if args.is_empty() => self.unify(&ty, expected, span),
            _ => Err(self.error(span, Some("E301"), "constructor pattern arity mismatch")),
        }
    }

    fn pattern_arguments(
        &mut self,
        args: &[Pattern],
        payload: &[Ty],
        span: Span,
    ) -> Result<(), Diagnostic> {
        if args.len() != payload.len() {
            return Err(self.error(span, Some("E301"), "constructor pattern arity mismatch"));
        }
        for (pattern, ty) in args.iter().zip(payload) {
            self.pattern(pattern, ty)?;
        }
        Ok(())
    }

    fn condition(&mut self, node: &Node) -> Result<(), Diagnostic> {
        let ty = self.expression(node)?;
        self.unify(&ty, &Ty::Bool, node.span)
    }

    fn branch_result(&mut self, node: &Node, expected: &Ty) -> Result<(), Diagnostic> {
        let ty = self.expression(node)?;
        self.unify(&ty, expected, node.span)
    }

    fn conditions(
        &mut self,
        arms: &[(Node, Node)],
        fallback: Option<&Node>,
    ) -> Result<Ty, Diagnostic> {
        let result = self.inference.fresh();
        for (condition, body) in arms {
            self.condition(condition)?;
            self.branch_result(body, &result)?;
        }
        if let Some(body) = fallback {
            self.branch_result(body, &result)?;
        }
        Ok(self.inference.resolve(&result))
    }

    fn function(&mut self, params: &[Param], body: &Node) -> Result<Ty, Diagnostic> {
        let saved = self.values.clone();
        let result = self.function_body(params, body);
        self.values = saved;
        result
    }

    fn function_body(&mut self, params: &[Param], body: &Node) -> Result<Ty, Diagnostic> {
        let mut args = Vec::new();
        for param in params {
            let ty = match &param.ty {
                Some(annotation) => self.annotation(annotation, param.span)?,
                None => self.inference.fresh(),
            };
            self.values
                .insert(param.name.clone(), Scheme::monomorphic(ty.clone()));
            args.push(ty);
        }
        let result = self.expression(body)?;
        Ok(Ty::Fn(args, Box::new(result)))
    }

    fn pipe(&mut self, value: &Node, target: &Node) -> Result<Ty, Diagnostic> {
        let (callee, mut args) = match &target.kind {
            NodeKind::Apply(callee, args) => (callee.as_ref(), args.clone()),
            _ => (target, Vec::new()),
        };
        args.insert(0, value.clone());
        self.apply(callee, &args, value.span)
    }

    fn callee(&mut self, node: &Node) -> Result<Ty, Diagnostic> {
        if let NodeKind::Name(name) = &node.kind {
            if name.starts_with("__") {
                if !self.trusted_start.is_some_and(|line| node.span.line >= line) && !std::path::Path::new(self.path)
                    .components()
                    .any(|part| part.as_os_str() == "stdlib")
                {
                    return Err(self.error(
                        node.span,
                        Some("E202"),
                        "intrinsics are restricted to stdlib",
                    ));
                }
                let function = crate::runtime_abi::RuntimeFunction::from_intrinsic(name)
                    .ok_or_else(|| self.error(node.span, Some("E204"), "unknown intrinsic"))?;
                let signature = function.value_signature().ok_or_else(|| self.error(node.span, Some("E204"), "intrinsic has no value signature"))?;
                return Ok(Ty::Fn(signature.parameters.iter().map(Ty::from).collect(), Box::new(Ty::from(&signature.result))));
            }
        }
        self.expression(node)
    }

    fn apply(&mut self, callee: &Node, args: &[Node], span: Span) -> Result<Ty, Diagnostic> {
        if let NodeKind::Name(name) = &callee.kind {
            if self.constructors.contains_key(name) {
                return self.construct(name, args, callee.span);
            }
        }
        let function = self.callee(callee)?;
        let args = args
            .iter()
            .map(|arg| self.expression(arg))
            .collect::<Result<Vec<_>, _>>()?;
        let result = self.inference.fresh();
        self.unify(&function, &Ty::Fn(args, Box::new(result.clone())), span)?;
        Ok(self.inference.resolve(&result))
    }

    fn construct(&mut self, name: &str, args: &[Node], span: Span) -> Result<Ty, Diagnostic> {
        let scheme = self.constructors[name].clone();
        // Constructor schemes describe payloads internally; source expressions yield only data.
        let (payloads, result) = match self.inference.instantiate(&scheme) {
            Ty::Fn(payloads, result) => (payloads, *result),
            result => (Vec::new(), result),
        };
        if args.len() != payloads.len() {
            return Err(self.error(span, Some("E301"), "constructor arity mismatch"));
        }
        for (arg, payload) in args.iter().zip(payloads) {
            let actual = self.expression(arg)?;
            self.unify(&actual, &payload, span)?;
        }
        let result = self.inference.resolve(&result);
        self.value_types.insert((span.line, span.col), result.clone());
        Ok(result)
    }

    fn propagation(
        &mut self,
        pattern: Option<&Pattern>,
        value: &Node,
        body: &Node,
    ) -> Result<Ty, Diagnostic> {
        let container = self.expression(value)?;
        if matches!(self.inference.resolve(&container), Ty::Var(_)) {
            let payload = self.inference.fresh();
            let saved = self.values.clone();
            let checked = self.propagation_body(pattern, &payload, body);
            self.values = saved;
            let result = checked?;
            let (_, input) = self.propagation_types(&result, value.span)?;
            let Ty::Adt(name, mut args) = input else { unreachable!() };
            args[0] = payload;
            self.unify(&container, &Ty::Adt(name, args), value.span)?;
            return Ok(self.inference.resolve(&result));
        }
        let (payload, result) =
            self.propagation_types(&container, value.span)?;
        let saved = self.values.clone();
        let checked = self.propagation_body(pattern, &payload, body);
        self.values = saved;
        let body_type = checked?;
        self.unify(&body_type, &result, body.span)?;
        Ok(self.inference.resolve(&result))
    }

    fn constructor_groups(&self) -> HashMap<String, Vec<(String, usize)>> {
        let mut groups: HashMap<String, Vec<(String, usize)>> = HashMap::new();
        for (name, scheme) in &self.constructors {
            let (result, arity) = match scheme.body() {
                Ty::Fn(args, result) => (result.as_ref(), args.len()),
                ty => (ty, 0),
            };
            if let Ty::Adt(result, _) = result {
                groups.entry(result.clone()).or_default().push((name.clone(), arity));
            }
        }
        groups
    }

    fn check_irrefutable(&mut self, pattern: &Pattern) {
        let groups = self.constructor_groups();
        let families = groups.values().flat_map(|group|
            group.iter().map(|(name, _)| (name.clone(), group.clone()))).collect();
        if crate::coverage::covers_pattern(pattern, &families) != Some(true) {
            self.deferred.push(self.error(pattern.span, Some("E402"), "binding pattern does not cover its type"));
        }
    }

    fn propagation_body(
        &mut self,
        pattern: Option<&Pattern>,
        payload: &Ty,
        body: &Node,
    ) -> Result<Ty, Diagnostic> {
        if let Some(pattern) = pattern {
            self.pattern(pattern, payload)?;
            self.check_irrefutable(pattern);
        }
        self.expression(body)
    }

    fn propagation_types(
        &mut self,
        container: &Ty,
        span: Span,
    ) -> Result<(Ty, Ty), Diagnostic> {
        match self.inference.resolve(container) {
            Ty::Adt(name, args) if name == "Option" && args.len() == 1 => {
                let output = self.inference.fresh();
                Ok((args[0].clone(), Ty::Adt(name, vec![output])))
            }
            Ty::Adt(name, args) if name == "Result" && args.len() == 2 => {
                let output = self.inference.fresh();
                Ok((
                    args[0].clone(),
                    Ty::Adt(name, vec![output, args[1].clone()]),
                ))
            }
            _ => Err(self.error(
                span,
                Some("E303"),
                "propagation requires Option or Result",
            )),
        }
    }

    fn binding(&mut self, pat: &Pattern, value: &Node, body: &Node) -> Result<Ty, Diagnostic> {
        let ty = self.expression(value)?;
        let saved = self.values.clone();
        self.pattern(pat, &ty)?;
        self.check_irrefutable(pat);
        let result = self.expression(body);
        self.values = saved;
        result
    }

    fn annotation(&self, annotation: &TypeExpr, span: Span) -> Result<Ty, Diagnostic> {
        self.annotation_inner(annotation, span, &mut Vec::new())
    }

    fn annotation_inner(&self, annotation: &TypeExpr, span: Span, visiting: &mut Vec<String>) -> Result<Ty, Diagnostic> {
        if let TypeKind::Name(name) = &annotation.kind {
            if let Some(target) = self.aliases.get(name) {
                if visiting.contains(name) {
                    return Err(self.error(annotation.span, None, "cyclic abstract type instantiation has no specified diagnostic"));
                }
                visiting.push(name.clone());
                let result = self.annotation_inner(target, target.span, visiting);
                visiting.pop();
                return result;
            }
        }
        match &annotation.kind {
            TypeKind::Name(name) if self.abstract_types.get(name).is_some_and(|(arity, _)| *arity == 0) => {
                Ok(Ty::Abstract(self.abstract_types[name].1.clone(), vec![]))
            }
            TypeKind::Apply(name, args) if self.abstract_types.get(name).is_some_and(|(arity, _)| *arity == args.len()) => {
                let args = args
                    .iter()
                    .map(|arg| self.annotation_inner(arg, span, visiting))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Ty::Abstract(self.abstract_types[name].1.clone(), args))
            }
            TypeKind::Apply(name, args) if name == "Fn" && args.len() == 2 => {
                let inputs = match &args[0].kind {
                    TypeKind::Group(inputs) => inputs.as_slice(),
                    _ => std::slice::from_ref(&args[0]),
                };
                let inputs = inputs
                    .iter()
                    .map(|input| self.annotation_inner(input, span, visiting))
                    .collect::<Result<Vec<_>, _>>()?;
                let output = self.annotation_inner(&args[1], span, visiting)?;
                Ok(Ty::Fn(inputs, Box::new(output)))
            }
            TypeKind::Name(name) if self.type_arities.get(name) == Some(&0) => {
                Ok(Ty::Adt(name.clone(), vec![]))
            }
            TypeKind::Apply(name, args) if self.type_arities.get(name) == Some(&args.len()) => {
                let args = args
                    .iter()
                    .map(|arg| self.annotation_inner(arg, span, visiting))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Ty::Adt(name.clone(), args))
            }
            TypeKind::Name(name) => match name.as_str() {
                "Int" => Ok(Ty::Int),
                "Float" => Ok(Ty::Float),
                "Bool" => Ok(Ty::Bool),
                "Char" => Ok(Ty::Char),
                "String" => Ok(Ty::String),
                "Unit" => Ok(Ty::Unit),
                _ => Err(self.error(span, Some("E204"), &format!("unknown type {name}"))),
            },
            _ => Err(self.error(span, None, "type annotation is not implemented")),
        }
    }

    fn default_numbers(&mut self) -> Result<(), Diagnostic> {
        for (op, operand, result, span) in std::mem::take(&mut self.divisions) {
            if matches!(self.inference.resolve(&operand), Ty::Var(_)) {
                self.unify(&operand, &Ty::Int, span)?;
            }
            let operand = self.inference.resolve(&operand);
            let expected = self.numeric_binary(op, &operand, span)?;
            self.unify(&result, &expected, span)?;
        }
        for (ty, span) in std::mem::take(&mut self.numeric_constraints) {
            match self.inference.resolve(&ty) {
                Ty::Var(_) => self.unify(&ty, &Ty::Int, span)?,
                Ty::Int | Ty::Float => {}
                _ => return Err(self.error(span, Some("E305"), "expected Int or Float")),
            }
        }
        Ok(())
    }

    fn numeric(&mut self, ty: &Ty, span: Span) -> Result<(), Diagnostic> {
        match self.inference.resolve(ty) {
            Ty::Int | Ty::Float => Ok(()),
            Ty::Var(_) => {
                self.numeric_constraints.push((ty.clone(), span));
                Ok(())
            }
            _ => Err(self.error(span, Some("E305"), "expected Int or Float")),
        }
    }

    fn binary(&mut self, op: BinOp, a: &Node, b: &Node) -> Result<Ty, Diagnostic> {
        let left = self.expression(a)?;
        let right = self.expression(b)?;
        self.unify(&left, &right, a.span)?;
        match op {
            BinOp::Eq | BinOp::Ne => {
                self.comparison_constraints.push((left.clone(), a.span, "E703"));
                Ok(Ty::Bool)
            }
            BinOp::And | BinOp::Or => {
                self.unify(&left, &Ty::Bool, a.span)?;
                Ok(Ty::Bool)
            }
            _ => self.numeric_binary(op, &left, a.span),
        }
    }

    fn contains_function(&self, ty: &Ty, visited: &mut Vec<Ty>) -> bool {
        if matches!(ty, Ty::Fn(..)) { return true; }
        if visited.contains(ty) { return false; }
        visited.push(ty.clone());
        if let Ty::Abstract(name, _) = ty {
            return self.abstract_representations.get(name).is_some_and(|representation|
                self.contains_function(representation, visited));
        }
        let Ty::Adt(name, _) = ty else { return false; };
        self.constructors.values()
            .chain(self.records.values().map(|(_, scheme)| scheme))
            .chain(self.module_constructors.values().map(|(_, scheme)| scheme))
            .chain(self.module_records.values().map(|(_, scheme)| scheme))
            .any(|scheme| {
            let mut inference = self.inference.clone();
            let instantiated = inference.instantiate(scheme);
            let Ty::Fn(payload, result) = instantiated else { return false; };
            if !matches!(&*result, Ty::Adt(result, _) if result == name) || inference.unify(&result, ty).is_err() { return false; }
            payload.iter().any(|field| self.contains_function(&inference.resolve(field), visited))
        })
    }

    fn numeric_binary(&mut self, op: BinOp, ty: &Ty, span: Span) -> Result<Ty, Diagnostic> {
        self.numeric(ty, span)?;
        let ty = self.inference.resolve(ty);
        if matches!(op, BinOp::Div | BinOp::Mod) && matches!(ty, Ty::Var(_)) {
            let result = self.inference.fresh();
            self.divisions.push((op, ty, result.clone(), span));
            return Ok(result);
        }
        match op {
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => Ok(Ty::Bool),
            BinOp::Mod if ty == Ty::Float => {
                Err(self.error(span, Some("E305"), "Float has no remainder operator"))
            }
            BinOp::Div | BinOp::Mod if ty == Ty::Int => Ok(Ty::Adt(
                "Result".into(),
                vec![Ty::Int, Ty::Adt("DivisionError".into(), vec![])],
            )),
            _ => Ok(ty.clone()),
        }
    }
}
