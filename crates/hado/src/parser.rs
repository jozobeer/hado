//! Recursive-descent parser for the hado grammar of 付録C, §5 and §7.
//!
//! Covers the full v0.1 syntax: named definitions (`def` / `def rec`),
//! local bindings (`let` / `let p? =` / `do`), anonymous functions (`fn`),
//! the three branch forms (Boolean branch, condition column, pattern
//! branch), patterns, records, list literals, modules (`sig` / `struct`)
//! and type definitions. Every node carries a `Span`.
//!
//! Precedence (§7), high to low: application, unary `-`/`not`,
//! `* / %`, `+ -`, comparison (non-associative), `&&`, `||`, `|>`.
//! The parser ignores newlines (付録C:2364).

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
/// node's leftmost token, except that a pipeline uses its own `|>` token so
/// nested left operands and pipeline results have distinct type-table keys.
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
    Field(Box<Node>, FieldName),
    Record(Vec<(DeclaredName, Node)>),
    NamedConstructor(DeclaredName, Vec<(DeclaredName, Node)>),
    List(Vec<Node>, Option<Box<Node>>),
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
        scrutinees: Vec<Node>,
        arms: Vec<(Vec<Pattern>, Node)>,
        fallback: Option<Box<Node>>,
    },
    Pipe(Box<Node>, Box<Node>),
    With {
        handler: DeclaredName,
        args: Vec<Node>,
        body: Box<Node>,
    },
    Resume,
}

/// A pattern (`pattern :=` in 付録C).
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
    Record(Vec<(DeclaredName, Option<Pattern>)>),
    NamedConstructor(DeclaredName, Vec<(DeclaredName, Option<Pattern>)>),
    Literal(Box<Node>),
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(list) = source_print::list_pattern(self) {
            return write!(f, "{list}");
        }
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
            PatternKind::Record(fields) | PatternKind::NamedConstructor(_, fields) => {
                if let PatternKind::NamedConstructor(name, _) = &self.kind { write!(f, "{} ", name.name)?; }
                let inner = fields
                    .iter()
                    .map(|(n, p)| match p {
                        Some(p) if matches!(&p.kind, PatternKind::Name(name) if name == &n.name) => n.name.clone(),
                        Some(p) => format!("{} = {p}", n.name),
                        None => n.name.clone(),
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
pub struct FieldName {
    pub name: String,
    pub dot: Span,
    pub span: Span,
}

impl PartialEq for FieldName {
    fn eq(&self, other: &Self) -> bool { self.name == other.name }
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

/// A type expression (see §13, §16, §47, §20).
#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
    pub name_span: Span,
    pub effects: Option<EffectRow>,
}

impl TypeExpr {
    fn of(kind: TypeKind, span: Span) -> Self {
        Self { kind, span, name_span: span, effects: None }
    }
}

impl PartialEq for TypeExpr {
    fn eq(&self, other: &Self) -> bool { self.kind == other.kind && self.effects == other.effects }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EffectRow {
    Pure,
    Row(Vec<TypeExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Name(String),
    Apply(String, Vec<TypeExpr>),
    /// `(Int, Int)` argument group inside `Fn(...)`.
    Group(Vec<TypeExpr>),
    Record(Vec<TypeField>),
    Adt(Vec<ConstructorDecl>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorDecl {
    pub name: DeclaredName,
    pub payload: ConstructorPayload,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstructorPayload {
    Nullary,
    Positional(Vec<TypeExpr>),
    Named(Vec<TypeField>),
}

impl ConstructorPayload {
    pub fn types(&self) -> impl Iterator<Item = &TypeExpr> {
        let (positional, named): (&[TypeExpr], &[TypeField]) = match self {
            Self::Nullary => (&[], &[]),
            Self::Positional(types) => (types, &[]),
            Self::Named(fields) => (&[], fields),
        };
        positional.iter().chain(named.iter().map(|field| &field.ty))
    }

    pub fn types_mut(&mut self) -> impl Iterator<Item = &mut TypeExpr> {
        let (positional, named): (&mut [TypeExpr], &mut [TypeField]) = match self {
            Self::Nullary => (&mut [], &mut []),
            Self::Positional(types) => (types, &mut []),
            Self::Named(fields) => (&mut [], fields),
        };
        positional.iter_mut().chain(named.iter_mut().map(|field| &mut field.ty))
    }
}

#[derive(Debug, Clone)]
pub struct NameSite {
    pub start: Span,
    pub parts: Vec<DeclaredName>,
}

/// A parsed program: an ordered list of top-level items.
#[derive(Debug, Clone)]
pub struct Ast {
    pub name_sites: Vec<NameSite>,
    pub items: Vec<Item>,
    pub span: Span,
    pub library: Option<Span>,
    pub branches_after_fallback: Vec<Span>,
    pub branch_anchors: Vec<(Span, Span)>,
    /// §57: (branch start, first `|`, first token after the branch) for every
    /// pattern branch, so a rewrite can address the whole branch.
    pub branch_spans: Vec<(Span, Span, Span)>,
    pub equality_anchors: Vec<(Span, Span, Span)>,
    pub qualified_type_anchors: Vec<(DeclaredName, Span)>,
    pub written_list_patterns: Vec<Span>,
    pub long_condition_functions: Vec<Span>,
}

impl Ast {
    pub(crate) fn append(&mut self, other: Self) {
        self.items.extend(other.items);
        self.library = self.library.or(other.library);
        self.branches_after_fallback.extend(other.branches_after_fallback);
        self.branch_anchors.extend(other.branch_anchors);
        self.branch_spans.extend(other.branch_spans);
        self.equality_anchors.extend(other.equality_anchors);
        self.qualified_type_anchors.extend(other.qualified_type_anchors);
        self.name_sites.extend(other.name_sites);
        self.written_list_patterns.extend(other.written_list_patterns);
        self.long_condition_functions.extend(other.long_condition_functions);
    }


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
    Effect(EffectDecl),
    Handler(HandlerDecl),
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
    pub shared: bool,
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
pub struct FunctorParam {
    pub name: String,
    pub name_span: Span,
    pub contract: String,
    pub contract_span: Span,
}

#[derive(Debug, Clone)]
pub enum ImplementationForm {
    Ordinary,
    Functor(Vec<FunctorParam>),
    Application { functor: DeclaredName, arguments: Vec<DeclaredName> },
}

#[derive(Debug, Clone)]
pub struct ModuleImpl {
    pub form: ImplementationForm,
    pub name: String,
    pub name_span: Span,
    pub sig: String,
    pub sig_span: Span,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Once,
    Abort,
    Again,
}

impl OpKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Abort => "abort",
            Self::Again => "again",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EffectOp {
    pub kind: OpKind,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EffectDecl {
    pub name: String,
    pub name_span: Span,
    pub type_params: Vec<DeclaredName>,
    pub ops: Vec<EffectOp>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum HandlerArm {
    Operation {
        kind: OpKind,
        name: String,
        name_span: Span,
        params: Vec<Param>,
        body: Node,
        span: Span,
    },
    Return {
        param: Param,
        body: Node,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct HandlerDecl {
    pub type_params: Vec<DeclaredName>,
    pub effect: String,
    pub effect_span: Span,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub arms: Vec<HandlerArm>,
    pub span: Span,
}

/// A second position a diagnostic points at (§56). E205 names the first
/// declaration of a duplicate, E602 names the contract's member. The line and
/// column are in the checked coordinates; `project::source_diagnostic` rewrites
/// them into the file that holds them, exactly like the diagnostic's own anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct Related {
    pub path: String,
    pub line: usize,
    pub col: usize,
}

/// The label a code writes before its related position (§56). A code without a
/// label never carries one.
pub(crate) fn related_label(code: &str) -> Option<&'static str> {
    match code {
        "E205" => Some("first declared"),
        "E305" => Some("expected here"),
        "E602" => Some("declared in the contract"),
        _ => None,
    }
}

/// One-line diagnostic (`<path>:<line>:<col>: error[CODE]: <message>` for
/// E1xx, plain `error: <message>` otherwise). A related position stays on the
/// same line, because one run reports one diagnostic (§56).
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub code: Option<String>,
    pub message: String,
    pub related: Option<Related>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(code) = &self.code else {
            return write!(f, "{}:{}:{}: error: {}", self.path, self.line, self.col, self.message);
        };
        write!(f, "{}:{}:{}: error[{code}]: {}", self.path, self.line, self.col, self.message)?;
        if let (Some(related), Some(label)) = (&self.related, related_label(code)) {
            write!(f, "; {label} at {}:{}:{}", related.path, related.line, related.col)?;
        }
        Ok(())
    }
}

/// Parse a source string into the full AST.
pub fn parse_source(src: &str, path: &str) -> Result<Ast, Diagnostic> {
    parse_source_partial(src, path)?.finish(path)
}

pub(crate) struct PartialParse {
    pub ast: Ast,
    diagnostics: Vec<Diagnostic>,
}

impl PartialParse {
    pub(crate) fn finish(self, _path: &str) -> Result<Ast, Diagnostic> {
        match self.diagnostics.into_iter().min_by_key(crate::diagnostic_order) {
            Some(error) => Err(error),
            None => Ok(self.ast),
        }
    }
}

pub(crate) fn parse_source_partial(src: &str, path: &str) -> Result<PartialParse, Diagnostic> {
    let toks = crate::lexer::lex_full(src).map_err(|e| lex_error_to_diag(e, path))?;
    Ok(parse_tokens_partial(&toks, path))
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
    Diagnostic { related: None,
        path: path.to_string(),
        line: e.line,
        col: e.col,
        code: e.code.map(|s| s.to_string()),
        message: e.message,
    }
}

pub fn parse_tokens(toks: &[Tok], path: &str) -> Result<Ast, Diagnostic> {
    parse_tokens_partial(toks, path).finish(path)
}

fn parse_tokens_partial(toks: &[Tok], path: &str) -> PartialParse {
    let mut parser = Parser {
        toks,
        pos: 0,
        path,
        branches_after_fallback: Vec::new(),
        branch_anchors: Vec::new(),
        branch_spans: Vec::new(),
        equality_anchors: Vec::new(),
        qualified_type_anchors: Vec::new(),
        name_sites: Vec::new(),
        written_list_patterns: Vec::new(),
        long_condition_functions: Vec::new(),
        library: None,
        diagnostics: Vec::new(),
        in_resume_scope: false,
    };
    let ast = parser.parse_program();
    PartialParse { ast, diagnostics: parser.diagnostics }
}

#[derive(Clone, Copy, PartialEq)]
enum TypeContext { TopLevel, Signature, Implementation }

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
    path: &'a str,
    branches_after_fallback: Vec<Span>,
    branch_anchors: Vec<(Span, Span)>,
    branch_spans: Vec<(Span, Span, Span)>,
    equality_anchors: Vec<(Span, Span, Span)>,
    qualified_type_anchors: Vec<(DeclaredName, Span)>,
    name_sites: Vec<NameSite>,
    written_list_patterns: Vec<Span>,
    long_condition_functions: Vec<Span>,
    library: Option<Span>,
    diagnostics: Vec<Diagnostic>,
    in_resume_scope: bool,
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
        Diagnostic { related: None,
            path: self.path.to_string(),
            line: s.line,
            col: s.col,
            code: Some(code.unwrap_or("E100").to_string()),
            message: message.into(),
        }
    }

    // --- top level ---

    fn parse_program(&mut self) -> Ast {
        let span = self.span();
        let items = self.parse_items(TypeContext::TopLevel);
        Ast { library: self.library, items, span, name_sites: self.name_sites.clone(), branches_after_fallback: self.branches_after_fallback.clone(), branch_anchors: self.branch_anchors.clone(), branch_spans: self.branch_spans.clone(), equality_anchors: self.equality_anchors.clone(), qualified_type_anchors: self.qualified_type_anchors.clone(), written_list_patterns: self.written_list_patterns.clone(), long_condition_functions: self.long_condition_functions.clone() }
    }

    fn section_boundary(&self) -> bool {
        matches!(self.kind(), TokKind::KwModule | TokKind::KwImplement | TokKind::KwEffect | TokKind::KwHandler | TokKind::Eof)
    }

    fn parse_items(&mut self, context: TypeContext) -> Vec<Item> {
        let mut items = Vec::new();
        // 付録C: outer_item は最初の見出しより前にだけ現れる。
        let mut outer = context == TypeContext::TopLevel;
        while self.kind() != &TokKind::Eof {
            if context != TypeContext::TopLevel && self.section_boundary() { break; }
            if outer && matches!(self.kind(), TokKind::KwModule | TokKind::KwImplement | TokKind::KwEffect | TokKind::KwHandler) {
                outer = false;
            }
            let start = self.pos;
            let item = match self.kind() {
                TokKind::Def if context == TypeContext::Signature => self.parse_sig_def(),
                TokKind::Def => self.parse_definition(),
                TokKind::KwShared if context == TypeContext::Implementation => {
                    self.bump();
                    if self.kind() != &TokKind::KwType {
                        Err(self.error(None, "expected type after shared"))
                    } else {
                        self.parse_type_def(context, true)
                    }
                }
                TokKind::KwType => self.parse_type_def(context, false),
                TokKind::KwModule => self.parse_module(),
                TokKind::KwImplement => self.parse_implementation(),
                TokKind::KwEffect => self.parse_effect(),
                TokKind::KwHandler => self.parse_handler(),
                TokKind::KwLibrary if outer => {
                    if self.library.is_none() { self.library = Some(self.span()); }
                    self.bump();
                    continue;
                }
                other => Err(self.error(None, format!("expected a declaration, found {other}"))),
            };
            match item {
                Ok(item) => items.push(item),
                Err(error) => {
                    self.diagnostics.push(error);
                    self.recover_declaration(start);
                }
            }
        }
        items
    }

    fn recover_declaration(&mut self, start: usize) {
        let failed_at = self.pos;
        self.pos = start;
        while self.kind() != &TokKind::Eof {
            if self.pos > start && self.pos >= failed_at
                && matches!(self.kind(), TokKind::Def | TokKind::KwType | TokKind::KwShared | TokKind::KwModule | TokKind::KwImplement | TokKind::KwEffect | TokKind::KwHandler) {
                break;
            }
            self.bump();
        }
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
            let params = self.parse_params(false)?;
            if params.is_empty() {
                let code = if self.kind() == &TokKind::Arrow { "E109" } else { "E100" };
                return Err(self.error(Some(code), "function requires a parameter"));
            }
            let ret = if self.kind() == &TokKind::Colon {
                self.bump();
                Some(self.parse_result()?)
            } else {
                None
            };
            let body = self.parse_function_body(span)?;
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
        let type_params = self.parse_type_parameters()?;
        let name_span = self.span();
        let name = self.expect_lower_name("a declaration name")?;
        let params = match self.kind() {
            TokKind::Colon => Vec::new(),
            TokKind::LParen => self.parse_params(true)?,
            _ => return Err(self.error(None, "expected a value type or annotated contract parameters")),
        };
        self.expect(&TokKind::Colon)?;
        let ret = Some(self.parse_result()?);
        Ok(Item::SigDef(SigDef {
            name,
                name_span,
            type_params,
            params,
            ret,
            span,
        }))
    }

    fn parse_type_def(&mut self, context: TypeContext, shared: bool) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwType)?;
        let params = self.parse_optional_type_params()?;
        let name_span = self.span();
        let name = self.expect_upper_name("a declaration name")?;
        let ty = if context == TypeContext::Signature {
            if self.kind() == &TokKind::Eq {
                return Err(self.error(None, "signature types are abstract and have no body"));
            }
            None
        } else {
            self.expect(&TokKind::Eq)?;
            Some(self.parse_type_body(context, shared)?)
        };
        Ok(Item::TypeDef(TypeDef { name, name_span, params, ty, shared, span }))
    }

    fn parse_module(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwModule)?;
        let name_span = self.span();
        let name = self.expect_upper_name("a contract name")?;
        let items = self.parse_items(TypeContext::Signature);
        Ok(Item::ModuleSig(ModuleSig { name, name_span, items, span }))
    }

    fn parse_implementation(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwImplement)?;
        let sig_span = self.span();
        let sig = self.parse_upper_reference("a contract name")?;
        self.expect(&TokKind::KwAs)?;
        let name_span = self.span();
        let name = self.expect_upper_name("an implementation name")?;
        let mut module = ModuleImpl {
            name, name_span, sig, sig_span, span,
            form: self.parse_implementation_form()?, items: Vec::new(),
        };
        if matches!(&module.form, ImplementationForm::Application { .. }) {
            if !self.section_boundary() {
                return Err(self.error(Some("E100"), "functor applications cannot have a body"));
            }
        } else {
            module.items = self.parse_items(TypeContext::Implementation);
        }
        Ok(Item::ModuleImpl(module))
    }

    fn parse_optional_type_params(&mut self) -> Result<Vec<DeclaredName>, Diagnostic> {
        if self.kind() == &TokKind::LParen {
            return self.parse_type_parameters();
        }
        if self.single_type_var() {
            let span = self.span();
            return Ok(vec![DeclaredName { name: self.expect_upper_name("a type parameter")?, span }]);
        }
        Ok(Vec::new())
    }

    fn single_type_var(&self) -> bool {
        let TokKind::Name(name) = self.kind() else { return false };
        if !name.starts_with(char::is_uppercase) { return false; }
        match self.toks.get(self.pos + 1).map(|token| &token.kind) {
            Some(TokKind::Name(next)) if next.starts_with(char::is_uppercase) => true,
            Some(TokKind::Name(_)) => self.toks.get(self.pos + 2).is_some_and(|token| token.kind == TokKind::DoubleColon),
            _ => false,
        }
    }

    fn parse_op_kind(&mut self) -> Option<OpKind> {
        let kind = match self.kind() {
            TokKind::KwOnce => OpKind::Once,
            TokKind::KwAbort => OpKind::Abort,
            TokKind::KwAgain => OpKind::Again,
            _ => return None,
        };
        self.bump();
        Some(kind)
    }

    fn parse_effect(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwEffect)?;
        let type_params = self.parse_optional_type_params()?;
        let name_span = self.span();
        let name = self.expect_upper_name("an effect name")?;
        let mut ops = Vec::new();
        while let Some(kind) = self.parse_op_kind() {
            ops.push(self.parse_effect_op(kind)?);
        }
        Ok(Item::Effect(EffectDecl { name, name_span, type_params, ops, span }))
    }

    fn parse_effect_op(&mut self, kind: OpKind) -> Result<EffectOp, Diagnostic> {
        let span = self.span();
        let name_span = self.span();
        let name = self.expect_lower_name("an operation name")?;
        let params = if self.kind() == &TokKind::LParen {
            self.parse_typed_params(true)?
        } else {
            Vec::new()
        };
        let ret = if self.kind() == &TokKind::Colon {
            if kind == OpKind::Abort {
                return Err(self.error(Some("E123"), "abort operations cannot have a return type"));
            }
            self.bump();
            Some(self.parse_result()?)
        } else {
            None
        };
        Ok(EffectOp { kind, name, name_span, params, ret, span })
    }

    fn parse_typed_params(&mut self, contract: bool) -> Result<Vec<Param>, Diagnostic> {
        self.expect(&TokKind::LParen)?;
        let mut params = vec![self.parse_typed_param(contract)?];
        while self.kind() == &TokKind::Comma {
            self.bump();
            params.push(self.parse_typed_param(contract)?);
        }
        self.expect(&TokKind::RParen)?;
        Ok(params)
    }

    fn parse_typed_param(&mut self, contract: bool) -> Result<Param, Diagnostic> {
        let span = self.span();
        let name = self.expect_parameter_name(contract)?;
        self.expect(&TokKind::Colon)?;
        Ok(Param { name, ty: Some(self.parse_type()?), span })
    }

    fn parse_handler(&mut self) -> Result<Item, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwHandler)?;
        let type_params = self.parse_optional_type_params()?;
        let effect_span = self.span();
        let effect = self.parse_upper_reference("an effect name")?;
        self.expect(&TokKind::KwAs)?;
        let name_span = self.span();
        let name = self.expect_upper_name("a handler name")?;
        let params = if self.kind() == &TokKind::LParen {
            self.parse_typed_params(false)?
        } else {
            Vec::new()
        };
        let mut arms = Vec::new();
        loop {
            if self.kind() == &TokKind::KwReturn {
                arms.push(self.parse_return_arm()?);
                continue;
            }
            let Some(kind) = self.parse_op_kind() else { break };
            arms.push(self.parse_handler_op(kind)?);
        }
        Ok(Item::Handler(HandlerDecl { type_params, effect, effect_span, name, name_span, params, arms, span }))
    }

    fn parse_handler_op(&mut self, kind: OpKind) -> Result<HandlerArm, Diagnostic> {
        let span = self.span();
        let name_span = self.span();
        let name = self.expect_lower_name("an operation name")?;
        let mut params = Vec::new();
        while matches!(self.kind(), TokKind::Name(_)) {
            let span = self.span();
            let name = self.expect_parameter_name(false)?;
            params.push(Param { name, ty: None, span });
        }
        self.expect(&TokKind::Arrow)?;
        let saved = self.in_resume_scope;
        self.in_resume_scope = kind == OpKind::Again;
        let body = self.parse_expr()?;
        self.in_resume_scope = saved;
        Ok(HandlerArm::Operation { kind, name, name_span, params, body, span })
    }

    fn parse_return_arm(&mut self) -> Result<HandlerArm, Diagnostic> {
        let span = self.span();
        self.expect(&TokKind::KwReturn)?;
        let param_span = self.span();
        let name = self.expect_parameter_name(false)?;
        self.expect(&TokKind::Arrow)?;
        let saved = self.in_resume_scope;
        self.in_resume_scope = false;
        let body = self.parse_expr()?;
        self.in_resume_scope = saved;
        Ok(HandlerArm::Return { param: Param { name, ty: None, span: param_span }, body, span })
    }

    fn parse_implementation_form(&mut self) -> Result<ImplementationForm, Diagnostic> {
        match self.kind() {
            TokKind::LParen => self.parse_functor_parameters().map(ImplementationForm::Functor),
            TokKind::Eq => self.parse_functor_application(),
            _ => Ok(ImplementationForm::Ordinary),
        }
    }

    fn parse_functor_parameter(&mut self) -> Result<FunctorParam, Diagnostic> {
        let name_span = self.span();
        let name = self.expect_upper_name("a functor parameter name")?;
        self.expect(&TokKind::Colon)?;
        let contract_span = self.span();
        let contract = self.parse_upper_reference("a parameter contract")?;
        Ok(FunctorParam { name, name_span, contract, contract_span })
    }

    fn parse_functor_parameters(&mut self) -> Result<Vec<FunctorParam>, Diagnostic> {
        self.expect(&TokKind::LParen)?;
        let mut parameters = vec![self.parse_functor_parameter()?];
        while self.kind() == &TokKind::Comma {
            self.bump();
            parameters.push(self.parse_functor_parameter()?);
        }
        self.expect(&TokKind::RParen)?;
        Ok(parameters)
    }

    fn parse_implementation_name(&mut self) -> Result<DeclaredName, Diagnostic> {
        let span = self.span();
        let name = self.parse_upper_reference("an implementation name")?;
        Ok(DeclaredName { name, span })
    }

    fn parse_functor_application(&mut self) -> Result<ImplementationForm, Diagnostic> {
        self.expect(&TokKind::Eq)?;
        let functor = self.parse_implementation_name()?;
        self.expect(&TokKind::LParen)?;
        let mut arguments = vec![self.parse_implementation_name()?];
        while self.kind() == &TokKind::Comma {
            self.bump();
            arguments.push(self.parse_implementation_name()?);
        }
        self.expect(&TokKind::RParen)?;
        Ok(ImplementationForm::Application { functor, arguments })
    }

    fn starts_path(&self) -> bool {
        matches!(self.kind(), TokKind::Name(_))
            && self.toks.get(self.pos + 1).is_some_and(|token| token.kind == TokKind::DoubleColon)
    }

    fn starts_type_reference(&self) -> bool {
        matches!(self.kind(), TokKind::Name(name) if name.starts_with(char::is_uppercase)) || self.starts_path()
    }

    fn parse_upper_reference(&mut self, what: &str) -> Result<String, Diagnostic> {
        let start = self.span();
        let mut parts = Vec::new();
        while self.starts_path() {
            let span = self.span();
            let name = self.expect_lower_name("a namespace name")?;
            parts.push(DeclaredName { name, span });
            self.expect(&TokKind::DoubleColon)?;
        }
        let span = self.span();
        let name = self.expect_upper_name(what)?;
        parts.push(DeclaredName { name, span });
        let name = parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>().join("::");
        self.name_sites.push(NameSite { start, parts });
        Ok(name)
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

    fn expect_parameter_name(&mut self, contract: bool) -> Result<String, Diagnostic> {
        if !contract && matches!(self.kind(), TokKind::Name(name) if name == "_") {
            self.bump();
            Ok("_".into())
        } else {
            self.expect_lower_name("a parameter name")
        }
    }

    fn parse_params(&mut self, contract: bool) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();
        if self.kind() == &TokKind::LParen {
            self.bump();
            if contract && self.kind() == &TokKind::RParen {
                return Err(self.error(None, "contract functions require at least one parameter"));
            }
            if self.kind() != &TokKind::RParen {
                loop {
                    let span = self.span();
                    let pname = self.expect_parameter_name(contract)?;
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
            while matches!(self.kind(), TokKind::Name(_)) {
                let span = self.span();
                let n = self.expect_parameter_name(contract)?;
                params.push(Param {
                    name: n,
                    ty: None,
                    span,
                });
            }
        }
        Ok(params)
    }

    fn parse_type_body(&mut self, context: TypeContext, shared: bool) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        if shared {
            if let Some(anchor) = self.shared_body_anchor() {
                return Err(self.error_at(anchor, Some("E100"), "shared cannot prefix a constructor body"));
            }
        }
        match self.kind() {
            TokKind::LBrace => self.parse_record_type(),
            TokKind::Pipe => {
                let mut constructors = Vec::new();
                while self.kind() == &TokKind::Pipe {
                    self.bump();
                    constructors.push(self.parse_constructor_type()?);
                }
                Ok(TypeExpr::of(TypeKind::Adt(constructors), span))
            }
            TokKind::Name(name) if name.starts_with(char::is_uppercase)
                && self.toks.get(self.pos + 1).is_some_and(|token| token.kind == TokKind::LBrace) => {
                let constructor = self.parse_constructor_type()?;
                if self.kind() == &TokKind::Pipe {
                    return Err(self.error_at(span, Some("E108"), "constructor alternatives require a leading |"));
                }
                Ok(TypeExpr::of(TypeKind::Adt(vec![constructor]), span))
            }
            _ if context == TypeContext::Implementation => self.parse_alias_type(),
            _ => {
                let next = self.toks.get(self.pos + 1).map(|token| &token.kind);
                let code = if matches!(next, Some(TokKind::Pipe | TokKind::LParen)) { "E108" } else { "E100" };
                Err(self.error(Some(code), "expected a record or a leading | for an ADT"))
            }
        }
    }

    fn shared_body_anchor(&self) -> Option<Span> {
        match self.kind() {
            TokKind::LBrace | TokKind::Pipe => Some(self.span()),
            TokKind::Name(name) if name.starts_with(char::is_uppercase) => {
                self.toks.get(self.pos + 1).filter(|token| token.kind == TokKind::LBrace).map(|token| token.span)
            }
            _ => None,
        }
    }

    fn parse_alias_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let prefix = match self.kind() {
            TokKind::Name(name) => Some(TypeExpr::of(TypeKind::Name(name.clone()), self.span())),
            _ => None,
        };
        match (self.parse_type(), prefix) {
            (Err(error), Some(prefix)) => {
                // Preserve the alias anchor for contextual syntax checking. finish rejects this partial AST.
                self.diagnostics.push(error);
                Ok(prefix)
            }
            (result, _) => result,
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
        Ok(TypeExpr::of(TypeKind::Record(fields), span))
    }

    fn parse_constructor_type(&mut self) -> Result<ConstructorDecl, Diagnostic> {
        let span = self.span();
        let name = self.expect_upper_name("a constructor name")?;
        if self.kind() == &TokKind::LBrace {
            let TypeExpr { kind: TypeKind::Record(fields), .. } = self.parse_record_type()? else { unreachable!() };
            return Ok(ConstructorDecl { name: DeclaredName { name, span }, payload: ConstructorPayload::Named(fields) });
        }
        if self.kind() != &TokKind::LParen { return Ok(ConstructorDecl { name: DeclaredName { name, span }, payload: ConstructorPayload::Nullary }); }
        self.bump();
        let args = self.parse_type_list(&TokKind::RParen)?;
        Ok(ConstructorDecl { name: DeclaredName { name, span }, payload: ConstructorPayload::Positional(args) })
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

    fn parse_type_parameter(&mut self) -> Result<DeclaredName, Diagnostic> {
        let span = self.span();
        Ok(DeclaredName { name: self.expect_upper_name("a type parameter")?, span })
    }

    fn parse_type_parameters(&mut self) -> Result<Vec<DeclaredName>, Diagnostic> {
        if self.kind() != &TokKind::LParen {
            return if matches!(self.kind(), TokKind::Name(name) if name.starts_with(char::is_uppercase)) {
                Ok(vec![self.parse_type_parameter()?])
            } else { Ok(Vec::new()) };
        }
        let start = self.span();
        self.bump();
        let mut params = vec![self.parse_type_parameter()?];
        while self.kind() == &TokKind::Comma {
            self.bump();
            params.push(self.parse_type_parameter()?);
        }
        if params.len() < 2 { return Err(self.error_at(start, Some("E100"), "type parameter groups require at least two names")); }
        self.expect(&TokKind::RParen)?;
        Ok(params)
    }

    fn parse_type_ref(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        let mut name = self.parse_upper_reference("a type name")?;
        if self.kind() == &TokKind::Dot {
            self.bump();
            self.qualified_type_anchors.push((DeclaredName { name: name.clone(), span }, self.span()));
            name.push('.');
            name.push_str(&self.expect_upper_name("a qualified type name")?);
        }
        Ok(TypeExpr { kind: TypeKind::Name(name), span, name_span: span, effects: None })
    }

    fn parse_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let mut ty = self.parse_type_atom()?;
        while self.starts_type_reference() {
            let reference = self.parse_type_ref()?;
            let TypeKind::Name(name) = reference.kind else { unreachable!() };
            let span = if name.contains('.') { reference.span } else { ty.span };
            let args = match ty.kind { TypeKind::Group(args) => args, _ => vec![ty] };
            ty = TypeExpr { kind: TypeKind::Apply(name, args), span, name_span: reference.name_span, effects: None };
        }
        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.span();
        match self.kind() {
            TokKind::FnType => {
                self.bump();
                self.parse_function_type(span)
            }
            TokKind::LParen => {
                self.bump();
                let args = self.parse_type_list(&TokKind::RParen)?;
                if args.len() < 2 { return Err(self.error_at(span, Some("E100"), "type argument groups require at least two types")); }
                Ok(TypeExpr::of(TypeKind::Group(args), span))
            }
            _ => self.parse_type_ref(),
        }
    }

    fn parse_function_type(&mut self, span: Span) -> Result<TypeExpr, Diagnostic> {
        self.expect(&TokKind::LParen)?;
        let mut args = vec![self.parse_type()?];
        while self.kind() == &TokKind::Comma {
            self.bump();
            args.push(self.parse_type()?);
        }
        self.expect(&TokKind::Arrow)?;
        let result = self.parse_result()?;
        self.expect(&TokKind::RParen)?;
        let params = if args.len() == 1 { args.remove(0) } else { TypeExpr::of(TypeKind::Group(args), span) };
        Ok(TypeExpr::of(TypeKind::Apply("Fn".into(), vec![params, result]), span))
    }

    fn parse_result(&mut self) -> Result<TypeExpr, Diagnostic> {
        let mut ty = self.parse_type()?;
        ty.effects = self.parse_effect_row()?;
        Ok(ty)
    }

    fn parse_effect_row(&mut self) -> Result<Option<EffectRow>, Diagnostic> {
        match self.kind() {
            TokKind::KwPure => {
                self.bump();
                Ok(Some(EffectRow::Pure))
            }
            TokKind::LBrace => {
                self.bump();
                let mut effects = vec![self.parse_type()?];
                while self.kind() == &TokKind::Comma {
                    self.bump();
                    effects.push(self.parse_type()?);
                }
                self.expect(&TokKind::RBrace)?;
                Ok(Some(EffectRow::Row(effects)))
            }
            _ => Ok(None),
        }
    }

    // --- expressions ---

    fn parse_expr(&mut self) -> Result<Node, Diagnostic> {
        match self.kind() {
            TokKind::KwIf => self.parse_branch(),
            TokKind::KwLet => self.parse_let(),
            TokKind::KwDo => self.parse_do(),
            TokKind::KwWith => self.parse_with(),
            _ => self.parse_pipe(),
        }
    }

    /// Parse an expression that must not itself contain a branch
    /// (§30: branches cannot nest). Used for every component of a branch.
    fn parse_branch_expr(&mut self) -> Result<Node, Diagnostic> {
        if self.kind() == &TokKind::KwIf {
            return Err(self.error(Some("E101"), "branches cannot be nested inside another branch"));
        }
        if self.kind() == &TokKind::KwWith {
            return Err(self.error(None, "with cannot be a branch arm"));
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
            let pipe_span = self.span();
            self.bump();
            let right = self.parse_or()?;
            if constructor_head(&right) {
                return Err(self.error_at(right.span, Some("E106"), "constructor cannot be a pipe target"));
            }
            left = Node {
                kind: NodeKind::Pipe(Box::new(left), Box::new(right)),
                span: pipe_span,
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
                let operator = self.span();
                self.bump();
                let right = self.parse_add()?;
                if matches!(op, BinOp::Eq | BinOp::Ne) {
                    self.equality_anchors.push((left.span, right.span, operator));
                }
                // Comparisons are non-associative (§7): `1 < 2 < 3` is a
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
            // `not` sits in the same precedence tier as unary `-` (§7).
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
                | TokKind::KwFn
                | TokKind::KwResume
        )
    }

    fn parse_primary(&mut self) -> Result<Node, Diagnostic> {
        let mut node = self.parse_atom()?;
        while self.kind() == &TokKind::Dot {
            let dot = self.span();
            self.bump();
            let field_span = self.span();
            let field = self.expect_lower_name("a field name")?;
            let span = node.span;
            node = Node { kind: NodeKind::Field(Box::new(node), FieldName { name: field, dot, span: field_span }), span };
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
            TokKind::KwFn => return self.parse_fun(),
            TokKind::KwResume => {
                if !self.in_resume_scope {
                    return Err(self.error(Some("E124"), "resume is only allowed in an again arm"));
                }
                self.bump();
                NodeKind::Resume
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
                self.parse_list_items()?
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
        let name = if self.starts_path() { self.parse_upper_reference("a constructor or implementation name")? }
            else { self.expect_name("a name")? };
        if !name.chars().next().is_some_and(char::is_alphabetic) && !name.starts_with("__") {
            return Err(self.error_at(span, None, "expected a value or constructor name"));
        }
        let mut node = Node {
            kind: NodeKind::Name(name.clone()),
            span,
        };
        while self.kind() == &TokKind::Dot {
            let dot = self.span();
            self.bump();
            let field_span = self.span();
            let field = self.expect_lower_name("a field name after `.`")?;
            node = Node { kind: NodeKind::Field(Box::new(node), FieldName { name: field, dot, span: field_span }), span };
        }
        if !constructor_head(&node) {
            return Ok(node);
        }
        if self.kind() == &TokKind::LBrace {
            self.bump();
            return Ok(Node { kind: NodeKind::NamedConstructor(DeclaredName { name, span }, self.parse_record_fields()?), span });
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

    fn parse_list_items(&mut self) -> Result<NodeKind, Diagnostic> {
        let mut items = Vec::new();
        let mut tail = None;
        while self.kind() != &TokKind::RBracket {
            items.push(self.parse_expr()?);
            if self.kind() != &TokKind::Comma { break; }
            self.bump();
            if self.kind() == &TokKind::Ellipsis {
                self.bump();
                tail = Some(Box::new(self.parse_expr()?));
                break;
            }
            if self.kind() == &TokKind::RBracket {
                return Err(self.error(None, "expected an expression after comma"));
            }
        }
        self.expect(&TokKind::RBracket)?;
        Ok(NodeKind::List(items, tail))
    }

    fn parse_branch(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump(); // `if`
        if self.kind() == &TokKind::Pipe {
            return self.parse_cond_branch(span);
        }
        let first = self.parse_branch_expr()?;
        if self.kind() == &TokKind::Comma {
            let mut scrutinees = vec![first];
            while self.kind() == &TokKind::Comma {
                self.bump();
                scrutinees.push(self.parse_branch_expr()?);
            }
            match self.kind() {
                TokKind::Pipe => return self.parse_pattern_branch(span, scrutinees),
                other => {
                    return Err(self.error(
                        None,
                        format!("expected `|` after scrutinees, found {other}"),
                    ))
                }
            }
        }
        match self.kind() {
            TokKind::Arrow => {
                self.bump();
                let then = self.parse_branch_expr()?;
                self.expect(&TokKind::Pipe)?;
                let els = self.parse_branch_expr()?;
                Ok(Node {
                    kind: NodeKind::BoolBranch {
                        cond: Box::new(first),
                        then: Box::new(then),
                        els: Box::new(els),
                    },
                    span,
                })
            }
            TokKind::Pipe => self.parse_pattern_branch(span, vec![first]),
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
        let body = self.parse_pattern_branch(span, vec![scrutinee])?;
        Ok((vec![Param { name, ty: None, span }], body))
    }

    fn parse_pattern_branch(&mut self, span: Span, scrutinees: Vec<Node>) -> Result<Node, Diagnostic> {
        let first_pipe = self.span();
        let mut arms = Vec::new();
        let mut fallback = None;
        let columns = scrutinees.len();
        while self.kind() == &TokKind::Pipe {
            let pipe = self.span();
            self.bump();
            let save = self.pos;
            if let Some(arm) = self.try_parse_pattern_arm(pipe, columns)? {
                arms.push(arm);
                continue;
            }
            self.pos = save;
            let fb = self.parse_branch_expr()?;
            self.branch_anchors.push((fb.span, pipe));
            fallback = Some(Box::new(fb));
            self.record_fallback_end();
        }
        if arms.is_empty() {
            return Err(self.error_at(first_pipe, None, "pattern branch requires at least one arm"));
        }
        self.branch_spans.push((span, first_pipe, self.span()));
        Ok(Node {
            kind: NodeKind::PatternBranch {
                scrutinees,
                arms,
                fallback,
            },
            span,
        })
    }

    fn try_parse_pattern_arm(
        &mut self,
        pipe: Span,
        columns: usize,
    ) -> Result<Option<(Vec<Pattern>, Node)>, Diagnostic> {
        let first = match self.parse_pattern() {
            Ok(pattern) => pattern,
            Err(_) => return Ok(None),
        };
        let mut patterns = vec![first];
        while self.kind() == &TokKind::Comma {
            self.bump();
            match self.parse_pattern() {
                // The comma may belong to an enclosing call or `with` list.
                // Only a following `=>` commits the column row; otherwise rewind.
                Ok(pattern) => patterns.push(pattern),
                Err(_) => return Ok(None),
            }
        }
        if self.kind() != &TokKind::Arrow {
            return Ok(None);
        }
        if patterns.len() != columns {
            return Err(self.error_at(
                pipe,
                Some("E125"),
                "pattern column count must match the scrutinees",
            ));
        }
        self.bump();
        let body = self.parse_pattern_arm_body()?;
        self.branch_anchors.extend(patterns.iter().map(|pattern| (pattern.span, pipe)));
        self.branch_anchors.push((body.span, pipe));
        Ok(Some((patterns, body)))
    }

    fn parse_pattern_arm_body(&mut self) -> Result<Node, Diagnostic> {
        if self.kind() == &TokKind::KwIf && self.next_is(&TokKind::Pipe) {
            let span = self.span();
            self.bump();
            let body = self.parse_condition_column(span, true)?;
            if let Some(nested) = contains_nested_branch(&body) {
                return Err(self.error_at(
                    nested,
                    Some("E101"),
                    "branches cannot be nested inside another branch",
                ));
            }
            return Ok(body);
        }
        self.parse_branch_expr()
    }

    fn next_is(&self, kind: &TokKind) -> bool {
        self.toks.get(self.pos + 1).is_some_and(|tok| &tok.kind == kind)
    }

    fn parse_cond_branch(&mut self, span: Span) -> Result<Node, Diagnostic> {
        self.parse_condition_column(span, false)
    }

    fn parse_condition_column(&mut self, span: Span, stop_after_fallback: bool) -> Result<Node, Diagnostic> {
        let mut arms = Vec::new();
        let mut fallback = None;
        let mut branch_first_pipe: Option<Span> = None;
        while self.kind() == &TokKind::Pipe {
            let pipe = self.span();
            if branch_first_pipe.is_none() {
                branch_first_pipe = Some(pipe);
            }
            self.bump();
            let head = self.parse_branch_expr()?;
            if arms.is_empty() && self.kind() != &TokKind::Arrow {
                return Err(self.error_at(pipe, Some("E100"), "condition column requires a condition arm"));
            }
            if self.kind() == &TokKind::Arrow {
                self.bump();
                let body = self.parse_branch_expr()?;
                self.branch_anchors.extend([(head.span, pipe), (body.span, pipe)]);
                arms.push((head, body));
            } else {
                self.branch_anchors.push((head.span, pipe));
                fallback = Some(Box::new(head));
                if stop_after_fallback {
                    break;
                }
                self.record_fallback_end();
            }
        }
        if let Some(first_pipe) = branch_first_pipe {
            self.branch_spans.push((span, first_pipe, self.span()));
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

    fn parse_function_body(&mut self, span: Span) -> Result<Node, Diagnostic> {
        if self.kind() == &TokKind::Pipe {
            return self.parse_cond_branch(span);
        }
        self.expect(&TokKind::Arrow)?;
        let body = self.parse_expr()?;
        if matches!(&body.kind, NodeKind::CondBranch { .. }) {
            self.long_condition_functions.push(span);
        }
        Ok(body)
    }

    fn parse_lambda_param(&mut self, typed: Option<bool>) -> Result<Param, Diagnostic> {
        let span = self.span();
        let name = self.expect_parameter_name(false)?;
        let annotated = self.kind() == &TokKind::Colon;
        if typed.is_some_and(|expected| expected != annotated) {
            return Err(self.error(Some("E100"), "lambda parameters must all have annotations or all omit them"));
        }
        let ty = if annotated {
            self.bump();
            Some(self.parse_type()?)
        } else { None };
        Ok(Param { name, ty, span })
    }

    fn parse_lambda_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = vec![self.parse_lambda_param(None)?];
        let typed = params[0].ty.is_some();
        while self.kind() == &TokKind::Comma {
            self.bump();
            params.push(self.parse_lambda_param(Some(typed))?);
        }
        Ok(params)
    }

    fn parse_fun(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump();
        self.expect(&TokKind::LParen)?;
        let (params, body) = if self.kind() == &TokKind::Pipe {
            self.parse_arm_function(span)?
        } else {
            let params = self.parse_lambda_params()?;
            let body = self.parse_function_body(span)?;
            (params, body)
        };
        self.expect(&TokKind::RParen)?;
        Ok(Node { kind: NodeKind::Fn { params, body: Box::new(body) }, span })
    }

    fn parse_let(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump();
        let pat = self.parse_pattern()?;
        let question = self.kind() == &TokKind::Question;
        if question { self.bump(); }
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
        self.bump();
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

    fn parse_with(&mut self) -> Result<Node, Diagnostic> {
        let span = self.span();
        self.bump();
        let handler_span = self.span();
        let handler = DeclaredName { name: self.parse_upper_reference("a handler name")?, span: handler_span };
        let args = if self.kind() == &TokKind::LParen {
            self.bump();
            let mut args = vec![self.parse_expr()?];
            while self.kind() == &TokKind::Comma {
                self.bump();
                args.push(self.parse_expr()?);
            }
            self.expect(&TokKind::RParen)?;
            args
        } else {
            Vec::new()
        };
        self.expect(&TokKind::DashArrow)?;
        let body = self.parse_expr()?;
        Ok(Node { kind: NodeKind::With { handler, args, body: Box::new(body) }, span })
    }

    // --- patterns ---

    fn parse_list_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let span = self.span();
        self.bump();
        let mut items = Vec::new();
        let mut tail = Pattern { kind: PatternKind::Name("Nil".into()), span };
        while self.kind() != &TokKind::RBracket {
            items.push(self.parse_pattern()?);
            if self.kind() != &TokKind::Comma { break; }
            self.bump();
            if self.kind() == &TokKind::Ellipsis {
                self.bump();
                tail = self.parse_list_rest()?;
                break;
            }
            if self.kind() == &TokKind::RBracket {
                return Err(self.error(None, "expected a pattern after comma"));
            }
        }
        self.expect(&TokKind::RBracket)?;
        for head in items.into_iter().rev() {
            tail = Pattern { kind: PatternKind::Constructor("Cons".into(), vec![head, tail]), span };
        }
        Ok(tail)
    }

    fn parse_list_rest(&mut self) -> Result<Pattern, Diagnostic> {
        if !matches!(self.kind(), TokKind::Name(name) if name == "_" || name.starts_with(char::is_lowercase)) {
            return Err(self.error(None, "List rest pattern must be a name or _"));
        }
        self.parse_pattern()
    }

    fn parse_record_pattern_fields(&mut self) -> Result<Vec<(DeclaredName, Option<Pattern>)>, Diagnostic> {
        self.expect(&TokKind::LBrace)?;
        if self.kind() == &TokKind::RBrace { return Err(self.error(None, "record pattern requires a field")); }
        let mut fields = Vec::new();
        loop {
            let span = self.span();
            let name = self.expect_lower_name("a field name")?;
            let pattern = if self.kind() == &TokKind::Eq {
                self.bump();
                self.parse_pattern()?
            } else {
                Pattern { kind: PatternKind::Name(name.clone()), span }
            };
            fields.push((DeclaredName { name, span }, Some(pattern)));
            if self.kind() != &TokKind::Semicolon { break; }
            self.bump();
        }
        self.expect(&TokKind::RBrace)?;
        Ok(fields)
    }

    fn parse_pattern(&mut self) -> Result<Pattern, Diagnostic> {
        let span = self.span();
        let kind = match self.kind() {
            TokKind::Name(n) if n == "_" => {
                self.bump();
                PatternKind::Underscore
            }
            TokKind::Name(_) => {
                let name = if self.starts_path() { self.parse_upper_reference("a constructor name")? }
                    else { self.expect_name("a pattern name")? };
                if matches!(name.as_str(), "Nil" | "Cons") {
                    self.written_list_patterns.push(span);
                }
                if self.kind() == &TokKind::LBrace && crate::names::is_upper_reference(&name) {
                    PatternKind::NamedConstructor(DeclaredName { name, span }, self.parse_record_pattern_fields()?)
                } else if self.kind() == &TokKind::LParen && crate::names::is_upper_reference(&name) {
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
            TokKind::LBracket => return self.parse_list_pattern(),
            TokKind::LBrace => PatternKind::Record(self.parse_record_pattern_fields()?),
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
            // start of a pattern is always a tuple attempt (§20).
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
        NodeKind::Name(name) => crate::names::is_upper_reference(name),
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
/// Returns the span of the first branch found (§30).
/// A condition column is itself a branch; call `contains_nested_branch`
/// to allow a top-level arm-condition column while still rejecting
/// branches inside it.
fn contains_branch(node: &Node) -> Option<Span> {
    contains_branch_in(node, false)
}

fn contains_nested_branch(node: &Node) -> Option<Span> {
    contains_branch_in(node, true)
}

fn contains_branch_in(node: &Node, allow_arm_conditions: bool) -> Option<Span> {
    match &node.kind {
        NodeKind::BoolBranch { .. } | NodeKind::PatternBranch { .. } => Some(node.span),
        NodeKind::CondBranch { arms, fallback } => {
            let nested = arms
                .iter()
                .find_map(|(condition, body)| {
                    contains_branch_in(condition, false).or_else(|| contains_branch_in(body, false))
                })
                .or_else(|| fallback.as_deref().and_then(|body| contains_branch_in(body, false)));
            if allow_arm_conditions {
                nested
            } else {
                Some(node.span)
            }
        }
        NodeKind::Neg(c) | NodeKind::Not(c) => contains_branch_in(c, false),
        NodeKind::Bin(_, l, r) => contains_branch_in(l, false).or_else(|| contains_branch_in(r, false)),
        NodeKind::Apply(f, args) => {
            contains_branch_in(f, false).or_else(|| args.iter().find_map(|arg| contains_branch_in(arg, false)))
        }
        NodeKind::Call(_, args) => args.iter().find_map(|arg| contains_branch_in(arg, false)),
        NodeKind::Field(l, _) => contains_branch_in(l, false),
        NodeKind::Record(fields) | NodeKind::NamedConstructor(_, fields) => {
            fields.iter().find_map(|(_, v)| contains_branch_in(v, false))
        }
        NodeKind::List(items, tail) => items
            .iter()
            .chain(tail.iter().map(Box::as_ref))
            .find_map(|item| contains_branch_in(item, false)),
        NodeKind::Let { value, body, .. } | NodeKind::LetQ { value, body, .. } => {
            contains_branch_in(value, false).or_else(|| contains_branch_in(body, false))
        }
        NodeKind::DoQ { value, body } => {
            contains_branch_in(value, false).or_else(|| contains_branch_in(body, false))
        }
        NodeKind::With { args, body, .. } => args
            .iter()
            .find_map(|arg| contains_branch_in(arg, false))
            .or_else(|| contains_branch_in(body, false)),
        NodeKind::Fn { body, .. } => contains_branch_in(body, false),
        NodeKind::Pipe(l, r) => contains_branch_in(l, false).or_else(|| contains_branch_in(r, false)),
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

fn comma_nodes(nodes: &[Node]) -> String {
    nodes.iter().map(canonical).collect::<Vec<_>>().join(", ")
}

fn comma_patterns(patterns: &[Pattern]) -> String {
    patterns.iter().map(|pattern| pattern.to_string()).collect::<Vec<_>>().join(", ")
}

pub(super) fn pattern_arm_body(node: &Node) -> String {
    match &node.kind {
        NodeKind::CondBranch { arms, fallback } => format_condition_column(arms, fallback, false),
        _ => canonical(node),
    }
}

fn format_condition_column(
    arms: &[(Node, Node)],
    fallback: &Option<Box<Node>>,
    parenthesize: bool,
) -> String {
    let mut s = if parenthesize { "(if".to_string() } else { "if".to_string() };
    for (condition, body) in arms {
        s.push_str(&format!(" | {} => {}", canonical(condition), canonical(body)));
    }
    if let Some(body) = fallback {
        s.push_str(&format!(" | {}", canonical(body)));
    }
    if parenthesize {
        s.push(')');
    }
    s
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
                if matches!(a.kind, NodeKind::Apply(..)) || matches!(&a.kind, NodeKind::Name(name) if crate::names::is_upper_reference(name)) { s.push_str(&format!("({})", canonical(a))); }
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
            if matches!(l.kind, NodeKind::Apply(..) | NodeKind::Int(_)) { format!("({}).{}", canonical(l), r.name) }
            else { format!("{}.{}", canonical(l), r.name) }
        },
        NodeKind::NamedConstructor(name, fields) => {
            let inner = fields.iter().map(|(field, value)| format!("{} = {}", field.name, canonical(value))).collect::<Vec<_>>().join("; ");
            format!("{} {{ {inner} }}", name.name)
        }
        NodeKind::Record(fields) => {
            let inner = fields
                .iter()
                .map(|(n, v)| format!("{} = {}", n.name, canonical(v)))
                .collect::<Vec<_>>()
                .join("; ");
            format!("{{ {inner} }}")
        }
        NodeKind::List(items, tail) => {
            let mut inner = items
                .iter()
                .map(canonical)
                .collect::<Vec<_>>();
            if let Some(tail) = tail { inner.push(format!("...{}", canonical(tail))); }
            format!("[{}]", inner.join(", "))
        }
        NodeKind::Let { pat, value, body } => {
            format!("(let {pat} = {} -> {})", canonical(value), canonical(body))
        }
        NodeKind::LetQ { pat, value, body } => {
            format!("(let {pat}? = {} -> {})", canonical(value), canonical(body))
        }
        NodeKind::DoQ { value, body } => {
            format!("(do {} -> {})", canonical(value), canonical(body))
        }
        NodeKind::Fn { params, body } => {
            format!("fn({})", source_print::lambda_tail(params, body))
        }
        NodeKind::BoolBranch { cond, then, els } => format!(
            "(if {} => {} | {})",
            canonical(cond),
            canonical(then),
            canonical(els)
        ),
        NodeKind::CondBranch { arms, fallback } => {
            format_condition_column(arms, fallback, true)
        }
        NodeKind::PatternBranch {
            scrutinees,
            arms,
            fallback,
        } => {
            let mut s = format!("(if {}", comma_nodes(scrutinees));
            for (patterns, body) in arms {
                s.push_str(&format!(" | {} => {}", comma_patterns(patterns), pattern_arm_body(body)));
            }
            if let Some(fb) = fallback {
                s.push_str(&format!(" | {}", canonical(fb)));
            }
            s.push(')');
            s
        }
        NodeKind::With { handler, args, body } => {
            let args = if args.is_empty() {
                String::new()
            } else {
                format!("({})", args.iter().map(canonical).collect::<Vec<_>>().join(", "))
            };
            format!("(with {}{} -> {})", handler.name, args, canonical(body))
        }
        NodeKind::Resume => "resume".into(),
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
pub(crate) use source_print::{arm_function as arm_function_source, item_source, list_pattern as list_pattern_source};
