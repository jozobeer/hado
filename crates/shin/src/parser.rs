//! Recursive-descent parser for the shin grammar of §41, §43 and §44.
//!
//! Covers the full v0.1 syntax: named definitions (`def` / `def rec`),
//! local bindings (`let` / `let?` / `do?`), anonymous functions (`fn`),
//! the three branch forms (Boolean branch, condition column, pattern
//! branch), patterns, records, list literals, modules (`sig` / `struct`)
//! and type definitions. Every node carries a `Span`.
//!
//! Precedence (§44), high to low: application, unary `-`/`not`,
//! `* / %`, `+ -`, comparison (non-associative), `&&`, `||`, `|>`.
//! The parser ignores newlines (§41:1418).

use std::fmt;

pub use crate::lexer::Span;
use crate::lexer::{LexError, Tok, TokKind};

/// M0 IR (kept unchanged for `types.rs` / `emit.rs`).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Add(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub body: Expr,
}

/// Binary operators, in precedence order (lowest first for `as_str` usage):
/// `&&` < comparison < `+ -` < `* / %`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

/// Expression tree node. `span` is the start (1-based line/col) of the
/// node's leftmost token.
#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Int(i64),
    Float(f64),
    Char(char),
    Str(String),
    Bool(bool),
    Unit,
    Name(String),
    Neg(Box<Node>),
    Not(Box<Node>),
    Bin(BinOp, Box<Node>, Box<Node>),
    /// Juxtaposition application `f x y`.
    Apply(Box<Node>, Vec<Node>),
    /// Constructor call `Some(x)` (uppercase name immediately followed by `(`).
    Call(Box<Node>, Vec<Node>),
    /// Field access `user.name` / qualified name `M.inc`.
    Field(Box<Node>, String),
    Record(Vec<(String, Node)>),
    List(Vec<Node>),
    Let {
        pat: Pattern,
        value: Box<Node>,
        body: Box<Node>,
    },
    LetQ {
        pat: Pattern,
        value: Box<Node>,
        body: Box<Node>,
    },
    DoQ {
        value: Box<Node>,
        body: Box<Node>,
    },
    Fn {
        params: Vec<Param>,
        body: Box<Node>,
    },
    BoolBranch {
        cond: Box<Node>,
        then: Box<Node>,
        els: Box<Node>,
    },
    CondBranch {
        arms: Vec<(Node, Node)>,
        fallback: Option<Box<Node>>,
    },
    PatternBranch {
        scrutinee: Box<Node>,
        arms: Vec<(Pattern, Node)>,
        fallback: Option<Box<Node>>,
    },
    Pipe(Box<Node>, Box<Node>),
}

/// A pattern (`pattern :=` in §41).
#[derive(Debug, Clone)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    Underscore,
    Name(String),
    Constructor(String, Vec<Pattern>),
    Record(Vec<(String, Option<Pattern>)>),
    Literal(Box<Node>),
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            PatternKind::Underscore => write!(f, "_"),
            PatternKind::Name(n) => write!(f, "{n}"),
            PatternKind::Constructor(n, args) => {
                let inner = args
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "{n}({inner})")
            }
            PatternKind::Record(fields) => {
                let inner = fields
                    .iter()
                    .map(|(n, p)| match p {
                        Some(p) => format!("{n} = {p}"),
                        None => n.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(f, "{{ {inner} }}")
            }
            PatternKind::Literal(node) => write!(f, "{}", canonical(node)),
        }
    }
}

/// A parameter (bare `x` or typed `(x: Int)`).
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

/// A type expression (see §8, §10, §35, §37).
#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Name(String),
    /// `Option<T>` / `Cons2(T, List2<T>)` — type arguments or payloads.
    Apply(String, Vec<TypeExpr>),
    /// `(Int, Int)` argument group inside `Fn<...>`.
    Group(Vec<TypeExpr>),
    Record(Vec<(String, TypeExpr)>),
    /// `Nil2 | Cons2(T, List2<T>)` — ADT alternatives.
    Adt(Vec<TypeExpr>),
}

/// A parsed program: an ordered list of top-level items.
#[derive(Debug, Clone)]
pub struct Ast {
    pub items: Vec<Item>,
    pub span: Span,
}

impl Ast {
    /// Body of the `def main = <expr>` value definition, if present.
    pub fn main(&self) -> Option<&Node> {
        self.items.iter().find_map(|item| match item {
            Item::Def(d) if d.name == "main" => match &d.kind {
                DefKind::Value(body) => Some(body),
                _ => None,
            },
            _ => None,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Def(Def),
    SigDef(SigDef),
    TypeDef(TypeDef),
    ModuleSig(ModuleSig),
    ModuleImpl(ModuleImpl),
}

#[derive(Debug, Clone)]
pub struct Def {
    pub name: String,
    pub rec: bool,
    pub kind: DefKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DefKind {
    /// `def name = expression`
    Value(Node),
    /// `def [rec] name parameters return_type? => expression`
    Function {
        params: Vec<Param>,
        ret: Option<TypeExpr>,
        body: Node,
    },
}

/// A `def` inside a `sig` (typed signature, no body).
#[derive(Debug, Clone)]
pub struct SigDef {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ModuleSig {
    pub name: String,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ModuleImpl {
    pub name: String,
    pub sig: Option<String>,
    pub items: Vec<Item>,
    pub span: Span,
}

/// One-line diagnostic (`<path>:<line>:<col>: error[CODE]: <message>` for
/// E1xx, plain `error: <message>` otherwise).
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub code: Option<String>,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.code {
            Some(code) => write!(
                f,
                "{}:{}:{}: error[{code}]: {}",
                self.path, self.line, self.col, self.message
            ),
            None => write!(f, "error: {}", self.message),
        }
    }
}

/// Parse a source string into the full AST.
pub fn parse_source(src: &str, path: &str) -> Result<Ast, Diagnostic> {
    let toks = crate::lexer::lex_full(src).map_err(|e| lex_error_to_diag(e, path))?;
    parse_tokens(&toks, path)
}

/// M0-compatible entry: full-parse, then lower to the M0 IR
/// (`def main = <Int + ...>`). Everything else is an `M3+` error.
pub fn parse(toks: &[Tok]) -> Result<Program, String> {
    let ast = parse_tokens(toks, "<emit>").map_err(|d| d.to_string())?;
    lower_to_m0(&ast).ok_or_else(|| {
        "M3+ 以降で対応: only `def main = <Int arithmetic>` can be compiled yet".to_string()
    })
}

fn lex_error_to_diag(e: LexError, path: &str) -> Diagnostic {
    Diagnostic {
        path: path.to_string(),
        line: e.line,
        col: e.col,
        code: e.code.map(|s| s.to_string()),
        message: e.message,
    }
}

pub fn parse_tokens(toks: &[Tok], path: &str) -> Result<Ast, Diagnostic> {
    Parser {
        toks,
        pos: 0,
        path,
    }
    .parse_program()
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    path: &'a str,
}

impl Parser<'_> {
    fn cur(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }

    fn kind(&self) -> &TokKind {
        &self.cur().kind
    }

    fn span(&self) -> Span {
        self.cur().span
    }

    fn bump(&mut self) {
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, k: &TokKind) -> Result<Span, Diagnostic> {
        if self.kind() == k {
            let s = self.span();
            self.bump();
            Ok(s)
        } else {
            Err(self.error(None, format!("expected {k}, found {}", self.kind())))
        }
    }

    fn error(&self, code: Option<&'static str>, message: impl Into<String>) -> Diagnostic {
        let s = self.span();
        self.error_at(s, code, message)
    }

    fn error_at(
        &self,
        s: Span,
        code: Option<&'static str>,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            path: self.path.to_string(),
            line: s.line,
            col: s.col,
            code: code.map(|c| c.to_string()),
            message: message.into(),
        }
    }

    /// Whether a `(` immediately follows `name` in the source (no
    /// whitespace / comment in between). Used to separate constructor
    /// calls `Some(x)` from juxtaposition `f (x)`.
    fn paren_attaches(&self, name: &str, span: Span) -> bool {
        let next = self.span();
        next.line == span.line && next.col == span.col + name.len()
    }

    // --- top level ---

    fn parse_program(&mut self) -> Result<Ast, Diagnostic> {
        let span = self.span();
        let mut items = Vec::new();
        loop {
            match self.kind() {
                TokKind::Eof => break,
                TokKind::Def => items.push(self.parse_definition()?),
                TokKind::KwType => items.push(self.parse_type_def()?),
                TokKind::KwModule => items.push(self.parse_module()?),
                other => {
                    return Err(self.error(
                        None,
                        format!("expected `def`, `type`, or `module`, found {other}"),
                    ))
                }
            }
        }
        Ok(Ast { items, span })
    }

    fn parse_definition(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::Def)?;
        let mut rec = false;
        let mut rec_span = span;
        if self.kind() == &TokKind::KwRec {
            rec = true;
            rec_span = self.span();
            self.bump();
        }
        // E107: an operator cannot be the defined name (`def (+) a b => ...`).
        if self.kind() == &TokKind::LParen {
            let s = self.span();
            return Err(self.error_at(
                s,
                Some("E107"),
                "operator overloading is forbidden; operators cannot be defined",
            ));
        }
        let name = match self.kind() {
            TokKind::Name(n) => {
                let n = n.clone();
                self.bump();
                n
            }
            other => {
                return Err(self.error(
                    None,
                    format!("expected a name after `def`, found {other}"),
                ))
            }
        };
        if self.kind() == &TokKind::Eq {
            // E102: `def rec` is only allowed on function definitions.
            if rec {
                return Err(self.error_at(
                    rec_span,
                    Some("E102"),
                    "`rec` is only allowed on function definitions, not value definitions",
                ));
            }
            self.bump();
            let body = self.parse_expr()?;
            Ok(Item::Def(Def {
                name,
                rec: false,
                kind: DefKind::Value(body),
                span,
            }))
        } else {
            let params = self.parse_params()?;
            let ret = if self.kind() == &TokKind::Colon {
                self.bump();
                Some(self.parse_type()?)
            } else {
                None
            };
            self.expect(&TokKind::Arrow)?;
            let body = self.parse_expr()?;
            Ok(Item::Def(Def {
                name,
                rec,
                kind: DefKind::Function { params, ret, body },
                span,
            }))
        }
    }

    fn parse_sig_def(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::Def)?;
        let name = match self.kind() {
            TokKind::Name(n) => {
                let n = n.clone();
                self.bump();
                n
            }
            other => {
                return Err(self.error(
                    None,
                    format!("expected a name after `def`, found {other}"),
                ))
            }
        };
        let params = self.parse_params()?;
        let ret = if self.kind() == &TokKind::Colon {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        Ok(Item::SigDef(SigDef {
            name,
            params,
            ret,
            span,
        }))
    }

    fn parse_type_def(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwType)?;
        let name = match self.kind() {
            TokKind::Name(n) => {
                let n = n.clone();
                self.bump();
                n
            }
            other => {
                return Err(self.error(
                    None,
                    format!("expected a type name, found {other}"),
                ))
            }
        };
        if self.kind() == &TokKind::Lt {
            self.bump();
            loop {
                match self.kind() {
                    TokKind::Name(_) => self.bump(),
                    other => {
                        return Err(self.error(
                            None,
                            format!("expected a type parameter, found {other}"),
                        ))
                    }
                }
                if self.kind() == &TokKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect(&TokKind::Gt)?;
        }
        let ty = if self.kind() == &TokKind::Eq {
            self.bump();
            Some(self.parse_type()?)
        } else {
            None
        };
        Ok(Item::TypeDef(TypeDef { name, ty, span }))
    }

    fn parse_module(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwModule)?;
        if self.kind() == &TokKind::KwType {
            // module type NAME = sig ... end
            self.bump();
            let name = self.expect_name("a module-type name")?;
            self.expect(&TokKind::Eq)?;
            self.expect(&TokKind::KwSig)?;
            let mut items = Vec::new();
            while self.kind() != &TokKind::KwEnd {
                match self.kind() {
                    TokKind::KwType => items.push(self.parse_type_def()?),
                    TokKind::Def => items.push(self.parse_sig_def()?),
                    TokKind::Eof => {
                        return Err(self.error(None, "unterminated `sig`: expected `end`"))
                    }
                    other => {
                        return Err(self.error(
                            None,
                            format!("expected `type` or `def` in sig, found {other}"),
                        ))
                    }
                }
            }
            self.bump();
            Ok(Item::ModuleSig(ModuleSig { name, items, span }))
        } else {
            // module NAME [: SIG] = struct ... end
            let name = self.expect_name("a module name")?;
            let sig = if self.kind() == &TokKind::Colon {
                self.bump();
                Some(self.expect_name("a signature name after `:`")?)
            } else {
                None
            };
            self.expect(&TokKind::Eq)?;
            self.expect(&TokKind::KwStruct)?;
            let mut items = Vec::new();
            while self.kind() != &TokKind::KwEnd {
                match self.kind() {
                    TokKind::KwType => items.push(self.parse_type_def()?),
                    TokKind::Def => items.push(self.parse_definition()?),
                    TokKind::Eof => {
                        return Err(self.error(None, "unterminated `struct`: expected `end`"))
                    }
                    other => {
                        return Err(self.error(
                            None,
                            format!("expected `type` or `def` in struct, found {other}"),
                        ))
                    }
                }
            }
            self.bump();
            Ok(Item::ModuleImpl(ModuleImpl {
                name,
                sig,
                items,
                span,
            }))
        }
    }

    fn expect_name(&mut self, what: &str) -> Result<String, Diagnostic> {
        match self.kind() {
            TokKind::Name(n) => {
                let n = n.clone();
                self.bump();
                Ok(n)
            }
            other => Err(self.error(None, format!("expected {what}, found {other}"))),
        }
    }

    // --- parameters and types ---

    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        if self.kind() == &TokKind::LParen {
            self.bump();
            if self.kind() != &TokKind::RParen {
                loop {
                    let span = self.span();
                    let pname = self.expect_name("a parameter name")?;
                    let mut ty = None;
                    if self.kind() == &TokKind::Colon {
                        self.bump();
                        ty = Some(self.parse_type()?);
                    }
                    params.push(Param {
                        name: pname,
                        ty,
                        span,
                    });
                    if self.kind() == &TokKind::Comma {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(&TokKind::RParen)?;
        } else {
            while let TokKind::Name(n) = self.kind() {
                let span = self.span();
                let n = n.clone();
                self.bump();
                params.push(Param {
                    name: n,
                    ty: None,
                    span,
                });
            }
        }
        Ok(params)
    }

    fn parse_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        // Argument group `(Int, Int)` used inside `Fn<(Int, Int), Int>`.
        if self.kind() == &TokKind::LParen {
            self.bump();
            let mut items = Vec::new();
            if self.kind() != &TokKind::RParen {
                loop {
                    items.push(self.parse_type()?);
                    if self.kind() == &TokKind::Comma {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(&TokKind::RParen)?;
            return Ok(TypeExpr::Group(items));
        }
        // Record type `{ name: String; age: Int }`.
        if self.kind() == &TokKind::LBrace {
            self.bump();
            let mut fields = Vec::new();
            if self.kind() != &TokKind::RBrace {
                loop {
                    let fname = self.expect_name("a field name")?;
                    self.expect(&TokKind::Colon)?;
                    let ty = self.parse_type()?;
                    fields.push((fname, ty));
                    if self.kind() == &TokKind::Semicolon {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(&TokKind::RBrace)?;
            return Ok(TypeExpr::Record(fields));
        }
        // ADT alternatives separated by `|`. A leading `|` (as in §10/§11,
        // `type Nest<T> =\n  | Leaf(T)`) and no leading `|` are both accepted.
        if self.kind() == &TokKind::Pipe {
            self.bump();
        }
        let mut alts = vec![self.parse_type_variant()?];
        while self.kind() == &TokKind::Pipe {
            self.bump();
            alts.push(self.parse_type_variant()?);
        }
        if alts.len() == 1 {
            Ok(alts.pop().unwrap())
        } else {
            Ok(TypeExpr::Adt(alts))
        }
    }

    fn parse_type_variant(&mut self) -> Result<TypeExpr, Diagnostic> {
        let name = self.expect_name("a type name")?;
        if self.kind() == &TokKind::Lt {
            self.bump();
            let mut args = Vec::new();
            loop {
                args.push(self.parse_type()?);
                if self.kind() == &TokKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect(&TokKind::Gt)?;
            Ok(TypeExpr::Apply(name, args))
        } else if self.kind() == &TokKind::LParen {
            self.bump();
            let mut args = Vec::new();
            if self.kind() != &TokKind::RParen {
                loop {
                    args.push(self.parse_type()?);
                    if self.kind() == &TokKind::Comma {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            self.expect(&TokKind::RParen)?;
            Ok(TypeExpr::Apply(name, args))
        } else {
            Ok(TypeExpr::Name(name))
        }
    }

    // --- expressions ---

    fn parse_expr(&mut self) -> Result<Node, Diagnostic> {
        self.parse_pipe()
    }

    /// Parse an expression that must not itself contain a branch
    /// (§20: branches cannot nest). Used for every component of a branch.
    fn parse_branch_expr(&mut self) -> Result<Node, Diagnostic> {
        let node = self.parse_expr()?;
        if let Some(sp) = contains_branch(&node) {
            return Err(self.error_at(
                sp,
                Some("E101"),
                "branches cannot be nested inside another branch",
            ));
        }
        Ok(node)
    }

    fn parse_pipe(&mut self) -> Result<Node, Diagnostic> {
        let mut left = self.parse_or()?;
        while self.kind() == &TokKind::PipeArrow {
            self.bump();
            let right = self.parse_or()?;
            let lspan = left.span;
            left = Node {
                kind: NodeKind::Pipe(Box::new(left), Box::new(right)),
                span: lspan,
            };
        }
        Ok(left)
    }

    fn parse_or(&mut self) -> Result<Node, Diagnostic> {
        let mut left = self.parse_and()?;
        while self.kind() == &TokKind::OrOr {
            self.bump();
            let right = self.parse_and()?;
            left = bin_node(BinOp::Or, left, right);
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Node, Diagnostic> {
        let mut left = self.parse_cmp()?;
        while self.kind() == &TokKind::AndAnd {
            self.bump();
            let right = self.parse_cmp()?;
            left = bin_node(BinOp::And, left, right);
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Node, Diagnostic> {
        let left = self.parse_add()?;
        let op = match self.kind() {
            TokKind::Lt => Some(BinOp::Lt),
            TokKind::Le => Some(BinOp::Le),
            TokKind::Gt => Some(BinOp::Gt),
            TokKind::Ge => Some(BinOp::Ge),
            TokKind::EqEq => Some(BinOp::Eq),
            TokKind::Ne => Some(BinOp::Ne),
            _ => None,
        };
        match op {
            Some(op) => {
                self.bump();
                let right = self.parse_add()?;
                // Comparisons are non-associative (§44): `1 < 2 < 3` is a
                // syntax error and carries no E-code.
                if matches!(
                    self.kind(),
                    TokKind::Lt
                        | TokKind::Le
                        | TokKind::Gt
                        | TokKind::Ge
                        | TokKind::EqEq
                        | TokKind::Ne
                ) {
                    return Err(self.error(
                        None,
                        format!(
                            "comparison operators are non-associative; found {}",
                            self.kind()
                        ),
                    ));
                }
                Ok(bin_node(op, left, right))
            }
            None => Ok(left),
        }
    }

    fn parse_add(&mut self) -> Result<Node, Diagnostic> {
        let mut left = self.parse_mul()?;
        loop {
            let op = match self.kind() {
                TokKind::Plus => BinOp::Add,
                TokKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_mul()?;
            left = bin_node(op, left, right);
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Node, Diagnostic> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.kind() {
                TokKind::Star => BinOp::Mul,
                TokKind::Slash => BinOp::Div,
                TokKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary()?;
            left = bin_node(op, left, right);
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Node, Diagnostic> {
        if self.kind() == &TokKind::Minus {
            let span = self.span();
            self.bump();
            let operand = self.parse_unary()?;
            Ok(Node {
                kind: NodeKind::Neg(Box::new(operand)),
                span,
            })
        } else if self.kind() == &TokKind::KwNot {
            // `not` sits in the same precedence tier as unary `-` (§44).
            let span = self.span();
            self.bump();
            let operand = self.parse_unary()?;
            Ok(Node {
                kind: NodeKind::Not(Box::new(operand)),
                span,
            })
        } else {
            self.parse_apply()
        }
    }

    fn parse_apply(&mut self) -> Result<Node, Diagnostic> {
        let mut f = self.parse_primary()?;
        while self.atom_start() {
            let arg = self.parse_primary()?;
            let fspan = f.span;
            f = Node {
                kind: NodeKind::Apply(Box::new(f), vec![arg]),
                span: fspan,
            };
        }
        Ok(f)
    }

    fn atom_start(&self) -> bool {
        matches!(
            self.kind(),
            TokKind::Name(_)
                | TokKind::Int(_)
                | TokKind::Float(_)
                | TokKind::Char(_)
                | TokKind::Str(_)
                | TokKind::True
                | TokKind::False
                | TokKind::Unit
                | TokKind::LParen
                | TokKind::LBrace
                | TokKind::LBracket
                | TokKind::KwIf
                | TokKind::KwFn
                | TokKind::KwLet
                | TokKind::KwLetQ
                | TokKind::KwDoQ
        )
    }

    fn parse_primary(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        let kind = match self.kind() {
            TokKind::Int(n) => {
                let n = *n;
                self.bump();
                NodeKind::Int(n)
            }
            TokKind::Float(x) => {
                let x = *x;
                self.bump();
                NodeKind::Float(x)
            }
            TokKind::Char(c) => {
                let c = *c;
                self.bump();
                NodeKind::Char(c)
            }
            TokKind::Str(s) => {
                let s = s.clone();
                self.bump();
                NodeKind::Str(s)
            }
            TokKind::True => {
                self.bump();
                NodeKind::Bool(true)
            }
            TokKind::False => {
                self.bump();
                NodeKind::Bool(false)
            }
            TokKind::Unit => {
                self.bump();
                NodeKind::Unit
            }
            TokKind::Name(_) => return self.parse_name_or_call(),
            TokKind::LParen => {
                let lp = self.span();
                self.bump();
                let inner = self.parse_expr()?;
                // E105: no tuples.
                if self.kind() == &TokKind::Comma {
                    return Err(self.error_at(
                        lp,
                        Some("E105"),
                        "tuples are not supported; `(a, b)` is not an expression",
                    ));
                }
                self.expect(&TokKind::RParen)?;
                // Parenthesized grouping is not a node; redundant
                // parentheses are erased here.
                return Ok(inner);
            }
            TokKind::LBrace => {
                self.bump();
                let fields = self.parse_record_fields()?;
                NodeKind::Record(fields)
            }
            TokKind::LBracket => {
                self.bump();
                let items = self.parse_list_items()?;
                NodeKind::List(items)
            }
            TokKind::KwIf => return self.parse_branch(),
            TokKind::KwFn => return self.parse_fun(),
            TokKind::KwLet => return self.parse_let(false),
            TokKind::KwLetQ => return self.parse_let(true),
            TokKind::KwDoQ => return self.parse_do(),
            other => {
                return Err(self.error(
                    None,
                    format!("expected an expression, found {other}"),
                ))
            }
        };
        Ok(Node { kind, span })
    }

    fn parse_name_or_call(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        let name = self.expect_name("a name")?;
        let mut node = Node {
            kind: NodeKind::Name(name.clone()),
            span,
        };
        let mut last_name = name;
        let mut last_span = span;
        while self.kind() == &TokKind::Dot {
            self.bump();
            let fspan = self.span();
            let field = self.expect_name("a field name after `.`")?;
            let node_span = node.span;
            node = Node {
                kind: NodeKind::Field(Box::new(node), field.clone()),
                span: node_span,
            };
            last_name = field;
            last_span = fspan;
        }
        if self.kind() == &TokKind::LParen && self.paren_attaches(&last_name, last_span) {
            let lp = self.span();
            if last_name.chars().next().map_or(false, |c| c.is_uppercase()) {
                // Constructor call `Some(x)`.
                self.bump();
                let mut args = Vec::new();
                if self.kind() != &TokKind::RParen {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.kind() == &TokKind::Comma {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokKind::RParen)?;
                return Ok(Node {
                    kind: NodeKind::Call(Box::new(node), args),
                    span,
                });
            } else {
                // E106: functions are called by juxtaposition, not parens.
                return Err(self.error_at(
                    lp,
                    Some("E106"),
                    "function calls use juxtaposition `f x`; `f(x)` is only for constructors",
                ));
            }
        }
        Ok(node)
    }

    fn parse_record_fields(&mut self) -> Result<Vec<(String, Node)>, Diagnostic> {
        let mut fields = Vec::new();
        if self.kind() == &TokKind::RBrace {
            self.bump();
            return Ok(fields);
        }
        loop {
            let fname = self.expect_name("a field name")?;
            self.expect(&TokKind::Eq)?;
            let value = self.parse_expr()?;
            fields.push((fname, value));
            if self.kind() == &TokKind::Semicolon {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(&TokKind::RBrace)?;
        Ok(fields)
    }

    fn parse_list_items(&mut self) -> Result<Vec<Node>, Diagnostic> {
        let mut items = Vec::new();
        if self.kind() == &TokKind::RBracket {
            self.bump();
            return Ok(items);
        }
        loop {
            items.push(self.parse_expr()?);
            if self.kind() == &TokKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(&TokKind::RBracket)?;
        Ok(items)
    }

    fn parse_branch(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump(); // `if`
        if self.kind() == &TokKind::Pipe {
            return self.parse_cond_branch(span);
        }
        // Boolean branch or pattern branch share the scrutinee prefix.
        let scrutinee = self.parse_branch_expr()?;
        match self.kind() {
            TokKind::Arrow => {
                // Boolean branch: `if expr => then | else`.
                self.bump();
                let then = self.parse_branch_expr()?;
                self.expect(&TokKind::Pipe)?;
                let els = self.parse_branch_expr()?;
                Ok(Node {
                    kind: NodeKind::BoolBranch {
                        cond: Box::new(scrutinee),
                        then: Box::new(then),
                        els: Box::new(els),
                    },
                    span,
                })
            }
            TokKind::Pipe => {
                let mut arms = Vec::new();
                let mut fallback = None;
                while self.kind() == &TokKind::Pipe {
                    self.bump();
                    let save = self.pos;
                    if let Ok(pat) = self.parse_pattern() {
                        if self.kind() == &TokKind::Arrow {
                            self.bump();
                            let body = self.parse_branch_expr()?;
                            arms.push((pat, body));
                            continue;
                        }
                    }
                    // No `=>` after the pattern: it is the fallback
                    // expression (`pattern_fallback := "|" expression`).
                    self.pos = save;
                    let fb = self.parse_branch_expr()?;
                    fallback = Some(Box::new(fb));
                    break;
                }
                Ok(Node {
                    kind: NodeKind::PatternBranch {
                        scrutinee: Box::new(scrutinee),
                        arms,
                        fallback,
                    },
                    span,
                })
            }
            other => Err(self.error(
                None,
                format!("expected `=>` or `|` after `if`, found {other}"),
            )),
        }
    }

    fn parse_cond_branch(&mut self, span: Span) -> Result<Node, Diagnostic> {
        let mut arms = Vec::new();
        let mut fallback = None;
        while self.kind() == &TokKind::Pipe {
            self.bump();
            let head = self.parse_branch_expr()?;
            if self.kind() == &TokKind::Arrow {
                self.bump();
                let body = self.parse_branch_expr()?;
                arms.push((head, body));
            } else {
                // `| expression` — a fallback.
                fallback = Some(Box::new(head));
            }
        }
        Ok(Node {
            kind: NodeKind::CondBranch { arms, fallback },
            span,
        })
    }

    fn parse_fun(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump(); // `fn`
        let params = self.parse_params()?;
        self.expect(&TokKind::Arrow)?;
        let body = self.parse_expr()?;
        Ok(Node {
            kind: NodeKind::Fn {
                params,
                body: Box::new(body),
            },
            span,
        })
    }

    fn parse_let(&mut self, question: bool) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump(); // `let` / `let?`
        let pat = self.parse_pattern()?;
        self.expect(&TokKind::Eq)?;
        let value = self.parse_expr()?;
        self.expect(&TokKind::DashArrow)?;
        let body = self.parse_expr()?;
        let kind = if question {
            NodeKind::LetQ {
                pat,
                value: Box::new(value),
                body: Box::new(body),
            }
        } else {
            NodeKind::Let {
                pat,
                value: Box::new(value),
                body: Box::new(body),
            }
        };
        Ok(Node { kind, span })
    }

    fn parse_do(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump(); // `do?`
        let value = self.parse_expr()?;
        self.expect(&TokKind::DashArrow)?;
        let body = self.parse_expr()?;
        Ok(Node {
            kind: NodeKind::DoQ {
                value: Box::new(value),
                body: Box::new(body),
            },
            span,
        })
    }

    // --- patterns ---

    fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let span = self.span();
        let kind = match self.kind() {
            TokKind::Name(n) if n == "_" => {
                self.bump();
                PatternKind::Underscore
            }
            TokKind::Name(n) => {
                let name = n.clone();
                let name_span = self.span();
                self.bump();
                if self.kind() == &TokKind::LParen && self.paren_attaches(&name, name_span) {
                    self.bump();
                    let mut args = Vec::new();
                    if self.kind() != &TokKind::RParen {
                        loop {
                            args.push(self.parse_pattern()?);
                            if self.kind() == &TokKind::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.expect(&TokKind::RParen)?;
                    PatternKind::Constructor(name, args)
                } else {
                    PatternKind::Name(name)
                }
            }
            TokKind::LBrace => {
                self.bump();
                let mut fields = Vec::new();
                if self.kind() != &TokKind::RBrace {
                    loop {
                        let fname = self.expect_name("a field name")?;
                        let pat = if self.kind() == &TokKind::Eq {
                            self.bump();
                            Some(self.parse_pattern()?)
                        } else {
                            None
                        };
                        fields.push((fname, pat));
                        if self.kind() == &TokKind::Semicolon {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                self.expect(&TokKind::RBrace)?;
                PatternKind::Record(fields)
            }
            TokKind::Int(_)
            | TokKind::Float(_)
            | TokKind::Char(_)
            | TokKind::Str(_)
            | TokKind::True
            | TokKind::False => {
                let lit = self.parse_primary()?;
                PatternKind::Literal(Box::new(lit))
            }
            // E105: no tuples. `(a, b)` is not a pattern, so a `(` at the
            // start of a pattern is always a tuple attempt (§37).
            TokKind::LParen => {
                return Err(self.error(
                    Some("E105"),
                    "tuples are not supported; `(a, b)` is not a pattern",
                ))
            }
            other => {
                return Err(self.error(None, format!("expected a pattern, found {other}")))
            }
        };
        Ok(Pattern { kind, span })
    }
}

fn bin_node(op: BinOp, left: Node, right: Node) -> Node {
    let span = left.span;
    Node {
        kind: NodeKind::Bin(op, Box::new(left), Box::new(right)),
        span,
    }
}

/// Whether `node` (or any expression it contains) is a branch.
/// Returns the span of the first branch found (§20).
fn contains_branch(node: &Node) -> Option<Span> {
    match &node.kind {
        NodeKind::BoolBranch { .. }
        | NodeKind::CondBranch { .. }
        | NodeKind::PatternBranch { .. } => Some(node.span),
        NodeKind::Neg(c) | NodeKind::Not(c) => contains_branch(c),
        NodeKind::Bin(_, l, r) => contains_branch(l).or_else(|| contains_branch(r)),
        NodeKind::Apply(f, args) => contains_branch(f).or_else(|| args.iter().find_map(contains_branch)),
        NodeKind::Call(_, args) => args.iter().find_map(contains_branch),
        NodeKind::Field(l, _) => contains_branch(l),
        NodeKind::Record(fields) => fields.iter().find_map(|(_, v)| contains_branch(v)),
        NodeKind::List(items) => items.iter().find_map(contains_branch),
        NodeKind::Let { value, body, .. } | NodeKind::LetQ { value, body, .. } => {
            contains_branch(value).or_else(|| contains_branch(body))
        }
        NodeKind::DoQ { value, body } => {
            contains_branch(value).or_else(|| contains_branch(body))
        }
        NodeKind::Fn { body, .. } => contains_branch(body),
        NodeKind::Pipe(l, r) => contains_branch(l).or_else(|| contains_branch(r)),
        _ => None,
    }
}

/// Lower the full AST to the M0 IR, accepting only `def main = <Int + ...>`.
pub fn lower_to_m0(ast: &Ast) -> Option<Program> {
    let body = lower_expr(ast.main()?)?;
    Some(Program { body })
}

fn lower_expr(node: &Node) -> Option<Expr> {
    match &node.kind {
        NodeKind::Int(n) => Some(Expr::Int(*n)),
        NodeKind::Bin(BinOp::Add, l, r) => Some(Expr::Add(
            Box::new(lower_expr(l)?),
            Box::new(lower_expr(r)?),
        )),
        _ => None,
    }
}

/// Fully parenthesized canonical form of an expression tree.
/// `1 + 2 * 3` renders as `(1 + (2 * 3))`; leaves render bare.
pub fn canonical(node: &Node) -> String {
    match &node.kind {
        NodeKind::Int(n) => n.to_string(),
        NodeKind::Float(x) => x.to_string(),
        NodeKind::Char(c) => format!("'{c}'"),
        NodeKind::Str(s) => format!("\"{s}\""),
        NodeKind::Bool(b) => b.to_string(),
        NodeKind::Unit => "()".to_string(),
        NodeKind::Name(n) => n.clone(),
        NodeKind::Neg(c) => format!("(-{})", canonical(c)),
        NodeKind::Not(c) => format!("(not {})", canonical(c)),
        NodeKind::Bin(op, l, r) => {
            format!("({} {} {})", canonical(l), op.as_str(), canonical(r))
        }
        NodeKind::Pipe(l, r) => format!("({} |> {})", canonical(l), canonical(r)),
        NodeKind::Apply(f, args) => {
            let mut s = canonical(f);
            for a in args {
                s.push(' ');
                s.push_str(&canonical(a));
            }
            s
        }
        NodeKind::Call(f, args) => {
            let mut s = canonical(f);
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&canonical(a));
            }
            s.push(')');
            s
        }
        NodeKind::Field(l, r) => format!("{}.{}", canonical(l), r),
        NodeKind::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(n, v)| format!("{n} = {}", canonical(v)))
                .collect::<Vec<_>>()
                .join("; ");
            format!("{{ {inner} }}")
        }
        NodeKind::List(items) => {
            let inner = items
                .iter()
                .map(canonical)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        NodeKind::Let { pat, value, body } => {
            format!("(let {pat} = {} -> {})", canonical(value), canonical(body))
        }
        NodeKind::LetQ { pat, value, body } => {
            format!("(let? {pat} = {} -> {})", canonical(value), canonical(body))
        }
        NodeKind::DoQ { value, body } => {
            format!("(do? {} -> {})", canonical(value), canonical(body))
        }
        NodeKind::Fn { params, body } => {
            let ps = params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            format!("(fn {ps} => {})", canonical(body))
        }
        NodeKind::BoolBranch { cond, then, els } => format!(
            "(if {} => {} | {})",
            canonical(cond),
            canonical(then),
            canonical(els)
        ),
        NodeKind::CondBranch { arms, fallback } => {
            let mut s = "(if".to_string();
            for (c, b) in arms {
                s.push_str(&format!(" | {} => {}", canonical(c), canonical(b)));
            }
            if let Some(fb) = fallback {
                s.push_str(&format!(" | {}", canonical(fb)));
            }
            s.push(')');
            s
        }
        NodeKind::PatternBranch {
            scrutinee,
            arms,
            fallback,
        } => {
            let mut s = format!("(if {}", canonical(scrutinee));
            for (p, b) in arms {
                s.push_str(&format!(" | {p} => {}", canonical(b)));
            }
            if let Some(fb) = fallback {
                s.push_str(&format!(" | {}", canonical(fb)));
            }
            s.push(')');
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_expr_ok(src: &str) -> String {
        canonical(parse_source(src, "t").unwrap().main().unwrap())
    }

    /// Type of the first top-level `type` item, if present.
    fn type_of(src: &str) -> Option<TypeExpr> {
        match parse_source(src, "t").unwrap().items.into_iter().next() {
            Some(Item::TypeDef(td)) => td.ty,
            _ => None,
        }
    }

    #[test]
    fn precedence_mul_over_add() {
        assert_eq!(parse_expr_ok("def main = 1 + 2 * 3"), "(1 + (2 * 3))");
        assert_eq!(parse_expr_ok("def main = 2 * 3 + 1"), "((2 * 3) + 1)");
    }

    #[test]
    fn left_assoc_add_and_mul() {
        assert_eq!(
            parse_expr_ok("def main = 1 + 2 + 3"),
            "((1 + 2) + 3)"
        );
        assert_eq!(
            parse_expr_ok("def main = 10 / 2 / 5"),
            "((10 / 2) / 5)"
        );
    }

    #[test]
    fn comparisons_and_logic() {
        assert_eq!(
            parse_expr_ok("def main = 1 < 2 && 3 >= 4"),
            "((1 < 2) && (3 >= 4))"
        );
        assert_eq!(
            parse_expr_ok("def main = 1 < 2 || 3 > 4"),
            "((1 < 2) || (3 > 4))"
        );
        assert_eq!(parse_expr_ok("def main = 1 == 2"), "(1 == 2)");
    }

    #[test]
    fn unary_minus_binds_tightest() {
        assert_eq!(parse_expr_ok("def main = - 2 * 3"), "((-2) * 3)");
        assert_eq!(parse_expr_ok("def main = 1 - - 2"), "(1 - (-2))");
    }

    #[test]
    fn parens_override_precedence() {
        assert_eq!(
            parse_expr_ok("def main = (1 + 2) * 3"),
            "((1 + 2) * 3)"
        );
    }

    #[test]
    fn comments_and_newlines_are_ignored() {
        assert_eq!(
            parse_expr_ok("def main = 1 -- hi\n + 2"),
            "(1 + 2)"
        );
    }

    #[test]
    fn non_assoc_comparison_rejected() {
        let d = parse_source("def main = 1 < 2 < 3", "t").unwrap_err();
        assert!(d.message.contains("non-associative"), "got: {d}");
    }

    #[test]
    fn application_and_pipe() {
        assert_eq!(
            parse_expr_ok("def main = f x y"),
            "f x y"
        );
        assert_eq!(
            parse_expr_ok("def main = 1 |> add 2 3"),
            "(1 |> add 2 3)"
        );
    }

    #[test]
    fn constructor_and_field() {
        assert_eq!(
            parse_expr_ok("def main = Some(x)"),
            "Some(x)"
        );
        assert_eq!(
            parse_expr_ok("def main = M.inc 2"),
            "M.inc 2"
        );
        assert_eq!(
            parse_expr_ok("def main = user.name"),
            "user.name"
        );
    }

    #[test]
    fn record_and_list_literals() {
        assert_eq!(
            parse_expr_ok("def main = { name = \"a\"; age = 3 }"),
            "{ name = \"a\"; age = 3 }"
        );
        assert_eq!(parse_expr_ok("def main = [1, 2, 3]"), "[1, 2, 3]");
        assert_eq!(parse_expr_ok("def main = []"), "[]");
    }

    #[test]
    fn bindings_and_fn() {
        assert_eq!(
            parse_expr_ok("def main = let x = 1 -> x + 1"),
            "(let x = 1 -> (x + 1))"
        );
        assert_eq!(
            parse_expr_ok("def main = let? x = f y -> x"),
            "(let? x = f y -> x)"
        );
        assert_eq!(
            parse_expr_ok("def main = do? g x -> x"),
            "(do? g x -> x)"
        );
        assert_eq!(
            parse_expr_ok("def main = fn x => x + 1"),
            "(fn x => (x + 1))"
        );
    }

    #[test]
    fn branches() {
        assert_eq!(
            parse_expr_ok("def main = if x > 10 => Large | Small"),
            "(if (x > 10) => Large | Small)"
        );
        assert_eq!(
            parse_expr_ok("def main = if | x > 1 => 1 | 0"),
            "(if | (x > 1) => 1 | 0)"
        );
        assert_eq!(
            parse_expr_ok("def main = if x | Some(v) => v | 0"),
            "(if x | Some(v) => v | 0)"
        );
    }

    #[test]
    fn defs_modules_types() {
        assert!(parse_source("def add x y => x + y", "t").is_ok());
        assert!(parse_source("def rec ping x => pong x", "t").is_ok());
        assert!(parse_source("type List2<T> = Nil2 | Cons2(T, List2<T>)", "t").is_ok());
        assert!(parse_source("module type S = sig def inc(x: Int): Int end", "t").is_ok());
        assert!(parse_source("module M : S = struct def inc x => x + 1 end", "t").is_ok());
    }

    #[test]
    fn adt_accepts_leading_pipe() {
        // §10 / §11 write ADTs with a leading `|` on the first variant;
        // the no-leading-`|` form must also stay accepted.
        assert!(parse_source(
            "type Nest<T> =\n  | Leaf(T)\n  | Node(Nest<List<T>>)",
            "t",
        )
        .is_ok());
        assert!(parse_source("type Color =\n  | Red\n  | Green\n  | Blue", "t").is_ok());
        assert!(parse_source("type List2<T> = Nil2 | Cons2(T, List2<T>)", "t").is_ok());
        assert_eq!(
            type_of("type Color =\n  | Red\n  | Green\n  | Blue"),
            Some(TypeExpr::Adt(vec![
                TypeExpr::Name("Red".into()),
                TypeExpr::Name("Green".into()),
                TypeExpr::Name("Blue".into()),
            ])),
        );
    }

    #[test]
    fn fn_type_group() {
        // §37: `Fn<(Int, Int), Int>` is the v0.1 function type. The `( )`
        // is an argument group, not a tuple.
        assert_eq!(
            type_of("type F = Fn<(Int, Int), Int>"),
            Some(TypeExpr::Apply(
                "Fn".into(),
                vec![
                    TypeExpr::Group(vec![TypeExpr::Name("Int".into()), TypeExpr::Name("Int".into())]),
                    TypeExpr::Name("Int".into()),
                ],
            )),
        );
        assert!(parse_source("def f(x: Fn<(Int, Int), Int>): Int => 1", "t").is_ok());
        assert!(parse_source("type F = Fn<Int, String>", "t").is_ok());
    }

    #[test]
    fn not_unary_precedence() {
        // `not` sits in the same tier as unary `-` (§44), above `&&` and the
        // comparisons.
        assert_eq!(parse_expr_ok("def main = not true"), "(not true)");
        assert_eq!(parse_expr_ok("def main = not (1 == 2)"), "(not (1 == 2))");
        assert_eq!(
            parse_expr_ok("def main = not true && false"),
            "((not true) && false)"
        );
        assert_eq!(
            parse_expr_ok("def main = not 1 == 2"),
            "((not 1) == 2)"
        );
        assert_eq!(parse_expr_ok("def main = not not true"), "(not (not true))");
        assert_eq!(parse_expr_ok("def main = - not true"), "(-(not true))");
        assert_eq!(parse_expr_ok("def main = not - 1"), "(not (-1))");
    }

    #[test]
    fn e101_nested_branch() {
        let d = parse_source(
            "def f x =>\n  if x\n  | 1 => if x\n    | 2 => 5\n    | 0\n  | 0",
            "t",
        )
        .unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E101"));
        assert_eq!(d.line, 3, "inner `if` is on line 3 of the source, got {d}");
    }

    #[test]
    fn e102_rec_on_value() {
        let d = parse_source("def rec pi = 3.14159", "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E102"));
    }

    #[test]
    fn e105_no_tuple() {
        let d = parse_source("def main = (1, 2)", "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E105"));
    }

    #[test]
    fn e105_tuple_pattern() {
        // §37: tuple syntax in a pattern is also E105, not a plain error.
        let d = parse_source("def main = let (a, b) = x -> a", "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E105"));
        let d = parse_source("def main = if x | (a, b) => a | 0", "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E105"));
    }

    #[test]
    fn e106_constructor_call_form() {
        let d = parse_source("def inc x => x + 1\ndef main = inc(1)", "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E106"));
    }

    #[test]
    fn e107_operator_overloading() {
        let d = parse_source("def (+) a b => a", "t").unwrap_err();
        assert_eq!(d.code.as_deref(), Some("E107"));
    }

    #[test]
    fn lower_m0_subset() {
        let ast = parse_source("def main = 1 + 2 + 3", "t").unwrap();
        let prog = lower_to_m0(&ast).unwrap();
        assert_eq!(
            prog.body,
            Expr::Add(
                Box::new(Expr::Add(Box::new(Expr::Int(1)), Box::new(Expr::Int(2)))),
                Box::new(Expr::Int(3))
            )
        );
        assert!(lower_to_m0(&parse_source("def other = 1", "t").unwrap()).is_none());
        assert!(lower_to_m0(&parse_source("def main = 1 * 2", "t").unwrap()).is_none());
    }

    #[test]
    fn m0_parse_entry() {
        let toks = crate::lexer::lex("def main = 10 + 20").unwrap();
        let prog = parse(&toks).unwrap();
        assert_eq!(
            prog.body,
            Expr::Add(Box::new(Expr::Int(10)), Box::new(Expr::Int(20)))
        );
        let toks = crate::lexer::lex("def main = 1 < 2").unwrap();
        assert!(parse(&toks).is_err());
    }
}