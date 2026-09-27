use crate::parser::{Diagnostic, Item, SigDef, Span, TypeDef, TypeKind};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub(crate) struct AmbiguousDeclarations {
    pub declared_types: HashSet<String>,
    pub declared_constructors: HashSet<String>,
    pub declared_modules: HashSet<String>,
    pub declared_signatures: HashSet<String>,
    pub modules: HashSet<String>,
    pub signatures: HashSet<String>,
    pub values: HashSet<String>,
    pub types: HashSet<String>,
    pub constructors: HashSet<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// 走査のあいだに「その名前を最初に宣言した位置」を種別ごとに覚える。
/// 名前空間は種別ごとに別なので map も種別ごとに分ける。
#[derive(Default)]
struct Seen {
    values: HashMap<String, Span>,
    types: HashMap<String, Span>,
    constructors: HashMap<String, Span>,
    modules: HashMap<String, Span>,
    signatures: HashMap<String, Span>,
}

impl AmbiguousDeclarations {
    pub fn collect(items: &[Item], path: &str) -> Self {
        let mut result = Self::default();
        let mut seen = Seen::default();
        for item in items {
            result.declare(item, &mut seen, path);
        }
        result.collect_nested(items, path);
        result.declared_modules = seen.modules.into_keys().collect();
        result.declared_signatures = seen.signatures.into_keys().collect();
        result.declared_types = seen.types.into_keys().collect();
        result.declared_constructors = seen.constructors.into_keys().collect();
        result
    }

    /// 宣言 1 つをその名前空間へ記録する。腕の本体は種別ごとの関数に置く。
    fn declare(&mut self, item: &Item, seen: &mut Seen, path: &str) {
        match item {
            Item::Def(def) => record(
                &def.name,
                def.name_span,
                &mut seen.values,
                &mut self.values,
                &mut self.diagnostics,
                path,
            ),
            Item::SigDef(def) => {
                record(
                    &def.name,
                    def.name_span,
                    &mut seen.values,
                    &mut self.values,
                    &mut self.diagnostics,
                    path,
                );
                self.declare_type_parameters(def, path);
            }
            Item::ModuleImpl(module) => record(
                &module.name,
                module.name_span,
                &mut seen.modules,
                &mut self.modules,
                &mut self.diagnostics,
                path,
            ),
            Item::ModuleSig(signature) => record(
                &signature.name,
                signature.name_span,
                &mut seen.signatures,
                &mut self.signatures,
                &mut self.diagnostics,
                path,
            ),
            Item::TypeDef(def) => {
                record(
                    &def.name,
                    def.name_span,
                    &mut seen.types,
                    &mut self.types,
                    &mut self.diagnostics,
                    path,
                );
                self.declare_constructors(def, seen, path);
            }
            Item::Effect(_) | Item::Handler(_) => {}
        }
    }

    /// `SigDef` の型パラメータは専用の名前空間で重複だけを見る。
    fn declare_type_parameters(&mut self, def: &SigDef, path: &str) {
        let mut seen = HashMap::new();
        let mut repeated = HashSet::new();
        for parameter in &def.type_params {
            record(
                &parameter.name,
                parameter.span,
                &mut seen,
                &mut repeated,
                &mut self.diagnostics,
                path,
            );
        }
    }

    /// 型定義の構築子を、構築子の名前空間へ記録する。
    fn declare_constructors(&mut self, def: &TypeDef, seen: &mut Seen, path: &str) {
        for (name, span) in constructor_names(def) {
            record(
                name,
                span,
                &mut seen.constructors,
                &mut self.constructors,
                &mut self.diagnostics,
                path,
            );
        }
    }

    /// 内側の `ModuleSig` と、型名が重複した `TypeDef` の構築子を拾う。
    fn collect_nested(&mut self, items: &[Item], path: &str) {
        for item in items {
            self.collect_nested_item(item, path);
        }
    }

    fn collect_nested_item(&mut self, item: &Item, path: &str) {
        if let Item::ModuleSig(signature) = item {
            let inner = Self::collect(&signature.items, path);
            if !inner.diagnostics.is_empty() {
                self.signatures.insert(signature.name.clone());
            }
            self.diagnostics.extend(inner.diagnostics);
        }
        if let Item::TypeDef(def) = item {
            if self.types.contains(&def.name) {
                self.constructors.extend(
                    constructor_names(def)
                        .into_iter()
                        .map(|(name, _)| name.clone()),
                );
            }
        }
    }
}

fn record(
    name: &str,
    span: Span,
    seen: &mut HashMap<String, Span>,
    ambiguous: &mut HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
) {
    // §56: the duplicate names the first declaration of the same name.
    if let Some(&first) = seen.get(name) {
        ambiguous.insert(name.into());
        diagnostics.push(Diagnostic {
            mentions: Vec::new(),
            suggestion: None,
            related: Some(Box::new(crate::parser::Related {
                path: path.into(),
                line: first.line,
                col: first.col,
            })),
            path: path.into(),
            line: span.line,
            col: span.col,
            end_line: span.line,
            end_col: span.col,
            code: Some("E205".into()),
            message: format!("duplicate declaration {name}"),
        });
        return;
    }
    seen.insert(name.into(), span);
}

fn constructor_names(def: &crate::parser::TypeDef) -> Vec<(&String, Span)> {
    match def.ty.as_ref().map(|ty| &ty.kind) {
        Some(TypeKind::Adt(variants)) => variants
            .iter()
            .map(|variant| (&variant.name.name, variant.name.span))
            .collect(),
        _ => Vec::new(),
    }
}
