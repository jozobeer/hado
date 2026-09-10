use crate::types::Ty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Bool,
    Char,
    String,
    Unit,
    Data(String, Vec<Type>),
    Abstract(String, Vec<Type>),
    Closure(Vec<Type>, Box<Type>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnresolvedType { Variable(u32), Invalid }

impl TryFrom<&Ty> for Type {
    type Error = UnresolvedType;

    fn try_from(ty: &Ty) -> Result<Self, Self::Error> {
        Ok(match ty {
            Ty::Int => Self::Int,
            Ty::Float => Self::Float,
            Ty::Bool => Self::Bool,
            Ty::Char => Self::Char,
            Ty::String => Self::String,
            Ty::Unit => Self::Unit,
            Ty::Var(id) => return Err(UnresolvedType::Variable(*id)),
            Ty::Error => return Err(UnresolvedType::Invalid),
            Ty::Adt(name, args) => Self::Data(name.clone(), convert_types(args)?),
            Ty::Abstract(name, args) => Self::Abstract(name.clone(), convert_types(args)?),
            Ty::Fn(args, result, _) => Self::Closure(
                convert_types(args)?,
                Box::new(Self::try_from(result.as_ref())?),
            ),
        })
    }
}

fn convert_types(types: &[Ty]) -> Result<Vec<Type>, UnresolvedType> {
    types.iter().map(Type::try_from).collect()
}

pub(crate) const OUTCOME: &str = "$Outcome";
const DONE: &str = "$Done";
const FAIL: &str = "$Fail";
const YIELD: &str = "$Yield";

pub(crate) fn outcome_tag(constructor: &str) -> Option<u32> {
    if constructor == DONE {
        return Some(0);
    }
    if constructor == YIELD {
        return Some(2);
    }
    if is_fail(constructor) {
        return Some(fail_tag(constructor));
    }
    None
}

pub(crate) fn outcome_fields(constructor: &str, ty: &Type) -> Option<Vec<Type>> {
    let Type::Data(name, args) = ty else { return None };
    if name != OUTCOME {
        return None;
    }
    let success = args.first()?.clone();
    let answer = args.get(1).cloned().unwrap_or_else(|| success.clone());
    let payload = args.get(2).cloned().unwrap_or(Type::Unit);
    Some(match constructor {
        DONE => vec![success],
        YIELD => vec![payload, Type::Closure(vec![Type::Unit], Box::new(answer))],
        _ if is_fail(constructor) => vec![payload],
        _ => return None,
    })
}

fn is_fail(constructor: &str) -> bool {
    constructor.starts_with("$Fail#")
}

fn fail_constructor(effect: &str) -> String {
    format!("{FAIL}#{effect}")
}

fn fail_tag(constructor: &str) -> u32 {
    let effect = constructor.strip_prefix("$Fail#").unwrap_or("");
    let mut hash: u32 = 2166136261;
    for byte in effect.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16777619);
    }
    hash | 1
}

fn is_outcome(ty: &Type) -> bool {
    matches!(ty, Type::Data(name, _) if name == OUTCOME)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
    Unit,
}

impl Constant {
    pub fn ty(&self) -> Type {
        match self {
            Self::Int(_) => Type::Int,
            Self::Float(_) => Type::Float,
            Self::Bool(_) => Type::Bool,
            Self::Char(_) => Type::Char,
            Self::String(_) => Type::String,
            Self::Unit => Type::Unit,
        }
    }
}

pub fn lower_literal(node: &crate::parser::Node) -> Option<Constant> {
    use crate::parser::NodeKind;
    Some(match &node.kind {
        NodeKind::Int(value) => Constant::Int(*value),
        NodeKind::Float(value) => Constant::Float(*value),
        NodeKind::Bool(value) => Constant::Bool(*value),
        NodeKind::Char(value) => Constant::Char(*value),
        NodeKind::Str(value) => Constant::String(value.clone()),
        NodeKind::Unit => Constant::Unit,
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    CallRuntime { function: crate::runtime_abi::RuntimeFunction, arguments: Vec<Expression>, result: Type },
    Unreachable {
        result: Type,
    },
    Switch {
        value: Box<Expression>,
        arms: Vec<(String, Expression)>,
        fallback: Option<Box<Expression>>,
        result: Type,
    },
    Project {
        value: Box<Expression>,
        constructor: String,
        index: usize,
        result: Type,
    },
    RecordGet {
        record: Box<Expression>,
        field: String,
        result: Type,
    },
    Record {
        fields: Vec<(String, Expression)>,
        result: Type,
    },
    Construct {
        constructor: String,
        fields: Vec<Expression>,
        result: Type,
    },
    MakeClosure {
        captures: Vec<Expression>,
        function: FunctionId,
        signature: Signature,
    },
    CallClosure {
        closure: Box<Expression>,
        arguments: Vec<Expression>,
        result: Type,
    },
    TailCallClosure {
        closure: Box<Expression>,
        arguments: Vec<Expression>,
        result: Type,
    },
    TailCall {
        function: FunctionId,
        arguments: Vec<Expression>,
        result: Type,
    },
    CallDirect {
        function: FunctionId,
        arguments: Vec<Expression>,
        result: Type,
    },
    Variable {
        id: usize,
        ty: Type,
    },
    Let {
        id: usize,
        value: Box<Expression>,
        body: Box<Expression>,
    },
    Branch {
        condition: Box<Expression>,
        then: Box<Expression>,
        otherwise: Box<Expression>,
    },
    Constant(Constant),
    Binary {
        op: crate::parser::BinOp,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Unary {
        op: UnaryOp,
        value: Box<Expression>,
    },
}

impl Expression {
    pub fn call_runtime(function: crate::runtime_abi::RuntimeFunction, arguments: Vec<Expression>) -> Result<Self, String> {
        let signature = function.value_signature().ok_or("runtime function has no value-level signature")?;
        if arguments.iter().map(Expression::ty).collect::<Vec<_>>() != signature.parameters {
            return Err("runtime call argument type mismatch".into());
        }
        Ok(Self::CallRuntime { function, arguments, result: signature.result })
    }

    pub fn call_closure(closure: Expression, arguments: Vec<Expression>) -> Result<Self, String> {
        let Type::Closure(parameters, result) = closure.ty() else {
            return Err("callee is not a closure".into());
        };
        if arguments.iter().map(Expression::ty).collect::<Vec<_>>() != parameters {
            return Err("call arguments do not match the closure signature".into());
        }
        Ok(Self::CallClosure {
            closure: Box::new(closure),
            arguments,
            result: *result,
        })
    }

    pub fn call_direct(
        function: FunctionId,
        signature: &Signature,
        arguments: Vec<Expression>,
    ) -> Result<Self, String> {
        if arguments.len() != signature.parameters.len()
            || arguments
                .iter()
                .zip(&signature.parameters)
                .any(|(value, ty)| value.ty() != *ty)
        {
            return Err("call arguments do not match the function signature".into());
        }
        Ok(Self::CallDirect {
            function,
            arguments,
            result: signature.result.clone(),
        })
    }

    pub fn ty(&self) -> Type {
        match self {
            Self::CallRuntime { result, .. }
            | Self::Unreachable { result }
            | Self::Switch { result, .. }
            | Self::Project { result, .. }
            | Self::Construct { result, .. }
            | Self::Record { result, .. }
            | Self::RecordGet { result, .. } => result.clone(),
            Self::MakeClosure { signature, .. } => Type::Closure(
                signature.parameters.clone(),
                Box::new(signature.result.clone()),
            ),
            Self::CallDirect { result, .. }
            | Self::TailCall { result, .. }
            | Self::CallClosure { result, .. }
            | Self::TailCallClosure { result, .. } => result.clone(),
            Self::Branch { then, .. } => then.ty(),
            Self::Variable { ty, .. } => ty.clone(),
            Self::Let { body, .. } => body.ty(),
            Self::Constant(value) => value.ty(),
            Self::Binary { op, .. } if is_comparison(*op) => Type::Bool,
            Self::Binary { op, left, .. }
                if matches!(op, crate::parser::BinOp::Div | crate::parser::BinOp::Mod)
                    && left.ty() == Type::Int =>
            {
                Type::Int
            }
            Self::Binary { left, .. } => left.ty(),
            Self::Unary {
                op: UnaryOp::Not, ..
            } => Type::Bool,
            Self::Unary { value, .. } => value.ty(),
        }
    }
}

enum MatchStep {
    Bind(usize, Expression),
    Tag(String, Expression),
    Equal(Expression, Expression),
}

#[derive(Default)]
struct Lowerer {
    constructor_tags: std::collections::HashMap<String, u32>,
    scalar_comparisons: Vec<(Type, FunctionId)>,
    structural_equalities: Vec<(Type, FunctionId)>,
    constructors: std::collections::HashMap<String, crate::types::Scheme>,
    records: std::collections::HashMap<String, (Vec<String>, crate::types::Scheme)>,
    named_constructors: std::collections::HashMap<String, Vec<String>>,
    definitions: std::collections::BTreeMap<String, crate::parser::Def>,
    functions: Vec<Function>,
    active: Vec<(String, FunctionId, Signature)>,
    schemes: std::collections::HashMap<String, crate::types::Scheme>,
    value_types: std::collections::HashMap<(usize, usize), Ty>,
    specialization: crate::types::Inference,
    composite_types: std::collections::HashMap<(usize, usize), Ty>,
    operations: std::collections::HashMap<String, crate::parser::OpKind>,
    handlers: std::collections::HashMap<String, crate::parser::HandlerDecl>,
    handler_stack: Vec<crate::parser::HandlerDecl>,
    capture_answers: Vec<Type>,
    resume_binding: Option<(usize, Type)>,
    rewriting_return: bool,
    next_function: usize,
    scope: std::collections::BTreeMap<String, (usize, Type)>,
    next: usize,
}

pub fn lower_expression(node: &crate::parser::Node) -> Result<Expression, String> {
    Lowerer::default().expression(node)
}

impl Lowerer {
    fn expression(&mut self, node: &crate::parser::Node) -> Result<Expression, String> {
        use crate::parser::NodeKind;
        if let NodeKind::Pipe(value, target) = &node.kind {
            let mut arguments = vec![self.expression(value)?];
            let callee = if let NodeKind::Apply(callee, args) = &target.kind {
                for arg in args {
                    arguments.push(self.expression(arg)?);
                }
                callee.as_ref()
            } else {
                target.as_ref()
            };
            return self.application(node, callee, arguments);
        }
        if let NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) = &node.kind {
            let arguments = args
                .iter()
                .map(|arg| self.expression(arg))
                .collect::<Result<Vec<_>, _>>()?;
            return self.application(node, callee, arguments);
        }

        if let NodeKind::Field(record, field) = &node.kind {
            if let NodeKind::Name(module) = &record.kind {
                let qualified = self.resolve_definition(&format!("{module}.{}", field.name));
                if self.schemes.contains_key(&qualified) {
                    return self.definition_reference(&qualified, node.span);
                }
            }
            let record = self.expression(record)?;
            return self.record_get(record, &field.name);
        }
        if let NodeKind::NamedConstructor(name, fields) = &node.kind {
            return self.named_constructor(node, &name.name, fields);
        }
        if let NodeKind::Record(fields) = &node.kind {
            let ty = self
                .composite_types
                .get(&(node.span.line, node.span.col))
                .ok_or("missing record type")?;
            let result = Type::try_from(&self.specialization.resolve(ty))
                .map_err(|_| "record type is not concrete")?;
            let fields = fields
                .iter()
                .map(|(name, value)| Ok((name.name.clone(), self.expression(value)?)))
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(Expression::Record { fields, result });
        }
        if let NodeKind::List(items, tail) = &node.kind {
            let ty = self
                .composite_types
                .get(&(node.span.line, node.span.col))
                .ok_or("missing list type")?
                .clone();
            let values = items
                .iter()
                .map(|item| self.expression(item))
                .collect::<Result<Vec<_>, _>>()?;
            let mut list = match tail {
                Some(tail) => self.expression(tail)?,
                None => self.construct("Nil", vec![], &ty)?,
            };
            for value in values.into_iter().rev() {
                list = self.construct("Cons", vec![value, list], &ty)?;
            }
            return Ok(list);
        }
        if let NodeKind::Fn { params, body } = &node.kind {
            return self.lambda(node, params, body);
        }
        if let NodeKind::Name(name) = &node.kind {
            if let Some(kind) = self.operations.get(name).copied() {
                return self.operation(node, name, vec![], kind);
            }
            if crate::names::is_upper_reference(name) {
                let ty = self
                    .value_types
                    .get(&(node.span.line, node.span.col))
                    .ok_or("missing constructor type")?;
                return self.construct(name, vec![], ty);
            }
            if !self.scope.contains_key(name) {
                return self.definition_reference(name, node.span);
            }
            let (id, ty) = self
                .scope
                .get(name)
                .ok_or_else(|| format!("unknown local variable {name}"))?;
            return Ok(Expression::Variable {
                id: *id,
                ty: ty.clone(),
            });
        }
        if let NodeKind::LetQ { pat, value, body } = &node.kind {
            return self.propagation(Some(pat), value, body);
        }
        if let NodeKind::DoQ { value, body } = &node.kind {
            if self.is_capture_node(value) {
                return self.capture_do(value, body);
            }
            if self.is_abort_node(value) && self.handler_for_node(value).is_some() {
                return self.expression(value);
            }
            let source = self.expression(value)?;
            if matches!(&source.ty(), Type::Data(name, args) if name == "Option" && args.len() == 1) {
                return self.option_propagation(None, source, body);
            }
            let id = self.next;
            self.next += 1;
            return Ok(Expression::Let {
                id,
                value: Box::new(source),
                body: Box::new(self.expression(body)?),
            });
        }
        if let NodeKind::Let { pat, value, body } = &node.kind {
            let value = self.expression(value)?;
            if is_outcome(&value.ty()) {
                return self.propagate_outcome(pat, value, body);
            }
            return self.bound_body(pat, value, body);
        }

        if let NodeKind::PatternBranch {
            scrutinee,
            arms,
            fallback,
        } = &node.kind
        {
            return self.literal_branch(scrutinee, arms, fallback.as_deref());
        }
        if let NodeKind::CondBranch { arms, fallback } = &node.kind {
            let fallback = fallback
                .as_ref()
                .ok_or("condition column requires a fallback")?;
            let mut result = self.expression(fallback)?;
            for (condition, body) in arms.iter().rev() {
                let condition = self.expression(condition)?;
                let body = self.expression(body)?;
                let (body, result_arm) = self.align_outcomes(body, result)?;
                result = branch(condition, body, result_arm)?;
            }
            return Ok(result);
        }
        if let NodeKind::BoolBranch { cond, then, els } = &node.kind {
            let condition = self.expression(cond)?;
            let then = self.expression(then)?;
            let otherwise = self.expression(els)?;
            let (then, otherwise) = self.align_outcomes(then, otherwise)?;
            return branch(condition, then, otherwise);
        }
        if let NodeKind::With { handler, args, body } = &node.kind {
            return self.with_handler(node, &handler.name, args, body);
        }
        if let NodeKind::Resume = &node.kind {
            return self.resume_variable();
        }
        if let Some(value) = lower_literal(node) {
            return Ok(Expression::Constant(value));
        }
        if let NodeKind::Bin(op, left, right) = &node.kind {
            return self.binary(*op, left, right);
        }
        let (op, operand) = match &node.kind {
            NodeKind::Neg(value) => (UnaryOp::Negate, value),
            NodeKind::Not(value) => (UnaryOp::Not, value),
            _ => return Err("expression lowering is not implemented for this node".into()),
        };
        let value = self.expression(operand)?;
        let valid = match op {
            UnaryOp::Negate => matches!(value.ty(), Type::Int | Type::Float),
            UnaryOp::Not => value.ty() == Type::Bool,
        };
        if !valid {
            return Err("invalid unary operand type".into());
        }
        Ok(Expression::Unary {
            op,
            value: Box::new(value),
        })
    }

    fn binary(
        &mut self,
        op: crate::parser::BinOp,
        left: &crate::parser::Node,
        right: &crate::parser::Node,
    ) -> Result<Expression, String> {
        use crate::parser::BinOp;
        let left = self.expression(left)?;
        let right = self.expression(right)?;
        if op == BinOp::And {
            return branch(left, right, Expression::Constant(Constant::Bool(false)));
        }
        if op == BinOp::Or {
            return branch(left, Expression::Constant(Constant::Bool(true)), right);
        }
        if !matches!(
            op,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
        ) && !is_comparison(op)
        {
            return Err("binary operator lowering is not implemented".into());
        }
        if left.ty() != right.ty()
            || (!matches!(op, BinOp::Eq | BinOp::Ne)
                && !matches!(left.ty(), Type::Int | Type::Float))
        {
            return Err("invalid arithmetic operand types".into());
        }
        if op == BinOp::Mod && left.ty() == Type::Float {
            return Err("Float has no remainder operator".into());
        }
        if matches!(op, BinOp::Eq | BinOp::Ne) {
            let equality = self.structural_equality(left, right)?;
            return Ok(if op == BinOp::Ne { Expression::Unary { op: UnaryOp::Not, value: Box::new(equality) } } else { equality });
        }
        Ok(Expression::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
}

fn is_comparison(op: crate::parser::BinOp) -> bool {
    use crate::parser::BinOp;
    matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    )
}

fn branch(
    condition: Expression,
    then: Expression,
    otherwise: Expression,
) -> Result<Expression, String> {
    if condition.ty() != Type::Bool || then.ty() != otherwise.ty() {
        return Err("invalid branch types".into());
    }
    Ok(Expression::Branch {
        condition: Box::new(condition),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    })
}

pub fn lower_program(
    ast: &crate::parser::Ast,
    checked: &crate::Checked,
) -> Result<Program, String> {
    let checked = &checked.for_codegen();
    use crate::parser::Item;
    let definitions: std::collections::BTreeMap<_, _> = ast
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Def(def) => Some((def.name.as_str(), def)),
            _ => None,
        })
        .collect();
    let mut lowerer = Lowerer {
        constructor_tags: checked.constructor_tags.clone(),
        constructors: checked.constructors.clone(),
        records: checked.records.clone(),
        named_constructors: checked.named_constructors.clone(),
        schemes: checked.definitions.clone(),
        value_types: checked.value_types.clone(),
        composite_types: checked.composite_types.clone(),
        operations: checked.operations.clone(),
        handlers: ast
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Handler(handler) => Some((handler.name.clone(), handler.clone())),
                _ => None,
            })
            .collect(),
        definitions: definitions
            .iter()
            .map(|(name, def)| ((*name).to_owned(), (*def).clone()))
            .collect(),
        ..Lowerer::default()
    };
    lowerer.collect_modules(&ast.items, "");
    let instances: Vec<_> = checked.instantiated_modules.iter().cloned().map(Item::ModuleImpl).collect();
    lowerer.collect_modules(&instances, "");
    lowerer.schemes.extend(checked.module_definitions.clone());
    let body = lowerer.expression(ast.main().ok_or("main must be a value definition")?)?;
    Ok(Program {
        entry: body.in_tail_position(),
        functions: lowerer.functions,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub parameters: Vec<Type>,
    pub result: Type,
}

impl Signature {
    pub fn specialize(scheme: &crate::types::Scheme, parameters: &[Type]) -> Result<Self, String> {
        Self::specialize_result(scheme, parameters, None)
    }

    fn specialize_result(
        scheme: &crate::types::Scheme,
        parameters: &[Type],
        result: Option<&Type>,
    ) -> Result<Self, String> {
        let mut inference = crate::types::Inference::default();
        let function = inference.instantiate(scheme);
        let result = result.map(Ty::from).unwrap_or_else(|| inference.fresh());
        let expected = Ty::fn_with(
            parameters.iter().map(Ty::from).collect(),
            result.clone(),
            inference.fresh_row(),
        );
        inference
            .unify(&function, &expected)
            .map_err(|error| format!("{error:?}"))?;
        let result = Type::try_from(&inference.resolve(&result))
            .map_err(|error| format!("unresolved return type: {error:?}"))?;
        Ok(Self {
            parameters: parameters.to_vec(),
            result,
        })
    }
}

impl From<&Type> for Ty {
    fn from(ty: &Type) -> Self {
        match ty {
            Type::Int => Self::Int,
            Type::Float => Self::Float,
            Type::Bool => Self::Bool,
            Type::Char => Self::Char,
            Type::String => Self::String,
            Type::Unit => Self::Unit,
            Type::Data(name, args) => {
                Self::Adt(name.clone(), args.iter().map(Self::from).collect())
            }
            Type::Abstract(name, args) => {
                Self::Abstract(name.clone(), args.iter().map(Self::from).collect())
            }
            Type::Closure(args, result) => Self::fn_of(
                args.iter().map(Self::from).collect(),
                Self::from(result.as_ref()),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub captures: Vec<(usize, Type)>,
    pub id: FunctionId,
    pub name: String,
    pub signature: Signature,
    pub parameters: Vec<usize>,
    pub body: Expression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub entry: Expression,
    pub functions: Vec<Function>,
}

impl Lowerer {
    fn call(
        &mut self,
        callee: &crate::parser::Node,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        if let crate::parser::NodeKind::Field(module, member) = &callee.kind {
            if let crate::parser::NodeKind::Name(module) = &module.kind {
                let qualified = self.resolve_definition(&format!("{module}.{}", member.name));
                if self.schemes.contains_key(&qualified) {
                    if self.is_value_definition(&qualified) {
                        return Expression::call_closure(self.expression(callee)?, arguments);
                    }
                    return self.direct_call(&qualified, arguments);
                }
            }
        }
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            return self.direct_call(name, arguments);
        }
        Expression::call_closure(self.expression(callee)?, arguments)
    }

    fn direct_call(
        &mut self,
        name: &str,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        if let Some((id, ty)) = self.scope.get(name) {
            return Expression::call_closure(
                Expression::Variable {
                    id: *id,
                    ty: ty.clone(),
                },
                arguments,
            );
        }
        let types: Vec<_> = arguments.iter().map(Expression::ty).collect();
        let (id, signature) = self.specialize_function(name, types, None)?;
        Expression::call_direct(id, &signature, arguments)
    }

    fn propagation(
        &mut self,
        pattern: Option<&crate::parser::Pattern>,
        source: &crate::parser::Node,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let value = self.expression(source)?;
        self.option_propagation(pattern, value, body)
    }

    fn option_propagation(
        &mut self,
        pattern: Option<&crate::parser::Pattern>,
        value: Expression,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let ty = value.ty();
        let Type::Data(container, args) = &ty else {
            return Err("propagation requires Option".into());
        };
        if container != "Option" {
            return Err("propagation requires Option".into());
        }
        let payload = args
            .first()
            .ok_or("missing propagation payload type")?
            .clone();
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty };
        let projected = Expression::Project {
            value: Box::new(variable.clone()),
            constructor: "Some".into(),
            index: 0,
            result: payload,
        };
        let then = match pattern {
            Some(pattern) => self.bound_body(pattern, projected, body)?,
            None => self.expression(body)?,
        };
        let result = then.ty();
        let otherwise = self.construct("None", vec![], &Ty::from(&result))?;
        let switch = Expression::Switch {
            value: Box::new(variable),
            arms: vec![("Some".into(), then), ("None".into(), otherwise)],
            fallback: None,
            result,
        };
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(switch),
        })
    }

    fn literal_branch(
        &mut self,
        scrutinee: &crate::parser::Node,
        arms: &[(crate::parser::Pattern, crate::parser::Node)],
        fallback: Option<&crate::parser::Node>,
    ) -> Result<Expression, String> {
        let value = self.expression(scrutinee)?;
        if let [(pattern, body)] = arms {
            if fallback.is_none() && matches!(pattern.kind, crate::parser::PatternKind::Record(_)) {
                return self.bound_body(pattern, value, body);
            }
        }
        if let Type::Data(name, _) = value.ty() {
            if self.records.contains_key(&name) {
                return self.record_branch(value, arms, fallback);
            }
            return self.constructor_switch(value, arms, fallback);
        }
        if fallback.is_none() && value.ty() == Type::Bool {
            let mut on_true = None;
            let mut on_false = None;
            for (pattern, body) in arms {
                let crate::parser::PatternKind::Literal(literal) = &pattern.kind else {
                    return Err("expected Boolean pattern".into());
                };
                match literal.kind {
                    crate::parser::NodeKind::Bool(true) if on_true.is_none() => {
                        on_true = Some(body)
                    }
                    crate::parser::NodeKind::Bool(false) if on_false.is_none() => {
                        on_false = Some(body)
                    }
                    _ => return Err("invalid Boolean pattern coverage".into()),
                }
            }
            let then = self.expression(on_true.ok_or("missing true pattern")?)?;
            let otherwise = self.expression(on_false.ok_or("missing false pattern")?)?;
            let (then, otherwise) = self.align_outcomes(then, otherwise)?;
            return branch(value, then, otherwise);
        }
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty: value.ty() };
        let mut result =
            self.expression(fallback.ok_or("pattern branch without fallback is not implemented")?)?;
        for (pattern, body) in arms.iter().rev() {
            let crate::parser::PatternKind::Literal(literal) = &pattern.kind else {
                return Err("nonliteral pattern branch is not implemented".into());
            };
            let literal = self.expression(literal)?;
            if literal.ty() != variable.ty() {
                return Err("pattern type mismatch".into());
            }
            let condition = Expression::Binary {
                op: crate::parser::BinOp::Eq,
                left: Box::new(variable.clone()),
                right: Box::new(literal),
            };
            let body = self.expression(body)?;
            let (body, result_arm) = self.align_outcomes(body, result)?;
            result = branch(condition, body, result_arm)?;
        }
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(result),
        })
    }

    fn record_branch(
        &mut self,
        value: Expression,
        arms: &[(crate::parser::Pattern, crate::parser::Node)],
        fallback: Option<&crate::parser::Node>,
    ) -> Result<Expression, String> {
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty: value.ty() };
        let mut next = fallback.map(|body| self.expression(body)).transpose()?;
        for (pattern, body) in arms.iter().rev() {
            let saved = self.scope.clone();
            let mut steps = Vec::new();
            let result = self
                .match_steps(pattern, variable.clone(), &mut steps)
                .and_then(|()| self.expression(body));
            self.scope = saved;
            next = Some(Self::finish_match(steps, result?, next.as_ref())?);
        }
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(next.ok_or("empty record branch")?),
        })
    }

    fn constructor_switch(
        &mut self,
        value: Expression,
        arms: &[(crate::parser::Pattern, crate::parser::Node)],
        fallback: Option<&crate::parser::Node>,
    ) -> Result<Expression, String> {
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty: value.ty() };
        let fallback = fallback
            .map(|body| self.expression(body).map(Box::new))
            .transpose()?;
        let mut lowered: Vec<(String, Expression)> = Vec::new();
        for (pattern, body) in arms.iter().rev() {
            let pattern = crate::named_patterns::positional(pattern, &self.named_constructors, &|name| self.resolve_constructor(name));
            let (name, patterns) = match &pattern.kind {
                crate::parser::PatternKind::Name(name)
                    if crate::names::is_upper_reference(name) =>
                {
                    (name, &[][..])
                }
                crate::parser::PatternKind::Constructor(name, fields) => (name, fields.as_slice()),
                _ => return Err("expected constructor pattern".into()),
            };
            let name = &self.resolve_constructor(name);
            let position = lowered.iter().position(|(previous, _)| previous == name);
            let failure = position
                .map(|index| &lowered[index].1)
                .or(fallback.as_deref());
            let arm = self.constructor_arm(name, patterns, &variable, body, failure)?;
            if let Some(index) = position {
                lowered[index].1 = arm;
            } else {
                lowered.push((name.clone(), arm));
            }
        }
        lowered.reverse();
        let (lowered, fallback) = self.align_switch_arms(lowered, fallback)?;
        let result = lowered
            .first()
            .map(|(_, body)| body.ty())
            .or_else(|| fallback.as_ref().map(|body| body.ty()))
            .ok_or("empty switch")?;
        if lowered.iter().any(|(_, body)| body.ty() != result)
            || fallback.as_ref().is_some_and(|body| body.ty() != result)
        {
            return Err("switch result types differ".into());
        }
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(Expression::Switch {
                value: Box::new(variable),
                arms: lowered,
                fallback,
                result,
            }),
        })
    }

    fn constructor_arm(
        &mut self,
        name: &str,
        patterns: &[crate::parser::Pattern],
        value: &Expression,
        body: &crate::parser::Node,
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        let name = &self.resolve_constructor(name);
        let fields = self.constructor_fields(name, &value.ty())?;
        if fields.len() != patterns.len() {
            return Err("constructor pattern arity mismatch".into());
        }
        let saved = self.scope.clone();
        let mut steps = Vec::new();
        let result = (|| {
            for (index, (pattern, result)) in patterns.iter().zip(fields).enumerate() {
                let field = Expression::Project {
                    value: Box::new(value.clone()),
                    constructor: name.into(),
                    index,
                    result,
                };
                self.match_steps(pattern, field, &mut steps)?;
            }
            self.expression(body)
        })();
        self.scope = saved;
        Self::finish_match(steps, result?, fallback)
    }

    fn finish_match(
        steps: Vec<MatchStep>,
        mut body: Expression,
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        for step in steps.into_iter().rev() {
            body = match step {
                MatchStep::Bind(id, value) => Expression::Let {
                    id,
                    value: Box::new(value),
                    body: Box::new(body),
                },
                MatchStep::Tag(tag, value) => {
                    let result = body.ty();
                    Expression::Switch {
                        value: Box::new(value),
                        arms: vec![(tag, body)],
                        fallback: fallback.cloned().map(Box::new),
                        result,
                    }
                }
                MatchStep::Equal(value, literal) => {
                    let failure = fallback
                        .cloned()
                        .unwrap_or_else(|| Expression::Unreachable { result: body.ty() });
                    let condition = Expression::Binary {
                        op: crate::parser::BinOp::Eq,
                        left: Box::new(value),
                        right: Box::new(literal),
                    };
                    branch(condition, body, failure)?
                }
            };
        }
        Ok(body)
    }

    fn match_steps(
        &mut self,
        pattern: &crate::parser::Pattern,
        value: Expression,
        steps: &mut Vec<MatchStep>,
    ) -> Result<(), String> {
        use crate::parser::PatternKind;
        let pattern = crate::named_patterns::positional(pattern, &self.named_constructors, &|name| self.resolve_constructor(name));
        match &pattern.kind {
            PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
                let name = &self.resolve_constructor(name);
                if !self.constructor_fields(name, &value.ty())?.is_empty() {
                    return Err("constructor pattern needs fields".into());
                }
                steps.push(MatchStep::Tag(name.clone(), value));
            }
            PatternKind::Constructor(name, patterns) => {
                let name = &self.resolve_constructor(name);
                let fields = self.constructor_fields(name, &value.ty())?;
                if fields.len() != patterns.len() {
                    return Err("constructor pattern arity mismatch".into());
                }
                steps.push(MatchStep::Tag(name.clone(), value.clone()));
                for (index, (pattern, result)) in patterns.iter().zip(fields).enumerate() {
                    let field = Expression::Project {
                        value: Box::new(value.clone()),
                        constructor: name.clone(),
                        index,
                        result,
                    };
                    self.match_steps(pattern, field, steps)?;
                }
            }
            PatternKind::Literal(literal) => {
                let literal = self.expression(literal)?;
                if value.ty() != literal.ty() {
                    return Err("literal pattern type mismatch".into());
                }
                steps.push(MatchStep::Equal(value, literal));
            }
            PatternKind::NamedConstructor(_, _) => return Err("missing named constructor metadata".into()),
            PatternKind::Record(fields) => {
                for (name, nested) in fields {
                    let field = self.record_get(value.clone(), &name.name)?;
                    let shorthand = crate::parser::Pattern {
                        kind: PatternKind::Name(name.name.clone()),
                        span: name.span,
                    };
                    self.match_steps(nested.as_ref().unwrap_or(&shorthand), field, steps)?;
                }
            }
            _ => {
                let mut bindings = Vec::new();
                self.bind_pattern(&pattern, value, &mut bindings)?;
                steps.extend(
                    bindings
                        .into_iter()
                        .map(|(id, value)| MatchStep::Bind(id, value)),
                );
            }
        }
        Ok(())
    }

    fn bound_body(
        &mut self,
        pat: &crate::parser::Pattern,
        value: Expression,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let saved = self.scope.clone();
        let mut bindings = Vec::new();
        let result = self
            .bind_pattern(pat, value, &mut bindings)
            .and_then(|()| self.expression(body));
        self.scope = saved;
        let mut body = result?;
        for (id, value) in bindings.into_iter().rev() {
            body = Expression::Let {
                id,
                value: Box::new(value),
                body: Box::new(body),
            };
        }
        Ok(body)
    }

    fn bind_pattern(
        &mut self,
        pattern: &crate::parser::Pattern,
        value: Expression,
        bindings: &mut Vec<(usize, Expression)>,
    ) -> Result<(), String> {
        use crate::parser::PatternKind;
        let pattern = crate::named_patterns::positional(pattern, &self.named_constructors, &|name| self.resolve_constructor(name));
        let id = self.next;
        self.next += 1;
        let ty = value.ty();
        bindings.push((id, value));
        match &pattern.kind {
            PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
                return Err("nested constructor pattern requires a tag check".into());
            }
            PatternKind::Name(name) => {
                self.scope.insert(name.clone(), (id, ty));
            }
            PatternKind::Constructor(name, patterns) => {
                let alternatives = self
                    .constructors
                    .keys()
                    .filter(|candidate| self.constructor_fields(candidate, &ty).is_ok())
                    .count();
                if alternatives != 1 {
                    return Err("refutable constructor binding requires a tag check".into());
                }
                let name = &self.resolve_constructor(name);
                let fields = self.constructor_fields(name, &ty)?;
                if fields.len() != patterns.len() {
                    return Err("constructor pattern arity mismatch".into());
                }
                for (index, (pattern, result)) in patterns.iter().zip(fields).enumerate() {
                    let projection = Expression::Project {
                        value: Box::new(Expression::Variable { id, ty: ty.clone() }),
                        constructor: name.clone(),
                        index,
                        result,
                    };
                    self.bind_pattern(pattern, projection, bindings)?;
                }
            }
            PatternKind::NamedConstructor(_, _) => return Err("missing named constructor metadata".into()),
            PatternKind::Record(fields) => {
                for (field, nested) in fields {
                    let value =
                        self.record_get(Expression::Variable { id, ty: ty.clone() }, &field.name)?;
                    let shorthand = crate::parser::Pattern {
                        kind: PatternKind::Name(field.name.clone()),
                        span: field.span,
                    };
                    self.bind_pattern(nested.as_ref().unwrap_or(&shorthand), value, bindings)?;
                }
            }
            PatternKind::Underscore => {}
            _ => return Err("pattern binding lowering is not implemented for this pattern".into()),
        }
        Ok(())
    }

    fn constructor_fields(&self, name: &str, ty: &Type) -> Result<Vec<Type>, String> {
        let scheme = self.constructors.get(name).ok_or("unknown constructor")?;
        let mut inference = crate::types::Inference::default();
        let instantiated = inference.instantiate(scheme);
        let (fields, result) = match instantiated {
            Ty::Fn(fields, result, _) => (fields, *result),
            result => (vec![], result),
        };
        inference
            .unify(&result, &Ty::from(ty))
            .map_err(|error| format!("constructor type mismatch: {error:?}"))?;
        fields
            .iter()
            .map(|field| {
                Type::try_from(&inference.resolve(field))
                    .map_err(|_| "constructor field type is not concrete".into())
            })
            .collect()
    }

    fn named_constructor(
        &mut self, node: &crate::parser::Node, name: &str,
        written: &[(crate::parser::DeclaredName, crate::parser::Node)],
    ) -> Result<Expression, String> {
        let name = self.resolve_constructor(name);
        let declared = self.named_constructors.get(&name).ok_or("missing named constructor metadata")?.clone();
        let ty = self.composite_types.get(&(node.span.line, node.span.col)).ok_or("missing named constructor type")?.clone();
        let mut bindings = Vec::new();
        let mut variables = std::collections::HashMap::new();
        for (field, value) in written {
            let value = self.expression(value)?;
            let id = self.next;
            self.next += 1;
            variables.insert(field.name.clone(), Expression::Variable { id, ty: value.ty() });
            bindings.push((id, value));
        }
        let fields = declared.iter().map(|name| variables.remove(name).ok_or_else(|| "missing named field".to_owned())).collect::<Result<Vec<_>, _>>()?;
        let mut body = self.construct(&name, fields, &ty)?;
        for (id, value) in bindings.into_iter().rev() {
            body = Expression::Let { id, value: Box::new(value), body: Box::new(body) };
        }
        Ok(body)
    }

    fn record_get(&self, record: Expression, field: &str) -> Result<Expression, String> {
        let ty = record.ty();
        let Type::Data(name, _) = &ty else {
            return Err("field access requires a record".into());
        };
        if !self.records.contains_key(name) {
            for (constructor, names) in &self.named_constructors {
                let Ok(fields) = self.constructor_fields(constructor, &ty) else { continue; };
                let index = names.iter().position(|name| name == field).ok_or("unknown named field")?;
                return Ok(Expression::Project { value: Box::new(record), constructor: constructor.clone(), index, result: fields[index].clone() });
            }
            return Err("unknown record type".into());
        }
        let (names, scheme) = self.records.get(name).ok_or("unknown record type")?;
        let index = names
            .iter()
            .position(|name| name == field)
            .ok_or("unknown record field")?;
        let mut inference = crate::types::Inference::default();
        let Ty::Fn(fields, result, _) = inference.instantiate(scheme) else {
            return Err("invalid record schema".into());
        };
        inference
            .unify(&result, &Ty::from(&ty))
            .map_err(|error| format!("record type mismatch: {error:?}"))?;
        let result = Type::try_from(&inference.resolve(&fields[index]))
            .map_err(|_| "record field type is not concrete")?;
        Ok(Expression::RecordGet {
            record: Box::new(record),
            field: field.into(),
            result,
        })
    }

    fn construct(
        &self,
        name: &str,
        fields: Vec<Expression>,
        ty: &Ty,
    ) -> Result<Expression, String> {
        let result = Type::try_from(&self.specialization.resolve(ty))
            .map_err(|_| "constructor type is not concrete")?;
        if !matches!(result, Type::Data(..)) {
            return Err("constructor result must be data".into());
        }
        Ok(Expression::Construct {
            constructor: self.resolve_constructor(name),
            fields,
            result,
        })
    }

    fn application(
        &mut self,
        node: &crate::parser::Node,
        callee: &crate::parser::Node,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            if let Some(kind) = self.operations.get(name).copied() {
                return self.operation(node, name, arguments, kind);
            }
        }
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            if let Some(function) = crate::runtime_abi::RuntimeFunction::from_intrinsic(name) {
                return Expression::call_runtime(function, arguments);
            }
        }
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            if crate::names::is_upper_reference(name) {
                let ty = self
                    .composite_types
                    .get(&(node.span.line, node.span.col))
                    .ok_or("missing constructor result type")?;
                return self.construct(name, arguments, ty);
            }
        }
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            if !self.scope.contains_key(name) {
                let qualified = self.resolve_definition(name);
                if self.is_value_definition(&qualified) {
                    let closure = self.expression(callee)?;
                    return Expression::call_closure(closure, arguments);
                }
                let expected = self
                    .composite_types
                    .get(&(node.span.line, node.span.col))
                    .map(|ty| Type::try_from(&self.specialization.resolve(ty)))
                    .transpose()
                    .map_err(|_| "call result type is not concrete")?;
                let parameters = arguments.iter().map(Expression::ty).collect();
                let (id, signature) =
                    self.specialize_function(name, parameters, expected.as_ref())?;
                return Expression::call_direct(id, &signature, arguments);
            }
        }
        self.call(callee, arguments)
    }

    fn abort_raise(
        &mut self,
        node: &crate::parser::Node,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        let message = match arguments.len() {
            1 => Expression::RecordGet {
                record: Box::new(arguments.into_iter().next().unwrap()),
                field: "message".into(),
                result: Type::String,
            },
            0 => Expression::Constant(Constant::String(String::new())),
            _ => return Err("abort operation has too many arguments".into()),
        };
        let ty = self
            .composite_types
            .get(&(node.span.line, node.span.col))
            .or_else(|| self.value_types.get(&(node.span.line, node.span.col)))
            .ok_or("missing abort result type")?;
        let result = match Type::try_from(&self.specialization.resolve(ty)) {
            Ok(ty) => ty,
            Err(UnresolvedType::Variable(_)) => Type::Unit,
            Err(_) => return Err("abort result type is not concrete".into()),
        };
        let call = Expression::call_runtime(
            crate::runtime_abi::RuntimeFunction::AbortError,
            vec![message],
        )?;
        let id = self.next;
        self.next += 1;
        Ok(Expression::Let {
            id,
            value: Box::new(call),
            body: Box::new(Expression::Unreachable { result }),
        })
    }

    fn operation(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
        arguments: Vec<Expression>,
        kind: crate::parser::OpKind,
    ) -> Result<Expression, String> {
        match kind {
            crate::parser::OpKind::Capture => self.capture_operation(name, arguments, None),
            crate::parser::OpKind::Reply => self.handled_operation(name, arguments),
            crate::parser::OpKind::Abort => match self.handler_for(name).map(|handler| handler.effect.clone()) {
                Some(effect) => self.abort_outcome(node, arguments, &effect),
                None => self.abort_raise(node, arguments),
            },
        }
    }

    fn with_handler(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
        args: &[crate::parser::Node],
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let handler = self
            .handlers
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unknown handler {name}"))?;
        for arg in args {
            let _ = self.expression(arg)?;
        }
        let needs_answer = handler.arms.iter().any(|arm| {
            matches!(
                arm,
                crate::parser::HandlerArm::Return { .. }
                    | crate::parser::HandlerArm::Operation {
                        kind: crate::parser::OpKind::Capture | crate::parser::OpKind::Abort,
                        ..
                    }
            )
        });
        let answer = if needs_answer { self.capture_answer_of(node)? } else { None };
        if let Some(answer) = answer.clone() {
            self.capture_answers.push(answer);
        }
        let saved = std::mem::replace(&mut self.rewriting_return, false);
        self.handler_stack.push(handler.clone());
        let value = self.completion(body);
        self.handler_stack.pop();
        self.rewriting_return = saved;
        if answer.is_some() {
            self.capture_answers.pop();
        }
        value
    }

    fn handled_operation(
        &mut self,
        name: &str,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        let handler = self
            .handler_for(name)
            .cloned()
            .ok_or_else(|| format!("unhandled operation {name}"))?;
        let (params, body) = handler.arms.iter().find_map(|arm| match arm {
            crate::parser::HandlerArm::Operation { name: op, params, body, .. } if op == name => {
                Some((params.clone(), body.clone()))
            }
            _ => None,
        }).ok_or_else(|| format!("missing handler arm for {name}"))?;
        self.with_params(&params, arguments, |this| {
            let saved = std::mem::replace(&mut this.rewriting_return, true);
            let result = this.expression(&body);
            this.rewriting_return = saved;
            result
        })
    }

    fn with_params(
        &mut self,
        params: &[crate::parser::Param],
        arguments: Vec<Expression>,
        f: impl FnOnce(&mut Self) -> Result<Expression, String>,
    ) -> Result<Expression, String> {
        if params.len() != arguments.len() {
            return Err("handler arm arity mismatch".into());
        }
        let mut bindings = Vec::new();
        let mut saved = Vec::new();
        for (param, argument) in params.iter().zip(arguments) {
            let id = self.next;
            self.next += 1;
            let ty = argument.ty();
            if param.name != "_" {
                saved.push((param.name.clone(), self.scope.insert(param.name.clone(), (id, ty))));
            }
            bindings.push((id, argument));
        }
        let result = f(self);
        for (name, previous) in saved.into_iter().rev() {
            if let Some(value) = previous {
                self.scope.insert(name, value);
            } else {
                self.scope.remove(&name);
            }
        }
        let mut body = result?;
        for (id, value) in bindings.into_iter().rev() {
            body = Expression::Let {
                id,
                value: Box::new(value),
                body: Box::new(body),
            };
        }
        Ok(body)
    }

    fn handler_for(&self, operation: &str) -> Option<&crate::parser::HandlerDecl> {
        self.handler_stack.iter().rev().find(|handler| {
            handler.arms.iter().any(|arm| {
                matches!(arm, crate::parser::HandlerArm::Operation { name, .. } if name == operation)
            })
        })
    }

    fn handler_for_node(&self, node: &crate::parser::Node) -> Option<&crate::parser::HandlerDecl> {
        self.operation_name(node).and_then(|name| self.handler_for(name))
    }

    fn completion(&mut self, node: &crate::parser::Node) -> Result<Expression, String> {
        let value = self.expression(node)?;
        self.finish_handler(value)
    }

    fn specialized_name(&self, name: &str) -> String {
        if self.handler_stack.is_empty() {
            return name.into();
        }
        let handlers = self.handler_stack.iter().map(|handler| handler.name.as_str()).collect::<Vec<_>>().join("/");
        format!("{name}#{handlers}")
    }

    fn scheme_carries_effects(scheme: &crate::types::Scheme) -> bool {
        let mut inference = crate::types::Inference::default();
        match inference.instantiate(scheme) {
            Ty::Fn(_, _, row) => row.has_known_effects(),
            _ => false,
        }
    }

    fn is_abort_node(&self, node: &crate::parser::Node) -> bool {
        self.operation_name(node)
            .is_some_and(|name| self.operations.get(name) == Some(&crate::parser::OpKind::Abort))
            || matches!(&node.kind, crate::parser::NodeKind::DoQ { value, .. } if self.is_abort_node(value))
    }

    fn operation_name<'a>(&self, node: &'a crate::parser::Node) -> Option<&'a str> {
        match &node.kind {
            crate::parser::NodeKind::Name(name) => Some(name.as_str()),
            crate::parser::NodeKind::Apply(callee, _) | crate::parser::NodeKind::Call(callee, _) => {
                if let crate::parser::NodeKind::Name(name) = &callee.kind {
                    Some(name.as_str())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn is_capture_node(&self, node: &crate::parser::Node) -> bool {
        self.operation_name(node)
            .is_some_and(|name| self.operations.get(name) == Some(&crate::parser::OpKind::Capture))
            || matches!(&node.kind, crate::parser::NodeKind::DoQ { value, .. } if self.is_capture_node(value))
    }

    fn capture_answer_of(&self, node: &crate::parser::Node) -> Result<Option<Type>, String> {
        match self.composite_types.get(&(node.span.line, node.span.col)) {
            Some(ty) => Type::try_from(&self.specialization.resolve(ty))
                .map(Some)
                .map_err(|_| "with type is not concrete".into()),
            None => Ok(None),
        }
    }

    fn capture_do(
        &mut self,
        value: &crate::parser::Node,
        rest: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let name = self.operation_name(value).ok_or("capture do without operation")?.to_string();
        let arguments = match &value.kind {
            crate::parser::NodeKind::Apply(_, args) | crate::parser::NodeKind::Call(_, args) => {
                args.iter().map(|arg| self.expression(arg)).collect::<Result<Vec<_>, _>>()?
            }
            _ => Vec::new(),
        };
        self.capture_operation(&name, arguments, Some(rest))
    }

    fn capture_operation(
        &mut self,
        _name: &str,
        arguments: Vec<Expression>,
        rest: Option<&crate::parser::Node>,
    ) -> Result<Expression, String> {
        let answer = self.capture_answers.last().cloned().ok_or("unhandled capture")?;
        let continuation = match rest {
            Some(body) => self.make_continuation(body, &answer)?,
            None => {
                let unit = self.finish_handler(Expression::Constant(Constant::Unit))?;
                self.continuation_closure(unit, &answer)?
            }
        };
        let success = Type::Unit;
        let payload = arguments.first().map(Expression::ty).unwrap_or(Type::Unit);
        let outcome = Type::Data(OUTCOME.into(), vec![success, answer, payload]);
        let value = arguments.into_iter().next().unwrap_or(Expression::Constant(Constant::Unit));
        Ok(Expression::Construct {
            constructor: YIELD.into(),
            fields: vec![value, continuation],
            result: outcome,
        })
    }

    fn make_continuation(
        &mut self,
        body: &crate::parser::Node,
        answer: &Type,
    ) -> Result<Expression, String> {
        let saved = std::mem::take(&mut self.scope);
        for (name, binding) in &saved {
            if crate::names::references(body, name) {
                self.scope.insert(name.clone(), binding.clone());
            }
        }
        let captures: Vec<_> = self.scope.values().cloned().collect();
        let param = self.next;
        self.next += 1;
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        let lowered = self.completion(body);
        self.scope = saved;
        let body = lowered?.in_tail_position();
        if body.ty() != *answer {
            return Err("continuation result type mismatch".into());
        }
        self.continuation_function(id, param, captures, body, answer)
    }

    fn continuation_closure(&mut self, body: Expression, answer: &Type) -> Result<Expression, String> {
        let param = self.next;
        self.next += 1;
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.continuation_function(id, param, Vec::new(), body.in_tail_position(), answer)
    }

    fn continuation_function(
        &mut self,
        id: FunctionId,
        param: usize,
        captures: Vec<(usize, Type)>,
        body: Expression,
        answer: &Type,
    ) -> Result<Expression, String> {
        let signature = Signature { parameters: vec![Type::Unit], result: answer.clone() };
        self.functions.push(Function {
            id,
            name: format!("cont_{}", id.0),
            signature: signature.clone(),
            parameters: vec![param],
            captures: captures.clone(),
            body,
        });
        Ok(Expression::MakeClosure {
            function: id,
            signature,
            captures: captures
                .into_iter()
                .map(|(id, ty)| Expression::Variable { id, ty })
                .collect(),
        })
    }

    fn finish_handler(&mut self, value: Expression) -> Result<Expression, String> {
        let Some(handler) = self.handler_stack.last().cloned() else {
            return Ok(value);
        };
        if is_outcome(&value.ty()) {
            if self.rewriting_return {
                return self.take_done(value);
            }
            return self.interpret_outcome(&handler, value);
        }
        if self.rewriting_return {
            return Ok(value);
        }
        self.apply_return_arm(&handler, value)
    }

    fn take_done(&mut self, value: Expression) -> Result<Expression, String> {
        let ty = value.ty();
        let success = match &ty {
            Type::Data(name, args) if name == OUTCOME => {
                args.first().cloned().ok_or("outcome missing success type")?
            }
            _ => return Err("expected outcome".into()),
        };
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty: ty.clone() };
        let done = Expression::Project {
            value: Box::new(variable.clone()),
            constructor: DONE.into(),
            index: 0,
            result: success.clone(),
        };
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(Expression::Switch {
                value: Box::new(variable),
                arms: vec![(DONE.into(), done)],
                fallback: None,
                result: success,
            }),
        })
    }

    fn interpret_outcome(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        value: Expression,
    ) -> Result<Expression, String> {
        let ty = value.ty();
        let args = match &ty {
            Type::Data(name, args) if name == OUTCOME => args,
            _ => return Err("expected outcome".into()),
        };
        let success = args.first().cloned().ok_or("outcome missing success type")?;
        let answer = args.get(1).cloned().unwrap_or_else(|| success.clone());
        let payload = args.get(2).cloned().unwrap_or(Type::Unit);
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty: ty.clone() };
        let done_value = Expression::Project {
            value: Box::new(variable.clone()),
            constructor: DONE.into(),
            index: 0,
            result: success,
        };
        let done = self.apply_return_arm(handler, done_value)?;
        let mut arms = vec![(DONE.into(), done)];
        if handler.arms.iter().any(|arm| {
            matches!(arm, crate::parser::HandlerArm::Operation { kind: crate::parser::OpKind::Abort, .. })
        }) {
            let fail = fail_constructor(&handler.effect);
            let fail_value = Expression::Project {
                value: Box::new(variable.clone()),
                constructor: fail.clone(),
                index: 0,
                result: payload.clone(),
            };
            arms.push((fail, self.abort_arm(handler, fail_value)?));
        }
        if handler.arms.iter().any(|arm| {
            matches!(arm, crate::parser::HandlerArm::Operation { kind: crate::parser::OpKind::Capture, .. })
        }) {
            let yield_value = Expression::Project {
                value: Box::new(variable.clone()),
                constructor: YIELD.into(),
                index: 0,
                result: payload,
            };
            let resume = Expression::Project {
                value: Box::new(variable.clone()),
                constructor: YIELD.into(),
                index: 1,
                result: Type::Closure(vec![Type::Unit], Box::new(answer)),
            };
            arms.push((YIELD.into(), self.yield_arm(handler, yield_value, resume)?));
        }
        let forwarding = self.handler_stack.len() > 1;
        let (arms, fallback) = if forwarding {
            self.align_switch_arms(arms, Some(Box::new(variable.clone())))?
        } else {
            (arms, None)
        };
        let result = arms
            .first()
            .map(|(_, body)| body.ty())
            .or_else(|| fallback.as_ref().map(|body| body.ty()))
            .ok_or("handler produced no arms")?;
        if arms.iter().any(|(_, body)| body.ty() != result)
            || fallback.as_ref().is_some_and(|body| body.ty() != result)
        {
            return Err("handler arms disagree on the answer type".into());
        }
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(Expression::Switch {
                value: Box::new(variable),
                arms,
                fallback,
                result,
            }),
        })
    }

    fn abort_arm(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        payload: Expression,
    ) -> Result<Expression, String> {
        let Some((params, body)) = handler.arms.iter().find_map(|arm| match arm {
            crate::parser::HandlerArm::Operation { kind: crate::parser::OpKind::Abort, params, body, .. } => {
                Some((params.clone(), body.clone()))
            }
            _ => None,
        }) else {
            return Err("missing abort arm".into());
        };
        let arguments = match params.len() {
            0 => Vec::new(),
            1 => vec![payload],
            _ => return Err("abort arm arity mismatch".into()),
        };
        self.with_params(&params, arguments, |this| {
            let saved = std::mem::replace(&mut this.rewriting_return, true);
            let result = this.expression(&body).and_then(|value| this.finish_handler(value));
            this.rewriting_return = saved;
            result
        })
    }

    fn yield_arm(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        value: Expression,
        resume: Expression,
    ) -> Result<Expression, String> {
        let Some((params, body)) = handler.arms.iter().find_map(|arm| match arm {
            crate::parser::HandlerArm::Operation {
                kind: crate::parser::OpKind::Capture,
                params,
                body,
                ..
            } => Some((params.clone(), body.clone())),
            _ => None,
        }) else {
            return Err("missing capture arm".into());
        };
        let id = self.next;
        self.next += 1;
        let resume_ty = resume.ty();
        let saved_resume = self.resume_binding.replace((id, resume_ty));
        let result = self.with_params(&params, vec![value], |this| {
            let saved = std::mem::replace(&mut this.rewriting_return, true);
            let result = this.expression(&body).and_then(|value| this.finish_handler(value));
            this.rewriting_return = saved;
            result
        });
        self.resume_binding = saved_resume;
        Ok(Expression::Let {
            id,
            value: Box::new(resume),
            body: Box::new(result?),
        })
    }

    fn abort_outcome(
        &mut self,
        node: &crate::parser::Node,
        arguments: Vec<Expression>,
        effect: &str,
    ) -> Result<Expression, String> {
        let success = self.node_type(node).unwrap_or(Type::Unit);
        let answer = self.capture_answers.last().cloned().ok_or("abort without handler answer")?;
        let payload_value = arguments.into_iter().next().unwrap_or(Expression::Constant(Constant::Unit));
        let payload = payload_value.ty();
        let outcome = Type::Data(OUTCOME.into(), vec![success, answer, payload]);
        Ok(Expression::Construct {
            constructor: fail_constructor(effect),
            fields: vec![payload_value],
            result: outcome,
        })
    }

    fn apply_return_arm(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        value: Expression,
    ) -> Result<Expression, String> {
        let Some(crate::parser::HandlerArm::Return { param, body: ret, .. }) = handler
            .arms
            .iter()
            .find(|arm| matches!(arm, crate::parser::HandlerArm::Return { .. }))
            .cloned()
        else {
            return Ok(value);
        };
        let saved = std::mem::replace(&mut self.rewriting_return, true);
        let result = self.with_params(&[param], vec![value], |this| this.expression(&ret));
        self.rewriting_return = saved;
        result
    }

    fn align_outcomes(
        &self,
        left: Expression,
        right: Expression,
    ) -> Result<(Expression, Expression), String> {
        match (is_outcome(&left.ty()), is_outcome(&right.ty())) {
            (false, false) => Ok((left, right)),
            (true, true) if left.ty() == right.ty() => Ok((left, right)),
            (true, false) => {
                let ty = left.ty();
                Ok((left, self.done(right, &ty)?))
            }
            (false, true) => {
                let ty = right.ty();
                Ok((self.done(left, &ty)?, right))
            }
            _ => Err("outcome types differ".into()),
        }
    }

    fn align_switch_arms(
        &self,
        mut arms: Vec<(String, Expression)>,
        mut fallback: Option<Box<Expression>>,
    ) -> Result<(Vec<(String, Expression)>, Option<Box<Expression>>), String> {
        let outcome = arms
            .iter()
            .map(|(_, body)| body.ty())
            .chain(fallback.as_ref().map(|body| body.ty()))
            .find(is_outcome);
        let Some(outcome) = outcome else {
            return Ok((arms, fallback));
        };
        for (_, body) in &mut arms {
            if !is_outcome(&body.ty()) {
                *body = self.done(body.clone(), &outcome)?;
            }
        }
        if let Some(body) = fallback.as_mut() {
            if !is_outcome(&body.ty()) {
                *body = Box::new(self.done(body.as_ref().clone(), &outcome)?);
            }
        }
        Ok((arms, fallback))
    }

    fn done(&self, value: Expression, outcome: &Type) -> Result<Expression, String> {
        let Type::Data(name, args) = outcome else {
            return Err("done requires an outcome type".into());
        };
        if name != OUTCOME {
            return Err("done requires an outcome type".into());
        }
        let success = args.first().ok_or("outcome missing success type")?;
        if value.ty() != *success {
            return Err("done payload mismatch".into());
        }
        Ok(Expression::Construct {
            constructor: DONE.into(),
            fields: vec![value],
            result: outcome.clone(),
        })
    }

    fn propagate_outcome(
        &mut self,
        pat: &crate::parser::Pattern,
        value: Expression,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let ty = value.ty();
        let success = match &ty {
            Type::Data(name, args) if name == OUTCOME => args.first().cloned().ok_or("outcome missing success type")?,
            _ => return Err("propagate requires an outcome".into()),
        };
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty: ty.clone() };
        let payload = Expression::Project {
            value: Box::new(variable.clone()),
            constructor: DONE.into(),
            index: 0,
            result: success,
        };
        let mut rest = self.bound_body(pat, payload, body)?;
        if !is_outcome(&rest.ty()) {
            rest = self.done(rest, &ty)?;
        }
        let result = rest.ty();
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(Expression::Switch {
                value: Box::new(variable),
                arms: vec![(DONE.into(), rest)],
                fallback: Some(Box::new(Expression::Variable { id, ty })),
                result,
            }),
        })
    }

    fn handler_needs_outcome(&self) -> bool {
        self.handler_stack.last().is_some_and(|handler| {
            handler.arms.iter().any(|arm| {
                matches!(
                    arm,
                    crate::parser::HandlerArm::Operation {
                        kind: crate::parser::OpKind::Abort | crate::parser::OpKind::Capture,
                        ..
                    }
                )
            })
        })
    }

    fn outcome_of(&self, success: &Type, body: &crate::parser::Node) -> Result<Type, String> {
        let answer = self.capture_answers.last().cloned().ok_or("handler answer missing")?;
        Ok(Type::Data(OUTCOME.into(), vec![success.clone(), answer, self.payload_in(body)]))
    }

    fn payload_in(&self, node: &crate::parser::Node) -> Type {
        self.find_payload(node).unwrap_or(Type::Unit)
    }

    fn find_payload(&self, node: &crate::parser::Node) -> Option<Type> {
        use crate::parser::NodeKind::*;
        match &node.kind {
            Apply(callee, args) | Call(callee, args) => {
                if let crate::parser::NodeKind::Name(name) = &callee.kind {
                    if matches!(
                        self.operations.get(name),
                        Some(crate::parser::OpKind::Abort | crate::parser::OpKind::Capture)
                    ) {
                        return args.first().and_then(|arg| self.node_type(arg)).or(Some(Type::Unit));
                    }
                }
                self.find_payload(callee).or_else(|| args.iter().find_map(|arg| self.find_payload(arg)))
            }
            Name(name)
                if matches!(
                    self.operations.get(name),
                    Some(crate::parser::OpKind::Abort | crate::parser::OpKind::Capture)
                ) =>
            {
                Some(Type::Unit)
            }
            Neg(inner) | Not(inner) | Field(inner, _) => self.find_payload(inner),
            Bin(_, left, right) | Pipe(left, right) => {
                self.find_payload(left).or_else(|| self.find_payload(right))
            }
            Record(fields) => fields.iter().find_map(|(_, node)| self.find_payload(node)),
            NamedConstructor(_, fields) => fields.iter().find_map(|(_, node)| self.find_payload(node)),
            List(items, rest) => items
                .iter()
                .find_map(|node| self.find_payload(node))
                .or_else(|| rest.as_ref().and_then(|node| self.find_payload(node))),
            Let { value, body, .. } | LetQ { value, body, .. } | DoQ { value, body } => {
                self.find_payload(value).or_else(|| self.find_payload(body))
            }
            Fn { body, .. } => self.find_payload(body),
            BoolBranch { cond, then, els } => self
                .find_payload(cond)
                .or_else(|| self.find_payload(then))
                .or_else(|| self.find_payload(els)),
            CondBranch { arms, fallback } => arms
                .iter()
                .find_map(|(condition, body)| self.find_payload(condition).or_else(|| self.find_payload(body)))
                .or_else(|| fallback.as_ref().and_then(|node| self.find_payload(node))),
            PatternBranch { scrutinee, arms, fallback } => self
                .find_payload(scrutinee)
                .or_else(|| arms.iter().find_map(|(_, body)| self.find_payload(body)))
                .or_else(|| fallback.as_ref().and_then(|node| self.find_payload(node))),
            With { args, body, .. } => args
                .iter()
                .find_map(|node| self.find_payload(node))
                .or_else(|| self.find_payload(body)),
            _ => None,
        }
    }

    fn node_type(&self, node: &crate::parser::Node) -> Option<Type> {
        self.composite_types
            .get(&(node.span.line, node.span.col))
            .or_else(|| self.value_types.get(&(node.span.line, node.span.col)))
            .and_then(|ty| Type::try_from(&self.specialization.resolve(ty)).ok())
    }

    fn resume_variable(&self) -> Result<Expression, String> {
        let (id, ty) = self.resume_binding.clone().ok_or("resume without capture")?;
        Ok(Expression::Variable { id, ty })
    }

    fn lambda(
        &mut self,
        node: &crate::parser::Node,
        params: &[crate::parser::Param],
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let ty = self
            .value_types
            .get(&(node.span.line, node.span.col))
            .ok_or("missing lambda type")?;
        let Type::Closure(types, result) = Type::try_from(&self.specialization.resolve(ty))
            .map_err(|_| "lambda type is not concrete")?
        else {
            return Err("expected function type".into());
        };
        let signature = Signature {
            parameters: types.clone(),
            result: *result,
        };
        let saved = std::mem::take(&mut self.scope);
        for (name, binding) in &saved {
            if !params.iter().any(|param| &param.name == name)
                && crate::names::references(body, name)
            {
                self.scope.insert(name.clone(), binding.clone());
            }
        }
        let captures: Vec<_> = self.scope.values().cloned().collect();
        let mut parameters = Vec::new();
        for (param, ty) in params.iter().zip(types) {
            let id = self.next;
            self.next += 1;
            if param.name != "_" { self.scope.insert(param.name.clone(), (id, ty)); }
            parameters.push(id);
        }
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        let body = self.expression(body);
        self.scope = saved;
        let body = body?.in_tail_position();
        if body.ty() != signature.result {
            return Err("lambda return type mismatch".into());
        }
        self.functions.push(Function {
            id,
            name: format!("lambda_{}", id.0),
            signature: signature.clone(),
            parameters,
            captures: captures.clone(),
            body,
        });
        Ok(Expression::MakeClosure {
            function: id,
            signature,
            captures: captures
                .into_iter()
                .map(|(id, ty)| Expression::Variable { id, ty })
                .collect(),
        })
    }

    fn structural_equality(&mut self, left: Expression, right: Expression) -> Result<Expression, String> {
        let ty = left.ty();
        if matches!(ty, Type::Closure(..) | Type::Abstract(..)) { return Err("equality needs a concrete non-function type".into()); }
        let signature = Signature { parameters: vec![ty.clone(), ty.clone()], result: Type::Bool };
        if let Some((_, id)) = self.structural_equalities.iter().find(|(cached, _)| cached == &ty) {
            return Expression::call_direct(*id, &signature, vec![left, right]);
        }
        let fields = match &ty { Type::Data(name, _) => self.records.get(name).map(|record| record.0.clone()), _ => None };
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.structural_equalities.push((ty.clone(), id));
        let lhs = self.next;
        let rhs = lhs + 1;
        self.next += 2;
        let mut body = Expression::Constant(Constant::Bool(true));
        for field in fields.iter().flatten().rev() {
            let a = self.record_get(Expression::Variable { id: lhs, ty: ty.clone() }, field)?;
            let b = self.record_get(Expression::Variable { id: rhs, ty: ty.clone() }, field)?;
            let equal = self.structural_equality(a, b)?;
            body = branch(equal, body, Expression::Constant(Constant::Bool(false)))?;
        }
        if fields.is_none() {
            body = if matches!(ty, Type::Data(..)) {
                self.adt_equality_body(&ty, lhs, rhs)?
            } else {
                Expression::Binary {
                    op: crate::parser::BinOp::Eq,
                    left: Box::new(Expression::Variable { id: lhs, ty: ty.clone() }),
                    right: Box::new(Expression::Variable { id: rhs, ty: ty.clone() }),
                }
            };
        }
        self.functions.push(Function { id, name: format!("eq_{ty:?}"), signature: signature.clone(), parameters: vec![lhs, rhs], captures: vec![], body });
        Expression::call_direct(id, &signature, vec![left, right])
    }

    fn adt_equality_body(&mut self, ty: &Type, lhs: usize, rhs: usize) -> Result<Expression, String> {
        let mut names: Vec<_> = self.constructors.keys().cloned().collect();
        names.sort();
        let mut arms = Vec::new();
        for name in names {
            let Ok(fields) = self.constructor_fields(&name, ty) else { continue; };
            let mut body = Expression::Constant(Constant::Bool(true));
            for (index, field_type) in fields.iter().enumerate().rev() {
                let project = |id| Expression::Project {
                    value: Box::new(Expression::Variable { id, ty: ty.clone() }),
                    constructor: name.clone(), index, result: field_type.clone(),
                };
                let a = project(lhs);
                let b = project(rhs);
                let equal = self.structural_equality(a, b)?;
                body = branch(equal, body, Expression::Constant(Constant::Bool(false)))?;
            }
            arms.push((name.clone(), Expression::Switch {
                value: Box::new(Expression::Variable { id: rhs, ty: ty.clone() }),
                arms: vec![(name, body)],
                fallback: Some(Box::new(Expression::Constant(Constant::Bool(false)))), result: Type::Bool,
            }));
        }
        if arms.is_empty() { return Err("unknown ADT in equality derivation".into()); }
        Ok(Expression::Switch { value: Box::new(Expression::Variable { id: lhs, ty: ty.clone() }), arms, fallback: None, result: Type::Bool })
    }

    fn derive_comparison(&mut self, ty: Type) -> Result<(FunctionId, Signature), String> {
        if matches!(ty, Type::Int | Type::Float | Type::Bool | Type::Char | Type::Unit | Type::String) {
            return Ok(self.derive_scalar_compare(ty));
        }
        let Type::Data(name, _) = &ty else { return Err("compare derivation is not implemented for this type".into()); };
        let fields = self.records.get(name).map(|record| record.0.clone());
        let signature = Signature { parameters: vec![ty.clone(), ty.clone()], result: Type::Int };
        if let Some((_, id)) = self.scalar_comparisons.iter().find(|(cached, _)| cached == &ty) {
            return Ok((*id, signature));
        }
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.scalar_comparisons.push((ty.clone(), id));
        let lhs = self.next;
        let rhs = lhs + 1;
        self.next += 2;
        let mut body = Expression::Constant(Constant::Int(0));
        for field in fields.iter().flatten().rev() {
            let a = self.record_get(Expression::Variable { id: lhs, ty: ty.clone() }, field)?;
            let b = self.record_get(Expression::Variable { id: rhs, ty: ty.clone() }, field)?;
            let (callee, callee_signature) = self.derive_comparison(a.ty())?;
            let value = Expression::call_direct(callee, &callee_signature, vec![a, b])?;
            let temporary = self.next;
            self.next += 1;
            let order = Expression::Variable { id: temporary, ty: Type::Int };
            let condition = Expression::Binary { op: crate::parser::BinOp::Eq, left: Box::new(order.clone()), right: Box::new(Expression::Constant(Constant::Int(0))) };
            body = Expression::Let { id: temporary, value: Box::new(value), body: Box::new(branch(condition, body, order)?) };
        }
        if fields.is_none() { body = self.adt_comparison_body(&ty, lhs, rhs)?; }
        self.functions.push(Function { id, name: format!("compare_{ty:?}"), signature: signature.clone(), parameters: vec![lhs, rhs], captures: vec![], body });
        Ok((id, signature))
    }

    fn adt_comparison_body(&mut self, ty: &Type, lhs: usize, rhs: usize) -> Result<Expression, String> {
        let mut variants: Vec<_> = self.constructors.keys().filter_map(|name| {
            self.constructor_fields(name, ty).ok().map(|fields| (name.clone(), fields))
        }).collect();
        variants.sort_by_key(|(name, _)| self.constructor_tags[name]);
        let mut arms = Vec::new();
        for (left_index, (name, fields)) in variants.iter().enumerate() {
            let mut right_arms = Vec::new();
            for (right_index, (other, _)) in variants.iter().enumerate() {
                let mut body = Expression::Constant(Constant::Int(if left_index < right_index { -1 } else if left_index > right_index { 1 } else { 0 }));
                if left_index == right_index {
                    for (index, field) in fields.iter().enumerate().rev() {
                        let project = |id| Expression::Project { value: Box::new(Expression::Variable { id, ty: ty.clone() }), constructor: name.clone(), index, result: field.clone() };
                        let (callee, signature) = self.derive_comparison(field.clone())?;
                        let value = Expression::call_direct(callee, &signature, vec![project(lhs), project(rhs)])?;
                        let temporary = self.next;
                        self.next += 1;
                        let order = Expression::Variable { id: temporary, ty: Type::Int };
                        let condition = Expression::Binary { op: crate::parser::BinOp::Eq, left: Box::new(order.clone()), right: Box::new(Expression::Constant(Constant::Int(0))) };
                        body = Expression::Let { id: temporary, value: Box::new(value), body: Box::new(branch(condition, body, order)?) };
                    }
                }
                right_arms.push((other.clone(), body));
            }
            arms.push((name.clone(), Expression::Switch { value: Box::new(Expression::Variable { id: rhs, ty: ty.clone() }), arms: right_arms, fallback: None, result: Type::Int }));
        }
        if arms.is_empty() { return Err("unknown ADT in comparison derivation".into()); }
        Ok(Expression::Switch { value: Box::new(Expression::Variable { id: lhs, ty: ty.clone() }), arms, fallback: None, result: Type::Int })
    }

    fn derive_scalar_compare(&mut self, ty: Type) -> (FunctionId, Signature) {
        let signature = Signature {
            parameters: vec![ty.clone(), ty.clone()],
            result: Type::Int,
        };
        if let Some((_, id)) = self
            .scalar_comparisons
            .iter()
            .find(|(cached, _)| *cached == ty)
        {
            return (*id, signature);
        }
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        let left = self.next;
        let right = left + 1;
        self.next += 2;
        let comparison = |op| Expression::Binary {
            op,
            left: Box::new(Expression::Variable {
                id: left,
                ty: ty.clone(),
            }),
            right: Box::new(Expression::Variable {
                id: right,
                ty: ty.clone(),
            }),
        };
        let body = Expression::Branch {
            condition: Box::new(comparison(crate::parser::BinOp::Lt)),
            then: Box::new(Expression::Constant(Constant::Int(-1))),
            otherwise: Box::new(Expression::Branch {
                condition: Box::new(comparison(crate::parser::BinOp::Eq)),
                then: Box::new(Expression::Constant(Constant::Int(0))),
                otherwise: Box::new(Expression::Constant(Constant::Int(1))),
            }),
        };
        let body = if ty == Type::Float {
            let nan = |id| Expression::Binary {
                op: crate::parser::BinOp::Ne,
                left: Box::new(Expression::Variable { id, ty: Type::Float }),
                right: Box::new(Expression::Variable { id, ty: Type::Float }),
            };
            Expression::Branch {
                condition: Box::new(nan(left)),
                then: Box::new(Expression::Branch {
                    condition: Box::new(nan(right)),
                    then: Box::new(Expression::Constant(Constant::Int(0))),
                    otherwise: Box::new(Expression::Constant(Constant::Int(1))),
                }),
                otherwise: Box::new(Expression::Branch {
                    condition: Box::new(nan(right)),
                    then: Box::new(Expression::Constant(Constant::Int(-1))),
                    otherwise: Box::new(body),
                }),
            }
        } else { body };
        self.functions.push(Function {
            id,
            name: format!("compare_{ty:?}"),
            signature: signature.clone(),
            parameters: vec![left, right],
            captures: vec![],
            body: if ty == Type::Unit {
                Expression::Constant(Constant::Int(0))
            } else {
                body
            },
        });
        self.scalar_comparisons.push((ty, id));
        (id, signature)
    }

    fn collect_modules(&mut self, items: &[crate::parser::Item], prefix: &str) {
        for item in items {
            if let crate::parser::Item::ModuleImpl(module) = item {
                if !matches!(module.form, crate::parser::ImplementationForm::Ordinary) { continue; }
                let path = if prefix.is_empty() {
                    module.name.clone()
                } else {
                    format!("{prefix}.{}", module.name)
                };
                for item in &module.items {
                    if let crate::parser::Item::Def(definition) = item {
                        self.definitions
                            .insert(format!("{path}.{}", definition.name), definition.clone());
                    }
                }
                self.collect_modules(&module.items, &path);
            }
        }
    }

    fn resolve_constructor(&self, name: &str) -> String {
        let mut prefix = self.active.last()
            .and_then(|(current, _, _)| current.rsplit_once('.').map(|(prefix, _)| prefix));
        while let Some(module) = prefix {
            let qualified = format!("{module}.{name}");
            if self.constructors.contains_key(&qualified) { return qualified; }
            prefix = module.rsplit_once('.').map(|(prefix, _)| prefix);
        }
        name.into()
    }

    fn resolve_definition(&self, name: &str) -> String {
        let mut prefix = self
            .active
            .last()
            .and_then(|(current, _, _)| current.rsplit_once('.').map(|(prefix, _)| prefix));
        while let Some(module) = prefix {
            let qualified = format!("{module}.{name}");
            if self.definitions.contains_key(&qualified) {
                return qualified;
            }
            prefix = module.rsplit_once('.').map(|(prefix, _)| prefix);
        }
        name.into()
    }

    fn is_value_definition(&self, name: &str) -> bool {
        matches!(self.definitions.get(name).map(|definition| &definition.kind), Some(crate::parser::DefKind::Value(_)))
    }

    fn definition_reference(
        &mut self,
        name: &str,
        span: crate::parser::Span,
    ) -> Result<Expression, String> {
        let inferred = self
            .value_types
            .get(&(span.line, span.col))
            .ok_or("missing function reference type")?;
        let ty = Type::try_from(&self.specialization.resolve(inferred))
            .map_err(|_| "definition reference needs concrete types")?;
        if self.is_value_definition(&self.resolve_definition(name)) {
            let (function, signature) = self.specialize_function(name, vec![], Some(&ty))?;
            return Expression::call_direct(function, &signature, vec![]);
        }
        let Type::Closure(parameters, result) = ty else {
            return Err("function definition requires a function type".into());
        };
        let (function, signature) = self.specialize_function(name, parameters, Some(&result))?;
        Ok(Expression::MakeClosure {
            captures: vec![],
            function,
            signature,
        })
    }

    fn specialize_function(
        &mut self,
        name: &str,
        types: Vec<Type>,
        expected_result: Option<&Type>,
    ) -> Result<(FunctionId, Signature), String> {
        let qualified = self.resolve_definition(name);
        let name = qualified.as_str();
        if name == "compare" && !self.definitions.contains_key(name) {
            if types.len() != 2 || types[0] != types[1] {
                return Err("compare requires two values of the same type".into());
            }
            return self.derive_comparison(types[0].clone());
        }
        let definition = self.definitions.get(name).ok_or_else(|| format!("unknown function {name}"))?.clone();
        let scheme = self.schemes.get(name).ok_or_else(|| format!("missing inferred signature for {name}"))?.clone();
        let key = self.specialized_name(name);
        let (params, body, scheme) = match definition.kind {
            crate::parser::DefKind::Function { params, body, .. } => (params, body, scheme),
            crate::parser::DefKind::Value(body) => {
                let scheme = match scheme.body() {
                    Ty::Fn(args, _, _) if args.is_empty() => scheme,
                    _ => scheme.as_thunk(),
                };
                (vec![], body, scheme)
            }
        };
        if params.len() != types.len() {
            return Err("function arity mismatch".into());
        }
        let outcome = self.handler_needs_outcome() && Self::scheme_carries_effects(&scheme);
        let inferred = Signature::specialize_result(&scheme, &types, expected_result.filter(|_| !outcome))?;
        let signature = if outcome {
            Signature {
                parameters: inferred.parameters.clone(),
                result: self.outcome_of(&inferred.result, &body)?,
            }
        } else {
            inferred.clone()
        };
        if let Some(function) = self.functions.iter().find(|function| {
            function.name == key
                && function.signature.parameters == types
                && function.signature.result == signature.result
        }) {
            return Ok((function.id, function.signature.clone()));
        }
        if let Some((_, id, active)) = self.active.iter().find(|(active, _, _)| active == &key) {
            if active.parameters != types || active.result != signature.result {
                return Err("recursive specialization changes argument types".into());
            }
            return Ok((*id, active.clone()));
        }
        let mut specialization = crate::types::Inference::default();
        let concrete = Ty::fn_with(
            types.iter().map(Ty::from).collect(),
            Ty::from(&inferred.result),
            specialization.fresh_row(),
        );
        specialization
            .unify(scheme.body(), &concrete)
            .map_err(|error| format!("specialization failed: {error:?}"))?;
        let saved_specialization = std::mem::replace(&mut self.specialization, specialization);
        let saved = std::mem::take(&mut self.scope);
        let mut parameters = Vec::new();
        for (param, ty) in params.iter().zip(&types) {
            let id = self.next;
            self.next += 1;
            if param.name != "_" { self.scope.insert(param.name.clone(), (id, ty.clone())); }
            parameters.push(id);
        }
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.active.push((key.clone(), id, signature.clone()));
        let lowered = self.expression(&body);
        self.active.pop();
        self.specialization = saved_specialization;
        self.scope = saved;
        let mut body = lowered?.in_tail_position();
        if is_outcome(&signature.result) && !is_outcome(&body.ty()) {
            body = self.done(body, &signature.result)?;
        }
        if body.ty() != signature.result {
            return Err(format!(
                "lowered return type differs from inferred signature for {name}"
            ));
        }

        self.functions.push(Function {
            captures: vec![],
            id,
            name: key,
            signature: signature.clone(),
            parameters,
            body,
        });
        Ok((id, signature))
    }
}

impl Expression {
    fn in_tail_position(self) -> Self {
        match self {
            Self::Switch {
                value,
                arms,
                fallback,
                result,
            } => Self::Switch {
                value,
                arms: arms
                    .into_iter()
                    .map(|(name, body)| (name, body.in_tail_position()))
                    .collect(),
                fallback: fallback.map(|body| Box::new(body.in_tail_position())),
                result,
            },
            Self::CallClosure {
                closure,
                arguments,
                result,
            } => Self::TailCallClosure {
                closure,
                arguments,
                result,
            },
            Self::CallDirect {
                function,
                arguments,
                result,
            } => Self::TailCall {
                function,
                arguments,
                result,
            },
            Self::Let { id, value, body } => Self::Let {
                id,
                value,
                body: Box::new(body.in_tail_position()),
            },
            Self::Branch {
                condition,
                then,
                otherwise,
            } => Self::Branch {
                condition,
                then: Box::new(then.in_tail_position()),
                otherwise: Box::new(otherwise.in_tail_position()),
            },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_closure_calls_preserve_target_and_tail_position() {
        let mut lowerer = Lowerer::default();
        lowerer.scope.insert(
            "f".into(),
            (7, Type::Closure(vec![Type::Int], Box::new(Type::Bool))),
        );
        let call = lowerer
            .direct_call("f", vec![Expression::Constant(Constant::Int(3))])
            .unwrap();
        assert_eq!(call.ty(), Type::Bool);
        assert!(matches!(&call, Expression::CallClosure { closure, .. }
            if matches!(closure.as_ref(), Expression::Variable { id: 7, .. })));
        assert!(matches!(
            call.in_tail_position(),
            Expression::TailCallClosure { .. }
        ));
        assert!(lowerer.direct_call("f", vec![]).is_err());
        assert!(lowerer
            .direct_call("f", vec![Expression::Constant(Constant::Bool(true))])
            .is_err());
    }

    #[test]
    fn fail_tags_carry_effect_identity() {
        let mut tags = std::collections::BTreeSet::new();
        for index in 0..32u32 {
            let tag = fail_tag(&fail_constructor(&format!("Eff{index}")));
            assert_ne!(tag, 0);
            assert_ne!(tag, 2);
            assert!(tags.insert(tag), "effect tag collided at {index}");
        }
    }

    #[test]
    fn computed_callees_lower_as_closure_calls() {
        for source in [
            "def main = (let chosen = f -> chosen) 3",
            "def main = 3 |> (let chosen = f -> chosen)",
        ] {
            let ast = crate::parser::parse_source(source, "t").unwrap();
            let mut lowerer = Lowerer::default();
            lowerer.scope.insert(
                "f".into(),
                (7, Type::Closure(vec![Type::Int], Box::new(Type::Bool))),
            );
            let call = lowerer.expression(ast.main().unwrap()).unwrap();
            let Expression::CallClosure {
                closure,
                arguments,
                result,
            } = call
            else {
                panic!("closure call")
            };
            assert!(matches!(*closure, Expression::Let { .. }));
            assert_eq!(arguments, vec![Expression::Constant(Constant::Int(3))]);
            assert_eq!(result, Type::Bool);
        }
    }

    #[test]
    fn local_binding_shadows_cached_function() {
        let mut lowerer = Lowerer::default();
        lowerer.functions.push(Function {
            captures: vec![],
            id: FunctionId(0),
            name: "f".into(),
            signature: Signature {
                parameters: vec![Type::Int],
                result: Type::Int,
            },
            parameters: vec![0],
            body: Expression::Variable {
                id: 0,
                ty: Type::Int,
            },
        });
        lowerer.scope.insert("f".into(), (1, Type::Int));
        assert!(lowerer
            .direct_call("f", vec![Expression::Constant(Constant::Int(1))])
            .is_err());
    }
}

#[test]
fn runtime_call_in_tail_position_stays_external() {
    let call = Expression::call_runtime(crate::runtime_abi::RuntimeFunction::StringLength, vec![Expression::Constant(Constant::String("a".into()))]).unwrap();
    assert!(matches!(call.in_tail_position(), Expression::CallRuntime { .. }));
}

pub fn derive_int_printer(id: FunctionId) -> Result<Function, String> { derive_scalar_printer(id, Type::Int) }

pub fn derive_scalar_printer(id: FunctionId, ty: Type) -> Result<Function, String> {
    use crate::runtime_abi::RuntimeFunction;
    let value = Expression::Variable { id: 0, ty: ty.clone() };
    let text = |s: &str| Expression::Constant(Constant::String(s.into()));
    let (runtime, argument) = match ty {
        Type::Int => (RuntimeFunction::PrintInt, value),
        Type::String => (RuntimeFunction::PrintString, value),
        Type::Char => (RuntimeFunction::PrintChar, value),
        Type::Float => (RuntimeFunction::PrintString, Expression::call_runtime(RuntimeFunction::CanonicalFloat, vec![value])?),
        Type::Bool => (RuntimeFunction::PrintString, branch(value, text("true"), text("false"))?),
        Type::Unit => return Ok(Function {
            id, name: format!("print_{ty:?}"),
            signature: Signature { parameters: vec![ty], result: Type::Unit },
            parameters: vec![0], captures: vec![],
            body: Expression::Constant(Constant::Unit),
        }),
        _ => return Err("scalar printer is not implemented".into()),
    };
    Ok(Function {
        id,
        name: format!("print_{ty:?}"),
        signature: Signature { parameters: vec![ty.clone()], result: Type::Unit },
        parameters: vec![0],
        captures: vec![],
        body: Expression::call_runtime(runtime, vec![argument])?,
    })
}

pub fn derive_printers(id: FunctionId, ty: Type, checked: &crate::Checked) -> Result<Vec<Function>, String> {
    let checked = &checked.for_codegen();
    let mut context = DisplayContext { checked, next: id.0 + 1, cache: Vec::new(), functions: Vec::new() };
    let printer = derive_printer(id, ty, &mut context)?;
    context.functions.insert(0, printer);
    Ok(context.functions)
}

fn derive_printer(id: FunctionId, ty: Type, context: &mut DisplayContext<'_>) -> Result<Function, String> {
    if !matches!(ty, Type::Data(..)) {
        return derive_scalar_printer(id, ty);
    }
    let value = context.format(Expression::Variable { id: 0, ty: ty.clone() })?;
    Ok(Function { id, name: format!("print_{ty:?}"), signature: Signature { parameters: vec![ty], result: Type::Unit }, parameters: vec![0], captures: vec![], body: Expression::call_runtime(crate::runtime_abi::RuntimeFunction::PrintString, vec![value])? })
}

struct DisplayContext<'a> {
    checked: &'a crate::Checked,
    next: usize,
    cache: Vec<(Type, FunctionId)>,
    functions: Vec<Function>,
}

impl DisplayContext<'_> {
    fn format(&mut self, value: Expression) -> Result<Expression, String> {
        let ty = value.ty();
        if !matches!(ty, Type::Data(..)) { return display_value(value, self); }
        let signature = Signature { parameters: vec![ty.clone()], result: Type::String };
        if let Some((_, id)) = self.cache.iter().find(|(cached, _)| cached == &ty) {
            return Expression::call_direct(*id, &signature, vec![value]);
        }
        let id = FunctionId(self.next);
        self.next += 1;
        self.cache.push((ty.clone(), id));
        let body = display_value(Expression::Variable { id: 0, ty: ty.clone() }, self)?;
        self.functions.push(Function { id, name: format!("format_{ty:?}"), signature: signature.clone(), parameters: vec![0], captures: vec![], body });
        Expression::call_direct(id, &signature, vec![value])
    }
}

fn display_list(value: Expression, element: Type, context: &mut DisplayContext<'_>) -> Result<Expression, String> {
    let id = FunctionId(context.next);
    context.next += 1;
    let signature = Signature { parameters: vec![value.ty()], result: Type::String };
    let parameter = Expression::Variable { id: 0, ty: value.ty() };
    let body = display_list_items(parameter, element, id, &signature, context)?;
    context.functions.push(Function { id, name: format!("format_list_items_{id:?}"), signature: signature.clone(), parameters: vec![0], captures: vec![], body });
    let items = Expression::call_direct(id, &signature, vec![value])?;
    concat_display(display_text("["), concat_display(items, display_text("]"))?)
}

fn display_text(text: &str) -> Expression {
    Expression::Constant(Constant::String(text.into()))
}

fn concat_display(left: Expression, right: Expression) -> Result<Expression, String> {
    Expression::call_runtime(crate::runtime_abi::RuntimeFunction::StringConcat, vec![left, right])
}

fn display_list_items(value: Expression, element: Type, id: FunctionId, signature: &Signature, context: &mut DisplayContext<'_>) -> Result<Expression, String> {
    let head = context.format(Expression::Project { value: Box::new(value.clone()), constructor: "Cons".into(), index: 0, result: element })?;
    let tail = Expression::Project { value: Box::new(value.clone()), constructor: "Cons".into(), index: 1, result: value.ty() };
    let remaining = Expression::call_direct(id, signature, vec![tail.clone()])?;
    let suffix = Expression::Switch { value: Box::new(tail), arms: vec![
        ("Nil".into(), display_text("")),
        ("Cons".into(), concat_display(display_text(", "), remaining)?),
    ], fallback: None, result: Type::String };
    Ok(Expression::Switch { value: Box::new(value), arms: vec![
        ("Nil".into(), display_text("")),
        ("Cons".into(), concat_display(head, suffix)?),
    ], fallback: None, result: Type::String })
}

fn display_value(value: Expression, context: &mut DisplayContext<'_>) -> Result<Expression, String> {
    let checked = context.checked;
    use crate::runtime_abi::RuntimeFunction;
    let text = |s: &str| Expression::Constant(Constant::String(s.into()));
    let runtime = match value.ty() {
        Type::String => RuntimeFunction::QuoteString,
        Type::Int => RuntimeFunction::IntToString,
        Type::Float => RuntimeFunction::CanonicalFloat,
        Type::Char => RuntimeFunction::QuoteChar,
        Type::Bool => return branch(value, text("true"), text("false")),
        Type::Unit => return Ok(text("()")),
        Type::Data(name, parameters) if name == "List" && parameters.len() == 1 => {
            return display_list(value, parameters[0].clone(), context);
        }
        ty @ Type::Data(..) if matches!(&ty, Type::Data(name, _) if checked.records.contains_key(name)) => {
            let Type::Data(name, _) = &ty else { unreachable!() };
            let fields = &checked.records[name].0;
            let lowerer = Lowerer { records: checked.records.clone(), ..Lowerer::default() };
            let mut result = text("{ ");
            for (index, name) in fields.iter().enumerate() {
                let label = if index == 0 { format!("{name} = ") } else { format!("; {name} = ") };
                result = Expression::call_runtime(RuntimeFunction::StringConcat, vec![result, text(&label)])?;
                let field = lowerer.record_get(value.clone(), name)?;
                let content = context.format(field)?;
                result = Expression::call_runtime(RuntimeFunction::StringConcat, vec![result, content])?;
            }
            return Expression::call_runtime(RuntimeFunction::StringConcat, vec![result, text(" }")]);
        }
        ty @ Type::Data(..) => {
            let lowerer = Lowerer { constructors: checked.constructors.clone(), ..Lowerer::default() };
            let mut names: Vec<_> = checked.constructors.keys().cloned().collect();
            names.sort();
            let mut arms = Vec::new();
            for name in names {
                let Ok(fields) = lowerer.constructor_fields(&name, &ty) else { continue; };
                let mut content = text(&name);
                if let Some(names) = checked.named_constructors.get(&name) {
                    content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, text(" { ")])?;
                    for (index, (field, label)) in fields.iter().zip(names).enumerate() {
                        let label = if index == 0 { format!("{label} = ") } else { format!("; {label} = ") };
                        content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, text(&label)])?;
                        let projected = Expression::Project { value: Box::new(value.clone()), constructor: name.clone(), index, result: field.clone() };
                        let rendered = context.format(projected)?;
                        content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, rendered])?;
                    }
                    content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, text(" }")])?;
                } else if !fields.is_empty() {
                    content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, text("(")])?;
                    for (index, field) in fields.iter().enumerate() {
                        if index > 0 { content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, text(", ")])?; }
                        let projected = Expression::Project { value: Box::new(value.clone()), constructor: name.clone(), index, result: field.clone() };
                        let rendered = context.format(projected)?;
                        content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, rendered])?;
                    }
                    content = Expression::call_runtime(RuntimeFunction::StringConcat, vec![content, text(")")])?;
                }
                arms.push((name, content));
            }
            if arms.is_empty() { return Err("unknown display type".into()); }
            return Ok(Expression::Switch { value: Box::new(value), arms, fallback: None, result: Type::String });
        }
        _ => return Err("aggregate display is not implemented".into()),
    };
    Expression::call_runtime(runtime, vec![value])
}
