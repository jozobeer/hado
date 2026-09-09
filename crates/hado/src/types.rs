use crate::parser::{Expr, Program};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Error,
    Int,
    Float,
    Bool,
    Char,
    String,
    Unit,
    Var(u32),
    Fn(Vec<Ty>, Box<Ty>, Box<Row>),
    Adt(String, Vec<Ty>),
    Abstract(String, Vec<Ty>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    Var(u32),
    Closed(BTreeSet<String>),
}

impl Row {
    pub fn pure() -> Self {
        Self::Closed(BTreeSet::new())
    }

    pub fn is_pure(&self) -> bool {
        matches!(self, Self::Closed(labels) if labels.is_empty())
    }

    pub fn suffix(&self) -> String {
        match self {
            Self::Closed(labels) if !labels.is_empty() => format!(" {{{}}}", labels.iter().cloned().collect::<Vec<_>>().join(", ")),
            _ => String::new(),
        }
    }

    pub fn without_effect(&self, effect: &str) -> Self {
        match self {
            Self::Closed(labels) => Self::Closed(labels.iter().filter(|label| !label_matches_effect(label, effect)).cloned().collect()),
            row => row.clone(),
        }
    }

    pub fn has_known_effects(&self) -> bool {
        matches!(self, Self::Closed(labels) if !labels.is_empty())
    }

    fn free_variables(&self, result: &mut BTreeSet<u32>) {
        if let Self::Var(id) = self {
            result.insert(*id);
        }
    }

    fn map_variables(&self, f: &impl Fn(u32) -> Ty) -> Self {
        match self {
            Self::Var(id) => match f(*id) {
                Ty::Var(id) => Self::Var(id),
                _ => self.clone(),
            },
            Self::Closed(labels) => Self::Closed(labels.clone()),
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonConstraint {
    pub ty: Ty,
    pub code: &'static str,
    pub span: Option<crate::lexer::Span>,
}

impl ComparisonConstraint {
    fn with_type(&self, ty: Ty) -> Self {
        Self { ty, code: self.code, span: self.span }
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
            Ty::Adt(name, args) => Ty::Adt(name.clone(), args.iter().map(|arg| self.freshen_errors(arg)).collect()),
            Ty::Abstract(name, args) => Ty::Abstract(name.clone(), args.iter().map(|arg| self.freshen_errors(arg)).collect()),
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
            Ty::Adt(name, args) => Ty::Adt(name.clone(), args.iter().map(|ty| self.resolve(ty)).collect()),
            Ty::Abstract(name, args) => Ty::Abstract(name.clone(), args.iter().map(|ty| self.resolve(ty)).collect()),
            ty => ty.clone(),
        }
    }

    pub fn resolve_row(&self, row: &Row) -> Row {
        match row {
            Row::Var(id) => match self.rows.get(id) {
                Some(value) => self.resolve_row(value),
                None => Row::Var(*id),
            },
            Row::Closed(labels) => Row::Closed(labels.clone()),
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
            (Row::Closed(a), Row::Closed(b)) if a.is_subset(b) => Ok(()),
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
            (Row::Closed(_), Row::Closed(_)) => Err(TypeError::Mismatch),
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
            (Row::Closed(a), Row::Closed(b)) => {
                let mut labels = a.clone();
                labels.extend(b.iter().cloned());
                Row::Closed(labels)
            }
            (Row::Var(id), Row::Closed(labels)) | (Row::Closed(labels), Row::Var(id)) if labels.is_empty() => {
                Row::Var(*id)
            }
            (Row::Var(id), Row::Closed(labels)) | (Row::Closed(labels), Row::Var(id)) => {
                let labels = labels.clone();
                let _ = self.bind_row(*id, &Row::Closed(labels.clone()));
                Row::Closed(labels)
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
            closed => closed,
        }
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
        }
    }

    pub fn generalize_constrained(
        &self,
        ty: &Ty,
        environment: &[Ty],
        constraints: &[ComparisonConstraint],
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
        scheme
    }

    pub fn instantiate(&mut self, scheme: &Scheme) -> Ty {
        self.instantiate_constrained(scheme).0
    }

    pub fn instantiate_constrained(&mut self, scheme: &Scheme) -> (Ty, Vec<ComparisonConstraint>) {
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
        )
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Var(id) => write!(f, "t{id}"),
            Ty::Fn(args, ret, row) => write!(f, "Fn({} => {ret}{})", display_arguments(args), row.suffix()),
            Ty::Int => write!(f, "Int"),
            Ty::Float => write!(f, "Float"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Char => write!(f, "Char"),
            Ty::String => write!(f, "String"),
            Ty::Unit => write!(f, "Unit"),
            Ty::Error => write!(f, "Error"),
            Ty::Adt(name, args) | Ty::Abstract(name, args) => {
                let name = name.rsplit_once('$').map_or(name.as_str(), |(_, source)| source);
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
    types.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
}

fn label_matches_effect(label: &str, effect: &str) -> bool {
    label == effect || label.ends_with(&format!(" {effect}")) || label.ends_with(&format!(") {effect}"))
}

fn collect_row_vars(ty: &Ty, result: &mut BTreeSet<u32>) {
    match ty {
        Ty::Fn(args, ret, row) => {
            for arg in args { collect_row_vars(arg, result); }
            collect_row_vars(ret, result);
            row.free_variables(result);
        }
        Ty::Adt(_, args) | Ty::Abstract(_, args) => {
            for arg in args { collect_row_vars(arg, result); }
        }
        _ => {}
    }
}

impl Scheme {
    pub(crate) fn resolved(&self, inference: &Inference) -> Self {
        let resolve = |id| if self.variables.contains(&id) { Ty::Var(id) } else { inference.resolve(&Ty::Var(id)) };
        Self {
            variables: self.variables.clone(),
            ty: self.ty.map_variables(&resolve),
            constraints: self.constraints.iter().map(|constraint| constraint.with_type(constraint.ty.map_variables(&resolve))).collect(),
        }
    }

    pub(crate) fn as_thunk(&self) -> Self {
        Self { variables: self.variables.clone(), ty: Ty::fn_of(vec![], self.ty.clone()), constraints: self.constraints.clone() }
    }

    pub(crate) fn body(&self) -> &Ty { &self.ty }

    pub fn monomorphic(ty: Ty) -> Self {
        Self {
            variables: BTreeSet::new(),
            ty,
            constraints: vec![],
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
        assert_eq!(self.parameters.len(), arguments.len(), "checked abstract type arity");
        let substitutions: HashMap<_, _> = self.parameters.iter().copied().zip(arguments.iter()).collect();
        self.body.map_variables(&|id| substitutions.get(&id).map_or(Ty::Var(id), |ty| (*ty).clone()))
    }
}

impl Ty {
    pub(crate) fn abstract_names(&self, names: &mut BTreeSet<String>) {
        match self {
            Ty::Abstract(name, args) => {
                names.insert(name.clone());
                for arg in args { arg.abstract_names(names); }
            }
            Ty::Adt(_, args) => { for arg in args { arg.abstract_names(names); } }
            Ty::Fn(args, result, _) => {
                for arg in args { arg.abstract_names(names); }
                result.abstract_names(names);
            }
            _ => {}
        }
    }

    pub(crate) fn reveal(&self, names: &HashMap<String, TypeRealization>) -> Self {
        match self {
            Self::Abstract(name, args) if names.contains_key(name) => names[name].apply(args).reveal(names),
            Self::Abstract(name, args) => Self::Abstract(name.clone(), args.iter().map(|ty| ty.reveal(names)).collect()),
            Self::Adt(name, args) => Self::Adt(name.clone(), args.iter().map(|ty| ty.reveal(names)).collect()),
            Self::Fn(args, result, row) => Self::Fn(args.iter().map(|ty| ty.reveal(names)).collect(), Box::new(result.reveal(names)), row.clone()),
            ty => ty.clone(),
        }
    }

    pub(crate) fn rename_modules(&self, names: &HashMap<String, String>) -> Self {
        match self {
            Self::Adt(name, args) => Self::Adt(crate::functor_instances::rename(name, names), args.iter().map(|ty| ty.rename_modules(names)).collect()),
            Self::Abstract(name, args) => Self::Abstract(crate::functor_instances::rename(name, names), args.iter().map(|ty| ty.rename_modules(names)).collect()),
            Self::Fn(args, result, row) => Self::Fn(args.iter().map(|ty| ty.rename_modules(names)).collect(), Box::new(result.rename_modules(names)), row.clone()),
            ty => ty.clone(),
        }
    }

    pub(crate) fn qualify_data_types(&self, names: &HashMap<String, String>) -> Self {
        match self {
            Self::Adt(name, arguments) => Self::Adt(names.get(name).unwrap_or(name).clone(), arguments.iter().map(|ty| ty.qualify_data_types(names)).collect()),
            Self::Abstract(name, arguments) => Self::Abstract(name.clone(), arguments.iter().map(|ty| ty.qualify_data_types(names)).collect()),
            Self::Fn(parameters, result, row) => Self::Fn(parameters.iter().map(|ty| ty.qualify_data_types(names)).collect(), Box::new(result.qualify_data_types(names)), row.clone()),
            ty => ty.clone(),
        }
    }
}

impl Scheme {
    pub(crate) fn reveal(&self, names: &HashMap<String, TypeRealization>) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: self.ty.reveal(names),
            constraints: self.constraints.iter().map(|constraint| constraint.with_type(constraint.ty.reveal(names))).collect(),
        }
    }

    pub(crate) fn rename_modules(&self, names: &HashMap<String, String>) -> Self {
        Self { variables: self.variables.clone(), ty: self.ty.rename_modules(names),
            constraints: self.constraints.iter().map(|constraint| constraint.with_type(constraint.ty.rename_modules(names))).collect() }
    }

    pub(crate) fn qualify_data_types(&self, names: &HashMap<String, String>) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: self.ty.qualify_data_types(names),
            constraints: self.constraints.iter().map(|constraint| constraint.with_type(constraint.ty.qualify_data_types(names))).collect(),
        }
    }
}
