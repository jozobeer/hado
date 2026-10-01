use crate::types::Ty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Int,
    Float,
    Bool,
    Char,
    String,
    Bytes,
    Void,
    Data(String, Vec<Type>),
    Abstract(String, Vec<Type>),
    Closure(Vec<Type>, Box<Type>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnresolvedType {
    Variable(u32),
    Invalid,
}

impl TryFrom<&Ty> for Type {
    type Error = UnresolvedType;

    fn try_from(ty: &Ty) -> Result<Self, Self::Error> {
        Ok(match ty {
            Ty::Int => Self::Int,
            Ty::Float => Self::Float,
            Ty::Bool => Self::Bool,
            Ty::Char => Self::Char,
            Ty::String => Self::String,
            Ty::Bytes => Self::Bytes,
            Ty::Void => Self::Void,
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
pub const ARRAY: &str = "Array";
const DONE: &str = "$Done";
const FAIL: &str = "$Fail";
const YIELD: &str = "$Yield";

pub(crate) fn outcome_tag(constructor: &str) -> Option<u32> {
    if constructor == DONE {
        return Some(0);
    }
    if let Some(tag) = constructor.strip_prefix("$Fail#") {
        return tag.parse().ok();
    }
    constructor.strip_prefix("$Yield#")?.parse().ok()
}

fn is_outcome(ty: &Type) -> bool {
    matches!(ty, Type::Data(name, _) if name == OUTCOME)
}

// 効果を持つ関数値は mono では結果型に `$Outcome` を載せる。callee を
// 単一化するときは source 側の型に戻す。ADT の型引数・入れ子の閉包の結果・
// 閉包の引数に載っている場合も、内側を戻す。本体を実際の型で下ろすと閉包の
// 引数にも `$Outcome` が現れるので、引数の中も戻す。
fn logical_argument(ty: &Type) -> Type {
    match ty {
        Type::Closure(parameters, result) => {
            let result = logical_argument(result);
            Type::Closure(
                parameters.iter().map(logical_argument).collect(),
                Box::new(strip_outcome(result)),
            )
        }
        Type::Data(name, arguments) => Type::Data(
            name.clone(),
            arguments.iter().map(logical_argument).collect(),
        ),
        Type::Abstract(name, arguments) => Type::Abstract(
            name.clone(),
            arguments.iter().map(logical_argument).collect(),
        ),
        _ => ty.clone(),
    }
}

fn strip_outcome(ty: Type) -> Type {
    match ty {
        Type::Data(name, arguments) if name == OUTCOME && !arguments.is_empty() => {
            arguments.into_iter().next().unwrap()
        }
        ty => ty,
    }
}

// 呼び出しの形を callee と引数の並びに解く。`|>` は左辺を先頭に足した `Apply`
// と同じ形にする（`lift_again_arguments` の書き換えと同じ規則）。効果の判定は
// `Apply` と同じく callee を見るので、`|>` の target も同じ経路に載せる。
fn call_parts(
    node: &crate::parser::Node,
) -> Option<(&crate::parser::Node, Vec<&crate::parser::Node>)> {
    use crate::parser::NodeKind;
    match &node.kind {
        NodeKind::Apply(callee, arguments) | NodeKind::Call(callee, arguments) => {
            Some((callee.as_ref(), arguments.iter().collect()))
        }
        NodeKind::Pipe(value, target) => {
            let (callee, mut arguments) = match &target.kind {
                NodeKind::Apply(callee, arguments) => (callee.as_ref(), arguments.iter().collect()),
                _ => (target.as_ref(), Vec::new()),
            };
            arguments.insert(0, value.as_ref());
            Some((callee, arguments))
        }
        _ => None,
    }
}

// `Array.of_list` のような callee は、scheme の単一化で入れ子の outcome を
// 落とす。実際の型での単一化が通るなら、結果型はそちらから取る（下の
// `specialize_function`）。実際の型で単一化できない形は論理型のままにする。

// scheme が関数型を書いた引数は、本体がその outcome をほどいて伝播させる。
// 単一化には、その結果型だけ `$Outcome` を外した成功型を使う。閉包の引数
// （ドメイン）とデータの型引数は引数の実際の型のまま残す。本体はそれらを
// 値として持ち回り、呼ぶときに実際の型で outcome をほどく。全体を論理型に
// すると（`logical_argument`）、引数から来た値と本体が組み立てる値が食い違う。
fn mixed_argument(parameter: &Ty, actual: &Type) -> Type {
    match (parameter, actual) {
        (Ty::Fn(..), Type::Closure(parameters, result)) => Type::Closure(
            parameters.clone(),
            Box::new(strip_outcome((**result).clone())),
        ),
        (Ty::Adt(name, args) | Ty::Abstract(name, args), Type::Data(actual_name, actual_args))
            if name == actual_name =>
        {
            Type::Data(
                actual_name.clone(),
                args.iter()
                    .zip(actual_args)
                    .map(|(parameter, actual)| mixed_argument(parameter, actual))
                    .collect(),
            )
        }
        (
            Ty::Adt(name, args) | Ty::Abstract(name, args),
            Type::Abstract(actual_name, actual_args),
        ) if name == actual_name => Type::Abstract(
            actual_name.clone(),
            args.iter()
                .zip(actual_args)
                .map(|(parameter, actual)| mixed_argument(parameter, actual))
                .collect(),
        ),
        _ => actual.clone(),
    }
}

// scheme の単一化に渡す期待する結果型。閉包は、引数（ドメイン）を実際の型の
// まま、結果だけ `$Outcome` を外す。呼び出しの効果は scheme の効果行が表すので、
// 結果に載せたまま単一化すると `Fn(A => Int)` と `$Outcome(Int)` が食い違う。
fn scheme_expected(ty: &Type) -> Type {
    match ty {
        Type::Closure(parameters, result) => Type::Closure(
            parameters.clone(),
            Box::new(strip_outcome((**result).clone())),
        ),
        _ => ty.clone(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    String(String),
    Void,
}

impl Constant {
    pub fn ty(&self) -> Type {
        match self {
            Self::Int(_) => Type::Int,
            Self::Float(_) => Type::Float,
            Self::Bool(_) => Type::Bool,
            Self::Char(_) => Type::Char,
            Self::String(_) => Type::String,
            Self::Void => Type::Void,
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
        NodeKind::Void => Constant::Void,
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
    CallRuntime {
        function: crate::runtime_abi::RuntimeFunction,
        arguments: Vec<Expression>,
        result: Type,
    },
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
    pub fn call_array(
        function: crate::runtime_abi::RuntimeFunction,
        arguments: Vec<Expression>,
        allocation: Option<Type>,
    ) -> Result<Self, String> {
        use crate::runtime_abi::RuntimeFunction;
        let array = if function == RuntimeFunction::ArrayAlloc {
            allocation.ok_or("array allocation needs its concrete result type")?
        } else {
            arguments
                .first()
                .ok_or("array intrinsic needs an array")?
                .ty()
        };
        let Type::Data(name, elements) = &array else {
            return Err("expected array type".into());
        };
        if name != ARRAY || elements.len() != 1 {
            return Err("expected one array element type".into());
        }
        let element = elements[0].clone();
        let (parameters, result) = match function {
            RuntimeFunction::ArrayLength => (vec![array], Type::Int),
            RuntimeFunction::ArrayGet => (vec![array, Type::Int], element),
            RuntimeFunction::ArrayAlloc => (vec![Type::Int], array),
            RuntimeFunction::ArrayFillUnchecked => (vec![array.clone(), Type::Int, element], array),
            _ => return Err("expected polymorphic array intrinsic".into()),
        };
        if arguments.iter().map(Expression::ty).collect::<Vec<_>>() != parameters {
            return Err("array intrinsic argument type mismatch".into());
        }
        Ok(Self::CallRuntime {
            function,
            arguments,
            result,
        })
    }

    pub fn call_runtime(
        function: crate::runtime_abi::RuntimeFunction,
        arguments: Vec<Expression>,
    ) -> Result<Self, String> {
        let signature = function
            .value_signature()
            .ok_or("runtime function has no value-level signature")?;
        if arguments.iter().map(Expression::ty).collect::<Vec<_>>() != signature.parameters {
            return Err("runtime call argument type mismatch".into());
        }
        Ok(Self::CallRuntime {
            function,
            arguments,
            result: signature.result,
        })
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

#[derive(Clone)]
struct MatrixRow {
    patterns: Vec<crate::parser::Pattern>,
    body: crate::parser::Node,
    bindings: Vec<(String, usize, Type)>,
}

enum ColumnKind {
    Any,
    Bool,
    Constructor,
    Literal,
    Record,
}

// 列の先頭パターンがどの種類を要求するかを、行を走査しながら集める。
#[derive(Default)]
struct ColumnHeads {
    record: bool,
    constructor: bool,
    literal: bool,
}

impl ColumnHeads {
    fn kind(&self, ty: &Type) -> ColumnKind {
        if self.record {
            ColumnKind::Record
        } else if *ty == Type::Bool && self.constructor {
            ColumnKind::Bool
        } else if self.constructor {
            ColumnKind::Constructor
        } else if self.literal {
            ColumnKind::Literal
        } else {
            ColumnKind::Any
        }
    }
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
    checking_once: std::cell::RefCell<std::collections::BTreeSet<String>>,
    // 継続を張り替える binder は、本体の lower が進んで yield の tag が
    // 登録された後に組む。相互再帰の途中で組むと腕が欠ける。
    pending_binds: Vec<(
        FunctionId,
        Signature,
        std::collections::HashMap<String, Signature>,
    )>,
    active: Vec<(String, FunctionId, Signature)>,
    schemes: std::collections::HashMap<String, crate::types::Scheme>,
    value_types: std::collections::HashMap<(usize, usize), Ty>,
    specialization: crate::types::Inference,
    composite_types: std::collections::HashMap<(usize, usize), Ty>,
    /// `with` の位置ごとの節点の型（`crate::WithTypes`）。同じ節点が答え型の
    /// 違う複数の `with` に現れると span 単位の表は最後のもので上書きされる
    /// ので、腕を lower する間だけその `with` の型に差し替える。
    with_types: std::collections::HashMap<(usize, usize), crate::WithTypes>,
    operations: std::collections::HashMap<String, crate::parser::OpKind>,
    default_operations: std::collections::HashMap<String, String>,
    handlers: std::collections::HashMap<String, crate::parser::HandlerDecl>,
    handler_stack: Vec<crate::parser::HandlerDecl>,
    /// `handler_stack` と同じ長さ。各 `with` の値引数のうち `_` 以外を、いま
    /// lower しているフレームで参照できる id と型で持つ。`with` の本体では
    /// `with` の位置の変数、単相化した関数の中では capture の id である。
    /// `self.scope` には入れない。値引数の名前は腕からだけ見えるので、
    /// 本体の利用者の束縛を覆ってはならない（§38）。
    frame_value_args: Vec<Vec<(String, (usize, Type))>>,
    handler_answers: Vec<Option<Type>>,
    again_answers: Vec<Type>,
    context_answer: Option<Type>,
    resume_binding: Option<(usize, Type)>,
    rewriting_return: bool,
    /// 腕の本体を単相化している間、その handler が stack の何番目かを指す。
    /// 腕は自分の handler の外で走るので、handler は自分の腕を捕まえない。
    /// 腕の中でさらに別の handler の腕を lower するので、入れ子の分だけ積む。
    arm_handlers: Vec<usize>,
    /// `$Fail#` / `$Yield#` のタグは操作名で引く。効果名だと、同じ効果の
    /// 二番目以降の again / abort が最初の腕で処理される。
    fail_ids: std::collections::HashMap<String, u32>,
    yield_ids: std::collections::HashMap<String, u32>,
    operation_types: std::collections::HashMap<String, Signature>,
    next_fail_tag: u32,
    next_function: usize,
    scope: std::collections::BTreeMap<String, (usize, Type)>,
    next: usize,
}

/// handler の値引数をこのフレームへ束ねた結果。
#[derive(Default)]
struct HandlerValueBindings {
    /// `with` の位置で 1 回だけ評価するための `Let` の並び。
    lets: Vec<(usize, Expression)>,
    /// `_` 以外の値引数を腕から見えるようにするための表。
    named: Vec<(String, (usize, Type))>,
}

/// `with` ごとの型を差し替えるとき、元の表の値を戻せるよう控える。
struct InstalledTypes {
    values: Vec<((usize, usize), Option<Ty>)>,
    composites: Vec<((usize, usize), Option<Ty>)>,
}

pub fn lower_expression(node: &crate::parser::Node) -> Result<Expression, String> {
    Lowerer::default().expression(node)
}

impl Lowerer {
    fn contains_again(&self, node: &crate::parser::Node) -> bool {
        self.is_again_node(node)
            || (matches!(node.kind, crate::parser::NodeKind::Resume)
                && matches!(&self.resume_binding, Some((_, Type::Closure(_, result))) if is_outcome(result)))
            || crate::check::children(node)
                .iter()
                .any(|child| self.contains_again(child))
    }

    // handler の無い `once` 操作は既定実装を呼ぶので、既定実装が効果を運ぶなら
    // その呼び出しも outcome を返す（prelude の `cwd` などが `Error` を運ぶ）。
    fn default_operation_carries_effects(&self, name: &str) -> bool {
        let Some(definition) = self.default_operations.get(name) else {
            return false;
        };
        self.schemes
            .get(&self.resolve_definition(definition))
            .is_some_and(Self::scheme_carries_effects)
    }

    // callee が閉包を返す式（分岐や record field など）でも、結果が outcome の
    // 閉包を見つけたら同じく継続に載せる。
    fn callee_is_outcome_closure(&self, callee: &crate::parser::Node) -> bool {
        use crate::parser::NodeKind;
        match &callee.kind {
            NodeKind::Name(name) => self.scope.get(name).is_some_and(
                |(_, ty)| matches!(ty, Type::Closure(_, result) if is_outcome(result)),
            ),
            NodeKind::BoolBranch { then, els, .. } => {
                self.callee_is_outcome_closure(then) || self.callee_is_outcome_closure(els)
            }
            NodeKind::CondBranch { arms, fallback } => {
                arms.iter()
                    .any(|(_, body)| self.callee_is_outcome_closure(body))
                    || fallback
                        .as_ref()
                        .is_some_and(|fallback| self.callee_is_outcome_closure(fallback))
            }
            NodeKind::PatternBranch { arms, fallback, .. } => {
                arms.iter()
                    .any(|(_, body)| self.callee_is_outcome_closure(body))
                    || fallback
                        .as_ref()
                        .is_some_and(|fallback| self.callee_is_outcome_closure(fallback))
            }
            NodeKind::Field(..) => self
                .value_types
                .get(&(callee.span.line, callee.span.col))
                .is_some_and(|ty| {
                    self.outcome_closure_of(&self.specialization.resolve(ty))
                        .is_some()
                }),
            _ => false,
        }
    }

    // handler の `once` の腕が効果を運ぶか。腕が同じ操作を呼んでも止まるように、
    // 見ている操作名を控える。
    fn once_arm_carries_effects(&self, op: &str) -> bool {
        if self.checking_once.borrow().contains(op) {
            return false;
        }
        let Some(handler) = self.handler_for(op) else {
            return false;
        };
        let arms: Vec<crate::parser::Node> = handler
            .arms
            .iter()
            .filter_map(|arm| match arm {
                crate::parser::HandlerArm::Operation { name, body, .. } if name == op => {
                    Some(body.clone())
                }
                _ => None,
            })
            .collect();
        if arms.is_empty() {
            return false;
        }
        self.checking_once.borrow_mut().insert(op.to_owned());
        let carried = arms
            .iter()
            .any(|body| self.contains_again(body) || self.contains_handled_call(body));
        self.checking_once.borrow_mut().remove(op);
        carried
    }

    fn contains_handled_call(&self, node: &crate::parser::Node) -> bool {
        if self.division_needs_raise_handler(node) {
            return true;
        }
        if let crate::parser::NodeKind::Name(name) = &node.kind {
            if self.name_carries_effects(name) {
                return true;
            }
        }
        if self.is_abort_node(node) && self.handler_for_node(node).is_some() {
            return true;
        }
        // `|>` の target も `Apply` と同じ callee として見る。書いた形が違うだけで
        // 効果の有無が変わらないようにする。
        if let Some((callee, arguments)) = call_parts(node) {
            if self.call_carries_effects(callee, &arguments) {
                return true;
            }
        }
        crate::check::children(node)
            .iter()
            .any(|child| self.contains_handled_call(child))
    }

    fn division_needs_raise_handler(&self, node: &crate::parser::Node) -> bool {
        matches!(
            &node.kind,
            crate::parser::NodeKind::Bin(
                crate::parser::BinOp::Div | crate::parser::BinOp::Mod,
                _,
                _
            )
        ) && self.handler_for("raise").is_some()
    }

    fn name_carries_effects(&self, name: &str) -> bool {
        // 処理中の once 操作でも、handler の腕が効果を運ぶなら outcome になる。
        if !self.scope.contains_key(name)
            && self.stack_needs_outcome()
            && self.once_arm_carries_effects(name)
        {
            return true;
        }
        if !self.scope.contains_key(name)
            && self.handler_for(name).is_none()
            && self.stack_needs_outcome()
        {
            if self.default_operation_carries_effects(name) {
                return true;
            }
            // 効果を運ぶトップレベルの値（`def current = cwd`）も引数の位置で
            // outcome になる。
            if self
                .schemes
                .get(&self.resolve_definition(name))
                .is_some_and(Self::scheme_carries_effects)
            {
                return true;
            }
        }
        false
    }

    fn call_carries_effects(
        &self,
        callee: &crate::parser::Node,
        arguments: &[&crate::parser::Node],
    ) -> bool {
        if self.stack_needs_outcome() && self.callee_is_outcome_closure(callee) {
            return true;
        }
        let name = match &callee.kind {
            crate::parser::NodeKind::Name(name) if !self.scope.contains_key(name) => {
                Some(name.clone())
            }
            crate::parser::NodeKind::Field(module, member) => match &module.kind {
                crate::parser::NodeKind::Name(module) => Some(format!("{module}.{}", member.name)),
                _ => None,
            },
            _ => None,
        };
        if self.called_scheme_carries_effects(name.as_deref()) {
            return true;
        }

        // 効果を持つ閉包を引数に取る定義呼び出しは、callee の本体がその
        // 閉包を呼ぶので、outcome を呼び出し側の継続に載せる。
        if self.stack_needs_outcome()
            && name
                .as_deref()
                .is_some_and(|name| self.is_effectful_callee(name))
            && arguments
                .iter()
                .any(|argument| self.callee_is_outcome_closure(argument))
        {
            return true;
        }
        false
    }

    fn called_scheme_carries_effects(&self, name: Option<&str>) -> bool {
        self.stack_needs_outcome()
            && name.is_some_and(|name| {
                self.schemes
                    .get(&self.resolve_definition(name))
                    .is_some_and(Self::scheme_carries_effects)
                    || self.default_operation_carries_effects(name)
            })
    }

    fn is_effectful_callee(&self, name: &str) -> bool {
        !self.scope.contains_key(name)
            && !self.operations.contains_key(name)
            && !self
                .constructors
                .contains_key(&self.resolve_definition(name))
    }

    fn lift_again_arguments(&mut self, node: &crate::parser::Node) -> Option<crate::parser::Node> {
        use crate::parser::{BinOp, NodeKind::*};
        let mut result = node.clone();
        if let Pipe(value, target) = &node.kind {
            let (callee, mut args) = match &target.kind {
                Apply(callee, args) => (callee.clone(), args.clone()),
                _ => (target.clone(), Vec::new()),
            };
            args.insert(0, *value.clone());
            result.kind = Apply(callee, args);
        }
        let children: Vec<&mut crate::parser::Node> = match &mut result.kind {
            Apply(_, args) | Call(_, args) => args.iter_mut().collect(),
            Neg(value) | Not(value) | Field(value, _) => vec![value],
            Bin(BinOp::And | BinOp::Or, left, _) => vec![left],
            Bin(_, left, right) => vec![left, right],
            List(items, tail) => items
                .iter_mut()
                .chain(tail.iter_mut().map(Box::as_mut))
                .collect(),
            Record(fields) | NamedConstructor(_, fields) => {
                fields.iter_mut().map(|(_, value)| value).collect()
            }
            BoolBranch { cond, .. } => vec![cond],
            CondBranch { arms, .. } => arms.iter_mut().map(|(condition, _)| condition).collect(),
            PatternBranch { scrutinees, .. } => scrutinees.iter_mut().collect(),
            _ => Vec::new(),
        };
        if !children
            .iter()
            .any(|child| self.contains_again(child) || self.contains_handled_call(child))
        {
            return None;
        }
        let mut bindings = Vec::new();
        for child in children {
            let name = format!("$anf{}", self.next);
            self.next += 1;
            let value = child.clone();
            child.kind = Name(name.clone());
            bindings.push((
                crate::parser::Pattern {
                    kind: crate::parser::PatternKind::Name(name),
                    span: child.span,
                },
                value,
            ));
        }
        for (pat, value) in bindings.into_iter().rev() {
            result = crate::parser::Node {
                span: node.span,
                kind: Let {
                    pat,
                    value: Box::new(value),
                    body: Box::new(result),
                },
            };
        }
        Some(result)
    }

    fn expression(&mut self, node: &crate::parser::Node) -> Result<Expression, String> {
        use crate::parser::NodeKind;
        if let Some(normalized) = self.lift_again_arguments(node) {
            return self.expression(&normalized);
        }
        match &node.kind {
            NodeKind::Pipe(value, target) => self.pipe_expression(node, value, target),
            NodeKind::Apply(callee, args) | NodeKind::Call(callee, args) => {
                let arguments = args
                    .iter()
                    .map(|arg| self.expression(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                self.application(node, callee, arguments)
            }
            NodeKind::Field(record, field) => self.field_expression(node, record, field),
            NodeKind::NamedConstructor(name, fields) => {
                self.named_constructor(node, &name.name, fields)
            }
            NodeKind::Record(fields) => self.record_expression(node, fields),
            NodeKind::List(items, tail) => self.list_expression(node, items, tail),
            NodeKind::Fn { params, body } => self.lambda(node, params, body),
            NodeKind::Name(name) => self.name_expression(node, name),
            NodeKind::LetQ { pat, value, body } => self.propagation(Some(pat), value, body),
            NodeKind::DoQ { value, body } => self.do_expression(node, value, body),
            NodeKind::Let { pat, value, body } => self.let_expression(pat, value, body),
            NodeKind::PatternBranch {
                scrutinees,
                arms,
                fallback,
            } => self.pattern_branch(scrutinees, arms, fallback),
            NodeKind::CondBranch { arms, fallback } => self.cond_branch(arms, fallback),
            NodeKind::BoolBranch { cond, then, els } => self.bool_branch(cond, then, els),
            NodeKind::With {
                handler,
                args,
                body,
            } => self.with_handler(node, &handler.name, args, body),
            NodeKind::Resume => self.resume_variable(),
            NodeKind::Bin(op, left, right) => self.binary(*op, left, right),
            NodeKind::Neg(value) => self.unary_expression(UnaryOp::Negate, value),
            NodeKind::Not(value) => self.unary_expression(UnaryOp::Not, value),
            _ => self.literal_expression(node),
        }
    }

    fn pipe_expression(
        &mut self,
        node: &crate::parser::Node,
        value: &crate::parser::Node,
        target: &crate::parser::Node,
    ) -> Result<Expression, String> {
        use crate::parser::NodeKind;
        let mut arguments = vec![self.expression(value)?];
        let callee = if let NodeKind::Apply(callee, args) = &target.kind {
            for arg in args {
                arguments.push(self.expression(arg)?);
            }
            callee.as_ref()
        } else {
            target
        };
        self.application(node, callee, arguments)
    }

    fn field_expression(
        &mut self,
        node: &crate::parser::Node,
        record: &crate::parser::Node,
        field: &crate::parser::FieldName,
    ) -> Result<Expression, String> {
        use crate::parser::NodeKind;
        if let NodeKind::Name(module) = &record.kind {
            let qualified = self.resolve_definition(&format!("{module}.{}", field.name));
            if self.schemes.contains_key(&qualified) {
                return self.definition_reference(&qualified, node.span);
            }
        }
        let record = self.expression(record)?;
        self.record_get(record, &field.name)
    }

    fn record_expression(
        &mut self,
        node: &crate::parser::Node,
        fields: &[(crate::parser::DeclaredName, crate::parser::Node)],
    ) -> Result<Expression, String> {
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
        Ok(Expression::Record { fields, result })
    }

    fn list_expression(
        &mut self,
        node: &crate::parser::Node,
        items: &[crate::parser::Node],
        tail: &Option<Box<crate::parser::Node>>,
    ) -> Result<Expression, String> {
        let (list_ty, values) = self.list_elements(node, items)?;
        self.cons_list(tail, list_ty, values)
    }

    fn list_elements(
        &mut self,
        node: &crate::parser::Node,
        items: &[crate::parser::Node],
    ) -> Result<(Type, Vec<Expression>), String> {
        let ty = self
            .composite_types
            .get(&(node.span.line, node.span.col))
            .ok_or("missing list type")?
            .clone();
        // 要素が効果を持つ関数なら、mono の要素型は結果が outcome の閉包。
        let declared = match &ty {
            Ty::Adt(_, arguments) if arguments.len() == 1 => self.outcome_closure_of(&arguments[0]),
            _ => None,
        };
        let resolved = Type::try_from(&self.specialization.resolve(&ty))
            .map_err(|_| "list type is not concrete")?;
        let values = items
            .iter()
            .map(|item| self.expression(item))
            .collect::<Result<Vec<_>, _>>()?;
        let derived = Self::derived_element_type(&resolved, &values);
        let list_ty = match (&declared, derived, &resolved) {
            (Some(element), _, Type::Data(name, _)) => {
                Type::Data(name.clone(), vec![element.clone()])
            }
            (None, Some(element), Type::Data(name, _)) => Type::Data(name.clone(), vec![element]),
            _ => resolved,
        };
        // 宣言側の要素型が outcome の閉包なら、効果を運ばない要素も
        // 呼び出し側が outcome として解釈できる型に揃える。
        let values = match &declared {
            Some(element) => values
                .into_iter()
                .map(|value| self.as_outcome_closure(value, element))
                .collect::<Result<Vec<_>, _>>()?,
            None => values,
        };
        Ok((list_ty, values))
    }

    fn derived_element_type(resolved: &Type, values: &[Expression]) -> Option<Type> {
        // 宣言側が効果行を落としている（効果多相の callee を通った）場合は、
        // lower した値の型から要素型を決める。
        let Type::Data(_, arguments) = resolved else {
            return None;
        };
        let [declared] = arguments.as_slice() else {
            return None;
        };
        values
            .first()
            .map(Expression::ty)
            .filter(|first| values.iter().all(|value| &value.ty() == first))
            .filter(|first| logical_argument(first) == *declared)
    }

    fn cons_list(
        &mut self,
        tail: &Option<Box<crate::parser::Node>>,
        list_ty: Type,
        values: Vec<Expression>,
    ) -> Result<Expression, String> {
        let mut list = match tail {
            Some(tail) => self.expression(tail)?,
            None => Expression::Construct {
                constructor: self.resolve_constructor("Nil"),
                fields: vec![],
                result: list_ty.clone(),
            },
        };
        if tail.is_some() && list.ty() != list_ty {
            return Err("list element type mismatch".into());
        }
        for value in values.into_iter().rev() {
            if Self::element_type_mismatch(&list_ty, &value) {
                return Err("list element type mismatch".into());
            }
            list = Expression::Construct {
                constructor: self.resolve_constructor("Cons"),
                fields: vec![value, list],
                result: list_ty.clone(),
            };
        }
        Ok(list)
    }

    fn element_type_mismatch(list_ty: &Type, value: &Expression) -> bool {
        // 効果多相の callee を通ると、checker が handler の答えの制約を
        // 落とすことがある。型の食い違った `Cons` を組むと実行時に壊れる
        // ので、ここで止める。
        let Type::Data(_, arguments) = list_ty else {
            return false;
        };
        arguments
            .first()
            .is_some_and(|element| element != &value.ty())
    }

    // 宣言側の要素型が outcome の閉包のとき、効果を運ばない要素の閉包は
    // 結果を `done` で包む閉包に直す。呼び出し側は要素を outcome として
    // 解釈する（`g 4` の結果が handler の答えになる）ので、値の側でも
    // 同じ型に揃える。直せない不一致はそのまま返し、`cons_list` が止める。
    fn as_outcome_closure(
        &mut self,
        value: Expression,
        element: &Type,
    ) -> Result<Expression, String> {
        let Type::Closure(parameters, outcome) = element else {
            return Ok(value);
        };
        if !is_outcome(outcome) {
            return Ok(value);
        }
        let value_ty = value.ty();
        let Type::Closure(value_parameters, value_result) = &value_ty else {
            return Ok(value);
        };
        if value_parameters != parameters || **value_result == **outcome {
            return Ok(value);
        }
        let (success, _, _) = outcome_parts(outcome)?;
        if **value_result != success {
            return Ok(value);
        }
        let capture = self.next;
        self.next += 1;
        let mut ids = Vec::with_capacity(parameters.len());
        let mut arguments = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let id = self.next;
            self.next += 1;
            ids.push(id);
            arguments.push(Expression::Variable {
                id,
                ty: parameter.clone(),
            });
        }
        let call = Expression::CallClosure {
            closure: Box::new(Expression::Variable {
                id: capture,
                ty: value_ty.clone(),
            }),
            arguments,
            result: success,
        };
        let body = self.done(call, outcome)?.in_tail_position();
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        let signature = Signature {
            parameters: parameters.clone(),
            result: (**outcome).clone(),
        };
        self.functions.push(Function {
            id,
            name: format!("outcome_{}", id.0),
            signature: signature.clone(),
            parameters: ids,
            captures: vec![(capture, value_ty)],
            body,
        });
        Ok(Expression::MakeClosure {
            function: id,
            signature,
            captures: vec![value],
        })
    }

    fn name_expression(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
    ) -> Result<Expression, String> {
        if !self.scope.contains_key(name) {
            if let Some(kind) = self.operations.get(name).copied() {
                return self.operation(node, name, vec![], kind);
            }
        }
        if crate::names::is_upper_reference(name) {
            return self.upper_reference(node, name);
        }
        if !self.scope.contains_key(name) {
            return self.definition_reference(name, node.span);
        }
        let (id, ty) = self
            .scope
            .get(name)
            .ok_or_else(|| format!("unknown local variable {name}"))?;
        Ok(Expression::Variable {
            id: *id,
            ty: ty.clone(),
        })
    }

    fn upper_reference(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
    ) -> Result<Expression, String> {
        let ty = self
            .value_types
            .get(&(node.span.line, node.span.col))
            .ok_or("missing constructor type")?;
        // 効果多相の callee の中では、この位置の型が `$Outcome` に
        // なっている。構築するのは成功型なので、そこでほどく。
        let ty = match Type::try_from(&self.specialization.resolve(ty)) {
            Ok(concrete) if is_outcome(&concrete) => Ty::from(&outcome_parts(&concrete)?.0),
            _ => ty.clone(),
        };
        self.construct(name, vec![], &ty)
    }

    fn do_expression(
        &mut self,
        node: &crate::parser::Node,
        value: &crate::parser::Node,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        if self.is_again_node(value) {
            return self.again_do(value, body, None);
        }
        if self.is_abort_node(value) && self.handler_for_node(value).is_some() {
            return self.expression(value);
        }
        let source = self.expression(value)?;
        let captures = self.contains_again(value) || self.contains_handled_call(value);
        if is_outcome(&source.ty()) {
            return self.propagate_do_outcome(node, value, body, source, captures);
        }
        if Self::is_option_expression(&source) {
            return self.option_propagation(None, source, body);
        }
        let id = self.next;
        self.next += 1;
        Ok(Expression::Let {
            id,
            value: Box::new(source),
            body: Box::new(self.expression(body)?),
        })
    }

    fn propagate_do_outcome(
        &mut self,
        node: &crate::parser::Node,
        value: &crate::parser::Node,
        body: &crate::parser::Node,
        source: Expression,
        captures: bool,
    ) -> Result<Expression, String> {
        if captures {
            return self.bind_again_do(node, value, body, source);
        }
        let pat = crate::parser::Pattern {
            kind: crate::parser::PatternKind::Underscore,
            span: value.span,
        };
        self.propagate_outcome(&pat, source, body)
    }

    fn bind_again_do(
        &mut self,
        node: &crate::parser::Node,
        value: &crate::parser::Node,
        body: &crate::parser::Node,
        source: Expression,
    ) -> Result<Expression, String> {
        use crate::parser::NodeKind;
        let name = format!("$do{}", self.next);
        self.next += 1;
        let pat = crate::parser::Pattern {
            kind: crate::parser::PatternKind::Name(name.clone()),
            span: value.span,
        };
        let rest = crate::parser::Node {
            kind: NodeKind::DoQ {
                value: Box::new(crate::parser::Node {
                    kind: NodeKind::Name(name),
                    span: value.span,
                }),
                body: Box::new(body.clone()),
            },
            span: node.span,
        };
        self.bind_again_outcome(&pat, source, &rest)
    }

    fn is_option_expression(value: &Expression) -> bool {
        matches!(&value.ty(), Type::Data(name, args) if name == "Option" && args.len() == 1)
    }

    fn let_expression(
        &mut self,
        pat: &crate::parser::Pattern,
        value: &crate::parser::Node,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        if self.is_again_node(value) {
            return self.again_do(value, body, Some(pat));
        }
        let captures = self.contains_again(value) || self.contains_handled_call(value);
        let value = self.expression(value)?;
        if is_outcome(&value.ty()) {
            if captures {
                return self.bind_again_outcome(pat, value, body);
            }
            return self.propagate_outcome(pat, value, body);
        }
        self.bound_body(pat, value, body)
    }

    fn pattern_branch(
        &mut self,
        scrutinees: &[crate::parser::Node],
        arms: &[(Vec<crate::parser::Pattern>, crate::parser::Node)],
        fallback: &Option<Box<crate::parser::Node>>,
    ) -> Result<Expression, String> {
        if scrutinees.len() == 1 && arms.iter().all(|(patterns, _)| patterns.len() == 1) {
            let unary: Vec<(crate::parser::Pattern, crate::parser::Node)> = arms
                .iter()
                .map(|(patterns, body)| (patterns[0].clone(), body.clone()))
                .collect();
            return self.literal_branch(&scrutinees[0], &unary, fallback.as_deref());
        }
        self.column_branch(scrutinees, arms, fallback.as_deref())
    }

    fn cond_branch(
        &mut self,
        arms: &[(crate::parser::Node, crate::parser::Node)],
        fallback: &Option<Box<crate::parser::Node>>,
    ) -> Result<Expression, String> {
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
        Ok(result)
    }

    fn bool_branch(
        &mut self,
        cond: &crate::parser::Node,
        then: &crate::parser::Node,
        els: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let condition = self.expression(cond)?;
        let then = self.expression(then)?;
        let otherwise = self.expression(els)?;
        let (then, otherwise) = self.align_outcomes(then, otherwise)?;
        branch(condition, then, otherwise)
    }

    fn unary_expression(
        &mut self,
        op: UnaryOp,
        operand: &crate::parser::Node,
    ) -> Result<Expression, String> {
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

    fn literal_expression(&self, node: &crate::parser::Node) -> Result<Expression, String> {
        match lower_literal(node) {
            Some(value) => Ok(Expression::Constant(value)),
            None => Err("expression lowering is not implemented for this node".into()),
        }
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
            let (right, otherwise) =
                self.align_outcomes(right, Expression::Constant(Constant::Bool(false)))?;
            return branch(left, right, otherwise);
        }
        if op == BinOp::Or {
            let (then, right) =
                self.align_outcomes(Expression::Constant(Constant::Bool(true)), right)?;
            return branch(left, then, right);
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
            return Ok(if op == BinOp::Ne {
                Expression::Unary {
                    op: UnaryOp::Not,
                    value: Box::new(equality),
                }
            } else {
                equality
            });
        }
        if matches!(op, BinOp::Div | BinOp::Mod) && left.ty() == Type::Int {
            return self.integer_division(op, left, right);
        }
        Ok(Expression::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn integer_division(
        &mut self,
        op: crate::parser::BinOp,
        left: Expression,
        right: Expression,
    ) -> Result<Expression, String> {
        let left_id = self.next;
        self.next += 1;
        let right_id = self.next;
        self.next += 1;
        let left_var = Expression::Variable {
            id: left_id,
            ty: Type::Int,
        };
        let right_var = Expression::Variable {
            id: right_id,
            ty: Type::Int,
        };
        let is_zero = Expression::Binary {
            op: crate::parser::BinOp::Eq,
            left: Box::new(right_var.clone()),
            right: Box::new(Expression::Constant(Constant::Int(0))),
        };
        let raised = self.raise_message("DivisionByZero", Type::Int)?;
        let divided = Expression::Binary {
            op,
            left: Box::new(left_var),
            right: Box::new(right_var),
        };
        let (then, otherwise) = self.align_outcomes(raised, divided)?;
        let body = branch(is_zero, then, otherwise)?;
        Ok(Expression::Let {
            id: left_id,
            value: Box::new(left),
            body: Box::new(Expression::Let {
                id: right_id,
                value: Box::new(right),
                body: Box::new(body),
            }),
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

fn wrap_column(source: Option<Expression>, id: usize, body: Expression) -> Expression {
    match source {
        Some(value) => Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(body),
        },
        None => body,
    }
}

fn wildcards(count: usize, span: crate::parser::Span) -> Vec<crate::parser::Pattern> {
    vec![
        crate::parser::Pattern {
            kind: crate::parser::PatternKind::Underscore,
            span,
        };
        count
    ]
}

fn same_literal(left: &crate::parser::Node, right: &crate::parser::Node) -> bool {
    use crate::parser::NodeKind;
    match (&left.kind, &right.kind) {
        (NodeKind::Int(left), NodeKind::Int(right)) => left == right,
        (NodeKind::Float(left), NodeKind::Float(right)) => left == right,
        (NodeKind::Bool(left), NodeKind::Bool(right)) => left == right,
        (NodeKind::Char(left), NodeKind::Char(right)) => left == right,
        (NodeKind::Str(left), NodeKind::Str(right)) => left == right,
        (NodeKind::Void, NodeKind::Void) => true,
        _ => false,
    }
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
        with_types: checked.with_types.clone(),
        operations: checked.operations.clone(),
        default_operations: checked.default_operations.clone(),
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
    let instances: Vec<_> = checked
        .instantiated_modules
        .iter()
        .cloned()
        .map(Item::ModuleImpl)
        .collect();
    lowerer.collect_modules(&instances, "");
    lowerer.schemes.extend(checked.module_definitions.clone());
    let body = lowerer.expression(ast.main().ok_or("main must be a value definition")?)?;
    lowerer.flush_pending_binds()?;
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

fn outcome_parts(ty: &Type) -> Result<(Type, Type, Type), String> {
    let Type::Data(name, args) = ty else {
        return Err("expected outcome".into());
    };
    if name != OUTCOME {
        return Err("expected outcome".into());
    }
    let success = args
        .first()
        .cloned()
        .ok_or("outcome missing success type")?;
    let answer = args.get(1).cloned().unwrap_or_else(|| success.clone());
    let payload = args.get(2).cloned().unwrap_or(Type::Void);
    Ok((success, answer, payload))
}

fn handler_operations(
    handler: &crate::parser::HandlerDecl,
    kind: crate::parser::OpKind,
) -> Vec<String> {
    handler
        .arms
        .iter()
        .filter_map(|arm| match arm {
            crate::parser::HandlerArm::Operation {
                kind: found, name, ..
            } if *found == kind => Some(name.clone()),
            _ => None,
        })
        .collect()
}

fn handler_has_operation(handler: &crate::parser::HandlerDecl, name: &str) -> bool {
    handler.arms.iter().any(|arm| {
        matches!(arm, crate::parser::HandlerArm::Operation { name: found, .. } if found == name)
    })
}

fn substitute_fields(ty: &Type, declared: &[Type], actual: &[Type]) -> Type {
    if let Some(index) = declared.iter().position(|declared| declared == ty) {
        return actual[index].clone();
    }
    match ty {
        Type::Data(name, arguments) => Type::Data(
            name.clone(),
            arguments
                .iter()
                .map(|argument| substitute_fields(argument, declared, actual))
                .collect(),
        ),
        Type::Closure(arguments, result) => Type::Closure(
            arguments
                .iter()
                .map(|argument| substitute_fields(argument, declared, actual))
                .collect(),
            Box::new(substitute_fields(result, declared, actual)),
        ),
        _ => ty.clone(),
    }
}

fn project(value: &Expression, constructor: &str, index: usize, result: Type) -> Expression {
    Expression::Project {
        value: Box::new(value.clone()),
        constructor: constructor.into(),
        index,
        result,
    }
}

struct ConvertCtx {
    incoming: Type,
    outgoing: Type,
    success: Type,
    convert_id: FunctionId,
    convert_sig: Signature,
    forwarding: bool,
    /// §38: handler の値引数のうち `_` 以外。convert 関数の追加の引数で、
    /// 腕の本体からはこの id で見える。継続が convert を呼び直すときにも渡す。
    value_params: Vec<(usize, Type)>,
}

impl ConvertCtx {
    fn continuation_ty(&self, input: Type) -> Type {
        Type::Closure(vec![input], Box::new(self.incoming.clone()))
    }
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
        let result = match result {
            Some(ty) => ty_with_fresh_rows(ty, &mut inference),
            None => inference.fresh(),
        };
        let expected_parameters: Vec<Ty> = parameters
            .iter()
            .map(|ty| ty_with_fresh_rows(ty, &mut inference))
            .collect();
        let expected = Ty::fn_with(expected_parameters, result.clone(), inference.fresh_row());
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
            Type::Bytes => Self::Bytes,
            Type::Void => Self::Void,
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

/// `Type` is the monomorphic type codegen sees. It has no effect rows, because
/// effects are lowered away before codegen. Turning a closure back into `Ty`
/// therefore gets a *fresh* row instead of `Row::pure()`: the definition's
/// scheme decides the row during specialization, rather than a rowless `Type`
/// forcing `pure` and rejecting an effectful function value (R49).
fn ty_with_fresh_rows(ty: &Type, inference: &mut crate::types::Inference) -> Ty {
    match ty {
        Type::Data(name, args) => Ty::Adt(
            name.clone(),
            args.iter()
                .map(|arg| ty_with_fresh_rows(arg, inference))
                .collect(),
        ),
        Type::Abstract(name, args) => Ty::Abstract(
            name.clone(),
            args.iter()
                .map(|arg| ty_with_fresh_rows(arg, inference))
                .collect(),
        ),
        Type::Closure(args, result) => {
            let args: Vec<Ty> = args
                .iter()
                .map(|arg| ty_with_fresh_rows(arg, inference))
                .collect();
            let result = ty_with_fresh_rows(result, inference);
            let row = inference.fresh_row();
            Ty::fn_with(args, result, row)
        }
        other => Ty::from(other),
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

/// A switch's arms (label, body) and its fallback body.
type SwitchArms = (Vec<(String, Expression)>, Option<Box<Expression>>);

impl Lowerer {
    fn call(
        &mut self,
        callee: &crate::parser::Node,
        arguments: Vec<Expression>,
        expected: Option<&Type>,
    ) -> Result<Expression, String> {
        if let crate::parser::NodeKind::Field(module, member) = &callee.kind {
            if let crate::parser::NodeKind::Name(module) = &module.kind {
                let qualified = self.resolve_definition(&format!("{module}.{}", member.name));
                if self.schemes.contains_key(&qualified) {
                    if self.is_value_definition(&qualified) {
                        return Expression::call_closure(self.expression(callee)?, arguments);
                    }
                    return self.direct_call(&qualified, arguments, expected);
                }
            }
        }
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            return self.direct_call(name, arguments, expected);
        }
        Expression::call_closure(self.expression(callee)?, arguments)
    }

    fn direct_call(
        &mut self,
        name: &str,
        arguments: Vec<Expression>,
        expected: Option<&Type>,
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
        let (id, signature) = self.specialize_function(name, types, expected)?;
        self.specialized_call(id, &signature, arguments)
    }

    /// 単相化した関数を呼ぶ。フレームの値引数があるときは、それを capture に
    /// 載せた閉包にして呼ぶ。呼んだ先の `once` の腕が §38 の値引数を参照する
    /// ためである。値引数が無ければ今までどおり直接呼ぶ。
    fn specialized_call(
        &mut self,
        id: FunctionId,
        signature: &Signature,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        let captures = self.frame_value_captures();
        if captures.is_empty() {
            return Expression::call_direct(id, signature, arguments);
        }
        Expression::call_closure(
            Expression::MakeClosure {
                function: id,
                signature: signature.clone(),
                captures,
            },
            arguments,
        )
    }

    /// §34: `e as p? ->` は `Some` の腕で `p` を束縛し、`None` の腕で `None` を返す。
    /// `e` が効果を運ぶときは先に outcome をほどいてから Option を見る。`as p ->`
    /// （`let_expression`）と同じ順であり、戻した `LetQ` が `$Outcome` を直に
    /// Option と見なして `propagation requires Option` で落ちる穴を塞ぐ。
    fn propagation(
        &mut self,
        pattern: Option<&crate::parser::Pattern>,
        source: &crate::parser::Node,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        if self.contains_again(source) || self.contains_handled_call(source) {
            return self.bind_option_source(pattern, source, body);
        }
        let value = self.expression(source)?;
        if is_outcome(&value.ty()) {
            return self.option_after_outcome(pattern, value, body);
        }
        self.option_propagation(pattern, value, body)
    }

    /// 効果を持つ `e` を `as $q ->` でほどき、成功値に対して `as p? ->` を続ける。
    fn bind_option_source(
        &mut self,
        pattern: Option<&crate::parser::Pattern>,
        source: &crate::parser::Node,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let name = format!("$q{}", self.next);
        self.next += 1;
        let tmp = crate::parser::Pattern {
            kind: crate::parser::PatternKind::Name(name.clone()),
            span: source.span,
        };
        self.let_expression(
            &tmp,
            source,
            &self.option_rest(pattern, &name, source.span, body),
        )
    }

    fn option_after_outcome(
        &mut self,
        pattern: Option<&crate::parser::Pattern>,
        value: Expression,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let name = format!("$q{}", self.next);
        self.next += 1;
        let span = body.span;
        let tmp = crate::parser::Pattern {
            kind: crate::parser::PatternKind::Name(name.clone()),
            span,
        };
        self.propagate_outcome(&tmp, value, &self.option_rest(pattern, &name, span, body))
    }

    fn option_rest(
        &self,
        pattern: Option<&crate::parser::Pattern>,
        name: &str,
        span: crate::parser::Span,
        body: &crate::parser::Node,
    ) -> crate::parser::Node {
        crate::parser::Node {
            span,
            kind: crate::parser::NodeKind::LetQ {
                pat: pattern.cloned().unwrap_or(crate::parser::Pattern {
                    kind: crate::parser::PatternKind::Underscore,
                    span,
                }),
                value: Box::new(crate::parser::Node {
                    kind: crate::parser::NodeKind::Name(name.into()),
                    span,
                }),
                body: Box::new(body.clone()),
            },
        }
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
        // 効果多相の callee の中では腕の型が `$Outcome` になりうる。`None` は
        // 成功型の値なので、構築してから done で包む。
        let otherwise = if is_outcome(&result) {
            let success = outcome_parts(&result)?.0;
            let none = self.construct("None", vec![], &Ty::from(&success))?;
            self.done(none, &result)?
        } else {
            self.construct("None", vec![], &Ty::from(&result))?
        };
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
            let pattern =
                crate::named_patterns::positional(pattern, &self.named_constructors, &|name| {
                    self.resolve_constructor(name)
                });
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

    fn column_branch(
        &mut self,
        scrutinees: &[crate::parser::Node],
        arms: &[(Vec<crate::parser::Pattern>, crate::parser::Node)],
        fallback: Option<&crate::parser::Node>,
    ) -> Result<Expression, String> {
        if scrutinees.is_empty()
            || arms
                .iter()
                .any(|(patterns, _)| patterns.len() != scrutinees.len())
        {
            return Err("pattern column count mismatch".into());
        }
        let saved = self.scope.clone();
        let mut bindings = Vec::new();
        let mut values = Vec::new();
        for scrutinee in scrutinees {
            let value = self.expression(scrutinee)?;
            let id = self.next;
            self.next += 1;
            let ty = value.ty();
            bindings.push((id, value));
            values.push(Expression::Variable { id, ty });
        }
        let fallback = fallback.map(|body| self.expression(body)).transpose()?;
        let rows: Vec<MatrixRow> = arms
            .iter()
            .map(|(patterns, body)| MatrixRow {
                patterns: patterns
                    .iter()
                    .map(|pattern| self.positional_pattern(pattern))
                    .collect(),
                body: body.clone(),
                bindings: Vec::new(),
            })
            .collect();
        let compiled = self.compile_matrix(&values, &rows, fallback.as_ref());
        self.scope = saved;
        let mut compiled = compiled?;
        for (id, value) in bindings.into_iter().rev() {
            compiled = Expression::Let {
                id,
                value: Box::new(value),
                body: Box::new(compiled),
            };
        }
        Ok(compiled)
    }

    fn compile_matrix(
        &mut self,
        values: &[Expression],
        rows: &[MatrixRow],
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        if rows.is_empty() {
            return fallback.cloned().ok_or_else(|| "empty switch".into());
        }
        if values.is_empty() {
            return self.take_matrix_body(&rows[0]);
        }
        if rows.iter().any(|row| row.patterns.len() != values.len()) {
            return Err("pattern column count mismatch".into());
        }
        match self.column_kind(&values[0].ty(), rows)? {
            ColumnKind::Any => self.compile_any_column(values, rows, fallback),
            ColumnKind::Bool => self.compile_bool_column(values, rows, fallback),
            ColumnKind::Constructor => self.compile_constructor_column(values, rows, fallback),
            ColumnKind::Literal => self.compile_literal_column(values, rows, fallback),
            ColumnKind::Record => self.compile_record_column(values, rows, fallback),
        }
    }

    fn take_matrix_body(&mut self, row: &MatrixRow) -> Result<Expression, String> {
        let saved = self.scope.clone();
        for (name, id, ty) in &row.bindings {
            self.scope.insert(name.clone(), (*id, ty.clone()));
        }
        let result = self.expression(&row.body);
        self.scope = saved;
        result
    }

    fn column_kind(&self, ty: &Type, rows: &[MatrixRow]) -> Result<ColumnKind, String> {
        let mut heads = ColumnHeads::default();
        for row in rows {
            self.classify_head(&row.patterns[0], &mut heads)?;
        }
        Ok(heads.kind(ty))
    }

    fn classify_head(
        &self,
        pattern: &crate::parser::Pattern,
        heads: &mut ColumnHeads,
    ) -> Result<(), String> {
        if self.record_head(pattern).is_some() {
            heads.record = true;
        } else if self.constructor_head(pattern).is_some() {
            heads.constructor = true;
        } else if self.literal_head(pattern).is_some() {
            heads.literal = true;
        } else if self.catch_all(pattern).is_none() {
            return Err("unsupported pattern in column".into());
        }
        Ok(())
    }

    fn compile_any_column(
        &mut self,
        values: &[Expression],
        rows: &[MatrixRow],
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        let (id, ty, _, source) = self.column_var(&values[0]);
        let rest: Vec<MatrixRow> = rows
            .iter()
            .map(|row| self.drop_column(row, id, &ty))
            .collect();
        let body = self.compile_matrix(&values[1..], &rest, fallback)?;
        Ok(wrap_column(source, id, body))
    }

    fn compile_bool_column(
        &mut self,
        values: &[Expression],
        rows: &[MatrixRow],
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        let (id, ty, variable, source) = self.column_var(&values[0]);
        let on_true = self.specialize_rows(rows, "true", 0, id, &ty);
        let on_false = self.specialize_rows(rows, "false", 0, id, &ty);
        let then = self.compile_optional_side(&values[1..], &on_true, fallback)?;
        let otherwise = self.compile_optional_side(&values[1..], &on_false, fallback)?;
        let (then, otherwise) = match (then, otherwise) {
            (Some(then), Some(otherwise)) => (then, otherwise),
            (Some(then), None) => {
                let result = then.ty();
                (then, Expression::Unreachable { result })
            }
            (None, Some(otherwise)) => {
                let result = otherwise.ty();
                (Expression::Unreachable { result }, otherwise)
            }
            (None, None) => return fallback.cloned().ok_or_else(|| "empty switch".into()),
        };
        let (then, otherwise) = self.align_outcomes(then, otherwise)?;
        Ok(wrap_column(source, id, branch(variable, then, otherwise)?))
    }

    fn compile_optional_side(
        &mut self,
        values: &[Expression],
        rows: &[MatrixRow],
        fallback: Option<&Expression>,
    ) -> Result<Option<Expression>, String> {
        if rows.is_empty() {
            return Ok(fallback.cloned());
        }
        self.compile_matrix(values, rows, fallback).map(Some)
    }

    fn compile_constructor_column(
        &mut self,
        values: &[Expression],
        rows: &[MatrixRow],
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        let (id, ty, variable, source) = self.column_var(&values[0]);
        let family = self.constructors_of(&ty);
        if rows
            .iter()
            .any(|row| self.catch_all(&row.patterns[0]).is_some())
            && family.is_empty()
        {
            return Err("unknown ADT in pattern column".into());
        }
        let mut lowered = Vec::new();
        for name in self.needed_constructors(rows, &family) {
            let fields = self.constructor_fields(&name, &ty)?;
            let specialized = self.specialize_rows(rows, &name, fields.len(), id, &ty);
            if specialized.is_empty() {
                continue;
            }
            let mut next_values: Vec<Expression> = fields
                .iter()
                .enumerate()
                .map(|(index, result)| Expression::Project {
                    value: Box::new(variable.clone()),
                    constructor: name.clone(),
                    index,
                    result: result.clone(),
                })
                .collect();
            next_values.extend(values.iter().skip(1).cloned());
            lowered.push((
                name,
                self.compile_matrix(&next_values, &specialized, fallback)?,
            ));
        }
        if lowered.is_empty() {
            return fallback.cloned().ok_or_else(|| "empty switch".into());
        }
        let switch_fallback = if family
            .iter()
            .all(|(name, _)| lowered.iter().any(|(found, _)| found == name))
        {
            None
        } else {
            fallback.cloned().map(Box::new)
        };
        let (lowered, fallback) = self.align_switch_arms(lowered, switch_fallback)?;
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
        Ok(wrap_column(
            source,
            id,
            Expression::Switch {
                value: Box::new(variable),
                arms: lowered,
                fallback,
                result,
            },
        ))
    }

    fn compile_literal_column(
        &mut self,
        values: &[Expression],
        rows: &[MatrixRow],
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        let (id, ty, variable, source) = self.column_var(&values[0]);
        let literals = self.column_literals(rows);
        let wilds: Vec<MatrixRow> = rows
            .iter()
            .filter(|row| self.catch_all(&row.patterns[0]).is_some())
            .map(|row| self.drop_column(row, id, &ty))
            .collect();
        let mut result = if wilds.is_empty() {
            fallback
                .cloned()
                .ok_or("pattern branch without fallback is not implemented")?
        } else {
            self.compile_matrix(&values[1..], &wilds, fallback)?
        };
        for literal in literals.into_iter().rev() {
            let matching: Vec<MatrixRow> = rows
                .iter()
                .filter(|row| self.row_matches_literal(row, &literal))
                .map(|row| self.drop_column(row, id, &ty))
                .collect();
            let body = self.compile_matrix(&values[1..], &matching, fallback)?;
            let right = self.expression(&literal)?;
            if right.ty() != variable.ty() {
                return Err("pattern type mismatch".into());
            }
            let condition = Expression::Binary {
                op: crate::parser::BinOp::Eq,
                left: Box::new(variable.clone()),
                right: Box::new(right),
            };
            let (body, result_arm) = self.align_outcomes(body, result)?;
            result = branch(condition, body, result_arm)?;
        }
        Ok(wrap_column(source, id, result))
    }

    fn compile_record_column(
        &mut self,
        values: &[Expression],
        rows: &[MatrixRow],
        fallback: Option<&Expression>,
    ) -> Result<Expression, String> {
        let (id, ty, variable, source) = self.column_var(&values[0]);
        let fields = self.record_column_fields(&ty, rows)?;
        let mut next_values = Vec::new();
        for field in &fields {
            next_values.push(self.record_get(variable.clone(), field)?);
        }
        next_values.extend(values.iter().skip(1).cloned());
        let rest: Vec<MatrixRow> = rows
            .iter()
            .map(|row| self.expand_record_row(row, &fields, id, &ty))
            .collect::<Result<_, _>>()?;
        let body = self.compile_matrix(&next_values, &rest, fallback)?;
        Ok(wrap_column(source, id, body))
    }

    fn needed_constructors(&self, rows: &[MatrixRow], family: &[(String, usize)]) -> Vec<String> {
        let mut needed = Vec::new();
        for row in rows {
            if let Some((name, _)) = self.constructor_head(&row.patterns[0]) {
                if !needed.iter().any(|found| found == &name) {
                    needed.push(name);
                }
            }
        }
        if rows
            .iter()
            .any(|row| self.catch_all(&row.patterns[0]).is_some())
        {
            for (name, _) in family {
                if !needed.iter().any(|found| found == name) {
                    needed.push(name.clone());
                }
            }
        }
        needed
    }

    fn constructors_of(&self, ty: &Type) -> Vec<(String, usize)> {
        if *ty == Type::Bool {
            return vec![("true".into(), 0), ("false".into(), 0)];
        }
        let mut variants: Vec<_> = self
            .constructors
            .keys()
            .filter_map(|name| {
                self.constructor_fields(name, ty)
                    .ok()
                    .map(|fields| (name.clone(), fields.len()))
            })
            .collect();
        variants
            .sort_by_key(|(name, _)| self.constructor_tags.get(name).copied().unwrap_or(u32::MAX));
        variants
    }

    fn specialize_rows(
        &self,
        rows: &[MatrixRow],
        name: &str,
        arity: usize,
        id: usize,
        ty: &Type,
    ) -> Vec<MatrixRow> {
        rows.iter()
            .filter_map(|row| self.specialize_constructor(row, name, arity, id, ty))
            .collect()
    }

    fn specialize_constructor(
        &self,
        row: &MatrixRow,
        name: &str,
        arity: usize,
        id: usize,
        ty: &Type,
    ) -> Option<MatrixRow> {
        let span = row.patterns[0].span;
        if let Some(bind) = self.catch_all(&row.patterns[0]) {
            let mut bindings = row.bindings.clone();
            if let Some(name) = bind {
                bindings.push((name, id, ty.clone()));
            }
            let mut patterns = wildcards(arity, span);
            patterns.extend(row.patterns.iter().skip(1).cloned());
            return Some(MatrixRow {
                patterns,
                body: row.body.clone(),
                bindings,
            });
        }
        let (found, args) = self.constructor_head(&row.patterns[0])?;
        if found != name || args.len() != arity {
            return None;
        }
        let mut patterns = args;
        patterns.extend(row.patterns.iter().skip(1).cloned());
        Some(MatrixRow {
            patterns,
            body: row.body.clone(),
            bindings: row.bindings.clone(),
        })
    }

    fn drop_column(&self, row: &MatrixRow, id: usize, ty: &Type) -> MatrixRow {
        let mut bindings = row.bindings.clone();
        if let Some(Some(name)) = self.catch_all(&row.patterns[0]) {
            bindings.push((name, id, ty.clone()));
        }
        MatrixRow {
            patterns: row.patterns[1..].to_vec(),
            body: row.body.clone(),
            bindings,
        }
    }

    fn column_literals(&self, rows: &[MatrixRow]) -> Vec<crate::parser::Node> {
        let mut literals = Vec::new();
        for row in rows {
            if let Some(literal) = self.literal_head(&row.patterns[0]) {
                if !literals.iter().any(|found| same_literal(found, &literal)) {
                    literals.push(literal);
                }
            }
        }
        literals
    }

    fn row_matches_literal(&self, row: &MatrixRow, literal: &crate::parser::Node) -> bool {
        self.catch_all(&row.patterns[0]).is_some()
            || self
                .literal_head(&row.patterns[0])
                .is_some_and(|found| same_literal(&found, literal))
    }

    fn record_column_fields(&self, ty: &Type, rows: &[MatrixRow]) -> Result<Vec<String>, String> {
        let Type::Data(name, _) = ty else {
            return Err("record pattern requires a record".into());
        };
        if let Some((names, _)) = self.records.get(name) {
            return Ok(names.clone());
        }
        let mut names = Vec::new();
        for row in rows {
            if let Some(fields) = self.record_head(&row.patterns[0]) {
                for name in fields {
                    if !names.iter().any(|found| found == &name) {
                        names.push(name);
                    }
                }
            }
        }
        if names.is_empty() {
            Err("unknown record type".into())
        } else {
            Ok(names)
        }
    }

    fn expand_record_row(
        &self,
        row: &MatrixRow,
        fields: &[String],
        id: usize,
        ty: &Type,
    ) -> Result<MatrixRow, String> {
        let pattern = self.positional_pattern(&row.patterns[0]);
        let mut bindings = row.bindings.clone();
        let mut expanded = match &pattern.kind {
            crate::parser::PatternKind::Record(written) => fields
                .iter()
                .map(
                    |name| match written.iter().find(|(field, _)| &field.name == name) {
                        Some((_, Some(nested))) => nested.clone(),
                        Some((field, None)) => crate::parser::Pattern {
                            kind: crate::parser::PatternKind::Name(field.name.clone()),
                            span: field.span,
                        },
                        None => crate::parser::Pattern {
                            kind: crate::parser::PatternKind::Underscore,
                            span: pattern.span,
                        },
                    },
                )
                .collect(),
            _ => {
                if let Some(Some(name)) = self.catch_all(&pattern) {
                    bindings.push((name, id, ty.clone()));
                } else if self.catch_all(&pattern).is_none() {
                    return Err("expected record pattern".into());
                }
                wildcards(fields.len(), pattern.span)
            }
        };
        expanded.extend(row.patterns.iter().skip(1).cloned());
        Ok(MatrixRow {
            patterns: expanded,
            body: row.body.clone(),
            bindings,
        })
    }

    fn column_var(&mut self, value: &Expression) -> (usize, Type, Expression, Option<Expression>) {
        if let Expression::Variable { id, ty } = value {
            return (*id, ty.clone(), value.clone(), None);
        }
        let id = self.next;
        self.next += 1;
        let ty = value.ty();
        (
            id,
            ty.clone(),
            Expression::Variable { id, ty },
            Some(value.clone()),
        )
    }

    fn positional_pattern(&self, pattern: &crate::parser::Pattern) -> crate::parser::Pattern {
        crate::named_patterns::positional(pattern, &self.named_constructors, &|name| {
            self.resolve_constructor(name)
        })
    }

    fn catch_all(&self, pattern: &crate::parser::Pattern) -> Option<Option<String>> {
        match &self.positional_pattern(pattern).kind {
            crate::parser::PatternKind::Underscore => Some(None),
            crate::parser::PatternKind::Name(name) if !crate::names::is_upper_reference(name) => {
                Some(Some(name.clone()))
            }
            _ => None,
        }
    }

    fn constructor_head(
        &self,
        pattern: &crate::parser::Pattern,
    ) -> Option<(String, Vec<crate::parser::Pattern>)> {
        match &self.positional_pattern(pattern).kind {
            crate::parser::PatternKind::Name(name) if crate::names::is_upper_reference(name) => {
                Some((self.resolve_constructor(name), Vec::new()))
            }
            crate::parser::PatternKind::Constructor(name, args) => {
                Some((self.resolve_constructor(name), args.clone()))
            }
            crate::parser::PatternKind::Literal(node) => match node.kind {
                crate::parser::NodeKind::Bool(true) => Some(("true".into(), Vec::new())),
                crate::parser::NodeKind::Bool(false) => Some(("false".into(), Vec::new())),
                _ => None,
            },
            _ => None,
        }
    }

    fn literal_head(&self, pattern: &crate::parser::Pattern) -> Option<crate::parser::Node> {
        match &self.positional_pattern(pattern).kind {
            crate::parser::PatternKind::Literal(node)
                if !matches!(node.kind, crate::parser::NodeKind::Bool(_)) =>
            {
                Some((**node).clone())
            }
            _ => None,
        }
    }

    fn record_head(&self, pattern: &crate::parser::Pattern) -> Option<Vec<String>> {
        match &self.positional_pattern(pattern).kind {
            crate::parser::PatternKind::Record(fields) => {
                Some(fields.iter().map(|(name, _)| name.name.clone()).collect())
            }
            _ => None,
        }
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
        let pattern =
            crate::named_patterns::positional(pattern, &self.named_constructors, &|name| {
                self.resolve_constructor(name)
            });
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
            PatternKind::NamedConstructor(_, _) => {
                return Err("missing named constructor metadata".into())
            }
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
        let pattern =
            crate::named_patterns::positional(pattern, &self.named_constructors, &|name| {
                self.resolve_constructor(name)
            });
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
            PatternKind::NamedConstructor(_, _) => {
                return Err("missing named constructor metadata".into())
            }
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
        self.constructor_field_types(name, ty, true)
    }

    fn constructor_declared_fields(&self, name: &str, ty: &Type) -> Result<Vec<Type>, String> {
        self.constructor_field_types(name, ty, false)
    }

    // 効果を持つ関数を field に取る constructor は、mono では結果が outcome の
    // 閉包を field に持つ。宣言側の行き先に型引数が無い（単相の ADT）場合でも
    // 射影が実際の型を見るように、Ty のうちに効果行を見る。
    fn constructor_field_types(
        &self,
        name: &str,
        ty: &Type,
        adjust: bool,
    ) -> Result<Vec<Type>, String> {
        let scheme = self.constructors.get(name).ok_or("unknown constructor")?;
        let mut inference = crate::types::Inference::default();
        let instantiated = inference.instantiate(scheme);
        let (fields, result) = match instantiated {
            Ty::Fn(fields, result, _) => (fields, *result),
            result => (vec![], result),
        };
        let expected = ty_with_fresh_rows(ty, &mut inference);
        inference
            .unify(&result, &expected)
            .map_err(|error| format!("constructor type mismatch: {error:?}"))?;
        fields
            .iter()
            .map(|field| {
                let field = inference.resolve(field);
                if adjust {
                    if let Some(closure) = self.outcome_closure_of(&field) {
                        return Ok(closure);
                    }
                }
                Type::try_from(&field).map_err(|_| "constructor field type is not concrete".into())
            })
            .collect()
    }

    // 効果を持つ関数値は mono では結果が outcome の閉包になる。constructor の
    // 結果型も、宣言側の field 型ではなく実際の field 型で組む。
    fn constructed_type(
        &self,
        name: &str,
        declared: &Type,
        fields: &[Type],
    ) -> Result<Type, String> {
        let declared_fields = self
            .constructor_declared_fields(name, declared)
            .unwrap_or_default();
        if declared_fields.len() != fields.len() {
            return Ok(declared.clone());
        }
        Ok(substitute_fields(declared, &declared_fields, fields))
    }

    fn named_constructor(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
        written: &[(crate::parser::DeclaredName, crate::parser::Node)],
    ) -> Result<Expression, String> {
        let name = self.resolve_constructor(name);
        let declared = self
            .named_constructors
            .get(&name)
            .ok_or("missing named constructor metadata")?
            .clone();
        let ty = self
            .composite_types
            .get(&(node.span.line, node.span.col))
            .ok_or("missing named constructor type")?
            .clone();
        let mut bindings = Vec::new();
        let mut variables = std::collections::HashMap::new();
        for (field, value) in written {
            let value = self.expression(value)?;
            let id = self.next;
            self.next += 1;
            variables.insert(
                field.name.clone(),
                Expression::Variable { id, ty: value.ty() },
            );
            bindings.push((id, value));
        }
        let fields = declared
            .iter()
            .map(|name| {
                variables
                    .remove(name)
                    .ok_or_else(|| "missing named field".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let declared_type = Type::try_from(&self.specialization.resolve(&ty))
            .map_err(|_| "named constructor type is not concrete")?;
        let result = self.constructed_type(
            &name,
            &declared_type,
            &fields.iter().map(Expression::ty).collect::<Vec<_>>(),
        )?;
        let mut body = Expression::Construct {
            constructor: name.clone(),
            fields,
            result,
        };
        for (id, value) in bindings.into_iter().rev() {
            body = Expression::Let {
                id,
                value: Box::new(value),
                body: Box::new(body),
            };
        }
        Ok(body)
    }

    // 今の stack の handler が扱う効果を運ぶ関数型か。
    fn row_carries_handled_effect(&self, row: &crate::types::Row) -> bool {
        self.handler_stack
            .iter()
            .enumerate()
            .any(|(index, handler)| {
                !self.arm_handlers.contains(&index)
                    && Self::handler_has_propagation_ops(handler)
                    && row.without_effect(&handler.effect) != *row
            })
    }

    // checker の型が効果を運ぶ関数なら、mono では結果が outcome の閉包になる。
    // ADT の型引数や、効果を運ばない関数の引数・結果に入れ子で載る場合も、
    // その内側だけを直す。
    fn outcome_closure_of(&self, ty: &Ty) -> Option<Type> {
        let answer = self
            .context_answer
            .clone()
            .or_else(|| self.again_answers.last().cloned())?;
        let (converted, changed) = self.nested_outcome_closure(ty, &answer).ok()?;
        changed.then_some(converted)
    }

    // 効果を運ぶ関数を outcome の閉包へ直し、直した箇所があったかを返す。
    fn nested_outcome_closure(
        &self,
        ty: &Ty,
        answer: &Type,
    ) -> Result<(Type, bool), UnresolvedType> {
        match ty {
            Ty::Fn(arguments, result, row) => {
                let mut converted_arguments = Vec::with_capacity(arguments.len());
                let mut changed = false;
                for argument in arguments {
                    let (argument, nested) = self.nested_outcome_closure(argument, answer)?;
                    changed |= nested;
                    converted_arguments.push(argument);
                }
                let (converted_result, nested) = self.nested_outcome_closure(result, answer)?;
                changed |= nested;
                if self.row_carries_handled_effect(row) {
                    let outcome = self
                        .outcome_of(&converted_result, answer)
                        .map_err(|_| UnresolvedType::Invalid)?;
                    return Ok((Type::Closure(converted_arguments, Box::new(outcome)), true));
                }
                Ok((
                    Type::Closure(converted_arguments, Box::new(converted_result)),
                    changed,
                ))
            }
            Ty::Adt(name, arguments) => {
                let mut changed = false;
                let mut converted = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    let (argument, nested) = self.nested_outcome_closure(argument, answer)?;
                    changed |= nested;
                    converted.push(argument);
                }
                Ok((Type::Data(name.clone(), converted), changed))
            }
            Ty::Abstract(name, arguments) => {
                let mut changed = false;
                let mut converted = Vec::with_capacity(arguments.len());
                for argument in arguments {
                    let (argument, nested) = self.nested_outcome_closure(argument, answer)?;
                    changed |= nested;
                    converted.push(argument);
                }
                Ok((Type::Abstract(name.clone(), converted), changed))
            }
            _ => Ok((Type::try_from(ty)?, false)),
        }
    }

    fn record_get(&self, record: Expression, field: &str) -> Result<Expression, String> {
        let ty = record.ty();
        let Type::Data(name, _) = &ty else {
            return Err("field access requires a record".into());
        };
        if !self.records.contains_key(name) {
            for (constructor, names) in &self.named_constructors {
                let Ok(fields) = self.constructor_fields(constructor, &ty) else {
                    continue;
                };
                let index = names
                    .iter()
                    .position(|name| name == field)
                    .ok_or("unknown named field")?;
                return Ok(Expression::Project {
                    value: Box::new(record),
                    constructor: constructor.clone(),
                    index,
                    result: fields[index].clone(),
                });
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
        let expected = ty_with_fresh_rows(&ty, &mut inference);
        inference
            .unify(&result, &expected)
            .map_err(|error| format!("record type mismatch: {error:?}"))?;
        let field_ty = inference.resolve(&fields[index]);
        let result = match self.outcome_closure_of(&field_ty) {
            Some(closure) => closure,
            None => Type::try_from(&field_ty).map_err(|_| "record field type is not concrete")?,
        };
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
            if !self.scope.contains_key(name) {
                if let Some(kind) = self.operations.get(name).copied() {
                    return self.operation(node, name, arguments, kind);
                }
            }
        }
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            if let Some(function) = crate::runtime_abi::RuntimeFunction::from_intrinsic(name) {
                if function.is_polymorphic() {
                    let allocation = if function == crate::runtime_abi::RuntimeFunction::ArrayAlloc
                    {
                        let ty = self
                            .composite_types
                            .get(&(node.span.line, node.span.col))
                            .ok_or("missing array allocation result type")?;
                        let allocation = Type::try_from(&self.specialization.resolve(ty))
                            .map_err(|_| "array allocation result type is not concrete")?;
                        Some(allocation)
                    } else {
                        None
                    };
                    return Expression::call_array(function, arguments, allocation);
                }
                return Expression::call_runtime(function, arguments);
            }
        }
        if let crate::parser::NodeKind::Name(name) = &callee.kind {
            if crate::names::is_upper_reference(name) {
                let ty = self
                    .composite_types
                    .get(&(node.span.line, node.span.col))
                    .ok_or("missing constructor result type")?;
                let declared = Type::try_from(&self.specialization.resolve(ty))
                    .map_err(|_| "constructor type is not concrete")?;
                let result = self.constructed_type(
                    &self.resolve_constructor(name),
                    &declared,
                    &arguments.iter().map(Expression::ty).collect::<Vec<_>>(),
                )?;
                return Ok(Expression::Construct {
                    constructor: self.resolve_constructor(name),
                    fields: arguments,
                    result,
                });
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
                    .map(|ty| self.actual_type(&self.specialization.resolve(ty)))
                    .transpose()
                    .map_err(|_| "call result type is not concrete")?;
                return self.direct_call(name, arguments, expected.as_ref());
            }
        }
        // module 修飾の呼び出し（`Maker.make`）も、呼び出し位置の期待する結果型を
        // 渡す。契約の member が返す閉包の引数のように、引数から決まらない型を
        // 期待する結果型から解くため。
        let expected = self
            .composite_types
            .get(&(node.span.line, node.span.col))
            .and_then(|ty| self.actual_type(&self.specialization.resolve(ty)).ok());
        self.call(callee, arguments, expected.as_ref())
    }

    fn abort_raise(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        let ty = self
            .composite_types
            .get(&(node.span.line, node.span.col))
            .or_else(|| self.value_types.get(&(node.span.line, node.span.col)))
            .ok_or("missing abort result type")?;
        let result = match Type::try_from(&self.specialization.resolve(ty)) {
            Ok(ty) => ty,
            Err(UnresolvedType::Variable(_)) => Type::Void,
            Err(_) => return Err("abort result type is not concrete".into()),
        };
        self.abort_raise_with(name, arguments, result)
    }

    fn abort_raise_with(
        &mut self,
        fallback: &str,
        arguments: Vec<Expression>,
        result: Type,
    ) -> Result<Expression, String> {
        let message = match arguments.len() {
            1 => Expression::RecordGet {
                record: Box::new(arguments.into_iter().next().unwrap()),
                field: "message".into(),
                result: Type::String,
            },
            0 => Expression::Constant(Constant::String(fallback.into())),
            _ => return Err("abort operation has too many arguments".into()),
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

    fn raise_message(&mut self, message: &str, success: Type) -> Result<Expression, String> {
        let exception = Expression::Record {
            fields: vec![(
                "message".into(),
                Expression::Constant(Constant::String(message.into())),
            )],
            result: Type::Data("Exception".into(), vec![]),
        };
        match self.handler_for("raise") {
            Some(_) => self.abort_outcome_with(vec![exception], "raise", success),
            None => self.abort_raise_with("raise", vec![exception], success),
        }
    }

    fn operation(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
        arguments: Vec<Expression>,
        kind: crate::parser::OpKind,
    ) -> Result<Expression, String> {
        match kind {
            crate::parser::OpKind::Again => self.again_operation(node, name, arguments, None),
            crate::parser::OpKind::Once => {
                if self.handler_for(name).is_some() {
                    self.handled_operation(name, arguments)
                } else {
                    self.default_operation(node, name, arguments)
                }
            }
            crate::parser::OpKind::Abort => {
                if self.handler_for(name).is_some() {
                    self.abort_outcome(node, arguments, name)
                } else {
                    self.abort_raise(node, name, arguments)
                }
            }
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
        // この `with` の本体と腕の節点を、この `with` が推論された型で lower
        // する。同じ節点が答え型の違う別の `with` にも現れると span 単位の表は
        // 最後のもので上書きされているため、ここで差し替える。
        let installed = self
            .with_types
            .get(&(node.span.line, node.span.col))
            .cloned()
            .map(|types| self.install_with_types(&types));
        // §38: 値引数は `with` の位置で 1 回だけ評価する。名前は腕からだけ
        // 見えるので `self.scope` には入れず、`frame_value_args` に持つ。
        let mut evaluated = Vec::new();
        for arg in args {
            evaluated.push(self.expression(arg)?);
        }
        let bindings = self.handler_value_bindings(&handler, evaluated)?;
        let needs_answer = handler.arms.iter().any(|arm| {
            matches!(
                arm,
                crate::parser::HandlerArm::Return { .. }
                    | crate::parser::HandlerArm::Operation {
                        kind: crate::parser::OpKind::Again | crate::parser::OpKind::Abort,
                        ..
                    }
            )
        });
        let answer = if needs_answer {
            self.again_answer_of(node)?
        } else {
            None
        };
        if let Some(answer) = answer.clone() {
            self.again_answers.push(answer);
        }
        let saved = std::mem::replace(&mut self.rewriting_return, false);
        self.handler_stack.push(handler.clone());
        let named = bindings.named.clone();
        self.frame_value_args.push(named);
        self.handler_answers.push(answer.clone());
        let value = self.completion(body);
        self.handler_stack.pop();
        self.frame_value_args.pop();
        self.handler_answers.pop();
        self.rewriting_return = saved;
        if answer.is_some() {
            self.again_answers.pop();
        }
        if let Some(installed) = installed {
            self.restore_with_types(installed);
        }
        let mut value = value?;
        for (id, argument) in bindings.lets.into_iter().rev() {
            value = Expression::Let {
                id,
                value: Box::new(argument),
                body: Box::new(value),
            };
        }
        Ok(value)
    }

    /// handler の値引数を、このフレームで参照できる id に束ねる。
    fn handler_value_bindings(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        arguments: Vec<Expression>,
    ) -> Result<HandlerValueBindings, String> {
        if handler.params.len() != arguments.len() {
            return Err("handler value argument arity mismatch".into());
        }
        let mut bindings = HandlerValueBindings::default();
        for (param, argument) in handler.params.iter().zip(arguments) {
            let id = self.next;
            self.next += 1;
            let ty = argument.ty();
            if param.name != "_" {
                bindings.named.push((param.name.clone(), (id, ty)));
            }
            bindings.lets.push((id, argument));
        }
        Ok(bindings)
    }

    /// いま lower しているフレームの値引数を、そのまま値として取り出す。
    /// `carry_frame_value_args` と同じ順に並べる。腕を lower している間は、
    /// その腕の handler を数えない（`specialized_name` と同じ理由である。
    /// 腕は自分の handler の外で走るので、呼んだ先もその handler では捕まえない）。
    fn frame_value_captures(&self) -> Vec<Expression> {
        self.frame_value_args
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.arm_handlers.contains(index))
            .flat_map(|(_, values)| values.iter())
            .map(|(_, (id, ty))| Expression::Variable {
                id: *id,
                ty: ty.clone(),
            })
            .collect()
    }

    /// フレームの値引数を、これから lower する関数（単相化した `def`、lambda、
    /// 継続）へ capture として引き継ぐ。capture の id はこのフレームのまま使う。
    /// 閉包は環境の値をその id へ読み戻すので、本体の中でも同じ id で参照できる
    /// （lambda が `self.scope` の束縛を capture にするのと同じ形である）。
    /// 載せる組は handler の並びだけで決まる（`frame_value_captures` と同じ）。
    /// 同じ単相化を共有する呼び出し位置で capture の数が食い違わないためである。
    fn carry_frame_value_args(&self) -> Vec<(usize, Type)> {
        self.frame_value_args
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.arm_handlers.contains(index))
            .flat_map(|(_, values)| values.iter())
            .map(|(_, (id, ty))| (*id, ty.clone()))
            .collect()
    }

    fn install_with_types(&mut self, types: &crate::WithTypes) -> InstalledTypes {
        let values = types
            .values
            .iter()
            .map(|(span, ty)| (*span, self.value_types.insert(*span, ty.clone())))
            .collect();
        let composites = types
            .composites
            .iter()
            .map(|(span, ty)| (*span, self.composite_types.insert(*span, ty.clone())))
            .collect();
        InstalledTypes { values, composites }
    }

    fn restore_with_types(&mut self, installed: InstalledTypes) {
        for (span, previous) in installed.values {
            match previous {
                Some(ty) => {
                    self.value_types.insert(span, ty);
                }
                None => {
                    self.value_types.remove(&span);
                }
            }
        }
        for (span, previous) in installed.composites {
            match previous {
                Some(ty) => {
                    self.composite_types.insert(span, ty);
                }
                None => {
                    self.composite_types.remove(&span);
                }
            }
        }
    }

    fn default_operation(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
        arguments: Vec<Expression>,
    ) -> Result<Expression, String> {
        let definition = self
            .default_operations
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unhandled operation {name}"))?;
        if arguments.is_empty() {
            return self.definition_reference(&definition, node.span);
        }
        let expected = self
            .composite_types
            .get(&(node.span.line, node.span.col))
            .ok_or("missing default operation result type")?;
        let expected = Type::try_from(&self.specialization.resolve(expected))
            .map_err(|_| "default operation result type is not concrete")?;
        let parameters = arguments.iter().map(Expression::ty).collect();
        let (id, signature) = self.specialize_function(&definition, parameters, Some(&expected))?;
        self.specialized_call(id, &signature, arguments)
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
        let (params, body) = handler
            .arms
            .iter()
            .find_map(|arm| match arm {
                crate::parser::HandlerArm::Operation {
                    name: op,
                    params,
                    body,
                    ..
                } if op == name => Some((params.clone(), body.clone())),
                _ => None,
            })
            .ok_or_else(|| format!("missing handler arm for {name}"))?;
        // §38: `once` の腕は操作の位置で走る。値引数の名前はこのフレームの値
        // （`with` の位置の変数、または単相化した関数の capture）を指す。呼んだ
        // 関数の引数が同じ名前でも、腕から見えるのは値引数の方である。操作の
        // 引数はこの後に束ねるので、名前が重なれば操作の引数が勝つ。
        let saved_scope = self.install_handler_frame_values(&handler);
        let result = self.with_params(&params, arguments, |this| {
            let saved = std::mem::replace(&mut this.rewriting_return, true);
            let entered = this.enter_arm(&handler);
            let result = this.expression(&body);
            this.rewriting_return = saved;
            this.leave_arm(entered);
            result
        });
        self.restore_value_params(saved_scope);
        result
    }

    /// handler の値引数の名前を、このフレームの値に向け直す。戻すための元の
    /// 値を返す。腕の本体は handler の宣言の中にあり、`with` の本体の束縛は
    /// 見えないので、腕から見える値引数はこの表だけで決まる。
    fn install_handler_frame_values(
        &mut self,
        handler: &crate::parser::HandlerDecl,
    ) -> Vec<(String, Option<(usize, Type)>)> {
        let named = self
            .handler_index(handler)
            .and_then(|index| self.frame_value_args.get(index))
            .cloned()
            .unwrap_or_default();
        self.install_value_params(&named)
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
                saved.push((
                    param.name.clone(),
                    self.scope.insert(param.name.clone(), (id, ty)),
                ));
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

    fn outcome_payload(&self, constructor: &str) -> Result<Type, String> {
        let signature = self
            .operation_types
            .get(constructor)
            .ok_or("operation type missing")?;
        Ok(signature.parameters.first().cloned().unwrap_or(Type::Void))
    }

    fn fail_constructor(&mut self, operation: &str) -> String {
        format!("{FAIL}#{}", self.fail_tag_for(operation))
    }

    fn yield_constructor(&mut self, operation: &str) -> String {
        format!("{YIELD}#{}", self.yield_tag_for(operation))
    }

    fn fail_tag_for(&mut self, operation: &str) -> u32 {
        if let Some(&tag) = self.fail_ids.get(operation) {
            return tag;
        }
        let tag = self.alloc_outcome_tag();
        self.fail_ids.insert(operation.to_string(), tag);
        tag
    }

    fn yield_tag_for(&mut self, operation: &str) -> u32 {
        if let Some(&tag) = self.yield_ids.get(operation) {
            return tag;
        }
        let tag = self.alloc_outcome_tag();
        self.yield_ids.insert(operation.to_string(), tag);
        tag
    }

    fn alloc_outcome_tag(&mut self) -> u32 {
        if self.next_fail_tag < 1 {
            self.next_fail_tag = 1;
        }
        let tag = self.next_fail_tag;
        self.next_fail_tag += 1;
        tag
    }

    fn answer_for_handler(&self, target: &crate::parser::HandlerDecl) -> Result<Type, String> {
        self.handler_stack
            .iter()
            .rev()
            .zip(self.handler_answers.iter().rev())
            .find(|(handler, _)| handler.name == target.name && handler.effect == target.effect)
            .and_then(|(_, answer)| answer.clone())
            .ok_or_else(|| "again handler missing answer".into())
    }

    // 効果行の効果を捕まえる handler がこの文脈にあれば、その答えを返す。同じ
    // 効果を捕まえる handler が入れ子のときは内側を選ぶ（`handler_for` と同じ）。
    fn answer_for_row(&self, row: &crate::types::Row) -> Option<Type> {
        let labels = match row {
            crate::types::Row::Closed(labels) | crate::types::Row::Open(_, labels) => labels,
            crate::types::Row::Var(_) => return None,
        };
        let names: Vec<String> = labels.iter().map(crate::types::effect_ctor).collect();
        for (index, handler) in self.handler_stack.iter().enumerate().rev() {
            if self.arm_handlers.contains(&index) {
                continue;
            }
            if names
                .iter()
                .any(|label| crate::types::label_matches_effect(label, &handler.effect))
            {
                return self
                    .answer_for_handler(handler)
                    .ok()
                    .or_else(|| self.context_answer.clone());
            }
        }
        None
    }

    // checker の型を mono の実際の型にする。効果行を持つ閉包は結果に `$Outcome` を
    // 載せる。handler が無い効果は閉包を呼ぶ側で捕まえるので、この文脈で捕まえる
    // 効果だけを載せる。
    fn actual_type(&self, ty: &Ty) -> Result<Type, UnresolvedType> {
        match ty {
            Ty::Fn(parameters, result, row) => {
                let parameters = parameters
                    .iter()
                    .map(|parameter| self.actual_type(parameter))
                    .collect::<Result<Vec<_>, _>>()?;
                let result = self.actual_type(result)?;
                let result = match self.answer_for_row(row) {
                    Some(answer) => Type::Data(OUTCOME.into(), vec![result, answer, Type::Void]),
                    None => result,
                };
                Ok(Type::Closure(parameters, Box::new(result)))
            }
            Ty::Adt(name, arguments) => Ok(Type::Data(
                name.clone(),
                arguments
                    .iter()
                    .map(|argument| self.actual_type(argument))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            Ty::Abstract(name, arguments) => Ok(Type::Abstract(
                name.clone(),
                arguments
                    .iter()
                    .map(|argument| self.actual_type(argument))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            other => Type::try_from(other),
        }
    }

    fn handler_for(&self, operation: &str) -> Option<&crate::parser::HandlerDecl> {
        self.handler_stack
            .iter()
            .enumerate()
            .rev()
            .find(|(index, handler)| {
                !self.arm_handlers.contains(index)
                    && handler.arms.iter().any(|arm| {
                        matches!(arm, crate::parser::HandlerArm::Operation { name, .. } if name == operation)
                    })
            })
            .map(|(_, handler)| handler)
    }

    fn handler_index(&self, handler: &crate::parser::HandlerDecl) -> Option<usize> {
        self.handler_stack.iter().rposition(|candidate| {
            candidate.name == handler.name && candidate.effect == handler.effect
        })
    }

    // 腕の本体を lower している間、その腕の handler を除外する。腕の中でさらに
    // 別の handler の腕を lower しても外側の腕の除外が消えないよう、積む。
    fn enter_arm(&mut self, handler: &crate::parser::HandlerDecl) -> bool {
        match self.handler_index(handler) {
            Some(index) => {
                self.arm_handlers.push(index);
                true
            }
            None => false,
        }
    }

    fn leave_arm(&mut self, entered: bool) {
        if entered {
            self.arm_handlers.pop();
        }
    }

    fn handler_for_node(&self, node: &crate::parser::Node) -> Option<&crate::parser::HandlerDecl> {
        self.operation_name(node)
            .and_then(|name| self.handler_for(name))
    }

    fn completion(&mut self, node: &crate::parser::Node) -> Result<Expression, String> {
        let value = self.expression(node)?;
        self.finish_handler(value)
    }

    fn specialized_name(&self, name: &str) -> String {
        // 腕を実行中の handler は、その腕から見て外にあるので stack に数えない。
        let handlers = self
            .handler_stack
            .iter()
            .enumerate()
            .filter(|(index, _)| !self.arm_handlers.contains(index))
            .map(|(_, handler)| handler.name.as_str())
            .collect::<Vec<_>>();
        if handlers.is_empty() {
            return name.into();
        }
        format!("{name}#{}", handlers.join("/"))
    }

    fn scheme_carries_effects(scheme: &crate::types::Scheme) -> bool {
        let mut inference = crate::types::Inference::default();
        match inference.instantiate(scheme) {
            Ty::Fn(_, _, row) => row.has_known_effects(),
            _ => false,
        }
    }

    fn is_abort_node(&self, node: &crate::parser::Node) -> bool {
        self.operation_name(node).is_some_and(|name| {
            !self.scope.contains_key(name)
                && self.operations.get(name) == Some(&crate::parser::OpKind::Abort)
        }) || matches!(&node.kind, crate::parser::NodeKind::DoQ { value, .. } if self.is_abort_node(value))
    }

    fn operation_name<'a>(&self, node: &'a crate::parser::Node) -> Option<&'a str> {
        match &node.kind {
            crate::parser::NodeKind::Name(name) => Some(name.as_str()),
            crate::parser::NodeKind::Apply(callee, _)
            | crate::parser::NodeKind::Call(callee, _) => {
                if let crate::parser::NodeKind::Name(name) = &callee.kind {
                    Some(name.as_str())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn is_again_node(&self, node: &crate::parser::Node) -> bool {
        self.operation_name(node).is_some_and(|name| {
            !self.scope.contains_key(name)
                && self.operations.get(name) == Some(&crate::parser::OpKind::Again)
        }) || matches!(&node.kind, crate::parser::NodeKind::DoQ { value, .. } if self.is_again_node(value))
    }

    fn again_answer_of(&self, node: &crate::parser::Node) -> Result<Option<Type>, String> {
        match self.composite_types.get(&(node.span.line, node.span.col)) {
            Some(ty) => Type::try_from(&self.specialization.resolve(ty))
                .map(Some)
                .map_err(|_| "with type is not concrete".into()),
            None => Ok(None),
        }
    }

    fn again_do(
        &mut self,
        value: &crate::parser::Node,
        rest: &crate::parser::Node,
        pat: Option<&crate::parser::Pattern>,
    ) -> Result<Expression, String> {
        if let Some(value) = self.lift_again_arguments(value) {
            let pat = pat.cloned().unwrap_or(crate::parser::Pattern {
                kind: crate::parser::PatternKind::Underscore,
                span: value.span,
            });
            let node = crate::parser::Node {
                span: value.span,
                kind: crate::parser::NodeKind::Let {
                    pat,
                    value: Box::new(value),
                    body: Box::new(rest.clone()),
                },
            };
            return self.expression(&node);
        }
        let name = self
            .operation_name(value)
            .ok_or("again do without operation")?
            .to_string();
        let arguments = match &value.kind {
            crate::parser::NodeKind::Apply(_, args) | crate::parser::NodeKind::Call(_, args) => {
                args.iter()
                    .map(|arg| self.expression(arg))
                    .collect::<Result<Vec<_>, _>>()?
            }
            _ => Vec::new(),
        };
        self.again_operation(value, &name, arguments, Some((rest, pat)))
    }

    fn again_result_type(&self, node: &crate::parser::Node) -> Result<Type, String> {
        use crate::parser::NodeKind;
        let callee = match &node.kind {
            NodeKind::Apply(callee, _) | NodeKind::Call(callee, _) => callee.as_ref(),
            NodeKind::Pipe(_, target) => match &target.kind {
                NodeKind::Apply(callee, _) => callee.as_ref(),
                _ => target.as_ref(),
            },
            _ => node,
        };
        // pipe と左辺は span を共有するので、複合式の型ではなく操作名の型を使う。
        let ty = self
            .value_types
            .get(&(callee.span.line, callee.span.col))
            .ok_or("again operation type missing")?;
        let ty = self.specialization.resolve(ty);
        let result = match &ty {
            Ty::Fn(_, result, _) if !matches!(node.kind, NodeKind::Name(_)) => result.as_ref(),
            _ => &ty,
        };
        Type::try_from(result).map_err(|_| "again result type is not concrete".into())
    }

    fn again_operation(
        &mut self,
        node: &crate::parser::Node,
        name: &str,
        arguments: Vec<Expression>,
        rest: Option<(&crate::parser::Node, Option<&crate::parser::Pattern>)>,
    ) -> Result<Expression, String> {
        let handler = self.handler_for(name).cloned().ok_or("unhandled again")?;
        // 直接式では enclosing の outcome が無いので、その場で最も内側の
        // answer を使う。腕と yield が同じ answer を運び、分岐で揃う。
        let answer = match &self.context_answer {
            Some(answer) => answer.clone(),
            None => match self.again_answers.last() {
                Some(answer) => answer.clone(),
                None => self.answer_for_handler(&handler)?,
            },
        };
        let constructor = self.yield_constructor(name);
        let input = self.again_result_type(node)?;
        let signature = Signature {
            parameters: arguments.iter().map(Expression::ty).collect(),
            result: input.clone(),
        };
        self.operation_types.insert(constructor.clone(), signature);
        let value = arguments
            .into_iter()
            .next()
            .unwrap_or(Expression::Constant(Constant::Void));
        let outcome = Type::Data(OUTCOME.into(), vec![input.clone(), answer, Type::Void]);
        let continuation = match rest {
            Some((body, pat)) => self.make_continuation(body, pat, &outcome)?,
            None => {
                let param = self.next;
                self.next += 1;
                let id = FunctionId(self.next_function);
                self.next_function += 1;
                let body = self.done(
                    Expression::Variable {
                        id: param,
                        ty: input.clone(),
                    },
                    &outcome,
                )?;
                self.continuation_function(
                    id,
                    (param, input),
                    Vec::new(),
                    body.in_tail_position(),
                    &outcome,
                )?
            }
        };
        Ok(Expression::Construct {
            constructor,
            result: match continuation.ty() {
                Type::Closure(_, result) => *result,
                _ => return Err("again continuation is not a closure".into()),
            },
            fields: vec![value, continuation],
        })
    }

    fn make_continuation(
        &mut self,
        body: &crate::parser::Node,
        pat: Option<&crate::parser::Pattern>,
        outcome: &Type,
    ) -> Result<Expression, String> {
        let saved = std::mem::take(&mut self.scope);
        for (name, binding) in &saved {
            if crate::names::references(body, name) {
                self.scope.insert(name.clone(), binding.clone());
            }
        }
        // §38: 外側の `with` の値引数も capture に載せる。継続の本体が呼ぶ
        // 関数の `once` の腕が値引数を参照できるようにするためである。
        let mut captures: Vec<_> = self.scope.values().cloned().collect();
        captures.extend(self.carry_frame_value_args());
        let param = self.next;
        self.next += 1;
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        let input = outcome_parts(outcome)?.0;
        let lowered = match pat {
            Some(pat) => self.bound_body(
                pat,
                Expression::Variable {
                    id: param,
                    ty: input.clone(),
                },
                body,
            ),
            None => self.expression(body),
        };
        self.scope = saved;
        let lowered = lowered?;
        let body = if is_outcome(&lowered.ty()) {
            lowered
        } else {
            let (_, answer, payload) = outcome_parts(outcome)?;
            let result = Type::Data(OUTCOME.into(), vec![lowered.ty(), answer, payload]);
            self.done(lowered, &result)?
        }
        .in_tail_position();
        let result = body.ty();
        if outcome_parts(&result)?.1 != outcome_parts(outcome)?.1
            || outcome_parts(&result)?.2 != outcome_parts(outcome)?.2
        {
            return Err("continuation result type mismatch".into());
        }
        self.continuation_function(id, (param, input), captures, body, &result)
    }

    fn continuation_function(
        &mut self,
        id: FunctionId,
        param: (usize, Type),
        captures: Vec<(usize, Type)>,
        body: Expression,
        answer: &Type,
    ) -> Result<Expression, String> {
        let signature = Signature {
            parameters: vec![param.1],
            result: answer.clone(),
        };
        self.functions.push(Function {
            id,
            name: format!("cont_{}", id.0),
            signature: signature.clone(),
            parameters: vec![param.0],
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
        // §38: `return` の腕も値引数を見る。腕の本体は handler の宣言の中に
        // あり、`with` の本体の束縛は見えない。
        let saved_scope = self.install_handler_frame_values(&handler);
        let result = self.apply_return_arm(&handler, value);
        self.restore_value_params(saved_scope);
        result
    }

    fn take_done(&mut self, value: Expression) -> Result<Expression, String> {
        let ty = value.ty();
        let success = match &ty {
            Type::Data(name, args) if name == OUTCOME => args
                .first()
                .cloned()
                .ok_or("outcome missing success type")?,
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
        let incoming = value.ty();
        let (success, answer, payload) = outcome_parts(&incoming)?;
        // 腕の本体を単相化している間、その handler の枠は外にあるものとして数えない。
        let forwarding = (0..self.handler_stack.len().saturating_sub(1))
            .any(|index| !self.arm_handlers.contains(&index));
        let handler_answer = self.handler_answers.last().cloned().flatten();
        let outgoing = if forwarding {
            Type::Data(
                OUTCOME.into(),
                vec![
                    handler_answer.unwrap_or_else(|| success.clone()),
                    answer,
                    payload.clone(),
                ],
            )
        } else {
            handler_answer.unwrap_or_else(|| success.clone())
        };
        let convert_id = FunctionId(self.next_function);
        self.next_function += 1;
        let convert_param = self.next;
        self.next += 1;
        // §38: このフレームの値引数を convert の追加引数にする。convert は
        // 直接呼ぶ別の関数なので、新しい id を引数に取り、腕を lower する間は
        // その id で値引数を引く。
        let call_arguments = self.frame_value_captures();
        let mut convert_extras = Vec::new();
        let mut convert_frame = Vec::new();
        for (index, values) in self.frame_value_args.iter().enumerate() {
            if self.arm_handlers.contains(&index) {
                convert_frame.push(Vec::new());
                continue;
            }
            let mut carried = Vec::new();
            for (name, (_, ty)) in values {
                let id = self.next;
                self.next += 1;
                convert_extras.push((id, ty.clone()));
                carried.push((name.clone(), (id, ty.clone())));
            }
            convert_frame.push(carried);
        }
        let mut convert_parameters = vec![incoming.clone()];
        convert_parameters.extend(convert_extras.iter().map(|(_, ty)| ty.clone()));
        let convert = ConvertCtx {
            incoming: incoming.clone(),
            outgoing: outgoing.clone(),
            success,
            convert_id,
            convert_sig: Signature {
                parameters: convert_parameters,
                result: outgoing.clone(),
            },
            forwarding,
            value_params: convert_extras.clone(),
        };
        let variable = Expression::Variable {
            id: convert_param,
            ty: incoming,
        };
        let named = convert_frame.last().cloned().unwrap_or_default();
        let saved_scope = self.install_value_params(&named);
        let saved_frame = std::mem::replace(&mut self.frame_value_args, convert_frame);
        let arms = self.convert_arms(handler, &variable, &convert);
        self.frame_value_args = saved_frame;
        self.restore_value_params(saved_scope);
        let arms = arms?;
        let fallback = forwarding.then(|| {
            Box::new(Expression::Unreachable {
                result: outgoing.clone(),
            })
        });
        if arms.iter().any(|(_, body)| body.ty() != outgoing)
            || fallback.as_ref().is_some_and(|body| body.ty() != outgoing)
        {
            return Err("handler arms disagree on the answer type".into());
        }
        let mut parameters = vec![convert_param];
        parameters.extend(convert_extras.iter().map(|(id, _)| *id));
        self.functions.push(Function {
            id: convert_id,
            name: format!("convert_{}", convert_id.0),
            signature: convert.convert_sig.clone(),
            parameters,
            captures: Vec::new(),
            body: Expression::Switch {
                value: Box::new(variable),
                arms,
                fallback,
                result: outgoing,
            }
            .in_tail_position(),
        });
        let mut arguments = vec![value];
        arguments.extend(call_arguments);
        Expression::call_direct(convert_id, &convert.convert_sig, arguments)
    }

    /// 腕を lower する間だけ、handler の値引数の名前を convert の追加引数に
    /// 向け直す。戻すための元の値を返す。
    fn install_value_params(
        &mut self,
        named: &[(String, (usize, Type))],
    ) -> Vec<(String, Option<(usize, Type)>)> {
        let mut saved = Vec::new();
        for (name, (id, ty)) in named {
            saved.push((
                name.clone(),
                self.scope.insert(name.clone(), (*id, ty.clone())),
            ));
        }
        saved
    }

    fn restore_value_params(&mut self, saved: Vec<(String, Option<(usize, Type)>)>) {
        for (name, previous) in saved.into_iter().rev() {
            match previous {
                Some(value) => {
                    self.scope.insert(name, value);
                }
                None => {
                    self.scope.remove(&name);
                }
            }
        }
    }

    // 宣言では型引数を具体化できないため、本体を先に単相化して登録した操作だけを扱う。
    // 未登録タグは本体から到達しないので腕を生成しない。
    fn convert_arms(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        variable: &Expression,
        convert: &ConvertCtx,
    ) -> Result<Vec<(String, Expression)>, String> {
        let done =
            self.apply_return_arm(handler, project(variable, DONE, 0, convert.success.clone()))?;
        let done = self.finish_convert_arm(done, convert)?;
        let mut arms = vec![(DONE.into(), done)];
        for name in handler_operations(handler, crate::parser::OpKind::Abort) {
            let fail = self.fail_constructor(&name);
            if self.operation_types.contains_key(&fail) {
                let body = self.abort_arm(
                    handler,
                    &name,
                    project(variable, &fail, 0, self.outcome_payload(&fail)?),
                )?;
                arms.push((fail, self.finish_convert_arm(body, convert)?));
            }
        }
        for name in handler_operations(handler, crate::parser::OpKind::Again) {
            let constructor = self.yield_constructor(&name);
            if self.operation_types.contains_key(&constructor) {
                arms.push(self.own_yield_arm(handler, &name, variable, convert)?);
            }
        }
        if convert.forwarding {
            arms.extend(self.forward_yield_arms(handler, variable, convert)?);
            arms.extend(self.forward_fail_arms(handler, variable, convert)?);
        }
        Ok(arms)
    }

    fn finish_convert_arm(
        &self,
        value: Expression,
        convert: &ConvertCtx,
    ) -> Result<Expression, String> {
        if convert.forwarding && !is_outcome(&value.ty()) {
            self.done(value, &convert.outgoing)
        } else {
            Ok(value)
        }
    }

    fn own_yield_arm(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        operation: &str,
        variable: &Expression,
        convert: &ConvertCtx,
    ) -> Result<(String, Expression), String> {
        let constructor = self.yield_constructor(operation);
        let yield_value = project(
            variable,
            &constructor,
            0,
            self.outcome_payload(&constructor)?,
        );
        let continuation = project(
            variable,
            &constructor,
            1,
            convert.continuation_ty(
                self.operation_types
                    .get(&constructor)
                    .ok_or("again type missing")?
                    .result
                    .clone(),
            ),
        );
        let resume = self.continuation_calling_convert(continuation, convert)?;
        let body = self.yield_arm(handler, operation, yield_value, resume)?;
        Ok((constructor, self.finish_convert_arm(body, convert)?))
    }

    fn forward_yield_arms(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        variable: &Expression,
        convert: &ConvertCtx,
    ) -> Result<Vec<(String, Expression)>, String> {
        let operations: Vec<_> = self.yield_ids.keys().cloned().collect();
        let mut arms = Vec::new();
        for operation in operations {
            if handler_has_operation(handler, &operation) {
                continue;
            }
            let constructor = self.yield_constructor(&operation);
            if !self.operation_types.contains_key(&constructor) {
                continue;
            }
            let payload_value = project(
                variable,
                &constructor,
                0,
                self.outcome_payload(&constructor)?,
            );
            let continuation = project(
                variable,
                &constructor,
                1,
                convert.continuation_ty(
                    self.operation_types
                        .get(&constructor)
                        .ok_or("again type missing")?
                        .result
                        .clone(),
                ),
            );
            let wrapped = self.continuation_calling_convert(continuation, convert)?;
            arms.push((
                constructor.clone(),
                Expression::Construct {
                    constructor,
                    fields: vec![payload_value, wrapped],
                    result: convert.outgoing.clone(),
                },
            ));
        }
        Ok(arms)
    }

    fn forward_fail_arms(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        variable: &Expression,
        convert: &ConvertCtx,
    ) -> Result<Vec<(String, Expression)>, String> {
        let operations: Vec<_> = self.fail_ids.keys().cloned().collect();
        let mut arms = Vec::new();
        for operation in operations {
            if handler_has_operation(handler, &operation) {
                continue;
            }
            let constructor = self.fail_constructor(&operation);
            if !self.operation_types.contains_key(&constructor) {
                continue;
            }
            let payload_value = project(
                variable,
                &constructor,
                0,
                self.outcome_payload(&constructor)?,
            );
            arms.push((
                constructor.clone(),
                Expression::Construct {
                    constructor,
                    fields: vec![payload_value],
                    result: convert.outgoing.clone(),
                },
            ));
        }
        Ok(arms)
    }

    fn continuation_calling_convert(
        &mut self,
        continuation: Expression,
        convert: &ConvertCtx,
    ) -> Result<Expression, String> {
        let continuation_ty = continuation.ty();
        let Type::Closure(inputs, _) = &continuation_ty else {
            return Err("again continuation is not a closure".into());
        };
        let input = inputs
            .first()
            .cloned()
            .ok_or("again continuation input missing")?;
        let captured = self.next;
        self.next += 1;
        let param = self.next;
        self.next += 1;
        let called = Expression::call_closure(
            Expression::Variable {
                id: captured,
                ty: continuation_ty.clone(),
            },
            vec![Expression::Variable {
                id: param,
                ty: input.clone(),
            }],
        )?;
        let mut arguments = vec![called];
        arguments.extend(
            convert
                .value_params
                .iter()
                .map(|(id, ty)| Expression::Variable {
                    id: *id,
                    ty: ty.clone(),
                }),
        );
        let body = Expression::call_direct(convert.convert_id, &convert.convert_sig, arguments)?;
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        let result = body.ty();
        let mut captures = vec![(captured, continuation_ty)];
        captures.extend(convert.value_params.iter().cloned());
        let closure = self.continuation_function(
            id,
            (param, input),
            captures,
            body.in_tail_position(),
            &result,
        )?;
        Ok(Expression::Let {
            id: captured,
            value: Box::new(continuation),
            body: Box::new(closure),
        })
    }

    fn abort_arm(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        operation: &str,
        payload: Expression,
    ) -> Result<Expression, String> {
        let Some((params, body)) = handler.arms.iter().find_map(|arm| match arm {
            crate::parser::HandlerArm::Operation {
                kind: crate::parser::OpKind::Abort,
                name,
                params,
                body,
                ..
            } if name == operation => Some((params.clone(), body.clone())),
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
            let entered = this.enter_arm(handler);
            let result = this.expression(&body);
            this.rewriting_return = saved;
            this.leave_arm(entered);
            result
        })
    }

    fn yield_arm(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        operation: &str,
        value: Expression,
        resume: Expression,
    ) -> Result<Expression, String> {
        let Some((params, body)) = handler.arms.iter().find_map(|arm| match arm {
            crate::parser::HandlerArm::Operation {
                kind: crate::parser::OpKind::Again,
                name,
                params,
                body,
                ..
            } if name == operation => Some((params.clone(), body.clone())),
            _ => None,
        }) else {
            return Err("missing again arm".into());
        };
        let id = self.next;
        self.next += 1;
        let resume_ty = resume.ty();
        let saved_resume = self.resume_binding.replace((id, resume_ty));
        let result = self.with_params(&params, vec![value], |this| {
            let saved = std::mem::replace(&mut this.rewriting_return, true);
            let entered = this.enter_arm(handler);
            let result = this.expression(&body);
            this.rewriting_return = saved;
            this.leave_arm(entered);
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
        operation: &str,
    ) -> Result<Expression, String> {
        let success = self.node_type(node).unwrap_or(Type::Void);
        self.abort_outcome_with(arguments, operation, success)
    }

    fn abort_outcome_with(
        &mut self,
        arguments: Vec<Expression>,
        operation: &str,
        success: Type,
    ) -> Result<Expression, String> {
        let answer = self
            .context_answer
            .clone()
            .or_else(|| self.again_answers.last().cloned())
            .ok_or("abort without handler answer")?;
        let payload_value = arguments
            .into_iter()
            .next()
            .unwrap_or(Expression::Constant(Constant::Void));
        let constructor = self.fail_constructor(operation);
        self.operation_types.insert(
            constructor.clone(),
            Signature {
                parameters: vec![payload_value.ty()],
                result: Type::Void,
            },
        );
        let outcome = Type::Data(OUTCOME.into(), vec![success, answer, Type::Void]);
        Ok(Expression::Construct {
            constructor,
            fields: vec![payload_value],
            result: outcome,
        })
    }

    fn apply_return_arm(
        &mut self,
        handler: &crate::parser::HandlerDecl,
        value: Expression,
    ) -> Result<Expression, String> {
        let Some(crate::parser::HandlerArm::Return {
            param, body: ret, ..
        }) = handler
            .arms
            .iter()
            .find(|arm| matches!(arm, crate::parser::HandlerArm::Return { .. }))
            .cloned()
        else {
            return Ok(value);
        };
        let saved = std::mem::replace(&mut self.rewriting_return, true);
        let entered = self.enter_arm(handler);
        let result = self.with_params(&[param], vec![value], |this| this.expression(&ret));
        self.rewriting_return = saved;
        self.leave_arm(entered);
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
    ) -> Result<SwitchArms, String> {
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
                **body = self.done(body.as_ref().clone(), &outcome)?;
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

    fn bind_again_outcome(
        &mut self,
        pat: &crate::parser::Pattern,
        value: Expression,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let incoming = value.ty();
        let continuation = self.make_continuation(body, Some(pat), &incoming)?;
        let Type::Closure(_, outgoing) = continuation.ty() else {
            return Err("binding continuation is not a closure".into());
        };
        let signature = Signature {
            parameters: vec![incoming.clone(), continuation.ty()],
            result: *outgoing,
        };
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.again_bind_function(id, &signature)?;
        Expression::call_direct(id, &signature, vec![value, continuation])
    }

    // Yield は continuation を張り替える必要があるので登録済みの操作だけを扱う。
    // Fail は成功型だけが変わるので、未登録の tag も含めて outcome をそのまま載せる。
    fn again_bind_function(&mut self, id: FunctionId, signature: &Signature) -> Result<(), String> {
        // 後面で組むときには、この bind を作った時点の操作の型を使う。同じ効果を
        // 別の型で instantiate すると後勝ちで上書きされるため。
        self.pending_binds
            .push((id, signature.clone(), self.operation_types.clone()));
        Ok(())
    }

    fn flush_pending_binds(&mut self) -> Result<(), String> {
        while !self.pending_binds.is_empty() {
            for (id, signature, operation_types) in std::mem::take(&mut self.pending_binds) {
                self.build_again_bind(id, &signature, &operation_types)?;
            }
        }
        Ok(())
    }

    fn build_again_bind(
        &mut self,
        id: FunctionId,
        signature: &Signature,
        operation_types: &std::collections::HashMap<String, Signature>,
    ) -> Result<(), String> {
        let parameters = vec![self.next, self.next + 1];
        self.next += 2;
        let value = Expression::Variable {
            id: parameters[0],
            ty: signature.parameters[0].clone(),
        };
        let rest = Expression::Variable {
            id: parameters[1],
            ty: signature.parameters[1].clone(),
        };
        let (success, _, _) = outcome_parts(&value.ty())?;
        let mut arms = vec![(
            DONE.into(),
            Expression::call_closure(rest.clone(), vec![project(&value, DONE, 0, success)])?,
        )];
        for operation in self.yield_ids.keys().cloned().collect::<Vec<_>>() {
            let constructor = self.yield_constructor(&operation);
            let Some(operation) = operation_types
                .get(&constructor)
                .or_else(|| self.operation_types.get(&constructor))
            else {
                continue;
            };
            let input = operation.result.clone();
            let payload = operation.parameters.first().cloned().unwrap_or(Type::Void);
            let continuation =
                self.again_bind_yield(&constructor, &input, id, signature, &value, &rest)?;
            arms.push((
                constructor.clone(),
                Expression::Construct {
                    fields: vec![project(&value, &constructor, 0, payload), continuation],
                    constructor,
                    result: signature.result.clone(),
                },
            ));
        }
        let fail_fallback = Expression::Variable {
            id: parameters[0],
            ty: signature.result.clone(),
        };
        self.functions.push(Function {
            id,
            name: format!("bind_{}", id.0),
            signature: signature.clone(),
            parameters,
            captures: Vec::new(),
            body: Expression::Switch {
                value: Box::new(value),
                arms,
                fallback: Some(Box::new(fail_fallback)),
                result: signature.result.clone(),
            }
            .in_tail_position(),
        });
        Ok(())
    }

    fn again_bind_yield(
        &mut self,
        constructor: &str,
        input: &Type,
        bind: FunctionId,
        signature: &Signature,
        value: &Expression,
        rest: &Expression,
    ) -> Result<Expression, String> {
        let input = input.clone();
        let param = self.next;
        self.next += 1;
        let continuation = project(
            value,
            constructor,
            1,
            Type::Closure(vec![input.clone()], Box::new(value.ty())),
        );
        let resumed = Expression::call_closure(
            continuation,
            vec![Expression::Variable {
                id: param,
                ty: input.clone(),
            }],
        )?;
        let body = Expression::call_direct(bind, signature, vec![resumed, rest.clone()])?;
        let captures = [value, rest]
            .into_iter()
            .map(|value| match value {
                Expression::Variable { id, ty } => Ok((*id, ty.clone())),
                _ => Err("again bind requires variables".to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.continuation_function(
            id,
            (param, input),
            captures,
            body.in_tail_position(),
            &signature.result,
        )
    }

    fn propagate_outcome(
        &mut self,
        pat: &crate::parser::Pattern,
        value: Expression,
        body: &crate::parser::Node,
    ) -> Result<Expression, String> {
        let ty = value.ty();
        let (success, answer, payload) = outcome_parts(&ty)?;
        let id = self.next;
        self.next += 1;
        let variable = Expression::Variable { id, ty: ty.clone() };
        let projected = Expression::Project {
            value: Box::new(variable.clone()),
            constructor: DONE.into(),
            index: 0,
            result: success,
        };
        let mut rest = self.bound_body(pat, projected, body)?;
        let outgoing = if is_outcome(&rest.ty()) {
            let (_, rest_answer, rest_payload) = outcome_parts(&rest.ty())?;
            if rest_answer != answer || rest_payload != payload {
                return Err("continuation result type mismatch".into());
            }
            rest.ty()
        } else {
            Type::Data(OUTCOME.into(), vec![rest.ty(), answer, payload])
        };
        if !is_outcome(&rest.ty()) {
            rest = self.done(rest, &outgoing)?;
        }
        let arms = vec![(DONE.into(), rest)];
        // Fail は成功型だけが変わる。登録済み tag に限ると、相互再帰の相手が
        // まだ raise していない時点の switch が Unreachable のまま残る。
        Ok(Expression::Let {
            id,
            value: Box::new(value),
            body: Box::new(Expression::Switch {
                value: Box::new(variable),
                arms,
                fallback: Some(Box::new(Expression::Variable {
                    id,
                    ty: outgoing.clone(),
                })),
                result: outgoing,
            }),
        })
    }

    fn handler_has_propagation_ops(handler: &crate::parser::HandlerDecl) -> bool {
        handler.arms.iter().any(|arm| {
            matches!(
                arm,
                crate::parser::HandlerArm::Operation {
                    kind: crate::parser::OpKind::Abort | crate::parser::OpKind::Again,
                    ..
                }
            )
        })
    }

    fn stack_needs_outcome(&self) -> bool {
        self.handler_stack
            .iter()
            .any(Self::handler_has_propagation_ops)
    }

    fn scheme_carries_effect(scheme: &crate::types::Scheme, effect: &str) -> bool {
        let mut inference = crate::types::Inference::default();
        match inference.instantiate(scheme) {
            crate::types::Ty::Fn(_, _, row) => row.without_effect(effect) != *row,
            _ => false,
        }
    }

    // 高階の引数に効果付きの閉包を渡す場合の答え。効果の名前は scheme の行に
    // 現れない（未確定）ので、腕を実行中の handler を除いた最も内側の
    // propagation handler を使う。同じ効果を捕まえる handler が入れ子のときは
    // 内側を選ぶ（`handler_for` と同じ）。腕の実行中は本体の context の答えを
    // 優先する。
    fn callback_answer(&self) -> Result<Option<Type>, String> {
        for (index, handler) in self.handler_stack.iter().enumerate().rev() {
            if self.arm_handlers.contains(&index) || !Self::handler_has_propagation_ops(handler) {
                continue;
            }
            if let Some(answer) = &self.context_answer {
                return Ok(Some(answer.clone()));
            }
            return self.answer_for_handler(handler).map(Some);
        }
        Ok(None)
    }

    fn specialization_answer(&self, scheme: &crate::types::Scheme) -> Result<Option<Type>, String> {
        for (index, handler) in self.handler_stack.iter().enumerate().rev() {
            // 腕の本体から呼ぶ callee を、その腕の handler では捕まえない。
            if self.arm_handlers.contains(&index) {
                continue;
            }
            if Self::handler_has_propagation_ops(handler)
                && Self::scheme_carries_effect(scheme, &handler.effect)
            {
                if let Some(answer) = &self.context_answer {
                    return Ok(Some(answer.clone()));
                }
                return self.answer_for_handler(handler).map(Some);
            }
        }
        Ok(None)
    }

    fn outcome_of(&self, success: &Type, answer: &Type) -> Result<Type, String> {
        Ok(Type::Data(
            OUTCOME.into(),
            vec![success.clone(), answer.clone(), Type::Void],
        ))
    }

    fn node_type(&self, node: &crate::parser::Node) -> Option<Type> {
        self.composite_types
            .get(&(node.span.line, node.span.col))
            .or_else(|| self.value_types.get(&(node.span.line, node.span.col)))
            .and_then(|ty| Type::try_from(&self.specialization.resolve(ty)).ok())
    }

    fn resume_variable(&self) -> Result<Expression, String> {
        let (id, ty) = self.resume_binding.clone().ok_or("resume without again")?;
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
        let resolved = self.specialization.resolve(ty);
        let Type::Closure(declared_parameters, result) =
            Type::try_from(&resolved).map_err(|_| "lambda type is not concrete")?
        else {
            return Err("expected function type".into());
        };
        // 引数の型に効果行があれば、その閉包も mono では結果に `$Outcome` を載せる。
        // 本体がその引数を呼ぶときに outcome をほどけるよう、実際の型で束縛する。
        let types = match &resolved {
            Ty::Fn(parameters, _, _) => parameters
                .iter()
                .map(|parameter| self.actual_type(parameter))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "lambda type is not concrete")?,
            _ => declared_parameters,
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
        // §38: 外側の `with` の値引数も capture に載せる。本体が呼ぶ関数の
        // `once` の腕が値引数を参照できるようにするためである。
        let mut captures: Vec<_> = self.scope.values().cloned().collect();
        captures.extend(self.carry_frame_value_args());
        let mut parameters = Vec::new();
        for (param, ty) in params.iter().zip(types) {
            let id = self.next;
            self.next += 1;
            if param.name != "_" {
                self.scope.insert(param.name.clone(), (id, ty));
            }
            parameters.push(id);
        }
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        let body = self.expression(body);
        self.scope = saved;
        let body = body?.in_tail_position();
        // 効果を持つ本体は Outcome へ落ちる。handler が解釈するのは呼び出し側なので、
        // 閉包の結果型も Outcome にして呼び出し側の outcome と型を揃える。
        let lowered = body.ty();
        let result = if is_outcome(&lowered) {
            let (success, _, _) = outcome_parts(&lowered)?;
            if success != signature.result {
                return Err("lambda return type mismatch".into());
            }
            lowered
        } else if lowered == signature.result {
            lowered
        } else if logical_argument(&lowered) == signature.result {
            // 効果を持つ閉包を返す lambda。宣言側の型は効果行を落としている。
            lowered
        } else {
            return Err("lambda return type mismatch".into());
        };
        let signature = Signature {
            parameters: signature.parameters.clone(),
            result,
        };
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

    fn structural_equality(
        &mut self,
        left: Expression,
        right: Expression,
    ) -> Result<Expression, String> {
        let ty = left.ty();
        if matches!(ty, Type::Closure(..) | Type::Abstract(..)) {
            return Err("equality needs a concrete non-function type".into());
        }
        let signature = Signature {
            parameters: vec![ty.clone(), ty.clone()],
            result: Type::Bool,
        };
        if let Some((_, id)) = self
            .structural_equalities
            .iter()
            .find(|(cached, _)| cached == &ty)
        {
            return Expression::call_direct(*id, &signature, vec![left, right]);
        }
        if matches!(&ty, Type::Data(name, _) if name == ARRAY) {
            let (id, signature) = self.derive_array_relation(ty, true)?;
            return Expression::call_direct(id, &signature, vec![left, right]);
        }
        self.define_structural_equality(ty, &signature, left, right)
    }

    fn define_structural_equality(
        &mut self,
        ty: Type,
        signature: &Signature,
        left: Expression,
        right: Expression,
    ) -> Result<Expression, String> {
        let fields = match &ty {
            Type::Data(name, _) => self.records.get(name).map(|record| record.0.clone()),
            _ => None,
        };
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.structural_equalities.push((ty.clone(), id));
        let lhs = self.next;
        let rhs = lhs + 1;
        self.next += 2;
        let body = self.equality_body(&ty, fields.as_deref(), lhs, rhs)?;
        self.functions.push(Function {
            id,
            name: format!("eq_{ty:?}"),
            signature: signature.clone(),
            parameters: vec![lhs, rhs],
            captures: vec![],
            body,
        });
        Expression::call_direct(id, signature, vec![left, right])
    }

    fn equality_body(
        &mut self,
        ty: &Type,
        fields: Option<&[String]>,
        lhs: usize,
        rhs: usize,
    ) -> Result<Expression, String> {
        let mut body = Expression::Constant(Constant::Bool(true));
        for field in fields.unwrap_or_default().iter().rev() {
            let a = self.record_get(
                Expression::Variable {
                    id: lhs,
                    ty: ty.clone(),
                },
                field,
            )?;
            let b = self.record_get(
                Expression::Variable {
                    id: rhs,
                    ty: ty.clone(),
                },
                field,
            )?;
            let equal = self.structural_equality(a, b)?;
            body = branch(equal, body, Expression::Constant(Constant::Bool(false)))?;
        }
        if fields.is_none() {
            body = if matches!(ty, Type::Data(..)) {
                self.adt_equality_body(ty, lhs, rhs)?
            } else {
                Expression::Binary {
                    op: crate::parser::BinOp::Eq,
                    left: Box::new(Expression::Variable {
                        id: lhs,
                        ty: ty.clone(),
                    }),
                    right: Box::new(Expression::Variable {
                        id: rhs,
                        ty: ty.clone(),
                    }),
                }
            };
        }
        Ok(body)
    }

    fn adt_equality_body(
        &mut self,
        ty: &Type,
        lhs: usize,
        rhs: usize,
    ) -> Result<Expression, String> {
        let mut names: Vec<_> = self.constructors.keys().cloned().collect();
        names.sort();
        let mut arms = Vec::new();
        for name in names {
            let Ok(fields) = self.constructor_fields(&name, ty) else {
                continue;
            };
            let mut body = Expression::Constant(Constant::Bool(true));
            for (index, field_type) in fields.iter().enumerate().rev() {
                let project = |id| Expression::Project {
                    value: Box::new(Expression::Variable { id, ty: ty.clone() }),
                    constructor: name.clone(),
                    index,
                    result: field_type.clone(),
                };
                let a = project(lhs);
                let b = project(rhs);
                let equal = self.structural_equality(a, b)?;
                body = branch(equal, body, Expression::Constant(Constant::Bool(false)))?;
            }
            arms.push((
                name.clone(),
                Expression::Switch {
                    value: Box::new(Expression::Variable {
                        id: rhs,
                        ty: ty.clone(),
                    }),
                    arms: vec![(name, body)],
                    fallback: Some(Box::new(Expression::Constant(Constant::Bool(false)))),
                    result: Type::Bool,
                },
            ));
        }
        if arms.is_empty() {
            return Err("unknown ADT in equality derivation".into());
        }
        Ok(Expression::Switch {
            value: Box::new(Expression::Variable {
                id: lhs,
                ty: ty.clone(),
            }),
            arms,
            fallback: None,
            result: Type::Bool,
        })
    }

    fn derive_comparison(&mut self, ty: Type) -> Result<(FunctionId, Signature), String> {
        if matches!(
            ty,
            Type::Int
                | Type::Float
                | Type::Bool
                | Type::Char
                | Type::Void
                | Type::String
                | Type::Bytes
        ) {
            return Ok(self.derive_scalar_compare(ty));
        }
        if matches!(&ty, Type::Data(name, _) if name == ARRAY) {
            return self.derive_array_relation(ty, false);
        }
        let Type::Data(name, _) = &ty else {
            return Err("compare derivation is not implemented for this type".into());
        };
        let fields = self.records.get(name).map(|record| record.0.clone());
        let signature = Signature {
            parameters: vec![ty.clone(), ty.clone()],
            result: Type::Int,
        };
        if let Some((_, id)) = self
            .scalar_comparisons
            .iter()
            .find(|(cached, _)| cached == &ty)
        {
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
            let a = self.record_get(
                Expression::Variable {
                    id: lhs,
                    ty: ty.clone(),
                },
                field,
            )?;
            let b = self.record_get(
                Expression::Variable {
                    id: rhs,
                    ty: ty.clone(),
                },
                field,
            )?;
            let (callee, callee_signature) = self.derive_comparison(a.ty())?;
            let value = Expression::call_direct(callee, &callee_signature, vec![a, b])?;
            let temporary = self.next;
            self.next += 1;
            let order = Expression::Variable {
                id: temporary,
                ty: Type::Int,
            };
            let condition = Expression::Binary {
                op: crate::parser::BinOp::Eq,
                left: Box::new(order.clone()),
                right: Box::new(Expression::Constant(Constant::Int(0))),
            };
            body = Expression::Let {
                id: temporary,
                value: Box::new(value),
                body: Box::new(branch(condition, body, order)?),
            };
        }
        if fields.is_none() {
            body = self.adt_comparison_body(&ty, lhs, rhs)?;
        }
        self.functions.push(Function {
            id,
            name: format!("compare_{ty:?}"),
            signature: signature.clone(),
            parameters: vec![lhs, rhs],
            captures: vec![],
            body,
        });
        Ok((id, signature))
    }

    fn derive_array_relation(
        &mut self,
        ty: Type,
        equality: bool,
    ) -> Result<(FunctionId, Signature), String> {
        let signature = Signature {
            parameters: vec![ty.clone(), ty.clone()],
            result: if equality { Type::Bool } else { Type::Int },
        };
        let cache = if equality {
            &mut self.structural_equalities
        } else {
            &mut self.scalar_comparisons
        };
        if let Some((_, id)) = cache.iter().find(|(cached, _)| cached == &ty) {
            return Ok((*id, signature));
        }
        let id = FunctionId(self.next_function);
        self.next_function += 2;
        cache.push((ty.clone(), id));
        let parameters = [self.next, self.next + 1, self.next + 2];
        self.next += parameters.len();
        let arguments = [
            Expression::Variable {
                id: parameters[0],
                ty: ty.clone(),
            },
            Expression::Variable {
                id: parameters[1],
                ty,
            },
            Expression::Variable {
                id: parameters[2],
                ty: Type::Int,
            },
        ];
        let helper = Signature {
            parameters: arguments.iter().map(Expression::ty).collect(),
            result: signature.result.clone(),
        };
        let body = self.array_relation_body(&arguments, FunctionId(id.0 + 1), &helper, equality)?;
        self.functions.push(Function {
            id: FunctionId(id.0 + 1),
            name: format!("array_relation_items_{id:?}"),
            signature: helper.clone(),
            parameters: parameters.to_vec(),
            captures: vec![],
            body: body.in_tail_position(),
        });
        let body = Expression::call_direct(
            FunctionId(id.0 + 1),
            &helper,
            vec![
                arguments[0].clone(),
                arguments[1].clone(),
                Expression::Constant(Constant::Int(0)),
            ],
        )?;
        self.functions.push(Function {
            id,
            name: format!("array_relation_{id:?}"),
            signature: signature.clone(),
            parameters: parameters[..2].to_vec(),
            captures: vec![],
            body,
        });
        Ok((id, signature))
    }

    fn array_relation_body(
        &mut self,
        arguments: &[Expression; 3],
        id: FunctionId,
        signature: &Signature,
        equality: bool,
    ) -> Result<Expression, String> {
        use crate::{parser::BinOp, runtime_abi::RuntimeFunction};
        let at_end = |array: &Expression| -> Result<Expression, String> {
            Ok(Expression::Binary {
                op: BinOp::Eq,
                left: Box::new(arguments[2].clone()),
                right: Box::new(Expression::call_array(
                    RuntimeFunction::ArrayLength,
                    vec![array.clone()],
                    None,
                )?),
            })
        };
        let next = Expression::Binary {
            op: BinOp::Add,
            left: Box::new(arguments[2].clone()),
            right: Box::new(Expression::Constant(Constant::Int(1))),
        };
        let remaining = Expression::call_direct(
            id,
            signature,
            vec![arguments[0].clone(), arguments[1].clone(), next],
        )?;
        let items = self.array_relation_items(arguments, remaining, equality)?;
        if equality {
            return branch(
                at_end(&arguments[0])?,
                at_end(&arguments[1])?,
                branch(
                    at_end(&arguments[1])?,
                    Expression::Constant(Constant::Bool(false)),
                    items,
                )?,
            );
        }
        let left_ended = branch(
            at_end(&arguments[1])?,
            Expression::Constant(Constant::Int(0)),
            Expression::Constant(Constant::Int(-1)),
        )?;
        branch(
            at_end(&arguments[0])?,
            left_ended,
            branch(
                at_end(&arguments[1])?,
                Expression::Constant(Constant::Int(1)),
                items,
            )?,
        )
    }

    fn array_relation_items(
        &mut self,
        arguments: &[Expression; 3],
        remaining: Expression,
        equality: bool,
    ) -> Result<Expression, String> {
        use crate::{parser::BinOp, runtime_abi::RuntimeFunction};
        let left = Expression::call_array(
            RuntimeFunction::ArrayGet,
            vec![arguments[0].clone(), arguments[2].clone()],
            None,
        )?;
        let right = Expression::call_array(
            RuntimeFunction::ArrayGet,
            vec![arguments[1].clone(), arguments[2].clone()],
            None,
        )?;
        if equality {
            return branch(
                self.structural_equality(left, right)?,
                remaining,
                Expression::Constant(Constant::Bool(false)),
            );
        }
        let (callee, signature) = self.derive_comparison(left.ty())?;
        let value = Expression::call_direct(callee, &signature, vec![left, right])?;
        let temporary = self.next;
        self.next += 1;
        let order = Expression::Variable {
            id: temporary,
            ty: Type::Int,
        };
        let condition = Expression::Binary {
            op: BinOp::Eq,
            left: Box::new(order.clone()),
            right: Box::new(Expression::Constant(Constant::Int(0))),
        };
        Ok(Expression::Let {
            id: temporary,
            value: Box::new(value),
            body: Box::new(branch(condition, remaining, order)?),
        })
    }

    fn adt_comparison_body(
        &mut self,
        ty: &Type,
        lhs: usize,
        rhs: usize,
    ) -> Result<Expression, String> {
        let mut variants: Vec<_> = self
            .constructors
            .keys()
            .filter_map(|name| {
                self.constructor_fields(name, ty)
                    .ok()
                    .map(|fields| (name.clone(), fields))
            })
            .collect();
        variants.sort_by_key(|(name, _)| self.constructor_tags[name]);
        let mut arms = Vec::new();
        for (left_index, (name, fields)) in variants.iter().enumerate() {
            let mut right_arms = Vec::new();
            for (right_index, (other, _)) in variants.iter().enumerate() {
                let mut body = Expression::Constant(Constant::Int(if left_index < right_index {
                    -1
                } else if left_index > right_index {
                    1
                } else {
                    0
                }));
                if left_index == right_index {
                    for (index, field) in fields.iter().enumerate().rev() {
                        let project = |id| Expression::Project {
                            value: Box::new(Expression::Variable { id, ty: ty.clone() }),
                            constructor: name.clone(),
                            index,
                            result: field.clone(),
                        };
                        let (callee, signature) = self.derive_comparison(field.clone())?;
                        let value = Expression::call_direct(
                            callee,
                            &signature,
                            vec![project(lhs), project(rhs)],
                        )?;
                        let temporary = self.next;
                        self.next += 1;
                        let order = Expression::Variable {
                            id: temporary,
                            ty: Type::Int,
                        };
                        let condition = Expression::Binary {
                            op: crate::parser::BinOp::Eq,
                            left: Box::new(order.clone()),
                            right: Box::new(Expression::Constant(Constant::Int(0))),
                        };
                        body = Expression::Let {
                            id: temporary,
                            value: Box::new(value),
                            body: Box::new(branch(condition, body, order)?),
                        };
                    }
                }
                right_arms.push((other.clone(), body));
            }
            arms.push((
                name.clone(),
                Expression::Switch {
                    value: Box::new(Expression::Variable {
                        id: rhs,
                        ty: ty.clone(),
                    }),
                    arms: right_arms,
                    fallback: None,
                    result: Type::Int,
                },
            ));
        }
        if arms.is_empty() {
            return Err("unknown ADT in comparison derivation".into());
        }
        Ok(Expression::Switch {
            value: Box::new(Expression::Variable {
                id: lhs,
                ty: ty.clone(),
            }),
            arms,
            fallback: None,
            result: Type::Int,
        })
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
                left: Box::new(Expression::Variable {
                    id,
                    ty: Type::Float,
                }),
                right: Box::new(Expression::Variable {
                    id,
                    ty: Type::Float,
                }),
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
        } else {
            body
        };
        self.functions.push(Function {
            id,
            name: format!("compare_{ty:?}"),
            signature: signature.clone(),
            parameters: vec![left, right],
            captures: vec![],
            body: if ty == Type::Void {
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
            if let Some(module) = ordinary_module_impl(item) {
                self.collect_module(module, prefix);
            }
        }
    }

    fn collect_module(&mut self, module: &crate::parser::ModuleImpl, prefix: &str) {
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

    fn resolve_constructor(&self, name: &str) -> String {
        let mut prefix = self
            .active
            .last()
            .and_then(|(current, _, _)| current.rsplit_once('.').map(|(prefix, _)| prefix));
        while let Some(module) = prefix {
            let qualified = format!("{module}.{name}");
            if self.constructors.contains_key(&qualified) {
                return qualified;
            }
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
        matches!(
            self.definitions
                .get(name)
                .map(|definition| &definition.kind),
            Some(crate::parser::DefKind::Value(_))
        )
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
            return self.specialized_call(function, &signature, vec![]);
        }
        let Type::Closure(parameters, result) = ty else {
            return Err("function definition requires a function type".into());
        };
        let (function, signature) = self.specialize_function(name, parameters, Some(&result))?;
        Ok(Expression::MakeClosure {
            captures: self.frame_value_captures(),
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
        if let Some(comparison) = self.comparison_specialization(name, &types)? {
            return Ok(comparison);
        }
        let (params, body, scheme) = self.function_parts(name)?;
        if params.len() != types.len() {
            return Err("function arity mismatch".into());
        }
        let outcome_answer = self.outcome_answer(&scheme, &types)?;
        let outcome = outcome_answer.is_some();
        // 期待する結果型を渡す単一化用に、source 側の結果型（outcome を外した形）
        // を用意する。引数から結果型が決まらない形（契約の member が返す閉包の
        // 引数など）はこちらから取る。期待する結果型も本体が実際の型で下ろれて
        // いるので、論理型に戻してから単一化する。
        let logical: Vec<Type> = types.iter().map(logical_argument).collect();
        let inferred = Signature::specialize_result(
            &scheme,
            &logical,
            expected_result
                .filter(|_| !outcome)
                .map(logical_argument)
                .as_ref(),
        )?;
        // 本体も引数の実際の型で下ろす。scheme が関数型を書いた位置は、本体が
        // outcome をほどいて伝播させるので、単一化にはその結果型だけ `$Outcome`
        // を外した成功型を使う（`mixed_argument`）。引数の型とデータの型引数は
        // 実際の型のまま残す。全体を論理型で下ろすと、引数から来た値（実際の型）
        // と本体が組み立てる値（論理型）が食い違う。
        let mut probe = crate::types::Inference::default();
        let scheme_parameters = match probe.instantiate(&scheme) {
            Ty::Fn(parameters, _, _) => parameters,
            _ => Vec::new(),
        };
        let body_parameters: Vec<Type> = types
            .iter()
            .enumerate()
            .map(|(index, actual)| match scheme_parameters.get(index) {
                Some(parameter) => mixed_argument(parameter, actual),
                None => actual.clone(),
            })
            .collect();
        // 結果型は実際の型での単一化から取る。scheme が結果型を書いている
        // 位置（`Array.of_list` の `A Array`、`Array.get` の `A`）は、型変数に
        // 実際の型を束縛すれば outcome を保ったまま解ける。構造で結果型を書き換え
        // ると、`def replace(f: Fn(Int => Int {Error})): Fn(Int => Int) => do f 1
        // -> fn(x => (x + 1))` のように scheme が結果型そのものを書いている位置
        // まで引数の閉包で書き換えてしまう（戻り値の実体は別の閉包である）。
        //
        // 引数の実際の型だけで結果型が決まるならそれを先に使う。決まらない結果
        // （契約の member が返す閉包の引数など）だけ、呼び出し位置の期待する
        // 結果型から解く。期待は効果を `$Outcome` で持つので、scheme の効果行に
        // 合わせて外してから渡す。
        let body_result = match Signature::specialize_result(&scheme, &body_parameters, None) {
            Ok(actual) => actual.result,
            Err(_) => match Signature::specialize_result(
                &scheme,
                &body_parameters,
                expected_result.map(scheme_expected).as_ref(),
            ) {
                Ok(actual) => actual.result,
                Err(_) => inferred.result.clone(),
            },
        };
        let signature = self.outcome_signature(
            &types,
            &Signature {
                parameters: types.clone(),
                result: body_result.clone(),
            },
            outcome_answer.as_ref(),
        )?;
        let key = self.specialized_name(name);
        if let Some(function) = self.cached_specialization(&key, &types, &signature.result) {
            return Ok(function);
        }
        if let Some(function) = self.recursive_specialization(&key, &types, &signature.result)? {
            return Ok(function);
        }
        self.lower_specialization(SpecializationPlan {
            name: name.to_owned(),
            key,
            params,
            body,
            scheme,
            types,
            body_parameters,
            body_result,
            outcome_answer,
            signature,
        })
    }

    fn comparison_specialization(
        &mut self,
        name: &str,
        types: &[Type],
    ) -> Result<Option<(FunctionId, Signature)>, String> {
        if name != "compare" || self.definitions.contains_key(name) {
            return Ok(None);
        }
        if types.len() != 2 || types[0] != types[1] {
            return Err("compare requires two values of the same type".into());
        }
        self.derive_comparison(types[0].clone()).map(Some)
    }

    fn function_parts(
        &self,
        name: &str,
    ) -> Result<
        (
            Vec<crate::parser::Param>,
            crate::parser::Node,
            crate::types::Scheme,
        ),
        String,
    > {
        let definition = self
            .definitions
            .get(name)
            .ok_or_else(|| format!("unknown function {name}"))?
            .clone();
        let scheme = self
            .schemes
            .get(name)
            .ok_or_else(|| format!("missing inferred signature for {name}"))?
            .clone();
        Ok(match definition.kind {
            crate::parser::DefKind::Function { params, body, .. } => (params, body, scheme),
            crate::parser::DefKind::Value(body) => {
                let scheme = match scheme.body() {
                    Ty::Fn(args, _, _) if args.is_empty() => scheme,
                    _ => scheme.as_thunk(),
                };
                (vec![], body, scheme)
            }
        })
    }

    fn outcome_answer(
        &self,
        scheme: &crate::types::Scheme,
        types: &[Type],
    ) -> Result<Option<Type>, String> {
        // 効果を持つ関数値を引数に取る callee は、その効果を自分の効果行に載せる。
        // scheme の行が未確定（省略された効果行）でも、渡された閉包が outcome を
        // 返すなら handler の答えを使う。
        let callback = if types
            .iter()
            .any(|ty| matches!(ty, Type::Closure(_, result) if is_outcome(result)))
        {
            self.callback_answer()?
        } else {
            None
        };
        Ok(self.specialization_answer(scheme)?.or(callback))
    }

    fn outcome_signature(
        &self,
        types: &[Type],
        inferred: &Signature,
        answer: Option<&Type>,
    ) -> Result<Signature, String> {
        let result = match answer {
            Some(answer) => self.outcome_of(&inferred.result, answer)?,
            None => inferred.result.clone(),
        };
        Ok(Signature {
            parameters: types.to_vec(),
            result,
        })
    }

    fn cached_specialization(
        &self,
        key: &str,
        types: &[Type],
        result: &Type,
    ) -> Option<(FunctionId, Signature)> {
        self.functions
            .iter()
            .find(|function| {
                function.name == key
                    && function.signature.parameters == types
                    && function.signature.result == *result
            })
            .map(|function| (function.id, function.signature.clone()))
    }

    fn recursive_specialization(
        &self,
        key: &str,
        types: &[Type],
        result: &Type,
    ) -> Result<Option<(FunctionId, Signature)>, String> {
        let Some((_, id, active)) = self
            .active
            .iter()
            .find(|(active, _, _)| active.as_str() == key)
        else {
            return Ok(None);
        };
        if active.parameters != types || active.result != *result {
            return Err("recursive specialization changes argument types".into());
        }
        Ok(Some((*id, active.clone())))
    }

    fn lower_specialization(
        &mut self,
        plan: SpecializationPlan,
    ) -> Result<(FunctionId, Signature), String> {
        let SpecializationPlan {
            name,
            key,
            params,
            body,
            scheme,
            types,
            body_parameters,
            body_result,
            outcome_answer,
            mut signature,
        } = plan;
        let mut specialization = crate::types::Inference::default();
        let concrete_parameters: Vec<Ty> = body_parameters
            .iter()
            .map(|ty| ty_with_fresh_rows(ty, &mut specialization))
            .collect();
        let result = ty_with_fresh_rows(&body_result, &mut specialization);
        let concrete = Ty::fn_with(concrete_parameters, result, specialization.fresh_row());
        specialization
            .unify(scheme.body(), &concrete)
            .map_err(|error| format!("specialization failed: {error:?}"))?;
        let saved_specialization = std::mem::replace(&mut self.specialization, specialization);
        let saved = std::mem::take(&mut self.scope);
        let parameters = self.bind_parameters(&params, &types);
        let id = FunctionId(self.next_function);
        self.next_function += 1;
        self.active.push((key.clone(), id, signature.clone()));
        let saved_answer = std::mem::replace(&mut self.context_answer, outcome_answer.clone());
        // §38: 外側の `with` の値引数を capture として引き継ぐ。呼んだ先の
        // `once` の腕が値引数を参照できるようにするためである。値引数が
        // 無ければ capture も無く、今までどおりの関数になる。
        let captures = self.carry_frame_value_args();
        let lowered = self.expression(&body);
        self.context_answer = saved_answer;
        self.active.pop();
        self.specialization = saved_specialization;
        self.scope = saved;
        let body = self.reconcile_outcome(lowered?.in_tail_position(), &mut signature, &name)?;
        self.functions.push(Function {
            captures,
            id,
            name: key,
            signature: signature.clone(),
            parameters,
            body,
        });
        Ok((id, signature))
    }

    fn bind_parameters(&mut self, params: &[crate::parser::Param], types: &[Type]) -> Vec<usize> {
        let mut parameters = Vec::new();
        for (param, ty) in params.iter().zip(types) {
            let id = self.next;
            self.next += 1;
            if param.name != "_" {
                self.scope.insert(param.name.clone(), (id, ty.clone()));
            }
            parameters.push(id);
        }
        parameters
    }

    fn reconcile_outcome(
        &mut self,
        body: Expression,
        signature: &mut Signature,
        name: &str,
    ) -> Result<Expression, String> {
        let mut body = body;
        // callback を渡されただけで本体が呼ばない callee は、その効果を運ばない。
        // 引数の型から outcome と決めていた分をここで戻す。
        if !is_outcome(&body.ty()) && is_outcome(&signature.result) {
            signature.result = body.ty();
        }
        if is_outcome(&signature.result) && !is_outcome(&body.ty()) {
            body = self.done(body, &signature.result)?;
        }
        if body.ty() != signature.result {
            return Err(format!(
                "lowered return type differs from inferred signature for {name}"
            ));
        }
        Ok(body)
    }
}

// 単相化の準備で決まる文脈。lowering の間に参照し、署名だけを更新する。
struct SpecializationPlan {
    name: String,
    key: String,
    params: Vec<crate::parser::Param>,
    body: crate::parser::Node,
    scheme: crate::types::Scheme,
    types: Vec<Type>,
    body_parameters: Vec<Type>,
    body_result: Type,
    outcome_answer: Option<Type>,
    signature: Signature,
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

// 通常の module 実装だけを集める。呼び出し側が種別ごとの分岐を持たずに済む。
fn ordinary_module_impl(item: &crate::parser::Item) -> Option<&crate::parser::ModuleImpl> {
    let crate::parser::Item::ModuleImpl(module) = item else {
        return None;
    };
    matches!(module.form, crate::parser::ImplementationForm::Ordinary).then_some(module)
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
            .direct_call("f", vec![Expression::Constant(Constant::Int(3))], None)
            .unwrap();
        assert_eq!(call.ty(), Type::Bool);
        assert!(matches!(&call, Expression::CallClosure { closure, .. }
            if matches!(closure.as_ref(), Expression::Variable { id: 7, .. })));
        assert!(matches!(
            call.in_tail_position(),
            Expression::TailCallClosure { .. }
        ));
        assert!(lowerer.direct_call("f", vec![], None).is_err());
        assert!(lowerer
            .direct_call("f", vec![Expression::Constant(Constant::Bool(true))], None)
            .is_err());
    }

    #[test]
    fn fail_tags_are_dense_and_injective() {
        let mut lowerer = Lowerer::default();
        let mut tags = std::collections::BTreeSet::new();
        for index in 0..16u32 {
            let effect = format!("Eff{index}");
            let constructor = lowerer.fail_constructor(&effect);
            let tag = outcome_tag(&constructor).unwrap();
            assert_eq!(constructor, format!("$Fail#{tag}"));
            assert_ne!(tag, 0);
            assert_eq!(lowerer.fail_tag_for(&effect), tag);
            assert!(tags.insert(tag), "assigned tag collided at {index}");
        }
        for index in 0..16u32 {
            let effect = format!("Cap{index}");
            let constructor = lowerer.yield_constructor(&effect);
            let tag = outcome_tag(&constructor).unwrap();
            assert_eq!(constructor, format!("$Yield#{tag}"));
            assert_ne!(tag, 0);
            assert!(tags.insert(tag), "yield tag collided with fail at {index}");
        }
        assert_eq!(tags.len(), 32);
        assert_eq!(
            tags.iter().copied().collect::<Vec<_>>(),
            (1..=32).collect::<Vec<_>>()
        );
    }

    #[test]
    fn outcome_tags_are_per_operation_not_per_effect() {
        let mut lowerer = Lowerer::default();
        let ping = lowerer.yield_constructor("ping");
        let pong = lowerer.yield_constructor("pong");
        assert_ne!(ping, pong);
        assert_eq!(lowerer.yield_constructor("ping"), ping);
        let miss = lowerer.fail_constructor("miss");
        let bad = lowerer.fail_constructor("bad");
        assert_ne!(miss, bad);
        assert_eq!(lowerer.fail_constructor("miss"), miss);
    }

    #[test]
    fn computed_callees_lower_as_closure_calls() {
        for source in [
            "def main = (f as chosen -> chosen) 3",
            "def main = 3 |> (f as chosen -> chosen)",
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
            .direct_call("f", vec![Expression::Constant(Constant::Int(1))], None)
            .is_err());
    }
}

#[test]
fn runtime_call_in_tail_position_stays_external() {
    let call = Expression::call_runtime(
        crate::runtime_abi::RuntimeFunction::StringLength,
        vec![Expression::Constant(Constant::String("a".into()))],
    )
    .unwrap();
    assert!(matches!(
        call.in_tail_position(),
        Expression::CallRuntime { .. }
    ));
}

pub fn derive_int_printer(id: FunctionId) -> Result<Function, String> {
    derive_scalar_printer(id, Type::Int)
}

pub fn derive_scalar_printer(id: FunctionId, ty: Type) -> Result<Function, String> {
    use crate::runtime_abi::RuntimeFunction;
    let value = Expression::Variable {
        id: 0,
        ty: ty.clone(),
    };
    let text = |s: &str| Expression::Constant(Constant::String(s.into()));
    let (runtime, argument) = match ty {
        Type::Int => (RuntimeFunction::PrintInt, value),
        Type::String => (RuntimeFunction::PrintString, value),
        Type::Bytes => (
            RuntimeFunction::PrintString,
            Expression::call_runtime(RuntimeFunction::BytesDisplay, vec![value])?,
        ),
        Type::Char => (RuntimeFunction::PrintChar, value),
        Type::Float => (
            RuntimeFunction::PrintString,
            Expression::call_runtime(RuntimeFunction::CanonicalFloat, vec![value])?,
        ),
        Type::Bool => (
            RuntimeFunction::PrintString,
            branch(value, text("true"), text("false"))?,
        ),
        Type::Void => {
            return Ok(Function {
                id,
                name: format!("print_{ty:?}"),
                signature: Signature {
                    parameters: vec![ty],
                    result: Type::Void,
                },
                parameters: vec![0],
                captures: vec![],
                body: Expression::Constant(Constant::Void),
            })
        }
        _ => return Err("scalar printer is not implemented".into()),
    };
    Ok(Function {
        id,
        name: format!("print_{ty:?}"),
        signature: Signature {
            parameters: vec![ty.clone()],
            result: Type::Void,
        },
        parameters: vec![0],
        captures: vec![],
        body: Expression::call_runtime(runtime, vec![argument])?,
    })
}

pub fn derive_printers(
    id: FunctionId,
    ty: Type,
    checked: &crate::Checked,
) -> Result<Vec<Function>, String> {
    let checked = &checked.for_codegen();
    let mut context = DisplayContext {
        checked,
        next: id.0 + 1,
        cache: Vec::new(),
        functions: Vec::new(),
    };
    let printer = derive_printer(id, ty, &mut context)?;
    context.functions.insert(0, printer);
    Ok(context.functions)
}

fn derive_printer(
    id: FunctionId,
    ty: Type,
    context: &mut DisplayContext<'_>,
) -> Result<Function, String> {
    if !matches!(ty, Type::Data(..)) {
        return derive_scalar_printer(id, ty);
    }
    let value = context.format(Expression::Variable {
        id: 0,
        ty: ty.clone(),
    })?;
    Ok(Function {
        id,
        name: format!("print_{ty:?}"),
        signature: Signature {
            parameters: vec![ty],
            result: Type::Void,
        },
        parameters: vec![0],
        captures: vec![],
        body: Expression::call_runtime(
            crate::runtime_abi::RuntimeFunction::PrintString,
            vec![value],
        )?,
    })
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
        if !matches!(ty, Type::Data(..)) {
            return display_value(value, self);
        }
        let signature = Signature {
            parameters: vec![ty.clone()],
            result: Type::String,
        };
        if let Some((_, id)) = self.cache.iter().find(|(cached, _)| cached == &ty) {
            return Expression::call_direct(*id, &signature, vec![value]);
        }
        let id = FunctionId(self.next);
        self.next += 1;
        self.cache.push((ty.clone(), id));
        let body = display_value(
            Expression::Variable {
                id: 0,
                ty: ty.clone(),
            },
            self,
        )?;
        self.functions.push(Function {
            id,
            name: format!("format_{ty:?}"),
            signature: signature.clone(),
            parameters: vec![0],
            captures: vec![],
            body,
        });
        Expression::call_direct(id, &signature, vec![value])
    }
}

fn display_list(
    value: Expression,
    element: Type,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    let id = FunctionId(context.next);
    context.next += 1;
    let signature = Signature {
        parameters: vec![value.ty()],
        result: Type::String,
    };
    let parameter = Expression::Variable {
        id: 0,
        ty: value.ty(),
    };
    let body = display_list_items(parameter, element, id, &signature, context)?;
    context.functions.push(Function {
        id,
        name: format!("format_list_items_{id:?}"),
        signature: signature.clone(),
        parameters: vec![0],
        captures: vec![],
        body,
    });
    let items = Expression::call_direct(id, &signature, vec![value])?;
    concat_display(display_text("["), concat_display(items, display_text("]"))?)
}

fn display_array(
    value: Expression,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    let id = FunctionId(context.next);
    context.next += 1;
    let signature = Signature {
        parameters: vec![value.ty(), Type::Int],
        result: Type::String,
    };
    let parameter = Expression::Variable {
        id: 0,
        ty: value.ty(),
    };
    let body = display_array_items(parameter, id, &signature, context)?;
    context.functions.push(Function {
        id,
        name: format!("format_array_items_{id:?}"),
        signature: signature.clone(),
        parameters: vec![0, 1],
        captures: vec![],
        body,
    });
    let items = Expression::call_direct(
        id,
        &signature,
        vec![value, Expression::Constant(Constant::Int(0))],
    )?;
    concat_display(
        display_text("Array.of_list ["),
        concat_display(items, display_text("]"))?,
    )
}

fn display_array_items(
    value: Expression,
    id: FunctionId,
    signature: &Signature,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    use crate::{parser::BinOp, runtime_abi::RuntimeFunction};
    let index = Expression::Variable {
        id: 1,
        ty: Type::Int,
    };
    let length = Expression::call_array(RuntimeFunction::ArrayLength, vec![value.clone()], None)?;
    let at_end = Expression::Binary {
        op: BinOp::Eq,
        left: Box::new(index.clone()),
        right: Box::new(length.clone()),
    };
    let head = context.format(Expression::call_array(
        RuntimeFunction::ArrayGet,
        vec![value.clone(), index.clone()],
        None,
    )?)?;
    let next = Expression::Binary {
        op: BinOp::Add,
        left: Box::new(index),
        right: Box::new(Expression::Constant(Constant::Int(1))),
    };
    let last = Expression::Binary {
        op: BinOp::Eq,
        left: Box::new(next.clone()),
        right: Box::new(length),
    };
    let remaining = Expression::call_direct(id, signature, vec![value, next])?;
    let suffix = branch(
        last,
        display_text(""),
        concat_display(display_text(", "), remaining)?,
    )?;
    branch(at_end, display_text(""), concat_display(head, suffix)?)
}

fn display_text(text: &str) -> Expression {
    Expression::Constant(Constant::String(text.into()))
}

fn concat_display(left: Expression, right: Expression) -> Result<Expression, String> {
    Expression::call_runtime(
        crate::runtime_abi::RuntimeFunction::StringConcat,
        vec![left, right],
    )
}

fn display_list_items(
    value: Expression,
    element: Type,
    id: FunctionId,
    signature: &Signature,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    let head = context.format(Expression::Project {
        value: Box::new(value.clone()),
        constructor: "Cons".into(),
        index: 0,
        result: element,
    })?;
    let tail = Expression::Project {
        value: Box::new(value.clone()),
        constructor: "Cons".into(),
        index: 1,
        result: value.ty(),
    };
    let remaining = Expression::call_direct(id, signature, vec![tail.clone()])?;
    let suffix = Expression::Switch {
        value: Box::new(tail),
        arms: vec![
            ("Nil".into(), display_text("")),
            (
                "Cons".into(),
                concat_display(display_text(", "), remaining)?,
            ),
        ],
        fallback: None,
        result: Type::String,
    };
    Ok(Expression::Switch {
        value: Box::new(value),
        arms: vec![
            ("Nil".into(), display_text("")),
            ("Cons".into(), concat_display(head, suffix)?),
        ],
        fallback: None,
        result: Type::String,
    })
}

fn display_value(
    value: Expression,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    let checked = context.checked;
    use crate::runtime_abi::RuntimeFunction;
    let runtime = match value.ty() {
        Type::String => RuntimeFunction::QuoteString,
        Type::Bytes => RuntimeFunction::BytesDisplay,
        Type::Int => RuntimeFunction::IntToString,
        Type::Float => RuntimeFunction::CanonicalFloat,
        Type::Char => RuntimeFunction::QuoteChar,
        Type::Bool => return branch(value, display_text("true"), display_text("false")),
        Type::Void => return Ok(display_text("()")),
        Type::Data(name, parameters) if name == "List" && parameters.len() == 1 => {
            return display_list(value, parameters[0].clone(), context);
        }
        Type::Data(name, parameters) if name == ARRAY && parameters.len() == 1 => {
            return display_array(value, context);
        }
        Type::Data(name, _) if checked.records.contains_key(&name) => {
            return display_record_value(value, &name, context);
        }
        ty @ Type::Data(..) => return display_constructor_value(value, &ty, context),
        _ => return Err("aggregate display is not implemented".into()),
    };
    Expression::call_runtime(runtime, vec![value])
}

fn display_record_value(
    value: Expression,
    name: &str,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    let checked = context.checked;
    let fields = &checked.records[name].0;
    let lowerer = Lowerer {
        records: checked.records.clone(),
        ..Lowerer::default()
    };
    let mut result = display_text("{ ");
    for (index, name) in fields.iter().enumerate() {
        let label = if index == 0 {
            format!("{name} = ")
        } else {
            format!("; {name} = ")
        };
        result = concat_display(result, display_text(&label))?;
        let field = lowerer.record_get(value.clone(), name)?;
        let content = context.format(field)?;
        result = concat_display(result, content)?;
    }
    concat_display(result, display_text(" }"))
}

fn display_constructor_value(
    value: Expression,
    ty: &Type,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    let checked = context.checked;
    let lowerer = Lowerer {
        constructors: checked.constructors.clone(),
        ..Lowerer::default()
    };
    let mut names: Vec<_> = checked.constructors.keys().cloned().collect();
    names.sort();
    let mut arms = Vec::new();
    for name in names {
        let Ok(fields) = lowerer.constructor_fields(&name, ty) else {
            continue;
        };
        let content = constructor_display(&name, &fields, &value, context)?;
        arms.push((name, content));
    }
    if arms.is_empty() {
        return Err("unknown display type".into());
    }
    Ok(Expression::Switch {
        value: Box::new(value),
        arms,
        fallback: None,
        result: Type::String,
    })
}

fn constructor_display(
    name: &str,
    fields: &[Type],
    value: &Expression,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    let checked = context.checked;
    let Some(labels) = checked.named_constructors.get(name) else {
        return positional_constructor_display(name, fields, value, context);
    };
    let mut content = concat_display(display_text(name), display_text(" { "))?;
    for (index, (field, label)) in fields.iter().zip(labels).enumerate() {
        let label = if index == 0 {
            format!("{label} = ")
        } else {
            format!("; {label} = ")
        };
        content = concat_display(content, display_text(&label))?;
        let projected = Expression::Project {
            value: Box::new(value.clone()),
            constructor: name.to_owned(),
            index,
            result: field.clone(),
        };
        let rendered = context.format(projected)?;
        content = concat_display(content, rendered)?;
    }
    concat_display(content, display_text(" }"))
}

fn positional_constructor_display(
    name: &str,
    fields: &[Type],
    value: &Expression,
    context: &mut DisplayContext<'_>,
) -> Result<Expression, String> {
    if fields.is_empty() {
        return Ok(display_text(name));
    }
    let mut content = concat_display(display_text(name), display_text("("))?;
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            content = concat_display(content, display_text(", "))?;
        }
        let projected = Expression::Project {
            value: Box::new(value.clone()),
            constructor: name.to_owned(),
            index,
            result: field.clone(),
        };
        let rendered = context.format(projected)?;
        content = concat_display(content, rendered)?;
    }
    concat_display(content, display_text(")"))
}
