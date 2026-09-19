//! Tokenizer for the hado grammar.
//!
//! Follows §5 (literals and `//` line comments) and §7 (operators).
//! Every token carries a 1-based `Span`. Whitespace (including newlines) and
//! line comments are skipped: the parser does not look at newlines (付録C).

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
    Question,
    KwDo,
    KwFn,
    FnType,
    KwIf,
    KwModule,
    KwType,
    KwImplement,
    KwEffect,
    KwHandler,
    KwWith,
    KwResume,
    KwReturn,
    KwOnce,
    KwAbort,
    KwAgain,
    KwAs,
    KwLibrary,
    KwShared,
    KwPure,
    KwNot,
    True,
    False,
    Name(String),
    Int(i64),
    Float(f64),
    Char(char),
    Str(String),
    Void,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    DoubleColon,
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
            TokKind::Question => write!(f, "?"),
            TokKind::KwDo => write!(f, "do"),
            TokKind::KwFn => write!(f, "fn"),
            TokKind::FnType => write!(f, "Fn"),
            TokKind::KwIf => write!(f, "if"),
            TokKind::KwModule => write!(f, "module"),
            TokKind::KwType => write!(f, "type"),
            TokKind::KwImplement => write!(f, "implement"),
            TokKind::KwEffect => write!(f, "effect"),
            TokKind::KwHandler => write!(f, "handler"),
            TokKind::KwWith => write!(f, "with"),
            TokKind::KwResume => write!(f, "resume"),
            TokKind::KwReturn => write!(f, "return"),
            TokKind::KwOnce => write!(f, "once"),
            TokKind::KwAbort => write!(f, "abort"),
            TokKind::KwAgain => write!(f, "again"),
            TokKind::KwAs => write!(f, "as"),
            TokKind::KwLibrary => write!(f, "library"),
            TokKind::KwShared => write!(f, "shared"),
            TokKind::KwPure => write!(f, "Pure"),
            TokKind::KwNot => write!(f, "not"),
            TokKind::True => write!(f, "true"),
            TokKind::False => write!(f, "false"),
            TokKind::Name(n) => write!(f, "{n}"),
            TokKind::Int(n) => write!(f, "{n}"),
            TokKind::Float(x) => write!(f, "{x}"),
            TokKind::Char(c) => write!(f, "'{c}'"),
            TokKind::Str(s) => write!(f, "\"{s}\""),
            TokKind::Void => write!(f, "()"),
            TokKind::LParen => write!(f, "("),
            TokKind::RParen => write!(f, ")"),
            TokKind::LBrace => write!(f, "{{"),
            TokKind::RBrace => write!(f, "}}"),
            TokKind::LBracket => write!(f, "["),
            TokKind::RBracket => write!(f, "]"),
            TokKind::Comma => write!(f, ","),
            TokKind::Semicolon => write!(f, ";"),
            TokKind::Colon => write!(f, ":"),
            TokKind::DoubleColon => write!(f, "::"),
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
            Some(code) => write!(
                f,
                "{}:{}: error[{code}]: {}",
                self.line, self.col, self.message
            ),
            None => write!(f, "{}:{}: error: {}", self.line, self.col, self.message),
        }
    }
}

/// 綴りが 1 文字で決まる記号。2 文字目で意味が変わるものは含めない。
fn single_char_token(c: char) -> Option<TokKind> {
    Some(match c {
        '?' => TokKind::Question,
        ')' => TokKind::RParen,
        '{' => TokKind::LBrace,
        '}' => TokKind::RBrace,
        '[' => TokKind::LBracket,
        ']' => TokKind::RBracket,
        ',' => TokKind::Comma,
        ';' => TokKind::Semicolon,
        '+' => TokKind::Plus,
        '*' => TokKind::Star,
        '/' => TokKind::Slash,
        '%' => TokKind::Percent,
        _ => return None,
    })
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

    /// 1 文字目を消費した後、2 文字目が `second` ならそれも消費して
    /// `paired` を返す。違えば 1 文字分の `single` を返す。`==`・`>=` のような
    /// 2 文字の綴りの判断を 1 か所に置く。
    fn two_char(&mut self, second: char, paired: TokKind, single: TokKind) -> TokKind {
        if self.peek() == Some(second) {
            self.bump();
            paired
        } else {
            single
        }
    }

    fn lex_token(&mut self) -> Result<TokKind, LexError> {
        let c = self.peek().expect("lex_token called at EOF");
        // 綴りの無い 1 文字の記号は表で引く。表に無いものだけを順に判定する。
        if let Some(kind) = single_char_token(c) {
            self.bump();
            return Ok(kind);
        }
        self.lex_compound_token(c)
    }

    /// 2 文字以上になりうる綴りと、字種で決まる字句。
    fn lex_compound_token(&mut self, c: char) -> Result<TokKind, LexError> {
        match c {
            '(' => self.lex_open_paren(),
            '#' => Err(self.err(Some("E104"), "# is not a comment marker; use //")),
            '\'' => self.lex_char(),
            '"' => self.lex_string(),
            d if d.is_ascii_digit() => self.lex_number(),
            a if a.is_ascii_alphabetic() || a == '_' => self.lex_word(),
            _ => self.lex_operator(c),
        }
    }

    fn lex_open_paren(&mut self) -> Result<TokKind, LexError> {
        self.bump();
        Ok(self.two_char(')', TokKind::Void, TokKind::LParen))
    }

    /// 記号の綴り。`...` と `--` は並びで意味が変わるので先に見る。
    /// `c` は呼び出し側が peek 済みの 1 文字目。
    fn lex_operator(&mut self, c: char) -> Result<TokKind, LexError> {
        match c {
            '.' if self.rest().starts_with("...") => {
                self.bump();
                self.bump();
                self.bump();
                Ok(TokKind::Ellipsis)
            }
            '-' if self.rest().starts_with("--") => {
                Err(self.err(Some("E104"), "-- is not a comment marker; use //"))
            }
            '.' => {
                self.bump();
                if matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    Err(self.err(Some("E103"), "float must have digits before the '.'"))
                } else {
                    Ok(TokKind::Dot)
                }
            }
            '-' => {
                self.bump();
                Ok(self.two_char('>', TokKind::DashArrow, TokKind::Minus))
            }
            ':' => {
                self.bump();
                Ok(self.two_char(':', TokKind::DoubleColon, TokKind::Colon))
            }
            '>' => {
                self.bump();
                Ok(self.two_char('=', TokKind::Ge, TokKind::Gt))
            }
            '<' => {
                self.bump();
                Ok(self.two_char('=', TokKind::Le, TokKind::Lt))
            }
            '=' => self.lex_eq_family(),
            '|' => self.lex_pipe_family(),
            '!' => self.lex_bang(),
            '&' => self.lex_ampersand(),
            other => Err(self.err(None, format!("unexpected character: {other}"))),
        }
    }

    /// `=`・`==`・`=>`。
    fn lex_eq_family(&mut self) -> Result<TokKind, LexError> {
        self.bump();
        match self.peek() {
            Some('=') => {
                self.bump();
                Ok(TokKind::EqEq)
            }
            Some('>') => {
                self.bump();
                Ok(TokKind::Arrow)
            }
            _ => Ok(TokKind::Eq),
        }
    }

    /// `|`・`||`・`|>`。
    fn lex_pipe_family(&mut self) -> Result<TokKind, LexError> {
        self.bump();
        match self.peek() {
            Some('|') => {
                self.bump();
                Ok(TokKind::OrOr)
            }
            Some('>') => {
                self.bump();
                Ok(TokKind::PipeArrow)
            }
            _ => Ok(TokKind::Pipe),
        }
    }

    /// `!=` のみ。単独の `!` は言語に無い。
    fn lex_bang(&mut self) -> Result<TokKind, LexError> {
        self.bump();
        if self.peek() == Some('=') {
            self.bump();
            Ok(TokKind::Ne)
        } else {
            Err(self.err(None, "unexpected character: !"))
        }
    }

    /// `&&` のみ。単独の `&` は言語に無い。
    fn lex_ampersand(&mut self) -> Result<TokKind, LexError> {
        self.bump();
        if self.peek() == Some('&') {
            self.bump();
            Ok(TokKind::AndAnd)
        } else {
            Err(self.err(None, "unexpected character: &"))
        }
    }

    /// 数字が続く限り消費する。基数の判定はしない（10 進のみ）。
    fn take_while_digit(&mut self) {
        while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
            self.bump();
        }
    }

    fn lex_number(&mut self) -> Result<TokKind, LexError> {
        let start = self.i;
        self.take_while_digit();
        if self.peek() != Some('.') {
            return self.finish_int(start);
        }
        self.bump();
        if !matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
            return Err(self.err(Some("E103"), "float must have digits after the '.'"));
        }
        self.take_while_digit();
        self.finish_float(start)
    }

    /// 小数点を含む綴りを Float にする。範囲外は parse が失敗する。
    fn finish_float(&mut self, start: usize) -> Result<TokKind, LexError> {
        let text = &self.src[start..self.i];
        let value: f64 = text
            .parse()
            .map_err(|_| self.err(None, format!("bad float literal: {text}")))?;
        Ok(TokKind::Float(value))
    }

    /// 小数点を含まない綴りを Int にする。i64 を超えると E112。
    fn finish_int(&mut self, start: usize) -> Result<TokKind, LexError> {
        let text = &self.src[start..self.i];
        let value: i64 = text
            .parse()
            .map_err(|_| self.err(Some("E112"), format!("integer out of range: {text}")))?;
        Ok(TokKind::Int(value))
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
                        _ => {
                            return Err(LexError {
                                line: escape.line,
                                col: escape.col,
                                code: Some("E110"),
                                message: "unsupported escape in string literal".into(),
                            })
                        }
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
        let kind = match word {
            "def" => TokKind::Def,
            "rec" => TokKind::KwRec,
            "let" => TokKind::KwLet,
            "do" => TokKind::KwDo,
            "fn" => TokKind::KwFn,
            "Fn" => TokKind::FnType,
            "if" => TokKind::KwIf,
            "module" => TokKind::KwModule,
            "type" => TokKind::KwType,
            "implement" => TokKind::KwImplement,
            "effect" => TokKind::KwEffect,
            "handler" => TokKind::KwHandler,
            "with" => TokKind::KwWith,
            "resume" => TokKind::KwResume,
            "return" => TokKind::KwReturn,
            "once" => TokKind::KwOnce,
            "abort" => TokKind::KwAbort,
            "again" => TokKind::KwAgain,
            "as" => TokKind::KwAs,
            "library" => TokKind::KwLibrary,
            "shared" => TokKind::KwShared,
            "Pure" => TokKind::KwPure,
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
            vec![
                TokKind::Int(1),
                TokKind::Plus,
                TokKind::Int(2),
                TokKind::Eof
            ]
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
            vec![
                TokKind::Minus,
                TokKind::Minus,
                TokKind::Int(2),
                TokKind::Eof
            ]
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
