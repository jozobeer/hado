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

#[derive(Debug, Clone, PartialEq)]
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
    /// Constructor call `Some(x)`; whitespace before `(` is insignificant.
    Call(Box<Node>, Vec<Node>),
    /// Field access `user.name` / qualified name `M.inc`.
    Field(Box<Node>, String),
    Record(Vec<(DeclaredName, Node)>),
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

#[derive(Debug, Clone, PartialEq)]
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
                        Some(p) if matches!(&p.kind, PatternKind::Name(name) if name == n) => n.clone(),
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

#[derive(Debug, Clone)]
pub struct DeclaredName {
    pub name: String,
    pub span: Span,
}

impl PartialEq for DeclaredName {
    fn eq(&self, other: &Self) -> bool { self.name == other.name }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeField {
    pub name: DeclaredName,
    pub ty: TypeExpr,
}

/// A type expression (see §8, §10, §35, §37).
#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}

impl PartialEq for TypeExpr {
    fn eq(&self, other: &Self) -> bool { self.kind == other.kind }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Name(String),
    /// `Option<T>` / `Cons2(T, List2<T>)` — type arguments or payloads.
    Apply(String, Vec<TypeExpr>),
    /// `(Int, Int)` argument group inside `Fn<...>`.
    Group(Vec<TypeExpr>),
    Record(Vec<TypeField>),
    /// `Nil2 | Cons2(T, List2<T>)` — ADT alternatives.
    Adt(Vec<TypeExpr>),
}

/// A parsed program: an ordered list of top-level items.
#[derive(Debug, Clone)]
pub struct Ast {
    pub items: Vec<Item>,
    pub span: Span,
    pub branches_after_fallback: Vec<Span>,
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
    pub name_span: Span,
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
    pub name_span: Span,
    pub type_params: Vec<DeclaredName>,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub name_span: Span,
    pub params: Vec<DeclaredName>,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ModuleSig {
    pub name: String,
    pub name_span: Span,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ModuleImpl {
    pub name: String,
    pub name_span: Span,
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
            None => write!(f, "{}:{}:{}: error: {}", self.path, self.line, self.col, self.message),
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
        branches_after_fallback: Vec::new(),
    }
    .parse_program()
}

#[derive(Clone, Copy, PartialEq)]
enum TypeContext { TopLevel, Signature, Implementation }

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    path: &'a str,
    branches_after_fallback: Vec<Span>,
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
            code: Some(code.unwrap_or("E100").to_string()),
            message: message.into(),
        }
    }

    // --- top level ---

    fn parse_program(&mut self) -> Result<Ast, Diagnostic> {
        let span = self.span();
        let mut items = Vec::new();
        loop {
            match self.kind() {
                TokKind::Eof => break,
                TokKind::Def => items.push(self.parse_definition()?),
                TokKind::KwType => items.push(self.parse_type_def(TypeContext::TopLevel)?),
                TokKind::KwModule => items.push(self.parse_module()?),
                other => {
                    return Err(self.error(
                        None,
                        format!("expected `def`, `type`, or `module`, found {other}"),
                    ))
                }
            }
        }
        Ok(Ast { items, span, branches_after_fallback: self.branches_after_fallback.clone() })
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
            let s = self.toks.get(self.pos + 1).map_or(self.span(), |token| token.span);
            return Err(self.error_at(
                s,
                Some("E107"),
                "operator overloading is forbidden; operators cannot be defined",
            ));
        }
        let name_span = self.span();
        let name = self.expect_lower_name("a declaration name")?;
        if self.kind() == &TokKind::Pipe {
            let (params, body) = self.parse_arm_function(span)?;
            return Ok(Item::Def(Def { name, name_span, rec, kind: DefKind::Function { params, ret: None, body }, span }));
        }
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
                name_span,
                rec: false,
                kind: DefKind::Value(body),
                span,
            }))
        } else {
            let params = self.parse_params()?;
            if params.is_empty() && self.kind() == &TokKind::Arrow {
                return Err(self.error(Some("E109"), "function requires a parameter"));
            }
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
                name_span,
                rec,
                kind: DefKind::Function { params, ret, body },
                span,
            }))
        }
    }

    fn parse_sig_def(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::Def)?;
        let name_span = self.span();
        let name = self.expect_lower_name("a declaration name")?;
        let mut type_params = Vec::new();
        if self.kind() == &TokKind::Lt {
            self.bump();
            loop {
                let span = self.span();
                type_params.push(DeclaredName { name: self.expect_upper_name("a type parameter")?, span });
                if self.kind() != &TokKind::Comma { break; }
                self.bump();
            }
            self.expect(&TokKind::Gt)?;
        }
        if self.kind() != &TokKind::LParen {
            return Err(self.error(None, "signature parameters require type annotations"));
        }
        let params = self.parse_params()?;
        self.expect(&TokKind::Colon)?;
        let ret = Some(self.parse_type()?);
        Ok(Item::SigDef(SigDef {
            name,
                name_span,
            type_params,
            params,
            ret,
            span,
        }))
    }

    fn parse_type_def(&mut self, context: TypeContext) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwType)?;
        let name_span = self.span();
        let name = self.expect_upper_name("a declaration name")?;
        let mut params = Vec::new();
        if self.kind() == &TokKind::Lt {
            self.bump();
            loop {
                let span = self.span();
                params.push(DeclaredName { name: self.expect_upper_name("a type parameter")?, span });
                if self.kind() == &TokKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect(&TokKind::Gt)?;
        }
        let ty = if context == TypeContext::Signature {
            if !params.is_empty() || self.kind() == &TokKind::Eq {
                return Err(self.error(None, "signature types are abstract and have no parameters"));
            }
            None
        } else {
            self.expect(&TokKind::Eq)?;
            Some(self.parse_type_body(context)?)
        };
        Ok(Item::TypeDef(TypeDef { name, name_span, params, ty, span }))
    }

    fn parse_module(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwModule)?;
        if self.kind() == &TokKind::KwType {
            // module type NAME = sig ... end
            self.bump();
            let name_span = self.span();
            let name = self.expect_upper_name("a module-type name")?;
            self.expect(&TokKind::Eq)?;
            self.expect(&TokKind::KwSig)?;
            let mut items = Vec::new();
            while self.kind() != &TokKind::KwEnd {
                match self.kind() {
                    TokKind::KwType => items.push(self.parse_type_def(TypeContext::Signature)?),
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
            Ok(Item::ModuleSig(ModuleSig { name, name_span, items, span }))
        } else {
            // module NAME [: SIG] = struct ... end
            let name_span = self.span();
            let name = self.expect_upper_name("a module name")?;
            let sig = if self.kind() == &TokKind::Colon {
                self.bump();
                Some(self.expect_upper_name("a signature name after `:`")?)
            } else {
                return Err(self.error_at(span, Some("E113"), "module requires a signature"));
            };
            self.expect(&TokKind::Eq)?;
            self.expect(&TokKind::KwStruct)?;
            let mut items = Vec::new();
            while self.kind() != &TokKind::KwEnd {
                match self.kind() {
                    TokKind::KwType => items.push(self.parse_type_def(TypeContext::Implementation)?),
                    TokKind::Def => items.push(self.parse_definition()?),
                    TokKind::KwModule => items.push(self.parse_module()?),
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
                name_span,
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

    fn expect_lower_name(&mut self, what: &str) -> Result<String, Diagnostic> {
        self.expect_name_initial(what, char::is_lowercase)
    }

    fn expect_upper_name(&mut self, what: &str) -> Result<String, Diagnostic> {
        self.expect_name_initial(what, char::is_uppercase)
    }

    fn expect_name_initial(&mut self, what: &str, initial: fn(char) -> bool) -> Result<String, Diagnostic> {
        let span = self.span();
        let name = self.expect_name(what)?;
        if !name.chars().next().is_some_and(initial) {
            return Err(self.error_at(span, None, format!("expected {what}")));
        }
        Ok(name)
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
                    if !pname.chars().next().is_some_and(char::is_lowercase) {
                        return Err(self.error_at(span, None, "expected a lowercase parameter name"));
                    }
                    self.expect(&TokKind::Colon)?;
                    let ty = Some(self.parse_type()?);
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
                if !n.chars().next().is_some_and(char::is_lowercase) {
                    return Err(self.error(None, "expected a lowercase parameter name"));
                }
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

    fn parse_type_body(&mut self, context: TypeContext) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        match self.kind() {
            TokKind::LBrace => self.parse_record_type(),
            TokKind::Pipe => {
                let mut constructors = Vec::new();
                while self.kind() == &TokKind::Pipe {
                    self.bump();
                    constructors.push(self.parse_constructor_type()?);
                }
                Ok(TypeExpr { kind: TypeKind::Adt(constructors), span })
            }
            _ if context == TypeContext::Implementation => self.parse_type(),
            _ => {
                let next = self.toks.get(self.pos + 1).map(|token| &token.kind);
                let code = if matches!(next, Some(TokKind::Pipe | TokKind::LParen)) { "E108" } else { "E100" };
                Err(self.error(Some(code), "expected a record or a leading | for an ADT"))
            }
        }
    }

    fn parse_record_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::LBrace)?;
        let mut fields = Vec::new();
        loop {
            let span = self.span();
            let name = DeclaredName { name: self.expect_lower_name("a field name")?, span };
            self.expect(&TokKind::Colon)?;
            fields.push(TypeField { name, ty: self.parse_type()? });
            if self.kind() != &TokKind::Semicolon { break; }
            self.bump();
        }
        self.expect(&TokKind::RBrace)?;
        Ok(TypeExpr { kind: TypeKind::Record(fields), span })
    }

    fn parse_constructor_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        let name = self.expect_upper_name("a constructor name")?;
        if self.kind() != &TokKind::LParen { return Ok(TypeExpr { kind: TypeKind::Name(name), span }); }
        self.bump();
        let args = self.parse_type_list(&TokKind::RParen)?;
        Ok(TypeExpr { kind: TypeKind::Apply(name, args), span })
    }

    fn parse_type_list(&mut self, end: &TokKind) -> Result<Vec<TypeExpr>, Diagnostic> {
        let mut args = vec![self.parse_type()?];
        while self.kind() == &TokKind::Comma {
            self.bump();
            args.push(self.parse_type()?);
        }
        self.expect(end)?;
        Ok(args)
    }

    fn parse_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        let mut name = self.expect_name("a type name")?;
        if !name.chars().next().is_some_and(char::is_uppercase) {
            return Err(self.error(None, "type names start with an uppercase letter"));
        }
        if self.kind() == &TokKind::Dot {
            self.bump();
            name.push('.');
            name.push_str(&self.expect_upper_name("a qualified type name")?);
            return Ok(TypeExpr { kind: TypeKind::Name(name), span });
        }
        if self.kind() != &TokKind::Lt { return Ok(TypeExpr { kind: TypeKind::Name(name), span }); }
        self.bump();
        if name == "Fn" { return self.parse_function_type(span); }
        Ok(TypeExpr { kind: TypeKind::Apply(name, self.parse_type_list(&TokKind::Gt)?), span })
    }

    fn parse_function_type(&mut self, span: Span) -> Result<TypeExpr, Diagnostic> {
        let params = if self.kind() == &TokKind::LParen {
            let span = self.span();
            self.bump();
            TypeExpr { kind: TypeKind::Group(self.parse_type_list(&TokKind::RParen)?), span }
        } else {
            self.parse_type()?
        };
        self.expect(&TokKind::Comma)?;
        let result = self.parse_type()?;
        self.expect(&TokKind::Gt)?;
        Ok(TypeExpr { kind: TypeKind::Apply("Fn".into(), vec![params, result]), span })
    }

    // --- expressions ---

    fn parse_expr(&mut self) -> Result<Node, Diagnostic> {
        match self.kind() {
            TokKind::KwIf => self.parse_branch(),
            TokKind::KwFn => self.parse_fun(),
            TokKind::KwLet => self.parse_let(false),
            TokKind::KwLetQ => self.parse_let(true),
            TokKind::KwDoQ => self.parse_do(),
            _ => self.parse_pipe(),
        }
    }

    /// Parse an expression that must not itself contain a branch
    /// (§20: branches cannot nest). Used for every component of a branch.
    fn parse_branch_expr(&mut self) -> Result<Node, Diagnostic> {
        if self.kind() == &TokKind::KwIf {
            return Err(self.error(Some("E101"), "branches cannot be nested inside another branch"));
        }
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
            if constructor_head(&right) {
                return Err(self.error_at(right.span, Some("E106"), "constructor cannot be a pipe target"));
            }
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
        let f = self.parse_primary()?;
        let mut args = Vec::new();
        while self.atom_start() {
            args.push(self.parse_primary()?);
        }
        if args.is_empty() {
            Ok(f)
        } else {
            let span = f.span;
            if constructor_head(&f) {
                return Err(self.error_at(span, Some("E106"), "constructor arguments require parentheses"));
            }
            Ok(Node { kind: NodeKind::Apply(Box::new(f), args), span })
        }
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
        )
    }

    fn parse_primary(&mut self) -> Result<Node, Diagnostic> {
        let mut node = self.parse_atom()?;
        while self.kind() == &TokKind::Dot {
            self.bump();
            let field = self.expect_lower_name("a field name")?;
            let span = node.span;
            node = Node { kind: NodeKind::Field(Box::new(node), field), span };
        }
        Ok(node)
    }

    fn parse_atom(&mut self) -> Result<Node, Diagnostic> {
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
        if !name.chars().next().is_some_and(char::is_alphabetic) && !name.starts_with("__") {
            return Err(self.error_at(span, None, "expected a value or constructor name"));
        }
        let mut node = Node {
            kind: NodeKind::Name(name.clone()),
            span,
        };
        while self.kind() == &TokKind::Dot {
            self.bump();
            let field = self.expect_lower_name("a field name after `.`")?;
            node = Node { kind: NodeKind::Field(Box::new(node), field), span };
        }
        if !constructor_head(&node) {
            return Ok(node);
        }
        if name == "Ok" && self.kind() == &TokKind::Unit {
            let unit = Node { kind: NodeKind::Unit, span: self.span() };
            self.bump();
            return Ok(Node { kind: NodeKind::Apply(Box::new(node), vec![unit]), span });
        }
        if self.kind() == &TokKind::LParen {
            self.bump();
            let mut args = Vec::new();
            if self.kind() == &TokKind::RParen && name == "Ok" {
                let unit = Node { kind: NodeKind::Unit, span: self.span() };
                self.bump();
                return Ok(Node { kind: NodeKind::Apply(Box::new(node), vec![unit]), span });
            }
            loop {
                args.push(self.parse_expr()?);
                if self.kind() != &TokKind::Comma { break; }
                self.bump();
            }
            self.expect(&TokKind::RParen)?;
            return Ok(Node { kind: NodeKind::Call(Box::new(node), args), span });
        }
        Ok(node)
    }

    fn parse_record_fields(&mut self) -> Result<Vec<(DeclaredName, Node)>, Diagnostic> {
        let mut fields = Vec::new();
        if self.kind() == &TokKind::RBrace {
            return Err(self.error(None, "record requires a field"));
        }
        loop {
            let span = self.span();
            let fname = DeclaredName { name: self.expect_lower_name("a field name")?, span };
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
            TokKind::Pipe => self.parse_pattern_branch(span, scrutinee),
            other => Err(self.error(
                None,
                format!("expected `=>` or `|` after `if`, found {other}"),
            )),
        }
    }

    fn parse_arm_function(&mut self, span: Span) -> Result<(Vec<Param>, Node), Diagnostic> {
        // This name cannot occur in source, so lowering cannot capture a user binding.
        let name = "$argument".to_string();
        let scrutinee = Node { kind: NodeKind::Name(name.clone()), span };
        let body = self.parse_pattern_branch(span, scrutinee)?;
        Ok((vec![Param { name, ty: None, span }], body))
    }

    fn parse_pattern_branch(&mut self, span: Span, scrutinee: Node) -> Result<Node, Diagnostic> {
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
            self.record_fallback_end();
        }
        if arms.is_empty() {
            return Err(self.error_at(span, None, "pattern branch requires at least one arm"));
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
                self.record_fallback_end();
            }
        }
        Ok(Node {
            kind: NodeKind::CondBranch { arms, fallback },
            span,
        })
    }

    fn record_fallback_end(&mut self) {
        if self.kind() == &TokKind::Pipe {
            self.branches_after_fallback.push(self.span());
        }
    }

    fn parse_fun(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump(); // `fn`
        if self.kind() == &TokKind::Pipe {
            let (params, body) = self.parse_arm_function(span)?;
            return Ok(Node { kind: NodeKind::Fn { params, body: Box::new(body) }, span });
        }
        let params = self.parse_params()?;
        if params.is_empty() { return Err(self.error(None, "lambda requires a parameter")); }
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
                self.bump();
                if self.kind() == &TokKind::LParen && name.chars().next().is_some_and(char::is_uppercase) {
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
                if self.kind() == &TokKind::RBrace { return Err(self.error(None, "record pattern requires a field")); }
                let mut fields = Vec::new();
                if self.kind() != &TokKind::RBrace {
                    loop {
                        let binding_span = self.span();
                        let fname = self.expect_lower_name("a field name")?;
                        let pat = if self.kind() == &TokKind::Eq {
                            self.bump();
                            Some(self.parse_pattern()?)
                        } else {
                            Some(Pattern { kind: PatternKind::Name(fname.clone()), span: binding_span })
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
            TokKind::Minus => {
                self.bump();
                let kind = match self.kind() {
                    TokKind::Int(value) => NodeKind::Int(-value),
                    TokKind::Float(value) => NodeKind::Float(-value),
                    _ => return Err(self.error(None, "expected numeric literal after -")),
                };
                self.bump();
                PatternKind::Literal(Box::new(Node { kind, span }))
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

fn constructor_head(node: &Node) -> bool {
    match &node.kind {
        NodeKind::Name(name) => name.chars().next().is_some_and(char::is_uppercase),
        NodeKind::Call(head, _) | NodeKind::Apply(head, _) => constructor_head(head),
        _ => false,
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
    if ast.items.len() != 1 {
        return None;
    }
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
        NodeKind::Float(x) => {
            let text = x.to_string();
            if text.contains('.') { text } else { format!("{text}.0") }
        },
        NodeKind::Char(c) => source_print::literal(&c.to_string(), '\''),
        NodeKind::Str(s) => source_print::literal(s, '"'),
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
                if matches!(a.kind, NodeKind::Apply(..)) || matches!(&a.kind, NodeKind::Name(name) if name.chars().next().is_some_and(char::is_uppercase)) { s.push_str(&format!("({})", canonical(a))); }
                else { s.push_str(&canonical(a)); }
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
        NodeKind::Field(l, r) => {
            if matches!(l.kind, NodeKind::Apply(..)) { format!("({}).{r}", canonical(l)) }
            else { format!("{}.{r}", canonical(l)) }
        },
        NodeKind::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(n, v)| format!("{} = {}", n.name, canonical(v)))
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
            format!("(fn{})", source_print::function_tail(params, None, body))
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

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool { self.kind == other.kind }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool { self.kind == other.kind }
}

impl PartialEq for Param {
    fn eq(&self, other: &Self) -> bool { self.name == other.name && self.ty == other.ty }
}

#[cfg(test)]
#[path = "../tests/unit/parser.rs"]
mod tests;

#[path = "source_print.rs"]
mod source_print;
pub use source_print::canonical_program;
