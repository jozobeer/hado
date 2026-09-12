use super::*;
use crate::parser::DeclaredName;

impl Checker<'_> {
    fn named_schema(&self, name: &DeclaredName) -> Result<(Vec<String>, Scheme), Diagnostic> {
        let scheme = self.constructors.get(&name.name)
            .ok_or_else(|| self.error(name.span, Some("E204"), "unknown constructor"))?;
        let fields = self.named_constructors.get(&name.name)
            .ok_or_else(|| self.error(name.span, Some("E301"), "constructor does not have named fields"))?;
        Ok((fields.clone(), scheme.clone()))
    }

    pub(super) fn named_construct(&mut self, name: &DeclaredName, fields: &[(DeclaredName, Node)]) -> Result<Ty, Diagnostic> {
        let (declared, scheme) = self.named_schema(name)?;
        for (field, _) in fields {
            if !declared.contains(&field.name) {
                return Err(self.error(field.span, Some("E204"), "unknown constructor field"));
            }
        }
        if fields.len() != declared.len() {
            return Err(self.error(name.span, Some("E301"), "constructor field count mismatch"));
        }
        let Ty::Fn(payload, result, _) = self.inference.instantiate(&scheme) else { return Ok(Ty::Error); };
        for (field, value) in fields {
            let index = declared.iter().position(|candidate| candidate == &field.name).unwrap();
            self.branch_result(value, &self.reveal_shared(&payload[index]))?;
        }
        let result = self.inference.resolve(&result);
        self.value_types.insert((name.span.line, name.span.col), result.clone());
        Ok(result)
    }

    pub(super) fn named_pattern(&mut self, name: &DeclaredName, fields: &[(DeclaredName, Option<Pattern>)], expected: &Ty) -> Result<(), Diagnostic> {
        let (declared, scheme) = self.named_schema(name)?;
        for (field, _) in fields {
            if !declared.contains(&field.name) {
                return Err(self.error(field.span, Some("E204"), "unknown constructor field"));
            }
        }
        let Ty::Fn(payload, result, _) = self.inference.instantiate(&scheme) else { return Ok(()); };
        self.unify(&result, expected, name.span)?;
        for (field, nested) in fields {
            let index = declared.iter().position(|candidate| candidate == &field.name).unwrap();
            let payload = self.reveal_shared(&payload[index]);
            if let Some(pattern) = nested { self.pattern(pattern, &payload)?; }
            else { self.values.insert(field.name.clone(), Scheme::monomorphic(payload)); }
        }
        Ok(())
    }

    pub(super) fn has_named_variant(&self, ty: &str) -> bool {
        self.constructor_groups().get(ty).is_some_and(|group| group.iter().any(|(name, _)| self.named_constructors.contains_key(name)))
    }

    pub(super) fn named_projection_schema(&self, ty: &str) -> Option<(Vec<String>, Scheme)> {
        let groups = self.constructor_groups();
        let group = groups.get(ty)?;
        if group.len() != 1 { return None; }
        let name = &group[0].0;
        Some((self.named_constructors.get(name)?.clone(), self.constructors.get(name)?.clone()))
    }

    pub(super) fn named_projection_schemas(&self) -> Vec<(Vec<String>, Scheme)> {
        self.constructor_groups().keys().filter_map(|name| self.named_projection_schema(name)).collect()
    }
}
