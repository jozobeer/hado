use crate::parser::{Expr, Program};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    Char,
    String,
    Unit,
    Var(u32),
    Fn(Vec<Ty>, Box<Ty>),
    Adt(String, Vec<Ty>),
    Abstract(String, Vec<Ty>),
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
    constraints: Vec<(Ty, &'static str)>,
}

#[derive(Default, Clone)]
pub struct Inference {
    next: u32,
    substitutions: HashMap<u32, Ty>,
}

impl Ty {
    pub fn is_monomorphic(&self) -> bool {
        let mut variables = BTreeSet::new();
        self.free_variables(&mut variables);
        variables.is_empty()
    }

    fn map_variables(&self, f: &impl Fn(u32) -> Ty) -> Ty {
        match self {
            Ty::Var(id) => f(*id),
            Ty::Fn(args, ret) => Ty::Fn(
                args.iter().map(|ty| ty.map_variables(f)).collect(),
                Box::new(ret.map_variables(f)),
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
            Ty::Fn(args, ret) => {
                for arg in args {
                    arg.free_variables(result);
                }
                ret.free_variables(result);
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

    pub fn resolve(&self, ty: &Ty) -> Ty {
        ty.map_variables(&|id| match self.substitutions.get(&id) {
            Some(value) => self.resolve(value),
            None => Ty::Var(id),
        })
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
        let saved = self.substitutions.clone();
        let result = self.unify_inner(left, right);
        if result.is_err() { self.substitutions = saved; }
        result
    }

    fn unify_inner(&mut self, left: &Ty, right: &Ty) -> Result<(), TypeError> {
        let left = self.resolve(left);
        let right = self.resolve(right);
        if left == right {
            return Ok(());
        }
        match (&left, &right) {
            (Ty::Var(id), ty) | (ty, Ty::Var(id)) => self.bind(*id, ty),
            (Ty::Fn(a, ret_a), Ty::Fn(b, ret_b)) => {
                self.unify_arguments(a, b)?;
                self.unify_inner(ret_a, ret_b)
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
        constraints: &[(Ty, &'static str)],
    ) -> Scheme {
        let mut scheme = self.generalize(ty, environment);
        scheme.constraints = constraints
            .iter()
            .filter_map(|(ty, code)| {
                let ty = self.resolve(ty);
                let mut variables = BTreeSet::new();
                ty.free_variables(&mut variables);
                (!variables.is_disjoint(&scheme.variables)).then_some((ty, *code))
            })
            .collect();
        scheme
    }

    pub fn instantiate(&mut self, scheme: &Scheme) -> Ty {
        self.instantiate_constrained(scheme).0
    }

    pub fn instantiate_constrained(&mut self, scheme: &Scheme) -> (Ty, Vec<(Ty, &'static str)>) {
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
                .map(|(ty, code)| (ty.map_variables(&replace), *code))
                .collect(),
        )
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Var(id) => write!(f, "t{id}"),
            Ty::Fn(args, ret) => {
                let args = args
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                write!(f, "Fn<({args}),{ret}>")
            }
            Ty::Adt(name, args) if !args.is_empty() => {
                let args = args
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                write!(f, "{name}<{args}>")
            }
            Ty::Adt(name, _) => write!(f, "{name}"),
            ty => write!(f, "{ty:?}"),
        }
    }
}

impl Scheme {
    pub(crate) fn as_thunk(&self) -> Self {
        Self { variables: self.variables.clone(), ty: Ty::Fn(vec![], Box::new(self.ty.clone())), constraints: self.constraints.clone() }
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

impl Ty {
    pub(crate) fn reveal(&self, names: &HashMap<String, Ty>) -> Self {
        match self {
            Self::Abstract(name, _) if names.contains_key(name) => names[name].reveal(names),
            Self::Abstract(name, args) => Self::Abstract(name.clone(), args.iter().map(|ty| ty.reveal(names)).collect()),
            Self::Adt(name, args) => Self::Adt(name.clone(), args.iter().map(|ty| ty.reveal(names)).collect()),
            Self::Fn(args, result) => Self::Fn(args.iter().map(|ty| ty.reveal(names)).collect(), Box::new(result.reveal(names))),
            ty => ty.clone(),
        }
    }

    pub(crate) fn qualify_data_types(&self, names: &HashMap<String, String>) -> Self {
        match self {
            Self::Adt(name, arguments) => Self::Adt(names.get(name).unwrap_or(name).clone(), arguments.iter().map(|ty| ty.qualify_data_types(names)).collect()),
            Self::Abstract(name, arguments) => Self::Abstract(name.clone(), arguments.iter().map(|ty| ty.qualify_data_types(names)).collect()),
            Self::Fn(parameters, result) => Self::Fn(parameters.iter().map(|ty| ty.qualify_data_types(names)).collect(), Box::new(result.qualify_data_types(names))),
            ty => ty.clone(),
        }
    }
}

impl Scheme {
    pub(crate) fn reveal(&self, names: &HashMap<String, Ty>) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: self.ty.reveal(names),
            constraints: self.constraints.iter().map(|(ty, code)| (ty.reveal(names), *code)).collect(),
        }
    }

    pub(crate) fn qualify_data_types(&self, names: &HashMap<String, String>) -> Self {
        Self {
            variables: self.variables.clone(),
            ty: self.ty.qualify_data_types(names),
            constraints: self.constraints.iter().map(|(ty, code)| (ty.qualify_data_types(names), *code)).collect(),
        }
    }
}
