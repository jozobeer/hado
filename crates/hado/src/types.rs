use crate::parser::{Expr, Program};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Error,
    Int,
    Float,
    Bool,
    Char,
    String,
    Bytes,
    Void,
    Var(u32),
    Fn(Vec<Ty>, Box<Ty>, Box<Row>),
    Adt(String, Vec<Ty>),
    Abstract(String, Vec<Ty>),
}

/// The names of §19's primitive types that an annotation resolves without a
/// user declaration. Shared by position annotations and `shared` realizations
/// so the two cannot drift apart.
pub fn builtin_type(name: &str) -> Option<Ty> {
    Some(match name {
        "Int" => Ty::Int,
        "Float" => Ty::Float,
        "Bool" => Ty::Bool,
        "Char" => Ty::Char,
        "String" => Ty::String,
        "Bytes" => Ty::Bytes,
        "Void" => Ty::Void,
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Var(u32),
    Closed(Vec<Ty>),
    Open(u32, Vec<Ty>),
}

impl Row {
    pub fn pure() -> Self {
        Self::Closed(Vec::new())
    }

    pub fn suffix(&self) -> String {
        let labels = match self {
            Self::Closed(labels) | Self::Open(_, labels) => labels,
            Self::Var(_) => return String::new(),
        };
        let names: BTreeSet<_> = labels.iter().map(ToString::to_string).collect();
        if names.is_empty() {
            String::new()
        } else {
            format!(" {{{}}}", names.into_iter().collect::<Vec<_>>().join(", "))
        }
    }

    pub fn without_effect(&self, effect: &str) -> Self {
        let drop = |labels: &[Ty]| {
            labels
                .iter()
                .filter(|ty| !label_matches_effect(&ty.to_string(), effect))
                .cloned()
                .collect()
        };
        match self {
            Self::Closed(labels) => Self::Closed(drop(labels)),
            Self::Open(id, labels) => Self::Open(*id, drop(labels)),
            row => row.clone(),
        }
    }

    pub fn has_known_effects(&self) -> bool {
        match self {
            Self::Closed(labels) | Self::Open(_, labels) => !labels.is_empty(),
            Self::Var(_) => false,
        }
    }

    pub fn has_user_effect(&self, prelude_effects: &std::collections::HashSet<String>) -> bool {
        self.labels()
            .iter()
            .any(|ty| !prelude_effects.contains(&effect_ctor(ty)))
    }

    fn labels(&self) -> &[Ty] {
        match self {
            Self::Closed(labels) | Self::Open(_, labels) => labels,
            Self::Var(_) => &[],
        }
    }

    fn free_variables(&self, result: &mut BTreeSet<u32>) {
        match self {
            Self::Var(id) | Self::Open(id, _) => {
                result.insert(*id);
            }
            Self::Closed(_) => {}
        }
    }

    fn map_variables(&self, f: &impl Fn(u32) -> Ty) -> Self {
        let labels = self.labels().iter().map(|ty| ty.map_variables(f)).collect();
        match self {
            Self::Var(id) => match f(*id) {
                Ty::Var(id) => Self::Var(id),
                _ => self.clone(),
            },
            Self::Closed(_) => Self::Closed(labels),
            Self::Open(id, _) => match f(*id) {
                Ty::Var(id) => Self::Open(id, labels),
                _ => Self::Closed(labels),
            },
        }
    }
}

impl Ty {
    pub fn fn_of(args: Vec<Ty>, ret: Ty) -> Self {
        Self::Fn(args, Box::new(ret), Box::new(Row::pure()))
    }

    pub fn fn_with(args: Vec<Ty>, ret: Ty, row: Row) -> Self {
        Self::Fn(args, Box::new(ret), Box::new(row))
    }
}

/// M0: every literal is `Int` (`i64`), `+` keeps `Int`.
pub fn check(prog: &Program) -> Result<Ty, String> {
    check_expr(&prog.body)
}

fn check_expr(expr: &Expr) -> Result<Ty, String> {
    match expr {
        Expr::Int(_) => Ok(Ty::Int),
        Expr::Add(l, r) => {
            let lt = check_expr(l)?;
            let rt = check_expr(r)?;
            if lt == Ty::Int && rt == Ty::Int {
                Ok(Ty::Int)
            } else {
                Err(format!("+ needs Int, found {lt:?} and {rt:?}"))
            }
        }
    }
}

use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub enum TypeError {
    Mismatch,
    Arity,
    Occurs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scheme {
    variables: BTreeSet<u32>,
    ty: Ty,
    constraints: Vec<ComparisonConstraint>,
    row_constraints: Vec<RowConstraint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonConstraint {
    pub ty: Ty,
    pub code: &'static str,
    pub span: Option<crate::lexer::Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowConstraint {
    pub row: Row,
    pub bound: Row,
    pub code: &'static str,
    pub span: Option<crate::lexer::Span>,
}

impl ComparisonConstraint {
    fn with_type(&self, ty: Ty) -> Self {
        Self {
            ty,
            code: self.code,
            span: self.span,
        }
    }
}

impl RowConstraint {
    fn map_variables(&self, f: &impl Fn(u32) -> Ty) -> Self {
        Self {
            row: self.row.map_variables(f),
            bound: self.bound.map_variables(f),
            code: self.code,
            span: self.span,
        }
    }
}

#[derive(Default, Clone)]
pub struct Inference {
    next: u32,
    substitutions: HashMap<u32, Ty>,
    rows: HashMap<u32, Row>,
}

impl Ty {
    pub(crate) fn has_error(&self) -> bool {
        match self {
            Ty::Error => true,
            Ty::Fn(args, result, _) => args.iter().any(Ty::has_error) || result.has_error(),
            Ty::Adt(_, args) | Ty::Abstract(_, args) => args.iter().any(Ty::has_error),
            _ => false,
        }
    }

    pub fn is_monomorphic(&self) -> bool {
        let mut variables = BTreeSet::new();
        self.free_variables(&mut variables);
        variables.is_empty()
    }

    fn map_variables(&self, f: &impl Fn(u32) -> Ty) -> Ty {
        match self {
            Ty::Var(id) => f(*id),
            Ty::Fn(args, ret, row) => Ty::Fn(
                args.iter().map(|ty| ty.map_variables(f)).collect(),
                Box::new(ret.map_variables(f)),
                Box::new(row.map_variables(f)),
            ),
            Ty::Abstract(name, args) => Ty::Abstract(
                name.clone(),
                args.iter().map(|ty| ty.map_variables(f)).collect(),
            ),
            Ty::Adt(name, args) => Ty::Adt(
                name.clone(),
                args.iter().map(|ty| ty.map_variables(f)).collect(),
            ),
            ty => ty.clone(),
        }
    }

    fn free_variables(&self, result: &mut BTreeSet<u32>) {
        match self {
            Ty::Var(id) => {
                result.insert(*id);
            }
            Ty::Fn(args, ret, row) => {
                for arg in args {
                    arg.free_variables(result);
                }
                ret.free_variables(result);
                row.free_variables(result);
                for ty in row.labels() {
                    ty.free_variables(result);
                }
            }
            Ty::Adt(_, args) | Ty::Abstract(_, args) => {
                for arg in args {
                    arg.free_variables(result);
                }
            }
            _ => {}
        }
    }
}

impl Inference {
    pub fn fresh(&mut self) -> Ty {
        let variable = Ty::Var(self.next);
        self.next += 1;
        variable
    }

    pub(crate) fn freshen_errors(&mut self, ty: &Ty) -> Ty {
        match ty {
            Ty::Error => self.fresh(),
            Ty::Fn(args, result, row) => Ty::Fn(
                args.iter().map(|arg| self.freshen_errors(arg)).collect(),
                Box::new(self.freshen_errors(result)),
                Box::new(row.as_ref().clone()),
            ),
            Ty::Adt(name, args) => Ty::Adt(
                name.clone(),
                args.iter().map(|arg| self.freshen_errors(arg)).collect(),
            ),
            Ty::Abstract(name, args) => Ty::Abstract(
                name.clone(),
                args.iter().map(|arg| self.freshen_errors(arg)).collect(),
            ),
            ty => ty.clone(),
        }
    }

    pub fn resolve(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Var(id) => match self.substitutions.get(id) {
                Some(value) => self.resolve(value),
                None => Ty::Var(*id),
            },
            Ty::Fn(args, ret, row) => Ty::Fn(
                args.iter().map(|ty| self.resolve(ty)).collect(),
                Box::new(self.resolve(ret)),
                Box::new(self.resolve_row(row)),
            ),
            Ty::Adt(name, args) => Ty::Adt(
                name.clone(),
                args.iter().map(|ty| self.resolve(ty)).collect(),
            ),
            Ty::Abstract(name, args) => Ty::Abstract(
                name.clone(),
                args.iter().map(|ty| self.resolve(ty)).collect(),
            ),
            ty => ty.clone(),
        }
    }

    pub fn resolve_row(&self, row: &Row) -> Row {
        match row {
            Row::Var(id) => match self.rows.get(id) {
                Some(value) => self.resolve_row(value),
                None => Row::Var(*id),
            },
            Row::Closed(labels) => Row::Closed(labels.iter().map(|ty| self.resolve(ty)).collect()),
            Row::Open(id, labels) => {
                let labels: Vec<_> = labels.iter().map(|ty| self.resolve(ty)).collect();
                match self.resolve_row(&Row::Var(*id)) {
                    Row::Var(id) if labels.is_empty() => Row::Var(id),
                    Row::Var(id) => Row::Open(id, labels),
                    Row::Closed(more) => Row::Closed(self.merge_labels(&labels, &more)),
                    Row::Open(id, more) => Row::Open(id, self.merge_labels(&labels, &more)),
                }
            }
        }
    }

    pub fn fresh_row(&mut self) -> Row {
        let row = Row::Var(self.next);
        self.next += 1;
        row
    }

    fn occurs(&self, id: u32, ty: &Ty) -> bool {
        let mut variables = BTreeSet::new();
        self.resolve(ty).free_variables(&mut variables);
        variables.contains(&id)
    }

    fn bind(&mut self, id: u32, ty: &Ty) -> Result<(), TypeError> {
        if self.occurs(id, ty) {
            return Err(TypeError::Occurs);
        }
        self.substitutions.insert(id, ty.clone());
        Ok(())
    }

    pub fn unify(&mut self, left: &Ty, right: &Ty) -> Result<(), TypeError> {
        let saved = (self.substitutions.clone(), self.rows.clone());
        let result = self.unify_inner(left, right);
        if result.is_err() {
            self.substitutions = saved.0;
            self.rows = saved.1;
        }
        result
    }

    pub fn unify_row(&mut self, left: &Row, right: &Row) -> Result<(), TypeError> {
        let saved = (self.substitutions.clone(), self.rows.clone());
        let result = self.unify_row_inner(left, right);
        if result.is_err() {
            self.substitutions = saved.0;
            self.rows = saved.1;
        }
        result
    }

    pub fn row_contained(&mut self, inner: &Row, outer: &Row) -> Result<(), TypeError> {
        let inner = self.resolve_row(inner);
        let outer = self.resolve_row(outer);
        match (&inner, &outer) {
            (Row::Closed(a), Row::Closed(b))
                if self.label_names(a).is_subset(&self.label_names(b)) =>
            {
                self.unify_matching_labels(a, b)
            }
            (Row::Open(_, required), Row::Closed(actual))
                if self
                    .label_names(required)
                    .is_subset(&self.label_names(actual)) =>
            {
                self.unify_matching_labels(required, actual)
            }
            _ => self.unify_row(&inner, &outer),
        }
    }

    fn unify_row_inner(&mut self, left: &Row, right: &Row) -> Result<(), TypeError> {
        let left = self.resolve_row(left);
        let right = self.resolve_row(right);
        if left == right {
            return Ok(());
        }
        match (&left, &right) {
            (Row::Var(id), row) | (row, Row::Var(id)) => self.bind_row(*id, row),
            (Row::Closed(a), Row::Closed(b)) => self.unify_closed_labels(a, b),
            (Row::Open(id, required), Row::Closed(actual))
            | (Row::Closed(actual), Row::Open(id, required)) => {
                self.unify_open_closed(*id, required, actual)
            }
            (Row::Open(a, left_labels), Row::Open(b, right_labels)) => {
                let labels = self.merge_labels(left_labels, right_labels);
                let tail = self.next;
                self.next += 1;
                self.bind_row(*a, &Row::Open(tail, labels.clone()))?;
                if a != b {
                    self.bind_row(*b, &Row::Open(tail, labels))?;
                }
                Ok(())
            }
        }
    }

    fn bind_row(&mut self, id: u32, row: &Row) -> Result<(), TypeError> {
        let mut variables = BTreeSet::new();
        self.resolve_row(row).free_variables(&mut variables);
        if variables.contains(&id) {
            return Err(TypeError::Occurs);
        }
        self.rows.insert(id, row.clone());
        Ok(())
    }

    pub fn union_row(&mut self, left: &Row, right: &Row) -> Row {
        let left = self.resolve_row(left);
        let right = self.resolve_row(right);
        match (&left, &right) {
            (Row::Closed(a), Row::Closed(b)) => Row::Closed(self.merge_labels(a, b)),
            (Row::Var(id), Row::Closed(labels)) | (Row::Closed(labels), Row::Var(id))
                if labels.is_empty() =>
            {
                Row::Var(*id)
            }
            (Row::Var(id), Row::Closed(labels)) | (Row::Closed(labels), Row::Var(id)) => {
                Row::Open(*id, labels.clone())
            }
            (Row::Open(id, labels), Row::Closed(more))
            | (Row::Closed(more), Row::Open(id, labels)) => {
                Row::Open(*id, self.merge_labels(labels, more))
            }
            (Row::Var(id), Row::Open(tail, labels)) | (Row::Open(tail, labels), Row::Var(id)) => {
                let _ = self.unify_row_inner(&Row::Var(*id), &Row::Var(*tail));
                Row::Open(*id, labels.clone())
            }
            (Row::Open(a, left_labels), Row::Open(b, right_labels)) => {
                let _ = self.unify_row_inner(&Row::Var(*a), &Row::Var(*b));
                Row::Open(*a, self.merge_labels(left_labels, right_labels))
            }
            (Row::Var(a), Row::Var(b)) => {
                let _ = self.unify_row_inner(&Row::Var(*a), &Row::Var(*b));
                Row::Var(*a)
            }
        }
    }

    pub fn close_row(&mut self, row: &Row) -> Row {
        let row = self.resolve_row(row);
        match row {
            Row::Var(id) => {
                self.rows.entry(id).or_insert_with(Row::pure);
                Row::pure()
            }
            Row::Open(id, labels) => {
                self.rows.entry(id).or_insert_with(Row::pure);
                Row::Closed(labels)
            }
            closed => closed,
        }
    }

    fn merge_labels(&self, left: &[Ty], right: &[Ty]) -> Vec<Ty> {
        let mut labels = left.to_vec();
        labels.extend(right.iter().cloned());
        labels
    }

    fn label_names(&self, labels: &[Ty]) -> BTreeSet<String> {
        labels
            .iter()
            .map(|ty| effect_ctor(&self.resolve(ty)))
            .collect()
    }

    fn unify_closed_labels(&mut self, left: &[Ty], right: &[Ty]) -> Result<(), TypeError> {
        if self.label_names(left) != self.label_names(right) {
            return Err(TypeError::Mismatch);
        }
        self.unify_matching_labels(left, right)
    }

    fn unify_matching_labels(&mut self, left: &[Ty], right: &[Ty]) -> Result<(), TypeError> {
        let right = self.labels_by_ctor(right);
        for ty in left {
            let ty = self.resolve(ty);
            if let Some(other) = right.get(&effect_ctor(&ty)) {
                self.unify_inner(&ty, other)?;
            }
        }
        Ok(())
    }

    fn unify_open_closed(
        &mut self,
        tail: u32,
        required: &[Ty],
        actual: &[Ty],
    ) -> Result<(), TypeError> {
        let required_names = self.label_names(required);
        let actual_names = self.label_names(actual);
        if !required_names.is_subset(&actual_names) {
            return Err(TypeError::Mismatch);
        }
        self.unify_matching_labels(required, actual)?;
        let rest: Vec<_> = actual
            .iter()
            .filter(|&ty| !required_names.contains(&effect_ctor(&self.resolve(ty))))
            .cloned()
            .collect();
        self.bind_row(
            tail,
            &if rest.is_empty() {
                Row::pure()
            } else {
                Row::Closed(rest)
            },
        )
    }

    pub fn row_within(&self, inner: &Row, outer: &Row) -> bool {
        let inner = self.resolve_row(inner);
        let outer = self.resolve_row(outer);
        if matches!(inner, Row::Var(_)) || matches!(outer, Row::Var(_)) {
            return true;
        }
        if !self
            .label_names(inner.labels())
            .is_subset(&self.label_names(outer.labels()))
        {
            return false;
        }
        let mut probe = self.clone();
        probe
            .unify_matching_labels(inner.labels(), outer.labels())
            .is_ok()
    }

    fn labels_by_ctor(&self, labels: &[Ty]) -> HashMap<String, Ty> {
        let mut map = HashMap::new();
        for ty in labels {
            let ty = self.resolve(ty);
            map.insert(effect_ctor(&ty), ty);
        }
        map
    }

    pub fn close_rows(&mut self, ty: &Ty) {
        let mut variables = BTreeSet::new();
        collect_row_vars(&self.resolve(ty), &mut variables);
        for id in variables {
            self.rows.entry(id).or_insert_with(Row::pure);
        }
    }

    fn unify_inner(&mut self, left: &Ty, right: &Ty) -> Result<(), TypeError> {
        let left = self.resolve(left);
        let right = self.resolve(right);
        if left == right {
            return Ok(());
        }
        match (&left, &right) {
            (Ty::Var(id), ty) | (ty, Ty::Var(id)) => self.bind(*id, ty),
            (Ty::Error, _) | (_, Ty::Error) => Ok(()),
            (Ty::Fn(a, ret_a, row_a), Ty::Fn(b, ret_b, row_b)) => {
                self.unify_arguments(a, b)?;
                self.unify_inner(ret_a, ret_b)?;
                self.unify_row_inner(row_a, row_b)
            }
            (Ty::Adt(a, args_a), Ty::Adt(b, args_b))
            | (Ty::Abstract(a, args_a), Ty::Abstract(b, args_b))
                if a == b =>
            {
                self.unify_arguments(args_a, args_b)
            }
            _ => Err(TypeError::Mismatch),
        }
    }

    fn unify_arguments(&mut self, a: &[Ty], b: &[Ty]) -> Result<(), TypeError> {
        if a.len() != b.len() {
            return Err(TypeError::Arity);
        }
        for (left, right) in a.iter().zip(b) {
            self.unify_inner(left, right)?;
        }
        Ok(())
    }

    pub fn generalize(&self, ty: &Ty, environment: &[Ty]) -> Scheme {
        let ty = self.resolve(ty);
        let mut variables = BTreeSet::new();
        let mut captured = BTreeSet::new();
        ty.free_variables(&mut variables);
        for binding in environment {
            self.resolve(binding).free_variables(&mut captured);
        }
        variables.retain(|id| !captured.contains(id));
        Scheme {
            variables,
            ty,
            constraints: vec![],
            row_constraints: vec![],
        }
    }

    pub fn generalize_constrained(
        &self,
        ty: &Ty,
        environment: &[Ty],
        constraints: &[ComparisonConstraint],
        row_constraints: &[RowConstraint],
    ) -> Scheme {
        let mut scheme = self.generalize(ty, environment);
        scheme.constraints = constraints
            .iter()
            .filter_map(|constraint| {
                let ty = self.resolve(&constraint.ty);
                let mut variables = BTreeSet::new();
                ty.free_variables(&mut variables);
                (!variables.is_disjoint(&scheme.variables)).then_some(constraint.with_type(ty))
            })
            .collect();
        scheme.row_constraints = row_constraints
            .iter()
            .filter_map(|constraint| {
                let row = self.resolve_row(&constraint.row);
                let bound = self.resolve_row(&constraint.bound);
                let mut variables = BTreeSet::new();
                row.free_variables(&mut variables);
                (!variables.is_disjoint(&scheme.variables)).then_some(RowConstraint {
                    row,
                    bound,
                    code: constraint.code,
                    span: constraint.span,
                })
            })
            .collect();
        scheme
    }

    pub fn instantiate(&mut self, scheme: &Scheme) -> Ty {
        self.instantiate_constrained(scheme).0
    }

    pub fn instantiate_constrained(
        &mut self,
        scheme: &Scheme,
    ) -> (Ty, Vec<ComparisonConstraint>, Vec<RowConstraint>) {
        let replacements: HashMap<_, _> = scheme
            .variables
            .iter()
            .map(|id| (*id, self.fresh()))
            .collect();
        let replace = |id| replacements.get(&id).cloned().unwrap_or(Ty::Var(id));
        (
            scheme.ty.map_variables(&replace),
            scheme
                .constraints
                .iter()
                .map(|constraint| constraint.with_type(constraint.ty.map_variables(&replace)))
                .collect(),
            scheme
                .row_constraints
                .iter()
                .map(|constraint| constraint.map_variables(&replace))
                .collect(),
        )
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Var(id) => write!(f, "t{id}"),
            Ty::Fn(args, ret, row) => write!(
                f,
                "Fn({} => {ret}{})",
                display_arguments(args),
                row.suffix()
            ),
            Ty::Int => write!(f, "Int"),
            Ty::Float => write!(f, "Float"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Char => write!(f, "Char"),
            Ty::String => write!(f, "String"),
            Ty::Bytes => write!(f, "Bytes"),
            Ty::Void => write!(f, "Void"),
            Ty::Error => write!(f, "Error"),
            Ty::Adt(name, args) | Ty::Abstract(name, args) => {
                let name = name
                    .rsplit_once('$')
                    .map_or(name.as_str(), |(_, source)| source);
                match args.as_slice() {
                    [] => write!(f, "{name}"),
                    [argument] => write!(f, "{argument} {name}"),
                    _ => write!(f, "({}) {name}", display_arguments(args)),
                }
            }
        }
    }
}

fn display_arguments(types: &[Ty]) -> String {
    types
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

// --- 定義の型の印字（§58.3） ---

/// The printer of a scheme's type. It mirrors `Display for Ty` (§16, §41) so
/// that the two agree on a monomorphic type, and names the variables instead
/// of printing their inference numbers.
#[derive(Default)]
struct Printer {
    names: HashMap<u32, String>,
}

impl Printer {
    fn variable(&mut self, id: u32) -> String {
        if let Some(name) = self.names.get(&id) {
            return name.clone();
        }
        let name = variable_name(self.names.len());
        self.names.insert(id, name.clone());
        name
    }

    fn ty(&mut self, ty: &Ty) -> String {
        match ty {
            Ty::Var(id) => self.variable(*id),
            // §16 cannot write a nullary `Fn` (`Fn( => Int)` is E100), so the
            // checker builds one only for a value `def` (§15), where the type
            // the source means is the body's type with its effects.
            Ty::Fn(args, ret, row) if args.is_empty() => {
                format!("{}{}", self.ty(ret), self.row(row))
            }
            Ty::Fn(args, ret, row) => {
                format!(
                    "Fn({} => {}{})",
                    self.arguments(args),
                    self.ty(ret),
                    self.row(row)
                )
            }
            Ty::Int => "Int".into(),
            Ty::Float => "Float".into(),
            Ty::Bool => "Bool".into(),
            Ty::Char => "Char".into(),
            Ty::String => "String".into(),
            Ty::Bytes => "Bytes".into(),
            Ty::Void => "Void".into(),
            Ty::Error => "Error".into(),
            Ty::Adt(name, args) | Ty::Abstract(name, args) => {
                let name = name
                    .rsplit_once('$')
                    .map_or(name.as_str(), |(_, source)| source);
                match args.as_slice() {
                    [] => name.to_string(),
                    [argument] => format!("{} {name}", self.ty(argument)),
                    _ => format!("({}) {name}", self.arguments(args)),
                }
            }
        }
    }

    fn arguments(&mut self, types: &[Ty]) -> String {
        types
            .iter()
            .map(|ty| self.ty(ty))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// §20: the result's row. The known labels print in alphabetical order; an
    /// open row adds the variable it still contains (`{Io | A}`, `{A}`).
    fn row(&mut self, row: &Row) -> String {
        match row {
            Row::Closed(labels) => {
                let labels = self.labels(labels);
                if labels.is_empty() {
                    String::new()
                } else {
                    format!(" {{{labels}}}")
                }
            }
            Row::Var(id) => format!(" {{{}}}", self.variable(*id)),
            Row::Open(id, labels) => {
                let variable = self.variable(*id);
                let labels = self.labels(labels);
                if labels.is_empty() {
                    format!(" {{{variable}}}")
                } else {
                    format!(" {{{labels} | {variable}}}")
                }
            }
        }
    }

    fn labels(&mut self, labels: &[Ty]) -> String {
        let mut names: Vec<String> = labels.iter().map(|label| self.ty(label)).collect();
        names.sort();
        names.dedup();
        names.join(", ")
    }
}

/// §15.1: two types as one readable pair for a diagnostic. `Display for Ty`
/// prints the inference numbers (`t902`), which are neither readable nor
/// stable. The pair is walked with one table, so a variable that appears in
/// both types keeps one name.
pub(crate) fn readable_pair(left: &Ty, right: &Ty) -> (String, String) {
    let mut printer = Printer::default();
    let left = printer.ty(left);
    let right = printer.ty(right);
    (left, right)
}

/// `A`, `B`, …, `Z`, `A1`, `B1`, … in order.
fn variable_name(index: usize) -> String {
    let letter = char::from(b'A' + (index % 26) as u8);
    match index / 26 {
        0 => letter.to_string(),
        round => format!("{letter}{round}"),
    }
}

pub(crate) fn label_matches_effect(label: &str, effect: &str) -> bool {
    label == effect
        || label.ends_with(&format!(" {effect}"))
        || label.ends_with(&format!(") {effect}"))
}

pub(crate) fn effect_ctor(ty: &Ty) -> String {
    match ty {
        Ty::Adt(name, _) => name
            .rsplit_once('$')
            .map_or(name.as_str(), |(_, name)| name)
            .into(),
        ty => ty.to_string(),
    }
}

fn collect_row_vars(ty: &Ty, result: &mut BTreeSet<u32>) {
    match ty {
        Ty::Fn(args, ret, row) => {
            for arg in args {
                collect_row_vars(arg, result);
            }
            collect_row_vars(ret, result);
            row.free_variables(result);
        }
        Ty::Adt(_, args) | Ty::Abstract(_, args) => {
            for arg in args {
                collect_row_vars(arg, result);
            }
        }
        _ => {}
    }
}

impl Scheme {
    /// §58.3 / plan.md L2・Q2 の決着: whether the scheme still carries the `Error`
    /// placeholder inference leaves behind when it fails. そのときは型を出さない。
    pub(crate) fn has_error(&self) -> bool {
        self.ty.has_error()
    }

    pub(crate) fn resolved(&self, inference: &Inference) -> Self {
        let resolve = |id| {
            if self.variables.contains(&id) {
                Ty::Var(id)
            } else {
                inference.resolve(&Ty::Var(id))
            }
        };
        Self {
            variables: self.variables.clone(),
            ty: self.ty.map_variables(&resolve),
            constraints: self
                .constraints
                .iter()
                .map(|constraint| constraint.with_type(constraint.ty.map_variables(&resolve)))
                .collect(),
            row_constraints: self
                .row_constraints
                .iter()
                .map(|constraint| constraint.map_variables(&resolve))
                .collect(),
        }
    }

    pub(crate) fn as_thunk(&self) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: Ty::fn_of(vec![], self.ty.clone()),
            constraints: self.constraints.clone(),
            row_constraints: self.row_constraints.clone(),
        }
    }

    pub(crate) fn body(&self) -> &Ty {
        &self.ty
    }

    /// §58.3: the canonical print of the scheme's type. The variables are
    /// renamed `A`, `B`, …, `A1`, … in order of first appearance, so the same
    /// type reads the same way twice. Unlike `Ty`'s own print (§20), an effect
    /// row keeps the variables it still contains: hiding them would make an
    /// effect-polymorphic type look pure.
    pub(crate) fn canonical(&self) -> String {
        Printer::default().ty(&self.ty)
    }

    pub fn monomorphic(ty: Ty) -> Self {
        Self {
            variables: BTreeSet::new(),
            ty,
            constraints: vec![],
            row_constraints: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeRealization {
    pub parameters: Vec<u32>,
    pub body: Ty,
}

impl TypeRealization {
    pub(crate) fn apply(&self, arguments: &[Ty]) -> Ty {
        assert_eq!(
            self.parameters.len(),
            arguments.len(),
            "checked abstract type arity"
        );
        let substitutions: HashMap<_, _> = self
            .parameters
            .iter()
            .copied()
            .zip(arguments.iter())
            .collect();
        self.body.map_variables(&|id| {
            substitutions
                .get(&id)
                .map_or(Ty::Var(id), |ty| (*ty).clone())
        })
    }
}

impl Ty {
    pub(crate) fn abstract_names(&self, names: &mut BTreeSet<String>) {
        match self {
            Ty::Abstract(name, args) => {
                names.insert(name.clone());
                for arg in args {
                    arg.abstract_names(names);
                }
            }
            Ty::Adt(_, args) => {
                for arg in args {
                    arg.abstract_names(names);
                }
            }
            Ty::Fn(args, result, _) => {
                for arg in args {
                    arg.abstract_names(names);
                }
                result.abstract_names(names);
            }
            _ => {}
        }
    }

    pub(crate) fn reveal(&self, names: &HashMap<String, TypeRealization>) -> Self {
        match self {
            Self::Abstract(name, args) if names.contains_key(name) => {
                names[name].apply(args).reveal(names)
            }
            Self::Abstract(name, args) => Self::Abstract(
                name.clone(),
                args.iter().map(|ty| ty.reveal(names)).collect(),
            ),
            Self::Adt(name, args) => Self::Adt(
                name.clone(),
                args.iter().map(|ty| ty.reveal(names)).collect(),
            ),
            Self::Fn(args, result, row) => Self::Fn(
                args.iter().map(|ty| ty.reveal(names)).collect(),
                Box::new(result.reveal(names)),
                row.clone(),
            ),
            ty => ty.clone(),
        }
    }

    pub(crate) fn rename_modules(&self, names: &HashMap<String, String>) -> Self {
        match self {
            Self::Adt(name, args) => Self::Adt(
                crate::functor_instances::rename(name, names),
                args.iter().map(|ty| ty.rename_modules(names)).collect(),
            ),
            Self::Abstract(name, args) => Self::Abstract(
                crate::functor_instances::rename(name, names),
                args.iter().map(|ty| ty.rename_modules(names)).collect(),
            ),
            Self::Fn(args, result, row) => Self::Fn(
                args.iter().map(|ty| ty.rename_modules(names)).collect(),
                Box::new(result.rename_modules(names)),
                row.clone(),
            ),
            ty => ty.clone(),
        }
    }

    pub(crate) fn qualify_data_types(&self, names: &HashMap<String, String>) -> Self {
        match self {
            Self::Adt(name, arguments) => Self::Adt(
                names.get(name).unwrap_or(name).clone(),
                arguments
                    .iter()
                    .map(|ty| ty.qualify_data_types(names))
                    .collect(),
            ),
            Self::Abstract(name, arguments) => Self::Abstract(
                name.clone(),
                arguments
                    .iter()
                    .map(|ty| ty.qualify_data_types(names))
                    .collect(),
            ),
            Self::Fn(parameters, result, row) => Self::Fn(
                parameters
                    .iter()
                    .map(|ty| ty.qualify_data_types(names))
                    .collect(),
                Box::new(result.qualify_data_types(names)),
                row.clone(),
            ),
            ty => ty.clone(),
        }
    }
}

impl Scheme {
    pub(crate) fn reveal(&self, names: &HashMap<String, TypeRealization>) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: self.ty.reveal(names),
            constraints: self
                .constraints
                .iter()
                .map(|constraint| constraint.with_type(constraint.ty.reveal(names)))
                .collect(),
            row_constraints: self.row_constraints.clone(),
        }
    }

    pub(crate) fn rename_modules(&self, names: &HashMap<String, String>) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: self.ty.rename_modules(names),
            constraints: self
                .constraints
                .iter()
                .map(|constraint| constraint.with_type(constraint.ty.rename_modules(names)))
                .collect(),
            row_constraints: self.row_constraints.clone(),
        }
    }

    pub(crate) fn qualify_data_types(&self, names: &HashMap<String, String>) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: self.ty.qualify_data_types(names),
            constraints: self
                .constraints
                .iter()
                .map(|constraint| constraint.with_type(constraint.ty.qualify_data_types(names)))
                .collect(),
            row_constraints: self.row_constraints.clone(),
        }
    }
}
