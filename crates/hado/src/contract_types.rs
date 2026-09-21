use crate::parser::{Ast, Diagnostic, ImplementationForm, Item, TypeExpr, TypeKind};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(crate) struct Contracts {
    implementations: BTreeMap<String, Vec<String>>,
    pub types: BTreeMap<String, Option<String>>,
}

impl Contracts {
    pub fn collect(items: &[Item]) -> Self {
        let mut result = Self::default();
        for item in items {
            result.collect_implementation(item);
        }
        for item in items {
            result.collect_signature_types(item);
        }
        result
    }

    /// 通常の実装だけを数える。functor の本体は適用のたびに展開されるので数えない。
    fn collect_implementation(&mut self, item: &Item) {
        let Item::ModuleImpl(module) = item else {
            return;
        };
        if matches!(&module.form, ImplementationForm::Functor(_)) {
            return;
        }
        self.implementations
            .entry(module.sig.clone())
            .or_default()
            .push(module.name.clone());
    }

    /// signature が宣言する型を、その contract の実装の同名の型へ結びつける。
    fn collect_signature_types(&mut self, item: &Item) {
        let Item::ModuleSig(signature) = item else {
            return;
        };
        for item in &signature.items {
            if let Item::TypeDef(definition) = item {
                let implementation = self.implementation(&signature.name).map(str::to_owned);
                self.types.insert(
                    format!("{}.{}", signature.name, definition.name),
                    implementation.map(|name| format!("{name}.{}", definition.name)),
                );
            }
        }
    }

    pub fn implementation(&self, signature: &str) -> Option<&str> {
        match self.implementations.get(signature).map(Vec::as_slice) {
            Some([name]) => Some(name),
            _ => None,
        }
    }

    pub fn diagnostics(&self, ast: &Ast, path: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for item in &ast.items {
            if let Item::ModuleSig(signature) = item {
                for item in &signature.items {
                    if let Item::SigDef(member) = item {
                        for ty in member
                            .params
                            .iter()
                            .filter_map(|param| param.ty.as_ref())
                            .chain(member.ret.iter())
                        {
                            self.check_type(ty, path, &mut diagnostics);
                        }
                    }
                }
            }
        }
        diagnostics
    }

    fn check_type(&self, ty: &TypeExpr, path: &str, diagnostics: &mut Vec<Diagnostic>) {
        match &ty.kind {
            TypeKind::Name(name) | TypeKind::Apply(name, _)
                if self.types.get(name) == Some(&None) =>
            {
                diagnostics.push(Diagnostic {
                    related: None,
                    path: path.into(),
                    line: ty.span.line,
                    col: ty.span.col,
                    end_line: ty.span.line,
                    end_col: ty.span.col,
                    code: Some("E603".into()),
                    message: "contract type requires exactly one implementation".into(),
                });
            }
            _ => {}
        }
        match &ty.kind {
            TypeKind::Apply(_, args) | TypeKind::Group(args) => {
                for arg in args {
                    self.check_type(arg, path, diagnostics);
                }
            }
            _ => {}
        }
    }
}
