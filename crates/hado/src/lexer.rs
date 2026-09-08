//! Tokenizer for the hado grammar.
//!
//! Follows §43 (literals and `//` line comments) and §44 (operators).
//! Every token carries a 1-based `Span`. Whitespace (including newlines) and
//! line comments are skipped: the parser does not look at newlines (§41).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokKind {
    Def,
    KwRec,
    KwLet,
    KwLetQ,
    KwDoQ,
    KwFn,
    KwIf,
    KwModule,
    KwType,
    KwStruct,
    KwSig,
    KwEnd,
    KwNot,
    True,
    False,
    Name(String),
    Int(i64),
    Float(f64),
    Char(char),
    Str(String),
    Unit,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,
    Ellipsis,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    Eq,
    Arrow,
    DashArrow,
    Pipe,
    PipeArrow,
    Eof,
}

impl std::fmt::Display for TokKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokKind::Def => write!(f, "def"),
            TokKind::KwRec => write!(f, "rec"),
            TokKind::KwLet => write!(f, "let"),
            TokKind::KwLetQ => write!(f, "let?"),
            TokKind::KwDoQ => write!(f, "do?"),
            TokKind::KwFn => write!(f, "fn"),
            TokKind::KwIf => write!(f, "if"),
            TokKind::KwModule => write!(f, "module"),
            TokKind::KwType => write!(f, "type"),
            TokKind::KwStruct => write!(f, "struct"),
            TokKind::KwSig => write!(f, "sig"),
            TokKind::KwEnd => write!(f, "end"),
            TokKind::KwNot => write!(f, "not"),
            TokKind::True => write!(f, "true"),
            TokKind::False => write!(f, "false"),
            TokKind::Name(n) => write!(f, "{n}"),
            TokKind::Int(n) => write!(f, "{n}"),
            TokKind::Float(x) => write!(f, "{x}"),
            TokKind::Char(c) => write!(f, "'{c}'"),
            TokKind::Str(s) => write!(f, "\"{s}\""),
            TokKind::Unit => write!(f, "()"),
            TokKind::LParen => write!(f, "("),
            TokKind::RParen => write!(f, ")"),
            TokKind::LBrace => write!(f, "{{"),
            TokKind::RBrace => write!(f, "}}"),
            TokKind::LBracket => write!(f, "["),
            TokKind::RBracket => write!(f, "]"),
            TokKind::Comma => write!(f, ","),
            TokKind::Semicolon => write!(f, ";"),
            TokKind::Colon => write!(f, ":"),
            TokKind::Dot => write!(f, "."),
            TokKind::Ellipsis => write!(f, "..."),
            TokKind::Plus => write!(f, "+"),
            TokKind::Minus => write!(f, "-"),
            TokKind::Star => write!(f, "*"),
            TokKind::Slash => write!(f, "/"),
            TokKind::Percent => write!(f, "%"),
            TokKind::Lt => write!(f, "<"),
            TokKind::Le => write!(f, "<="),
            TokKind::Gt => write!(f, ">"),
            TokKind::Ge => write!(f, ">="),
            TokKind::EqEq => write!(f, "=="),
            TokKind::Ne => write!(f, "!="),
            TokKind::AndAnd => write!(f, "&&"),
            TokKind::OrOr => write!(f, "||"),
            TokKind::Eq => write!(f, "="),
            TokKind::Arrow => write!(f, "=>"),
            TokKind::DashArrow => write!(f, "->"),
            TokKind::Pipe => write!(f, "|"),
            TokKind::PipeArrow => write!(f, "|>"),
            TokKind::Eof => write!(f, "end of input"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tok {
    pub kind: TokKind,
    pub span: Span,
}

/// A lexical error carrying the specification diagnostic code and anchor.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub line: usize,
    pub col: usize,
    pub code: Option<&'static str>,
    pub message: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.code {
            Some(code) => write!(f, "{}:{}: error[{code}]: {}", self.line, self.col, self.message),
            None => write!(f, "{}:{}: error: {}", self.line, self.col, self.message),
        }
    }
}

/// Lex a whole source into a token stream ending in `Eof`.
pub fn lex(src: &str) -> Result<Vec<Tok>, String> {
    lex_full(src).map_err(|e| e.to_string())
}

pub fn lex_full(src: &str) -> Result<Vec<Tok>, LexError> {
    let mut lx = Lexer {
        src,
        i: 0,
        line: 1,
        col: 1,
        tok_start: Span { line: 1, col: 1 },
    };
    let mut toks = Vec::new();
    loop {
        let span = lx.span();
        lx.tok_start = span;
        match lx.peek() {
            None => {
                toks.push(Tok {
                    kind: TokKind::Eof,
                    span,
                });
                break;
            }
            Some(c) if c == ' ' || c == '\t' || c == '\r' || c == '\n' => {
                lx.bump();
            }
            Some('/') if lx.rest().starts_with("//") => {
                while let Some(c) = lx.peek() {
                    if c == '\n' {
                        break;
                    }
                    lx.bump();
                }
            }
            Some(_) => {
                let kind = lx.lex_token()?;
                toks.push(Tok { kind, span });
            }
        }
    }
    Ok(toks)
}

struct Lexer<'a> {
    src: &'a str,
    i: usize,
    line: usize,
    col: usize,
    tok_start: Span,
}

impl Lexer<'_> {
    fn rest(&self) -> &str {
        &self.src[self.i..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn span(&self) -> Span {
        Span {
            line: self.line,
            col: self.col,
        }
    }

    fn err(&self, code: Option<&'static str>, message: impl Into<String>) -> LexError {
        LexError {
            line: self.tok_start.line,
            col: self.tok_start.col,
            code: Some(code.unwrap_or("E100")),
            message: message.into(),
        }
    }

    fn lex_token(&mut self) -> Result<TokKind, LexError> {
        let c = self.peek().expect("lex_token called at EOF");
        match c {
            '(' => {
                self.bump();
                if self.peek() == Some(')') {
                    self.bump();
                    Ok(TokKind::Unit)
                } else {
                    Ok(TokKind::LParen)
                }
            }
            ')' => {
                self.bump();
                Ok(TokKind::RParen)
            }
            '{' => {
                self.bump();
                Ok(TokKind::LBrace)
            }
            '}' => {
                self.bump();
                Ok(TokKind::RBrace)
            }
            '[' => {
                self.bump();
                Ok(TokKind::LBracket)
            }
            ']' => {
                self.bump();
                Ok(TokKind::RBracket)
            }
            ',' => {
                self.bump();
                Ok(TokKind::Comma)
            }
            ';' => {
                self.bump();
                Ok(TokKind::Semicolon)
            }
            ':' => {
                self.bump();
                Ok(TokKind::Colon)
            }
            '.' if self.rest().starts_with("...") => {
                self.bump();
                self.bump();
                self.bump();
                Ok(TokKind::Ellipsis)
            }
            '.' => {
                self.bump();
                if matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    Err(self.err(
                        Some("E103"),
                        "float must have digits before the '.'",
                    ))
                } else {
                    Ok(TokKind::Dot)
                }
            }
            '+' => {
                self.bump();
                Ok(TokKind::Plus)
            }
            '*' => {
                self.bump();
                Ok(TokKind::Star)
            }
            '/' => {
                self.bump();
                Ok(TokKind::Slash)
            }
            '%' => {
                self.bump();
                Ok(TokKind::Percent)
            }
            '#' => Err(self.err(
                Some("E104"),
                "# is not a comment marker; use //",
            )),
            '=' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(TokKind::EqEq)
                } else if self.peek() == Some('>') {
                    self.bump();
                    Ok(TokKind::Arrow)
                } else {
                    Ok(TokKind::Eq)
                }
            }
            '>' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(TokKind::Ge)
                } else {
                    Ok(TokKind::Gt)
                }
            }
            '<' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(TokKind::Le)
                } else {
                    Ok(TokKind::Lt)
                }
            }
            '!' => {
                self.bump();
                if self.peek() == Some('=') {
                    self.bump();
                    Ok(TokKind::Ne)
                } else {
                    Err(self.err(None, "unexpected character: !"))
                }
            }
            '&' => {
                self.bump();
                if self.peek() == Some('&') {
                    self.bump();
                    Ok(TokKind::AndAnd)
                } else {
                    Err(self.err(None, "unexpected character: &"))
                }
            }
            '|' => {
                self.bump();
                if self.peek() == Some('|') {
                    self.bump();
                    Ok(TokKind::OrOr)
                } else if self.peek() == Some('>') {
                    self.bump();
                    Ok(TokKind::PipeArrow)
                } else {
                    Ok(TokKind::Pipe)
                }
            }
            '-' if self.rest().starts_with("--") => Err(self.err(
                Some("E104"),
                "-- is not a comment marker; use //",
            )),
            '-' => {
                self.bump();
                if self.peek() == Some('>') {
                    self.bump();
                    Ok(TokKind::DashArrow)
                } else {
                    Ok(TokKind::Minus)
                }
            }
            '\'' => self.lex_char(),
            '"' => self.lex_string(),
            d if d.is_ascii_digit() => self.lex_number(),
            a if a.is_ascii_alphabetic() || a == '_' => self.lex_word(),
            other => Err(self.err(None, format!("unexpected character: {other}"))),
        }
    }

    fn lex_number(&mut self) -> Result<TokKind, LexError> {
        let start = self.i;
        while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
            self.bump();
        }
        if self.peek() == Some('.') {
            self.bump();
            if matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    self.bump();
                }
                let text = &self.src[start..self.i];
                let value: f64 = text
                    .parse()
                    .map_err(|_| self.err(None, format!("bad float literal: {text}")))?;
                Ok(TokKind::Float(value))
            } else {
                Err(self.err(
                    Some("E103"),
                    "float must have digits after the '.'",
                ))
            }
        } else {
            let text = &self.src[start..self.i];
            let value: i64 = text
                .parse()
                .map_err(|_| self.err(Some("E112"), format!("integer out of range: {text}")))?;
            Ok(TokKind::Int(value))
        }
    }

    fn lex_char(&mut self) -> Result<TokKind, LexError> {
        self.bump(); // opening '
        let c = match self.bump() {
            Some('\\') => match self.bump() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some('\\') => '\\',
                Some('\'') => '\'',
                _ => return Err(self.err(Some("E111"), "unsupported char escape")),
            },
            Some(c) if c != '\'' && c != '\n' => c,
            _ => return Err(self.err(Some("E111"), "expected one character")),
        };
        match self.peek() {
            Some('\'') => {
                self.bump();
                Ok(TokKind::Char(c))
            }
            _ => Err(self.err(
                Some("E111"),
                "char literal must contain exactly one character",
            )),
        }
    }

    fn lex_string(&mut self) -> Result<TokKind, LexError> {
        self.bump(); // opening "
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err(None, "unterminated string literal")),
                Some('"') => {
                    self.bump();
                    return Ok(TokKind::Str(out));
                }
                Some('\\') => {
                    let escape = self.span();
                    self.bump();
                    match self.peek() {
                        Some('n') => {
                            self.bump();
                            out.push('\n');
                        }
                        Some('t') => {
                            self.bump();
                            out.push('\t');
                        }
                        Some('\\') => {
                            self.bump();
                            out.push('\\');
                        }
                        Some('"') => {
                            self.bump();
                            out.push('"');
                        }
                        _ => return Err(LexError {
                            line: escape.line,
                            col: escape.col,
                            code: Some("E110"),
                            message: "unsupported escape in string literal".into(),
                        }),
                    }
                }
                Some(c) => {
                    self.bump();
                    out.push(c);
                }
            }
        }
    }

    fn lex_word(&mut self) -> Result<TokKind, LexError> {
        let start = self.i;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.bump();
        }
        let word = &self.src[start..self.i];
        // `let?` / `do?` are single tokens.
        if (word == "let" || word == "do") && self.peek() == Some('?') {
            self.bump();
            return Ok(if word == "let" {
                TokKind::KwLetQ
            } else {
                TokKind::KwDoQ
            });
        }
        let kind = match word {
            "def" => TokKind::Def,
            "rec" => TokKind::KwRec,
            "let" => TokKind::KwLet,
            "fn" => TokKind::KwFn,
            "if" => TokKind::KwIf,
            "module" => TokKind::KwModule,
            "type" => TokKind::KwType,
            "struct" => TokKind::KwStruct,
            "sig" => TokKind::KwSig,
            "end" => TokKind::KwEnd,
            "true" => TokKind::True,
            "false" => TokKind::False,
            "not" => TokKind::KwNot,
            word => TokKind::Name(word.to_string()),
        };
        Ok(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn lex_ops_and_ints() {
        assert_eq!(
            kinds("1 + 2 * 3"),
            vec![
                TokKind::Int(1),
                TokKind::Plus,
                TokKind::Int(2),
                TokKind::Star,
                TokKind::Int(3),
                TokKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_comments_and_newlines() {
        assert_eq!(
            kinds("1 // hi\n  + 2"),
            vec![TokKind::Int(1), TokKind::Plus, TokKind::Int(2), TokKind::Eof]
        );
    }

    #[test]
    fn lex_multichar_ops() {
        assert_eq!(
            kinds("1 <= 2 && 3 >= 4 || 5 == 6 != 7"),
            vec![
                TokKind::Int(1),
                TokKind::Le,
                TokKind::Int(2),
                TokKind::AndAnd,
                TokKind::Int(3),
                TokKind::Ge,
                TokKind::Int(4),
                TokKind::OrOr,
                TokKind::Int(5),
                TokKind::EqEq,
                TokKind::Int(6),
                TokKind::Ne,
                TokKind::Int(7),
                TokKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_unary_and_neg() {
        assert_eq!(
            kinds("- - 2"),
            vec![TokKind::Minus, TokKind::Minus, TokKind::Int(2), TokKind::Eof]
        );
    }

    #[test]
    fn lex_e103_dot_forms() {
        assert!(lex(".5").is_err());
        assert!(lex("5.").is_err());
        assert!(lex("1.5").is_ok());
    }

    #[test]
    fn lex_e104_hash_comment() {
        let e = lex("# c").unwrap_err();
        assert!(e.contains("E104"), "got: {e}");
    }

    #[test]
    fn spans_are_one_based() {
        let toks = lex("1\n  + 2").unwrap();
        let plus = &toks[1];
        assert_eq!(plus.span, Span { line: 2, col: 3 });
    }
}
